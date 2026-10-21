use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PackageTagPattern {
    #[default]
    At,
    DashV,
}

impl PackageTagPattern {
    pub fn format(&self, package_name: &str, version: &str) -> String {
        let clean_ver = version.strip_prefix('v').unwrap_or(version);
        match self {
            PackageTagPattern::At => format!("{package_name}@{clean_ver}"),
            PackageTagPattern::DashV => format!("{package_name}-v{clean_ver}"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettings {
    #[serde(default)]
    pub ollama_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub context_limit: Option<usize>,
    #[serde(default)]
    pub readiness_commands: Vec<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub commands_required: bool,
    #[serde(default)]
    pub allow_git_tag: bool,
    #[serde(default)]
    pub package_tag_pattern: Option<PackageTagPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: Option<i64>,
    pub name: String,
    pub repository_path: PathBuf,
    pub default_branch: String,
    pub default_template_id: Option<String>,
    pub settings: ProjectSettings,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Project {
    pub fn new(
        name: impl Into<String>,
        repository_path: impl Into<PathBuf>,
        default_branch: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: None,
            name: name.into(),
            repository_path: repository_path.into(),
            default_branch: default_branch.into(),
            default_template_id: None,
            settings: ProjectSettings::default(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_json_roundtrip() {
        let project = Project {
            id: Some(42),
            name: "release-weave".to_string(),
            repository_path: PathBuf::from("/home/meow/dev/release-weave"),
            default_branch: "main".to_string(),
            default_template_id: Some("keep-a-changelog".to_string()),
            settings: ProjectSettings {
                ollama_url: Some("http://localhost:11434".to_string()),
                model: Some("qwen2.5-coder:7b".to_string()),
                context_limit: Some(8192),
                readiness_commands: vec!["cargo test".to_string()],
                language: Some("en".to_string()),
                commands_required: false,
                allow_git_tag: true,
                package_tag_pattern: Some(PackageTagPattern::At),
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&project).expect("serialize project");
        let deserialized: Project = serde_json::from_str(&json).expect("deserialize project");

        assert_eq!(project, deserialized);
    }
}
