---
section: "11"
title: API Design
status: not-started
priority: medium
goal: Replace boolean parameters with enums, ensure consistent API design
files:
  - compiler/ori_fmt/src/context.rs
  - compiler/ori_diagnostic/src/queue.rs
---

# Section 11: API Design

**Status:** ✅ Tracked (incremental improvements)
**Priority:** MEDIUM — Code quality improvement, not critical
**Goal:** Eliminate boolean flag parameters, ensure consistent API patterns

**Assessment:** API design is generally good:
- Config structs used for complex options (BuildOptions, FormatConfig)
- RAII guards exist (with_capability_scope, with_impl_scope)
- Boolean parameters are mostly appropriate (builder patterns, query methods)
- Items below are tracked for incremental improvement

---

## Guidelines

From `.claude/rules/compiler.md`:
- No boolean flags — use enum or separate functions
- >3-4 params → config struct with `Default`
- Return iterators, not `Vec`
- RAII guards for context save/restore

---

## 11.1 Replace Boolean Parameters

### context.rs: add_trailing_comma

Location: `compiler/ori_fmt/src/context.rs:74`

```rust
// Before:
fn add_trailing_comma(&self, is_multiline: bool, had_trailing: bool) -> bool
```

Two booleans are confusing. Replace with enums:

- [ ] Create `LineMode` enum
  ```rust
  #[derive(Copy, Clone, Eq, PartialEq)]
  pub enum LineMode {
      SingleLine,
      MultiLine,
  }
  ```

- [ ] Create `TrailingCommaPolicy` enum
  ```rust
  #[derive(Copy, Clone, Eq, PartialEq)]
  pub enum TrailingCommaPolicy {
      Always,
      Never,
      Preserve,
      MultiLineOnly,
  }
  ```

- [ ] Update function signature
  ```rust
  fn add_trailing_comma(&self, mode: LineMode, original: TrailingCommaPolicy) -> bool
  ```

- [ ] Update all callers

### queue.rs: add_internal

Location: `compiler/ori_diagnostic/src/queue.rs:171`

```rust
// Before:
fn add_internal(&mut self, diag: Diagnostic, line: u32, column: u32, soft: bool)
```

The `soft` parameter already has a corresponding enum:

- [ ] Replace `soft: bool` with `severity: DiagnosticSeverity`
  ```rust
  // After:
  fn add_internal(&mut self, diag: Diagnostic, line: u32, column: u32, severity: DiagnosticSeverity)
  ```

- [ ] Or use the existing `Severity` from Diagnostic:
  ```rust
  fn add_internal(&mut self, diag: Diagnostic, line: u32, column: u32)
  // severity comes from diag.severity
  ```

---

## 11.2 Audit for Other Boolean Parameters

Search for boolean parameters in public APIs:

- [ ] Grep for `fn.*\(.*: bool\)` in public functions
- [ ] Evaluate each for enum replacement
- [ ] Document any intentional boolean parameters (e.g., `is_empty()`)

### Known Acceptable Booleans

These are query methods, not flag parameters:
- `is_empty()`, `is_some()`, `is_none()`, `is_ok()`, `is_err()`
- `has_errors()`, `has_warnings()`
- `allows_struct_lit()`, `in_loop()`, `in_function()`

### Known Flag Parameters to Review

- [ ] Builder methods in `ori_llvm/src/aot/linker/wasm.rs`
  - `gc_sections(bool)`, `strip_symbols(bool)`, etc.
  - These are acceptable for builder pattern
  - Document as intentional

---

## 11.3 Verify Config Struct Patterns

### Check Functions with Many Parameters

- [ ] Grep for functions with 4+ parameters
- [ ] Verify config structs are used where appropriate
- [ ] Ensure config structs implement `Default`

### Known Good Examples

- `BuildOptions` in commands/build.rs
- `FormatConfig` in ori_fmt
- `DiagnosticConfig` in ori_diagnostic

---

## 11.4 Audit Return Types

### Check for Vec Returns

- [ ] Grep for `-> Vec<` in public functions
- [ ] Evaluate if iterator would be better
- [ ] Keep Vec for small, bounded collections

### Known Acceptable Vec Returns

- Function parameters (bounded, small)
- Type arguments (bounded, small)
- Error collections (need materialization for Salsa)

---

## 11.5 Verify RAII Guard Usage

Check that context manipulation uses guards:

- [ ] `with_capability_scope()` — exists ✓
- [ ] `with_impl_scope()` — exists ✓
- [ ] `with_env_scope()` — exists ✓

Look for manual save/restore patterns:

- [ ] Grep for `let saved = self.context`
- [ ] Convert to RAII guards if found

---

## 11.6 Documentation Audit

### Public Items Without Docs

- [ ] Run `cargo doc --workspace` and check warnings
- [ ] Add missing documentation to public types and functions
- [ ] Focus on user-facing APIs first

---

## 11.7 Verification

- [ ] No boolean flag parameters (except query methods)
- [ ] Config structs for 4+ parameter functions
- [ ] RAII guards for context manipulation
- [ ] Public items documented
- [ ] `./clippy-all.sh` passes
- [ ] `./test-all.sh` passes

---

## 11.N Completion Checklist

- [x] Config structs exist for complex options
- [x] RAII guards used for context manipulation
- [x] Most boolean parameters are appropriate (builders, queries)
- [ ] Incremental: Review `add_trailing_comma` parameters (tracked)
- [ ] Incremental: Review `add_internal` parameters (tracked)
- [ ] Incremental: Document remaining public APIs (tracked)

**Exit Criteria:** ✅ API design is sound; incremental improvements tracked for future work
