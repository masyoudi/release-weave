use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Git error: {0}")]
    Git(#[from] git2::Error),

    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("AI provider error: {0}")]
    Ai(String),

    #[error("{0}")]
    Validation(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_io_error() {
        let err: AppError = std::io::Error::new(std::io::ErrorKind::NotFound, "missing").into();
        assert!(matches!(err, AppError::Io(_)));
        assert_eq!(err.to_string(), "File system error: missing");
    }

    #[test]
    fn converts_git_error() {
        let err: AppError = git2::Error::from_str("bad ref").into();
        assert!(matches!(err, AppError::Git(_)));
        assert_eq!(err.to_string(), "Git error: bad ref");
    }

    #[test]
    fn converts_db_error() {
        let err: AppError = rusqlite::Error::QueryReturnedNoRows.into();
        assert!(matches!(err, AppError::Db(_)));
        assert!(err.to_string().starts_with("Database error:"));
    }

    #[test]
    fn validation_message_is_shown_as_is() {
        let err = AppError::Validation("Selected folder is not a Git repository".into());
        assert_eq!(err.to_string(), "Selected folder is not a Git repository");
    }
}
