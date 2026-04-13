---
section: "07"
title: "FileCheck-Style IR Pattern Matching"
status: not-started
reviewed: false
goal: "Expand the existing FileCheck IR assertion framework in compiler/ori_llvm/tests/ to >=30 directive-based tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns — using .exact mode with function-scoped CHECK-LABEL anchoring for all order-sensitive categories, fixing known harness bugs, and splitting the over-limit aot.rs helper file"
success_criteria:
  - "compiler/ori_llvm/tests/codegen/ contains >=30 FileCheck-style tests with // CHECK: directives"
  - "Order-sensitive tests (RC, COW, closure env, ABI, iterator cleanup) use .exact mode with CHECK-LABEL function scoping"
  - ".matches mode reserved for pure existence/absence checks only"
  - "No regex syntax in CHECK patterns — engine uses literal substring matching"
  - "Tests run within 150s timeout as part of cargo test -p ori_llvm --test aot"
  - "At least 5 tests per category: RC emission, COW patterns, closure codegen, ABI, iterator patterns"
  - "Every 'should optimize' test has a corresponding 'should NOT optimize' negative pin"
  - "aot.rs split below 500-line limit into proper submodules"
inspired_by:
  - "Rust compiletest codegen tests — CHECK directives in source files"
  - "Zig addCheckFile — .matches mode for order-independent matching"
  - "LLVM FileCheck — CHECK/CHECK-NOT/CHECK-LABEL/CHECK-NEXT"
depends_on: ["02"]
third_party_review:
  status: resolved
  updated: 2026-04-12
sections:
  - id: "07.0"
    title: "Prerequisites: Harness Fixes and aot.rs Split"
    status: not-started
  - id: "07.1"
    title: "Evolve ir_checks.rs into Harness-Based Runner"
    status: not-started
  - id: "07.2"
    title: "RC Emission Tests"
    status: not-started
  - id: "07.3"
    title: "COW and Closure Tests"
    status: not-started
  - id: "07.4"
    title: "ABI, Iterator, and Cross-Feature Interaction Tests"
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
**Goal:** Expand the existing FileCheck IR assertion framework to 30+ directive-based tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns. Tests use `// CHECK:` directives embedded in `.ori` source files, compiled through the LLVM pipeline, with the resulting IR matched against directives via literal substring matching. Order-sensitive categories (RC, COW, closure env layout, ABI, iterator cleanup) use `.exact` mode with `CHECK-LABEL` function anchoring to enforce ordering. Pure existence/absence checks (e.g., "no RC ops anywhere", "sret attribute present") use `.matches` mode.

**Success Criteria:**

- [ ] 30+ FileCheck tests in `compiler/ori_llvm/tests/codegen/` — satisfies mission criterion: "FileCheck IR assertions"
- [ ] `.exact` mode with `CHECK-LABEL` function scoping for order-sensitive tests — correctness over convenience
- [ ] `.matches` mode reserved for existence/absence-only checks — avoids the multiple-match flaw
- [ ] No regex syntax (`{{.*}}`) in CHECK patterns — engine uses literal substring matching only
- [ ] Every "should optimize" test has a "should NOT optimize" companion — positive+negative pairing
- [ ] 5+ tests per category — satisfies mission criterion: "comprehensive IR coverage"
- [ ] `aot.rs` split below 500-line limit — per impl-hygiene.md file size rule

**Context:** Behavioral tests (Ori spec tests) verify that programs produce correct output but cannot catch codegen quality regressions. A program that leaks memory due to missing `RcDec` still produces correct stdout. A COW fast path that silently degrades to always-copy still produces correct output. FileCheck-style tests pin the LLVM IR patterns that indicate correct codegen, catching regressions at the IR level before they manifest as runtime bugs.

**Current state:** 12 FileCheck tests already exist in `compiler/ori_llvm/tests/codegen/` and run via `ir_checks.rs` (a module inside the `aot` integration test target). The `check.rs` matching engine in `ori_test_harness` is complete with both `.matches` and `.exact` modes. This section expands coverage to 30+ tests and fixes known harness limitations.

**Matching mode rationale:** The `.matches` mode (order-independent) is convenient but **dangerous as default for order-sensitive tests**:
- **RC ordering is load-bearing**: `ori_rc_inc` BEFORE use, `ori_rc_dec` AFTER last use. `.matches` mode cannot verify ordering.
- **COW ordering is load-bearing**: `is_shared` check BEFORE mutation, copy BEFORE write. `.matches` cannot verify this.
- **Iterator cleanup placement is load-bearing**: `ori_iter_drop` AFTER loop exit. `.matches` cannot verify placement.
- **Multiple-match flaw**: In `.matches` mode, two identical `CHECK: ori_rc_inc` directives both match the same IR line. A test expecting 2 increments passes with only 1 actual. This is a known limitation of the current engine (see 07.0 task below).
- **CHECK-NOT global scope**: In `.matches` mode, `CHECK-NOT` scans the entire module including runtime/stdlib functions. In `.exact` mode, `CHECK-NOT` scans from the current position to EOF (not bounded by labels). The robust mitigation is function-scoped IR slicing via `extract_function_ir()` in the FileCheckStrategy (see 07.0 prerequisite), which feeds only the target function's IR to the matching engine.

**No debug/release revisions for IR-level tests.** LLVM IR is identical between debug and release builds — FastISel only affects machine code instruction selection, not the IR. Debug/release revisions only make sense for behavioral/execution tests (in `aot/`), not IR pattern tests. IR tests compile at default optimization level.

**No `.ll` baseline blessing.** FileCheck pins specific patterns, not full IR. Full-IR baseline comparison is Section 12's role (`scripts/ir-baseline.sh`). These are complementary: Section 07 pins codegen correctness patterns, Section 12 detects any IR shape drift. Both use `ORI_BLESS=1` as the control plane but at different granularities.

**Reference implementations:**
- **Rust** `compiler/tests/codegen/`: CHECK directives in Rust source files, compiled with `--emit=llvm-ir`, matched against LLVM IR.
- **Zig** `test/src/LlvmIr.zig` (lines 45-73): `.matches` mode for order-independent matching.
- **LLVM** `llvm-project/llvm/utils/FileCheck/`: Reference FileCheck with CHECK/CHECK-NOT/CHECK-LABEL/CHECK-NEXT.

**Depends on:** Section 02 (shared harness provides directive parsing, check engine). Section 02 is complete.

**Cross-section notes:**
- **Section 11 (CI)**: The `ir_checks` module runs as part of `cargo test -p ori_llvm --test aot`, which `test-all.sh` already runs. No CI wiring changes needed for existing tests. New tests added to `ir_checks.rs` are automatically picked up.
- **Section 12 (Baselines)**: Section 07 pins patterns via CHECK directives; Section 12 pins full IR shape. No overlap — complementary coverage at different granularities.
- **aot.rs helpers**: The `compile_and_capture_ir()` function uses the debug binary and `ORI_DEBUG_LLVM=1`. This is sufficient for IR-level pattern testing since IR is identical between debug/release. If release-specific IR testing is ever needed, `compile_to_llvm_ir()` (also in `aot.rs`) provides a separate path.

---

## 07.0 Prerequisites: Harness Fixes and aot.rs Split

**Goal:** Fix known harness bugs that would make tests unreliable, and split the over-limit `aot.rs` file into proper submodules.

### Harness Bug Fixes

- [ ] **Document the multiple-match flaw in check.rs.** In `.matches` mode, two identical `CHECK: ori_rc_inc` directives both match the same IR line. A test expecting N occurrences of a pattern passes with only 1 actual. Add a `//!` doc comment in `check.rs` warning about this. Mitigation: use `.exact` mode with `CHECK-LABEL` scoping for any test that cares about occurrence count, or use distinct substring patterns (e.g., include the argument: `CHECK: call void @ori_rc_inc(ptr %xs)` instead of bare `CHECK: ori_rc_inc`).

- [ ] **Fix CHECK-NOT scope in exact mode.** Currently `run_exact_mode` in `check.rs` implements `CheckNot` by scanning from `search_from` to EOF. This means CHECK-NOT is unbounded — it picks up symbols from later functions, runtime declarations, and stdlib stubs. **Fix**: update `CheckNot` logic to evaluate only the lines between `search_from` and the *next* positive `Check` or `CheckLabel` match (or EOF if no subsequent matches), aligning with standard LLVM FileCheck semantics where CHECK-NOT checks the region between its preceding and following positive directives.

- [ ] **Add function-scoped IR slicing to the FileCheckStrategy.** CHECK-LABEL provides section anchoring within a file, but it does NOT provide true function isolation — `CheckNot` can still see symbols from later functions. The robust solution: the `FileCheckStrategy::execute()` method should use `extract_function_ir()` (from `util/ir_capture.rs` after the split) to slice the captured module IR to the target function before passing it to `run_checks()`. Convention: each test file must have a `// @function: <name>` custom directive specifying the target function. If absent, use `_ori_main` as default. This ensures CHECK-NOT patterns are truly function-scoped.

- [ ] **Document CHECK-LABEL search behavior.** In exact mode, `CHECK-LABEL` searches from line 0 (resetting search position). Document that LABEL patterns should be specific enough to unambiguously identify the target function (e.g., `CHECK-LABEL: define void @_ori_main` not just `CHECK-LABEL: @main`). With function-scoped IR slicing above, CHECK-LABEL is still useful for within-function structure (e.g., anchoring to a specific basic block label) but no longer the primary function isolation mechanism.

### aot.rs Split

- [ ] **Split `compiler/ori_llvm/tests/aot/util/aot.rs` (737 lines) into submodules.** The file exceeds the 500-line limit (impl-hygiene.md). Extract to:
  - `util/compile.rs` — `compile_and_run()`, `compile_and_run_capture()`, `compile_and_run_with_args()`, `assert_aot_success()`, `assert_multifile_aot_success()`, exit code helpers
  - `util/ir_capture.rs` — `compile_and_capture_ir()`, `extract_function_ir()`, `compile_to_llvm_ir()`, IR inspection helpers
  - `util/binary.rs` — `ori_binary()`, `ir_capture_binary()`, `stdlib_path()`, `workspace_root()`, path/binary discovery
  - `util/aot.rs` — re-exports from submodules for backward compatibility (thin facade)
  Each submodule must be under 500 lines. Update `util/mod.rs` to expose all submodules.

- [ ] **Subsection close-out (07.0)** — MANDATORY before starting 07.1:
  - [ ] All tasks above are `[x]` and verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**

---

## 07.1 Evolve ir_checks.rs into Harness-Based Runner

**File(s):** `compiler/ori_llvm/tests/aot/ir_checks.rs` (existing — evolve)

**Current state:** `ir_checks.rs` is a module inside `compiler/ori_llvm/tests/aot/main.rs` with 12 hand-written test functions, each calling `run_filecheck()` which compiles a source file from `tests/codegen/`, parses CHECK directives, and runs `check::run_checks()` in `.matches` mode. This works but does not use the shared harness's `run_test_directory()` orchestration.

**Decision: evolve ir_checks.rs, do NOT create a separate `codegen_checks.rs` target.** Creating a separate integration test binary would:
1. Duplicate compilation test infrastructure (the aot helpers are already available inside `aot/`)
2. Require extracting aot helpers to a shared location just for cross-target imports
3. Add a second test binary that `test-all.sh` would need to discover

Instead, evolve `ir_checks.rs` to optionally use `run_test_directory()` for automatic test discovery while keeping the existing per-test functions as explicit entry points.

- [ ] **Add a `run_all_codegen_filecheck` test** that uses `run_test_directory()` from the shared harness to automatically discover and run all `.ori` files in `compiler/ori_llvm/tests/codegen/`. This requires implementing `FileCheckStrategy` that implements `TestStrategy`:
  - `execute()`: calls `compile_and_capture_ir()` (from util) on the source, then slices to the target function using `extract_function_ir()` if a `// @function: <name>` custom directive is present (default: `_ori_main`). Returns sliced IR as `TestOutput.content`
  - `verify()`: calls `run_checks()` with the appropriate `CheckMode` (see below)
  - `baseline_suffix()`: returns `None` (no `.ll` baselines — pattern matching only)

- [ ] **Determine CheckMode per test file.** The `FileCheckStrategy::verify()` must select `.exact` or `.matches` mode per file. Convention: if a test file contains any `CHECK-LABEL` directive, use `.exact` mode (the author is asserting ordering). If only bare `CHECK` and `CHECK-NOT` directives are present, use `.matches` mode. This keeps backward compatibility with existing tests while enabling order-sensitive tests going forward.

- [ ] **Remove existing per-file test functions after migration.** Once `run_all_codegen_filecheck` via `run_test_directory()` is confirmed working for all existing 12 tests, delete the individual `filecheck_rc_simple_inc_dec()` etc. functions from `ir_checks.rs`. Running 30+ AOT compilations twice (once per manual test, once via discovery) threatens the mandatory 150s timeout and is WASTE. The discovery runner provides sufficient granularity via per-file pass/fail reporting in `TestSummary`.

- [ ] **Pass `bless` parameter correctly.** The `run_test_directory()` call must pass `bless::is_bless_enabled()` as the third argument, not hardcode `false`.

- [ ] **Add tests for the FileCheckStrategy:**
  - `test_filecheck_strategy_selects_exact_when_label_present`
  - `test_filecheck_strategy_selects_matches_when_no_label`
  - `test_filecheck_strategy_discovers_all_codegen_tests`

- [ ] **Subsection close-out (07.1)** — MANDATORY before starting 07.2:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — reflect on the debugging journey for 07.1 specifically: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where test failures gave unhelpful messages. Implement every accepted improvement NOW and commit each via SEPARATE `/commit-push` using a valid conventional-commit type.

---

## 07.2 RC Emission Tests

**File(s):** `compiler/ori_llvm/tests/codegen/rc/` (new subdirectory), `compiler/ori_llvm/tests/aot/ir_checks.rs` (add test entry points)

Write FileCheck tests that pin RC emission patterns. These tests verify that the AIMS pipeline + LLVM codegen emits the expected RC operations. **All RC tests use `.exact` mode with `CHECK-LABEL` function anchoring** because RC operation ordering is correctness-critical (inc before use, dec after last use).

**Important: use actual runtime symbol names.** The runtime uses type-specific RC functions, not generic ones. Common patterns in emitted IR:
- Lists: `ori_buffer_rc_dec`, `ori_buffer_store_elem_dec`, `ori_buffer_store_elem_count`
- Strings: `ori_str_rc_inc`, `ori_str_rc_dec`
- RC alloc/free: `ori_rc_alloc`, `ori_rc_dec`
- Verify the actual symbols by running `ORI_DEBUG_LLVM=1 cargo run -- build <test.ori>` and inspecting the IR before writing CHECK patterns.

Every "should emit RC" test has a companion "should NOT emit RC" test (positive+negative pairing).

- [ ] `compiler/ori_llvm/tests/codegen/rc/shared_value_inc_dec.ori` — shared list value produces buffer RC cleanup ops:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_buffer_rc_dec

  @main () -> void = {
      let xs = [1, 2, 3];
      let ys = xs;
      print(msg: ys.len().to_str())
  }
  ```
  **Note:** The exact CHECK patterns should be verified against `ORI_DEBUG_LLVM=1` output during implementation. The runtime uses type-specific symbols (e.g., `ori_buffer_rc_dec` for lists, `ori_str_rc_inc` for strings), not generic `ori_rc_inc`/`ori_rc_dec`.

- [ ] `compiler/ori_llvm/tests/codegen/rc/unique_owner_no_rc.ori` — unique owner consumed linearly, no RC operations (negative pin):
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK-NOT: ori_rc_inc
  // CHECK-NOT: ori_str_rc_inc

  @main () -> void = {
      let xs = [1, 2, 3];
      print(msg: xs.len().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/rc/scalar_no_rc.ori` — scalar int/bool have no RC operations (negative pin):
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK-NOT: ori_rc_inc
  // CHECK-NOT: ori_rc_dec
  // CHECK-NOT: ori_buffer_rc

  @main () -> void = {
      let x = 42;
      let y = x + 1;
      print(msg: y.to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/rc/string_copy_inc.ori` — string copy produces RC increment:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_str_rc_inc

  @main () -> void = {
      let s = "hello";
      let t = s;
      print(msg: t);
      print(msg: s)
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/rc/loop_rc_balanced.ori` — RC in for loop body is balanced:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = ["hello", "world"];
      for x in xs do print(msg: x)
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/rc/nested_struct_sharing.ori` — sharing a struct with heap-allocated fields triggers RC:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  type Wrapper = { inner: [int] }

  @main () -> void = {
      let w = Wrapper { inner: [1, 2, 3] };
      let w2 = w;
      print(msg: w2.inner.len().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/rc/list_of_strings_elem_dec.ori` — list of strings uses element dec for cleanup:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_buffer_store_elem_dec

  @main () -> void = {
      let xs = ["hello", "world"];
      print(msg: xs.len().to_str())
  }
  ```

- [ ] **Add test entry points in ir_checks.rs** for each new `.ori` file.

- [ ] **TPR checkpoint** — `/tpr-review` covering 07.0–07.2 implementation work

- [ ] **Subsection close-out (07.2)** — MANDATORY before starting 07.3:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection** — same protocol as 07.1's close-out. Commit improvements separately.

---

## 07.3 COW and Closure Tests

**File(s):** `compiler/ori_llvm/tests/codegen/cow/` (new), `compiler/ori_llvm/tests/codegen/closures/` (new)

### COW Pattern Tests

COW is one of the highest-risk codegen areas — a silent degradation from fast-path to always-copy is invisible to behavioral tests. **COW tests use `.exact` mode with `CHECK-LABEL`** because `is_shared` check ordering relative to mutation is correctness-critical.

- [ ] `compiler/ori_llvm/tests/codegen/cow/mutation_via_updated.ori` — list mutation via `.updated()` emits store operations:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_buffer_store_elem_dec
  // CHECK: ori_buffer_store_elem_count

  @main () -> void = {
      let xs = [1, 2, 3];
      let ys = xs.updated(key: 0, value: 42);
      print(msg: ys[0].to_str())
  }
  ```
  **Note:** `xs[0] = 42` index assignment syntax is not yet supported (pending design proposal — see `tests/spec/expressions/index_access.ori:359`). Use `.updated()` method instead, which is the current supported mutation API.

- [ ] `compiler/ori_llvm/tests/codegen/cow/push_triggers_cow_ops.ori` — list push triggers COW operations:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      let ys = xs;
      ys.push(value: 4);
      print(msg: ys.len().to_str());
      print(msg: xs.len().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/cow/unique_push_no_copy.ori` — unique owner push skips copy (negative pin):
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK-NOT: memcpy

  @main () -> void = {
      let xs = [1, 2, 3];
      xs.push(value: 4);
      print(msg: xs.len().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/cow/map_insert.ori` — map mutation via `.insert()`:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let m = {"a": 1, "b": 2};
      let m2 = m;
      m2.insert(key: "a", value: 42);
      print(msg: m2["a"].to_str())
  }
  ```
  **Note:** `m["a"] = 42` bracket mutation syntax is not yet supported. Use `.insert()` method.

- [ ] `compiler/ori_llvm/tests/codegen/cow/drop_at_scope_end.ori` — drop hint emitted for COW values at scope end:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      let ys = xs;
      print(msg: ys.len().to_str())
  }
  ```

### Closure Codegen Tests

Closure tests pin capture patterns, environment layout, and RC for captured values. **Closure env layout tests use `.exact` mode with `CHECK-LABEL`** because the environment allocation must happen before closure invocation.

- [ ] `compiler/ori_llvm/tests/codegen/closures/capture_allocates_env.ori` — closure with captures creates env allocation:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_rc_alloc

  @main () -> void = {
      let x = "hello";
      let f = () -> str = x;
      print(msg: f())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/closures/no_capture_no_env.ori` — closure without captures has no env allocation (negative pin):
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK-NOT: ori_rc_alloc

  @main () -> void = {
      let f = (x: int) -> int = x + 1;
      print(msg: f(x: 5).to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/closures/nested_closure_rc_chain.ori` — nested closures create RC chain:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      let f = () -> (() -> int) = {
          () -> int = xs.len()
      };
      let g = f();
      print(msg: g().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/closures/closure_in_loop.ori` — closure created inside loop:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      for x in xs do {
          let f = () -> str = x.to_str();
          print(msg: f())
      }
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/closures/closure_as_argument.ori` — closure passed as function argument:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @apply (f: () -> int) -> int = f();

  @main () -> void = {
      let x = 10;
      let result = apply(f: () -> int = x * 2);
      print(msg: result.to_str())
  }
  ```

- [ ] **Add test entry points in ir_checks.rs** for each new `.ori` file.

- [ ] **Subsection close-out (07.3)** — MANDATORY before starting 07.4:
  - [ ] All tasks above are `[x]` and the subsection's behavior is verified
  - [ ] Update this subsection's `status` in section frontmatter to `complete`
  - [ ] **Run `/improve-tooling` retrospectively on THIS subsection**.

---

## 07.4 ABI, Iterator, and Cross-Feature Interaction Tests

**File(s):** `compiler/ori_llvm/tests/codegen/abi/` (new), `compiler/ori_llvm/tests/codegen/iterator/` (new), `compiler/ori_llvm/tests/codegen/cross/` (new)

### ABI Tests

Verify parameter passing modes and return conventions. ABI prologue/epilogue tests use `.exact` mode with `CHECK-LABEL` to verify function signature structure.

- [ ] `compiler/ori_llvm/tests/codegen/abi/scalar_direct_pass.ori` — scalar params passed directly (not via pointer):
  ```ori
  // CHECK-LABEL: define i64 @_ori_add
  // CHECK-NOT: sret

  @add (a: int, b: int) -> int = a + b;

  @main () -> void = print(msg: add(a: 1, b: 2).to_str())
  ```

- [ ] `compiler/ori_llvm/tests/codegen/abi/struct_sret_return.ori` — large struct returned via sret pointer:
  ```ori
  // CHECK-LABEL: define void @_ori_make_point
  // CHECK: sret

  type Point = { x: int, y: int }

  @make_point (x: int, y: int) -> Point = Point { x, y };

  @main () -> void = {
      let p = make_point(x: 1, y: 2);
      print(msg: p.x.to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/abi/void_return.ori` — void return convention:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = print(msg: "hello")
  ```

- [ ] `compiler/ori_llvm/tests/codegen/abi/borrowed_param_no_rc.ori` — borrowed param passed without RC operations:
  ```ori
  // CHECK-LABEL: define i64 @_ori_helper
  // CHECK-NOT: ori_rc_inc
  // CHECK-NOT: ori_rc_dec

  @helper (xs: [int]) -> int = xs.len();

  @main () -> void = {
      let xs = [1, 2, 3];
      print(msg: helper(xs:).to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/abi/multi_param_mixed.ori` — multiple params with mixed types:
  ```ori
  // CHECK-LABEL: define void @_ori_mixed

  @mixed (n: int, s: str, b: bool) -> void = {
      if b then print(msg: s) else print(msg: n.to_str())
  };

  @main () -> void = mixed(n: 42, s: "hello", b: true)
  ```

### Iterator Tests

Verify iterator codegen patterns. **Iterator cleanup tests use `.exact` mode** because `ori_iter_drop` placement relative to loop exit is correctness-critical.

- [ ] `compiler/ori_llvm/tests/codegen/iterator/for_loop_normal_drop.ori` — normal for loop exit triggers iter_drop:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_iter_drop

  @main () -> void = {
      let xs = [1, 2, 3];
      for x in xs do print(msg: x.to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/iterator/break_triggers_drop.ori` — early break still triggers iter_drop:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_iter_drop

  @main () -> void = {
      let xs = [1, 2, 3, 4, 5];
      for x in xs do {
          if x == 3 then break;
          print(msg: x.to_str())
      }
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/iterator/map_filter_chain.ori` — chained iterator methods produce composed iteration:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3, 4, 5];
      let ys = xs.iter().filter(predicate: x -> x > 2).collect();
      print(msg: ys.len().to_str())
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/iterator/enumerate_produces_tuple.ori` — enumerate produces (int, T) tuples:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = ["a", "b", "c"];
      for (i, x) in xs.iter().enumerate() do {
          print(msg: `{i}: {x}`)
      }
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/iterator/collect_materializes.ori` — collect materializes iterator into list:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      let doubled = xs.iter().map(transform: x -> x * 2).collect();
      print(msg: doubled.len().to_str())
  }
  ```

### Cross-Feature Interaction Tests

Tests that verify codegen correctness at feature boundaries — where compilers most commonly break.

- [ ] `compiler/ori_llvm/tests/codegen/cross/cow_inside_closure.ori` — COW push inside a closure capturing a list:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let xs = [1, 2, 3];
      let mutator = () -> void = {
          xs.push(value: 4);
          print(msg: xs.len().to_str())
      };
      mutator()
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/cross/rc_iterator_break.ori` — RC-tracked value inside for loop with early break:
  ```ori
  // CHECK-LABEL: define void @_ori_main
  // CHECK: ori_iter_drop

  @main () -> void = {
      let xs = ["hello", "world", "!"];
      for x in xs do {
          if x == "!" then break;
          let copy = x;
          print(msg: copy)
      }
  }
  ```

- [ ] `compiler/ori_llvm/tests/codegen/cross/closure_loop_drop.ori` — closure capturing a list, created inside a loop, properly drops env:
  ```ori
  // CHECK-LABEL: define void @_ori_main

  @main () -> void = {
      let names = ["alice", "bob"];
      for name in names do {
          let greet = () -> str = `hello {name}`;
          print(msg: greet())
      }
  }
  ```

- [ ] Verify test count: at this point, `compiler/ori_llvm/tests/codegen/` should contain at least 30 tests. Count with:
  ```bash
  find compiler/ori_llvm/tests/codegen/ -name '*.ori' | wc -l
  ```
  If under 30, add additional tests in the thinnest category.

- [ ] **TPR checkpoint** — `/tpr-review` covering 07.3–07.4 implementation work

- [ ] **Subsection close-out (07.4)** — MANDATORY before starting 07.R:
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

- [x] `[TPR-07-001-codex][high]` `section-07-filecheck.md:72` — Replace claimed CHECK-LABEL function scoping with real function slicing.
  Resolved: Fixed on 2026-04-12. Added `extract_function_ir()` slicing to FileCheckStrategy in 07.0+07.1, fixed CHECK-NOT scope in check.rs as prerequisite.
- [x] `[TPR-07-002-codex][high]` `section-07-filecheck.md:268` — Rewrite COW mutation examples to supported set/insert forms.
  Resolved: Fixed on 2026-04-12. Replaced `xs[0] = 42` with `.updated()` and `.push()`, `m["a"] = 42` with `.insert()`. Added notes about unsupported syntax.
- [x] `[TPR-07-003-codex][medium]` `section-07-filecheck.md:164` — Retarget RC checks to actual runtime symbols.
  Resolved: Fixed on 2026-04-12. Added type-specific symbol guidance, updated examples to use `ori_buffer_rc_dec`/`ori_str_rc_inc`, added verification instructions.
- [x] `[TPR-07-001-gemini][high]` `section-07-filecheck.md:65` — Fix CHECK-NOT unbounded scope in exact mode.
  Resolved: Fixed on 2026-04-12. Added check.rs fix as prerequisite in 07.0 (bound CHECK-NOT to region between positive directives). Same root cause as TPR-07-001-codex.
- [x] `[TPR-07-002-gemini][high]` `section-07-filecheck.md:121` — Remove duplicate per-file manual tests.
  Resolved: Fixed on 2026-04-12. Updated 07.1 to delete per-file manual test functions after migration to discovery runner.
- [x] `[TPR-07-003-gemini][medium]` `section-07-filecheck.md:79` — Update aot.rs split target list to actual functions.
  Resolved: Fixed on 2026-04-12. Replaced `compile_and_check_output()`/`compile_expect_error()` with actual functions: `compile_and_run_capture()`, `assert_aot_success()`, etc.

---

## 07.N Completion Checklist

- [ ] `compiler/ori_llvm/tests/codegen/` contains 30+ FileCheck-style `.ori` test files
- [ ] `ir_checks.rs` has `run_all_codegen_filecheck` discovery test (per-file manual tests removed after migration)
- [ ] `FileCheckStrategy` implements `TestStrategy` and auto-selects mode (`.exact` when CHECK-LABEL present, `.matches` otherwise)
- [ ] Order-sensitive tests (RC, COW, closure env, ABI, iterator cleanup) use `.exact` mode with `CHECK-LABEL`
- [ ] `.matches` mode used only for pure existence/absence checks
- [ ] No `{{.*}}` regex syntax in any CHECK patterns — all patterns are literal substrings
- [ ] Every "should optimize" test has a "should NOT optimize" companion
- [ ] `aot.rs` split into submodules, all under 500-line limit
- [ ] `compiler/ori_llvm/tests/codegen/rc/` contains 7+ RC emission tests
- [ ] `compiler/ori_llvm/tests/codegen/cow/` contains 5+ COW pattern tests
- [ ] `compiler/ori_llvm/tests/codegen/closures/` contains 5+ closure codegen tests
- [ ] `compiler/ori_llvm/tests/codegen/abi/` contains 5+ ABI pattern tests
- [ ] `compiler/ori_llvm/tests/codegen/iterator/` contains 5+ iterator pattern tests
- [ ] `compiler/ori_llvm/tests/codegen/cross/` contains 3+ cross-feature interaction tests
- [ ] Multiple-match flaw documented in `check.rs`
- [ ] CHECK-NOT global scope limitation documented in `check.rs`
- [ ] All FileCheck tests pass: `timeout 150 cargo test -p ori_llvm --test aot -- ir_checks`
- [ ] No regressions: `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] Plan annotation cleanup: no stale section-07 annotations in source code
- [ ] All intermediate TPR checkpoint findings resolved
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` -> `complete`, subsection statuses updated
  - [ ] `00-overview.md` Quick Reference updated
  - [ ] `00-overview.md` mission success criteria checkboxes updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed (final, full-section)
- [ ] `/impl-hygiene-review` passed — AFTER `/tpr-review` is clean
- [ ] `/improve-tooling` **section-close sweep** — verify per-subsection retrospectives ran, add cross-cutting items.

**Exit Criteria:** `compiler/ori_llvm/tests/codegen/` contains 30+ FileCheck-style tests covering RC emission, COW patterns, closure codegen, ABI, iterator patterns, and cross-feature interactions. All tests pass via `timeout 150 cargo test -p ori_llvm --test aot -- ir_checks`. Order-sensitive tests use `.exact` mode with `CHECK-LABEL` function scoping. Pure existence/absence tests use `.matches` mode. No regex patterns. Every positive pin has a negative companion. A deliberately introduced codegen regression (e.g., removing an RC dec) causes the corresponding FileCheck test to fail.
