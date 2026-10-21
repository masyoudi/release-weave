#![allow(dead_code)]

use std::fs;
use std::path::Path;
use tempfile::TempDir;

pub fn commit_file(
    repo: &git2::Repository,
    file_rel_path: &str,
    content: &str,
    message: &str,
) -> git2::Oid {
    let workdir = repo.workdir().expect("repository has working directory");
    let full_path = workdir.join(file_rel_path);

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("failed to create parent directories");
    }
    fs::write(&full_path, content).expect("failed to write file");

    let mut index = repo.index().expect("failed to get repo index");
    index
        .add_path(Path::new(file_rel_path))
        .expect("failed to add path to index");
    index.write().expect("failed to write index");

    let tree_id = index.write_tree().expect("failed to write tree");
    let tree = repo.find_tree(tree_id).expect("failed to find tree");

    let signature = git2::Signature::now("ReleaseWeave Bot", "bot@releaseweave.dev")
        .expect("failed to create signature");

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let parents: Vec<&git2::Commit> = match &parent_commit {
        Some(c) => vec![c],
        None => vec![],
    };

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )
    .expect("failed to create commit")
}

pub fn create_tag(repo: &git2::Repository, commit_oid: git2::Oid, tag_name: &str) {
    let target = repo
        .find_object(commit_oid, Some(git2::ObjectType::Commit))
        .expect("target commit exists");
    let signature = git2::Signature::now("ReleaseWeave Bot", "bot@releaseweave.dev")
        .expect("failed to create signature");

    repo.tag(tag_name, &target, &signature, tag_name, false)
        .expect("failed to create tag");
}

pub fn make_fixture_repo() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    let repo = git2::Repository::init(dir.path()).expect("failed to init repo");

    let c1 = commit_file(
        &repo,
        "README.md",
        "# Fixture Project\n",
        "chore: initial commit",
    );
    create_tag(&repo, c1, "v0.1.0");

    commit_file(
        &repo,
        "src/auth.rs",
        "pub fn login() {}\n",
        "feat: add user authentication",
    );
    commit_file(
        &repo,
        "src/auth.rs",
        "pub fn login() { /* sanitized */ }\n",
        "fix: prevent token leak in logs",
    );
    let c4 = commit_file(
        &repo,
        "README.md",
        "# Fixture Project\nSee auth guide.\n",
        "docs: update auth setup guide",
    );
    create_tag(&repo, c4, "v0.2.0");

    commit_file(
        &repo,
        "src/auth.rs",
        "pub fn login_fast() {}\n",
        "refactor: optimize token verification",
    );
    commit_file(
        &repo,
        "src/auth.rs",
        "pub fn login_v2() {}\n",
        "feat!: drop legacy authentication endpoint",
    );
    commit_file(
        &repo,
        "config.yaml",
        "version: 2\n",
        "feat: redesign configuration format\n\nBREAKING CHANGE: the config file now requires yaml format",
    );
    commit_file(
        &repo,
        "config.yaml",
        "version: 2\nstrict: true\n",
        "fix: handle empty config edge case",
    );

    dir
}

pub fn make_untagged_repo() -> TempDir {
    let dir = TempDir::new().expect("failed to create temp dir");
    let repo = git2::Repository::init(dir.path()).expect("failed to init repo");

    commit_file(&repo, "README.md", "# Untagged\n", "chore: initial commit");
    commit_file(
        &repo,
        "src/lib.rs",
        "pub fn ready() -> bool { true }\n",
        "feat: implement initial logic",
    );

    dir
}

pub fn make_dirty_repo() -> TempDir {
    let dir = make_fixture_repo();
    let dirty_path = dir.path().join("README.md");
    fs::write(&dirty_path, "# Dirty Working Tree Modification\n")
        .expect("failed to write dirty changes");

    let untracked_path = dir.path().join("untracked.txt");
    fs::write(&untracked_path, "untracked content\n").expect("failed to write untracked file");

    dir
}
