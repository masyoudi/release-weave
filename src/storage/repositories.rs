use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::check::{CheckResult, CheckStatus};
use crate::domain::commit::ChangeCategory;
use crate::domain::package::Package;
use crate::domain::project::{Project, ProjectSettings};
use crate::domain::provenance::Origin;
use crate::domain::release::{ChangeItem, Release, ReleaseStatus, SourceRef};
use crate::errors::{AppError, Result};

pub struct ProjectRepo;

impl ProjectRepo {
    pub fn insert(conn: &Connection, project: &Project) -> Result<i64> {
        let settings_json = serde_json::to_string(&project.settings)
            .map_err(|e| AppError::Validation(format!("Invalid settings JSON: {e}")))?;

        conn.execute(
            "INSERT INTO projects (
                name, repository_path, default_branch, default_template_id,
                settings_json, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                project.name,
                project.repository_path.to_string_lossy().to_string(),
                project.default_branch,
                project.default_template_id,
                settings_json,
                project.created_at.to_rfc3339(),
                project.updated_at.to_rfc3339(),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn list(conn: &Connection) -> Result<Vec<Project>> {
        let mut stmt = conn.prepare(
            "SELECT id, name, repository_path, default_branch, default_template_id,
                    settings_json, created_at, updated_at
             FROM projects ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let name: String = row.get(1)?;
            let repo_path_str: String = row.get(2)?;
            let default_branch: String = row.get(3)?;
            let default_template_id: Option<String> = row.get(4)?;
            let settings_json: String = row.get(5)?;
            let created_at_str: String = row.get(6)?;
            let updated_at_str: String = row.get(7)?;

            let settings = serde_json::from_str(&settings_json).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Project {
                id: Some(id),
                name,
                repository_path: PathBuf::from(repo_path_str),
                default_branch,
                default_template_id,
                settings,
                created_at,
                updated_at,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn get(conn: &Connection, id: i64) -> Result<Option<Project>> {
        conn.query_row(
            "SELECT id, name, repository_path, default_branch, default_template_id,
                    settings_json, created_at, updated_at
             FROM projects WHERE id = ?1",
            params![id],
            |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                let repo_path_str: String = row.get(2)?;
                let default_branch: String = row.get(3)?;
                let default_template_id: Option<String> = row.get(4)?;
                let settings_json: String = row.get(5)?;
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                let settings = serde_json::from_str(&settings_json).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Project {
                    id: Some(id),
                    name,
                    repository_path: PathBuf::from(repo_path_str),
                    default_branch,
                    default_template_id,
                    settings,
                    created_at,
                    updated_at,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn get_by_path(conn: &Connection, path: &Path) -> Result<Option<Project>> {
        let path_str = path.to_string_lossy().to_string();
        conn.query_row(
            "SELECT id, name, repository_path, default_branch, default_template_id,
                    settings_json, created_at, updated_at
             FROM projects WHERE repository_path = ?1",
            params![path_str],
            |row| {
                let id: i64 = row.get(0)?;
                let name: String = row.get(1)?;
                let repo_path_str: String = row.get(2)?;
                let default_branch: String = row.get(3)?;
                let default_template_id: Option<String> = row.get(4)?;
                let settings_json: String = row.get(5)?;
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                let settings = serde_json::from_str(&settings_json).unwrap_or_default();
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Project {
                    id: Some(id),
                    name,
                    repository_path: PathBuf::from(repo_path_str),
                    default_branch,
                    default_template_id,
                    settings,
                    created_at,
                    updated_at,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn remove(conn: &Connection, id: i64) -> Result<bool> {
        let count = conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }

    pub fn update_settings(
        conn: &Connection,
        project_id: i64,
        settings: &ProjectSettings,
    ) -> Result<()> {
        let settings_json = serde_json::to_string(settings)
            .map_err(|e| AppError::Validation(format!("Invalid settings JSON: {e}")))?;

        conn.execute(
            "UPDATE projects SET settings_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![settings_json, Utc::now().to_rfc3339(), project_id],
        )?;

        Ok(())
    }
}

pub struct ReleaseRepo;

impl ReleaseRepo {
    pub fn insert(conn: &Connection, release: &Release) -> Result<i64> {
        let status_str = match release.status {
            ReleaseStatus::Draft => "draft",
            ReleaseStatus::Final => "final",
        };

        conn.execute(
            "INSERT INTO releases (
                project_id, package_id, version, previous_version, start_commit, end_commit,
                status, release_notes, model_metadata, created_at, completed_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                release.project_id,
                release.package_id,
                release.version,
                release.previous_version,
                release.start_commit,
                release.end_commit,
                status_str,
                release.release_notes,
                release.model_metadata,
                release.created_at.to_rfc3339(),
                release.completed_at.map(|dt| dt.to_rfc3339()),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    pub fn update_notes_and_status(
        conn: &Connection,
        release_id: i64,
        status: ReleaseStatus,
        notes: &str,
    ) -> Result<()> {
        let status_str = match status {
            ReleaseStatus::Draft => "draft",
            ReleaseStatus::Final => "final",
        };
        let completed_at = match status {
            ReleaseStatus::Final => Some(Utc::now().to_rfc3339()),
            ReleaseStatus::Draft => None,
        };

        conn.execute(
            "UPDATE releases SET status = ?1, release_notes = ?2, completed_at = ?3 WHERE id = ?4",
            params![status_str, notes, completed_at, release_id],
        )?;

        Ok(())
    }

    pub fn get(conn: &Connection, id: i64) -> Result<Option<Release>> {
        conn.query_row(
            "SELECT id, project_id, package_id, version, previous_version, start_commit, end_commit,
                    status, release_notes, model_metadata, created_at, completed_at
             FROM releases WHERE id = ?1",
            params![id],
            |row| {
                let id: i64 = row.get(0)?;
                let project_id: i64 = row.get(1)?;
                let package_id: Option<i64> = row.get(2)?;
                let version: String = row.get(3)?;
                let previous_version: Option<String> = row.get(4)?;
                let start_commit: Option<String> = row.get(5)?;
                let end_commit: String = row.get(6)?;
                let status_str: String = row.get(7)?;
                let release_notes: String = row.get(8)?;
                let model_metadata: Option<String> = row.get(9)?;
                let created_at_str: String = row.get(10)?;
                let completed_at_str: Option<String> = row.get(11)?;

                let status = match status_str.as_str() {
                    "final" => ReleaseStatus::Final,
                    _ => ReleaseStatus::Draft,
                };
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let completed_at = completed_at_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .ok()
                });

                Ok(Release {
                    id: Some(id),
                    project_id,
                    package_id,
                    version,
                    previous_version,
                    start_commit,
                    end_commit,
                    status,
                    release_notes,
                    model_metadata,
                    created_at,
                    completed_at,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn list_by_project(conn: &Connection, project_id: i64) -> Result<Vec<Release>> {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, package_id, version, previous_version, start_commit, end_commit,
                    status, release_notes, model_metadata, created_at, completed_at
             FROM releases WHERE project_id = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![project_id], |row| {
            let id: i64 = row.get(0)?;
            let project_id: i64 = row.get(1)?;
            let package_id: Option<i64> = row.get(2)?;
            let version: String = row.get(3)?;
            let previous_version: Option<String> = row.get(4)?;
            let start_commit: Option<String> = row.get(5)?;
            let end_commit: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let release_notes: String = row.get(8)?;
            let model_metadata: Option<String> = row.get(9)?;
            let created_at_str: String = row.get(10)?;
            let completed_at_str: Option<String> = row.get(11)?;

            let status = match status_str.as_str() {
                "final" => ReleaseStatus::Final,
                _ => ReleaseStatus::Draft,
            };
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let completed_at = completed_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            Ok(Release {
                id: Some(id),
                project_id,
                package_id,
                version,
                previous_version,
                start_commit,
                end_commit,
                status,
                release_notes,
                model_metadata,
                created_at,
                completed_at,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn list_by_package(
        conn: &Connection,
        project_id: i64,
        package_id: i64,
    ) -> Result<Vec<Release>> {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, package_id, version, previous_version, start_commit, end_commit,
                    status, release_notes, model_metadata, created_at, completed_at
             FROM releases WHERE project_id = ?1 AND package_id = ?2 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(params![project_id, package_id], |row| {
            let id: i64 = row.get(0)?;
            let project_id: i64 = row.get(1)?;
            let package_id: Option<i64> = row.get(2)?;
            let version: String = row.get(3)?;
            let previous_version: Option<String> = row.get(4)?;
            let start_commit: Option<String> = row.get(5)?;
            let end_commit: String = row.get(6)?;
            let status_str: String = row.get(7)?;
            let release_notes: String = row.get(8)?;
            let model_metadata: Option<String> = row.get(9)?;
            let created_at_str: String = row.get(10)?;
            let completed_at_str: Option<String> = row.get(11)?;

            let status = match status_str.as_str() {
                "final" => ReleaseStatus::Final,
                _ => ReleaseStatus::Draft,
            };
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let completed_at = completed_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            Ok(Release {
                id: Some(id),
                project_id,
                package_id,
                version,
                previous_version,
                start_commit,
                end_commit,
                status,
                release_notes,
                model_metadata,
                created_at,
                completed_at,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn update_metadata(conn: &Connection, release_id: i64, metadata: &str) -> Result<()> {
        conn.execute(
            "UPDATE releases SET model_metadata = ?1 WHERE id = ?2",
            params![metadata, release_id],
        )?;
        Ok(())
    }

    pub fn save_change_items(
        conn: &mut Connection,
        release_id: i64,
        items: &[ChangeItem],
    ) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM change_items WHERE release_id = ?1",
            params![release_id],
        )?;

        for item in items {
            let refs_json = serde_json::to_string(&item.source_refs)
                .map_err(|e| AppError::Validation(format!("Invalid source_refs JSON: {e}")))?;
            let origin_json = serde_json::to_string(&item.origin)
                .map_err(|e| AppError::Validation(format!("Invalid origin JSON: {e}")))?;

            tx.execute(
                "INSERT INTO change_items (
                    release_id, commit_hash, category, title, description,
                    source_refs_json, origin_json, confidence, is_included
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    release_id,
                    item.commit_hash,
                    item.category.as_str(),
                    item.title,
                    item.description,
                    refs_json,
                    origin_json,
                    item.confidence,
                    if item.is_included { 1 } else { 0 },
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn list_change_items(conn: &Connection, release_id: i64) -> Result<Vec<ChangeItem>> {
        let mut stmt = conn.prepare(
            "SELECT id, release_id, commit_hash, category, title, description,
                    source_refs_json, origin_json, confidence, is_included
             FROM change_items WHERE release_id = ?1 ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![release_id], |row| {
            let id: i64 = row.get(0)?;
            let release_id: i64 = row.get(1)?;
            let commit_hash: Option<String> = row.get(2)?;
            let category_str: String = row.get(3)?;
            let title: String = row.get(4)?;
            let description: Option<String> = row.get(5)?;
            let source_refs_json: String = row.get(6)?;
            let origin_json: String = row.get(7)?;
            let confidence: f32 = row.get(8)?;
            let is_included_int: i32 = row.get(9)?;

            let category = match category_str.as_str() {
                "feature" => ChangeCategory::Feature,
                "fix" => ChangeCategory::Fix,
                "refactor" => ChangeCategory::Refactor,
                "docs" => ChangeCategory::Docs,
                "build" => ChangeCategory::Build,
                "perf" => ChangeCategory::Perf,
                "test" => ChangeCategory::Test,
                "chore" => ChangeCategory::Chore,
                _ => ChangeCategory::Other,
            };

            let source_refs: Vec<SourceRef> =
                serde_json::from_str(&source_refs_json).unwrap_or_default();
            let origin: Origin = serde_json::from_str(&origin_json).unwrap_or_default();

            Ok(ChangeItem {
                id: Some(id),
                release_id: Some(release_id),
                commit_hash,
                category,
                title,
                description,
                source_refs,
                origin,
                confidence,
                is_included: is_included_int == 1,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}

pub struct ReadinessRepo;

impl ReadinessRepo {
    pub fn save_checks(
        conn: &mut Connection,
        release_id: i64,
        checks: &[CheckResult],
    ) -> Result<()> {
        let tx = conn.transaction()?;
        tx.execute(
            "DELETE FROM readiness_checks WHERE release_id = ?1",
            params![release_id],
        )?;

        for check in checks {
            tx.execute(
                "INSERT INTO readiness_checks (
                    release_id, check_type, status, required, details, executed_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    release_id,
                    check.check_type,
                    check.status.as_str(),
                    if check.required { 1 } else { 0 },
                    check.details,
                    check.executed_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn list_checks(conn: &Connection, release_id: i64) -> Result<Vec<CheckResult>> {
        let mut stmt = conn.prepare(
            "SELECT id, release_id, check_type, status, required, details, executed_at
             FROM readiness_checks WHERE release_id = ?1 ORDER BY id ASC",
        )?;

        let rows = stmt.query_map(params![release_id], |row| {
            let id: i64 = row.get(0)?;
            let release_id: i64 = row.get(1)?;
            let check_type: String = row.get(2)?;
            let status_str: String = row.get(3)?;
            let required_int: i32 = row.get(4)?;
            let details: Option<String> = row.get(5)?;
            let executed_at_str: Option<String> = row.get(6)?;

            let status = match status_str.as_str() {
                "passed" => CheckStatus::Passed,
                "warning" => CheckStatus::Warning,
                "failed" => CheckStatus::Failed,
                "skipped" => CheckStatus::Skipped,
                _ => CheckStatus::NotRun,
            };

            let executed_at = executed_at_str.and_then(|s| {
                DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            });

            Ok(CheckResult {
                id: Some(id),
                release_id: Some(release_id),
                check_type,
                status,
                required: required_int == 1,
                details,
                executed_at,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }
}

pub struct PackageRepo;

impl PackageRepo {
    pub fn insert(conn: &Connection, package: &Package) -> Result<i64> {
        conn.execute(
            "INSERT INTO packages (
                project_id, name, relative_path, manifest_path, current_version, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                package.project_id,
                package.name,
                package.relative_path,
                package.manifest_path,
                package.current_version,
                package.created_at.to_rfc3339(),
                package.updated_at.to_rfc3339(),
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn upsert(conn: &Connection, package: &Package) -> Result<i64> {
        conn.execute(
            "INSERT INTO packages (
                project_id, name, relative_path, manifest_path, current_version, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(project_id, name) DO UPDATE SET
                relative_path = excluded.relative_path,
                manifest_path = excluded.manifest_path,
                current_version = excluded.current_version,
                updated_at = excluded.updated_at",
            params![
                package.project_id,
                package.name,
                package.relative_path,
                package.manifest_path,
                package.current_version,
                package.created_at.to_rfc3339(),
                package.updated_at.to_rfc3339(),
            ],
        )?;

        let id: i64 = conn.query_row(
            "SELECT id FROM packages WHERE project_id = ?1 AND name = ?2",
            params![package.project_id, package.name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn list_by_project(conn: &Connection, project_id: i64) -> Result<Vec<Package>> {
        let mut stmt = conn.prepare(
            "SELECT id, project_id, name, relative_path, manifest_path, current_version, created_at, updated_at
             FROM packages WHERE project_id = ?1 ORDER BY name ASC",
        )?;

        let rows = stmt.query_map(params![project_id], |row| {
            let id: i64 = row.get(0)?;
            let project_id: i64 = row.get(1)?;
            let name: String = row.get(2)?;
            let relative_path: String = row.get(3)?;
            let manifest_path: String = row.get(4)?;
            let current_version: Option<String> = row.get(5)?;
            let created_at_str: String = row.get(6)?;
            let updated_at_str: String = row.get(7)?;

            let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Package {
                id: Some(id),
                project_id,
                name,
                relative_path,
                manifest_path,
                current_version,
                created_at,
                updated_at,
            })
        })?;

        let mut list = Vec::new();
        for item in rows {
            list.push(item?);
        }
        Ok(list)
    }

    pub fn find_by_name(conn: &Connection, project_id: i64, name: &str) -> Result<Option<Package>> {
        conn.query_row(
            "SELECT id, project_id, name, relative_path, manifest_path, current_version, created_at, updated_at
             FROM packages WHERE project_id = ?1 AND name = ?2",
            params![project_id, name],
            |row| {
                let id: i64 = row.get(0)?;
                let project_id: i64 = row.get(1)?;
                let name: String = row.get(2)?;
                let relative_path: String = row.get(3)?;
                let manifest_path: String = row.get(4)?;
                let current_version: Option<String> = row.get(5)?;
                let created_at_str: String = row.get(6)?;
                let updated_at_str: String = row.get(7)?;

                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Package {
                    id: Some(id),
                    project_id,
                    name,
                    relative_path,
                    manifest_path,
                    current_version,
                    created_at,
                    updated_at,
                })
            },
        )
        .optional()
        .map_err(AppError::from)
    }

    pub fn delete(conn: &Connection, id: i64) -> Result<bool> {
        let count = conn.execute("DELETE FROM packages WHERE id = ?1", params![id])?;
        Ok(count > 0)
    }
}

pub struct SettingsRepo;

impl SettingsRepo {
    const GLOBAL_KEY: &'static str = "global_settings";

    pub fn get_global_settings(conn: &Connection) -> Result<ProjectSettings> {
        let json_opt: Option<String> = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key = ?1",
                params![Self::GLOBAL_KEY],
                |row| row.get(0),
            )
            .optional()?;

        match json_opt {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| AppError::Validation(format!("Invalid global settings JSON: {e}"))),
            None => Ok(ProjectSettings::default()),
        }
    }

    pub fn set_global_settings(conn: &Connection, settings: &ProjectSettings) -> Result<()> {
        let json = serde_json::to_string(settings)
            .map_err(|e| AppError::Validation(format!("Failed to serialize settings: {e}")))?;

        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = ?2, updated_at = ?3",
            params![Self::GLOBAL_KEY, json, Utc::now().to_rfc3339()],
        )?;

        Ok(())
    }

    pub fn get_merged_settings(conn: &Connection, project_id: i64) -> Result<ProjectSettings> {
        let global = Self::get_global_settings(conn)?;
        let project = ProjectRepo::get(conn, project_id)?;

        let Some(project) = project else {
            return Ok(global);
        };

        let p_settings = project.settings;

        Ok(ProjectSettings {
            ollama_url: p_settings.ollama_url.or(global.ollama_url),
            model: p_settings.model.or(global.model),
            context_limit: p_settings.context_limit.or(global.context_limit),
            readiness_commands: if !p_settings.readiness_commands.is_empty() {
                p_settings.readiness_commands
            } else {
                global.readiness_commands
            },
            language: p_settings.language.or(global.language),
            commands_required: p_settings.commands_required,
            allow_git_tag: p_settings.allow_git_tag,
            package_tag_pattern: p_settings
                .package_tag_pattern
                .or(global.package_tag_pattern),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;
    use crate::storage::database::open_in_memory;

    #[test]
    fn project_crud_operations_and_leaves_filesystem_untouched() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().join("my-repo");
        fs::create_dir_all(&repo_path).unwrap();

        let conn = open_in_memory().unwrap();

        let project = Project::new("My Repo", &repo_path, "main");
        let id = ProjectRepo::insert(&conn, &project).unwrap();
        assert!(id > 0);

        let fetched = ProjectRepo::get(&conn, id).unwrap().expect("project found");
        assert_eq!(fetched.name, "My Repo");
        assert_eq!(fetched.repository_path, repo_path);

        let list = ProjectRepo::list(&conn).unwrap();
        assert_eq!(list.len(), 1);

        // Deleting project from DB must never delete the underlying directory
        let removed = ProjectRepo::remove(&conn, id).unwrap();
        assert!(removed);
        assert!(
            repo_path.exists(),
            "directory must still exist after DB removal"
        );
        assert!(ProjectRepo::get(&conn, id).unwrap().is_none());
    }

    #[test]
    fn release_and_change_items_persistence() {
        let mut conn = open_in_memory().unwrap();
        let dir = tempdir().unwrap();
        let proj_id = ProjectRepo::insert(&conn, &Project::new("P", dir.path(), "main")).unwrap();

        let draft = Release::new_draft(proj_id, "1.0.0", None, None, "commit123");
        let rel_id = ReleaseRepo::insert(&conn, &draft).unwrap();

        let fetched_draft = ReleaseRepo::get(&conn, rel_id).unwrap().unwrap();
        assert_eq!(fetched_draft.status, ReleaseStatus::Draft);
        assert!(fetched_draft.completed_at.is_none());

        // Save change items
        let item =
            ChangeItem::new_deterministic("commit123", ChangeCategory::Feature, "Initial feature");
        ReleaseRepo::save_change_items(&mut conn, rel_id, &[item]).unwrap();

        let items = ReleaseRepo::list_change_items(&conn, rel_id).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Initial feature");
        assert_eq!(items[0].category, ChangeCategory::Feature);

        // Finalize release
        ReleaseRepo::update_notes_and_status(
            &conn,
            rel_id,
            ReleaseStatus::Final,
            "# Release 1.0.0",
        )
        .unwrap();
        let finalized = ReleaseRepo::get(&conn, rel_id).unwrap().unwrap();
        assert_eq!(finalized.status, ReleaseStatus::Final);
        assert!(finalized.completed_at.is_some());
    }

    #[test]
    fn readiness_checks_roundtrip() {
        let mut conn = open_in_memory().unwrap();
        let dir = tempdir().unwrap();
        let proj_id = ProjectRepo::insert(&conn, &Project::new("P", dir.path(), "main")).unwrap();
        let rel_id = ReleaseRepo::insert(
            &conn,
            &Release::new_draft(proj_id, "1.0.0", None, None, "c1"),
        )
        .unwrap();

        let checks = vec![
            CheckResult::new("working_tree_clean", true).pass("Clean tree"),
            CheckResult::new("cargo_test", true).fail("1 test failed"),
            CheckResult::new("optional_lint", false).warn("warnings present"),
            CheckResult::new("changelog", true).skip("not configured"),
            CheckResult::new("not_executed_check", true),
        ];

        ReadinessRepo::save_checks(&mut conn, rel_id, &checks).unwrap();
        let loaded = ReadinessRepo::list_checks(&conn, rel_id).unwrap();

        assert_eq!(loaded.len(), 5);
        assert_eq!(loaded[0].status, CheckStatus::Passed);
        assert_eq!(loaded[1].status, CheckStatus::Failed);
        assert_eq!(loaded[2].status, CheckStatus::Warning);
        assert_eq!(loaded[3].status, CheckStatus::Skipped);
        assert_eq!(loaded[4].status, CheckStatus::NotRun);
    }

    #[test]
    fn settings_global_and_project_override() {
        let conn = open_in_memory().unwrap();

        let global = ProjectSettings {
            ollama_url: Some("http://localhost:11434".to_string()),
            model: Some("qwen2.5-coder:7b".to_string()),
            context_limit: Some(4096),
            readiness_commands: vec!["cargo check".to_string()],
            language: Some("en".to_string()),
            commands_required: false,
            allow_git_tag: false,
            package_tag_pattern: None,
        };
        SettingsRepo::set_global_settings(&conn, &global).unwrap();

        let dir = tempdir().unwrap();
        let mut proj = Project::new("Override Project", dir.path(), "main");
        // Project overrides only model, language, and allow_git_tag
        proj.settings.model = Some("deepseek-coder:6.7b".to_string());
        proj.settings.language = Some("id".to_string());
        proj.settings.allow_git_tag = true;

        let proj_id = ProjectRepo::insert(&conn, &proj).unwrap();

        let merged = SettingsRepo::get_merged_settings(&conn, proj_id).unwrap();
        assert_eq!(merged.model.as_deref(), Some("deepseek-coder:6.7b"));
        assert_eq!(merged.language.as_deref(), Some("id"));
        assert_eq!(merged.ollama_url.as_deref(), Some("http://localhost:11434"));
        assert_eq!(merged.context_limit, Some(4096));
        assert_eq!(merged.readiness_commands, vec!["cargo check".to_string()]);
        assert!(merged.allow_git_tag);
    }

    #[test]
    fn package_crud_and_release_package_association() {
        let conn = open_in_memory().unwrap();
        let dir = tempdir().unwrap();
        let proj = Project::new("MonoRepo", dir.path(), "main");
        let proj_id = ProjectRepo::insert(&conn, &proj).unwrap();

        let pkg = Package::new(
            proj_id,
            "core-engine",
            "packages/core",
            "packages/core/Cargo.toml",
            Some("1.0.0".to_string()),
        );

        let pkg_id = PackageRepo::insert(&conn, &pkg).unwrap();
        assert!(pkg_id > 0);

        let list = PackageRepo::list_by_project(&conn, proj_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "core-engine");
        assert_eq!(list[0].relative_path, "packages/core");

        // Upsert update
        let mut updated = pkg.clone();
        updated.current_version = Some("1.1.0".to_string());
        let upserted_id = PackageRepo::upsert(&conn, &updated).unwrap();
        assert_eq!(upserted_id, pkg_id);

        let found = PackageRepo::find_by_name(&conn, proj_id, "core-engine")
            .unwrap()
            .expect("Package should be found");
        assert_eq!(found.current_version.as_deref(), Some("1.1.0"));

        // Release associated with package
        let rel = Release::new_draft(
            proj_id,
            "1.1.0",
            Some("1.0.0".to_string()),
            Some("c1".to_string()),
            "c2",
        )
        .with_package_id(Some(pkg_id));

        let rel_id = ReleaseRepo::insert(&conn, &rel).unwrap();
        let loaded = ReleaseRepo::get(&conn, rel_id).unwrap().unwrap();
        assert_eq!(loaded.package_id, Some(pkg_id));

        let pkg_releases = ReleaseRepo::list_by_package(&conn, proj_id, pkg_id).unwrap();
        assert_eq!(pkg_releases.len(), 1);
        assert_eq!(pkg_releases[0].version, "1.1.0");

        // Delete package
        let deleted = PackageRepo::delete(&conn, pkg_id).unwrap();
        assert!(deleted);
        assert!(
            PackageRepo::list_by_project(&conn, proj_id)
                .unwrap()
                .is_empty()
        );
    }
}
