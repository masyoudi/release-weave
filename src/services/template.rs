use std::collections::HashMap;

use chrono::NaiveDate;

use crate::domain::commit::ChangeCategory;
use crate::domain::release::ChangeItem;
use crate::errors::{AppError, Result};

pub const TEMPLATE_KEEP_A_CHANGELOG: &str = r#"## [{{version}}] - {{date}}

### Added
{{#category:feature}}
- {{title}} ({{source_ref}})
{{/category:feature}}

### Fixed
{{#category:fix}}
- {{title}} ({{source_ref}})
{{/category:fix}}

### Changed
{{#category:refactor}}
- {{title}} ({{source_ref}})
{{/category:refactor}}

### Documentation
{{#category:docs}}
- {{title}} ({{source_ref}})
{{/category:docs}}

### Maintenance
{{#category:chore}}
- {{title}} ({{source_ref}})
{{/category:chore}}
"#;

pub const TEMPLATE_SIMPLE_MARKDOWN: &str = r#"# Release {{version}} ({{date}})

{{#category:feature}}
- [Feature] {{title}} ({{source_ref}})
{{/category:feature}}
{{#category:fix}}
- [Fix] {{title}} ({{source_ref}})
{{/category:fix}}
{{#category:refactor}}
- [Refactor] {{title}} ({{source_ref}})
{{/category:refactor}}
{{#category:docs}}
- [Docs] {{title}} ({{source_ref}})
{{/category:docs}}
{{#category:other}}
- [Other] {{title}} ({{source_ref}})
{{/category:other}}
"#;

#[derive(Debug, Clone)]
pub struct TemplateContext<'a> {
    pub version: &'a str,
    pub date: NaiveDate,
    pub previous_version: Option<&'a str>,
    pub items: &'a [ChangeItem],
}

pub struct TemplateEngine;

impl TemplateEngine {
    pub fn validate(template_str: &str) -> Result<()> {
        let allowed_vars = [
            "version",
            "date",
            "previous_version",
            "title",
            "description",
            "commit_hash",
            "short_hash",
            "source_ref",
        ];

        let mut rest = template_str;
        while let Some(start_idx) = rest.find("{{") {
            let after_start = &rest[start_idx + 2..];
            let end_idx = after_start.find("}}").ok_or_else(|| {
                AppError::Validation("Unclosed template variable '{{'".to_string())
            })?;

            let tag = after_start[..end_idx].trim();
            if tag.starts_with('#') || tag.starts_with('/') {
                let block_tag = &tag[1..];
                if !block_tag.starts_with("category:") {
                    return Err(AppError::Validation(format!(
                        "Unknown template block tag '{{{{{tag}}}}}'"
                    )));
                }
            } else if !allowed_vars.contains(&tag) {
                return Err(AppError::Validation(format!(
                    "Unknown template variable '{{{{{tag}}}}}'"
                )));
            }

            rest = &after_start[end_idx + 2..];
        }

        Ok(())
    }

    pub fn render(template_str: &str, ctx: &TemplateContext<'_>) -> Result<String> {
        Self::validate(template_str)?;

        let mut by_category: HashMap<ChangeCategory, Vec<&ChangeItem>> = HashMap::new();
        for item in ctx.items {
            if item.is_included {
                by_category.entry(item.category).or_default().push(item);
            }
        }

        let mut output = String::new();
        let mut lines = template_str.lines().peekable();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();
            if trimmed.starts_with("{{#category:") && trimmed.ends_with("}}") {
                let category_name = &trimmed[12..trimmed.len() - 2].trim();
                let category = match *category_name {
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

                let end_marker = format!("{{{{/category:{category_name}}}}}");
                let mut block_lines = Vec::new();

                for inner_line in lines.by_ref() {
                    if inner_line.trim() == end_marker {
                        break;
                    }
                    block_lines.push(inner_line);
                }

                if let Some(items) = by_category.get(&category) {
                    for item in items {
                        for b_line in &block_lines {
                            let rendered_line = Self::render_item_line(b_line, item);
                            output.push_str(&rendered_line);
                            output.push('\n');
                        }
                    }
                }
            } else {
                let replaced = line
                    .replace("{{version}}", ctx.version)
                    .replace("{{date}}", &ctx.date.format("%Y-%m-%d").to_string())
                    .replace("{{previous_version}}", ctx.previous_version.unwrap_or(""));
                output.push_str(&replaced);
                output.push('\n');
            }
        }

        // Clean up excessive blank lines
        let cleaned = Self::clean_empty_sections(&output);
        Ok(cleaned)
    }

    fn render_item_line(template_line: &str, item: &ChangeItem) -> String {
        let commit_hash = item.commit_hash.as_deref().unwrap_or("");
        let short_hash = if commit_hash.len() >= 7 {
            &commit_hash[..7]
        } else {
            commit_hash
        };
        let desc = item.description.as_deref().unwrap_or("");

        let source_ref_str = if !item.source_refs.is_empty() {
            match &item.source_refs[0] {
                crate::domain::release::SourceRef::Commit(h) => {
                    if h.len() >= 7 {
                        &h[..7]
                    } else {
                        h
                    }
                }
                crate::domain::release::SourceRef::Issue(iss) => iss,
                crate::domain::release::SourceRef::File(f) => f,
            }
        } else {
            short_hash
        };

        template_line
            .replace("{{title}}", &item.title)
            .replace("{{description}}", desc)
            .replace("{{commit_hash}}", commit_hash)
            .replace("{{short_hash}}", short_hash)
            .replace("{{source_ref}}", source_ref_str)
    }

    fn clean_empty_sections(rendered: &str) -> String {
        let mut result = Vec::new();
        let lines: Vec<&str> = rendered.lines().collect();
        let mut i = 0;

        while i < lines.len() {
            let line = lines[i];
            // If line is a heading ### Section and next line is another heading or empty
            if line.starts_with("### ") {
                let mut has_items = false;
                let mut j = i + 1;
                while j < lines.len() {
                    let next_line = lines[j].trim();
                    if next_line.starts_with('#') {
                        break;
                    }
                    if !next_line.is_empty() {
                        has_items = true;
                        break;
                    }
                    j += 1;
                }
                if has_items {
                    result.push(line);
                } else {
                    // Skip empty heading and following blank line
                    i = j;
                    continue;
                }
            } else {
                result.push(line);
            }
            i += 1;
        }

        result.join("\n").trim().to_string() + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_rejects_unknown_variables() {
        assert!(TemplateEngine::validate("{{version}} - {{date}}").is_ok());
        assert!(TemplateEngine::validate("{{unknown_var}}").is_err());
        assert!(
            TemplateEngine::validate("{{#category:feature}}{{title}}{{/category:feature}}").is_ok()
        );
        assert!(TemplateEngine::validate("{{#unknown_block}}{{/unknown_block}}").is_err());
    }

    #[test]
    fn renders_keep_a_changelog_correctly() {
        let items = vec![
            ChangeItem::new_deterministic("a1b2c3d4e5", ChangeCategory::Feature, "Add dark theme"),
            ChangeItem::new_deterministic("f6e5d4c3b2", ChangeCategory::Fix, "Fix memory leak"),
        ];

        let ctx = TemplateContext {
            version: "1.0.0",
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            previous_version: Some("0.9.0"),
            items: &items,
        };

        let rendered = TemplateEngine::render(TEMPLATE_KEEP_A_CHANGELOG, &ctx).unwrap();

        assert!(rendered.contains("## [1.0.0] - 2026-10-20"));
        assert!(rendered.contains("### Added"));
        assert!(rendered.contains("- Add dark theme (a1b2c3d)"));
        assert!(rendered.contains("### Fixed"));
        assert!(rendered.contains("- Fix memory leak (f6e5d4c)"));
        // Empty sections should be cleaned up
        assert!(!rendered.contains("### Changed"));
    }

    #[test]
    fn renders_simple_markdown_correctly() {
        let items = vec![ChangeItem::new_deterministic(
            "1122334455",
            ChangeCategory::Docs,
            "Update readme",
        )];

        let ctx = TemplateContext {
            version: "0.2.0",
            date: NaiveDate::from_ymd_opt(2026, 10, 20).unwrap(),
            previous_version: None,
            items: &items,
        };

        let rendered = TemplateEngine::render(TEMPLATE_SIMPLE_MARKDOWN, &ctx).unwrap();
        assert!(rendered.contains("# Release 0.2.0 (2026-10-20)"));
        assert!(rendered.contains("- [Docs] Update readme (1122334)"));
    }
}
