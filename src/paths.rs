use std::path::PathBuf;

use directories::ProjectDirs;

use crate::errors::{AppError, Result};

pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("RW_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    ProjectDirs::from("", "", "release-weave")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
        .ok_or_else(|| AppError::NotFound("home directory for application data".into()))
}
