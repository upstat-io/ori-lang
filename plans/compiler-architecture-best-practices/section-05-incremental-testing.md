---
section: "05"
title: "Incremental Edit-Sequence Testing"
status: not-started
reviewed: false
goal: "Expand the existing revision test infrastructure into multi-step edit-sequence testing that validates Salsa cache invalidation correctness — new Salsa-touching changes require revision tests"
success_criteria:
  - "ori_test_harness supports multi-step edit-sequence tests (change source → recompile → verify only affected queries re-execute)"
  - "At least 5 edit-sequence tests exist covering: lexer change, parser change, type change, function body change, import change"
  - "tests.md contains the mandatory incremental-testing rule for Salsa-touching changes"
  - "New tests compare incremental results with a clean rebuild for equivalence"
inspired_by:
  - "Rust #[rustc_clean] / #[rustc_dirty] annotations (incremental test suite)"
  - "TypeScript testRunner/unittests/tscWatch/incremental.ts:171-225 (changedFilesSet, fileInfos)"
  - "Zig test/cases/README.md:25-38 (hello.0.zig, hello.1.zig, hello.2.zig revision files)"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Edit-Sequence Test Harness"
    status: not-started
  - id: "05.2"
    title: "Salsa Invalidation Tests"
    status: not-started
  - id: "05.3"
    title: "Rule & Documentation"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Incremental Edit-Sequence Testing

**Status:** Not Started
**Goal:** Build the infrastructure for testing Salsa cache invalidation across multi-revision edit sequences, then add mandatory rules requiring such tests for Salsa-touching changes.

**Success Criteria:**
- [ ] Edit-sequence test harness works end-to-end — satisfies mission criterion "Salsa cache invalidation tested"
- [ ] 5+ edit-sequence tests cover the major invalidation paths — satisfies mission criterion "new Salsa-touching PRs require revision tests"
- [ ] `tests.md` contains the incremental-testing rule — satisfies mission criterion "rules documented"
- [ ] Incremental results match clean-rebuild results for all test cases

**Context:** Research found existing infrastructure:
- `ori_test_harness/src/revision/mod.rs` (88 lines): `RevisionConfig`, `expand_revisions()` for `// @revisions:` directives, `filter_directives_for_revision()`. Used for compile-flags variation (debug/release), NOT for multi-step edit sequences.
- `oric/src/query/tests.rs` (~200+ lines): Salsa query tests using `CompilerDb::new()` and `SourceFile::new()`, testing `line_count`, `lex`, `parse`, `type_check` queries. Uses `salsa::Setter` to mutate source — the foundation exists.
- Missing: no tests that change source and verify which Salsa queries re-execute vs reuse cached results. No comparison of incremental vs clean-rebuild output.

**Reference implementations:**
- **Rust** incremental test suite: `#[rustc_clean(cfg="cfail2")]` / `#[rustc_dirty(cfg="cfail2")]` annotations verifying specific items are/aren't recompiled across revisions
- **Zig** `test/cases/README.md:25-38`: revision files like `hello.0.zig`, `hello.1.zig` representing successive edits
- **TypeScript** `tscWatch/incremental.ts:171-225`: baselines `changedFilesSet`, `fileInfos`, `semanticDiagnosticsPerFile`

**Depends on:** Section 01.

---

## 05.1 Edit-Sequence Test Harness

**File(s):** `compiler/ori_test_harness/src/revision/mod.rs`, `compiler/oric/src/query/tests.rs`

Extend the existing revision infrastructure to support multi-step edit sequences where each step mutates source via Salsa, recompiles, and the test verifies which queries were re-executed.

- [ ] Design the edit-sequence test API:
  ```rust
  // In oric/src/query/tests.rs or a new oric/tests/incremental/ module
  #[test]
  fn test_incremental_function_body_change_preserves_type_cache() {
      let mut db = CompilerDb::new();
      let source_v1 = SourceFile::new(&db, "test.ori".into(), "@add (a: int, b: int) -> int = a + b;".into());
      
      // First compilation — cold
      let result_v1 = db.type_check(source_v1);
      assert!(result_v1.errors.is_empty());
      
      // Edit: change function body only (types unchanged)
      source_v1.set_contents(&mut db).to("@add (a: int, b: int) -> int = a + b + 0;".into());
      
      // Second compilation — incremental
      let result_v2 = db.type_check(source_v1);
      assert!(result_v2.errors.is_empty());
      
      // Verify: type-check result should be equivalent to clean rebuild
      let mut clean_db = CompilerDb::new();
      let clean_source = SourceFile::new(&clean_db, "test.ori".into(), "@add (a: int, b: int) -> int = a + b + 0;".into());
      let clean_result = clean_db.type_check(clean_source);
      
      assert_eq!(result_v2.typed_ir_summary(), clean_result.typed_ir_summary());
  }
  ```

- [ ] Implement a `typed_ir_summary()` or equivalent comparison mechanism that can diff incremental vs clean results (comparing `Idx` values across pools requires structural comparison, not index equality — see `TYPES:TI-4`)

- [ ] Add at least 3 initial edit-sequence tests:
  1. Function body change (types unchanged → type cache reused)
  2. Function signature change (return type changes → type cache invalidated)
  3. Import addition (new dependency → downstream invalidated)

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 05.2 Salsa Invalidation Tests

**File(s):** `compiler/oric/src/query/tests.rs` or `compiler/oric/tests/incremental/`

Write the full set of invalidation tests covering every major invalidation path.

- [ ] Add edit-sequence tests for:
  1. Lexer-only change (whitespace/comment edit → parser cache reused, lex re-runs)
  2. Parser-affecting change (add statement → parse invalidated, type check re-runs)
  3. Type declaration change (add field to struct → type registry invalidated, all dependents re-check)
  4. Trait impl change (modify impl method → trait-dependent callers re-check)
  5. Import change (add/remove import → import-dependent code re-checks)

- [ ] For each test, verify:
  - Incremental result matches clean-rebuild result (equivalence check)
  - No stale diagnostics (errors from v1 don't persist in v2 if the error was fixed)
  - No stale types (type of a changed expression reflects the edit)

- [ ] **Subsection close-out (05.2)** — MANDATORY before starting 05.3:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 05.3 Rule & Documentation

**File(s):** `.claude/rules/tests.md`, `.claude/rules/impl-hygiene.md`

- [ ] Add rule to `tests.md` (new section "Incremental Edit-Sequence Testing"):
  - Changes to Salsa tracked queries, stable IDs, dependency edges, or reparsing MUST add edit-sequence tests
  - Tests MUST exercise at least 2 revisions (edit → recompile → verify)
  - Tests MUST compare incremental results with a clean rebuild
  - Cite Rust/TypeScript/Zig patterns as prior art

- [ ] Update `impl-hygiene.md` §Salsa & Caching (line 583-591) — add cross-reference to the new incremental testing rule in `tests.md`

- [ ] **Subsection close-out (05.3)** — MANDATORY before completing section:
  - [ ] All tasks above are `[x]`
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**
  - [ ] **Run `/sync-claude` to check if code changes affect CLAUDE.md or rules files**

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] All subsections (05.1-05.3) complete
- [ ] Edit-sequence harness works end-to-end
- [ ] 5+ invalidation tests pass
- [ ] Incremental results match clean-rebuild for all tests
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] **`/tpr-review`** — independent dual-source review clean
- [ ] **`/impl-hygiene-review`** — implementation hygiene clean
- [ ] **`/improve-tooling`** — section-close sweep
- [ ] **`/sync-claude`** — section-close doc sync
