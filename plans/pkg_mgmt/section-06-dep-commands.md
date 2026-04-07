---
section: "06"
title: "Dependency Commands"
status: not-started
goal: "CLI commands for dependency management"
depends_on: ["05"]
sections:
  - id: "6.1"
    title: "ori sync"
    status: not-started
  - id: "6.2"
    title: "ori check"
    status: not-started
  - id: "6.3"
    title: "ori install"
    status: not-started
  - id: "6.4"
    title: "ori upgrade"
    status: not-started
  - id: "6.5"
    title: "ori remove"
    status: not-started
  - id: "6.6"
    title: "ori clean"
    status: not-started
  - id: "6.7"
    title: "ori audit"
    status: not-started
  - id: "6.8"
    title: "Analysis Commands"
    status: not-started
  - id: "6.9"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 6: Dependency Commands

**Goal**: CLI commands for dependency management

**Status**: ⬜ Not Started

---

## 6.1 ori sync

- [ ] **Implement**: `ori sync`
  - [ ] Sync dependencies to manifest
  - [ ] Auto-regenerate oripk.lock if stale (no error)
  - [ ] Download missing packages
  - [ ] Verify checksums
  - [ ] **Rust Tests**: `oric/src/commands/sync.rs`

- [ ] **Subsection close-out (6.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.2 ori check

- [ ] **Implement**: `ori check`
  - [ ] Show available updates (informational only)
  - [ ] No modifications to manifest
  - [ ] **Rust Tests**: `oric/src/commands/check.rs`

- [ ] **Implement**: `ori check @scope/package`
  - [ ] Show available versions for specific package
  - [ ] **Rust Tests**: `oric/src/commands/check.rs`

- [ ] **Subsection close-out (6.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.3 ori install

- [ ] **Implement**: `ori install @scope/package`
  - [ ] Add NEW dependency only
  - [ ] Without version: shows available versions, prompts for confirmation
  - [ ] ERROR if package already exists (tells user to use `ori upgrade`)
  - [ ] Pin exact version in oripk.toml
  - [ ] Run sync
  - [ ] **Rust Tests**: `oric/src/commands/install.rs`

- [ ] **Implement**: `ori install @scope/package --dev`
  - [ ] Add to dev-dependencies
  - [ ] **Rust Tests**: `oric/src/commands/install.rs`

- [ ] **Implement**: `ori install @scope/package --features a,b`
  - [ ] Enable features
  - [ ] **Rust Tests**: `oric/src/commands/install.rs`

- [ ] **Subsection close-out (6.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.4 ori upgrade

- [ ] **Implement**: `ori upgrade @scope/package:1.2.3`
  - [ ] Update EXISTING dependency only
  - [ ] Version is required (no implicit latest)
  - [ ] ERROR if package doesn't exist (tells user to use `ori install`)
  - [ ] With transitive changes: shows what will change, prompts unless `--yes`
  - [ ] Without transitive changes: just does it, no prompt
  - [ ] **Rust Tests**: `oric/src/commands/upgrade.rs`

- [ ] **Implement**: `ori upgrade @scope/package`
  - [ ] Without version: shows available versions (informational)
  - [ ] Does not modify anything
  - [ ] **Rust Tests**: `oric/src/commands/upgrade.rs`

- [ ] **Implement**: `ori upgrade @scope/package:1.2.3 --yes`
  - [ ] Skip prompts for transitive changes
  - [ ] **Rust Tests**: `oric/src/commands/upgrade.rs`

- [ ] **Subsection close-out (6.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.5 ori remove

- [ ] **Implement**: `ori remove @scope/package`
  - [ ] Remove from oripk.toml
  - [ ] Run sync
  - [ ] **Rust Tests**: `oric/src/commands/remove.rs`

- [ ] **Subsection close-out (6.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.6 ori clean

- [ ] **Implement**: `ori clean`
  - [ ] Wipe local package cache
  - [ ] **Rust Tests**: `oric/src/commands/clean.rs`

- [ ] **Subsection close-out (6.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.7 ori audit

- [ ] **Implement**: `ori audit`
  - [ ] Check against advisory database
  - [ ] Report vulnerabilities
  - [ ] **Rust Tests**: `oric/src/commands/audit.rs`

- [ ] **Subsection close-out (6.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.8 Analysis Commands

- [ ] **Implement**: `ori deps`
  - [ ] Show dependency tree
  - [ ] **Rust Tests**: `oric/src/commands/deps.rs`

- [ ] **Implement**: `ori deps --sizes`
  - [ ] Show size breakdown
  - [ ] **Rust Tests**: `oric/src/commands/deps.rs`

- [ ] **Implement**: `ori deps --graph`
  - [ ] DOT format output
  - [ ] **Rust Tests**: `oric/src/commands/deps.rs`

- [ ] **Implement**: `ori why @scope/package`
  - [ ] Show why package is included
  - [ ] **Rust Tests**: `oric/src/commands/why.rs`

- [ ] **Implement**: `ori diff 1.0.0 1.1.0`
  - [ ] Compare dependency changes
  - [ ] **Rust Tests**: `oric/src/commands/diff.rs`

- [ ] **Implement**: `ori licenses`
  - [ ] Show license summary
  - [ ] **Rust Tests**: `oric/src/commands/licenses.rs`

- [ ] **Subsection close-out (6.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 6.9 Phase Completion Checklist

- [ ] sync working (auto-regenerates lock)
- [ ] check shows available updates
- [ ] install for new deps (prompts for version)
- [ ] upgrade for existing deps (requires version)
- [ ] remove working
- [ ] clean wipes cache
- [ ] audit against advisories
- [ ] Analysis commands (deps, why, diff, licenses)
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Full dependency management via CLI

- [ ] **Subsection close-out (6.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-6.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 6.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

