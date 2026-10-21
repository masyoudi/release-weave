use std::path::{Path, PathBuf};

const DEFAULT_WINDOWS_PATHEXT: &[&str] = &[".COM", ".EXE", ".BAT", ".CMD"];
const BATCH_METACHARS: &[char] = &['&', '|', '<', '>', '^', '%', '"', '\r', '\n'];

pub fn resolve_program(
    program: &str,
    cwd: Option<&Path>,
    path_var: Option<&str>,
    pathext_var: Option<&str>,
    is_windows: bool,
) -> Option<PathBuf> {
    let trimmed = program.trim();
    if trimmed.is_empty() {
        return None;
    }

    let program_path = Path::new(trimmed);
    let has_separator = trimmed.contains('/') || (is_windows && trimmed.contains('\\'));

    if has_separator {
        let mut base_candidates = Vec::new();
        if let Some(dir) = cwd {
            base_candidates.push(dir.join(program_path));
        }
        base_candidates.push(program_path.to_path_buf());

        if is_windows {
            let pathexts = parse_pathext(pathext_var);
            for base in &base_candidates {
                if base.extension().is_some() && base.is_file() {
                    return Some(base.clone());
                }

                for ext in &pathexts {
                    let clean_ext = ext.trim_start_matches('.');
                    let candidates = [
                        base.with_extension(clean_ext),
                        base.with_extension(clean_ext.to_lowercase()),
                        base.with_extension(clean_ext.to_uppercase()),
                    ];
                    for candidate in candidates {
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
                if base.is_file() {
                    return Some(base.clone());
                }
            }
        } else {
            for base in &base_candidates {
                if base.is_file() {
                    return Some(base.clone());
                }
            }
        }
        return None;
    }

    // On Windows, the current working directory is searched before PATH.
    if is_windows && let Some(dir) = cwd {
        let has_ext = program_path.extension().is_some();
        let pathexts = parse_pathext(pathext_var);

        if has_ext {
            let candidate = dir.join(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
        } else {
            for ext in &pathexts {
                let formatted_ext = if ext.starts_with('.') {
                    ext.to_string()
                } else {
                    format!(".{ext}")
                };
                let candidates = [
                    dir.join(format!("{trimmed}{formatted_ext}")),
                    dir.join(format!("{trimmed}{}", formatted_ext.to_lowercase())),
                    dir.join(format!("{trimmed}{}", formatted_ext.to_uppercase())),
                ];
                for candidate in candidates {
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
            let candidate = dir.join(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    let raw_path = path_var?;
    let dirs = split_path_entries(raw_path, is_windows);

    if is_windows {
        let has_ext = program_path.extension().is_some();
        let pathexts = parse_pathext(pathext_var);

        for dir in dirs {
            if has_ext {
                let candidate = dir.join(trimmed);
                if candidate.is_file() {
                    return Some(candidate);
                }
            } else {
                for ext in &pathexts {
                    let formatted_ext = if ext.starts_with('.') {
                        ext.to_string()
                    } else {
                        format!(".{ext}")
                    };
                    let candidates = [
                        dir.join(format!("{trimmed}{formatted_ext}")),
                        dir.join(format!("{trimmed}{}", formatted_ext.to_lowercase())),
                        dir.join(format!("{trimmed}{}", formatted_ext.to_uppercase())),
                    ];
                    for candidate in candidates {
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
                let candidate = dir.join(trimmed);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    } else {
        for dir in dirs {
            let candidate = dir.join(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

pub fn resolve_program_env(program: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let path_var = std::env::var("PATH").ok();
    let pathext_var = std::env::var("PATHEXT").ok();
    let is_windows = cfg!(target_os = "windows");
    resolve_program(
        program,
        cwd,
        path_var.as_deref(),
        pathext_var.as_deref(),
        is_windows,
    )
}

pub fn is_windows_batch(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd")
    } else {
        false
    }
}

pub fn contains_batch_metachars(arg: &str) -> bool {
    arg.contains(|c| BATCH_METACHARS.contains(&c))
}

fn split_path_entries(raw_path: &str, is_windows: bool) -> Vec<PathBuf> {
    let sep = if is_windows { ';' } else { ':' };
    raw_path
        .split(sep)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn parse_pathext(pathext_var: Option<&str>) -> Vec<String> {
    match pathext_var {
        Some(raw) if !raw.trim().is_empty() => raw
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s.starts_with('.') {
                    s.to_string()
                } else {
                    format!(".{s}")
                }
            })
            .collect(),
        _ => DEFAULT_WINDOWS_PATHEXT
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::tempdir;

    #[test]
    fn resolves_windows_batch_extensions_in_path() {
        let dir = tempdir().unwrap();
        let npm_cmd = dir.path().join("npm.cmd");
        File::create(&npm_cmd).unwrap();

        let path_str = dir.path().to_str().unwrap();

        let resolved_win =
            resolve_program("npm", None, Some(path_str), Some(".COM;.EXE;.CMD"), true);
        assert_eq!(resolved_win, Some(npm_cmd));

        let resolved_nix = resolve_program("npm", None, Some(path_str), None, false);
        assert_eq!(resolved_nix, None);
    }

    #[test]
    fn resolves_windows_batch_extensions_in_cwd() {
        let dir = tempdir().unwrap();
        let test_bat = dir.path().join("test.bat");
        File::create(&test_bat).unwrap();

        let resolved_win = resolve_program(
            "test",
            Some(dir.path()),
            None,
            Some(".COM;.EXE;.BAT;.CMD"),
            true,
        );
        assert_eq!(resolved_win, Some(test_bat));
    }

    #[test]
    fn resolves_unix_executable_directly() {
        let dir = tempdir().unwrap();
        let cargo_bin = dir.path().join("cargo");
        File::create(&cargo_bin).unwrap();

        let path_str = dir.path().to_str().unwrap();
        let resolved = resolve_program("cargo", None, Some(path_str), None, false);
        assert_eq!(resolved, Some(cargo_bin));
    }

    #[test]
    fn detects_batch_metacharacters() {
        assert!(contains_batch_metachars("foo & bar"));
        assert!(contains_batch_metachars("foo | bar"));
        assert!(contains_batch_metachars("foo < bar"));
        assert!(contains_batch_metachars("foo > bar"));
        assert!(contains_batch_metachars("foo ^ bar"));
        assert!(contains_batch_metachars("foo %bar%"));
        assert!(contains_batch_metachars("foo \"bar\""));
        assert!(contains_batch_metachars("foo\rbar"));
        assert!(contains_batch_metachars("foo\nbar"));
        assert!(!contains_batch_metachars("test --all --verbose"));
    }

    #[test]
    fn identifies_batch_files() {
        assert!(is_windows_batch(Path::new("npm.cmd")));
        assert!(is_windows_batch(Path::new("build.bat")));
        assert!(is_windows_batch(Path::new("BUILD.BAT")));
        assert!(!is_windows_batch(Path::new("cargo.exe")));
        assert!(!is_windows_batch(Path::new("cargo")));
    }
}
