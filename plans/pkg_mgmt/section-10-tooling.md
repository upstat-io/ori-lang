---
section: "10"
title: "Tooling"
status: not-started
goal: "Developer experience commands"
depends_on: ["09"]
sections:
  - id: "10.1"
    title: "REPL"
    status: not-started
  - id: "10.2"
    title: "Documentation"
    status: not-started
  - id: "10.3"
    title: "Project Creation"
    status: not-started
  - id: "10.4"
    title: "System Commands"
    status: not-started
  - id: "10.5"
    title: "Cleanup"
    status: not-started
  - id: "10.6"
    title: "Phase Completion Checklist"
    status: not-started
---

# Phase 10: Tooling

**Goal**: Developer experience commands

**Status**: ⬜ Not Started

---

## 10.1 REPL

- [ ] **Implement**: `ori repl`
  - [ ] Interactive Ori shell
  - [ ] Expression evaluation
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

- [ ] **Implement**: REPL history
  - [ ] Persist history
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

- [ ] **Implement**: REPL completion
  - [ ] Tab completion
  - [ ] **Rust Tests**: `oric/src/commands/repl.rs`

---

## 10.2 Documentation

- [ ] **Implement**: `ori docs`
  - [ ] Open Ori documentation
  - [ ] **Rust Tests**: `oric/src/commands/docs.rs`

- [ ] **Implement**: `ori docs @scope/package`
  - [ ] Open package repository
  - [ ] **Rust Tests**: `oric/src/commands/docs.rs`

---

## 10.3 Project Creation

- [ ] **Implement**: `ori new <name>`
  - [ ] Create project directory
  - [ ] Generate oripk.toml
  - [ ] Generate .gitignore
  - [ ] Flags only (non-interactive)
  - [ ] **Rust Tests**: `oric/src/commands/new.rs`

- [ ] **Implement**: `ori new <name> --lib`
  - [ ] Library project
  - [ ] **Rust Tests**: `oric/src/commands/new.rs`

- [ ] **Implement**: `ori init`
  - [ ] Initialize in current directory
  - [ ] **Rust Tests**: `oric/src/commands/init.rs`

---

## 10.4 System Commands

- [ ] **Implement**: `ori self-update`
  - [ ] Download and replace binary
  - [ ] **Rust Tests**: `oric/src/commands/self_update.rs`

- [ ] **Implement**: `ori doctor`
  - [ ] Diagnose setup issues
  - [ ] Check connectivity
  - [ ] Verify cache
  - [ ] **Rust Tests**: `oric/src/commands/doctor.rs`

- [ ] **Implement**: `ori completions <shell>`
  - [ ] Generate shell completions
  - [ ] bash, zsh, fish
  - [ ] **Rust Tests**: `oric/src/commands/completions.rs`

---

## 10.5 Cleanup

- [ ] **Implement**: `ori clean`
  - [ ] Remove build artifacts
  - [ ] Remove .ori/deps
  - [ ] **Rust Tests**: `oric/src/commands/clean.rs`

---

## 10.6 Phase Completion Checklist

- [ ] REPL working
- [ ] Documentation commands
- [ ] Project creation
- [ ] Self-update
- [ ] Doctor diagnostics
- [ ] Shell completions
- [ ] Clean command
- [ ] Run full test suite
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.

**Exit Criteria**: Full developer tooling
