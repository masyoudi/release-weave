use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use semver::Version;

use crate::ai::prompts::{ContextReport, SYSTEM_PROMPT, build_user_prompt};
use crate::ai::provider::AiProvider;
use crate::ai::schemas::validate_and_convert_items;
use crate::domain::check::CheckResult;
use crate::domain::commit::{ChangeCategory, CommitInfo};
use crate::domain::file_change::{FileChangePlan, FilePatch, sha256_hex};
use crate::domain::release::{ChangeItem, Release, ReleaseStatus};
use crate::errors::{AppError, Result};
use crate::security::path_validation::validate_output_file;
use crate::services::changelog_writer::ChangelogWriter;
use crate::services::manifest_writer::ManifestWriter;
use crate::services::template::{TemplateContext, TemplateEngine};
use crate::storage::repositories::{ReadinessRepo, ReleaseRepo};

pub struct AiDraftResult {
    pub release_notes: String,
    pub items: Vec<ChangeItem>,
    pub context_report: ContextReport,
    pub model_metadata: String,
}

#[derive(Debug, Clone)]
pub struct DraftOptions<'a> {
    pub version: &'a str,
    pub template_str: &'a str,
    pub previous_version: Option<&'a str>,
    pub context_limit_chars: usize,
}

pub struct ReleaseService;

impl ReleaseService {
    pub fn generate_deterministic_draft(
        version: &str,
        items: &[ChangeItem],
        template_str: &str,
        previous_version: Option<&str>,
    ) -> Result<String> {
        let ctx = TemplateContext {
            version,
            date: Utc::now().date_naive(),
            previous_version,
            items,
        };

        TemplateEngine::render(template_str, &ctx)
    }

    pub fn generate_ai_draft(
        provider: &dyn AiProvider,
        commits: &[CommitInfo],
        opts: &DraftOptions<'_>,
    ) -> Result<AiDraftResult> {
        let (user_prompt, report) = build_user_prompt(commits, opts.context_limit_chars);
        let info = provider.info();
        let model_metadata = format!("{}:{}", info.provider_name, info.model_name);

        let valid_hashes: HashSet<String> = commits.iter().map(|c| c.hash.clone()).collect();

        let ai_raw = provider.generate_json(SYSTEM_PROMPT, &user_prompt)?;
        let items = validate_and_convert_items(
            &ai_raw,
            &valid_hashes,
            &info.provider_name,
            &info.model_name,
        )?;

        let release_notes = Self::generate_deterministic_draft(
            opts.version,
            &items,
            opts.template_str,
            opts.previous_version,
        )?;

        Ok(AiDraftResult {
            release_notes,
            items,
            context_report: report,
            model_metadata,
        })
    }

    pub fn regenerate_category(
        provider: &dyn AiProvider,
        commits: &[CommitInfo],
        target_category: ChangeCategory,
        existing_items: &[ChangeItem],
        opts: &DraftOptions<'_>,
    ) -> Result<AiDraftResult> {
        let draft_res = Self::generate_ai_draft(provider, commits, opts)?;

        // Filter new items for the target category only
        let new_cat_items: Vec<ChangeItem> = draft_res
            .items
            .into_iter()
            .filter(|i| i.category == target_category)
            .collect();

        // Keep existing items of other categories untouched
        let mut merged = Vec::new();
        for item in existing_items {
            if item.category != target_category {
                merged.push(item.clone());
            }
        }
        merged.extend(new_cat_items);

        let release_notes = Self::generate_deterministic_draft(
            opts.version,
            &merged,
            opts.template_str,
            opts.previous_version,
        )?;

        Ok(AiDraftResult {
            release_notes,
            items: merged,
            context_report: draft_res.context_report,
            model_metadata: draft_res.model_metadata,
        })
    }

    pub fn export_markdown(content: &str, destination_path: impl AsRef<Path>) -> Result<PathBuf> {
        let validated = validate_output_file(destination_path)?;
        fs::write(&validated, content)?;
        Ok(validated)
    }

    pub fn finalize_release(
        conn: &mut rusqlite::Connection,
        release_id: i64,
        final_notes: &str,
        items: &[ChangeItem],
        checks: &[CheckResult],
    ) -> Result<Release> {
        ReleaseRepo::update_notes_and_status(conn, release_id, ReleaseStatus::Final, final_notes)?;
        ReleaseRepo::save_change_items(conn, release_id, items)?;
        ReadinessRepo::save_checks(conn, release_id, checks)?;

        ReleaseRepo::get(conn, release_id)?
            .ok_or_else(|| AppError::Validation(format!("Release {release_id} not found")))
    }

    pub fn plan_apply(
        repo_root: &Path,
        target_version: &Version,
        release_notes: &str,
    ) -> Result<FileChangePlan> {
        let mut patches = Vec::new();

        // 1. Cargo.toml
        let cargo_path = repo_root.join("Cargo.toml");
        if cargo_path.is_file() {
            let old = fs::read_to_string(&cargo_path)?;
            if let Ok(new_content) = ManifestWriter::bump_cargo_toml(&old, target_version) {
                patches.push(FilePatch::new(
                    cargo_path,
                    "Cargo.toml".to_string(),
                    Some(old),
                    new_content,
                ));
            }
        }

        // 2. package.json
        let pkg_path = repo_root.join("package.json");
        if pkg_path.is_file() {
            let old = fs::read_to_string(&pkg_path)?;
            if let Ok(new_content) = ManifestWriter::bump_package_json(&old, target_version) {
                patches.push(FilePatch::new(
                    pkg_path,
                    "package.json".to_string(),
                    Some(old),
                    new_content,
                ));
            }
        }

        // 3. pyproject.toml
        let pyp_path = repo_root.join("pyproject.toml");
        if pyp_path.is_file() {
            let old = fs::read_to_string(&pyp_path)?;
            if let Ok(new_content) = ManifestWriter::bump_pyproject_toml(&old, target_version) {
                patches.push(FilePatch::new(
                    pyp_path,
                    "pyproject.toml".to_string(),
                    Some(old),
                    new_content,
                ));
            }
        }

        // 4. CHANGELOG.md
        let changelog_path = repo_root.join("CHANGELOG.md");
        let old_changelog = if changelog_path.is_file() {
            Some(fs::read_to_string(&changelog_path)?)
        } else {
            None
        };

        let date_str = Utc::now().format("%Y-%m-%d").to_string();
        let new_changelog = ChangelogWriter::insert_entry(
            old_changelog.as_deref(),
            target_version,
            &date_str,
            release_notes,
        )?;

        patches.push(FilePatch::new(
            changelog_path,
            "CHANGELOG.md".to_string(),
            old_changelog,
            new_changelog,
        ));

        Ok(FileChangePlan::new(target_version.clone(), patches))
    }

    pub fn apply_plan(plan: &FileChangePlan) -> Result<Vec<PathBuf>> {
        let mut modified = Vec::new();

        // Pass 1: Verify all file hashes match disk before making ANY changes
        for patch in &plan.patches {
            if patch.old_content.is_some() {
                if !patch.path.is_file() {
                    return Err(AppError::Validation(format!(
                        "File '{}' no longer exists on disk since plan creation",
                        patch.relative_path
                    )));
                }

                let current_content = fs::read_to_string(&patch.path)?;
                let current_sha256 = sha256_hex(current_content.as_bytes());

                if Some(&current_sha256) != patch.old_sha256.as_ref() {
                    return Err(AppError::Validation(format!(
                        "File '{}' has been modified on disk since the preview was generated. Apply aborted to prevent overwriting concurrent changes.",
                        patch.relative_path
                    )));
                }
            } else if patch.path.exists() {
                return Err(AppError::Validation(format!(
                    "File '{}' was created on disk after preview was generated",
                    patch.relative_path
                )));
            }
        }

        // Pass 2: Atomically apply changes
        for patch in &plan.patches {
            let parent = patch.path.parent().unwrap_or_else(|| Path::new("."));
            let filename = patch
                .path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("tmp");
            let tmp_path = parent.join(format!(".{filename}.rw-tmp"));

            fs::write(&tmp_path, &patch.new_content)?;
            if let Err(e) = fs::rename(&tmp_path, &patch.path) {
                let _ = fs::remove_file(&tmp_path);
                return Err(AppError::Io(e));
            }

            modified.push(patch.path.clone());
        }

        Ok(modified)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;
    use crate::ai::provider::FakeProvider;
    use crate::services::template::TEMPLATE_KEEP_A_CHANGELOG;

    fn make_commit(hash: &str, summary: &str) -> CommitInfo {
        CommitInfo {
            hash: hash.to_string(),
            short_hash: hash[..7.min(hash.len())].to_string(),
            author: "Tester".to_string(),
            email: "tester@example.com".to_string(),
            time: Utc::now(),
            summary: summary.to_string(),
            body: None,
        }
    }

    #[test]
    fn deterministic_draft_includes_short_hashes_per_item() {
        let items = vec![
            ChangeItem::new_deterministic(
                "abcdef1234567890",
                ChangeCategory::Feature,
                "New auth flow",
            ),
            ChangeItem::new_deterministic("1234567890abcdef", ChangeCategory::Fix, "Resolve crash"),
        ];

        let draft = ReleaseService::generate_deterministic_draft(
            "1.0.0",
            &items,
            TEMPLATE_KEEP_A_CHANGELOG,
            Some("0.9.0"),
        )
        .unwrap();

        assert!(draft.contains("abcdef1"));
        assert!(draft.contains("1234567"));
        assert!(draft.contains("- New auth flow (abcdef1)"));
        assert!(draft.contains("- Resolve crash (1234567)"));
    }

    #[test]
    fn ai_draft_generation_flow() {
        let commits = vec![make_commit("1122334455667788", "feat: implement dark mode")];
        let fake_ai_json = r#"{
            "items": [
                {
                    "category": "feature",
                    "title": "Add sleek dark mode",
                    "description": "Reduces eye strain in night environments",
                    "commit_hashes": ["1122334455667788"]
                }
            ]
        }"#;

        let provider = FakeProvider::success(fake_ai_json);
        let opts = DraftOptions {
            version: "1.1.0",
            template_str: TEMPLATE_KEEP_A_CHANGELOG,
            previous_version: Some("1.0.0"),
            context_limit_chars: 10000,
        };
        let res = ReleaseService::generate_ai_draft(&provider, &commits, &opts).unwrap();

        assert_eq!(res.model_metadata, "fake:fake-coder:7b");
        assert_eq!(res.items.len(), 1);
        assert_eq!(res.items[0].title, "Add sleek dark mode");
        assert!(res.release_notes.contains("Add sleek dark mode"));
        assert!(!res.context_report.is_truncated);
    }

    #[test]
    fn regenerate_category_preserves_other_categories() {
        let commits = vec![make_commit("1111222233334444", "fix: resolve crash")];
        let fake_ai_json = r#"{
            "items": [
                {
                    "category": "fix",
                    "title": "Prevent null pointer crash",
                    "commit_hashes": ["1111222233334444"]
                }
            ]
        }"#;
        let provider = FakeProvider::success(fake_ai_json);

        let existing = vec![
            ChangeItem::new_deterministic(
                "aaaa1111bbbb2222",
                ChangeCategory::Feature,
                "Keep this feature",
            ),
            ChangeItem::new_deterministic(
                "cccc3333dddd4444",
                ChangeCategory::Fix,
                "Old fix to replace",
            ),
        ];

        let opts = DraftOptions {
            version: "1.1.0",
            template_str: TEMPLATE_KEEP_A_CHANGELOG,
            previous_version: None,
            context_limit_chars: 10000,
        };
        let res = ReleaseService::regenerate_category(
            &provider,
            &commits,
            ChangeCategory::Fix,
            &existing,
            &opts,
        )
        .unwrap();

        // Feature category must remain untouched
        let feat = res
            .items
            .iter()
            .find(|i| i.category == ChangeCategory::Feature)
            .unwrap();
        assert_eq!(feat.title, "Keep this feature");

        // Fix category must have the newly generated fix
        let fix = res
            .items
            .iter()
            .find(|i| i.category == ChangeCategory::Fix)
            .unwrap();
        assert_eq!(fix.title, "Prevent null pointer crash");
    }

    #[test]
    fn exports_markdown_via_path_validation() {
        let dir = tempdir().unwrap();
        let out_file = dir.path().join("RELEASE_NOTES.md");
        let content = "# Release 1.0.0\n\n- Verified notes\n";

        let saved = ReleaseService::export_markdown(content, &out_file).unwrap();
        assert_eq!(saved, out_file.canonicalize().unwrap());

        let read_back = fs::read_to_string(&saved).unwrap();
        assert_eq!(read_back, content);
    }

    #[test]
    fn finalize_release_persists_final_status_and_leaves_git_untouched() {
        use crate::domain::check::CheckStatus;
        use crate::domain::project::Project;
        use crate::storage::database::open_in_memory;
        use crate::storage::repositories::ProjectRepo;

        let dir = tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("Tester", "test@example.com").unwrap();
        let fpath = dir.path().join("file.txt");
        fs::write(&fpath, "hello").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let head_commit = repo
            .commit(Some("HEAD"), &sig, &sig, "chore: initial", &tree, &[])
            .unwrap();

        let mut conn = open_in_memory().unwrap();
        let project = Project::new("TestProj", dir.path(), "main");
        let pid = ProjectRepo::insert(&conn, &project).unwrap();

        let release = Release::new_draft(pid, "1.0.0", None, None, head_commit.to_string());
        let rid = ReleaseRepo::insert(&conn, &release).unwrap();

        let items = vec![ChangeItem::new_deterministic(
            head_commit.to_string(),
            ChangeCategory::Feature,
            "Initial feature",
        )];
        let checks = vec![CheckResult {
            id: None,
            release_id: Some(rid),
            check_type: "working_tree_clean".to_string(),
            status: CheckStatus::Passed,
            required: true,
            details: None,
            executed_at: Some(Utc::now()),
        }];

        let finalized = ReleaseService::finalize_release(
            &mut conn,
            rid,
            "Finalized release notes content",
            &items,
            &checks,
        )
        .unwrap();

        assert_eq!(finalized.status, ReleaseStatus::Final);
        assert_eq!(finalized.release_notes, "Finalized release notes content");
        assert!(finalized.completed_at.is_some());

        // Git repo state verification: no tags, HEAD unchanged, clean tree
        let current_head = repo.head().unwrap().target().unwrap();
        assert_eq!(current_head, head_commit);
        assert_eq!(repo.tag_names(None).unwrap().len(), 0);
        assert!(repo.statuses(None).unwrap().is_empty());
    }

    #[test]
    fn plan_apply_is_strictly_read_only() {
        let dir = tempdir().unwrap();
        let cargo_path = dir.path().join("Cargo.toml");
        let pkg_path = dir.path().join("package.json");

        fs::write(
            &cargo_path,
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(
            &pkg_path,
            "{\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\"\n}\n",
        )
        .unwrap();

        let new_ver = Version::parse("0.2.0").unwrap();
        let plan = ReleaseService::plan_apply(dir.path(), &new_ver, "- Notes").unwrap();

        assert_eq!(plan.patches.len(), 3); // Cargo.toml, package.json, CHANGELOG.md

        // Assert nothing changed on disk yet
        assert_eq!(
            fs::read_to_string(&cargo_path).unwrap(),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n"
        );
        assert_eq!(
            fs::read_to_string(&pkg_path).unwrap(),
            "{\n  \"name\": \"demo\",\n  \"version\": \"1.0.0\"\n}\n"
        );
        assert!(!dir.path().join("CHANGELOG.md").exists());
    }

    #[test]
    fn apply_plan_updates_files_and_rejects_concurrent_mutation() {
        let dir = tempdir().unwrap();
        let cargo_path = dir.path().join("Cargo.toml");
        fs::write(
            &cargo_path,
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let new_ver = Version::parse("0.2.0").unwrap();
        let plan = ReleaseService::plan_apply(dir.path(), &new_ver, "- Notes").unwrap();

        // 1. Apply successfully
        let modified = ReleaseService::apply_plan(&plan).unwrap();
        assert_eq!(modified.len(), 2); // Cargo.toml and new CHANGELOG.md
        assert!(
            fs::read_to_string(&cargo_path)
                .unwrap()
                .contains("version = \"0.2.0\"")
        );
        assert!(dir.path().join("CHANGELOG.md").exists());

        // 2. Concurrent mutation check: modify Cargo.toml manually
        fs::write(
            &cargo_path,
            "[package]\nname = \"demo\"\nversion = \"0.3.0\"\n",
        )
        .unwrap();

        // Applying the previous plan now MUST fail because sha256 mismatch
        let err = ReleaseService::apply_plan(&plan).unwrap_err();
        assert!(
            err.to_string()
                .contains("modified on disk since the preview was generated")
        );
    }
}
