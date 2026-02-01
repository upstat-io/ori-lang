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

**Exit Criteria**: Can fetch packages from registry
