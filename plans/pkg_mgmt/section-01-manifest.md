---
section: "01"
title: "Manifest & Lock File"
status: not-started
goal: "Parse and validate oripk.toml and oripk.lock"
depends_on: []
sections:
  - id: "1.1"
    title: "Manifest Parsing (oripk.toml)"
    status: not-started
  - id: "1.2"
    title: "Manifest Validation"
    status: not-started
  - id: "1.3"
    title: "Lock File Parsing (oripk.lock)"
    status: not-started
  - id: "1.4"
    title: "Lock File Generation"
    status: not-started
  - id: "1.5"
    title: "Manifest Formatting"
    status: not-started
  - id: "1.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 1: Manifest & Lock File

**Goal**: Parse and validate oripk.toml and oripk.lock

**Status**: ⬜ Not Started

---

## 1.1 Manifest Parsing (oripk.toml)

### Project Section

- [ ] **Implement**: Parse `[project]` section
  - [ ] `name` — scoped package name (@scope/name)
  - [ ] `version` — strict semver (major.minor.patch)
  - [ ] `description` — required for publish
  - [ ] `license` — SPDX expression
  - [ ] `authors` — array of strings
  - [ ] `repository` — URL
  - [ ] `keywords` — array
  - [ ] `categories` — array
  - [ ] `funding` — optional URL
  - [ ] `edition` — Ori edition
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/project.rs`

- [ ] **Implement**: Parse `ori` version requirement
  - [ ] Required field
  - [ ] Strict semver format
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/ori_version.rs`

- [ ] **Implement**: Parse `[project.entry]`
  - [ ] `lib` — library entry point
  - [ ] `main` — binary entry point
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/entry.rs`

### Dependencies Section

- [ ] **Implement**: Parse `[dependencies]`
  - [ ] Stdlib deps: `std.json` (no version)
  - [ ] Exact versions: `@scope/name = "1.0.0"`
  - [ ] With features: `[dependencies.@scope/name]`
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse git dependencies
  - [ ] `git = "url"` + `rev = "hash"`
  - [ ] Mark as non-publishable
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse path dependencies
  - [ ] `path = "../local"`
  - [ ] Mark as non-publishable
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse `[dev-dependencies]`
  - [ ] Same format as dependencies
  - [ ] Isolated feature resolution
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

- [ ] **Implement**: Parse `[target."cfg(...)".dependencies]`
  - [ ] Platform-specific deps
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/deps.rs`

### Features Section

- [ ] **Implement**: Parse `[features]`
  - [ ] `default` — default features
  - [ ] Feature enables deps
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/features.rs`

### Other Sections

- [ ] **Implement**: Parse `[registries]`
  - [ ] Private registry URLs
  - [ ] Auth token reference
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/registries.rs`

- [ ] **Implement**: Parse `[scripts]`
  - [ ] Simple string commands
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/scripts.rs`

- [ ] **Implement**: Parse `[publish]`
  - [ ] `registry` — target registry
  - [ ] `include` / `exclude` — file patterns
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/publish.rs`

- [ ] **Subsection close-out (1.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 1.2 Manifest Validation

- [ ] **Implement**: Package name validation
  - [ ] Lowercase a-z, 0-9, hyphens only
  - [ ] Max 64 characters
  - [ ] Reserved names rejected
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Version format validation
  - [ ] Strict major.minor.patch
  - [ ] No build metadata
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Required fields validation
  - [ ] `name`, `version`, `ori` required
  - [ ] `description` required for publish
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Implement**: Unknown fields handling
  - [ ] Warn and ignore
  - [ ] Forward compatibility
  - [ ] **Rust Tests**: `ori_pkg/src/manifest/validate.rs`

- [ ] **Subsection close-out (1.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 1.3 Lock File Parsing (oripk.lock)

The lock file contains **only checksums** for security verification, not for version locking.

- [ ] **Implement**: Parse `[checksums]` section
  - [ ] Format: `"@scope/name:version" = "sha256:..."`
  - [ ] Validate checksum format
  - [ ] **Rust Tests**: `ori_pkg/src/lock/checksums.rs`

- [ ] **Subsection close-out (1.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 1.4 Lock File Generation

- [ ] **Implement**: Generate lock file from resolution
  - [ ] Alphabetical ordering by package:version
  - [ ] SHA256 checksums only
  - [ ] **Rust Tests**: `ori_pkg/src/lock/generate.rs`

- [ ] **Implement**: Staleness detection
  - [ ] Detect when oripk.toml deps changed
  - [ ] Auto-regenerate on `ori sync` (no error)
  - [ ] **Rust Tests**: `ori_pkg/src/lock/staleness.rs`

- [ ] **Subsection close-out (1.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 1.5 Manifest Formatting

- [ ] **Implement**: Format oripk.toml via `ori fmt`
  - [ ] Integrate with existing formatter
  - [ ] Consistent key ordering
  - [ ] **Rust Tests**: `ori_fmt/src/toml.rs`

- [ ] **Subsection close-out (1.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 1.6 Phase Completion Checklist

- [ ] oripk.toml parsing complete
- [ ] oripk.lock parsing complete (checksums only)
- [ ] Validation with helpful errors
- [ ] Unknown fields warn, don't error
- [ ] Formatting integration
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can parse and validate oripk.toml and oripk.lock files

- [ ] **Subsection close-out (1.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-1.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 1.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

