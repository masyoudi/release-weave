use std::collections::HashSet;

use serde::Deserialize;

use crate::domain::commit::ChangeCategory;
use crate::domain::provenance::Origin;
use crate::domain::release::{ChangeItem, SourceRef};
use crate::errors::{AppError, Result};

#[derive(Debug, Deserialize)]
pub struct AiResponseRaw {
    pub items: Vec<AiItemRaw>,
}

#[derive(Debug, Deserialize)]
pub struct AiItemRaw {
    pub category: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub commit_hashes: Option<Vec<String>>,
}

pub fn parse_ai_json(raw: &str) -> Result<AiResponseRaw> {
    let trimmed = raw.trim();
    let cleaned = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else {
        trimmed
    };

    serde_json::from_str::<AiResponseRaw>(cleaned).map_err(|e| {
        AppError::Ai(format!(
            "Failed to parse structured JSON from AI output: {e}. Raw response:\n{raw}"
        ))
    })
}

pub fn validate_and_convert_items(
    raw_response: &str,
    valid_hashes: &HashSet<String>,
    provider_name: &str,
    model_name: &str,
) -> Result<Vec<ChangeItem>> {
    let parsed = parse_ai_json(raw_response)?;
    let mut items = Vec::new();

    for raw_item in parsed.items {
        let category = match raw_item.category.as_deref().unwrap_or("other") {
            "feature" => ChangeCategory::Feature,
            "fix" => ChangeCategory::Fix,
            "refactor" => ChangeCategory::Refactor,
            "docs" => ChangeCategory::Docs,
            "build" => ChangeCategory::Build,
            "perf" => ChangeCategory::Perf,
            "test" => ChangeCategory::Test,
            "chore" => ChangeCategory::Chore,
            _ => ChangeCategory::Other,
        };

        let mut verified_sources = Vec::new();
        let mut has_unverified = false;

        let declared_hashes = raw_item.commit_hashes.unwrap_or_default();
        if declared_hashes.is_empty() {
            has_unverified = true;
        } else {
            for h in declared_hashes {
                let clean_h = h.trim();
                let is_valid = valid_hashes
                    .iter()
                    .any(|vh| vh.starts_with(clean_h) || clean_h.starts_with(vh));
                if is_valid {
                    verified_sources.push(SourceRef::Commit(clean_h.to_string()));
                } else {
                    has_unverified = true;
                }
            }
        }

        let confidence = if has_unverified && verified_sources.is_empty() {
            0.3 // Unsourced hallucination candidate
        } else if has_unverified {
            0.7
        } else {
            1.0
        };

        let primary_hash = verified_sources.first().and_then(|s| match s {
            SourceRef::Commit(h) => Some(h.clone()),
            _ => None,
        });

        items.push(ChangeItem {
            id: None,
            release_id: None,
            commit_hash: primary_hash,
            category,
            title: raw_item.title,
            description: raw_item.description,
            source_refs: verified_sources,
            origin: Origin::Ai {
                provider: provider_name.to_string(),
                model: model_name.to_string(),
            },
            confidence,
            is_included: true,
        });
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_json_with_codeblock_wrapper() {
        let raw = r#"```json
        {
            "items": [
                {
                    "category": "feature",
                    "title": "Add OAuth2 integration",
                    "description": "Supports GitHub and Google logins",
                    "commit_hashes": ["abc1234"]
                }
            ]
        }
        ```"#;

        let mut valid_hashes = HashSet::new();
        valid_hashes.insert("abc1234567890".to_string());

        let items = validate_and_convert_items(raw, &valid_hashes, "ollama", "qwen2.5").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Add OAuth2 integration");
        assert_eq!(items[0].category, ChangeCategory::Feature);
        assert_eq!(items[0].confidence, 1.0);
    }

    #[test]
    fn marks_unsourced_items_with_low_confidence() {
        let raw = r#"{
            "items": [
                {
                    "category": "fix",
                    "title": "Fabricated fix without commit",
                    "commit_hashes": ["fake_non_existent_hash"]
                }
            ]
        }"#;

        let valid_hashes = HashSet::new();
        let items = validate_and_convert_items(raw, &valid_hashes, "ollama", "qwen2.5").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].confidence, 0.3);
        assert!(items[0].source_refs.is_empty());
    }

    #[test]
    fn rejects_malformed_json() {
        let raw = "{ broken json items: [ }";
        let valid_hashes = HashSet::new();
        let res = validate_and_convert_items(raw, &valid_hashes, "ollama", "qwen2.5");
        assert!(matches!(res, Err(AppError::Ai(_))));
    }
}
