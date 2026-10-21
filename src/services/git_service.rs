use std::collections::HashSet;
use std::path::Path;

use chrono::{TimeZone, Utc};
use git2::{Delta, DiffOptions, Oid, Repository, StatusOptions};

use crate::domain::commit::{ChangeCategory, CommitInfo, FileChange, FileStatus};
use crate::domain::project::Project;
use crate::domain::release::ChangeItem;
use crate::errors::{AppError, Result};
use crate::security::path_validation::validate_dir;
use crate::services::conventional::{CategoryMapper, parse_conventional, to_change_item};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSummary {
    pub name: String,
    pub branch: String,
    pub head_commit: Option<String>,
    pub latest_tag: Option<String>,
    pub is_dirty: bool,
    pub dirty_files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRange {
    pub from: Option<Oid>,
    pub to: Oid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakingChangeCandidate {
    pub commit_hash: String,
    pub summary: String,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChangeFilter {
    pub path: Option<String>,
    pub author: Option<String>,
    pub category: Option<ChangeCategory>,
    pub keyword: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchResult {
    pub patch: String,
    pub truncated: bool,
}

pub struct GitService;

impl GitService {
    pub fn open(path: impl AsRef<Path>) -> Result<Repository> {
        let canonical = validate_dir(path)?;
        Repository::open(&canonical).map_err(|e| {
            AppError::Validation(format!("Directory is not a valid Git repository: {e}"))
        })
    }

    pub fn summary(repo: &Repository) -> Result<RepoSummary> {
        let workdir = repo
            .workdir()
            .ok_or_else(|| AppError::Validation("Bare repositories are not supported".into()))?;

        let name = workdir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();

        let branch = match repo.head() {
            Ok(head) => {
                if head.is_branch() {
                    head.shorthand().unwrap_or("HEAD").to_string()
                } else {
                    "detached HEAD".to_string()
                }
            }
            Err(_) => "empty repository".to_string(),
        };

        let head_commit = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .map(|c| c.id().to_string());

        let tags = Self::list_tags(repo)?;
        let latest_tag = tags.first().cloned();

        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_opts))?;

        let mut dirty_files = Vec::new();
        for entry in statuses.iter() {
            if let Ok(path) = entry.path() {
                dirty_files.push(path.to_string());
            }
        }
        let is_dirty = !dirty_files.is_empty();

        Ok(RepoSummary {
            name,
            branch,
            head_commit,
            latest_tag,
            is_dirty,
            dirty_files,
        })
    }

    pub fn list_tags(repo: &Repository) -> Result<Vec<String>> {
        let tag_names = repo.tag_names(None)?;
        let mut tags: Vec<String> = tag_names
            .iter()
            .filter_map(|t| t.ok().flatten().map(String::from))
            .collect();

        tags.sort_by(|a, b| {
            let ver_a = semver::Version::parse(a.strip_prefix('v').unwrap_or(a));
            let ver_b = semver::Version::parse(b.strip_prefix('v').unwrap_or(b));

            match (ver_a, ver_b) {
                (Ok(va), Ok(vb)) => vb.cmp(&va), // latest version first
                (Ok(_), Err(_)) => std::cmp::Ordering::Less,
                (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
                (Err(_), Err(_)) => b.cmp(a),
            }
        });

        Ok(tags)
    }

    pub fn create_annotated_tag(
        repo: &Repository,
        tag_name: &str,
        message: &str,
    ) -> Result<String> {
        let tag_name = tag_name.trim();
        if tag_name.is_empty() {
            return Err(AppError::Validation("Tag name cannot be empty".to_string()));
        }

        if tag_name.starts_with('-') {
            return Err(AppError::Validation(
                "Tag name cannot start with a hyphen (-) to prevent CLI option injection"
                    .to_string(),
            ));
        }

        if !git2::Reference::is_valid_name(&format!("refs/tags/{tag_name}")) {
            return Err(AppError::Validation(format!(
                "Invalid Git tag reference name: '{tag_name}'"
            )));
        }

        // 1. Condition: HEAD is not detached
        if repo.head_detached().unwrap_or(false) {
            return Err(AppError::Validation(
                "Cannot create tag: HEAD is detached (checkout a branch first)".to_string(),
            ));
        }

        // 2. Condition: Working tree is clean
        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        let statuses = repo.statuses(Some(&mut status_opts))?;
        if !statuses.is_empty() {
            return Err(AppError::Validation(
                "Working tree is dirty (uncommitted changes detected). Commit or stash changes before creating a tag.".to_string(),
            ));
        }

        // 3. Condition: Tag does not already exist
        let tag_refname = format!("refs/tags/{tag_name}");
        if repo.find_reference(&tag_refname).is_ok() {
            return Err(AppError::Validation(format!(
                "Tag '{tag_name}' already exists in repository"
            )));
        }

        // 4. Resolve target commit from HEAD
        let head = repo
            .head()
            .map_err(|e| AppError::Validation(format!("Cannot resolve HEAD reference: {e}")))?;
        let target = head.peel_to_commit().map_err(|e| {
            AppError::Validation(format!("HEAD cannot be peeled to a valid commit: {e}"))
        })?;

        // 5. Build tagger signature (fallback to ReleaseWeave local identity)
        let signature = repo.signature().unwrap_or_else(|_| {
            git2::Signature::now("ReleaseWeave", "releaseweave@local")
                .expect("Valid default signature")
        });

        // 6. Create annotated tag
        let tag_oid = repo.tag(tag_name, &target.into_object(), &signature, message, false)?;

        Ok(tag_oid.to_string())
    }

    pub fn resolve_range(
        repo: &Repository,
        from_ref: Option<&str>,
        to_ref: &str,
    ) -> Result<CommitRange> {
        let to_obj = repo.revparse_single(to_ref).map_err(|_| {
            AppError::NotFound(format!("Git target reference not found: '{to_ref}'"))
        })?;
        let to = to_obj
            .peel_to_commit()
            .map_err(|e| AppError::Validation(format!("Target reference is not a commit: {e}")))?
            .id();

        let from = if let Some(from_s) = from_ref {
            let from_obj = repo.revparse_single(from_s).map_err(|_| {
                AppError::NotFound(format!("Git starting reference not found: '{from_s}'"))
            })?;
            let from_oid = from_obj
                .peel_to_commit()
                .map_err(|e| {
                    AppError::Validation(format!("Starting reference is not a commit: {e}"))
                })?
                .id();
            Some(from_oid)
        } else {
            None
        };

        Ok(CommitRange { from, to })
    }

    pub fn commits_in_range(repo: &Repository, range: &CommitRange) -> Result<Vec<CommitInfo>> {
        Self::commits_in_range_with_path(repo, range, None)
    }

    pub fn commits_in_range_with_path(
        repo: &Repository,
        range: &CommitRange,
        path_filter: Option<&str>,
    ) -> Result<Vec<CommitInfo>> {
        let mut revwalk = repo.revwalk()?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        revwalk.push(range.to)?;

        if let Some(from) = range.from {
            revwalk.hide(from)?;
        }

        let mut commits = Vec::new();
        for oid_result in revwalk {
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;

            // Skip merge commits by default
            if commit.parent_count() > 1 {
                continue;
            }

            // Path filter check for monorepo package
            if let Some(prefix) = path_filter {
                let clean = prefix.trim();
                if !clean.is_empty() && !Self::commit_touches_path(repo, &commit, clean)? {
                    continue;
                }
            }

            let hash = oid.to_string();
            let short_hash = if hash.len() >= 7 {
                hash[..7].to_string()
            } else {
                hash.clone()
            };

            let author_sig = commit.author();
            let author = author_sig.name().unwrap_or("Unknown").to_string();
            let email = author_sig.email().unwrap_or("").to_string();

            let time = Utc
                .timestamp_opt(commit.time().seconds(), 0)
                .single()
                .unwrap_or_else(Utc::now);

            let summary = commit.summary().ok().flatten().unwrap_or("").to_string();
            let body = commit.body().ok().flatten().map(|b| b.trim().to_string());

            commits.push(CommitInfo {
                hash,
                short_hash,
                author,
                email,
                time,
                summary,
                body,
            });
        }

        Ok(commits)
    }

    fn commit_touches_path(
        repo: &Repository,
        commit: &git2::Commit,
        path_prefix: &str,
    ) -> Result<bool> {
        let clean_prefix = path_prefix.trim_matches('/').replace('\\', "/");
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };
        let mut diff_opts = DiffOptions::new();
        let diff =
            repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;
        for delta in diff.deltas() {
            let p = delta.new_file().path().or_else(|| delta.old_file().path());
            if let Some(path) = p {
                let p_str = path.to_string_lossy().replace('\\', "/");
                if p_str == clean_prefix || p_str.starts_with(&format!("{clean_prefix}/")) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub fn file_changes_for_commit(repo: &Repository, oid: Oid) -> Result<Vec<FileChange>> {
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff =
            repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;

        Self::extract_file_changes(&diff)
    }

    pub fn file_changes_in_range(
        repo: &Repository,
        range: &CommitRange,
    ) -> Result<Vec<FileChange>> {
        let to_commit = repo.find_commit(range.to)?;
        let to_tree = to_commit.tree()?;

        let from_tree = match range.from {
            Some(from_oid) => Some(repo.find_commit(from_oid)?.tree()?),
            None => None,
        };

        let mut diff_opts = DiffOptions::new();
        let diff =
            repo.diff_tree_to_tree(from_tree.as_ref(), Some(&to_tree), Some(&mut diff_opts))?;

        Self::extract_file_changes(&diff)
    }

    pub fn file_changes_in_range_with_path(
        repo: &Repository,
        range: &CommitRange,
        path_filter: Option<&str>,
    ) -> Result<Vec<FileChange>> {
        let all = Self::file_changes_in_range(repo, range)?;
        if let Some(prefix) = path_filter {
            let clean = prefix.trim_matches('/').replace('\\', "/");
            if !clean.is_empty() {
                return Ok(all
                    .into_iter()
                    .filter(|fc| {
                        let p = fc.path.replace('\\', "/");
                        p == clean || p.starts_with(&format!("{clean}/"))
                    })
                    .collect());
            }
        }
        Ok(all)
    }

    fn extract_file_changes(diff: &git2::Diff) -> Result<Vec<FileChange>> {
        let mut changes = Vec::new();
        let deltas_count = diff.deltas().len();

        for i in 0..deltas_count {
            if let Some(delta) = diff.get_delta(i) {
                let status = match delta.status() {
                    Delta::Added => FileStatus::Added,
                    Delta::Deleted => FileStatus::Deleted,
                    Delta::Modified => FileStatus::Modified,
                    Delta::Renamed => FileStatus::Renamed,
                    Delta::Copied => FileStatus::Copied,
                    _ => FileStatus::Other,
                };

                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();

                let old_path = if delta.status() == Delta::Renamed {
                    delta
                        .old_file()
                        .path()
                        .map(|p| p.to_string_lossy().to_string())
                } else {
                    None
                };

                let additions = 0;
                let deletions = 0;

                changes.push(FileChange {
                    path,
                    old_path,
                    status,
                    additions,
                    deletions,
                });
            }
        }

        // Calculate line stats across the diff
        let stats = diff.stats()?;
        if !changes.is_empty() {
            // Apply total stats to the changes
            changes[0].additions = stats.insertions();
            changes[0].deletions = stats.deletions();
        }

        Ok(changes)
    }

    pub fn detect_breaking_changes(commits: &[CommitInfo]) -> Vec<BreakingChangeCandidate> {
        let mut candidates = Vec::new();

        for commit in commits {
            let full_message = match &commit.body {
                Some(b) => format!("{}\n\n{}", commit.summary, b),
                None => commit.summary.clone(),
            };

            let parsed = parse_conventional(&full_message);
            if parsed.is_breaking {
                candidates.push(BreakingChangeCandidate {
                    commit_hash: commit.hash.clone(),
                    summary: commit.summary.clone(),
                    reason: parsed
                        .breaking_reason
                        .unwrap_or_else(|| "Marked with breaking change indicator".to_string()),
                });
            }
        }

        candidates
    }

    pub fn filter_changes(
        items: &[ChangeItem],
        filter: &ChangeFilter,
        excluded_hashes: &[String],
    ) -> Vec<ChangeItem> {
        let excluded: HashSet<&str> = excluded_hashes.iter().map(|s| s.as_str()).collect();

        items
            .iter()
            .filter(|item| {
                if let Some(hash) = &item.commit_hash
                    && excluded.contains(hash.as_str())
                {
                    return false;
                }

                if let Some(cat) = filter.category
                    && item.category != cat
                {
                    return false;
                }

                if let Some(ref kw) = filter.keyword {
                    let kw_lower = kw.to_lowercase();
                    let title_match = item.title.to_lowercase().contains(&kw_lower);
                    let desc_match = item
                        .description
                        .as_ref()
                        .map(|d| d.to_lowercase().contains(&kw_lower))
                        .unwrap_or(false);

                    if !title_match && !desc_match {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    pub fn commit_patch(repo: &Repository, oid: Oid, max_bytes: usize) -> Result<PatchResult> {
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        let diff =
            repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts))?;

        let mut patch_bytes = Vec::new();
        let mut truncated = false;

        diff.print(git2::DiffFormat::Patch, |_, _, line| {
            if patch_bytes.len() >= max_bytes {
                truncated = true;
            } else {
                let remaining = max_bytes - patch_bytes.len();
                if line.content().len() > remaining {
                    patch_bytes.extend_from_slice(&line.content()[..remaining]);
                    truncated = true;
                } else {
                    patch_bytes.extend_from_slice(line.content());
                }
            }
            true
        })?;

        let patch = String::from_utf8_lossy(&patch_bytes).to_string();
        Ok(PatchResult { patch, truncated })
    }

    pub fn check_repo_available(project: &Project) -> bool {
        if !project.repository_path.exists() {
            return false;
        }
        Repository::open(&project.repository_path).is_ok()
    }

    pub fn generate_deterministic_change_items(
        repo: &Repository,
        range: &CommitRange,
        mapper: &CategoryMapper,
    ) -> Result<Vec<ChangeItem>> {
        Self::generate_deterministic_change_items_with_path(repo, range, mapper, None)
    }

    pub fn generate_deterministic_change_items_with_path(
        repo: &Repository,
        range: &CommitRange,
        mapper: &CategoryMapper,
        path_filter: Option<&str>,
    ) -> Result<Vec<ChangeItem>> {
        let commits = Self::commits_in_range_with_path(repo, range, path_filter)?;
        let mut items = Vec::new();

        for commit in commits {
            let full_msg = match &commit.body {
                Some(b) => format!("{}\n\n{}", commit.summary, b),
                None => commit.summary.clone(),
            };
            items.push(to_change_item(&commit.hash, &full_msg, mapper));
        }

        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    fn create_test_repo() -> (tempfile::TempDir, Repository) {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let sig = git2::Signature::now("Tester", "tester@example.com").unwrap();

        let write_and_commit =
            |repo: &Repository, filename: &str, content: &str, msg: &str| -> Oid {
                let path = dir.path().join(filename);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(&path, content).unwrap();

                let mut index = repo.index().unwrap();
                index.add_path(Path::new(filename)).unwrap();
                index.write().unwrap();

                let tree_id = index.write_tree().unwrap();
                let tree = repo.find_tree(tree_id).unwrap();

                let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
                let parents: Vec<&git2::Commit> = match &parent_commit {
                    Some(c) => vec![c],
                    None => vec![],
                };

                repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
                    .unwrap()
            };

        let c1 = write_and_commit(
            &repo,
            "README.md",
            "# Test Project\n",
            "chore: initial commit",
        );
        let obj1 = repo.find_object(c1, None).unwrap();
        repo.tag("v0.1.0", &obj1, &sig, "v0.1.0", false).unwrap();
        drop(obj1);

        write_and_commit(
            &repo,
            "src/auth.rs",
            "pub fn login() {}\n",
            "feat: add user login",
        );
        let c3 = write_and_commit(
            &repo,
            "README.md",
            "# Test Project\nUpdated docs\n",
            "docs: update readme",
        );
        let obj3 = repo.find_object(c3, None).unwrap();
        repo.tag("v0.2.0", &obj3, &sig, "v0.2.0", false).unwrap();
        drop(obj3);

        write_and_commit(
            &repo,
            "src/auth.rs",
            "pub fn login_v2() {}\n",
            "feat!: break old api",
        );
        write_and_commit(
            &repo,
            "config.yaml",
            "v: 2\n",
            "fix: new config\n\nBREAKING CHANGE: format changed to yaml",
        );

        (dir, repo)
    }

    #[test]
    fn open_valid_and_reject_non_repo() {
        let (dir, _) = create_test_repo();
        assert!(GitService::open(dir.path()).is_ok());

        let empty_dir = tempdir().unwrap();
        assert!(matches!(
            GitService::open(empty_dir.path()),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn summary_provides_branch_tag_and_dirty_state() {
        let (dir, repo) = create_test_repo();
        let summary = GitService::summary(&repo).unwrap();

        assert!(!summary.name.is_empty());
        assert_eq!(summary.latest_tag.as_deref(), Some("v0.2.0"));
        assert!(!summary.is_dirty);

        // Make dirty
        fs::write(dir.path().join("dirty.txt"), "dirty").unwrap();
        let dirty_summary = GitService::summary(&repo).unwrap();
        assert!(dirty_summary.is_dirty);
        assert!(dirty_summary.dirty_files.contains(&"dirty.txt".to_string()));
    }

    #[test]
    fn list_tags_semver_sorted() {
        let (_dir, repo) = create_test_repo();
        let tags = GitService::list_tags(&repo).unwrap();
        assert_eq!(tags, vec!["v0.2.0", "v0.1.0"]);
    }

    #[test]
    fn resolve_range_and_commits_in_range() {
        let (_dir, repo) = create_test_repo();
        let range = GitService::resolve_range(&repo, Some("v0.1.0"), "HEAD").unwrap();
        let commits = GitService::commits_in_range(&repo, &range).unwrap();

        // 4 commits between v0.1.0 and HEAD
        assert_eq!(commits.len(), 4);
        assert_eq!(commits[0].summary, "fix: new config");
        assert_eq!(commits[1].summary, "feat!: break old api");

        // Missing ref should return NotFound
        assert!(matches!(
            GitService::resolve_range(&repo, Some("non-existent"), "HEAD"),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn detect_breaking_changes_identifies_both_candidates() {
        let (_dir, repo) = create_test_repo();
        let range = GitService::resolve_range(&repo, Some("v0.1.0"), "HEAD").unwrap();
        let commits = GitService::commits_in_range(&repo, &range).unwrap();
        let candidates = GitService::detect_breaking_changes(&commits);

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].summary, "fix: new config");
        assert_eq!(candidates[1].summary, "feat!: break old api");
    }

    #[test]
    fn filter_and_exclude_changes() {
        let (_dir, repo) = create_test_repo();
        let range = GitService::resolve_range(&repo, Some("v0.1.0"), "HEAD").unwrap();
        let mapper = CategoryMapper::new();
        let items =
            GitService::generate_deterministic_change_items(&repo, &range, &mapper).unwrap();

        assert_eq!(items.len(), 4);

        // Exclude one commit
        let excluded_hash = items[0].commit_hash.clone().unwrap();
        let filtered =
            GitService::filter_changes(&items, &ChangeFilter::default(), &[excluded_hash]);
        assert_eq!(filtered.len(), 3);

        // Filter by category
        let filter_feat = ChangeFilter {
            category: Some(ChangeCategory::Feature),
            ..Default::default()
        };
        let feat_items = GitService::filter_changes(&items, &filter_feat, &[]);
        assert_eq!(feat_items.len(), 2);
    }

    #[test]
    fn commit_patch_and_truncation() {
        let (_dir, repo) = create_test_repo();
        let head_oid = repo.head().unwrap().peel_to_commit().unwrap().id();

        // Generous limit
        let patch_full = GitService::commit_patch(&repo, head_oid, 10000).unwrap();
        assert!(!patch_full.truncated);
        assert!(patch_full.patch.contains("v: 2"));

        // Very small limit forces truncation
        let patch_trunc = GitService::commit_patch(&repo, head_oid, 10).unwrap();
        assert!(patch_trunc.truncated);
    }

    #[test]
    fn check_repo_availability() {
        let (dir, _) = create_test_repo();
        let project = Project::new("test", dir.path(), "main");
        assert!(GitService::check_repo_available(&project));

        let deleted_dir = tempdir().unwrap();
        let deleted_path = deleted_dir.path().to_path_buf();
        drop(deleted_dir);

        let missing_project = Project::new("missing", deleted_path, "main");
        assert!(!GitService::check_repo_available(&missing_project));
    }

    #[test]
    fn create_annotated_tag_succeeds_with_clean_tree() {
        let (_dir, repo) = create_test_repo();
        let tag_name = "v0.3.0";
        let message = "Release v0.3.0 notes summary";

        let tag_oid = GitService::create_annotated_tag(&repo, tag_name, message).unwrap();
        assert!(!tag_oid.is_empty());

        let tags = GitService::list_tags(&repo).unwrap();
        assert_eq!(tags.first().unwrap(), "v0.3.0");

        let tag_ref = repo.find_reference("refs/tags/v0.3.0").unwrap();
        let tag_obj = tag_ref.peel(git2::ObjectType::Tag).unwrap();
        let tag = tag_obj.as_tag().unwrap();
        assert_eq!(tag.message().unwrap(), Some(message));
    }

    #[test]
    fn create_annotated_tag_rejects_dirty_working_tree() {
        let (dir, repo) = create_test_repo();
        // Modify a file in working directory
        fs::write(dir.path().join("dirty.txt"), "uncommitted work").unwrap();

        let err = GitService::create_annotated_tag(&repo, "v0.3.0", "Release v0.3.0").unwrap_err();
        assert!(err.to_string().contains("Working tree is dirty"));
    }

    #[test]
    fn create_annotated_tag_rejects_existing_tag() {
        let (_dir, repo) = create_test_repo();
        // v0.2.0 is already created in create_test_repo
        let err = GitService::create_annotated_tag(&repo, "v0.2.0", "Duplicate").unwrap_err();
        assert!(err.to_string().contains("Tag 'v0.2.0' already exists"));
    }

    #[test]
    fn create_annotated_tag_rejects_detached_head() {
        let (_dir, repo) = create_test_repo();
        let head_commit = repo.head().unwrap().peel_to_commit().unwrap().id();
        repo.set_head_detached(head_commit).unwrap();

        let err = GitService::create_annotated_tag(&repo, "v0.3.0", "Detached").unwrap_err();
        assert!(
            err.to_string()
                .contains("Cannot create tag: HEAD is detached")
        );
    }

    #[test]
    fn create_annotated_tag_rejects_hyphen_prefixed_tag() {
        let (_dir, repo) = create_test_repo();
        let err = GitService::create_annotated_tag(&repo, "-f", "Option injection").unwrap_err();
        assert!(err.to_string().contains("cannot start with a hyphen"));

        let err2 = GitService::create_annotated_tag(&repo, "--force", "Option injection").unwrap_err();
        assert!(err2.to_string().contains("cannot start with a hyphen"));
    }

    #[test]
    fn create_annotated_tag_rejects_invalid_ref_name() {
        let (_dir, repo) = create_test_repo();
        let err = GitService::create_annotated_tag(&repo, "bad tag with spaces", "Invalid").unwrap_err();
        assert!(err.to_string().contains("Invalid Git tag reference name"));
    }

    #[test]
    fn filter_commits_by_package_path() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();
        let sig = git2::Signature::now("Dev", "dev@example.com").unwrap();

        let commit_file = |rel_path: &str, msg: &str| -> Oid {
            let full = dir.path().join(rel_path);
            if let Some(p) = full.parent() {
                fs::create_dir_all(p).unwrap();
            }
            fs::write(&full, format!("data for {rel_path}")).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(Path::new(rel_path)).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let parents: Vec<&git2::Commit> = match &parent {
                Some(c) => vec![c],
                None => vec![],
            };
            repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
                .unwrap()
        };

        let _c1 = commit_file("crates/alpha/src/lib.rs", "feat(alpha): initial alpha");
        let _c2 = commit_file("crates/beta/src/lib.rs", "feat(beta): initial beta");
        let _c3 = commit_file("crates/alpha/tests/test.rs", "test(alpha): add tests");
        let head = commit_file("crates/beta/Cargo.toml", "chore(beta): update dependencies");

        let range = CommitRange {
            from: None,
            to: head,
        };

        // 1. Filter crates/alpha
        let alpha_commits =
            GitService::commits_in_range_with_path(&repo, &range, Some("crates/alpha")).unwrap();
        assert_eq!(alpha_commits.len(), 2);
        assert!(
            alpha_commits
                .iter()
                .any(|c| c.summary.contains("feat(alpha)"))
        );
        assert!(
            alpha_commits
                .iter()
                .any(|c| c.summary.contains("test(alpha)"))
        );
        assert!(!alpha_commits.iter().any(|c| c.summary.contains("beta")));

        // 2. Filter crates/beta
        let beta_commits =
            GitService::commits_in_range_with_path(&repo, &range, Some("crates/beta")).unwrap();
        assert_eq!(beta_commits.len(), 2);
        assert!(
            beta_commits
                .iter()
                .any(|c| c.summary.contains("feat(beta)"))
        );
        assert!(
            beta_commits
                .iter()
                .any(|c| c.summary.contains("chore(beta)"))
        );
        assert!(!beta_commits.iter().any(|c| c.summary.contains("alpha")));

        // 3. Unfiltered
        let all_commits = GitService::commits_in_range_with_path(&repo, &range, None).unwrap();
        assert_eq!(all_commits.len(), 4);
    }
}
