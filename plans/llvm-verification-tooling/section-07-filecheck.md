---
section: "07"
title: "FileCheck-Style IR Pattern Matching"
status: not-started
reviewed: false
goal: "Build a FileCheck-style IR assertion framework in tests/codegen/ with >=30 directive-based tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns — using the shared harness from Section 02 for directive parsing, bless mode, and revision support"
success_criteria:
  - "tests/codegen/ contains >=30 FileCheck-style tests with // CHECK: directives"
  - "Tests use .matches mode (order-independent substring matching) as default"
  - "Revision system supports debug/release/no-repr-opt configurations per test"
  - "Bless mode (--bless) updates expected baselines for .ll artifacts"
  - "Tests run via cargo test -p ori_llvm --test codegen_checks within 150s timeout"
  - "At least 6 tests per category: RC emission, COW patterns, closure codegen, ABI, iterator patterns"
inspired_by:
  - "Rust compiletest codegen tests (tests/codegen/) — CHECK directives in Ori source files"
  - "Zig addCheckFile (test/src/LlvmIr.zig:45-73) — .matches mode for order-independent matching"
  - "LLVM FileCheck (llvm-project/llvm/utils/FileCheck/) — CHECK/CHECK-NOT/CHECK-LABEL/CHECK-NEXT"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "FileCheck Test Runner"
    status: not-started
  - id: "07.2"
    title: "RC Emission Tests"
    status: not-started
  - id: "07.3"
    title: "COW Pattern Tests"
    status: not-started
  - id: "07.4"
    title: "Closure Codegen Tests"
    status: not-started
  - id: "07.5"
    title: "ABI and Iterator Pattern Tests"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: FileCheck-Style IR Pattern Matching

**Status:** Not Started
**Goal:** Build a FileCheck-style IR assertion framework in `tests/codegen/` with at least 30 directive-based tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns. Tests use `// CHECK:` directives embedded in Ori source files, compiled through the full LLVM pipeline, with the resulting IR matched against the directives. The shared harness from Section 02 provides directive parsing, bless mode, and revision support. The default matching mode is `.matches` (order-independent substring search, following Zig's pattern), which is more robust against IR reordering by LLVM's optimizer than strict sequential matching.

**Success Criteria:**

- [ ] 30+ FileCheck tests in `tests/codegen/` — satisfies mission criterion: "FileCheck IR assertions"
- [ ] `.matches` mode as default — satisfies mission criterion: "FileCheck IR assertions"
- [ ] Revision support for debug/release/no-repr-opt — satisfies mission criterion: "FileCheck revision support"
- [ ] Bless mode updates baselines — satisfies mission criterion: "FileCheck IR assertions"
- [ ] 6+ tests per category — satisfies mission criterion: "comprehensive IR coverage"

**Context:** Behavioral tests (Ori spec tests) verify that programs produce correct output but cannot catch codegen quality regressions. A program that leaks memory due to missing `RcDec` still produces correct stdout. A COW fast path that silently degrades to always-copy still produces correct output. An ABI mismatch that happens to work due to LLVM's optimizer still produces correct output — until it doesn't. FileCheck-style tests pin the LLVM IR patterns that indicate correct codegen, catching regressions at the IR level before they manifest as runtime bugs.

**FastISel limitation:** Revisions testing debug vs release can detect behavioral differences but cannot catch FastISel bugs at the IR level — FastISel affects instruction selection (which LLVM instructions are emitted), not the IR. The IR patterns are identical between debug and release; the difference manifests only in the generated machine code. For FastISel-sensitive patterns (struct loads >16 bytes, aggregate spills), use AOT integration tests (`ori_llvm/tests/aot/`) that execute the generated binary, not IR-level checks.

**Reference implementations:**
- **Rust** `tests/codegen/`: CHECK directives in Rust source files, compiled with `--emit=llvm-ir`, matched against LLVM IR output. Revisions for optimization levels.
- **Zig** `test/src/LlvmIr.zig` (lines 45-73): `.matches` mode — order-independent substring search. More robust against IR reordering than LLVM's FileCheck sequential matching.
- **LLVM** `llvm-project/llvm/utils/FileCheck/`: The reference FileCheck tool with CHECK/CHECK-NOT/CHECK-LABEL/CHECK-NEXT directives.

**Depends on:** Section 02 (shared harness provides directive parsing, artifact naming, bless mode, revision expansion).

---

## 07.1 FileCheck Test Runner

**File(s):** `compiler/ori_llvm/tests/codegen_checks.rs` (new), `compiler/ori_test_harness/src/check.rs` (new)

Build the test runner that: (1) reads `.ori` test files from `tests/codegen/`, (2) compiles each through the full LLVM pipeline to produce LLVM IR, (3) parses `// CHECK:` directives from the source, (4) matches directives against the LLVM IR output.

- [ ] Add `check.rs` module to `ori_test_harness` with the matching engine:
  ```rust
  //! FileCheck-style matching engine.
  //!
  //! Supports two modes:
  //! - `.matches` (default): order-independent substring matching.
  //!   Every CHECK pattern must appear somewhere in the IR, but order
  //!   between CHECK lines is not enforced. CHECK-NOT patterns must
  //!   NOT appear anywhere.
  //! - `.exact`: sequential matching (traditional FileCheck behavior).
  //!   CHECK patterns must appear in the order specified. CHECK-NEXT
  //!   requires the match to be on the immediately following line.

  pub enum CheckMode {
      /// Order-independent substring matching (default, Zig pattern).
      Matches,
      /// Sequential matching (traditional FileCheck).
      Exact,
  }

  pub struct CheckResult {
      pub passed: bool,
      pub failures: Vec<CheckFailure>,
  }

  pub enum CheckFailure {
      /// A CHECK: pattern was not found in the IR.
      PatternNotFound { line: usize, pattern: String },
      /// A CHECK-NOT: pattern was unexpectedly found.
      NegativePatternFound { line: usize, pattern: String, found_at: usize },
      /// A CHECK-LABEL: pattern was not found (section anchor missing).
      LabelNotFound { line: usize, pattern: String },
      /// A CHECK-NEXT: pattern was not on the next line after the previous match.
      NextNotAdjacent { line: usize, pattern: String, expected_line: usize, actual_line: usize },
  }

  pub fn run_checks(
      ir: &str,
      directives: &[DirectiveLine],
      mode: CheckMode,
  ) -> CheckResult {
      // Implementation
  }
  ```

- [ ] Create `compiler/ori_llvm/tests/codegen_checks.rs` as an integration test that discovers and runs all `.ori` files in `tests/codegen/`:
  ```rust
  //! FileCheck-style codegen tests.
  //!
  //! Each `.ori` file in `tests/codegen/` is compiled through the full LLVM
  //! pipeline, and `// CHECK:` directives in the source are matched against
  //! the emitted LLVM IR.

  use std::path::PathBuf;

  fn discover_test_files() -> Vec<PathBuf> {
      // Walk tests/codegen/ recursively, collect .ori files
  }

  fn compile_to_ir(path: &Path) -> String {
      // Use the compiler's LLVM pipeline to emit IR for the test file.
      // Equivalent to: ORI_DUMP_AFTER_LLVM=1 ori build test.ori
      // Capture the LLVM IR output as a string.
  }

  #[test]
  fn run_all_codegen_checks() {
      let test_files = discover_test_files();
      assert!(!test_files.is_empty(), "no codegen test files found in tests/codegen/");

      let mut failures = Vec::new();
      for file in &test_files {
          // Parse directives, expand revisions, compile, check
          // Collect failures
      }

      if !failures.is_empty() {
          panic!("{} codegen check(s) failed:\n{}", failures.len(),
              failures.join("\n"));
      }
  }
  ```

- [ ] Wire revision expansion: when a test has `// @revisions: debug release`, run the compilation and check for each revision with the appropriate flags. Store per-revision `.ll` artifacts.

- [ ] Wire bless mode: when `ORI_BLESS=1` is set, instead of matching CHECK directives, dump the IR to the expected baseline file. (Bless mode is primarily for snapshot-style tests; for CHECK-directive tests, bless mode could regenerate the CHECK lines — but for v1, bless mode updates `.ll` baselines for visual inspection only.)

- [ ] Add tests:
  - `test_check_matches_mode_finds_substring`
  - `test_check_not_fails_on_present_pattern`
  - `test_check_label_anchors_section`
  - `test_check_next_requires_adjacent_line`
  - `test_revision_expansion_compiles_both_configs`

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 07.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type.

---

## 07.2 RC Emission Tests

**File(s):** `tests/codegen/rc/` (new test files)

Write FileCheck tests that pin RC emission patterns. These tests verify that the AIMS pipeline + LLVM codegen emits the expected RC operations for common patterns.

- [ ] `tests/codegen/rc/basic_inc_dec.ori` — basic RC increment/decrement for shared values:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_main
  // CHECK: call void @ori_rc_inc
  // CHECK: call void @ori_rc_dec

  @main () -> void = {
      let xs = [1, 2, 3]
      let ys = xs  // shared reference — should RC inc
      print(msg: ys.len().to_str())
  }
  ```

- [ ] `tests/codegen/rc/elision_unique_owner.ori` — RC elision for unique owners (no inc/dec when linear):
  ```ori
  // @test: codegen
  // Unique owner consumed linearly — no RC operations expected
  // CHECK-LABEL: define {{.*}} @_ori_main
  // CHECK-NOT: call void @ori_rc_inc

  @main () -> void = {
      let xs = [1, 2, 3]
      print(msg: xs.len().to_str())
  }
  ```

- [ ] `tests/codegen/rc/param_borrowed.ori` — borrowed parameters have no RC ops:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_helper
  // CHECK-NOT: call void @ori_rc_inc
  // CHECK-NOT: call void @ori_rc_dec

  @helper (xs: [int]) -> int = xs.len()

  @main () -> void = {
      let xs = [1, 2, 3]
      print(msg: helper(xs:).to_str())
  }
  ```

- [ ] `tests/codegen/rc/param_owned_consumed.ori` — owned parameter consumed by callee has RC dec:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_consumer
  // CHECK: call void @ori_rc_dec

  @consumer (xs: [int]) -> void = {
      let a = xs
      let b = xs  // second use — RC inc needed, then dec at end
      print(msg: a.len().to_str())
      print(msg: b.len().to_str())
  }

  @main () -> void = consumer(xs: [1, 2, 3])
  ```

- [ ] `tests/codegen/rc/loop_inc_dec.ori` — RC in loops (the highest-risk pattern):
  ```ori
  // @test: codegen
  // @revisions: debug release
  // CHECK-LABEL: define {{.*}} @_ori_main

  @main () -> void = {
      let xs = ["hello", "world"]
      for x in xs do print(msg: x)
  }
  ```

- [ ] `tests/codegen/rc/nested_struct.ori` — RC for nested struct fields:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_main

  type Wrapper = { inner: [int] }

  @main () -> void = {
      let w = Wrapper { inner: [1, 2, 3] }
      let w2 = w  // sharing nested struct
      print(msg: w2.inner.len().to_str())
  }
  ```

- [ ] **TPR checkpoint** — `/tpr-review` covering 07.1–07.2 implementation work

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 07.1's close-out, scoped to 07.2's debugging journey. Commit improvements separately.

---

## 07.3 COW Pattern Tests

**File(s):** `tests/codegen/cow/` (new test files)

Write FileCheck tests that pin COW (Copy-On-Write) codegen patterns. COW is one of the highest-risk codegen areas — a silent degradation from fast-path to always-copy is invisible to behavioral tests.

- [ ] `tests/codegen/cow/is_shared_check.ori` — COW emits `IsShared` check before mutation:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_mutator
  // CHECK: load {{.*}} ; refcount
  // CHECK: icmp {{.*}} ; is_shared check

  @mutator (xs: [int]) -> [int] = {
      let mut result = xs
      result[0] = 42
      result
  }

  @main () -> void = {
      let xs = [1, 2, 3]
      let ys = mutator(xs:)
      print(msg: ys[0].to_str())
  }
  ```

- [ ] `tests/codegen/cow/unique_no_copy.ori` — unique owner mutation skips copy:
  ```ori
  // @test: codegen
  // When the value is provably unique, COW should take the fast path
  // CHECK-LABEL: define {{.*}} @_ori_main
  // CHECK-NOT: call {{.*}}memcpy

  @main () -> void = {
      let mut xs = [1, 2, 3]
      xs[0] = 42
      print(msg: xs[0].to_str())
  }
  ```

- [ ] `tests/codegen/cow/shared_triggers_copy.ori` — shared value mutation triggers copy:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3]
      let ys = xs  // creates sharing
      let mut zs = ys
      zs[0] = 42  // must copy because shared
      print(msg: zs[0].to_str())
      print(msg: xs[0].to_str())
  }
  ```

- [ ] `tests/codegen/cow/struct_field_mutation.ori` — struct field mutation COW pattern:
  ```ori
  // @test: codegen

  type Point = { x: int, y: int }

  @main () -> void = {
      let mut p = Point { x: 1, y: 2 }
      p.x = 10
      print(msg: p.x.to_str())
  }
  ```

- [ ] `tests/codegen/cow/map_update.ori` — map COW pattern:
  ```ori
  // @test: codegen

  @main () -> void = {
      let mut m = {"a": 1, "b": 2}
      m["a"] = 42
      print(msg: m["a"].to_str())
  }
  ```

- [ ] `tests/codegen/cow/drop_hints.ori` — drop hints emitted correctly for COW paths:
  ```ori
  // @test: codegen

  @main () -> void = {
      let xs = [1, 2, 3]
      let ys = xs
      print(msg: ys.len().to_str())
      // xs should be dropped with correct drop hint
  }
  ```

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 07.4 Closure Codegen Tests

**File(s):** `tests/codegen/closures/` (new test files)

Write FileCheck tests for closure codegen — capture patterns, closure environment layout, and RC for captured values.

**Note:** Capture stack spill patterns are sensitive to FastISel; ensure tests focus on IR-level environment layout, not machine-level spills.

- [ ] `tests/codegen/closures/capture_by_value.ori` — closure captures value, RC incremented:
  ```ori
  // @test: codegen
  // CHECK-LABEL: define {{.*}} @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3]
      let f = () -> int = xs.len()
      print(msg: f().to_str())
  }
  ```

- [ ] `tests/codegen/closures/closure_env_layout.ori` — closure environment struct emitted:
  ```ori
  // @test: codegen

  @main () -> void = {
      let a = "hello"
      let b = 42
      let f = () -> str = `{a} {b}`
      print(msg: f())
  }
  ```

- [ ] `tests/codegen/closures/closure_as_argument.ori` — closure passed as function argument:
  ```ori
  // @test: codegen

  @apply (f: () -> int) -> int = f()

  @main () -> void = {
      let x = 10
      let result = apply(f: () -> int = x * 2)
      print(msg: result.to_str())
  }
  ```

- [ ] `tests/codegen/closures/nested_closures.ori` — nested closures with RC chain:
  ```ori
  // @test: codegen

  @main () -> void = {
      let xs = [1, 2, 3]
      let f = () -> (() -> int) = {
          () -> int = xs.len()
      }
      let g = f()
      print(msg: g().to_str())
  }
  ```

- [ ] `tests/codegen/closures/closure_in_loop.ori` — closure created inside loop:
  ```ori
  // @test: codegen

  @main () -> void = {
      let xs = [1, 2, 3]
      for x in xs do {
          let f = () -> str = x.to_str()
          print(msg: f())
      }
  }
  ```

- [ ] `tests/codegen/closures/partial_application.ori` — partial application creates closure env:
  ```ori
  // @test: codegen

  @add (a: int, b: int) -> int = a + b

  @main () -> void = {
      let add5 = add(a: 5)
      print(msg: add5(b: 3).to_str())
  }
  ```

- [ ] **TPR checkpoint** — `/tpr-review` covering 07.3–07.4 implementation work

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting 07.5:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 07.5 ABI and Iterator Pattern Tests

**File(s):** `tests/codegen/abi/` (new), `tests/codegen/iterator/` (new)

Write FileCheck tests for ABI patterns (parameter passing modes, return conventions) and iterator codegen patterns.

- [ ] **ABI tests** — verify parameter passing modes:
  - `tests/codegen/abi/scalar_params.ori` — scalar params passed directly (not via pointer)
  - `tests/codegen/abi/struct_sret.ori` — large struct returned via sret pointer
  - `tests/codegen/abi/borrowed_param.ori` — borrowed param passed as pointer without RC
  - `tests/codegen/abi/owned_param.ori` — owned param carries RC obligation
  - `tests/codegen/abi/void_return.ori` — void return convention
  - `tests/codegen/abi/multi_param.ori` — multiple params with mixed ownership

- [ ] **Iterator tests** — verify iterator codegen patterns:
  - `tests/codegen/iterator/for_loop_basic.ori` — basic for loop with iter/next/drop
  - `tests/codegen/iterator/for_loop_break.ori` — early break triggers iter_drop
  - `tests/codegen/iterator/for_yield.ori` — for-yield creates lazy iterator
  - `tests/codegen/iterator/map_filter.ori` — chained iterator methods
  - `tests/codegen/iterator/enumerate.ori` — enumerate produces (int, T) tuples
  - `tests/codegen/iterator/collect.ori` — collect materializes iterator into list

- [ ] Verify test count: at this point, the full `tests/codegen/` directory should contain at least 30 tests across all categories. Count with:
  ```bash
  find tests/codegen/ -name '*.ori' | wc -l
  ```
  If under 30, add additional tests in the thinnest category.

- [ ] **Subsection close-out (07.5)** — MANDATORY before starting 07.R:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 07.R Third Party Review Findings

<!-- Reserved for Codex or other external reviewers.
If unresolved findings exist here:
- section frontmatter `status` must be `in-progress`
- `third_party_review.status` must be `findings`

When all findings are triaged:
- accepted findings are integrated into the relevant implementation subsection(s)
- rejected findings are closed with rationale
- all items in this block are marked resolved
- `third_party_review.status` becomes `resolved` or `none`
-->

- None.

---

## 07.N Completion Checklist

- [ ] `ori_test_harness/src/check.rs` implements `.matches` and `.exact` modes
- [ ] `ori_llvm/tests/codegen_checks.rs` discovers and runs all `tests/codegen/*.ori` files
- [ ] `.matches` mode is the default (order-independent substring matching)
- [ ] Revision system works: debug/release/no-repr-opt configurations produce separate IR
- [ ] Bless mode updates `.ll` baselines when `ORI_BLESS=1`
- [ ] `tests/codegen/rc/` contains 6+ RC emission tests
- [ ] `tests/codegen/cow/` contains 6+ COW pattern tests
- [ ] `tests/codegen/closures/` contains 6+ closure codegen tests
- [ ] `tests/codegen/abi/` contains 6+ ABI pattern tests
- [ ] `tests/codegen/iterator/` contains 6+ iterator pattern tests
- [ ] Total: 30+ FileCheck tests across all categories
- [ ] All FileCheck tests pass: `timeout 150 cargo test -p ori_llvm --test codegen_checks`
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 07` returns 0 annotations
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `tests/codegen/` contains 30+ FileCheck-style tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns. All tests pass via `timeout 150 cargo test -p ori_llvm --test codegen_checks`. `.matches` mode is the default. Revision support works for debug/release/no-repr-opt. Bless mode updates baselines. A deliberately introduced codegen regression (e.g., removing an RC dec) causes the corresponding FileCheck test to fail.
