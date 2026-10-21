use std::fs;
use std::path::Path;

use semver::{Prerelease, Version};

use crate::domain::commit::CommitInfo;
use crate::domain::package::Package;
use crate::domain::project::PackageTagPattern;
use crate::errors::{AppError, Result};
use crate::services::conventional::parse_conventional;
use crate::services::git_service::GitService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpLevel {
    Major,
    Minor,
    Patch,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BumpReason {
    pub commit_hash: Option<String>,
    pub summary: String,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BumpRecommendation {
    pub current_version: Version,
    pub recommended_version: Version,
    pub level: BumpLevel,
    pub is_zero_version: bool,
    pub reasons: Vec<BumpReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionManifestInfo {
    pub file_path: String,
    pub version: Version,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionConsistencyReport {
    pub tag_version: Option<Version>,
    pub manifest_versions: Vec<VersionManifestInfo>,
    pub is_consistent: bool,
    pub message: String,
}

pub struct VersionService;

impl VersionService {
    pub fn read_manifest_version(repo_root: &Path) -> Result<Option<VersionManifestInfo>> {
        let cargo_toml = repo_root.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = fs::read_to_string(&cargo_toml)?;
            if let Some(ver) = Self::extract_cargo_pkg_version(&content) {
                let parsed = Version::parse(&ver).map_err(|e| {
                    AppError::Validation(format!("Invalid Cargo.toml SemVer '{ver}': {e}"))
                })?;
                return Ok(Some(VersionManifestInfo {
                    file_path: "Cargo.toml".to_string(),
                    version: parsed,
                }));
            }
        }

        let package_json = repo_root.join("package.json");
        if package_json.exists() {
            let content = fs::read_to_string(&package_json)?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(ver_str) = val.get("version").and_then(|v| v.as_str())
            {
                let parsed = Version::parse(ver_str).map_err(|e| {
                    AppError::Validation(format!("Invalid package.json SemVer '{ver_str}': {e}"))
                })?;
                return Ok(Some(VersionManifestInfo {
                    file_path: "package.json".to_string(),
                    version: parsed,
                }));
            }
        }

        Ok(None)
    }

    fn extract_cargo_pkg_version(content: &str) -> Option<String> {
        let mut in_package = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_package = trimmed == "[package]";
                continue;
            }
            if in_package
                && trimmed.starts_with("version")
                && let Some((_, val)) = trimmed.split_once('=')
            {
                let clean = val.trim().trim_matches('"').trim_matches('\'');
                return Some(clean.to_string());
            }
        }
        None
    }

    pub fn read_current_version(repo: &git2::Repository, repo_root: &Path) -> Result<Version> {
        let tags = GitService::list_tags(repo)?;
        for tag in tags {
            let clean = tag
                .strip_prefix('v')
                .or_else(|| tag.strip_prefix('V'))
                .unwrap_or(&tag);
            if let Ok(ver) = Version::parse(clean) {
                return Ok(ver);
            }
        }

        if let Some(manifest) = Self::read_manifest_version(repo_root)? {
            return Ok(manifest.version);
        }

        Ok(Version::new(0, 1, 0))
    }

    pub fn parse_package_tag(tag: &str, package_name: &str) -> Option<Version> {
        let tag = tag.trim();
        let prefix_at = format!("{package_name}@");
        let prefix_dash_v = format!("{package_name}-v");
        let prefix_dash = format!("{package_name}-");
        let prefix_slash_v = format!("{package_name}/v");
        let prefix_slash = format!("{package_name}/");

        let ver_str = tag
            .strip_prefix(&prefix_at)
            .or_else(|| tag.strip_prefix(&prefix_dash_v))
            .or_else(|| tag.strip_prefix(&prefix_slash_v))
            .or_else(|| tag.strip_prefix(&prefix_dash))
            .or_else(|| tag.strip_prefix(&prefix_slash))?;

        let clean_ver = ver_str.strip_prefix('v').unwrap_or(ver_str);
        Version::parse(clean_ver).ok()
    }

    pub fn format_package_tag(
        package_name: &str,
        version: &Version,
        pattern: PackageTagPattern,
    ) -> String {
        pattern.format(package_name, &version.to_string())
    }

    pub fn read_package_current_version(
        repo: &git2::Repository,
        package: &Package,
        repo_root: &Path,
    ) -> Result<Version> {
        let tags = GitService::list_tags(repo)?;
        let mut matched_versions = Vec::new();
        for tag in tags {
            if let Some(ver) = Self::parse_package_tag(&tag, &package.name) {
                matched_versions.push(ver);
            }
        }

        if !matched_versions.is_empty() {
            matched_versions.sort();
            if let Some(highest) = matched_versions.pop() {
                return Ok(highest);
            }
        }

        // Fallback to manifest version recorded on package entity
        if let Some(ref ver_str) = package.current_version
            && let Ok(parsed) = Version::parse(ver_str)
        {
            return Ok(parsed);
        }

        // Fallback to reading manifest from disk
        let manifest_path = repo_root.join(&package.manifest_path);
        if manifest_path.is_file() {
            if manifest_path.ends_with("Cargo.toml")
                && let Ok(content) = fs::read_to_string(&manifest_path)
                && let Some(ver_str) = Self::extract_cargo_pkg_version(&content)
                && let Ok(parsed) = Version::parse(&ver_str)
            {
                return Ok(parsed);
            } else if manifest_path.ends_with("package.json")
                && let Ok(content) = fs::read_to_string(&manifest_path)
                && let Ok(val) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(ver_str) = val.get("version").and_then(|v| v.as_str())
                && let Ok(parsed) = Version::parse(ver_str)
            {
                return Ok(parsed);
            }
        }

        Ok(Version::new(0, 1, 0))
    }

    pub fn recommend_bump(current: &Version, commits: &[CommitInfo]) -> BumpRecommendation {
        let is_zero = current.major == 0;
        let mut has_breaking = false;
        let mut has_feat = false;
        let mut has_other = false;
        let mut reasons = Vec::new();

        for commit in commits {
            let full_msg = match &commit.body {
                Some(b) => format!("{}\n\n{}", commit.summary, b),
                None => commit.summary.clone(),
            };
            let parsed = parse_conventional(&full_msg);

            if parsed.is_breaking {
                has_breaking = true;
                reasons.push(BumpReason {
                    commit_hash: Some(commit.short_hash.clone()),
                    summary: commit.summary.clone(),
                    explanation: parsed
                        .breaking_reason
                        .unwrap_or_else(|| "Breaking change detected".to_string()),
                });
            } else if parsed.commit_type.as_deref() == Some("feat") {
                has_feat = true;
                reasons.push(BumpReason {
                    commit_hash: Some(commit.short_hash.clone()),
                    summary: commit.summary.clone(),
                    explanation: "New feature".to_string(),
                });
            } else {
                has_other = true;
            }
        }

        let level = if has_breaking {
            if is_zero {
                BumpLevel::Minor
            } else {
                BumpLevel::Major
            }
        } else if has_feat {
            BumpLevel::Minor
        } else if has_other {
            BumpLevel::Patch
        } else {
            BumpLevel::None
        };

        let recommended_version = match level {
            BumpLevel::Major => Version::new(current.major + 1, 0, 0),
            BumpLevel::Minor => {
                if is_zero && has_breaking {
                    Version::new(0, current.minor + 1, 0)
                } else {
                    Version::new(current.major, current.minor + 1, 0)
                }
            }
            BumpLevel::Patch => Version::new(current.major, current.minor, current.patch + 1),
            BumpLevel::None => current.clone(),
        };

        BumpRecommendation {
            current_version: current.clone(),
            recommended_version,
            level,
            is_zero_version: is_zero,
            reasons,
        }
    }

    pub fn with_prerelease(version: &Version, tag: &str) -> Result<Version> {
        let tag = tag.trim().trim_start_matches('-');
        if tag.is_empty()
            || !tag
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(AppError::Validation(format!(
                "Invalid prerelease tag '{tag}'"
            )));
        }

        let mut next = version.clone();
        let pre_str = version.pre.as_str();

        if pre_str.is_empty() {
            let initial = format!("{tag}.1");
            next.pre = Prerelease::new(&initial)
                .map_err(|e| AppError::Validation(format!("Invalid prerelease identifier: {e}")))?;
        } else if let Some((prefix, num_str)) = pre_str.rsplit_once('.') {
            if prefix == tag {
                if let Ok(num) = num_str.parse::<u64>() {
                    let incremented = format!("{prefix}.{}", num + 1);
                    next.pre = Prerelease::new(&incremented).map_err(|e| {
                        AppError::Validation(format!("Invalid prerelease identifier: {e}"))
                    })?;
                } else {
                    let initial = format!("{tag}.1");
                    next.pre = Prerelease::new(&initial).map_err(|e| {
                        AppError::Validation(format!("Invalid prerelease identifier: {e}"))
                    })?;
                }
            } else {
                let initial = format!("{tag}.1");
                next.pre = Prerelease::new(&initial).map_err(|e| {
                    AppError::Validation(format!("Invalid prerelease identifier: {e}"))
                })?;
            }
        } else {
            let initial = format!("{tag}.1");
            next.pre = Prerelease::new(&initial)
                .map_err(|e| AppError::Validation(format!("Invalid prerelease identifier: {e}")))?;
        }

        Ok(next)
    }

    pub fn parse_manual_version(input: &str) -> Result<Version> {
        let trimmed = input
            .trim()
            .strip_prefix('v')
            .or_else(|| input.trim().strip_prefix('V'))
            .unwrap_or(input.trim());
        Version::parse(trimmed).map_err(|e| {
            AppError::Validation(format!(
                "Invalid Semantic Version '{input}': {e}. Expected format: MAJOR.MINOR.PATCH"
            ))
        })
    }

    pub fn check_consistency(
        tag_version: Option<&Version>,
        manifests: &[VersionManifestInfo],
    ) -> VersionConsistencyReport {
        if manifests.is_empty() {
            return VersionConsistencyReport {
                tag_version: tag_version.cloned(),
                manifest_versions: Vec::new(),
                is_consistent: true,
                message: "No package manifests found to compare".to_string(),
            };
        }

        let Some(tag_ver) = tag_version else {
            return VersionConsistencyReport {
                tag_version: None,
                manifest_versions: manifests.to_vec(),
                is_consistent: true,
                message: "No Git tags found; using manifest version".to_string(),
            };
        };

        let mut mismatches = Vec::new();
        for m in manifests {
            if &m.version != tag_ver {
                mismatches.push(format!("{}: {}", m.file_path, m.version));
            }
        }

        if mismatches.is_empty() {
            VersionConsistencyReport {
                tag_version: Some(tag_ver.clone()),
                manifest_versions: manifests.to_vec(),
                is_consistent: true,
                message: format!("Version {tag_ver} is consistent across tags and manifests"),
            }
        } else {
            VersionConsistencyReport {
                tag_version: Some(tag_ver.clone()),
                manifest_versions: manifests.to_vec(),
                is_consistent: false,
                message: format!(
                    "Version mismatch: Git tag is {tag_ver}, but manifests differ ({})",
                    mismatches.join(", ")
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    fn make_commit(summary: &str, body: Option<&str>) -> CommitInfo {
        CommitInfo {
            hash: "abcdef1234567890".to_string(),
            short_hash: "abcdef1".to_string(),
            author: "Dev".to_string(),
            email: "dev@example.com".to_string(),
            time: Utc::now(),
            summary: summary.to_string(),
            body: body.map(String::from),
        }
    }

    #[test]
    fn reads_cargo_and_package_json_manifests() {
        let dir = tempdir().unwrap();
        let cargo_path = dir.path().join("Cargo.toml");
        fs::write(
            &cargo_path,
            "[package]\nname = \"demo\"\nversion = \"1.4.2\"\n",
        )
        .unwrap();

        let info = VersionService::read_manifest_version(dir.path())
            .unwrap()
            .expect("manifest found");
        assert_eq!(info.file_path, "Cargo.toml");
        assert_eq!(info.version, Version::parse("1.4.2").unwrap());
    }

    #[test]
    fn recommend_bump_major_for_stable() {
        let current = Version::parse("1.2.3").unwrap();
        let commits = vec![make_commit("feat!: change public api", None)];
        let rec = VersionService::recommend_bump(&current, &commits);

        assert_eq!(rec.level, BumpLevel::Major);
        assert_eq!(rec.recommended_version, Version::parse("2.0.0").unwrap());
        assert!(!rec.is_zero_version);
    }

    #[test]
    fn recommend_bump_minor_for_zero_version_breaking() {
        let current = Version::parse("0.2.1").unwrap();
        let commits = vec![make_commit("feat!: breaking in 0.x", None)];
        let rec = VersionService::recommend_bump(&current, &commits);

        assert_eq!(rec.level, BumpLevel::Minor);
        assert_eq!(rec.recommended_version, Version::parse("0.3.0").unwrap());
        assert!(rec.is_zero_version);
    }

    #[test]
    fn recommend_bump_minor_for_features() {
        let current = Version::parse("1.2.3").unwrap();
        let commits = vec![
            make_commit("fix: patch bug", None),
            make_commit("feat: add export button", None),
        ];
        let rec = VersionService::recommend_bump(&current, &commits);

        assert_eq!(rec.level, BumpLevel::Minor);
        assert_eq!(rec.recommended_version, Version::parse("1.3.0").unwrap());
    }

    #[test]
    fn recommend_bump_patch_for_fixes() {
        let current = Version::parse("1.2.3").unwrap();
        let commits = vec![make_commit("fix: correct typo", None)];
        let rec = VersionService::recommend_bump(&current, &commits);

        assert_eq!(rec.level, BumpLevel::Patch);
        assert_eq!(rec.recommended_version, Version::parse("1.2.4").unwrap());
    }

    #[test]
    fn recommend_bump_none_for_empty() {
        let current = Version::parse("1.2.3").unwrap();
        let rec = VersionService::recommend_bump(&current, &[]);

        assert_eq!(rec.level, BumpLevel::None);
        assert_eq!(rec.recommended_version, current);
    }

    #[test]
    fn prerelease_increments_and_creates() {
        let v = Version::parse("1.2.0").unwrap();
        let rc1 = VersionService::with_prerelease(&v, "rc").unwrap();
        assert_eq!(rc1, Version::parse("1.2.0-rc.1").unwrap());

        let rc2 = VersionService::with_prerelease(&rc1, "rc").unwrap();
        assert_eq!(rc2, Version::parse("1.2.0-rc.2").unwrap());
    }

    #[test]
    fn parse_manual_version_validates() {
        assert_eq!(
            VersionService::parse_manual_version("v1.5.0").unwrap(),
            Version::parse("1.5.0").unwrap()
        );
        assert!(VersionService::parse_manual_version("1.5").is_err());
        assert!(VersionService::parse_manual_version("invalid").is_err());
    }

    #[test]
    fn consistency_check_reports_matching_and_discrepancy() {
        let tag_v = Version::parse("1.0.0").unwrap();
        let manifests_matching = vec![VersionManifestInfo {
            file_path: "Cargo.toml".to_string(),
            version: Version::parse("1.0.0").unwrap(),
        }];

        let rep_ok = VersionService::check_consistency(Some(&tag_v), &manifests_matching);
        assert!(rep_ok.is_consistent);

        let manifests_diff = vec![VersionManifestInfo {
            file_path: "Cargo.toml".to_string(),
            version: Version::parse("0.9.0").unwrap(),
        }];
        let rep_diff = VersionService::check_consistency(Some(&tag_v), &manifests_diff);
        assert!(!rep_diff.is_consistent);
        assert!(rep_diff.message.contains("Version mismatch"));
    }

    #[test]
    fn package_tag_parsing_formatting_and_mixed_detection() {
        assert_eq!(
            VersionService::parse_package_tag("core@1.2.3", "core").unwrap(),
            Version::parse("1.2.3").unwrap()
        );
        assert_eq!(
            VersionService::parse_package_tag("web-v2.0.0-rc.1", "web").unwrap(),
            Version::parse("2.0.0-rc.1").unwrap()
        );
        assert_eq!(
            VersionService::parse_package_tag("cli/1.0.0", "cli").unwrap(),
            Version::parse("1.0.0").unwrap()
        );
        // Unrelated package tags
        assert!(VersionService::parse_package_tag("core@1.2.3", "web").is_none());
        assert!(VersionService::parse_package_tag("v1.2.3", "core").is_none());

        // Formatting
        let ver = Version::parse("0.4.0").unwrap();
        assert_eq!(
            VersionService::format_package_tag("my-lib", &ver, PackageTagPattern::At),
            "my-lib@0.4.0"
        );
        assert_eq!(
            VersionService::format_package_tag("my-lib", &ver, PackageTagPattern::DashV),
            "my-lib-v0.4.0"
        );

        // Test with git repository containing mixed tags
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("Dev", "dev@example.com").unwrap();

        // Create initial commit
        fs::write(dir.path().join("file.txt"), "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let commit_id = repo
            .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        let obj = repo.find_object(commit_id, None).unwrap();

        // Tag mixed tags:
        // core: 1.0.0, 1.3.0
        // ui: ui-v0.9.0, ui-v2.1.0
        // global: v9.9.9
        repo.tag("core@1.0.0", &obj, &sig, "c1", false).unwrap();
        repo.tag("core@1.3.0", &obj, &sig, "c2", false).unwrap();
        repo.tag("ui-v0.9.0", &obj, &sig, "u1", false).unwrap();
        repo.tag("ui-v2.1.0", &obj, &sig, "u2", false).unwrap();
        repo.tag("v9.9.9", &obj, &sig, "root", false).unwrap();

        let pkg_core = Package::new(1, "core", "packages/core", "packages/core/Cargo.toml", None);
        let pkg_ui = Package::new(1, "ui", "packages/ui", "packages/ui/package.json", None);

        let core_ver =
            VersionService::read_package_current_version(&repo, &pkg_core, dir.path()).unwrap();
        assert_eq!(core_ver, Version::parse("1.3.0").unwrap());

        let ui_ver =
            VersionService::read_package_current_version(&repo, &pkg_ui, dir.path()).unwrap();
        assert_eq!(ui_ver, Version::parse("2.1.0").unwrap());
    }
}
