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

---

## 5.3 Search

- [ ] **Implement**: `ori search <query>`
  - [ ] Search registry
  - [ ] Show availability
  - [ ] **Rust Tests**: `ori_pkg/src/client/search.rs`

---

## 5.4 Package Info

- [ ] **Implement**: `ori info <package>`
  - [ ] Fetch and display metadata
  - [ ] **Rust Tests**: `ori_pkg/src/client/info.rs`

---

## 5.5 Multi-Registry Support

- [ ] **Implement**: Registry selection by scope
  - [ ] `@company/*` → company registry
  - [ ] Default for others
  - [ ] **Rust Tests**: `ori_pkg/src/client/registry.rs`

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
