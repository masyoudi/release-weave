use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Passed,
    Warning,
    Failed,
    Skipped,
    #[default]
    NotRun,
}

impl CheckStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Warning => "warning",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::NotRun => "not_run",
        }
    }

    pub fn is_blocking(&self, required: bool) -> bool {
        required && *self != Self::Passed
    }
}

impl fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckResult {
    pub id: Option<i64>,
    pub release_id: Option<i64>,
    pub check_type: String,
    pub status: CheckStatus,
    pub required: bool,
    pub details: Option<String>,
    pub executed_at: Option<DateTime<Utc>>,
}

impl CheckResult {
    pub fn new(check_type: impl Into<String>, required: bool) -> Self {
        Self {
            id: None,
            release_id: None,
            check_type: check_type.into(),
            status: CheckStatus::NotRun,
            required,
            details: None,
            executed_at: None,
        }
    }

    pub fn pass(mut self, details: impl Into<String>) -> Self {
        self.status = CheckStatus::Passed;
        self.details = Some(details.into());
        self.executed_at = Some(Utc::now());
        self
    }

    pub fn fail(mut self, details: impl Into<String>) -> Self {
        self.status = CheckStatus::Failed;
        self.details = Some(details.into());
        self.executed_at = Some(Utc::now());
        self
    }

    pub fn warn(mut self, details: impl Into<String>) -> Self {
        self.status = CheckStatus::Warning;
        self.details = Some(details.into());
        self.executed_at = Some(Utc::now());
        self
    }

    pub fn skip(mut self, details: impl Into<String>) -> Self {
        self.status = CheckStatus::Skipped;
        self.details = Some(details.into());
        self.executed_at = Some(Utc::now());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_status_default_is_not_run() {
        assert_eq!(CheckStatus::default(), CheckStatus::NotRun);
    }

    #[test]
    fn check_blocking_logic() {
        assert!(CheckStatus::NotRun.is_blocking(true));
        assert!(CheckStatus::Failed.is_blocking(true));
        assert!(CheckStatus::Warning.is_blocking(true));
        assert!(!CheckStatus::Passed.is_blocking(true));
        assert!(!CheckStatus::Failed.is_blocking(false));
    }

    #[test]
    fn check_result_json_roundtrip() {
        let result = CheckResult {
            id: Some(12),
            release_id: Some(3),
            check_type: "working_tree_clean".to_string(),
            status: CheckStatus::Passed,
            required: true,
            details: Some("Working directory has no uncommitted changes".to_string()),
            executed_at: Some(Utc::now()),
        };

        let json = serde_json::to_string(&result).expect("serialize check result");
        let deserialized: CheckResult =
            serde_json::from_str(&json).expect("deserialize check result");
        assert_eq!(result, deserialized);
    }
}
