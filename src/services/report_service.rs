use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::domain::check::{CheckResult, CheckStatus};
use crate::domain::release::{ChangeItem, Release, ReleaseStatus};
use crate::errors::{AppError, Result};
use crate::security::path_validation::validate_output_file;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReleaseReport {
    pub project_name: String,
    pub version: String,
    pub previous_version: Option<String>,
    pub start_commit: Option<String>,
    pub end_commit: String,
    pub status: ReleaseStatus,
    pub model_metadata: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub release_notes: String,
    pub change_items: Vec<ChangeItem>,
    pub readiness_checks: Vec<CheckResult>,
    pub unavailable_data: Vec<String>,
}

impl ReleaseReport {
    pub fn build(
        project_name: impl Into<String>,
        release: &Release,
        change_items: Vec<ChangeItem>,
        readiness_checks: Vec<CheckResult>,
    ) -> Self {
        let mut unavailable_data = Vec::new();

        if release.model_metadata.is_none() {
            unavailable_data.push("AI Model Metadata: Not used (deterministic)".to_string());
        }
        if release.previous_version.is_none() {
            unavailable_data.push("Previous Version: None (initial release)".to_string());
        }
        if release.start_commit.is_none() {
            unavailable_data.push("Start Commit: Not specified (analyzed from root)".to_string());
        }
        if readiness_checks.is_empty() {
            unavailable_data.push("Readiness Checks: No check records".to_string());
        } else {
            let not_run_count = readiness_checks
                .iter()
                .filter(|c| c.status == CheckStatus::NotRun)
                .count();
            if not_run_count > 0 {
                unavailable_data.push(format!("{not_run_count} readiness checks not yet run"));
            }
        }

        Self {
            project_name: project_name.into(),
            version: release.version.clone(),
            previous_version: release.previous_version.clone(),
            start_commit: release.start_commit.clone(),
            end_commit: release.end_commit.clone(),
            status: release.status,
            model_metadata: release.model_metadata.clone(),
            created_at: release.created_at,
            completed_at: release.completed_at,
            release_notes: release.release_notes.clone(),
            change_items,
            readiness_checks,
            unavailable_data,
        }
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| AppError::Validation(e.to_string()))
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Release Report: {} v{}\n\n",
            self.project_name, self.version
        ));

        let status_str = match self.status {
            ReleaseStatus::Draft => "Draft",
            ReleaseStatus::Final => "Final",
        };
        out.push_str(&format!("- **Status:** {}\n", status_str));
        out.push_str(&format!(
            "- **Created:** {}\n",
            self.created_at.to_rfc3339()
        ));
        if let Some(comp) = self.completed_at {
            out.push_str(&format!("- **Finalized:** {}\n", comp.to_rfc3339()));
        }
        let prev = self.previous_version.as_deref().unwrap_or("None");
        out.push_str(&format!("- **Previous Version:** {}\n", prev));

        let start = self.start_commit.as_deref().unwrap_or("root");
        out.push_str(&format!(
            "- **Commit Range:** {}..{}\n",
            start, self.end_commit
        ));

        let ai_info = self.model_metadata.as_deref().unwrap_or("None / Not used");
        out.push_str(&format!("- **AI Model:** {}\n", ai_info));

        if let Some(meta) = &self.model_metadata {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(meta) {
                if let Some(pub_url) = v["published_url"].as_str() {
                    out.push_str(&format!("- **Published Remote Draft:** {}\n", pub_url));
                }
            } else if meta.contains("https://") {
                out.push_str(&format!("- **Published Remote Draft:** {}\n", meta));
            }
        }
        out.push('\n');

        out.push_str("## Release Notes\n\n");
        if self.release_notes.is_empty() {
            out.push_str("*(No release notes)*\n\n");
        } else {
            out.push_str(&self.release_notes);
            out.push_str("\n\n");
        }

        out.push_str("## Readiness Checks\n\n");
        if self.readiness_checks.is_empty() {
            out.push_str("*(No checks)*\n\n");
        } else {
            out.push_str("| Check Type | Status | Required | Details |\n");
            out.push_str("|---|---|---|---|\n");
            for c in &self.readiness_checks {
                let req_str = if c.required { "Yes" } else { "No" };
                let det = c.details.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    c.check_type,
                    c.status.as_str(),
                    req_str,
                    det
                ));
            }
            out.push('\n');
        }

        out.push_str("## Unavailable Data\n\n");
        if self.unavailable_data.is_empty() {
            out.push_str("- All release metadata was collected completely.\n");
        } else {
            for item in &self.unavailable_data {
                out.push_str(&format!("- {}\n", item));
            }
        }

        out
    }

    pub fn export_json(&self, path: &Path) -> Result<PathBuf> {
        let valid_path = validate_output_file(path)?;
        let json = self.to_json()?;
        fs::write(&valid_path, json)?;
        Ok(valid_path)
    }

    pub fn export_markdown(&self, path: &Path) -> Result<PathBuf> {
        let valid_path = validate_output_file(path)?;
        let md = self.to_markdown();
        fs::write(&valid_path, md)?;
        Ok(valid_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::check::CheckResult;
    use crate::domain::commit::ChangeCategory;
    use tempfile::tempdir;

    #[test]
    fn report_json_valid_and_unavailable_data_reported_when_ai_absent() {
        let mut release = Release::new_draft(1, "1.0.0", None, None, "abcdef123456");
        release.release_notes = "### Release 1.0.0\n\n- Initial release".to_string();

        let items = vec![ChangeItem::new_deterministic(
            "abcdef123456",
            ChangeCategory::Feature,
            "Initial setup",
        )];
        let checks = vec![CheckResult {
            id: None,
            release_id: None,
            check_type: "working_tree_clean".to_string(),
            status: CheckStatus::Passed,
            required: true,
            details: None,
            executed_at: Some(Utc::now()),
        }];

        let report = ReleaseReport::build("ReleaseWeave", &release, items, checks);

        assert!(
            report
                .unavailable_data
                .iter()
                .any(|d| d.contains("AI Model Metadata: Not used"))
        );
        assert!(
            report
                .unavailable_data
                .iter()
                .any(|d| d.contains("Previous Version: None"))
        );
        assert!(
            report
                .unavailable_data
                .iter()
                .any(|d| d.contains("Start Commit: Not specified"))
        );

        let json = report.to_json().unwrap();
        let parsed: ReleaseReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.project_name, "ReleaseWeave");
        assert_eq!(parsed.version, "1.0.0");
        assert_eq!(parsed.change_items.len(), 1);

        let md = report.to_markdown();
        assert!(md.contains("# Release Report: ReleaseWeave v1.0.0"));
        assert!(md.contains("AI Model Metadata: Not used"));
    }

    #[test]
    fn exports_json_and_markdown_to_files() {
        let dir = tempdir().unwrap();
        let release = Release::new_draft(1, "1.0.0", Some("0.9.0".into()), Some("a".into()), "b");
        let report = ReleaseReport::build("TestProject", &release, vec![], vec![]);

        let json_path = dir.path().join("report.json");
        let md_path = dir.path().join("report.md");

        report.export_json(&json_path).unwrap();
        report.export_markdown(&md_path).unwrap();

        assert!(json_path.exists());
        assert!(md_path.exists());
        let json_content = fs::read_to_string(&json_path).unwrap();
        assert!(json_content.contains("TestProject"));
    }
}
