# Contributing to ReleaseWeave

Thank you for your interest in contributing to ReleaseWeave! ReleaseWeave is a native desktop release engineering toolkit built with Rust, GPUI Kit, and verified local AI (Ollama).

---

## 1. Architecture & Code Principles

1. **Strict UI/Domain Decoupling:**
   - Modules `domain/`, `services/`, `ai/`, `storage/`, and `security/` **MUST NOT** import types from `gpui` or `gpui_kit`.
   - The `ui/` module functions strictly as visual presentation that invokes methods on services and application state.
2. **Zero-WebView:**
   - All user interfaces are rendered directly through GPUI Kit to the GPU (Metal, Vulkan, DirectX). Do not introduce browser engine dependencies, Chromium, Electron, or WebViews.
3. **Anti-Slop Comment Guidelines:**
   - Code comments must remain concise (maximum 1 line).
   - Explain only the rationale ("why") or non-obvious constraints. Avoid decorative comments or restating what the code already expresses.
4. **Security & Data Privacy:**
   - All filesystem paths must be validated using `validate_dir` or `validate_output_file`.
   - Commands must never invoke shell interpreters (`sh`, `bash`, `cmd`, `powershell`).
   - Secrets (API tokens, authorization keys) must be redacted before being displayed or logged.
   - Local commit analysis must never transmit source code to external cloud endpoints.

---

## 2. Development Setup

### Prerequisites
- **Rust Toolchain:** Latest stable channel (`rustc >= 1.85`).
- **Git:** Version 2.30+.
- **Ollama:** (Optional, for local AI release notes features) Start `ollama serve` and pull the default model:
  ```bash
  ollama pull qwen2.5-coder:7b
  ```

### Running the Project
```bash
# Launch desktop application
cargo run

# Run unit tests and integration tests
cargo test

# Check formatting and lint compliance
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

---

## 3. Pull Request Workflow

1. Create a descriptive feature branch from `main`: `feat/feature-name` or `fix/issue-name`.
2. Follow Conventional Commits conventions (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, `test:`).
3. Ensure all tests pass (`cargo test`) with zero Clippy warnings (`-D warnings`).
4. Submit a Pull Request with a clear, concise summary of your changes.
