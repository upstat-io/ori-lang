---
section: "12"
title: Watch Mode
status: not-started
tier: 3
goal: Persistent compilation session with file-watching and Salsa cache reuse
sections:
  - id: "12.1"
    title: File Watcher
    status: not-started
  - id: "12.2"
    title: Persistent Session
    status: not-started
  - id: "12.3"
    title: Watch Commands
    status: not-started
  - id: "12.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 12: Watch Mode

**Status:** 📋 Planned
**Goal:** Implement a persistent compilation session that watches for file changes and recompiles incrementally using Salsa's caching.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Phase 4
> **Inspired by**: TypeScript's `createWatchProgram` with debouncing
> **Location**: `compiler/oric/src/watch.rs`
> **Depends on**: Section 3 (Session), Section 5 (Pipeline)

---

## 12.1 File Watcher

File system change detection with debouncing.

- [ ] **Implement**: `FileWatcher` struct in `compiler/oric/src/watch.rs`
  - [ ] Use `notify` crate for cross-platform file system events
  - [ ] Watch source directories for `.ori` file changes
  - [ ] Debounce changes (coalesce multiple rapid events into one recompilation)
  - [ ] Configurable debounce interval (default: 200ms)

- [ ] **Implement**: Change detection
  - [ ] Track modified files
  - [ ] Track added files
  - [ ] Track deleted files
  - [ ] Ignore non-`.ori` files
  - [ ] Ignore `build/` and `target/` directories

- [ ] **Rust Tests**: `compiler/oric/src/watch.rs`
  - [ ] `test_debounce_coalescing` — rapid changes produce one event
  - [ ] `test_ignore_non_ori_files` — .txt changes ignored
  - [ ] `test_ignore_build_dir` — build directory changes ignored

---

## 12.2 Persistent Session

Reuse Salsa database across recompilations for incremental speedup.

- [ ] **Implement**: `WatchSession` struct
  - [ ] Wraps `Session` with persistent `CompilerDb`
  - [ ] On file change: update `SourceFile` input via `file.set_text(&mut db, new_content)`
  - [ ] Re-run pipeline — Salsa early cutoff skips unchanged phases
  - [ ] Clear diagnostic context between runs

- [ ] **Implement**: `WatchSession::recompile(&mut self, changed_files: &[PathBuf])`
  - [ ] Update changed SourceFile inputs
  - [ ] Re-run pipeline for affected files
  - [ ] Report diagnostics (clear previous output first)
  - [ ] Report compilation time

- [ ] **Rust Tests**: `compiler/oric/src/watch.rs`
  - [ ] `test_watch_session_initial_compile` — first compile works
  - [ ] `test_watch_session_recompile_same` — no changes → instant (Salsa cache)
  - [ ] `test_watch_session_recompile_whitespace` — whitespace edit → early cutoff
  - [ ] `test_watch_session_recompile_change` — real change → recompile

---

## 12.3 Watch Commands

CLI commands for watch mode.

- [ ] **Implement**: `ori check --watch` flag
  - [ ] Start watch session
  - [ ] Run check on startup
  - [ ] Re-check on file changes
  - [ ] Print "Watching for changes..." between runs
  - [ ] Ctrl+C to exit

- [ ] **Implement**: `ori test --watch` flag
  - [ ] Start watch session
  - [ ] Run tests on startup
  - [ ] Re-run tests on file changes
  - [ ] Only re-run affected tests if possible

- [ ] **Design**: `ori watch` standalone command (future consideration)
  - [ ] Would watch and run arbitrary command on change
  - [ ] Document as potential future addition

- [ ] **Rust Tests**: `compiler/oric/src/watch.rs`
  - [ ] `test_watch_check_initial` — initial check runs
  - [ ] `test_watch_check_recompile` — change triggers re-check

---

## 12.4 Section Completion Checklist

- [ ] File watcher detects .ori changes with debouncing
- [ ] Persistent session reuses Salsa cache between runs
- [ ] `ori check --watch` works for interactive development
- [ ] `ori test --watch` works for test-driven development
- [ ] Incremental recompilation is measurably faster than fresh compile
- [ ] No regressions: `./test-all.sh` passes

**Exit Criteria:** `ori check --watch` provides sub-second recompilation for typical edits by reusing Salsa's cached query results across file changes.
