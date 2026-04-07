---
section: "05"
title: "Registry Client"
status: not-started
goal: "Client-side registry communication"
depends_on: ["04"]
sections:
  - id: "5.1"
    title: "HTTP Client"
    status: not-started
  - id: "5.2"
    title: "Package Fetching"
    status: not-started
  - id: "5.3"
    title: "Search"
    status: not-started
  - id: "5.4"
    title: "Package Info"
    status: not-started
  - id: "5.5"
    title: "Multi-Registry Support"
    status: not-started
  - id: "5.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 5: Registry Client

**Goal**: Client-side registry communication

**Status**: ⬜ Not Started

---

## 5.1 HTTP Client

- [ ] **Implement**: Async HTTP client
  - [ ] Parallel metadata fetching
  - [ ] **Rust Tests**: `ori_pkg/src/client/http.rs`

- [ ] **Implement**: Timeout handling
  - [ ] 30 second default
  - [ ] **Rust Tests**: `ori_pkg/src/client/http.rs`

- [ ] **Implement**: Retry logic
  - [ ] 3 retries with backoff
  - [ ] **Rust Tests**: `ori_pkg/src/client/http.rs`

- [ ] **Implement**: Proxy support
  - [ ] `HTTP_PROXY`, `HTTPS_PROXY`, `NO_PROXY`
  - [ ] **Rust Tests**: `ori_pkg/src/client/proxy.rs`

- [ ] **Subsection close-out (5.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.2 Package Fetching

- [ ] **Implement**: Fetch package metadata
  - [ ] Cache responses
  - [ ] **Rust Tests**: `ori_pkg/src/client/fetch.rs`

- [ ] **Implement**: Download package archive
  - [ ] Progress reporting
  - [ ] Checksum verification
  - [ ] **Rust Tests**: `ori_pkg/src/client/download.rs`

- [ ] **Implement**: Progress bars
  - [ ] Show download progress
  - [ ] **Rust Tests**: `ori_pkg/src/client/progress.rs`

- [ ] **Subsection close-out (5.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.3 Search

- [ ] **Implement**: `ori search <query>`
  - [ ] Search registry
  - [ ] Show availability
  - [ ] **Rust Tests**: `ori_pkg/src/client/search.rs`

- [ ] **Subsection close-out (5.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.4 Package Info

- [ ] **Implement**: `ori info <package>`
  - [ ] Fetch and display metadata
  - [ ] **Rust Tests**: `ori_pkg/src/client/info.rs`

- [ ] **Subsection close-out (5.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.5 Multi-Registry Support

- [ ] **Implement**: Registry selection by scope
  - [ ] `@company/*` → company registry
  - [ ] Default for others
  - [ ] **Rust Tests**: `ori_pkg/src/client/registry.rs`

- [ ] **Subsection close-out (5.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.6 Phase Completion Checklist

- [ ] HTTP client with retry/timeout
- [ ] Proxy support
- [ ] Package fetching with progress
- [ ] Search working
- [ ] Multi-registry support
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can fetch packages from registry

- [ ] **Subsection close-out (5.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

