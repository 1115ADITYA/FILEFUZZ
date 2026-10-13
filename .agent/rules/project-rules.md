# Mandatory Agent Engineering & Safety Rules

These rules are non-negotiable constraints governing all AI agent activity for the **Offline File Finder** project. They take precedence over development speed or convenience.

---

### Rule 1: Single-Phase Scope
Work on only one phase or clearly defined small subtask at a time. Never implement multiple phases or future roadmap features in a single prompt or change.

### Rule 2: Documentation-First Workflow
Read `PRD.md`, `ARCHITECTURE.md`, `TASKS.md`, `PROGRESS.md`, and this rules file before making code modifications.

### Rule 3: Inspect Before Editing
Always inspect the current project state, active branch, and `git status` before writing or modifying code. Preserve working code and user changes.

### Rule 4: Preserve Working Code
Never delete, rewrite, or overwrite working code, configuration, or tests without documented technical necessity and user alignment.

### Rule 5: Strict Sandbox — No Real Folders During Development
Never scan real user folders (such as Desktop, Documents, Downloads, or entire drive letters) during development.

### Rule 6: Canonical `test-files/` Boundary
All development-time scanner tests and benchmarks must be strictly restricted to `<project-root>/test-files/`. The scanner must verify canonical root paths and reject any path that attempts to escape this root.

### Rule 7: User Files are Read-Only Inputs
Never intentionally modify, rename, move, overwrite, clean up, or delete original user files under any circumstance. Open files strictly with read-only permissions.

### Rule 8: Strict Data Segregation
Keep all databases (`metadata.sqlite`), Tantivy search indexes, caches, and log files in the platform-standard application data directory (`%LOCALAPPDATA%\com.offlinefilefinder.app`). Never write metadata, indexes, or temporary files into scanned folders.

### Rule 9: 100% Offline & Zero Telemetry
The application must operate strictly offline. No cloud LLMs, no remote embedding APIs, no cloud document processors, and no telemetry or analytics services are permitted.

### Rule 10: Just-in-Time Dependencies
Do not introduce future-phase dependencies (e.g. Tantivy, Rusqlite, FastEmbed, Tesseract) before their designated milestone. Only add libraries required for the current phase after verifying toolchain compatibility.

### Rule 11: Honest & Verified Testing
Every change must be validated using concrete build, format, and execution checks (`npm run build`, `cargo check`, `cargo build`, etc.). Never fabricate test results or claim a feature works without executed proof.

### Rule 12: Two-Failure Escalation Stop
If the same build, compile, or test fix fails twice consecutively, stop immediately. Report the exact error, what was attempted, and the minimal recommended next step. Do not repeatedly guess or make large refactors.

### Rule 13: No Automatic Git Commits or Pushes
Do not create automatic Git commits or push to remote repositories. Always display the modified file summary and `git status` so the user can review and commit verified checkpoints themselves.

### Rule 14: Stop and Await Approval
Stop immediately upon completing the currently authorized phase or task. Never automatically proceed to the next phase without explicit user approval.

### Rule 15: No Premature Automatic Scanning
Do not implement real-folder scanning, background file watchers, or automatic startup scanning until its designated phase (Phase 9) and after security boundaries have been fully audited.
