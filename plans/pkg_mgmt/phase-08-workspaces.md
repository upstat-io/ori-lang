# Phase 8: Workspaces

**Goal**: Monorepo support with shared dependencies

**Status**: ⬜ Not Started

---

## 8.1 Workspace Configuration

- [ ] **Implement**: Parse `[workspace]` section
  - [ ] `members` array
  - [ ] `exclude` patterns
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/config.rs`

- [ ] **Implement**: Parse `[workspace.dependencies]`
  - [ ] Shared dependency versions
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/deps.rs`

- [ ] **Implement**: Member `workspace = true` syntax
  - [ ] Reference workspace deps
  - [ ] Add features to workspace deps
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/member.rs`

---

## 8.2 Workspace Resolution

- [ ] **Implement**: Single lock file at root
  - [ ] All members share lock
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/lock.rs`

- [ ] **Implement**: Single version per package
  - [ ] Across all members
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/resolve.rs`

---

## 8.3 Workspace Scripts

- [ ] **Implement**: Root scripts available everywhere
  - [ ] `ori run` from any member uses root scripts
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/scripts.rs`

---

## 8.4 Workspace Commands

- [ ] **Implement**: `ori workspace list`
  - [ ] List all members
  - [ ] **Rust Tests**: `oric/src/commands/workspace.rs`

- [ ] **Implement**: `ori workspace add <path>`
  - [ ] Add member to workspace
  - [ ] **Rust Tests**: `oric/src/commands/workspace.rs`

- [ ] **Implement**: `ori build --workspace`
  - [ ] Build all members
  - [ ] **Rust Tests**: `oric/src/commands/build.rs`

- [ ] **Implement**: `ori test --workspace`
  - [ ] Test all members
  - [ ] **Rust Tests**: `oric/src/commands/test.rs`

---

## 8.5 Workspace Publishing

- [ ] **Implement**: Independent member publishing
  - [ ] Each member has own version
  - [ ] Publish individually
  - [ ] **Rust Tests**: `ori_pkg/src/workspace/publish.rs`

---

## 8.6 Phase Completion Checklist

- [ ] Workspace configuration parsing
- [ ] Shared dependencies
- [ ] Single lock file
- [ ] Script inheritance
- [ ] Workspace commands
- [ ] Independent publishing
- [ ] Run full test suite

**Exit Criteria**: Monorepo workflows working
