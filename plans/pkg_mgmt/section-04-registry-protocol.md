---
section: "04"
title: "Registry Protocol"
status: not-started
goal: "Define and implement registry HTTP API"
depends_on: ["03"]
sections:
  - id: "4.1"
    title: "API Endpoints"
    status: not-started
  - id: "4.2"
    title: "Authentication"
    status: not-started
  - id: "4.3"
    title: "Rate Limiting"
    status: not-started
  - id: "4.4"
    title: "Package Archive Format"
    status: not-started
  - id: "4.5"
    title: "Checksum Database"
    status: not-started
  - id: "4.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 4: Registry Protocol

**Goal**: Define and implement registry HTTP API

**Status**: ⬜ Not Started

---

## 4.1 API Endpoints

- [ ] **Implement**: `GET /v1/packages/{scope}/{name}/versions`
  - [ ] Return JSON list of versions
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `GET /v1/packages/{scope}/{name}/{version}/metadata`
  - [ ] Return package metadata
  - [ ] Dependencies, features, checksums
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `GET /v1/packages/{scope}/{name}/{version}/download`
  - [ ] Return package archive
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `POST /v1/packages/{scope}/{name}/publish`
  - [ ] Multipart upload
  - [ ] Require authentication
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `POST /v1/packages/{scope}/{name}/{version}/yank`
  - [ ] Mark version as yanked
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `POST /v1/packages/{scope}/{name}/{version}/unyank`
  - [ ] Restore yanked version
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `POST /v1/packages/{scope}/{name}/{version}/deprecate`
  - [ ] Add deprecation message
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `POST /v1/packages/{scope}/{name}/transfer`
  - [ ] Transfer ownership
  - [ ] Require both parties
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `GET /v1/search?q=query`
  - [ ] Search packages
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

- [ ] **Implement**: `GET /v1/advisories`
  - [ ] Security advisories
  - [ ] **Rust Tests**: `ori_pkg/src/registry/api.rs`

---

## 4.2 Authentication

- [ ] **Implement**: Bearer token authentication
  - [ ] `Authorization: Bearer <token>`
  - [ ] **Rust Tests**: `ori_pkg/src/registry/auth.rs`

- [ ] **Implement**: Token scopes
  - [ ] `read` — download, search
  - [ ] `publish` — publish to owned scopes
  - [ ] `admin` — manage owners
  - [ ] **Rust Tests**: `ori_pkg/src/registry/auth.rs`

- [ ] **Implement**: Environment variable tokens
  - [ ] `ORI_REGISTRY_TOKEN` for default
  - [ ] `ORI_REGISTRY_{NAME}_TOKEN` for named
  - [ ] **Rust Tests**: `ori_pkg/src/registry/auth.rs`

---

## 4.3 Rate Limiting

- [ ] **Implement**: Rate limit headers
  - [ ] `X-RateLimit-Limit`
  - [ ] `X-RateLimit-Remaining`
  - [ ] `X-RateLimit-Reset`
  - [ ] **Rust Tests**: `ori_pkg/src/registry/ratelimit.rs`

- [ ] **Implement**: Rate limit tiers
  - [ ] Authenticated: 1000/min
  - [ ] Unauthenticated: 100/min
  - [ ] **Rust Tests**: `ori_pkg/src/registry/ratelimit.rs`

---

## 4.4 Package Archive Format

- [ ] **Implement**: Archive structure
  - [ ] `.oripk` format
  - [ ] `oripk.toml` (scripts stripped)
  - [ ] `oripk.lock` (checksums for verification)
  - [ ] `CHECKSUM`
  - [ ] `src/`
  - [ ] **Rust Tests**: `ori_pkg/src/archive/format.rs`

- [ ] **Implement**: Size limit enforcement
  - [ ] 10MB compressed max
  - [ ] **Rust Tests**: `ori_pkg/src/archive/validate.rs`

---

## 4.5 Checksum Database

- [ ] **Implement**: Transparency log integration
  - [ ] Verify checksums against log
  - [ ] **Rust Tests**: `ori_pkg/src/registry/transparency.rs`

---

## 4.6 Phase Completion Checklist

- [ ] All API endpoints defined
- [ ] Authentication working
- [ ] Rate limiting
- [ ] Archive format validated
- [ ] Checksum verification
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Registry protocol fully specified and testable
