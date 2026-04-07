---
section: "08"
title: "Workspaces"
status: not-started
goal: "Monorepo support with shared dependencies"
depends_on: ["07"]
sections:
  - id: "8.1"
    title: "Workspace Configuration"
    status: not-started
  - id: "8.2"
    title: "Workspace Resolution"
    status: not-started
  - id: "8.3"
    title: "Workspace Scripts"
    status: not-started
  - id: "8.4"
    title: "Workspace Commands"
    status: not-started
  - id: "8.5"
    title: "Workspace Publishing"
    status: not-started
  - id: "8.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 8: Workspaces

**Goal**: Monorepo support with shared dependencies

**Status**: ⬜ Not Started

---

## 8.1 Workspace Configuration

- [ ] **Implement**: Parse `[workspace]` section
  - [ ] `members` array
  - [ ] `exclude` patterns
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/config.rs`

- [ ] **Implement**: Parse `[workspace.dependencies]`
  - [ ] Shared dependency versions
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/deps.rs`

- [ ] **Implement**: Member `workspace = true` syntax
  - [ ] Reference workspace deps
  - [ ] Add features to workspace deps
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/member.rs`

- [ ] **Subsection close-out (8.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.2 Workspace Resolution

- [ ] **Implement**: Single lock file at root
  - [ ] All members share lock
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/lock.rs`

- [ ] **Implement**: Single version per package
  - [ ] Across all members
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/resolve.rs`

- [ ] **Subsection close-out (8.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.3 Workspace Scripts

- [ ] **Implement**: Root scripts available everywhere
  - [ ] `ori run` from any member uses root scripts
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/scripts.rs`

- [ ] **Subsection close-out (8.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.4 Workspace Commands

- [ ] **Implement**: `ori workspace list`
  - [ ] List all members
  - [ ] **Rust Tests**: `oric/src/commands/workspace.rs`

- [ ] **Implement**: `ori workspace add <path>`
  - [ ] Add member to workspace
  - [ ] **Rust Tests**: `oric/src/commands/workspace.rs`

- [ ] **Implement**: `ori build --workspace`
  - [ ] Build all members
  - [ ] **Rust Tests**: `oric/src/commands/build.rs`

- [ ] **Implement**: `ori test --workspace`
  - [ ] Test all members
  - [ ] **Rust Tests**: `oric/src/commands/test.rs`

- [ ] **Subsection close-out (8.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.5 Workspace Publishing

- [ ] **Implement**: Independent member publishing
  - [ ] Each member has own version
  - [ ] Publish individually
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/publish.rs`

- [ ] **Subsection close-out (8.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.6 Phase Completion Checklist

- [ ] Workspace configuration parsing
- [ ] Shared dependencies
- [ ] Single lock file
- [ ] Script inheritance
- [ ] Workspace commands
- [ ] Independent publishing
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Monorepo workflows working

- [ ] **Subsection close-out (8.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

