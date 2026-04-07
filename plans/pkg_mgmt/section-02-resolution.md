---
section: "02"
title: "Version Resolution"
status: not-started
goal: "Resolve dependency graph with exact versions"
depends_on: ["01"]
sections:
  - id: "2.1"
    title: "Dependency Graph"
    status: not-started
  - id: "2.2"
    title: "Version Matching"
    status: not-started
  - id: "2.3"
    title: "Conflict Detection"
    status: not-started
  - id: "2.4"
    title: "Feature Resolution"
    status: not-started
  - id: "2.5"
    title: "Stdlib Handling"
    status: not-started
  - id: "2.6"
    title: "Incremental Resolution"
    status: not-started
  - id: "2.7"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 2: Version Resolution

**Goal**: Resolve dependency graph with exact versions

**Status**: ⬜ Not Started

---

## 2.1 Dependency Graph

- [ ] **Implement**: Build dependency graph from manifest
  - [ ] Parse all dependencies
  - [ ] Resolve transitive deps
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/graph.rs`

- [ ] **Implement**: Circular dependency detection
  - [ ] Error with full cycle path
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/cycle.rs`

- [ ] **Subsection close-out (2.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.2 Version Matching

- [ ] **Implement**: Exact version matching
  - [ ] "1.2.3" matches exactly 1.2.3
  - [ ] No range semantics
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/match.rs`

- [ ] **Implement**: Pre-release handling
  - [ ] Opt-in only
  - [ ] "1.0.0" never matches "1.0.0-alpha"
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/prerelease.rs`

- [ ] **Subsection close-out (2.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.3 Conflict Detection

- [ ] **Implement**: Single version policy
  - [ ] Only one version of each package
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/conflict.rs`

- [ ] **Implement**: Conflict error messages
  - [ ] Show which packages require different versions
  - [ ] Suggest finding compatible versions
  - [ ] **No patch escape hatch** - conflicts must be resolved properly
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/conflict.rs`

- [ ] **Subsection close-out (2.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.4 Feature Resolution

- [ ] **Implement**: Feature resolution per dependency kind
  - [ ] Normal deps isolated from dev deps
  - [ ] Platform deps isolated
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/features.rs`

- [ ] **Implement**: Default features
  - [ ] Apply unless `default-features = false`
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/features.rs`

- [ ] **Subsection close-out (2.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.5 Stdlib Handling

- [ ] **Implement**: Bundled stdlib resolution
  - [ ] std.* deps don't need version
  - [ ] Implied from `ori` version
  - [ ] Not included in lock file
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/stdlib.rs`

- [ ] **Subsection close-out (2.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.6 Incremental Resolution

- [ ] **Implement**: Reuse lock file decisions
  - [ ] If version in lock matches constraint, keep it
  - [ ] Only resolve changed deps
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/incremental.rs`

- [ ] **Subsection close-out (2.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 2.7 Phase Completion Checklist

- [ ] Dependency graph construction
- [ ] Exact version matching
- [ ] Single version policy with good errors (no patch escape hatch)
- [ ] Feature isolation
- [ ] Stdlib handling
- [ ] Incremental resolution
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can resolve dependency graph with exact versions, detect conflicts

- [ ] **Subsection close-out (2.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

