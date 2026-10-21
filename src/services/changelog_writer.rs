use regex::Regex;
use semver::Version;

use crate::errors::{AppError, Result};

pub struct ChangelogWriter;

impl ChangelogWriter {
    pub fn insert_entry(
        existing_content: Option<&str>,
        new_version: &Version,
        release_date: &str,
        release_notes: &str,
    ) -> Result<String> {
        let ver_str = new_version.to_string();

        if let Some(content) = existing_content
            && (content.contains(&format!("## [{ver_str}]"))
                || content.contains(&format!("## {ver_str}"))
                || content.contains(&format!("## [v{ver_str}]"))
                || content.contains(&format!("## v{ver_str}")))
        {
            return Err(AppError::Validation(format!(
                "CHANGELOG.md already contains an entry for version {ver_str}"
            )));
        }

        let clean_notes = release_notes.trim();
        let formatted_entry = if clean_notes.is_empty() {
            format!("## [{ver_str}] - {release_date}\n")
        } else {
            format!("## [{ver_str}] - {release_date}\n\n{clean_notes}\n")
        };

        let Some(content) = existing_content else {
            return Ok(format!(
                "# Changelog\n\nAll notable changes to this project will be documented in this file.\n\nThe format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\nand this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n{formatted_entry}"
            ));
        };

        if content.trim().is_empty() {
            return Ok(format!(
                "# Changelog\n\nAll notable changes to this project will be documented in this file.\n\nThe format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),\nand this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).\n\n{formatted_entry}"
            ));
        }

        // Case 1: Has ## [Unreleased] section
        let re_unreleased = Regex::new(r#"(?im)^##\s*\[?unreleased\]?.*$"#)
            .map_err(|e| AppError::Validation(format!("Regex error: {e}")))?;

        if let Some(mat) = re_unreleased.find(content) {
            let end_idx = mat.end();
            let (prefix, suffix) = content.split_at(end_idx);
            let result = format!("{}\n\n{}{}", prefix.trim_end(), formatted_entry, suffix);
            return Ok(result);
        }

        // Case 2: No unreleased section, find first version heading
        let re_first_version = Regex::new(r#"(?m)^##\s+\[?\d+\.\d+"#)
            .map_err(|e| AppError::Validation(format!("Regex error: {e}")))?;

        if let Some(mat) = re_first_version.find(content) {
            let start_idx = mat.start();
            let (prefix, suffix) = content.split_at(start_idx);
            let result = format!(
                "{}\n\n{}{}",
                prefix.trim_end(),
                formatted_entry,
                suffix.trim_start()
            );
            return Ok(result);
        }

        // Case 3: Just description without any version headings
        Ok(format!("{}\n\n{}", content.trim_end(), formatted_entry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_new_changelog_if_missing() {
        let ver = Version::parse("0.1.0").unwrap();
        let generated =
            ChangelogWriter::insert_entry(None, &ver, "2026-10-21", "- Initial release").unwrap();

        assert!(generated.contains("# Changelog"));
        assert!(generated.contains("## [0.1.0] - 2026-10-21"));
        assert!(generated.contains("- Initial release"));
    }

    #[test]
    fn inserts_below_unreleased_heading() {
        let existing = r#"# Changelog

## [Unreleased]

## [0.1.0] - 2026-10-01
- First version
"#;
        let ver = Version::parse("0.2.0").unwrap();
        let updated = ChangelogWriter::insert_entry(
            Some(existing),
            &ver,
            "2026-10-21",
            "### Added\n- New shiny feature",
        )
        .unwrap();

        assert!(updated.contains("## [Unreleased]\n\n## [0.2.0] - 2026-10-21"));
        assert!(updated.contains("### Added\n- New shiny feature"));
        assert!(updated.contains("## [0.1.0] - 2026-10-01"));
    }

    #[test]
    fn inserts_above_first_version_when_no_unreleased() {
        let existing = r#"# Changelog

All notable changes documented here.

## [0.1.0] - 2026-10-01
- First version
"#;
        let ver = Version::parse("0.2.0").unwrap();
        let updated =
            ChangelogWriter::insert_entry(Some(existing), &ver, "2026-10-21", "- Second version")
                .unwrap();

        let v2_pos = updated.find("## [0.2.0] - 2026-10-21").unwrap();
        let v1_pos = updated.find("## [0.1.0] - 2026-10-01").unwrap();
        assert!(v2_pos < v1_pos);
    }

    #[test]
    fn rejects_duplicate_version() {
        let existing = r#"# Changelog

## [0.1.0] - 2026-10-01
- First version
"#;
        let ver = Version::parse("0.1.0").unwrap();
        let err = ChangelogWriter::insert_entry(Some(existing), &ver, "2026-10-21", "duplicate")
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("already contains an entry for version 0.1.0")
        );
    }
}
