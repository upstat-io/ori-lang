---
section: "02"
title: "File Hygiene"
status: not-started
reviewed: false
goal: "Split all ori_parse files exceeding the 500-line limit into focused submodules with zero behavioral changes."
inspired_by:
  - "Ori .claude/rules/impl-hygiene.md — File Organization (500-line limit, submodule extraction)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Split lib.rs"
    status: not-started
  - id: "02.2"
    title: "Split copier.rs"
    status: not-started
  - id: "02.3"
    title: "Split Remaining Oversized Files"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: File Hygiene

**Status:** Not Started
**Goal:** All `ori_parse` source files are under 500 lines. `lib.rs` (1,326L), `copier.rs` (1,595L), `kind.rs` (1,041L), `cursor.rs` (665L), and `outcome.rs` (697L) are split into focused submodules. All existing tests pass unchanged — purely structural refactoring with zero behavioral changes.

**Context:** Per `.claude/rules/impl-hygiene.md`, source files (excluding tests) must be under 500 lines. Five files in `ori_parse` exceed this limit, with `lib.rs` and `copier.rs` being critical blockers for performance work in Section 04. `lib.rs` mixes parser struct definition, context management, parsing entry points, module parsing, and error collection into a single file. `copier.rs` is a LEAK violation — manual AST node copying that should be centralized.

Splitting these files before performance work ensures that optimization patches touch focused files, not monoliths. It also makes code review tractable.

**Reference implementations:**
- **Ori** `.claude/rules/impl-hygiene.md`: "500-line limit (excl. tests); exceeding = BLOAT finding. Proactive split at ~450 lines."

**Depends on:** None.

---

## 02.1 Split lib.rs (1,326L → ~4 files)

**File(s):** `compiler/ori_parse/src/lib.rs`

`lib.rs` currently contains: `KnownNames` struct + constructor (~20L), `Parser` struct + constructor + capacity helpers (~60L), context management (~60L), error context helpers (~60L), token capture (~40L), speculative parsing/snapshots (~80L), `parse_module()` (~90L), `parse_imports()` (~100L), `dispatch_declaration()` (~190L), `handle_declaration_error` + recovery methods (~60L), `parse_module_incremental()` bridge (~50L), `ParseOutput` impl (~30L), free functions `parse`/`parse_with_metadata`/`parse_incremental` (~80L), and post-parse analysis (~60L).

Per hygiene rules, `lib.rs` should be an **index**: `//!` doc, `mod` declarations, `pub use` re-exports — no function bodies.

- [ ] Extract `Parser` struct definition, constructor, and core methods to `parser/mod.rs`:
  - `Parser<'a>` struct with `cursor`, `arena`, `context`, `known`, `deferred_errors`, `deferred_warnings`
  - `Parser::new()` constructor, `estimated_source_len()`, `take_arena()` (test-only)
  - Context management methods (`with_context`, `without_context`, `has_context`, `allows_struct_lit`)
  - Error context methods (`in_error_context`, `in_error_context_result`)
  - Token capture methods (`with_capture`, `capture_if`)
  - Speculative parsing methods (`snapshot`, `restore`, `try_parse`, `look_ahead`)
  - Utility methods (`check_one_of`, `expect_one_of`)

- [ ] Extract module-level parsing to `parser/module_parse.rs`:
  - `Parser::parse_module()` (~90L)
  - `Parser::parse_imports()` (~100L)
  - `Parser::dispatch_declaration()` (~190L)
  - `Parser::handle_declaration_error()`, recovery methods
  - `Parser::parse_module_incremental()` bridge (~50L)
  - `handle_outcome()` helper

- [ ] Extract `KnownNames` pre-interning to `known_names.rs`:
  - `KnownNames` struct with pre-interned contextual keyword `Name` fields
  - `KnownNames::new(interner)` constructor

- [ ] Item-level parsing dispatch already exists in `grammar/item/mod.rs` with submodules — no extraction needed. Verify `dispatch_declaration()` delegates to these correctly.

- [ ] Wire up new submodules: add `mod parser;` and `mod known_names;` to `lib.rs`. Update all internal `use` paths within `ori_parse` that reference `Parser`, `KnownNames`, or any moved methods. Downstream crates (`oric`, benchmarks) should not be affected since they only import the public API (`parse()`, `ParseOutput`, etc.) which will be re-exported from `lib.rs`.

- [ ] Reduce `lib.rs` to an index:
  - `//!` module doc
  - `mod` declarations for all submodules
  - `pub use` re-exports for `parse()`, `parse_with_metadata()`, `parse_incremental()`, `ParseOutput`, `ParseError`, `ParseOutcome`, `ParseContext`, `ParseWarning`
  - Free functions `parse()`, `parse_with_metadata()`, `parse_incremental()`, `find_token_end_before()` move to `parser/` submodule (they are thin wrappers around `Parser::parse_module()`)
  - `FunctionOrTest` enum, `ParsedAttrs` re-export move to `parser/` or `grammar/`
  - Target: < 80 lines

- [ ] Verify all tests pass in debug: `timeout 150 cargo t -p ori_parse`
- [ ] Verify all spec tests pass: `timeout 150 cargo st`
- [ ] Verify all tests pass in release: `cargo b --release && timeout 150 cargo t -p ori_parse --release`

**TDD ordering:** Write no new tests (purely structural). Existing tests serve as the regression guard. Run the full suite BEFORE any split to establish a green baseline, then after each split to verify zero regressions. Debug AND release must pass.

---

## 02.2 Split copier.rs (1,595L → ~3 files)

**File(s):** `compiler/ori_parse/src/incremental/copier.rs`

The `incremental/` directory already contains: `mod.rs` (dispatch), `copier.rs` (1,595L), `cursor.rs` (navigation), `decl.rs` (declaration collection/categorization), and `tests.rs`. The existing `decl.rs` handles declaration discovery and categorization — NOT copying. The proposed `copy_decl.rs` handles declaration-level deep-copy logic (distinct from `decl.rs`).

`copier.rs` contains a monolithic `deep_copy_expr()` function that matches on every `ExprKind` variant to clone AST nodes with span adjustment for incremental parsing. This is a LEAK violation — manual variant matching that should be centralized or auto-generated.

- [ ] Extract expression copying to `incremental/copy_expr.rs`:
  - `deep_copy_expr()` function and its helpers
  - Variant-specific copy logic for `ExprKind` variants

- [ ] Extract declaration copying to `incremental/copy_decl.rs`:
  - Declaration-level deep-copy logic (functions, types, traits, impls)
  - Note: this is DISTINCT from existing `decl.rs` which handles declaration discovery/categorization, not copying

- [ ] Reduce `copier.rs` to a dispatch hub:
  - Public `copy_module()` API
  - Dispatch to `copy_expr` and `copy_decl` submodules
  - Target: < 200 lines

- [ ] Verify incremental tests pass: `timeout 150 cargo t -p ori_parse -- incremental`

**TDD ordering:** Existing incremental tests (`incremental/tests.rs`, 547 lines) serve as the regression guard.

---

## 02.3 Split Remaining Oversized Files

**File(s):** `compiler/ori_parse/src/error/kind.rs` (1,041L), `compiler/ori_parse/src/cursor/mod.rs` (665L), `compiler/ori_parse/src/outcome/mod.rs` (697L), `compiler/ori_parse/src/grammar/expr/patterns/match_patterns.rs` (595L), `compiler/ori_parse/src/grammar/item/function/mod.rs` (532L)

These files are over 500 lines but less critical than `lib.rs` and `copier.rs`. Split them to stay within hygiene limits.

- [ ] **`error/kind.rs`** (1,041L): Extract context enums (`ErrorContext`, `ParseExpected`, etc.) to `error/context_kinds.rs`. Keep `ParseErrorKind` enum in `kind.rs`. Target: both files < 500L.
  - While splitting, remove all decorative `// ===` banners (e.g., `// === Token-level errors ===`, `// === Expression errors ===`, etc. -- ~13 instances). Replace with plain `// Section name` comments per hygiene rules.

- [ ] **`cursor/mod.rs`** (665L): Extract error-building methods (`make_expect_error`, `make_expect_ident_error`, `make_expect_ident_or_keyword_error`) to `cursor/errors.rs`. Keep navigation and token access methods in `mod.rs`. Target: `mod.rs` < 500L.

- [ ] **`outcome/mod.rs`** (697L): Extract synchronization/recovery helper methods to `outcome/recovery_helpers.rs`. Keep `ParseOutcome<T>` enum and core `From`/conversion impls in `mod.rs`. Target: `mod.rs` < 500L.
  - While splitting, remove all decorative `// ===` banners (`// === Constructors ===`, `// === Predicates ===`, `// === Transformations ===`, `// === Conversions ===`, `// === Backtracking Macros ===` -- 5 instances at lines 104, 142, 188, 401, 409). Replace with plain `// Section name` comments per hygiene rules.

- [ ] **`grammar/expr/patterns/match_patterns.rs`** (595L): Extract complex match arm parsing helpers to `grammar/expr/patterns/match_helpers.rs`. Keep the primary `parse_match_arm` and `parse_match_pattern` dispatch in `match_patterns.rs`. Target: both files < 500L.

- [ ] **`grammar/item/function/mod.rs`** (532L): Extract function clause parsing or contract parsing to a sibling submodule (e.g., `function/clauses.rs` or `function/contracts.rs`). Target: `mod.rs` < 500L.

- [ ] Verify all tests pass after each split: `timeout 150 cargo t -p ori_parse`

**Codebase scan findings — files touched by later sections that also exceed 500 lines:**

The following files are modified by Sections 03-05 and also exceed the 500-line limit. They are NOT in `ori_parse` and thus not part of this section's primary scope, but the plan should not touch bloated files without at least noting the issue. These are recorded here for awareness; splitting them is optional in this plan but MANDATORY if the plan's changes would push them further over the limit.

- `compiler/ori_ir/src/arena/mod.rs` (530L, ~528L production — `#[cfg(test)] mod tests;` at L529): Touched by Sections 03.1 and 04.1/04.2. 28 lines over limit. If adding code, split first.
- `compiler/ori_ir/src/ast/expr.rs` (510L, all production): Touched by Section 04.1 (`Expr::new` inline). 10 lines over limit. If adding code, split first.
- `compiler/oric/src/query/mod.rs` (582L, ~580L production — `#[cfg(test)] mod tests;` at L78): Touched by Section 05. 80 lines over limit. If adding incremental parsing logic, split first (e.g., extract `canonicalize_cached()` and related functions to `query/canonicalize.rs`).

**TDD ordering:** Existing tests in `error/tests.rs` (899L), `cursor/tests.rs` (358L), and `outcome/tests.rs` (395L) serve as regression guards.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `lib.rs` reduced to < 80 lines (index only)
- [ ] `copier.rs` split into 3 files, each < 500 lines
- [ ] `error/kind.rs` split, both parts < 500 lines
- [ ] `cursor/mod.rs` reduced to < 500 lines
- [ ] `outcome/mod.rs` reduced to < 500 lines
- [ ] `grammar/expr/patterns/match_patterns.rs` reduced to < 500 lines
- [ ] `grammar/item/function/mod.rs` reduced to < 500 lines
- [ ] No file in `ori_parse/src/` (excluding tests) exceeds 500 lines
- [ ] All decorative `// ===` banners removed from `outcome/mod.rs` (5 instances) and `error/kind.rs` (~13 instances), replaced with plain `// Section name` comments
- [ ] All `ori_parse` tests pass: `timeout 150 cargo t -p ori_parse`
- [ ] All spec tests pass: `timeout 150 cargo st`
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] Debug AND release builds pass: `timeout 150 cargo t -p ori_parse --release`
- [ ] Zero net behavioral changes (diffs are purely `mod`/`use` additions)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `find compiler/ori_parse/src -name '*.rs' ! -path '*/tests*' ! -name 'tests.rs' | xargs wc -l | sort -rn | head -10` shows no file exceeding 500 lines. All test suites green.
