---
section: "01"
title: CompilerHost Trait
status: not-started
tier: 0
goal: Abstract all I/O behind a trait for CLI/LSP/test/WASM portability
sections:
  - id: "1.1"
    title: Core Trait Definition
    status: not-started
  - id: "1.2"
    title: CliHost Implementation
    status: not-started
  - id: "1.3"
    title: TestHost Implementation
    status: not-started
  - id: "1.4"
    title: Database Integration
    status: not-started
  - id: "1.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 01: CompilerHost Trait

**Status:** 📋 Planned
**Goal:** Abstract all file system and output operations behind a `CompilerHost` trait, enabling the same compiler core to serve CLI, LSP, testing, and WASM contexts.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 1
> **Inspired by**: TypeScript's `CompilerHost` interface
> **Location**: `compiler/oric/src/host.rs`

---

## 1.1 Core Trait Definition

Define the `CompilerHost` trait with all I/O operations the compiler needs.

- [ ] **Implement**: `CompilerHost` trait in `compiler/oric/src/host.rs`
  - [ ] File operations: `read_file`, `file_exists`, `write_file`
  - [ ] Path operations: `resolve_path`, `canonicalize`
  - [ ] Directory operations: `glob_files`, `current_dir`
  - [ ] Stdlib: `stdlib_path`, `is_stdlib`
  - [ ] Output streams: `diagnostic_output`, `program_output`, `supports_color`
  - [ ] **Rust Tests**: `compiler/oric/src/host.rs` — trait is object-safe (`dyn CompilerHost` compiles)

- [ ] **Design decision**: `dyn CompilerHost` (trait object), not generic `H: CompilerHost`
  - [ ] Rationale documented in module doc: vtable cost negligible vs I/O cost; avoids generic infection

---

## 1.2 CliHost Implementation

Default host for command-line usage — wraps real file system operations.

- [ ] **Implement**: `CliHost` struct in `compiler/oric/src/host.rs`
  - [ ] `CliHost::new()` — detect CWD and stdlib path
  - [ ] `read_file` → `std::fs::read_to_string`
  - [ ] `file_exists` → `std::path::Path::exists`
  - [ ] `write_file` → `std::fs::write`
  - [ ] `resolve_path` → `Path::join` with CWD
  - [ ] `canonicalize` → `std::fs::canonicalize`
  - [ ] `glob_files` → walk directory with pattern matching
  - [ ] `stdlib_path` → detect from binary location or env var
  - [ ] `is_stdlib` → check path prefix against `stdlib_path()`
  - [ ] `diagnostic_output` → `Box::new(std::io::stderr())`
  - [ ] `program_output` → `Box::new(std::io::stdout())`
  - [ ] `supports_color` → `std::io::IsTerminal::is_terminal(&std::io::stderr())`
  - [ ] **Rust Tests**: `compiler/oric/src/host.rs`
    - [ ] `test_cli_host_read_file` — reads real temp file
    - [ ] `test_cli_host_file_exists` — true for existing, false for missing
    - [ ] `test_cli_host_resolve_path` — resolves relative to CWD
    - [ ] `test_cli_host_stdlib_detection` — finds `library/std/`

---

## 1.3 TestHost Implementation

In-memory file system for unit and integration testing.

- [ ] **Implement**: `TestHost` struct in `compiler/oric/src/host.rs`
  - [ ] `files: HashMap<PathBuf, String>` — in-memory file store
  - [ ] `output: Arc<Mutex<Vec<u8>>>` — captured output
  - [ ] `TestHost::new()` — empty file system
  - [ ] `TestHost::with_file(path, content)` — builder pattern for adding files
  - [ ] `TestHost::captured_output()` — read what was written to diagnostic/program output
  - [ ] All trait methods operate on in-memory `files` map
  - [ ] `canonicalize` returns path as-is (no real FS)
  - [ ] `is_stdlib` returns `false` by default
  - [ ] `supports_color` returns `false`
  - [ ] **Rust Tests**: `compiler/oric/src/host.rs`
    - [ ] `test_test_host_read_file` — returns file content from map
    - [ ] `test_test_host_file_not_found` — returns None for missing files
    - [ ] `test_test_host_write_and_read` — round-trip file content
    - [ ] `test_test_host_captured_output` — verify output capture works
    - [ ] `test_test_host_builder` — chain `with_file` calls

---

## 1.4 Database Integration

Wire `CompilerHost` into `CompilerDb::load_file()` to replace direct `fs::read_to_string`.

- [ ] **Implement**: Add `host` field to `Db` trait or thread host through `load_file`
  - [ ] Option A: Add `fn host(&self) -> &dyn CompilerHost` to `Db` trait
  - [ ] Option B: Pass host as parameter to methods that need I/O
  - [ ] **Decision**: Document chosen approach with rationale
  - [ ] `CompilerDb::load_file` delegates to `self.host().read_file()`
  - [ ] `is_stdlib_path()` delegates to `self.host().is_stdlib()`
  - [ ] **Rust Tests**: Verify `load_file` works with `TestHost`
    - [ ] `test_load_file_via_test_host` — load file without real FS
    - [ ] `test_load_file_stdlib_detection` — stdlib durability still works

- [ ] **Migrate**: Replace `commands::read_file()` usage with host-based loading
  - [ ] This is a preparatory step — actual command migration is Section 10

---

## 1.5 Section Completion Checklist

- [ ] `CompilerHost` trait compiles and is object-safe
- [ ] `CliHost` passes all tests with real file system
- [ ] `TestHost` passes all tests with in-memory files
- [ ] `CompilerDb::load_file` can use either host
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments
- [ ] Module added to `compiler/oric/src/lib.rs` exports

**Exit Criteria:** The compiler can load files through either `CliHost` (real FS) or `TestHost` (in-memory) with identical compilation results.
