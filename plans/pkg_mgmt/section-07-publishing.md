# Phase 7: Publishing

**Goal**: Package publishing workflow

**Status**: ⬜ Not Started

---

## 7.1 ori login

- [ ] **Implement**: `ori login`
  - [ ] Prompt for token
  - [ ] Store in environment
  - [ ] **Rust Tests**: `oric/src/commands/login.rs`

---

## 7.2 Pre-publish Validation

- [ ] **Implement**: Validate publishable
  - [ ] No git dependencies
  - [ ] No path dependencies
  - [ ] Tests must pass
  - [ ] Description required
  - [ ] **Rust Tests**: `ori_pkg/src/publish/validate.rs`

- [ ] **Implement**: Version immutability check
  - [ ] Error if version exists
  - [ ] **Rust Tests**: `ori_pkg/src/publish/validate.rs`

---

## 7.3 Archive Creation

- [ ] **Implement**: Create package archive
  - [ ] Strip `[scripts]` section
  - [ ] Include oripk.lock (checksums for verification)
  - [ ] Compute checksums
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

- [ ] **Implement**: Respect include/exclude
  - [ ] From `[publish]` section
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

---

## 7.4 ori publish

- [ ] **Implement**: `ori publish`
  - [ ] Run validation
  - [ ] Create archive
  - [ ] Upload to registry
  - [ ] **Rust Tests**: `oric/src/commands/publish.rs`

- [ ] **Implement**: `ori publish --dry-run`
  - [ ] Validate without uploading
  - [ ] **Rust Tests**: `oric/src/commands/publish.rs`

---

## 7.5 Version Management

- [ ] **Implement**: `ori yank <version>`
  - [ ] Mark version as yanked
  - [ ] **Rust Tests**: `oric/src/commands/yank.rs`

- [ ] **Implement**: `ori unyank <version>`
  - [ ] Restore yanked version
  - [ ] **Rust Tests**: `oric/src/commands/unyank.rs`

- [ ] **Implement**: `ori deprecate <version> <message>`
  - [ ] Add deprecation warning
  - [ ] **Rust Tests**: `oric/src/commands/deprecate.rs`

---

## 7.6 Version Bumping

- [ ] **Implement**: `ori version patch`
  - [ ] Bump 1.0.0 → 1.0.1
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

- [ ] **Implement**: `ori version minor`
  - [ ] Bump 1.0.0 → 1.1.0
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

- [ ] **Implement**: `ori version major`
  - [ ] Bump 1.0.0 → 2.0.0
  - [ ] **Rust Tests**: `oric/src/commands/version.rs`

---

## 7.7 Phase Completion Checklist

- [ ] Login working
- [ ] Pre-publish validation
- [ ] Archive creation
- [ ] Publish with dry-run
- [ ] Yank/unyank/deprecate
- [ ] Version bumping
- [ ] Run full test suite

**Exit Criteria**: Can publish packages to registry
