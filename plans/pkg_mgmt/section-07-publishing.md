---
section: "07"
title: "Publishing"
status: not-started
goal: "Package publishing workflow"
depends_on: ["06"]
sections:
  - id: "7.1"
    title: "ori login"
    status: not-started
  - id: "7.2"
    title: "Pre-publish Validation"
    status: not-started
  - id: "7.3"
    title: "Archive Creation"
    status: not-started
  - id: "7.4"
    title: "ori publish"
    status: not-started
  - id: "7.5"
    title: "Version Management"
    status: not-started
  - id: "7.6"
    title: "Version Bumping"
    status: not-started
  - id: "7.7"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 7: Publishing

**Goal**: Package publishing workflow

**Status**: ⬜ Not Started

---

## 7.1 ori login

- [ ] **Implement**: `ori login`
  - [ ] Prompt for token
  - [ ] Store in environment
  - [ ] **Rust Tests**: `oric/src/commands/login.rs`

- [ ] **Subsection close-out (7.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.2 Pre-publish Validation

- [ ] **Implement**: Validate publishable
  - [ ] No git dependencies
  - [ ] No path dependencies
  - [ ] Tests must pass
  - [ ] Description required
  - [ ] **Rust Tests**: `ori_pkg/src/publish/validate.rs`

- [ ] **Implement**: Version immutability check
  - [ ] Error if version exists
  - [ ] **Rust Tests**: `ori_pkg/src/publish/validate.rs`

- [ ] **Subsection close-out (7.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.3 Archive Creation

- [ ] **Implement**: Create package archive
  - [ ] Strip `[scripts]` section
  - [ ] Include oripk.lock (checksums for verification)
  - [ ] Compute checksums
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

- [ ] **Implement**: Respect include/exclude
  - [ ] From `[publish]` section
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

- [ ] **Subsection close-out (7.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.4 ori publish

- [ ] **Implement**: `ori publish`
  - [ ] Run validation
  - [ ] Create archive
  - [ ] Upload to registry
  - [ ] **Rust Tests**: `oric/src/commands/publish.rs`

- [ ] **Implement**: `ori publish --dry-run`
  - [ ] Validate without uploading
  - [ ] **Rust Tests**: `oric/src/commands/publish.rs`

- [ ] **Subsection close-out (7.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.5 Version Management

- [ ] **Implement**: `ori yank <version>`
  - [ ] Mark version as yanked
  - [ ] **Rust Tests**: `oric/src/commands/yank.rs`

- [ ] **Implement**: `ori unyank <version>`
  - [ ] Restore yanked version
  - [ ] **Rust Tests**: `oric/src/commands/unyank.rs`

- [ ] **Implement**: `ori deprecate <version> <message>`
  - [ ] Add deprecation warning
  - [ ] **Rust Tests**: `oric/src/commands/deprecate.rs`

- [ ] **Subsection close-out (7.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.6 Version Bumping

- [ ] **Implement**: `ori version patch`
  - [ ] Bump 1.0.0 → 1.0.1
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

- [ ] **Implement**: `ori version minor`
  - [ ] Bump 1.0.0 → 1.1.0
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

- [ ] **Implement**: `ori version major`
  - [ ] Bump 1.0.0 → 2.0.0
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

- [ ] **Subsection close-out (7.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 7.7 Phase Completion Checklist

- [ ] Login working
- [ ] Pre-publish validation
- [ ] Archive creation
- [ ] Publish with dry-run
- [ ] Yank/unyank/deprecate
- [ ] Version bumping
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can publish packages to registry

- [ ] **Subsection close-out (7.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-7.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 7.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

