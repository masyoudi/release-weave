# ReleaseWeave Security Model

ReleaseWeave is designed following the **Defense-in-Depth** principle to ensure that Git operations, readiness command execution, token storage, and AI interactions cannot compromise the user's workstation.

---

## 1. Path Traversal Prevention

The `validate_dir` and `validate_output_file` functions in `src/security/path_validation.rs` validate all filesystem paths before read or write operations:
- Blocks directory traversal sequences such as `../../../../etc/passwd`.
- Verifies that target directories exist and are genuine directories rather than files.
- Ensures output files reside within valid, writable directories.
- Rejects writes if the destination path points to an existing directory.

---

## 2. Command Injection Prevention (Command Policy & Resolver)

The `CommandSpec::validate` function in `src/security/command_policy.rs` and the `exec_resolve` module in `src/security/exec_resolve.rs` enforce strict controls on automated readiness commands:
- **Banning Shell Interpreters:** Execution of shell interpreters such as `sh`, `bash`, `zsh`, `cmd`, `cmd.exe`, `powershell`, and `pwsh` is rejected unconditionally.
- **Banning Inline Evaluation Flags:** Arguments such as `-c`, `/c`, `/C`, and `-Command` are rejected.
- **Strict Separation of Program and Arguments:** Command arguments must be provided as a structured `Vec<String>` spawned directly via `std::process::Command`, preventing shell argument injection and string interpolation.
- **Safe Windows Batch Handling:** On Windows, executables are resolved through `PATH` and `PATHEXT`. If a `.bat` or `.cmd` file is resolved, arguments containing `cmd.exe` metacharacters (`& | < > ^ % "`) are rejected.
- **Execution Timeouts:** All commands run with an enforced timeout (30 to 60 seconds) to terminate hung or zombie processes.

---

## 3. Secret Redaction & Token Masking

The `src/security/redact.rs` module cleans sensitive credentials from log files, export artifacts, and UI elements:
- **Bearer Tokens:** Redacts `Bearer <token>` to `Bearer [REDACTED]`.
- **GitHub Tokens:** Patterns including `ghp_***`, `gho_***`, and fine-grained `github_pat_***` are replaced with `[REDACTED]`.
- **GitLab Tokens:** Personal access tokens matching `glpat-***` are redacted.
- **Cloud & AI API Keys:** Keys matching `sk-proj-***` or `sk-***` are masked.
- **Sensitive Key-Value Pairs:** Patterns such as `api_key=...`, `password=...`, `secret=...`, and `token=...` are replaced with `[REDACTED]`.

---

## 4. Operating System Keychain Storage (`TokenStore`)

The `src/security/secrets.rs` module isolates API tokens used for GitHub and GitLab release publishing:
- Credentials are stored in native operating system keychains:
  - macOS Keychain
  - Linux Secret Service (via Freedesktop DBus secret service)
  - Windows Credential Manager
- Tokens are **never** stored in the local SQLite database, configuration files, or plaintext on disk.
- Tokens are retrieved on-demand only when a release is explicitly published by the user.

---

## 5. Prompt Security & AI Data Isolation

The `src/ai/prompts.rs` module defends local AI models against prompt injection attempts embedded within Git commit messages:
- **Data-Oriented System Instructions:** Explicitly instructs the model that all content within the commit envelope represents untrusted data rather than executable instructions.
- **Delimiter Escaping:** All occurrences of `<<<` and `>>>` within commit titles, authors, or bodies are escaped to `&lt;&lt;&lt;` and `&gt;&gt;&gt;`.
- **Hash Provenance Verification:** Models must provide a valid source commit hash for every generated change item. Bullets with fabricated or missing hashes have their confidence score reduced to `0.3` and are marked `unsourced`.

---

## 6. Git Operation Safety & Opt-In Tagging

- **Zero Automatic Remote Pushes:** ReleaseWeave never runs `git push` to any remote repository.
- **Opt-In Tag Creation:** Annotated Git tag generation is disabled by default (`allow_git_tag = false`). Tags can only be created if explicitly enabled per project in Settings and the release has been finalized.
- **Pre-Tag Verification:** Tagging is rejected if the working tree has uncommitted modifications, the tag already exists, or HEAD is in a detached state.
- **Verified File Modification:** File patches (`Cargo.toml`, `package.json`, `pyproject.toml`, `CHANGELOG.md`) require visual diff confirmation and SHA-256 hash pre-validation prior to atomic disk writes.
