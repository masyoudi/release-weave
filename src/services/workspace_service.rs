use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestKind {
    Cargo,
    PackageJson,
}

impl ManifestKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            ManifestKind::Cargo => "Cargo.toml",
            ManifestKind::PackageJson => "package.json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceMember {
    pub name: String,
    pub relative_path: String,
    pub manifest_path: String,
    pub manifest_kind: ManifestKind,
    pub current_version: Option<String>,
}

pub struct WorkspaceService;

impl WorkspaceService {
    /// Detects monorepo workspace members across Cargo, pnpm, and npm/yarn configurations.
    /// Returns an empty Vec if no monorepo workspace configuration is found.
    pub fn detect_workspace(repo_root: &Path) -> Result<Vec<WorkspaceMember>> {
        let mut members = Vec::new();

        // 1. Check Cargo Workspace ([workspace] in Cargo.toml)
        let cargo_toml = repo_root.join("Cargo.toml");
        if cargo_toml.is_file()
            && let Ok(cargo_members) = Self::detect_cargo_workspace(repo_root, &cargo_toml)
        {
            members.extend(cargo_members);
        }

        // 2. Check pnpm Workspace (pnpm-workspace.yaml)
        let pnpm_yaml = repo_root.join("pnpm-workspace.yaml");
        if pnpm_yaml.is_file()
            && let Ok(pnpm_members) = Self::detect_pnpm_workspace(repo_root, &pnpm_yaml)
        {
            members.extend(pnpm_members);
        }

        // 3. Check npm / yarn Workspaces ("workspaces" in root package.json)
        let root_package_json = repo_root.join("package.json");
        if root_package_json.is_file()
            && let Ok(npm_members) = Self::detect_npm_workspace(repo_root, &root_package_json)
        {
            members.extend(npm_members);
        }

        // Deduplicate and sort by member name
        members.sort_by(|a, b| a.name.cmp(&b.name));
        members.dedup_by(|a, b| a.relative_path == b.relative_path);

        Ok(members)
    }

    fn detect_cargo_workspace(
        repo_root: &Path,
        cargo_toml_path: &Path,
    ) -> Result<Vec<WorkspaceMember>> {
        let content = fs::read_to_string(cargo_toml_path)?;
        let doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| AppError::Validation(format!("Failed to parse root Cargo.toml: {e}")))?;

        let Some(workspace) = doc.get("workspace").and_then(|w| w.as_table_like()) else {
            return Ok(Vec::new());
        };

        let Some(members_item) = workspace.get("members").and_then(|m| m.as_array()) else {
            return Ok(Vec::new());
        };

        let mut patterns = Vec::new();
        for val in members_item.iter() {
            if let Some(s) = val.as_str() {
                patterns.push(s.to_string());
            }
        }

        let mut members = Vec::new();
        for pattern in patterns {
            let matched_dirs = Self::expand_glob(repo_root, &pattern);
            for dir in matched_dirs {
                let member_cargo = dir.join("Cargo.toml");
                if member_cargo.is_file()
                    && let Ok(member) = Self::parse_cargo_member(repo_root, &member_cargo)
                {
                    members.push(member);
                }
            }
        }

        Ok(members)
    }

    fn parse_cargo_member(repo_root: &Path, member_manifest: &Path) -> Result<WorkspaceMember> {
        let content = fs::read_to_string(member_manifest)?;
        let doc: toml_edit::DocumentMut = content
            .parse()
            .map_err(|e| AppError::Validation(format!("Failed to parse member Cargo.toml: {e}")))?;

        let pkg = doc
            .get("package")
            .and_then(|p| p.as_table_like())
            .ok_or_else(|| {
                AppError::Validation("Missing [package] table in member Cargo.toml".to_string())
            })?;

        let name = pkg
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| {
                AppError::Validation("Missing package name in member Cargo.toml".to_string())
            })?
            .to_string();

        let version = pkg
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let member_dir = member_manifest
            .parent()
            .ok_or_else(|| AppError::Validation("Invalid member manifest parent".to_string()))?;
        let relative_path = member_dir
            .strip_prefix(repo_root)
            .map_err(|e| AppError::Validation(format!("Path prefix error: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        let rel_manifest = member_manifest
            .strip_prefix(repo_root)
            .map_err(|e| AppError::Validation(format!("Path prefix error: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        Ok(WorkspaceMember {
            name,
            relative_path,
            manifest_path: rel_manifest,
            manifest_kind: ManifestKind::Cargo,
            current_version: version,
        })
    }

    fn detect_pnpm_workspace(
        repo_root: &Path,
        pnpm_yaml_path: &Path,
    ) -> Result<Vec<WorkspaceMember>> {
        let content = fs::read_to_string(pnpm_yaml_path)?;
        let mut patterns = Vec::new();
        let mut in_packages = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("packages:") {
                in_packages = true;
                continue;
            }

            if in_packages {
                if trimmed.starts_with('-') {
                    let raw = trimmed.trim_start_matches('-').trim();
                    let unquoted = raw.trim_matches('\'').trim_matches('"');
                    if !unquoted.is_empty() {
                        patterns.push(unquoted.to_string());
                    }
                } else if !trimmed.is_empty() && !trimmed.starts_with('#') {
                    // Encountered another top-level key
                    in_packages = false;
                }
            }
        }

        let mut members = Vec::new();
        for pattern in patterns {
            let matched_dirs = Self::expand_glob(repo_root, &pattern);
            for dir in matched_dirs {
                let pkg_json = dir.join("package.json");
                if pkg_json.is_file()
                    && let Ok(member) = Self::parse_package_json_member(repo_root, &pkg_json)
                {
                    members.push(member);
                }
            }
        }

        Ok(members)
    }

    fn detect_npm_workspace(
        repo_root: &Path,
        package_json_path: &Path,
    ) -> Result<Vec<WorkspaceMember>> {
        let content = fs::read_to_string(package_json_path)?;
        let val: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| AppError::Validation(format!("Failed to parse root package.json: {e}")))?;

        let mut patterns = Vec::new();
        if let Some(arr) = val.get("workspaces").and_then(|w| w.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    patterns.push(s.to_string());
                }
            }
        } else if let Some(arr) = val
            .get("workspaces")
            .and_then(|w| w.get("packages"))
            .and_then(|p| p.as_array())
        {
            for item in arr {
                if let Some(s) = item.as_str() {
                    patterns.push(s.to_string());
                }
            }
        }

        if patterns.is_empty() {
            return Ok(Vec::new());
        }

        let mut members = Vec::new();
        for pattern in patterns {
            let matched_dirs = Self::expand_glob(repo_root, &pattern);
            for dir in matched_dirs {
                // Ensure we don't treat repo root itself as a sub-member
                if dir == repo_root {
                    continue;
                }
                let pkg_json = dir.join("package.json");
                if pkg_json.is_file()
                    && let Ok(member) = Self::parse_package_json_member(repo_root, &pkg_json)
                {
                    members.push(member);
                }
            }
        }

        Ok(members)
    }

    fn parse_package_json_member(
        repo_root: &Path,
        member_manifest: &Path,
    ) -> Result<WorkspaceMember> {
        let content = fs::read_to_string(member_manifest)?;
        let val: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            AppError::Validation(format!("Failed to parse member package.json: {e}"))
        })?;

        let name = val
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or_else(|| {
                AppError::Validation("Missing 'name' in member package.json".to_string())
            })?
            .to_string();

        let version = val
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let member_dir = member_manifest
            .parent()
            .ok_or_else(|| AppError::Validation("Invalid member manifest parent".to_string()))?;
        let relative_path = member_dir
            .strip_prefix(repo_root)
            .map_err(|e| AppError::Validation(format!("Path prefix error: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        let rel_manifest = member_manifest
            .strip_prefix(repo_root)
            .map_err(|e| AppError::Validation(format!("Path prefix error: {e}")))?
            .to_string_lossy()
            .replace('\\', "/");

        Ok(WorkspaceMember {
            name,
            relative_path,
            manifest_path: rel_manifest,
            manifest_kind: ManifestKind::PackageJson,
            current_version: version,
        })
    }

    /// Simple glob expansion supporting single trailing `/*` or direct paths.
    /// Excludes hidden directories (`.` prefix), `target`, and `node_modules`.
    fn expand_glob(repo_root: &Path, pattern: &str) -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        let cleaned = pattern.trim().trim_end_matches('/');

        if let Some(base_str) = cleaned.strip_suffix("/*") {
            let base_dir = repo_root.join(base_str);
            if let Ok(entries) = fs::read_dir(&base_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let file_name = entry.file_name();
                        let name_str = file_name.to_string_lossy();
                        if !name_str.starts_with('.')
                            && name_str != "node_modules"
                            && name_str != "target"
                        {
                            dirs.push(path);
                        }
                    }
                }
            }
        } else {
            let direct = repo_root.join(cleaned);
            if direct.is_dir() {
                dirs.push(direct);
            }
        }

        dirs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_cargo_workspace() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("Cargo.toml"),
            r#"
[workspace]
members = [
    "crates/*",
    "libs/shared",
]
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("crates/core")).unwrap();
        fs::write(
            root.join("crates/core/Cargo.toml"),
            r#"
[package]
name = "my-core"
version = "0.3.0"
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("crates/cli")).unwrap();
        fs::write(
            root.join("crates/cli/Cargo.toml"),
            r#"
[package]
name = "my-cli"
version = "1.2.0"
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("libs/shared")).unwrap();
        fs::write(
            root.join("libs/shared/Cargo.toml"),
            r#"
[package]
name = "my-shared"
version = "0.1.0"
"#,
        )
        .unwrap();

        let members = WorkspaceService::detect_workspace(root).unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].name, "my-cli");
        assert_eq!(members[0].relative_path, "crates/cli");
        assert_eq!(members[0].current_version.as_deref(), Some("1.2.0"));
        assert_eq!(members[0].manifest_kind, ManifestKind::Cargo);

        assert_eq!(members[1].name, "my-core");
        assert_eq!(members[1].relative_path, "crates/core");

        assert_eq!(members[2].name, "my-shared");
        assert_eq!(members[2].relative_path, "libs/shared");
    }

    #[test]
    fn detects_pnpm_workspace() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("pnpm-workspace.yaml"),
            r#"
packages:
  - 'packages/*'
  - "apps/web"
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("packages/utils")).unwrap();
        fs::write(
            root.join("packages/utils/package.json"),
            r#"{"name": "@mono/utils", "version": "2.0.1"}"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("apps/web")).unwrap();
        fs::write(
            root.join("apps/web/package.json"),
            r#"{"name": "@mono/web", "version": "0.5.0"}"#,
        )
        .unwrap();

        let members = WorkspaceService::detect_workspace(root).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].name, "@mono/utils");
        assert_eq!(members[0].relative_path, "packages/utils");
        assert_eq!(members[0].current_version.as_deref(), Some("2.0.1"));

        assert_eq!(members[1].name, "@mono/web");
        assert_eq!(members[1].relative_path, "apps/web");
        assert_eq!(members[1].current_version.as_deref(), Some("0.5.0"));
    }

    #[test]
    fn detects_npm_yarn_workspace() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("package.json"),
            r#"
{
  "name": "root-project",
  "private": true,
  "workspaces": [
    "modules/*"
  ]
}
"#,
        )
        .unwrap();

        fs::create_dir_all(root.join("modules/auth")).unwrap();
        fs::write(
            root.join("modules/auth/package.json"),
            r#"{"name": "auth-service", "version": "1.0.0"}"#,
        )
        .unwrap();

        let members = WorkspaceService::detect_workspace(root).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].name, "auth-service");
        assert_eq!(members[0].relative_path, "modules/auth");
        assert_eq!(members[0].current_version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn non_workspace_returns_empty_list() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "single-crate"
version = "0.1.0"
"#,
        )
        .unwrap();

        let members = WorkspaceService::detect_workspace(root).unwrap();
        assert!(members.is_empty());
    }
}
