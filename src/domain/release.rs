use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::commit::ChangeCategory;
use super::provenance::Origin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseStatus {
    Draft,
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "lowercase")]
pub enum SourceRef {
    Commit(String),
    File(String),
    Issue(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeItem {
    pub id: Option<i64>,
    pub release_id: Option<i64>,
    pub commit_hash: Option<String>,
    pub category: ChangeCategory,
    pub title: String,
    pub description: Option<String>,
    pub source_refs: Vec<SourceRef>,
    pub origin: Origin,
    pub confidence: f32,
    pub is_included: bool,
}

impl ChangeItem {
    pub fn new_deterministic(
        commit_hash: impl Into<String>,
        category: ChangeCategory,
        title: impl Into<String>,
    ) -> Self {
        let hash = commit_hash.into();
        Self {
            id: None,
            release_id: None,
            commit_hash: Some(hash.clone()),
            category,
            title: title.into(),
            description: None,
            source_refs: vec![SourceRef::Commit(hash)],
            origin: Origin::Deterministic,
            confidence: 1.0,
            is_included: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    pub id: Option<i64>,
    pub project_id: i64,
    #[serde(default)]
    pub package_id: Option<i64>,
    pub version: String,
    pub previous_version: Option<String>,
    pub start_commit: Option<String>,
    pub end_commit: String,
    pub status: ReleaseStatus,
    pub release_notes: String,
    pub model_metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl Release {
    pub fn new_draft(
        project_id: i64,
        version: impl Into<String>,
        previous_version: Option<String>,
        start_commit: Option<String>,
        end_commit: impl Into<String>,
    ) -> Self {
        Self {
            id: None,
            project_id,
            package_id: None,
            version: version.into(),
            previous_version,
            start_commit,
            end_commit: end_commit.into(),
            status: ReleaseStatus::Draft,
            release_notes: String::new(),
            model_metadata: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    pub fn with_package_id(mut self, package_id: Option<i64>) -> Self {
        self.package_id = package_id;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_json_roundtrip() {
        let release = Release {
            id: Some(1),
            project_id: 10,
            package_id: Some(5),
            version: "0.2.0".to_string(),
            previous_version: Some("0.1.0".to_string()),
            start_commit: Some("commit-aaa".to_string()),
            end_commit: "commit-bbb".to_string(),
            status: ReleaseStatus::Draft,
            release_notes: "## Changes\n- initial draft".to_string(),
            model_metadata: Some("qwen2.5-coder:7b".to_string()),
            created_at: Utc::now(),
            completed_at: None,
        };

        let json = serde_json::to_string(&release).expect("serialize release");
        let deserialized: Release = serde_json::from_str(&json).expect("deserialize release");
        assert_eq!(release, deserialized);
    }

    #[test]
    fn change_item_json_roundtrip() {
        let item = ChangeItem {
            id: Some(101),
            release_id: Some(1),
            commit_hash: Some("abcdef123456".to_string()),
            category: ChangeCategory::Feature,
            title: "Add user authentication".to_string(),
            description: Some("Support JWT login".to_string()),
            source_refs: vec![
                SourceRef::Commit("abcdef123456".to_string()),
                SourceRef::File("src/auth.rs".to_string()),
                SourceRef::Issue("#42".to_string()),
            ],
            origin: Origin::Ai {
                provider: "ollama".to_string(),
                model: "qwen2.5-coder".to_string(),
            },
            confidence: 0.95,
            is_included: true,
        };

        let json = serde_json::to_string(&item).expect("serialize change item");
        let deserialized: ChangeItem =
            serde_json::from_str(&json).expect("deserialize change item");
        assert_eq!(item, deserialized);
    }
}
