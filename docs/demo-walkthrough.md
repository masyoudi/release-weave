# ReleaseWeave Demo & Visual Walkthrough

An interactive walkthrough and workflow guide for the native desktop application **ReleaseWeave**.

---

## 1. End-to-End User Interface

![ReleaseWeave Native Overview](screenshots/overview.png)

*ReleaseWeave native desktop interface featuring Carbon Cobalt theme, persistent sidebar navigation, active Git repository metadata, current SemVer detection, and release readiness action gates.*

---

## 2. Interface Navigation

ReleaseWeave provides global keyboard shortcuts and mouse interactions:
- `Ctrl+1` / `Cmd+1`: **Overview**: Project metadata summary, active branch, current version, and readiness banner.
- `Ctrl+2` / `Cmd+2`: **Changes**: Deterministic Git analysis, conventional commit categorization, category filters, and diff patch inspection.
- `Ctrl+3` / `Cmd+3`: **Release Notes**: Release notes studio with local AI (Ollama), commit hash provenance verification, Markdown editor, and live preview.
- `Ctrl+4` / `Cmd+4`: **Versioning**: Semantic Versioning (SemVer 2.0) recommendations, bump rationale explanations, and manual overrides.
- `Ctrl+5` / `Cmd+5`: **Readiness**: Multi-stage readiness gate execution (working tree, manifest, changelog, test/lint commands).
- `Ctrl+6` / `Cmd+6`: **History**: Release audit trail stored in local SQLite along with commit range history.
- `Ctrl+7` / `Cmd+7`: **Settings**: Ollama endpoint configuration, local AI model selection, readiness command policies, and remote publishing credentials.
- `Ctrl+K` / `Cmd+K`: **Command Palette**: Rapid fuzzy-search overlay for navigation, repository switching, package selection, and common workflows.

---

## 3. Step-by-Step Release Workflow

### Step 1: Open Local Git Repository
1. Click the **Open Repository** button in the navbar.
2. Select or enter your local Git repository path (e.g., `/home/user/projects/my-app`).
3. The system validates the `.git` directory and loads project configurations without altering any files (*Pristine Repository Guarantee*).
4. If a monorepo is detected (Cargo workspace, pnpm, or npm/yarn), the package selector appears in the top bar.

### Step 2: Git Analysis & Conventional Commits (`Ctrl+2`)
1. Select a commit range: start tag (e.g., `v1.2.0`) and target revision / HEAD.
2. If working within a monorepo, commits are filtered strictly to the selected package subdirectory.
3. Review categorized commits:
   - `Features (feat)`
   - `Fixes (fix)`
   - `Breaking Changes` (`feat!:`, `BREAKING CHANGE:`)
   - `Docs (docs)`
   - `Refactor (refactor)`
4. Click any commit to inspect syntax-highlighted diff patches with byte truncation safety limits.

### Step 3: Check SemVer Recommendations (`Ctrl+4`)
1. The **Versioning** page identifies the current version (e.g., `1.2.0`).
2. Click **Calculate Recommendation**.
3. Evaluates commits across the release window:
   - Breaking changes: recommends **Major Bump (Breaking Changes)** (`2.0.0`) (or **Minor** in `0.x.y` per SemVer specification).
   - Features present: recommends **Minor Bump (New Features)** (`1.3.0`).
   - Fixes only: recommends **Patch Bump (Bug Fixes)** (`1.2.1`).
4. Accept the recommendation or enter a custom version, with helpers for **Add -rc** and **Add -beta** tags.

### Step 4: Run Release Readiness Gates (`Ctrl+5`)
1. Click **Run All Checks**.
2. The system executes verification gates asynchronously in the background:
   - `[✓] working_tree_clean`: No uncommitted modifications.
   - `[✓] version_consistency`: Git tags align with `Cargo.toml` or `package.json`.
   - `[✓] changelog_updated`: `CHANGELOG.md` exists and is ready for release staging.
   - `[✓] command_policy`: Test and lint commands execute safely with zero secrets leaked to logs.
3. Every status displays accessible symbols and explicit text labels (`✓ Passed`, `✗ Failed`, `⚠ Warning`, `⊘ Skipped`, `- Not run`).

### Step 5: Synthesize Verified Release Notes (`Ctrl+3`)
1. Select generation method: **Generate AI (Ollama)** or **Deterministic**.
2. Click **Generate AI (Ollama)**.
3. Commits are enclosed in secure delimiters `<<<COMMITS_START>>>` and sent to Ollama requesting structured JSON.
4. Generated notes include commit hash provenance badges:
   - Valid hash: 1.0 confidence (`[AI] provider:model (100%)`).
   - Fabricated or unmatched hash: flagged `[AI] (Unsourced)` with a warning and 0.3 confidence.
5. Edit notes in the Markdown editor on the left and observe real-time rendering in the preview panel on the right.
6. Click **Finalize Release** to commit the release record to the local SQLite database, or **Export Markdown** to save to `RELEASE_NOTES.md`.

### Step 6: Publish Draft Release to GitHub or GitLab
1. Once a release is finalized, click **Publish Draft** in the toolbar.
2. The system connects to GitHub or GitLab using personal access tokens stored securely in your OS keychain.
3. Draft release notes are created or updated idempotently without exposing credentials or modifying local Git files.

### Step 7: Review Audit Trail & Reports (`Ctrl+6`)
1. Open the **History** tab to inspect finalized releases and previous audit trails.
2. Export structured Markdown or JSON release reports containing model metadata, timestamps, commit hashes, and full readiness check results.
