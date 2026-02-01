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

---

## 3.2 Project Linking

- [ ] **Implement**: `.ori/deps/` symlinks
  - [ ] Link to global cache
  - [ ] **Rust Tests**: `ori_pkg/src/install/link.rs`

- [ ] **Implement**: Windows support
  - [ ] Junction points preferred
  - [ ] File copy fallback
  - [ ] **Rust Tests**: `ori_pkg/src/install/link.rs`

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

---

## 3.4 Offline Mode

- [ ] **Implement**: Offline fallback
  - [ ] Use cached packages when registry unreachable
  - [ ] Show warning
  - [ ] Fail only if not cached
  - [ ] **Rust Tests**: `ori_pkg/src/cache/offline.rs`

---

## 3.5 Phase Completion Checklist

- [ ] Content-addressable storage works
- [ ] Project linking via symlinks
- [ ] Windows junction point support
- [ ] Cache clean/list/verify commands
- [ ] Offline fallback
- [ ] Run full test suite

**Exit Criteria**: Packages cached and linked correctly
