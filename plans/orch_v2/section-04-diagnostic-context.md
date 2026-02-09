---
section: "04"
title: DiagnosticContext
status: not-started
tier: 1
goal: Centralized thread-safe diagnostic accumulation replacing per-command boilerplate
sections:
  - id: "4.1"
    title: Core DiagnosticContext
    status: not-started
  - id: "4.2"
    title: Phase-Specific Accumulation
    status: not-started
  - id: "4.3"
    title: Multi-Source Support
    status: not-started
  - id: "4.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 04: DiagnosticContext

**Status:** 📋 Planned
**Goal:** Replace the duplicated error-accumulation pattern across `check.rs`, `run.rs`, and `build.rs` with a centralized, thread-safe diagnostic queue.

> **Proposal**: `docs/ori_lang/proposals/drafts/cli-orchestrator-architecture-proposal.md` § Module 8
> **Inspired by**: Rust's diagnostic handler + Gleam's accumulation pattern
> **Location**: `compiler/oric/src/diagnostic_context.rs`
> **Depends on**: Section 3 (Session)

---

## 4.1 Core DiagnosticContext

Centralized accumulator that collects all diagnostics during compilation.

- [ ] **Implement**: `DiagnosticContext` struct in `compiler/oric/src/diagnostic_context.rs`
  - [ ] `queue: Mutex<DiagnosticQueue>` — accumulated diagnostics
  - [ ] `emitter: Mutex<Box<dyn DiagnosticEmitter + Send>>` — output target
  - [ ] `sources: RwLock<HashMap<PathBuf, String>>` — source text for snippets
  - [ ] `has_errors: AtomicBool` — fast error check without locking

- [ ] **Implement**: `DiagnosticContext::new(output, color_mode)` — create from output writer
  - [ ] Creates `TerminalEmitter` or `JsonEmitter` based on output format config

- [ ] **Implement**: Core methods
  - [ ] `fn has_errors(&self) -> bool` — check if any errors accumulated
  - [ ] `fn add(&self, diagnostic: Diagnostic)` — add a single diagnostic
  - [ ] `fn flush(&self) -> bool` — flush all diagnostics, return has_errors

- [ ] **Rust Tests**: `compiler/oric/src/diagnostic_context.rs`
  - [ ] `test_empty_context` — no errors, flush returns false
  - [ ] `test_add_error` — has_errors returns true after adding error
  - [ ] `test_add_warning` — has_errors stays false for warnings only
  - [ ] `test_flush_order` — errors emitted in order added
  - [ ] `test_deduplication` — duplicate diagnostics deduplicated

---

## 4.2 Phase-Specific Accumulation

Convenience methods for each compilation phase, replacing the per-command boilerplate.

- [ ] **Implement**: `fn add_lex_errors(&self, errors: &[LexError], interner: &StringInterner)`
  - [ ] Convert `LexError` → `LexProblem` → `Diagnostic`
  - [ ] Set `has_errors` if any errors

- [ ] **Implement**: `fn add_parse_errors(&self, output: &ParseOutput)`
  - [ ] Route through `DiagnosticQueue` for deduplication and severity filtering
  - [ ] Set `has_errors` if any hard errors

- [ ] **Implement**: `fn add_type_errors(&self, result: &TypeCheckResult, pool: &Pool, interner: &StringInterner)`
  - [ ] Use `TypeErrorRenderer` for rich error rendering
  - [ ] Set `has_errors` if any type errors

- [ ] **Implement**: `fn add_coverage_errors(&self, untested: &[String])`
  - [ ] Format "function @X has no tests" errors
  - [ ] Set `has_errors` if any untested functions

- [ ] **Rust Tests**: `compiler/oric/src/diagnostic_context.rs`
  - [ ] `test_add_lex_errors_empty` — no-op for empty errors
  - [ ] `test_add_parse_errors_dedup` — soft errors suppressed after hard errors
  - [ ] `test_add_type_errors_rendering` — type errors rendered with rich context
  - [ ] `test_accumulation_across_phases` — lex + parse + type errors all accumulated

---

## 4.3 Multi-Source Support

Enable rich snippet rendering for multi-file compilations.

- [ ] **Implement**: `fn register_source(&self, path: PathBuf, source: String)`
  - [ ] Store source text for use during rendering

- [ ] **Implement**: Enhanced flush with source context
  - [ ] Attach file path to emitter when rendering each diagnostic
  - [ ] Look up source text from registered sources
  - [ ] Fall back to path-only rendering if source not registered

- [ ] **Implement**: `fn into_outcome<T>(&self, value: T) -> Outcome<T>`
  - [ ] Returns `Outcome::Ok(value)` if no errors
  - [ ] Returns `Outcome::TotalFailure` if errors
  - [ ] (Depends on Section 9 for Outcome type; can use Result initially)

- [ ] **Rust Tests**: `compiler/oric/src/diagnostic_context.rs`
  - [ ] `test_register_source` — source text available during rendering
  - [ ] `test_flush_with_source` — snippet rendering includes source
  - [ ] `test_flush_without_source` — graceful fallback

---

## 4.4 Section Completion Checklist

- [ ] `DiagnosticContext` replaces per-command error accumulation pattern
- [ ] All phase-specific methods match current behavior in `check.rs`/`run.rs`
- [ ] Thread-safe (Mutex/RwLock) for future parallel use
- [ ] Deduplication matches existing `DiagnosticQueue` behavior
- [ ] No regressions: `./test-all.sh` passes
- [ ] Public API documented with `///` doc comments
- [ ] Module added to `compiler/oric/src/lib.rs` exports

**Exit Criteria:** The ~100 lines of error-accumulation boilerplate in `check.rs` can be replaced with ~5 calls to `DiagnosticContext` methods.
