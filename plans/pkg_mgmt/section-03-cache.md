---
section: "03"
title: "Cache & Installation"
status: not-started
goal: "Content-addressable cache and project linking"
depends_on: ["02"]
sections:
  - id: "3.1"
    title: "Global Cache Structure"
    status: not-started
  - id: "3.2"
    title: "Project Linking"
    status: not-started
  - id: "3.3"
    title: "Cache Operations"
    status: not-started
  - id: "3.4"
    title: "Offline Mode"
    status: not-started
  - id: "3.5"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 3: Cache & Installation

**Goal**: Content-addressable cache and project linking

**Status**: ⬜ Not Started

---

## 3.1 Global Cache Structure

- [ ] **Implement**: Content-addressable storage
  - [ ] `~/.ori/cache/packages/sha256-xxx/` structure
  - [ ] Package contents stored by hash
  - [ ] **Rust Tests**: `ori_pkg/src/cache/store.rs`

- [ ] **Implement**: Registry metadata cache
  - [ ] `~/.ori/cache/registry/` structure
  - [ ] Version lists, package metadata
  - [ ] **Rust Tests**: `ori_pkg/src/cache/registry.rs`

- [ ] **Implement**: Git dependency cache
  - [ ] `~/.ori/cache/git/` structure
  - [ ] Cached by repo + commit
  - [ ] **Rust Tests**: `ori_pkg/src/cache/git.rs`

- [ ] **Subsection close-out (3.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-3.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 3.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 3.2 Project Linking

- [ ] **Implement**: `.ori/deps/` symlinks
  - [ ] Link to global cache
  - [ ] **Rust Tests**: `ori_pkg/src/install/link.rs`

- [ ] **Implement**: Windows support
  - [ ] Junction points preferred
  - [ ] File copy fallback
  - [ ] **Rust Tests**: `ori_pkg/src/install/link.rs`

- [ ] **Subsection close-out (3.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-3.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 3.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 3.3 Cache Operations

- [ ] **Implement**: `ori cache clean`
  - [ ] Remove unused packages
  - [ ] Track last-used timestamps
  - [ ] **Rust Tests**: `ori_pkg/src/cache/clean.rs`

- [ ] **Implement**: `ori cache list`
  - [ ] Show cache contents
  - [ ] Size information
  - [ ] **Rust Tests**: `ori_pkg/src/cache/list.rs`

- [ ] **Implement**: `ori cache verify`
  - [ ] Verify checksums
  - [ ] Report corruption
  - [ ] **Rust Tests**: `ori_pkg/src/cache/verify.rs`

- [ ] **Subsection close-out (3.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-3.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 3.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 3.4 Offline Mode

- [ ] **Implement**: Offline fallback
  - [ ] Use cached packages when registry unreachable
  - [ ] Show warning
  - [ ] Fail only if not cached
  - [ ] **Rust Tests**: `ori_pkg/src/cache/offline.rs`

- [ ] **Subsection close-out (3.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-3.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 3.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 3.5 Phase Completion Checklist

- [ ] Content-addressable storage works
- [ ] Project linking via symlinks
- [ ] Windows junction point support
- [ ] Cache clean/list/verify commands
- [ ] Offline fallback
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Packages cached and linked correctly

- [ ] **Subsection close-out (3.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-3.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 3.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

