---
section: "11"
title: TestHost + LspHost
status: not-started
tier: 3
goal: Specialized CompilerHost implementations for testing and LSP
sections:
  - id: "11.1"
    title: Enhanced TestHost
    status: not-started
  - id: "11.2"
    title: LspHost
    status: not-started
  - id: "11.3"
    title: Section Completion Checklist
    status: not-started
---

# Section 11: TestHost + LspHost

**Status:** 📋 Planned
**Goal:** Build specialized `CompilerHost` implementations that enable fully in-memory compilation testing and LSP server integration.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Phase 4
> **Inspired by**: TypeScript's multiple `CompilerHost` implementations
> **Location**: `compiler/oric/src/host.rs` (TestHost), `compiler/ori_lsp/src/host.rs` (LspHost)
> **Depends on**: Section 1 (CompilerHost)

---

## 11.1 Enhanced TestHost

Extend the basic TestHost from Section 1 with advanced testing capabilities.

- [ ] **Implement**: File mutation support
  - [ ] `TestHost::update_file(path, new_content)` — simulate file edit
  - [ ] `TestHost::delete_file(path)` — simulate file deletion
  - [ ] `TestHost::clear()` — reset all files

- [ ] **Implement**: Stdlib injection for testing
  - [ ] `TestHost::with_stdlib()` — load real prelude files into memory
  - [ ] `TestHost::with_minimal_prelude(content)` — inject minimal prelude for fast tests

- [ ] **Implement**: Output assertions
  - [ ] `TestHost::assert_output_contains(needle)` — check program output
  - [ ] `TestHost::assert_diagnostics_contain(needle)` — check error output
  - [ ] `TestHost::diagnostic_output_string() -> String` — get all diagnostic output

- [ ] **Implement**: Integration test harness using TestHost
  - [ ] `fn compile_and_check(source: &str) -> (Session, Outcome<()>)` — one-shot test helper
  - [ ] `fn compile_and_run(source: &str) -> (Session, Outcome<String>)` — eval test helper

- [ ] **Rust Tests**: `compiler/oric/src/host.rs`
  - [ ] `test_test_host_file_mutation` — update file, verify new content
  - [ ] `test_test_host_stdlib_injection` — prelude available to type checker
  - [ ] `test_compile_and_check_helper` — end-to-end check via TestHost
  - [ ] `test_compile_and_run_helper` — end-to-end eval via TestHost
  - [ ] `test_test_host_output_assertions` — verify output capture works

---

## 11.2 LspHost

Editor-buffer-backed host for the language server.

- [ ] **Implement**: `LspHost` struct in `compiler/ori_lsp/src/host.rs`
  - [ ] `buffers: HashMap<PathBuf, String>` — open editor buffers
  - [ ] `disk_fallback: CliHost` — fall back to disk for unopened files
  - [ ] Thread-safe via `RwLock`

- [ ] **Implement**: `CompilerHost` for `LspHost`
  - [ ] `read_file` → check buffers first, fall back to disk
  - [ ] `file_exists` → check buffers, then disk
  - [ ] `write_file` → write to disk (for format-on-save)
  - [ ] `canonicalize` → delegate to CliHost
  - [ ] `diagnostic_output` → null sink (LSP sends diagnostics as notifications)
  - [ ] `program_output` → null sink
  - [ ] `supports_color` → false (LSP doesn't use ANSI)

- [ ] **Implement**: Buffer management methods
  - [ ] `LspHost::open_buffer(path, content)` — editor opened a file
  - [ ] `LspHost::update_buffer(path, content)` — editor changed a file
  - [ ] `LspHost::close_buffer(path)` — editor closed a file
  - [ ] `LspHost::is_buffered(path) -> bool` — check if file is open

- [ ] **Rust Tests**: `compiler/ori_lsp/src/host.rs`
  - [ ] `test_lsp_host_buffer_read` — reads from buffer, not disk
  - [ ] `test_lsp_host_disk_fallback` — falls back to disk for unbuffered files
  - [ ] `test_lsp_host_buffer_lifecycle` — open → update → close
  - [ ] `test_lsp_host_diagnostic_output_silent` — no terminal output

---

## 11.3 Section Completion Checklist

- [ ] Enhanced TestHost enables fully in-memory compilation testing
- [ ] LspHost enables LSP server to share the compiler core
- [ ] Both implementations pass CompilerHost contract tests
- [ ] No regressions: `./test-all.sh` passes

**Exit Criteria:** The compiler can run a full check/run cycle using only `TestHost` (no disk access), and the LSP server can use `LspHost` to compile from editor buffers.
