mod common;

use common::fixture::*;

#[test]
fn fixture_repo_has_expected_commits_and_tags() {
    let fixture = make_fixture_repo();
    let repo = git2::Repository::open(fixture.path()).expect("open fixture repo");

    let tags = repo.tag_names(None).expect("fetch tag names");
    let tag_list: Vec<&str> = tags.iter().filter_map(|t| t.ok().flatten()).collect();
    assert_eq!(tag_list, vec!["v0.1.0", "v0.2.0"]);

    let mut revwalk = repo.revwalk().expect("create revwalk");
    revwalk.push_head().expect("push HEAD");
    let commit_count = revwalk.count();
    assert_eq!(commit_count, 8);

    let statuses = repo.statuses(None).expect("get repo statuses");
    assert!(statuses.is_empty(), "working tree should be clean");
}

#[test]
fn untagged_repo_has_no_tags() {
    let untagged = make_untagged_repo();
    let repo = git2::Repository::open(untagged.path()).expect("open untagged repo");

    let tags = repo.tag_names(None).expect("fetch tag names");
    assert_eq!(tags.len(), 0);

    let mut revwalk = repo.revwalk().expect("create revwalk");
    revwalk.push_head().expect("push HEAD");
    assert_eq!(revwalk.count(), 2);
}

#[test]
fn dirty_repo_reports_modified_and_untracked() {
    let dirty = make_dirty_repo();
    let repo = git2::Repository::open(dirty.path()).expect("open dirty repo");

    let statuses = repo.statuses(None).expect("get repo statuses");
    assert!(!statuses.is_empty());

    let mut has_modified = false;
    let mut has_untracked = false;

    for entry in statuses.iter() {
        let status = entry.status();
        if status.contains(git2::Status::WT_MODIFIED) {
            has_modified = true;
        }
        if status.contains(git2::Status::WT_NEW) {
            has_untracked = true;
        }
    }

    assert!(has_modified, "should have modified file in working tree");
    assert!(has_untracked, "should have untracked file");
}
