---
section: "03"
title: Session
status: not-started
tier: 0
goal: Central struct connecting database, host, config, and diagnostics
sections:
  - id: "3.1"
    title: Session Struct
    status: not-started
  - id: "3.2"
    title: Source Loading
    status: not-started
  - id: "3.3"
    title: Session Builder
    status: not-started
  - id: "3.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 03: Session

**Status:** 📋 Planned
**Goal:** Create a central `Session` struct that connects the Salsa database, I/O host, validated configuration, and diagnostic context into a coherent compilation unit.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 3
> **Inspired by**: Rust's `Session` struct (spine of the compiler)
> **Location**: `compiler/oric/src/session.rs`
> **Depends on**: Section 1 (CompilerHost), Section 2 (CompilerConfig)

---

## 3.1 Session Struct

The central struct that replaces ad-hoc `CompilerDb::new()` in every command handler.

- [ ] **Implement**: `Session` struct in `compiler/oric/src/session.rs`
  - [ ] `db: CompilerDb` — Salsa incremental database
  - [ ] `host: Box<dyn CompilerHost>` — I/O abstraction
  - [ ] `config: CompilerConfig` — validated configuration
  - [ ] `context: CompilerContext` — registries (patterns, etc.)

- [ ] **Implement**: `Session::new(host, config)` — staged initialization
  - [ ] Stage 1: Host (I/O ready)
  - [ ] Stage 2: Config (already validated)
  - [ ] Stage 3: Database (Salsa storage created)
  - [ ] Stage 4: CompilerContext (registries initialized)

- [ ] **Implement**: Accessor methods
  - [ ] `fn db(&self) -> &CompilerDb`
  - [ ] `fn db_mut(&mut self) -> &mut CompilerDb`
  - [ ] `fn host(&self) -> &dyn CompilerHost`
  - [ ] `fn config(&self) -> &CompilerConfig`
  - [ ] `fn context(&self) -> &CompilerContext`
  - [ ] `fn interner(&self) -> &StringInterner`

- [ ] **Rust Tests**: `compiler/oric/src/session.rs`
  - [ ] `test_session_creation` — create with TestHost
  - [ ] `test_session_accessors` — all fields accessible
  - [ ] `test_session_db_is_functional` — can create SourceFile inputs

---

## 3.2 Source Loading

Unified file loading that wires host → database.

- [ ] **Implement**: `Session::load_source(&self, path: &Path) -> Option<SourceFile>`
  - [ ] Read file through `self.host.read_file(path)`
  - [ ] Canonicalize through `self.host.canonicalize(path)`
  - [ ] Create `SourceFile::new(&self.db, canonical, content)`
  - [ ] Set durability HIGH if `self.host.is_stdlib(path)`
  - [ ] Return None if file not found

- [ ] **Implement**: `Session::load_source_with_content(&self, path: PathBuf, content: String) -> SourceFile`
  - [ ] For cases where content is already in memory (e.g., LSP buffers)
  - [ ] Skip host read, create SourceFile directly

- [ ] **Rust Tests**: `compiler/oric/src/session.rs`
  - [ ] `test_load_source_from_test_host` — load in-memory file
  - [ ] `test_load_source_not_found` — returns None
  - [ ] `test_load_source_with_content` — direct content loading
  - [ ] `test_load_source_salsa_tracking` — verify SourceFile is Salsa input
  - [ ] `test_load_source_deduplication` — same path returns same SourceFile

---

## 3.3 Session Builder

Optional builder for complex session configuration (testing, LSP).

- [ ] **Implement**: `SessionBuilder` for advanced use cases
  - [ ] `SessionBuilder::new(config: CompilerConfig)`
  - [ ] `.host(Box<dyn CompilerHost>)` — override host (default: CliHost)
  - [ ] `.interner(SharedInterner)` — share interner across sessions
  - [ ] `.context(CompilerContext)` — override context
  - [ ] `.build() -> Session`

- [ ] **Rust Tests**: `compiler/oric/src/session.rs`
  - [ ] `test_session_builder_defaults` — builder with defaults
  - [ ] `test_session_builder_custom_host` — inject TestHost
  - [ ] `test_session_builder_shared_interner` — shared interner across sessions

---

## 3.4 Section Completion Checklist

- [ ] `Session::new()` creates functional session with all components
- [ ] `Session::load_source()` loads files through host
- [ ] `SessionBuilder` supports testing and LSP use cases
- [ ] Session works with both `CliHost` and `TestHost`
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments
- [ ] Module added to `compiler/oric/src/lib.rs` exports

**Exit Criteria:** A `Session` can be created from a `CompilerHost` + `CompilerConfig`, load source files, and provide access to the Salsa database — all without touching the file system directly.
