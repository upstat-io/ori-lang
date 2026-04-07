---
section: "11"
title: "Registry Infrastructure"
status: not-started
goal: "Deploy registry on Cloudflare"
depends_on: ["10"]
sections:
  - id: "11.1"
    title: "Architecture"
    status: not-started
  - id: "11.2"
    title: "Workers (Ori/WASM)"
    status: not-started
  - id: "11.3"
    title: "Containers (Ori native)"
    status: not-started
  - id: "11.4"
    title: "R2 Storage"
    status: not-started
  - id: "11.5"
    title: "KV Storage"
    status: not-started
  - id: "11.6"
    title: "Advisory Database"
    status: not-started
  - id: "11.7"
    title: "Monitoring"
    status: not-started
  - id: "11.8"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 11: Registry Infrastructure

**Goal**: Deploy registry on Cloudflare

**Status**: ⬜ Not Started

---

## 11.1 Architecture

```
┌─────────────────────────────────────┐
│         Cloudflare Edge             │
│                                     │
│  Workers (Ori/WASM)                 │
│    ├── API endpoints                │
│    ├── Auth / rate limiting         │
│    ├── Search                       │
│    │                                │
│  Containers (Ori native)            │
│    ├── Package processing           │
│    ├── Advisory scanning            │
│    │                                │
│   R2              KV                │
│  (packages)    (metadata)           │
└─────────────────────────────────────┘
```

- [ ] **Subsection close-out (11.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.2 Workers (Ori/WASM)

- [ ] **Implement**: API endpoint handlers
  - [ ] All v1 endpoints
  - [ ] Written in Ori, compiled to WASM
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Authentication middleware
  - [ ] Token validation
  - [ ] Scope checking
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Rate limiting
  - [ ] Per-IP and per-token limits
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Search indexing
  - [ ] Query KV for matches
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.3 Containers (Ori native)

- [ ] **Implement**: Package processor
  - [ ] Validate uploads
  - [ ] Compute checksums
  - [ ] Written in Ori, native binary
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Advisory scanner
  - [ ] Check new packages against advisories
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.4 R2 Storage

- [ ] **Implement**: Package storage
  - [ ] `.ori.tar.gz` archives
  - [ ] Content-addressed paths
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Checksum storage
  - [ ] Transparency log data
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.5 KV Storage

- [ ] **Implement**: Package metadata
  - [ ] Version lists
  - [ ] Package info
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Search index
  - [ ] Name/description indexing
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: User/scope data
  - [ ] Ownership records
  - [ ] Token records
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.6 Advisory Database

- [ ] **Implement**: Advisory storage
  - [ ] CVE records
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Advisory API
  - [ ] Query by package
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.7 Monitoring

- [ ] **Implement**: Health checks
  - [ ] Endpoint monitoring
  - [ ] **Tests**: Integration tests

- [ ] **Implement**: Error tracking
  - [ ] Log errors
  - [ ] **Tests**: Integration tests

- [ ] **Subsection close-out (11.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 11.8 Phase Completion Checklist

- [ ] Workers handling all API endpoints
- [ ] Containers processing packages
- [ ] R2 storing packages
- [ ] KV storing metadata
- [ ] Advisory database working
- [ ] Monitoring in place
- [ ] Production deployment
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Registry running on Cloudflare

- [ ] **Subsection close-out (11.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-11.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 11.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

