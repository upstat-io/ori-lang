# Phase 9: Scripts

**Goal**: Project-defined task runner

**Status**: ⬜ Not Started

---

## 9.1 Script Parsing

- [ ] **Implement**: Parse `[scripts]` section
  - [ ] Simple string commands
  - [ ] **Rust Tests**: `ori_pkg/src/scripts/parse.rs`

---

## 9.2 Script Execution

- [ ] **Implement**: `ori run`
  - [ ] List available scripts
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: `ori run <script>`
  - [ ] Execute named script
  - [ ] Run in project root
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: `ori run <script> -- <args>`
  - [ ] Pass additional arguments
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

---

## 9.3 Single-File Mode

- [ ] **Implement**: `ori run file.ori`
  - [ ] Detect .ori extension
  - [ ] Run without project
  - [ ] Stdlib only
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

- [ ] **Implement**: Shebang support
  - [ ] `#!/usr/bin/env ori`
  - [ ] **Rust Tests**: `oric/src/commands/run.rs`

---

## 9.4 Script Stripping

- [ ] **Implement**: Strip scripts on publish
  - [ ] Remove `[scripts]` from published manifest
  - [ ] **Rust Tests**: `ori_pkg/src/archive/create.rs`

---

## 9.5 Phase Completion Checklist

- [ ] Script parsing
- [ ] Script execution
- [ ] Argument passing
- [ ] Single-file mode
- [ ] Shebang support
- [ ] Stripped on publish
- [ ] Run full test suite

**Exit Criteria**: `ori run` works like `npm run`
