---
section: "10"
title: "Tooling"
status: not-started
goal: "Developer experience commands"
depends_on: ["09"]
sections:
  - id: "10.1"
    title: "REPL"
    status: not-started
  - id: "10.2"
    title: "Documentation"
    status: not-started
  - id: "10.3"
    title: "Project Creation"
    status: not-started
  - id: "10.4"
    title: "System Commands"
    status: not-started
  - id: "10.5"
    title: "Cleanup"
    status: not-started
  - id: "10.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 10: Tooling

**Goal**: Developer experience commands

**Status**: ⬜ Not Started

---

## 10.1 REPL

- [ ] **Implement**: `ori repl`
  - [ ] Interactive Ori shell
  - [ ] Expression evaluation
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

- [ ] **Implement**: REPL history
  - [ ] Persist history
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

- [ ] **Implement**: REPL completion
  - [ ] Tab completion
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

- [ ] **Subsection close-out (10.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.2 Documentation

- [ ] **Implement**: `ori docs`
  - [ ] Open Ori documentation
  - [ ] **Rust Tests**: `oric/src/commands/docs.rs`

- [ ] **Implement**: `ori docs @scope/package`
  - [ ] Open package repository
  - [ ] **Rust Tests**: `oric/src/commands/docs.rs`

- [ ] **Subsection close-out (10.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.3 Project Creation

- [ ] **Implement**: `ori new <name>`
  - [ ] Create project directory
  - [ ] Generate oripk.toml
  - [ ] Generate .gitignore
  - [ ] Flags only (non-interactive)
  - [ ] **Rust Tests**: `oric/src/commands/new.rs`

- [ ] **Implement**: `ori new <name> --lib`
  - [ ] Library project
  - [ ] **Rust Tests**: `oric/src/commands/new.rs`

- [ ] **Implement**: `ori init`
  - [ ] Initialize in current directory
  - [ ] **Rust Tests**: `oric/src/commands/init.rs`

- [ ] **Subsection close-out (10.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.4 System Commands

- [ ] **Implement**: `ori self-update`
  - [ ] Download and replace binary
  - [ ] **Rust Tests**: `oric/src/commands/self_update.rs`

- [ ] **Implement**: `ori doctor`
  - [ ] Diagnose setup issues
  - [ ] Check connectivity
  - [ ] Verify cache
  - [ ] **Rust Tests**: `oric/src/commands/doctor.rs`

- [ ] **Implement**: `ori completions <shell>`
  - [ ] Generate shell completions
  - [ ] bash, zsh, fish
  - [ ] **Rust Tests**: `oric/src/commands/completions.rs`

- [ ] **Subsection close-out (10.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.5 Cleanup

- [ ] **Implement**: `ori clean`
  - [ ] Remove build artifacts
  - [ ] Remove .ori/deps
  - [ ] **Rust Tests**: `oric/src/commands/clean.rs`

- [ ] **Subsection close-out (10.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 10.6 Phase Completion Checklist

- [ ] REPL working
- [ ] Documentation commands
- [ ] Project creation
- [ ] Self-update
- [ ] Doctor diagnostics
- [ ] Shell completions
- [ ] Clean command
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Full developer tooling

- [ ] **Subsection close-out (10.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-10.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 10.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

