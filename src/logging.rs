use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

use crate::errors::{AppError, Result};

const DEFAULT_FILTER: &str = "warn,release_weave=info";
const LOG_FILE: &str = "release-weave.log";

pub fn init() -> Result<PathBuf> {
    init_in(&crate::paths::data_dir()?.join("logs"))
}

pub fn init_in(log_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(log_dir)?;
    let path = log_dir.join(LOG_FILE);
    let file = OpenOptions::new().create(true).append(true).open(&path)?;

    let filter =
        EnvFilter::try_from_env("RW_LOG").unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_ansi(false).with_writer(Mutex::new(file)))
        .try_init()
        .map_err(|e| AppError::Validation(format!("Logging is already initialized: {e}")))?;

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_events_to_log_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = init_in(&dir.path().join("logs")).unwrap();

        tracing::info!("startup check");

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("startup check"));
    }
}
