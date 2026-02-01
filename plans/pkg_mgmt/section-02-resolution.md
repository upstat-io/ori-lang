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

---

## 2.4 Feature Resolution

- [ ] **Implement**: Feature resolution per dependency kind
  - [ ] Normal deps isolated from dev deps
  - [ ] Platform deps isolated
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/features.rs`

- [ ] **Implement**: Default features
  - [ ] Apply unless `default-features = false`
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/features.rs`

---

## 2.5 Stdlib Handling

- [ ] **Implement**: Bundled stdlib resolution
  - [ ] std.* deps don't need version
  - [ ] Implied from `ori` version
  - [ ] Not included in lock file
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/stdlib.rs`

---

## 2.6 Incremental Resolution

- [ ] **Implement**: Reuse lock file decisions
  - [ ] If version in lock matches constraint, keep it
  - [ ] Only resolve changed deps
  - [ ] **Rust Tests**: `ori_pkg/src/resolve/incremental.rs`

---

## 2.7 Phase Completion Checklist

- [ ] Dependency graph construction
- [ ] Exact version matching
- [ ] Single version policy with good errors (no patch escape hatch)
- [ ] Feature isolation
- [ ] Stdlib handling
- [ ] Incremental resolution
- [ ] Run full test suite

**Exit Criteria**: Can resolve dependency graph with exact versions, detect conflicts
