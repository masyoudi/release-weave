use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeCategory {
    Feature,
    Fix,
    Refactor,
    Docs,
    Build,
    Perf,
    Test,
    Chore,
    Other,
}

impl ChangeCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Feature => "feature",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Docs => "docs",
            Self::Build => "build",
            Self::Perf => "perf",
            Self::Test => "test",
            Self::Chore => "chore",
            Self::Other => "other",
        }
    }
}

impl fmt::Display for ChangeCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub old_path: Option<String>,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub author: String,
    pub email: String,
    pub time: DateTime<Utc>,
    pub summary: String,
    pub body: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_info_json_roundtrip() {
        let commit = CommitInfo {
            hash: "a1b2c3d4e5f67890123456789abcdef012345678".to_string(),
            short_hash: "a1b2c3d".to_string(),
            author: "Release Maintainer".to_string(),
            email: "maintainer@example.com".to_string(),
            time: Utc::now(),
            summary: "feat: add user authentication".to_string(),
            body: Some("Detailed commit explanation".to_string()),
        };

        let json = serde_json::to_string(&commit).expect("serialize commit");
        let deserialized: CommitInfo = serde_json::from_str(&json).expect("deserialize commit");

        assert_eq!(commit, deserialized);
    }

    #[test]
    fn file_change_json_roundtrip() {
        let file_change = FileChange {
            path: "src/auth.rs".to_string(),
            old_path: None,
            status: FileStatus::Modified,
            additions: 12,
            deletions: 4,
        };

        let json = serde_json::to_string(&file_change).expect("serialize file change");
        let deserialized: FileChange =
            serde_json::from_str(&json).expect("deserialize file change");

        assert_eq!(file_change, deserialized);
    }

    #[test]
    fn change_category_display_and_serde() {
        let cat = ChangeCategory::Feature;
        assert_eq!(cat.to_string(), "feature");

        let json = serde_json::to_string(&cat).expect("serialize category");
        assert_eq!(json, "\"feature\"");
        let deserialized: ChangeCategory =
            serde_json::from_str(&json).expect("deserialize category");
        assert_eq!(cat, deserialized);
    }
}
