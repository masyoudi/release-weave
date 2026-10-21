use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::errors::{AppError, Result};
use crate::security::path_validation::validate_dir;

const BLOCKED_PROGRAMS: &[&str] = &[
    "sh",
    "bash",
    "zsh",
    "dash",
    "ksh",
    "csh",
    "tcsh",
    "cmd",
    "cmd.exe",
    "powershell",
    "powershell.exe",
    "pwsh",
    "pwsh.exe",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub timeout: Duration,
}

impl CommandSpec {
    pub fn new(
        program: impl Into<String>,
        args: Vec<String>,
        cwd: impl Into<PathBuf>,
        timeout: Duration,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            cwd: cwd.into(),
            timeout,
        }
    }

    pub fn validate(&self) -> Result<PathBuf> {
        let trimmed_prog = self.program.trim();
        if trimmed_prog.is_empty() {
            return Err(AppError::Validation(
                "Command program cannot be empty".into(),
            ));
        }

        let prog_file_name = Path::new(trimmed_prog)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(trimmed_prog)
            .to_lowercase();

        if BLOCKED_PROGRAMS.contains(&prog_file_name.as_str()) {
            return Err(AppError::Validation(format!(
                "Direct invocation of shell interpreter '{trimmed_prog}' is forbidden by security policy"
            )));
        }

        for arg in &self.args {
            let arg_lower = arg.to_lowercase();
            if arg_lower == "-c" || arg_lower == "/c" {
                return Err(AppError::Validation(
                    "Inline shell execution argument (-c / /c) is forbidden by security policy"
                        .into(),
                ));
            }
        }

        validate_dir(&self.cwd)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn accepts_valid_safe_command() {
        let dir = tempdir().unwrap();
        let spec = CommandSpec::new(
            "cargo",
            vec!["test".to_string(), "--no-run".to_string()],
            dir.path(),
            Duration::from_secs(30),
        );

        assert!(spec.validate().is_ok());
    }

    #[test]
    fn rejects_shell_invocations() {
        let dir = tempdir().unwrap();
        let blocked = vec![
            "sh",
            "bash",
            "/bin/sh",
            "/usr/bin/bash",
            "cmd.exe",
            "powershell",
        ];

        for prog in blocked {
            let spec = CommandSpec::new(
                prog,
                vec!["-c".to_string(), "echo safe".to_string()],
                dir.path(),
                Duration::from_secs(5),
            );
            assert!(
                spec.validate().is_err(),
                "should reject shell program: {prog}"
            );
        }
    }

    #[test]
    fn rejects_inline_c_flag() {
        let dir = tempdir().unwrap();
        let spec = CommandSpec::new(
            "python3",
            vec!["-c".to_string(), "import os".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );

        assert!(spec.validate().is_err());
    }

    #[test]
    fn rejects_missing_cwd() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("missing_dir");
        let spec = CommandSpec::new(
            "git",
            vec!["status".to_string()],
            missing,
            Duration::from_secs(5),
        );

        assert!(spec.validate().is_err());
    }
}
