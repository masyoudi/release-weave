use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use git2::Repository;
use semver::Version;

use crate::domain::check::{CheckResult, CheckStatus};
use crate::domain::project::ProjectSettings;
use crate::security::command_policy::CommandSpec;
use crate::security::exec_resolve::{
    contains_batch_metachars, is_windows_batch, resolve_program_env,
};
use crate::security::redact::redact;
use crate::services::git_service::GitService;
use crate::services::version_service::VersionService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadinessSummary {
    pub passed_count: usize,
    pub warning_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub not_run_count: usize,
    pub is_blocking: bool,
    pub checks: Vec<CheckResult>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedStack {
    pub language: String,
    pub manifest_file: Option<String>,
    pub suggested_commands: Vec<String>,
}

pub struct ReadinessService;

impl ReadinessService {
    pub fn check_working_tree(repo: &Repository, required: bool) -> CheckResult {
        let result = CheckResult::new("working_tree_clean", required);
        match GitService::summary(repo) {
            Ok(summary) => {
                if summary.is_dirty {
                    result.fail(format!(
                        "Working tree has {} uncommitted file(s): {}",
                        summary.dirty_files.len(),
                        summary.dirty_files.join(", ")
                    ))
                } else {
                    result.pass("Working tree is clean with no uncommitted changes")
                }
            }
            Err(e) => result.fail(format!("Failed to inspect working tree: {e}")),
        }
    }

    pub fn check_changelog(
        repo_root: &Path,
        target_version: &Version,
        required: bool,
    ) -> CheckResult {
        let result = CheckResult::new("changelog_updated", required);
        let candidates = ["CHANGELOG.md", "changelog.md", "CHANGELOG"];

        let mut changelog_path = None;
        for name in candidates {
            let p = repo_root.join(name);
            if p.exists() {
                changelog_path = Some(p);
                break;
            }
        }

        let Some(path) = changelog_path else {
            return result.fail("CHANGELOG.md file is missing in repository root");
        };

        match fs::read_to_string(&path) {
            Ok(content) => {
                let ver_str = target_version.to_string();
                let has_version = content.contains(&ver_str)
                    || content.contains(&format!("v{ver_str}"))
                    || content.contains(&format!("[{ver_str}]"));
                let has_unreleased = content.to_lowercase().contains("unreleased");

                if has_version {
                    result.pass(format!(
                        "CHANGELOG.md contains an entry for target version {target_version}"
                    ))
                } else if has_unreleased {
                    result.pass(
                        "CHANGELOG.md contains an active [Unreleased] section ready for release",
                    )
                } else {
                    result.fail(format!(
                        "CHANGELOG.md does not contain version {target_version} or an [Unreleased] section"
                    ))
                }
            }
            Err(e) => result.fail(format!("Failed to read CHANGELOG.md: {e}")),
        }
    }

    pub fn check_version_consistency(
        repo: &Repository,
        repo_root: &Path,
        required: bool,
    ) -> CheckResult {
        let result = CheckResult::new("version_consistency", required);
        let tag_version = VersionService::read_current_version(repo, repo_root).ok();

        let manifests = match VersionService::read_manifest_version(repo_root) {
            Ok(Some(m)) => vec![m],
            Ok(None) => Vec::new(),
            Err(e) => return result.fail(format!("Failed to inspect package manifests: {e}")),
        };

        let report = VersionService::check_consistency(tag_version.as_ref(), &manifests);
        if report.is_consistent {
            result.pass(report.message)
        } else {
            result.fail(report.message)
        }
    }

    pub fn execute_command_check(spec: &CommandSpec, required: bool) -> CheckResult {
        let check_name = format!("{}_{}", spec.program, spec.args.join("_"));
        let result = CheckResult::new(check_name, required);

        if let Err(e) = spec.validate() {
            let msg = format!("Command rejected by security policy: {e}");
            return if required {
                result.fail(msg)
            } else {
                result.warn(msg)
            };
        }

        let resolved = resolve_program_env(&spec.program, Some(&spec.cwd));
        let (effective_program, is_batch) = if let Some(ref path) = resolved {
            (path.to_string_lossy().to_string(), is_windows_batch(path))
        } else {
            let p = Path::new(&spec.program);
            let in_cwd = spec.cwd.join(p);
            let bat_in_cwd = spec.cwd.join(format!("{}.bat", spec.program));
            let cmd_in_cwd = spec.cwd.join(format!("{}.cmd", spec.program));
            let is_batch = is_windows_batch(p)
                || is_windows_batch(&in_cwd)
                || bat_in_cwd.is_file()
                || cmd_in_cwd.is_file();
            (spec.program.clone(), is_batch)
        };

        if is_batch {
            for arg in &spec.args {
                if contains_batch_metachars(arg) {
                    let msg = format!(
                        "Batch command execution rejected: argument '{arg}' contains shell metacharacters forbidden by security policy"
                    );
                    return if required {
                        result.fail(msg)
                    } else {
                        result.warn(msg)
                    };
                }
            }
        }

        let args = spec.args.clone();
        let cwd = spec.cwd.clone();
        let timeout = spec.timeout;

        let mut cmd = Command::new(&effective_program);
        cmd.args(&args)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => {
                let msg = format!("Failed to execute command: {e}");
                return if required {
                    result.fail(msg)
                } else {
                    result.warn(msg)
                };
            }
        };

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let child_arc = Arc::new(Mutex::new(child));
        let child_clone = Arc::clone(&child_arc);

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let start = Instant::now();
            let stdout_handle = thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut out) = stdout {
                    let _ = out.read_to_end(&mut buf);
                }
                buf
            });

            let stderr_handle = thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut err) = stderr {
                    let _ = err.read_to_end(&mut buf);
                }
                buf
            });

            let status = loop {
                thread::sleep(Duration::from_millis(20));
                let mut lock = child_clone.lock().unwrap();
                match lock.try_wait() {
                    Ok(Some(status)) => break Ok(status),
                    Ok(None) => {}
                    Err(e) => break Err(e),
                }
            };

            let out_buf = stdout_handle.join().unwrap_or_default();
            let err_buf = stderr_handle.join().unwrap_or_default();

            let res = status.map(|s| (s, out_buf, err_buf));
            let _ = tx.send((res, start.elapsed()));
        });

        match rx.recv_timeout(timeout) {
            Ok((output_res, elapsed)) => match output_res {
                Ok((status, out_buf, err_buf)) => {
                    let stdout = String::from_utf8_lossy(&out_buf);
                    let stderr = String::from_utf8_lossy(&err_buf);
                    let combined = format!("{stdout}\n{stderr}").trim().to_string();
                    let redacted = redact(&combined);

                    if status.success() {
                        result.pass(format!(
                            "Command succeeded in {:.2}s. Output:\n{redacted}",
                            elapsed.as_secs_f32()
                        ))
                    } else {
                        let msg = format!(
                            "Command exited with status {:?}:\n{redacted}",
                            status.code()
                        );
                        if required {
                            result.fail(msg)
                        } else {
                            result.warn(msg)
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Failed to execute command: {e}");
                    if required {
                        result.fail(msg)
                    } else {
                        result.warn(msg)
                    }
                }
            },
            Err(_) => {
                if let Ok(mut lock) = child_arc.lock() {
                    let _ = lock.kill();
                    let _ = lock.wait();
                }

                let msg = format!("Command execution timed out after {}s", timeout.as_secs());
                if required {
                    result.fail(msg)
                } else {
                    result.warn(msg)
                }
            }
        }
    }

    pub fn detect_project_stack(repo_root: &Path) -> DetectedStack {
        // 1. Rust
        let cargo_toml = repo_root.join("Cargo.toml");
        if cargo_toml.exists() {
            return DetectedStack {
                language: "Rust".to_string(),
                manifest_file: Some("Cargo.toml".to_string()),
                suggested_commands: vec![
                    "cargo test".to_string(),
                    "cargo clippy".to_string(),
                    "cargo build --release".to_string(),
                ],
            };
        }

        // 2. Node.js / TypeScript
        let package_json = repo_root.join("package.json");
        if package_json.exists() {
            let runner = if repo_root.join("pnpm-lock.yaml").exists() {
                "pnpm"
            } else if repo_root.join("yarn.lock").exists() {
                "yarn"
            } else if repo_root.join("bun.lockb").exists() || repo_root.join("bun.lock").exists() {
                "bun"
            } else {
                "npm"
            };

            let mut cmds = Vec::new();
            if let Ok(content) = fs::read_to_string(&package_json)
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(scripts) = json.get("scripts").and_then(|s| s.as_object())
            {
                if scripts.contains_key("test") {
                    cmds.push(if runner == "npm" {
                        "npm test".to_string()
                    } else {
                        format!("{runner} test")
                    });
                }
                if scripts.contains_key("lint") {
                    cmds.push(if runner == "npm" {
                        "npm run lint".to_string()
                    } else {
                        format!("{runner} run lint")
                    });
                }
                if scripts.contains_key("build") {
                    cmds.push(if runner == "npm" {
                        "npm run build".to_string()
                    } else {
                        format!("{runner} run build")
                    });
                }
            }

            if cmds.is_empty() {
                cmds.push(format!("{runner} test"));
            }

            return DetectedStack {
                language: "Node.js / TypeScript".to_string(),
                manifest_file: Some("package.json".to_string()),
                suggested_commands: cmds,
            };
        }

        // 3. Go
        let go_mod = repo_root.join("go.mod");
        if go_mod.exists() {
            return DetectedStack {
                language: "Go".to_string(),
                manifest_file: Some("go.mod".to_string()),
                suggested_commands: vec![
                    "go test ./...".to_string(),
                    "go vet ./...".to_string(),
                    "go build ./...".to_string(),
                ],
            };
        }

        // 4. Python
        let pyproject = repo_root.join("pyproject.toml");
        let pytest_ini = repo_root.join("pytest.ini");
        let setup_py = repo_root.join("setup.py");
        let requirements = repo_root.join("requirements.txt");

        if pyproject.exists() || pytest_ini.exists() || setup_py.exists() || requirements.exists() {
            let has_pytest = pytest_ini.exists()
                || (pyproject.exists()
                    && fs::read_to_string(&pyproject)
                        .map(|c| c.contains("pytest"))
                        .unwrap_or(false));
            let manifest = if pyproject.exists() {
                Some("pyproject.toml".to_string())
            } else if pytest_ini.exists() {
                Some("pytest.ini".to_string())
            } else if setup_py.exists() {
                Some("setup.py".to_string())
            } else {
                Some("requirements.txt".to_string())
            };

            return DetectedStack {
                language: "Python".to_string(),
                manifest_file: manifest,
                suggested_commands: if has_pytest {
                    vec!["pytest".to_string()]
                } else {
                    vec!["python3 -m unittest".to_string()]
                },
            };
        }

        // 5. PHP
        let composer_json = repo_root.join("composer.json");
        if composer_json.exists() {
            let has_phpunit = repo_root.join("vendor/bin/phpunit").exists();
            return DetectedStack {
                language: "PHP".to_string(),
                manifest_file: Some("composer.json".to_string()),
                suggested_commands: if has_phpunit {
                    vec!["./vendor/bin/phpunit".to_string()]
                } else {
                    vec!["composer test".to_string()]
                },
            };
        }

        // 6. Java / Maven
        let pom_xml = repo_root.join("pom.xml");
        if pom_xml.exists() {
            return DetectedStack {
                language: "Java / Maven".to_string(),
                manifest_file: Some("pom.xml".to_string()),
                suggested_commands: vec!["mvn test".to_string()],
            };
        }

        // 7. Java / Kotlin Gradle
        let build_gradle = repo_root.join("build.gradle");
        let build_gradle_kts = repo_root.join("build.gradle.kts");
        if build_gradle.exists() || build_gradle_kts.exists() {
            let manifest = if build_gradle.exists() {
                "build.gradle"
            } else {
                "build.gradle.kts"
            };
            let cmd = if repo_root.join("gradlew").exists() {
                "./gradlew test".to_string()
            } else {
                "gradle test".to_string()
            };
            return DetectedStack {
                language: "Java / Kotlin (Gradle)".to_string(),
                manifest_file: Some(manifest.to_string()),
                suggested_commands: vec![cmd],
            };
        }

        // 8. .NET / C#
        if let Ok(entries) = fs::read_dir(repo_root) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension().and_then(|e| e.to_str())
                    && (ext == "sln" || ext == "csproj")
                {
                    return DetectedStack {
                        language: ".NET / C#".to_string(),
                        manifest_file: Some(entry.file_name().to_string_lossy().to_string()),
                        suggested_commands: vec![
                            "dotnet test".to_string(),
                            "dotnet build".to_string(),
                        ],
                    };
                }
            }
        }

        // 9. Generic Git repo
        DetectedStack {
            language: "Generic Repository".to_string(),
            manifest_file: None,
            suggested_commands: Vec::new(),
        }
    }

    pub fn aggregate(checks: Vec<CheckResult>) -> ReadinessSummary {
        let mut passed_count = 0;
        let mut warning_count = 0;
        let mut failed_count = 0;
        let mut skipped_count = 0;
        let mut not_run_count = 0;
        let mut is_blocking = false;

        for check in &checks {
            match check.status {
                CheckStatus::Passed => passed_count += 1,
                CheckStatus::Warning => warning_count += 1,
                CheckStatus::Failed => failed_count += 1,
                CheckStatus::Skipped => skipped_count += 1,
                CheckStatus::NotRun => not_run_count += 1,
            }

            if check.status.is_blocking(check.required) {
                is_blocking = true;
            }
        }

        ReadinessSummary {
            passed_count,
            warning_count,
            failed_count,
            skipped_count,
            not_run_count,
            is_blocking,
            checks,
        }
    }

    pub fn parse_command_string(
        raw: &str,
        cwd: &Path,
        default_timeout: Duration,
    ) -> Option<CommandSpec> {
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let program = parts[0].to_string();
        let args = parts[1..].iter().map(|s| s.to_string()).collect();

        Some(CommandSpec::new(program, args, cwd, default_timeout))
    }

    pub fn run_all(
        repo: &Repository,
        repo_root: &Path,
        target_version: &Version,
        settings: &ProjectSettings,
    ) -> ReadinessSummary {
        let mut checks = Vec::new();

        checks.push(Self::check_working_tree(repo, true));
        // Changelog check is non-blocking (warning only) if missing or target version unlisted
        checks.push(Self::check_changelog(repo_root, target_version, false));
        checks.push(Self::check_version_consistency(repo, repo_root, true));

        let default_timeout = Duration::from_secs(60);
        for cmd_str in &settings.readiness_commands {
            if let Some(spec) = Self::parse_command_string(cmd_str, repo_root, default_timeout) {
                checks.push(Self::execute_command_check(
                    &spec,
                    settings.commands_required,
                ));
            }
        }

        Self::aggregate(checks)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use tempfile::tempdir;

    use super::*;

    fn init_git_repo(path: &Path) -> Repository {
        let repo = Repository::init(path).unwrap();
        let sig = git2::Signature::now("Bot", "bot@example.com").unwrap();
        let file_path = path.join("file.txt");
        fs::write(&file_path, "content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
            .unwrap();
        drop(tree);
        repo
    }

    #[test]
    fn check_working_tree_clean_and_dirty() {
        let dir = tempdir().unwrap();
        let repo = init_git_repo(dir.path());

        let clean_check = ReadinessService::check_working_tree(&repo, true);
        assert_eq!(clean_check.status, CheckStatus::Passed);

        fs::write(dir.path().join("dirty.txt"), "dirty").unwrap();
        let dirty_check = ReadinessService::check_working_tree(&repo, true);
        assert_eq!(dirty_check.status, CheckStatus::Failed);
        assert!(dirty_check.details.unwrap().contains("dirty.txt"));
    }

    #[test]
    fn check_changelog_scenarios() {
        let dir = tempdir().unwrap();
        let v = Version::parse("1.2.0").unwrap();

        // 1. Missing changelog
        let check_missing = ReadinessService::check_changelog(dir.path(), &v, true);
        assert_eq!(check_missing.status, CheckStatus::Failed);

        // 2. Changelog with [Unreleased]
        let p = dir.path().join("CHANGELOG.md");
        fs::write(&p, "# Changelog\n## [Unreleased]\n- Some changes\n").unwrap();
        let check_unreleased = ReadinessService::check_changelog(dir.path(), &v, true);
        assert_eq!(check_unreleased.status, CheckStatus::Passed);

        // 3. Changelog with target version
        fs::write(&p, "# Changelog\n## [1.2.0] - 2026-10-20\n- Final notes\n").unwrap();
        let check_version = ReadinessService::check_changelog(dir.path(), &v, true);
        assert_eq!(check_version.status, CheckStatus::Passed);

        // 4. Changelog without target version or unreleased
        fs::write(&p, "# Changelog\n## [1.0.0] - 2026-01-01\n- Old notes\n").unwrap();
        let check_old = ReadinessService::check_changelog(dir.path(), &v, true);
        assert_eq!(check_old.status, CheckStatus::Failed);
    }

    #[test]
    fn execute_command_check_success_and_failure() {
        let dir = tempdir().unwrap();
        let spec_ok = CommandSpec::new(
            "git",
            vec!["--version".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );
        let check_ok = ReadinessService::execute_command_check(&spec_ok, true);
        assert_eq!(check_ok.status, CheckStatus::Passed);

        let spec_fail = CommandSpec::new(
            "git",
            vec!["bukan-perintah-apapun-valid".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );
        let check_fail = ReadinessService::execute_command_check(&spec_fail, true);
        assert_eq!(check_fail.status, CheckStatus::Failed);
    }

    #[test]
    fn execute_command_check_timeout() {
        let dir = tempdir().unwrap();
        let spec_timeout = CommandSpec::new(
            "sleep",
            vec!["2".to_string()],
            dir.path(),
            Duration::from_millis(50),
        );
        let check_timeout = ReadinessService::execute_command_check(&spec_timeout, true);
        assert_eq!(check_timeout.status, CheckStatus::Failed);
        assert!(check_timeout.details.unwrap().contains("timed out"));
    }

    #[test]
    fn aggregate_blocks_when_required_is_not_passed() {
        let checks = vec![
            CheckResult::new("test1", true).pass("ok"),
            CheckResult::new("test2", true), // Default is NotRun, which is blocking for required
        ];

        let summary = ReadinessService::aggregate(checks);
        assert!(summary.is_blocking);
        assert_eq!(summary.not_run_count, 1);
        assert_eq!(summary.passed_count, 1);
    }

    #[test]
    fn aggregate_passes_when_all_required_pass() {
        let checks = vec![
            CheckResult::new("test1", true).pass("ok"),
            CheckResult::new("optional", false).warn("not critical"),
        ];

        let summary = ReadinessService::aggregate(checks);
        assert!(!summary.is_blocking);
        assert_eq!(summary.passed_count, 1);
        assert_eq!(summary.warning_count, 1);
    }

    #[test]
    fn detect_project_stack_rust_node_go_python_and_generic() {
        // Rust
        let dir_rust = tempdir().unwrap();
        fs::write(
            dir_rust.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .unwrap();
        let stack_rust = ReadinessService::detect_project_stack(dir_rust.path());
        assert_eq!(stack_rust.language, "Rust");
        assert_eq!(stack_rust.manifest_file.as_deref(), Some("Cargo.toml"));
        assert!(
            stack_rust
                .suggested_commands
                .contains(&"cargo test".to_string())
        );

        // Node with pnpm & package.json scripts
        let dir_node = tempdir().unwrap();
        fs::write(
            dir_node.path().join("package.json"),
            r#"{"scripts": {"test": "vitest", "lint": "eslint"}}"#,
        )
        .unwrap();
        fs::write(dir_node.path().join("pnpm-lock.yaml"), "lockfile").unwrap();
        let stack_node = ReadinessService::detect_project_stack(dir_node.path());
        assert_eq!(stack_node.language, "Node.js / TypeScript");
        assert_eq!(stack_node.manifest_file.as_deref(), Some("package.json"));
        assert_eq!(
            stack_node.suggested_commands,
            vec!["pnpm test".to_string(), "pnpm run lint".to_string()]
        );

        // Go
        let dir_go = tempdir().unwrap();
        fs::write(dir_go.path().join("go.mod"), "module demo\n").unwrap();
        let stack_go = ReadinessService::detect_project_stack(dir_go.path());
        assert_eq!(stack_go.language, "Go");
        assert!(
            stack_go
                .suggested_commands
                .contains(&"go test ./...".to_string())
        );

        // Python with pytest
        let dir_py = tempdir().unwrap();
        fs::write(dir_py.path().join("pytest.ini"), "[pytest]\n").unwrap();
        let stack_py = ReadinessService::detect_project_stack(dir_py.path());
        assert_eq!(stack_py.language, "Python");
        assert_eq!(stack_py.suggested_commands, vec!["pytest".to_string()]);

        // Generic
        let dir_gen = tempdir().unwrap();
        let stack_gen = ReadinessService::detect_project_stack(dir_gen.path());
        assert_eq!(stack_gen.language, "Generic Repository");
        assert!(stack_gen.suggested_commands.is_empty());
    }

    #[test]
    fn optional_command_check_warns_instead_of_blocking() {
        let dir = tempdir().unwrap();
        let spec_fail = CommandSpec::new(
            "git",
            vec!["nonexistent-arg-command".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );
        let check_optional = ReadinessService::execute_command_check(&spec_fail, false);
        assert_eq!(check_optional.status, CheckStatus::Warning);
        assert!(!check_optional.required);

        let summary = ReadinessService::aggregate(vec![check_optional]);
        assert!(!summary.is_blocking);
        assert_eq!(summary.warning_count, 1);
        assert_eq!(summary.failed_count, 0);
    }

    #[test]
    fn rejects_shell_programs_via_execute_command_check() {
        let dir = tempdir().unwrap();

        for shell in [
            "cmd",
            "cmd.exe",
            "powershell",
            "powershell.exe",
            "sh",
            "bash",
        ] {
            let spec = CommandSpec::new(shell, vec![], dir.path(), Duration::from_secs(5));
            let check = ReadinessService::execute_command_check(&spec, true);
            assert_eq!(check.status, CheckStatus::Failed);
            let details = check.details.as_deref().unwrap_or_default();
            assert!(
                details.contains("Direct invocation of shell interpreter"),
                "Expected shell rejection message for {shell}, got: {details}"
            );
        }

        let spec_inline_c = CommandSpec::new(
            "cargo",
            vec!["-c".to_string(), "malicious".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );
        let check_c = ReadinessService::execute_command_check(&spec_inline_c, true);
        assert_eq!(check_c.status, CheckStatus::Failed);
        let details_c = check_c.details.as_deref().unwrap_or_default();
        assert!(details_c.contains("Inline shell execution argument"));
    }

    #[test]
    fn rejects_batch_metacharacters() {
        let dir = tempdir().unwrap();
        let batch_file = dir.path().join("test_script.bat");
        fs::write(&batch_file, "@echo off\r\n").unwrap();

        let malicious_spec = CommandSpec::new(
            batch_file.to_str().unwrap(),
            vec!["arg1".to_string(), "foo & calc.exe".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );

        let check = ReadinessService::execute_command_check(&malicious_spec, true);
        assert_eq!(check.status, CheckStatus::Failed);
        let details = check.details.as_deref().unwrap_or_default();
        assert!(
            details.contains("contains shell metacharacters"),
            "Expected batch metacharacter rejection, got: {details}"
        );
    }

    #[test]
    fn rejects_batch_metacharacters_when_script_is_in_cwd_without_extension() {
        let dir = tempdir().unwrap();
        let batch_file = dir.path().join("local_task.bat");
        fs::write(&batch_file, "@echo off\r\n").unwrap();

        let malicious_spec = CommandSpec::new(
            "local_task",
            vec!["arg1".to_string(), "foo | calc.exe".to_string()],
            dir.path(),
            Duration::from_secs(5),
        );

        let check = ReadinessService::execute_command_check(&malicious_spec, true);
        assert_eq!(check.status, CheckStatus::Failed);
        let details = check.details.as_deref().unwrap_or_default();
        assert!(
            details.contains("contains shell metacharacters"),
            "Expected batch metacharacter rejection for CWD batch file, got: {details}"
        );
    }
}
