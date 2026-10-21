use regex::Regex;
use semver::Version;
use toml_edit::DocumentMut;

use crate::errors::{AppError, Result};

pub struct ManifestWriter;

impl ManifestWriter {
    pub fn bump_cargo_toml(content: &str, new_version: &Version) -> Result<String> {
        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| AppError::Validation(format!("Invalid Cargo.toml syntax: {e}")))?;

        let package_table = doc
            .get_mut("package")
            .and_then(|p| p.as_table_like_mut())
            .ok_or_else(|| {
                AppError::Validation(
                    "Cargo.toml does not contain a valid [package] table".to_string(),
                )
            })?;

        if let Some(ver_item) = package_table.get_mut("version") {
            if let Some(table) = ver_item.as_table_like()
                && table.get("workspace").and_then(|w| w.as_bool()) == Some(true)
            {
                return Err(AppError::Validation(
                    "Cargo.toml version is inherited from workspace (version.workspace = true)"
                        .to_string(),
                ));
            }

            if let Some(val) = ver_item.as_value_mut() {
                let decor = val.decor().clone();
                let mut new_val = toml_edit::Value::from(new_version.to_string());
                *new_val.decor_mut() = decor;
                *val = new_val;
            } else {
                *ver_item = toml_edit::Item::Value(toml_edit::Value::from(new_version.to_string()));
            }
        } else {
            return Err(AppError::Validation(
                "Cargo.toml [package] table is missing 'version' field".to_string(),
            ));
        }

        Ok(doc.to_string())
    }

    pub fn bump_package_json(content: &str, new_version: &Version) -> Result<String> {
        let parsed: serde_json::Value = serde_json::from_str(content)
            .map_err(|e| AppError::Validation(format!("Invalid package.json syntax: {e}")))?;

        if parsed.get("version").is_none() {
            return Err(AppError::Validation(
                "package.json is missing top-level 'version' field".to_string(),
            ));
        }

        let re = Regex::new(r#"(?m)^(\s*"version"\s*:\s*")[^"]*(")"#)
            .map_err(|e| AppError::Validation(format!("Regex error: {e}")))?;

        if !re.is_match(content) {
            return Err(AppError::Validation(
                "Could not find matching top-level version field in package.json".to_string(),
            ));
        }

        let replacement = format!("${{1}}{new_version}${{2}}");
        let updated = re.replace(content, replacement.as_str()).to_string();
        Ok(updated)
    }

    pub fn bump_pyproject_toml(content: &str, new_version: &Version) -> Result<String> {
        let mut doc = content
            .parse::<DocumentMut>()
            .map_err(|e| AppError::Validation(format!("Invalid pyproject.toml syntax: {e}")))?;

        let ver_str = new_version.to_string();

        if let Some(project) = doc.get_mut("project").and_then(|p| p.as_table_like_mut())
            && let Some(ver_item) = project.get_mut("version")
            && let Some(val) = ver_item.as_value_mut()
        {
            let decor = val.decor().clone();
            let mut new_val = toml_edit::Value::from(ver_str.as_str());
            *new_val.decor_mut() = decor;
            *val = new_val;
            return Ok(doc.to_string());
        }

        if let Some(poetry) = doc
            .get_mut("tool")
            .and_then(|t| t.as_table_like_mut())
            .and_then(|t| t.get_mut("poetry"))
            .and_then(|p| p.as_table_like_mut())
            && let Some(ver_item) = poetry.get_mut("version")
            && let Some(val) = ver_item.as_value_mut()
        {
            let decor = val.decor().clone();
            let mut new_val = toml_edit::Value::from(ver_str.as_str());
            *new_val.decor_mut() = decor;
            *val = new_val;
            return Ok(doc.to_string());
        }

        Err(AppError::Validation(
            "pyproject.toml does not declare 'version' under [project] or [tool.poetry]"
                .to_string(),
        ))
    }

    pub fn bump_manifest(filename: &str, content: &str, new_version: &Version) -> Result<String> {
        match filename {
            "Cargo.toml" => Self::bump_cargo_toml(content, new_version),
            "package.json" => Self::bump_package_json(content, new_version),
            "pyproject.toml" => Self::bump_pyproject_toml(content, new_version),
            other => Err(AppError::Validation(format!(
                "Unsupported manifest type: {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bump_cargo_toml_preserves_comments_and_spacing() {
        let original = r#"# Main configuration
[package]
name = "release-weave"
# Version comment
version = "0.1.0"
edition = "2024"

[dependencies]
serde = "1.0"
"#;
        let new_ver = Version::parse("0.2.0").unwrap();
        let bumped = ManifestWriter::bump_cargo_toml(original, &new_ver).unwrap();

        assert!(bumped.contains("version = \"0.2.0\""));
        assert!(bumped.contains("# Main configuration"));
        assert!(bumped.contains("# Version comment"));
        assert!(bumped.contains("name = \"release-weave\""));
    }

    #[test]
    fn bump_cargo_toml_rejects_workspace_inherited_version() {
        let original = r#"[package]
name = "sub-crate"
version.workspace = true
edition = "2024"
"#;
        let new_ver = Version::parse("0.2.0").unwrap();
        let err = ManifestWriter::bump_cargo_toml(original, &new_ver).unwrap_err();
        assert!(err.to_string().contains("inherited from workspace"));
    }

    #[test]
    fn bump_package_json_preserves_indentation_and_formatting() {
        let original_2sp =
            "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"private\": true\n}\n";
        let new_ver = Version::parse("1.1.0").unwrap();
        let bumped_2sp = ManifestWriter::bump_package_json(original_2sp, &new_ver).unwrap();

        assert_eq!(
            bumped_2sp,
            "{\n  \"name\": \"app\",\n  \"version\": \"1.1.0\",\n  \"private\": true\n}\n"
        );

        let original_4sp =
            "{\n    \"name\": \"app\",\n    \"version\": \"2.0.0\",\n    \"scripts\": {}\n}\n";
        let bumped_4sp = ManifestWriter::bump_package_json(original_4sp, &new_ver).unwrap();
        assert_eq!(
            bumped_4sp,
            "{\n    \"name\": \"app\",\n    \"version\": \"1.1.0\",\n    \"scripts\": {}\n}\n"
        );
    }

    #[test]
    fn bump_pyproject_toml_updates_pep621_and_poetry() {
        let pep621 = r#"[project]
name = "my-pkg"
version = "0.1.0"
dependencies = []
"#;
        let new_ver = Version::parse("0.2.0").unwrap();
        let bumped_pep = ManifestWriter::bump_pyproject_toml(pep621, &new_ver).unwrap();
        assert!(bumped_pep.contains("version = \"0.2.0\""));

        let poetry = r#"[tool.poetry]
name = "poetry-pkg"
version = "1.0.0"
description = "A package"
"#;
        let bumped_poetry = ManifestWriter::bump_pyproject_toml(poetry, &new_ver).unwrap();
        assert!(bumped_poetry.contains("version = \"0.2.0\""));
    }
}
