# ReleaseWeave Privacy Policy & Commitments

ReleaseWeave is designed under a strict **Local-First & Zero-Telemetry** philosophy. Your source code is your most valuable asset and must never leave your development machine without your explicit direction.

---

## 1. 100% Local Data Processing

1. **Zero Telemetry:**
   - ReleaseWeave does not collect or transmit telemetry, user metrics, tracking identifiers, crash reports, or analytics to any remote server.
2. **Local AI Model Execution:**
   - AI inferences are dispatched exclusively to your locally hosted Ollama instance (`http://localhost:11434` by default).
   - Git commits, diffs, code summaries, and metadata are **never** shared with third-party cloud AI vendors.
3. **Local-First Persistence:**
   - The release database is stored locally in standard OS data directories (for example, `~/.local/share/release-weave/` on Linux).
   - No cloud syncing is active by default.

---

## 2. Repository Integrity Guarantees

- ReleaseWeave never silently modifies your Git repository and never initiates unprompted remote push actions.
- Staging draft releases, calculating SemVer bumps, and inspecting changes are entirely read-only with respect to your working tree.
- Creating local annotated Git tags is opt-in (disabled by default in Settings), requires a clean working tree and finalized release status, and is never pushed remotely.
- Generating output artifacts (such as `RELEASE_NOTES.md` or `report.json`) and applying version updates to manifests or changelogs requires explicit user confirmation via visual diff inspection before atomic disk writes take place.
- Remote publishing to GitHub Releases or GitLab Releases only occurs when explicitly initiated on a finalized release, using tokens securely fetched from your local OS keychain.
