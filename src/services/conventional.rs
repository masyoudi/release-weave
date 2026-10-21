use std::collections::HashMap;

use crate::domain::commit::ChangeCategory;
use crate::domain::provenance::Origin;
use crate::domain::release::{ChangeItem, SourceRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionalCommit {
    pub commit_type: Option<String>,
    pub scope: Option<String>,
    pub is_breaking: bool,
    pub breaking_reason: Option<String>,
    pub description: String,
    pub body: Option<String>,
}

pub struct CategoryMapper {
    custom_map: HashMap<String, ChangeCategory>,
}

impl Default for CategoryMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl CategoryMapper {
    pub fn new() -> Self {
        Self {
            custom_map: HashMap::new(),
        }
    }

    pub fn with_mapping(
        mut self,
        commit_type: impl Into<String>,
        category: ChangeCategory,
    ) -> Self {
        self.custom_map
            .insert(commit_type.into().to_lowercase(), category);
        self
    }

    pub fn map(&self, commit_type: Option<&str>) -> ChangeCategory {
        let Some(t) = commit_type else {
            return ChangeCategory::Other;
        };

        let t_lower = t.to_lowercase();
        if let Some(cat) = self.custom_map.get(&t_lower) {
            return *cat;
        }

        match t_lower.as_str() {
            "feat" | "feature" => ChangeCategory::Feature,
            "fix" | "bugfix" => ChangeCategory::Fix,
            "refactor" => ChangeCategory::Refactor,
            "docs" => ChangeCategory::Docs,
            "build" => ChangeCategory::Build,
            "perf" => ChangeCategory::Perf,
            "test" | "tests" => ChangeCategory::Test,
            "chore" | "ci" | "style" => ChangeCategory::Chore,
            _ => ChangeCategory::Other,
        }
    }
}

pub fn parse_conventional(message: &str) -> ConventionalCommit {
    let mut lines = message.lines();
    let first_line = lines.next().unwrap_or("").trim();

    let mut body_lines = Vec::new();
    for line in lines {
        body_lines.push(line);
    }
    let body_text = if body_lines.is_empty() {
        None
    } else {
        Some(body_lines.join("\n").trim().to_string())
    };

    let mut breaking_from_footer = None;
    if let Some(ref body) = body_text {
        for line in body.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("BREAKING CHANGE:") {
                breaking_from_footer = Some(rest.trim().to_string());
            } else if let Some(rest) = trimmed.strip_prefix("BREAKING-CHANGE:") {
                breaking_from_footer = Some(rest.trim().to_string());
            }
        }
    }

    if let Some((header_meta, desc)) = first_line.split_once(':') {
        let header_meta = header_meta.trim();
        let description = desc.trim();

        let (type_scope, has_bang) = if let Some(stripped) = header_meta.strip_suffix('!') {
            (stripped, true)
        } else {
            (header_meta, false)
        };

        let (commit_type, scope) = if let Some((c_type, rest)) = type_scope.split_once('(') {
            if let Some(s) = rest.strip_suffix(')') {
                (c_type.trim().to_string(), Some(s.trim().to_string()))
            } else {
                (type_scope.to_string(), None)
            }
        } else {
            (type_scope.to_string(), None)
        };

        if !commit_type.is_empty()
            && commit_type
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            let is_breaking = has_bang || breaking_from_footer.is_some();
            let breaking_reason = if has_bang {
                breaking_from_footer.or_else(|| Some(description.to_string()))
            } else {
                breaking_from_footer
            };

            return ConventionalCommit {
                commit_type: Some(commit_type),
                scope,
                is_breaking,
                breaking_reason,
                description: description.to_string(),
                body: body_text,
            };
        }
    }

    let is_breaking = breaking_from_footer.is_some();
    ConventionalCommit {
        commit_type: None,
        scope: None,
        is_breaking,
        breaking_reason: breaking_from_footer,
        description: first_line.to_string(),
        body: body_text,
    }
}

pub fn to_change_item(commit_hash: &str, message: &str, mapper: &CategoryMapper) -> ChangeItem {
    let parsed = parse_conventional(message);
    let category = mapper.map(parsed.commit_type.as_deref());

    let title = if let Some(scope) = &parsed.scope {
        format!("{scope}: {}", parsed.description)
    } else {
        parsed.description
    };

    let mut item = ChangeItem::new_deterministic(commit_hash, category, title);
    item.description = parsed.body;
    item.origin = Origin::Deterministic;
    item.source_refs = vec![SourceRef::Commit(commit_hash.to_string())];
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_conventional_commits() {
        let cases = vec![
            (
                "feat: add login page",
                Some("feat"),
                None,
                false,
                "add login page",
            ),
            (
                "fix(auth): handle expired token",
                Some("fix"),
                Some("auth"),
                false,
                "handle expired token",
            ),
            (
                "refactor!: restructure modules",
                Some("refactor"),
                None,
                true,
                "restructure modules",
            ),
            (
                "feat(api)!: remove v1 routes",
                Some("feat"),
                Some("api"),
                true,
                "remove v1 routes",
            ),
            (
                "docs: update installation steps",
                Some("docs"),
                None,
                false,
                "update installation steps",
            ),
            (
                "chore(deps): bump serde from 1.0 to 1.1",
                Some("chore"),
                Some("deps"),
                false,
                "bump serde from 1.0 to 1.1",
            ),
            (
                "perf(db): add index on email column",
                Some("perf"),
                Some("db"),
                false,
                "add index on email column",
            ),
            (
                "test: add unit test for password validation",
                Some("test"),
                None,
                false,
                "add unit test for password validation",
            ),
            (
                "build: configure cross-compilation",
                Some("build"),
                None,
                false,
                "configure cross-compilation",
            ),
            (
                "ordinary non-conventional commit message",
                None,
                None,
                false,
                "ordinary non-conventional commit message",
            ),
        ];

        for (msg, expected_type, expected_scope, expected_breaking, expected_desc) in cases {
            let parsed = parse_conventional(msg);
            assert_eq!(
                parsed.commit_type.as_deref(),
                expected_type,
                "type for: {msg}"
            );
            assert_eq!(parsed.scope.as_deref(), expected_scope, "scope for: {msg}");
            assert_eq!(parsed.is_breaking, expected_breaking, "breaking for: {msg}");
            assert_eq!(parsed.description, expected_desc, "desc for: {msg}");
        }
    }

    #[test]
    fn parses_breaking_change_footer() {
        let msg = "feat: change database schema\n\nBREAKING CHANGE: requires migration script";
        let parsed = parse_conventional(msg);
        assert_eq!(parsed.commit_type.as_deref(), Some("feat"));
        assert!(parsed.is_breaking);
        assert_eq!(
            parsed.breaking_reason.as_deref(),
            Some("requires migration script")
        );
    }

    #[test]
    fn category_mapping_default_and_custom() {
        let mapper = CategoryMapper::new().with_mapping("sec", ChangeCategory::Fix);

        assert_eq!(mapper.map(Some("feat")), ChangeCategory::Feature);
        assert_eq!(mapper.map(Some("fix")), ChangeCategory::Fix);
        assert_eq!(mapper.map(Some("refactor")), ChangeCategory::Refactor);
        assert_eq!(mapper.map(Some("docs")), ChangeCategory::Docs);
        assert_eq!(mapper.map(Some("build")), ChangeCategory::Build);
        assert_eq!(mapper.map(Some("perf")), ChangeCategory::Perf);
        assert_eq!(mapper.map(Some("test")), ChangeCategory::Test);
        assert_eq!(mapper.map(Some("ci")), ChangeCategory::Chore);
        assert_eq!(mapper.map(Some("sec")), ChangeCategory::Fix);
        assert_eq!(mapper.map(None), ChangeCategory::Other);
    }

    #[test]
    fn to_change_item_preserves_hash_and_origin() {
        let mapper = CategoryMapper::new();
        let item = to_change_item(
            "1234567890abcdef",
            "feat(ui): add dark mode toggle",
            &mapper,
        );

        assert_eq!(item.commit_hash.as_deref(), Some("1234567890abcdef"));
        assert_eq!(item.category, ChangeCategory::Feature);
        assert_eq!(item.title, "ui: add dark mode toggle");
        assert_eq!(item.origin, Origin::Deterministic);
        assert_eq!(
            item.source_refs,
            vec![SourceRef::Commit("1234567890abcdef".to_string())]
        );
    }
}
