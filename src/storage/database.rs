use std::fs;
use std::path::Path;

use chrono::Utc;
use rusqlite::Connection;

use crate::errors::Result;
use crate::paths;

const CURRENT_SCHEMA_VERSION: i32 = 2;

pub fn open_default() -> Result<Connection> {
    let data_dir = paths::data_dir()?;
    fs::create_dir_all(&data_dir)?;
    open_path(&data_dir.join("release-weave.db"))
}

pub fn open_path(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(path)?;
    init_connection(&mut conn)?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    init_connection(&mut conn)?;
    Ok(conn)
}

fn init_connection(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    run_migrations(conn)?;
    Ok(())
}

pub fn run_migrations(conn: &mut Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;

    let current_version: Option<i32> = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
            row.get(0)
        })
        .unwrap_or(None);

    let version = current_version.unwrap_or(0);

    if version < 1 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                repository_path TEXT NOT NULL UNIQUE,
                default_branch TEXT NOT NULL,
                default_template_id TEXT,
                settings_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS releases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                version TEXT NOT NULL,
                previous_version TEXT,
                start_commit TEXT,
                end_commit TEXT NOT NULL,
                status TEXT NOT NULL,
                release_notes TEXT NOT NULL,
                model_metadata TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS change_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                release_id INTEGER NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
                commit_hash TEXT,
                category TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                source_refs_json TEXT NOT NULL,
                origin_json TEXT NOT NULL,
                confidence REAL NOT NULL,
                is_included INTEGER NOT NULL DEFAULT 1
            );

            CREATE TABLE IF NOT EXISTS readiness_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                release_id INTEGER NOT NULL REFERENCES releases(id) ON DELETE CASCADE,
                check_type TEXT NOT NULL,
                status TEXT NOT NULL,
                required INTEGER NOT NULL,
                details TEXT,
                executed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )?;

        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![1, Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
    }

    if version < 2 {
        let tx = conn.transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS packages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                manifest_path TEXT NOT NULL,
                current_version TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(project_id, name)
            );",
        )?;

        let mut has_package_id = false;
        {
            let mut stmt = tx.prepare("PRAGMA table_info(releases)")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let col_name: String = row.get(1)?;
                if col_name == "package_id" {
                    has_package_id = true;
                    break;
                }
            }
        }

        if !has_package_id {
            tx.execute(
                "ALTER TABLE releases ADD COLUMN package_id INTEGER REFERENCES packages(id) ON DELETE CASCADE",
                [],
            )?;
        }

        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![2, Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_and_are_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        run_migrations(&mut conn).unwrap();

        // Second run must be completely idempotent
        run_migrations(&mut conn).unwrap();

        let version: i32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |row| {
                row.get(0)
            })
            .unwrap();

        assert_eq!(version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migration_from_v1_preserves_existing_releases() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Manually setup v1 state
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        conn.execute(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                repository_path TEXT NOT NULL UNIQUE,
                default_branch TEXT NOT NULL,
                default_template_id TEXT,
                settings_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE releases (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                version TEXT NOT NULL,
                previous_version TEXT,
                start_commit TEXT,
                end_commit TEXT NOT NULL,
                status TEXT NOT NULL,
                release_notes TEXT NOT NULL,
                model_metadata TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, repository_path, default_branch, settings_json, created_at, updated_at)
             VALUES (1, 'demo', '/tmp/demo', 'main', '{}', '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO releases (id, project_id, version, end_commit, status, release_notes, created_at)
             VALUES (10, 1, '1.0.0', 'c123', 'Final', 'Notes', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (1, '2026-01-01')",
            [],
        )
        .unwrap();

        // Now run migrations to bring it to v2
        run_migrations(&mut conn).unwrap();

        let v: i32 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);

        // Verify existing release still exists and has NULL package_id
        let pkg_id: Option<i64> = conn
            .query_row("SELECT package_id FROM releases WHERE id = 10", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(pkg_id, None);
    }

    #[test]
    fn tables_created_with_foreign_keys_active() {
        let conn = open_in_memory().unwrap();
        let fk_enabled: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }
}
