---
section: "14"
title: Codegen Test Harness
status: not-started
goal: Three-level test strategy covering unit tests, LLVM FileCheck IR verification, and end-to-end execution tests for both ori_arc and ori_llvm
sections:
  - id: "14.1"
    title: Existing Test Infrastructure Audit
    status: not-started
  - id: "14.2"
    title: "V2 Test Strategy: Three Levels"
    status: not-started
  - id: "14.3"
    title: ori_arc Test Strategy
    status: not-started
  - id: "14.4"
    title: "@test Annotation in AOT Context"
    status: not-started
---

# Section 14: Codegen Test Harness

**Status:** Not Started
**Goal:** A comprehensive test framework for codegen with three testing levels: unit tests per lowering module, LLVM FileCheck-based IR verification, and end-to-end execution tests. Covers both `ori_arc` (ARC IR transformations) and `ori_llvm` (LLVM IR generation). Includes `ori_arc`-specific test strategy for borrow inference, RC insertion, RC elimination, and constructor reuse.

**Reference compilers:**
- **Rust** `compiler/rustc_codegen_llvm/tests/` -- FileCheck-based IR tests, codegen test infrastructure
- **Zig** `test/behavior/` -- execution tests; `test/compile_errors/` -- expected diagnostics
- **LLVM** `llvm/test/` -- canonical FileCheck usage patterns, `lit` test runner
- **Roc** `crates/compiler/test_mono/` -- ARC IR tests, `crates/compiler/gen_llvm/` -- execution tests

**Current state:** Extensive execution tests exist in `ori_llvm/src/tests/` (17 files, 6,836 lines) and `oric/tests/phases/codegen/` (17 files, 5,451 lines). V2 adds IR verification with FileCheck and ARC-specific testing.

---

## 14.1 Existing Test Infrastructure Audit

### ori_llvm/src/tests/ (17 files, 6,836 lines)

JIT-based execution tests using the `TestCodegen` helper struct:

| File | Lines | What it tests |
|------|-------|--------------|
| `evaluator_tests.rs` | 283 | End-to-end evaluation of expressions |
| `arithmetic_tests.rs` | 121 | Integer and float arithmetic |
| `operator_tests.rs` | 936 | Binary/unary operators, comparisons, short-circuit |
| `control_flow_tests.rs` | 439 | If/else, match, basic branching |
| `more_control_flow_tests.rs` | 385 | Nested control flow, complex match |
| `advanced_control_flow_tests.rs` | 536 | Loops, break/continue, for expressions |
| `function_tests.rs` | 488 | Function declaration, recursion, closures |
| `function_call_tests.rs` | 376 | Named arguments, multi-arg calls |
| `function_seq_tests.rs` | 521 | FunctionSeq (multi-expression functions) |
| `function_exp_tests.rs` | 350 | FunctionExp (expression functions) |
| `string_tests.rs` | 114 | String operations, concatenation |
| `collection_tests.rs` | 697 | Lists, maps, tuples, structs |
| `matching_tests.rs` | 533 | Pattern matching compilation |
| `builtins_tests.rs` | 245 | Built-in function calls |
| `runtime_tests.rs` | 291 | Runtime function integration |
| `type_conversion_tests.rs` | 272 | Type coercions and casts |
| `mod.rs` | 249 | `TestCodegen` struct, `setup_test!` macro, JIT helpers |

**`TestCodegen` pattern:** Creates a `CodegenCx`, declares runtime functions, compiles a function from parsed AST, and JIT-executes it. Methods: `compile_function()`, `jit_execute_i64()`, `jit_execute_bool()`, `print_to_string()`. Runtime mappings added via `add_runtime_mappings()` for print, panic, assert, list, string operations.

### oric/tests/phases/codegen/ (17 files, 5,451 lines)

Phase-level tests for the AOT pipeline:

| File | Lines | What it tests |
|------|-------|--------------|
| `debug_config.rs` | 226 | DebugInfoConfig, DebugLevel, DebugFormat |
| `debug_builder.rs` | 276 | DebugInfoBuilder creation, basic type creation |
| `debug_types.rs` | 362 | Composite debug types (struct, enum, Option, Result, list) |
| `debug_context.rs` | 287 | DebugContext, LineMap, span-to-location conversion |
| `linker_core.rs` | 351 | Linker flavor detection, output paths, library search |
| `linker_gcc.rs` | 384 | GCC/Clang linker driver |
| `linker_msvc.rs` | 150 | MSVC linker driver |
| `linker_wasm.rs` | 156 | WebAssembly linker driver |
| `optimization.rs` | 649 | OptimizationConfig, pass pipeline, verification |
| `object_emit.rs` | 479 | Object file emission, target machines |
| `targets.rs` | 407 | Target triple parsing, feature detection |
| `mangling.rs` | 292 | Symbol name mangling and demangling |
| `runtime.rs` | 60 | Runtime function declarations |
| `runtime_lib.rs` | 232 | Runtime library discovery |
| `build_command.rs` | 407 | Build command construction |
| `wasm.rs` | 661 | WASM-specific codegen tests |

### oric/src/testing/ (3 files)

Test harness utilities:

| File | What it provides |
|------|-----------------|
| `harness.rs` | `eval_expr()`, `eval_source()`, `parse_source()`, `type_check_source()`, assertion helpers |
| `mocks.rs` | Mock implementations for testing |
| `mod.rs` | Module re-exports |

### What exists vs what is missing

| Capability | Status | Notes |
|-----------|--------|-------|
| JIT execution tests | Done | 17 files, extensive coverage |
| AOT pipeline tests | Done | 17 files, debug/linker/opt/emit |
| IR verification (FileCheck) | **Missing** | Cannot assert specific IR patterns |
| ARC IR tests | **Missing** | No tests for borrow inference, RC insertion, etc. |
| AOT execution tests | **Partial** | WASM tests exist; native AOT test runner incomplete |
| Memory safety tests | **Missing** | No ASAN/Valgrind integration |
| `@test` AOT compilation | **Missing** | Test annotation not compiled to AOT |

- [ ] Preserve all existing JIT and AOT pipeline tests
- [ ] Add FileCheck-based IR verification (Section 14.2)
- [ ] Add ARC IR transformation tests (Section 14.3)

---

## 14.2 V2 Test Strategy -- Three Levels

### Level 1: Unit Tests (Per Lowering Module)

Test individual lowering functions using real parsing and type checking (extend the `TestCodegen` pattern, not mocks). Each expression lowering module from Section 03 gets its own unit test file:

```rust
// Pseudocode: unit test for literal lowering
#[test]
fn test_lower_int_literal() {
    setup_test!(test_lower_int_literal);
    let source = "@main () -> int = 42";
    // Parse and type check using harness
    let (parsed, type_result, interner) = type_check_source(source);
    // Compile via TestCodegen
    codegen.compile_function(/* ... */);
    // JIT execute and verify
    let result = codegen.jit_execute_i64("test_fn").unwrap();
    assert_eq!(result, 42);
}
```

Unit tests also verify builder abstractions in isolation:
- `TypeInfo` classification and LLVM type creation (Section 01)
- `ScopeBinding` management and variable lookup (Section 02)
- `LoopContext` break/continue value collection (Section 03)
- `ParamPassing`/`ReturnPassing` computation (Section 04)

### Level 2: IR Verification with LLVM FileCheck

LLVM FileCheck is the industry-standard tool for verifying compiler IR output, used by Rust, Zig, and LLVM itself. It matches patterns in textual IR against expected annotations.

**Setup:** FileCheck is distributed with LLVM. Add as an external tool dependency (not a Rust crate). Invoke via `std::process::Command` in test infrastructure.

**Test pattern:** Compile Ori source to LLVM IR text, pipe to FileCheck with expected patterns:

```rust
// Pseudocode: FileCheck test infrastructure
fn filecheck_test(ori_source: &str, check_patterns: &str) {
    // 1. Compile Ori source to LLVM IR string
    let ir = compile_to_llvm_ir(ori_source);

    // 2. Write IR to temp file
    let ir_path = write_temp_file(&ir);

    // 3. Write CHECK patterns to temp file
    let check_path = write_temp_file(check_patterns);

    // 4. Run FileCheck
    let status = Command::new("FileCheck")
        .arg(&check_path)
        .arg("--input-file").arg(&ir_path)
        .status()
        .expect("FileCheck not found -- install LLVM tools");

    assert!(status.success(), "FileCheck failed:\nIR:\n{ir}\nPatterns:\n{check_patterns}");
}
```

**Note:** FileCheck tests require the `FileCheck` binary from an LLVM installation. Gate these tests behind a `#[cfg(feature = "filecheck")]` feature flag. When FileCheck is unavailable, tests should be `#[ignore]` with a skip message explaining the dependency. Add `filecheck` as an optional feature in the test crate's `Cargo.toml`.

**Example FileCheck patterns for common codegen outputs:**

Note: The calling convention (`fastcc`, `ccc`, etc.) depends on the ABI decision in Section 04. Use flexible patterns like `define {{.*}} i64` rather than hardcoding `define fastcc i64` so tests remain valid regardless of calling convention.

```llvm
; Function declaration (flexible calling convention)
; CHECK: define {{.*}} i64 @_ori_main$add(i64 %0, i64 %1) {
; CHECK-NEXT: entry:
; CHECK-NEXT:   %2 = add i64 %0, %1
; CHECK-NEXT:   ret i64 %2
; CHECK-NEXT: }

; If/else branching
; CHECK: %[[COND:.*]] = icmp sgt i64 %{{.*}}, 0
; CHECK-NEXT: br i1 %[[COND]], label %[[THEN:.*]], label %[[ELSE:.*]]
; CHECK: [[THEN]]:
; CHECK: br label %[[MERGE:.*]]
; CHECK: [[ELSE]]:
; CHECK: br label %[[MERGE]]
; CHECK: [[MERGE]]:
; CHECK-NEXT: %{{.*}} = phi i64

; Pattern match with tag dispatch
; CHECK: %[[TAG:.*]] = load i8, ptr %{{.*}}
; CHECK: switch i8 %[[TAG]], label %[[DEFAULT:.*]] [
; CHECK-NEXT:   i8 0, label %[[CASE0:.*]]
; CHECK-NEXT:   i8 1, label %[[CASE1:.*]]
; CHECK-NEXT: ]

; RC operations (after ARC lowering)
; CHECK: call void @ori_rc_inc(ptr %[[VAR:.*]])
; CHECK: call void @ori_rc_dec(ptr %[[VAR]], ptr @_ori_drop$MyStruct)
```

**Register name normalization:** FileCheck's `%[[NAME:.*]]` syntax handles SSA register renaming. Use named capture groups for values referenced across multiple CHECK lines. Avoid matching exact register numbers (`%3`) -- always use regex patterns (`%{{.*}}`).

**Integration:** Add a `cargo filecheck` alias or integrate into the test runner:

```rust
// In tests/codegen_ir/mod.rs
#[test]
fn test_add_function_ir() {
    filecheck_test(
        r#"add (a: int, b: int) -> int = a + b"#,
        r#"
; CHECK: define {{.*}} i64 @{{.*}}add(i64 %0, i64 %1)
; CHECK:   %{{.*}} = add i64 %0, %1
; CHECK:   ret i64
        "#,
    );
}
```

### Level 3: Execution Tests (End-to-End)

**Existing JIT tests (preserve and extend):** All 17 existing `ori_llvm/src/tests/` files continue to work. Extend with new tests for ARC-managed types, closures, and pattern matching.

**AOT execution tests:** Compile Ori source to native binary, run it, assert on exit code and stdout:

```rust
// Pseudocode: AOT execution test
fn aot_exec_test(ori_source: &str, expected_stdout: &str, expected_exit: i32) {
    let binary = compile_to_binary(ori_source);
    let output = Command::new(&binary).output().unwrap();
    assert_eq!(output.status.code(), Some(expected_exit));
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected_stdout);
}
```

**ASAN/Valgrind integration for memory safety:**

```bash
# Compile with AddressSanitizer
ori build --sanitize=address tests/aot/rc_basic.ori -o rc_basic
./rc_basic  # ASAN reports use-after-free, leaks

# Compile normally, run under Valgrind
ori build tests/aot/rc_basic.ori -o rc_basic
valgrind --leak-check=full --error-exitcode=1 ./rc_basic
```

Memory safety tests verify:
- No memory leaks (every `ori_rc_alloc` paired with `ori_rc_free`)
- No use-after-free (dec to zero followed by access)
- No double-free (redundant dec)
- Constructor reuse correctness (reset/reuse does not corrupt live data)

- [ ] Implement FileCheck test infrastructure (`filecheck_test()` function)
- [ ] Create FileCheck tests for basic expressions, control flow, functions
- [ ] Create FileCheck tests for ARC operations (inc, dec, drop, reset/reuse)
- [ ] Add AOT execution test infrastructure
- [ ] Add ASAN integration to CI for memory safety regression testing
- [ ] Ensure all existing JIT tests pass with V2 pipeline

---

## 14.3 ori_arc Test Strategy (Critical Gap)

The `ori_arc` crate performs multiple transformation passes on ARC IR. Each pass requires dedicated tests because bugs in ARC transformations cause use-after-free, leaks, or incorrect behavior at runtime -- and these bugs are extremely difficult to diagnose from execution tests alone.

### ARC IR Lowering Tests

Verify that typed AST correctly lowers to ARC IR basic blocks:

```rust
// Test: function body becomes a single block with Return terminator
#[test]
fn test_lower_simple_function() {
    let arc_fn = lower_to_arc_ir("f (x: int) -> int = x + 1");
    assert_eq!(arc_fn.blocks.len(), 1);
    assert!(matches!(arc_fn.blocks[0].terminator, ArcTerminator::Return(_)));
}

// Test: if/else creates three blocks (then, else, merge)
#[test]
fn test_lower_if_else() {
    let arc_fn = lower_to_arc_ir("f (x: int) -> int = if x > 0 then x else 0 - x");
    assert_eq!(arc_fn.blocks.len(), 4); // entry, then, else, merge
    assert!(matches!(arc_fn.blocks[0].terminator, ArcTerminator::Branch { .. }));
}
```

### Borrow Inference Tests

Assert that parameters are correctly classified as borrowed vs owned after inference (Section 06):

```rust
// Test: parameter used but not consumed is borrowed
#[test]
fn test_borrow_read_only_param() {
    let arc_fn = lower_and_infer("f (x: [int]) -> int = x.len()");
    assert!(arc_fn.params[0].is_borrowed);
}

// Test: parameter passed to consuming function is owned
#[test]
fn test_owned_consumed_param() {
    let arc_fn = lower_and_infer("f (xs: [int]) -> [int] = xs.append(42)");
    assert!(!arc_fn.params[0].is_borrowed);
}

// Test: parameter used in multiple branches remains owned (conservative)
#[test]
fn test_owned_multi_branch() {
    let arc_fn = lower_and_infer(r#"
        f (xs: [int], flag: bool) -> [int] =
            if flag then xs.append(1) else xs.append(2)
    "#);
    assert!(!arc_fn.params[0].is_borrowed);
}
```

### RC Insertion Tests

Verify that RcInc/RcDec instructions are placed at the correct positions relative to variable liveness (Section 07):

```rust
// Test: parameter used once gets no inc, dec at end
#[test]
fn test_rc_single_use_param() {
    let arc_fn = lower_infer_and_insert_rc("f (x: str) -> str = x");
    let block = &arc_fn.blocks[0];
    // No RcInc (single use, consumed by return)
    assert!(!block.instrs.iter().any(|i| matches!(i, ArcInstr::RcInc { .. })));
}

// Test: variable used twice gets inc before second use
#[test]
fn test_rc_multi_use() {
    let arc_fn = lower_infer_and_insert_rc(r#"
        f (x: str) -> (str, str) = (x, x)
    "#);
    let block = &arc_fn.blocks[0];
    let inc_count = block.instrs.iter().filter(|i| matches!(i, ArcInstr::RcInc { .. })).count();
    assert_eq!(inc_count, 1); // One inc for the second use
}
```

### RC Elimination Tests

Verify that paired retain/release operations are correctly removed (Section 08):

```rust
// Test: inc immediately followed by dec on same var is eliminated
#[test]
fn test_eliminate_paired_inc_dec() {
    let before = build_arc_fn_with(|block| {
        block.push(ArcInstr::RcInc { var: x });
        block.push(ArcInstr::RcDec { var: x, drop_fn: None });
    });
    let after = eliminate_rc(before);
    assert!(after.blocks[0].instrs.is_empty());
}
```

### Constructor Reuse Tests

Verify that eligible patterns produce Reset/Reuse instructions (Section 09):

```rust
// Test: match arm that reconstructs same type gets reuse
#[test]
fn test_reuse_same_constructor() {
    let arc_fn = lower_through_reuse(r#"
        map_inc (xs: [int]) -> [int] = match xs
            [] -> []
            [head, ...tail] -> [head + 1, ...map_inc(tail)]
    "#);
    // Verify Reset and Reuse instructions exist
    assert!(has_instr(&arc_fn, |i| matches!(i, ArcInstr::Reset { .. })));
    assert!(has_instr(&arc_fn, |i| matches!(i, ArcInstr::Reuse { .. })));
}
```

### Decision Tree Tests

Verify that pattern compilation produces correct Switch structures (Section 10):

```rust
// Test: two-variant enum produces switch with two arms
#[test]
fn test_decision_tree_two_variants() {
    let tree = compile_patterns(r#"
        match x
            Some(v) -> v
            None -> 0
    "#);
    assert!(matches!(tree, DecisionTree::Switch { branches, .. } if branches.len() == 2));
}
```

- [ ] Create `ori_arc/src/tests/` module structure
- [ ] Add ARC IR lowering tests (AST to basic blocks)
- [ ] Add borrow inference tests (borrowed vs owned classification)
- [ ] Add RC insertion tests (RcInc/RcDec placement)
- [ ] Add RC elimination tests (paired operation removal)
- [ ] Add constructor reuse tests (Reset/Reuse detection)
- [ ] Add decision tree tests (pattern compilation)

---

## 14.4 @test Annotation in AOT Context

Ori's `@test` annotation declares test functions that are compiled and run as part of the verification system. In the AOT context, this requires special handling.

### Test Function Compilation

`@test` functions compile to regular LLVM functions with the `_ori_test_` name prefix (Section 04). Their signatures are `() -> void` (test functions take no parameters and cannot return values). The `_ori_test_` prefix follows the same `_ori_` mangling convention used for all Ori symbols (e.g., `_ori_test_math$test_add`), keeping the namespace consistent. The AOT compiler collects all test functions during compilation and generates a test runner binary.

### Test Runner Binary Generation

When `ori test` is invoked:

1. **Discovery:** The compiler scans all modules for `@test`-annotated functions
2. **Filtering:** `--only-attached` filters to tests that target a specific function
3. **Object file compilation:** Test wrapper functions (`_ori_test_*`) and the synthetic `main()` are compiled into the module's LLVM module alongside the functions they test. In 0.1-alpha, compilation uses per-module LLVM modules (not per-function `.o` files — see Section 12.2). Each module produces a single `.o` that includes both the tested functions and their test wrappers.
4. **Runner generation:** A synthetic `main()` is generated that calls each test function in sequence:

```rust
// Pseudocode: generated test runner main
fn generate_test_runner(test_fns: &[TestFunction]) -> LLVMModule {
    // For each test function, emit:
    //   call void @_ori_test_<module>$<name>()
    //   // If it panics, the @panic handler reports failure
    //
    // After all tests:
    //   call void @ori_test_summary(total, passed, failed)
    //   ret i32 (failed > 0 ? 1 : 0)
}
```

### JIT vs AOT Test Execution

| Mode | When | How |
|------|------|-----|
| JIT | `ori test` (development) | Compile to IR, JIT-execute each test. Faster iteration. |
| AOT | `ori test --aot` or CI | Compile to binary, execute. Enables ASAN/Valgrind. |

Both JIT and AOT modes use the unified `_ori_test_` prefix for test function names. The current JIT path uses the legacy `__test_` prefix and must be migrated to `_ori_test_` as part of V2 (see Section 04.7).

JIT mode is the default for development because it avoids the link step. AOT mode is used in CI for memory safety verification and to catch codegen bugs that only manifest in the AOT pipeline (e.g., incorrect ABI, missing runtime symbols).

### Test Discovery at Compile Time

Test functions are discovered during type checking (they have the `@test` attribute). The type checker produces a list of `TestDescriptor` entries:

```rust
// Pseudocode: test descriptor
struct TestDescriptor {
    name: Name,
    module: ModulePath,
    target: Option<FunctionRef>,  // For @test tests @target
    span: Span,
    mangled_name: String,         // _ori_test_<module>$<name>
}
```

The AOT compiler receives these descriptors and generates the appropriate test runner. Attached tests (`@test tests @target fn_name`) are re-run when `fn_name` or its transitive dependencies change (Section 12's incremental compilation tracks this).

### --only-attached Filtering

The `--only-attached` flag restricts execution to tests that have a `@target` annotation. This is useful for focused testing during development:

```bash
ori test --only-attached src/math.ori  # Only tests targeting functions in math.ori
```

In AOT mode, filtering happens at the runner generation step -- non-matching tests are simply not included in the generated `main()`.

- [ ] Implement `_ori_test_` prefix for @test functions in codegen
- [ ] Implement test runner binary generation
- [ ] Add JIT test execution mode (default for `ori test`)
- [ ] Add AOT test execution mode (`ori test --aot`)
- [ ] Implement `--only-attached` filtering
- [ ] Wire test discovery into incremental compilation dependency tracking

**Exit Criteria:** Each codegen module has unit tests. FileCheck-based IR verification catches IR regressions. Execution tests verify end-to-end correctness. ARC-specific tests verify borrow inference, RC insertion/elimination, and constructor reuse independently. `@test` functions compile and execute in both JIT and AOT modes.
