use std::path::{Path, PathBuf};

use crate::errors::{AppError, Result};

pub fn validate_dir(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "Directory does not exist: {}",
            path.display()
        )));
    }

    let canonical = path.canonicalize()?;
    if !canonical.is_dir() {
        return Err(AppError::Validation(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    Ok(canonical)
}

pub fn validate_output_file(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let parent = path.parent().ok_or_else(|| {
        AppError::Validation(format!(
            "Output file has no parent directory: {}",
            path.display()
        ))
    })?;

    let canonical_parent = validate_dir(parent)?;
    let filename = path.file_name().ok_or_else(|| {
        AppError::Validation(format!("Output path missing file name: {}", path.display()))
    })?;

    let destination = canonical_parent.join(filename);

    if destination.exists() {
        if destination.is_dir() {
            return Err(AppError::Validation(format!(
                "Output file target is a directory: {}",
                destination.display()
            )));
        }
        let canonical_dest = destination.canonicalize()?;
        if !canonical_dest.starts_with(&canonical_parent) {
            return Err(AppError::Validation(
                "Output path escapes parent directory".into(),
            ));
        }
    }

    Ok(destination)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn validates_existing_directory() {
        let dir = tempdir().unwrap();
        let validated = validate_dir(dir.path()).unwrap();
        assert_eq!(validated, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn rejects_missing_directory() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does_not_exist");
        let result = validate_dir(&missing);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn rejects_file_as_directory() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").unwrap();
        let result = validate_dir(&file_path);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }

    #[test]
    fn validates_valid_output_file() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("release-notes.md");
        let validated = validate_output_file(&out).unwrap();
        assert_eq!(
            validated,
            dir.path().canonicalize().unwrap().join("release-notes.md")
        );
    }

    #[test]
    fn rejects_output_file_in_missing_parent() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("missing_subdir").join("notes.md");
        let result = validate_output_file(&out);
        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[test]
    fn rejects_output_file_pointing_to_existing_dir() {
        let dir = tempdir().unwrap();
        let subdir = dir.path().join("sub");
        fs::create_dir(&subdir).unwrap();
        let result = validate_output_file(&subdir);
        assert!(matches!(result, Err(AppError::Validation(_))));
    }
}
