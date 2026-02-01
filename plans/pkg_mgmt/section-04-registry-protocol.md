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

**Exit Criteria**: Registry protocol fully specified and testable
