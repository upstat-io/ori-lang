---
section: "21A"
title: LLVM Backend
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 8
goal: JIT compilation and LLVM codegen for Ori language
sections:
  - id: "21.1"
    title: LLVM Setup & Infrastructure
    status: done
  - id: "21.2"
    title: Type Lowering
    status: in-progress
  - id: "21.3"
    title: Expression Codegen
    status: in-progress
  - id: "21.4"
    title: Operator Trait Dispatch
    status: in-progress
  - id: "21.5"
    title: Control Flow
    status: in-progress
  - id: "21.6"
    title: Pattern Matching
    status: in-progress
  - id: "21.7"
    title: Function Sequences & Expressions
    status: in-progress
  - id: "21.8"
    title: Concurrency Patterns
    status: not-started
  - id: "21.9"
    title: Capabilities & With Pattern
    status: not-started
  - id: "21.10"
    title: Collections & Iterators
    status: in-progress
  - id: "21.11"
    title: Lambda & Closure Support
    status: in-progress
  - id: "21.12"
    title: Built-in Functions
    status: in-progress
  - id: "21.13"
    title: FFI Support
    status: not-started
  - id: "21.14"
    title: Conditional Compilation
    status: not-started
  - id: "21.15"
    title: Memory Management (ARC)
    status: in-progress
  - id: "21.16"
    title: Optimization Passes
    status: in-progress
  - id: "21.17"
    title: Runtime Support
    status: in-progress
  - id: "21.18"
    title: Architecture (Reference)
    status: in-progress
  - id: "21.19"
    title: Section Completion Checklist
    status: in-progress
---

# Section 21A: LLVM Backend

## Current Test Results (verified 2026-04-06)

| Test Suite | Passed | Failed | Skipped/Ignored | Total |
|------------|--------|--------|-----------------|-------|
| Ori spec (evaluator) | 4409+ | 0 | 154 | 4563+ |
| Rust unit tests (ori_llvm lib) | 501+ | 0 | 0 | 501+ |
| AOT integration tests | 2096+ | 0 | 0 | 2096+ |
| ori_rt unit tests | 367+ | 0 | 0 | 367+ |
| Ori spec (LLVM) | 1781+ | 0 | 2475 LCFail | 4256+ |

## Known Blocker: BUG-04-030 — 2475 LCFails (high severity)

See `plans/bug-tracker/section-04-codegen-llvm.md:198` for full details. <!-- unblocks:jit-exception-handling/04B,05,06 -->

**Root causes** (6 identified, 3 remaining after JIT EH plan fixes):
- **(A) Generalized vars leak to codegen** — type checker stores unresolved vars in expr_types. ~279 occurrences. Touches §21.2 (Type Lowering) and §21.7 (Monomorphization).
- **(B) Index out of bounds (u32::MAX) in ARC IR** — missing block/var ID sentinel. ~50+ files. Touches §21.15 (ARC/Memory Management).
- **(C) StructValue vs IntValue type confusion** — 4 files. Touches §21.2 (Type Lowering).
- ~~(D) Missing JIT runtime functions~~ — fixed in JIT EH plan §06.1.
- ~~(E) Polymorphic type selection~~ — fixed in JIT EH plan §06.4.
- ~~(F) List concat calling convention~~ — fixed in JIT EH plan §06.5.

Resolving remaining root causes (A, B, C) would unblock completion of the JIT Exception Handling plan (Sections 04B, 05, 06).

## Import Resolution (Unified Pipeline)

The JIT test runner resolves imports via `oric::imports::resolve_imports()` and compiles
imported function bodies directly into the JIT module. This uses the same `declare_all` /
`define_all` path as main module functions, so **most new codegen features automatically
work for imported functions too**.

### Features that need import-aware changes

When implementing these features, ensure they also work across module boundaries:

- **Generic monomorphization**: IMPLEMENTED (verified 2026-03-29). 33 generics tests pass including cross-module `assert_eq` instantiation.

- **Impl blocks from imported modules**: IMPLEMENTED (verified 2026-03-29). Imported modules' impl blocks are compiled via `compile_impls()`.

- **Type declarations from imported modules**: IMPLEMENTED (verified 2026-03-29). `register_user_types()` processes imported modules' type declarations.

- **Prelude functions**: IMPLEMENTED (verified 2026-03-29). Sum type codegen works; prelude compilation re-enabled.

---

## 21.1 LLVM Setup & Infrastructure (verified 2026-03-29)

- [x] **Setup**: LLVM development environment
  - [x] Docker container with LLVM 21 and development headers
  - [x] Add `inkwell` crate to `compiler/ori_llvm/Cargo.toml`
  - [x] Verify LLVM bindings compile and link correctly
  - [x] Create `compiler/ori_llvm/src/` module structure

- [x] **Implement**: LLVM context and module initialization
  - [x] **Rust Tests**: `context.rs` — SimpleCx, CodegenCx, TypeCache
  - [x] Create LLVM context, module, and builder abstractions

- [x] **Implement**: Basic target configuration
  - [x] Support native target detection (JIT)
  - [x] Configure data layout and target features (AOT)

---

## 21.2 Type Lowering (verified 2026-03-29)

- [x] **Implement**: Primitive type mapping
  - [x] **Rust Tests**: `types.rs`, `context.rs` — type mapping
  - [x] Map Ori primitives (int → i64, float → f64, bool → i1, char → i32, byte → i8)
  - [x] Map strings to `{ i64 len, ptr data }` struct
  - [x] Handle function types

- [x] **Implement**: Option/Result types
  - [x] Map Option/Result to `{ i8 tag, i64 payload }` tagged unions
  - [x] Tag values: None=0, Some=1; Err=0, Ok=1
  - [x] Proper payload handling for non-primitive types

- [x] **Implement**: Collection type representations in `ori_llvm/src/codegen/` — list/map/set LLVM struct layouts
  - [x] Map lists to `{ i64 len, i64 cap, ptr data }` struct
  - [x] Map maps to appropriate hash table representation
  - [x] Map sets to appropriate hash set representation

- [ ] **Implement**: Duration & Size types
  - [x] Map Duration to i64 (nanoseconds)
  - [x] Map Size to i64 (bytes)
  - [x] Duration arithmetic operations (add, sub, mul, div, mod)
  - [x] Size arithmetic operations (add, sub, mul, div, mod)
  - [ ] Size cross-unit normalization — `512kb + 512kb == 1mb` equality fails in AOT (`test_op_size_arithmetic`)
  - [ ] Mixed-type arithmetic (Duration * int, int * Duration, Duration / Duration → int)
  - [ ] Size unary negation compile error
  - [ ] Overflow panic semantics for Duration
  - [ ] Division by zero panics
  - [ ] Duration methods: `.nanoseconds()`, `.microseconds()`, `.milliseconds()`, `.seconds()`, `.minutes()`, `.hours()`
  - [ ] Size methods: `.bytes()`, `.kilobytes()`, `.megabytes()`, `.gigabytes()`, `.terabytes()`
  - [ ] Factory functions: `Duration.from_nanoseconds(ns:)`, `Size.from_bytes(b:)`

- [x] **Implement**: Newtype codegen in `ori_llvm/src/codegen/` — transparent wrapper representation
  - [x] Newtype type representation (same as wrapped type)
  - [x] Constructor codegen: `TypeName(value)`
  - [x] `.inner` field access
  - [x] Proper type distinction for trait dispatch

- [x] **Implement**: Sum type codegen (beyond Option/Result)
  - [x] User-defined sum type representation (tagged union)
  - [x] Variant constructor codegen
  - [x] Tag-based dispatch in match
  - [x] Multi-field variant payloads
  - [x] **Import note**: Once sum types work, re-enable prelude function compilation in
        JIT mode (`runner.rs`). Currently skipped because `compare() -> Ordering` fails.

- [ ] **Implement**: Fixed-capacity lists `[T, max N]`
  - [ ] Inline allocation strategy
  - [ ] Compile-time capacity tracking
  - [ ] `.capacity()`, `.is_full()`, `.remaining()` methods
  - [ ] `.push()` with panic on full
  - [ ] `.try_push()` → `bool`
  - [ ] `.push_or_drop()`, `.push_or_oldest()`
  - [ ] `.to_dynamic()` conversion
  - [ ] `.to_fixed<$N>()` and `.try_to_fixed<$N>()`

- [ ] **Implement**: Channel type codegen — `Producer<T>`/`Consumer<T>` LLVM representations and runtime calls
  - [ ] `Producer<T>` and `Consumer<T>` type representation
  - [ ] `CloneableProducer<T>` and `CloneableConsumer<T>`
  - [ ] `Sendable` trait constraint checking
  - [ ] Channel buffer management

- [ ] **Subsection close-out (21.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.3 Expression Codegen (verified 2026-03-29)

- [x] **Implement**: Core expression codegen — literals, binary/unary ops, calls, field access in `ori_llvm/src/codegen/`
  - [x] **Rust Tests**: `tests/arithmetic_tests.rs`, `tests/operator_tests.rs`
  - [x] Literals (int, float, bool, string, char, byte)
  - [x] Binary ops (add, sub, mul, div, mod, comparisons, logical)
  - [x] Unary ops (neg, not)
  - [x] Function calls and method dispatch
  - [x] Field access and basic indexing

- [ ] **Implement**: Range expressions with step
  - [ ] `start..end by step` codegen
  - [ ] `start..=end by step` inclusive ranges
  - [ ] Negative step support (descending: `10..0 by -1`)
  - [ ] Infinite ranges: `0..`, `0.. by step`
  - [ ] Zero-step panic
  - [ ] Empty range detection (mismatched direction)

- [ ] **Implement**: Spread operator `...`
  - [ ] List spread: `[...a, x, ...b]`
  - [ ] Map spread: `{...a, key: v, ...b}`
  - [ ] Struct spread: `Point { ...orig, x: 10 }`
  - [ ] Later-wins merge semantics

- [ ] **Implement**: Coalesce operator `??`
  - [ ] `option ?? default` codegen
  - [ ] `result ?? default` codegen
  - [ ] Short-circuit evaluation
  - [ ] Type inference for coalesce chains

- [ ] **Implement**: Floor division operator `div`
  - [ ] Integer floor division codegen
  - [ ] Distinct from `/` (truncating division)

- [ ] **Implement**: Bitwise operator codegen — `~`/`&`/`|`/`^`/`<<`/`>>` for int and byte types
  - [ ] `~` (BitNot) codegen
  - [ ] `&`, `|`, `^` for int and byte types
  - [ ] `<<`, `>>` shift operators
  - [ ] Shift overflow panics (count < 0 or count >= bit width)

- [ ] **Implement**: Type conversion codegen — `as`/`as?`/`Into` trait dispatch in LLVM IR
  - [ ] `expr as Type` infallible conversion codegen
  - [ ] `expr as? Type` fallible conversion → `Option<T>`
  - [ ] Standard conversions: int→float, str→Error, Set<T>→[T]
  - [ ] Into trait method dispatch

- [ ] **Implement**: Assignment to complex targets
  - [ ] Field assignment: `point.x = 10`
  - [ ] Index assignment: `list[0] = value`
  - [ ] Nested assignments: `a.b.c = value`

- [x] **Implement**: String operation codegen — indexing, interpolation, escapes, methods via `ori_rt` calls (verified 2026-03-29)
  - [x] String indexing with Unicode handling (`str[i]` → single-codepoint `str`)
  - [x] String interpolation: `` `Hello {name}` ``
  - [x] All escape sequences: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`
  - [x] `str.concat()` return type tracking -- FIXED (`test_str_concat_basic` passes)
  - [x] `str.to_str()` identity return type tracking -- FIXED (`test_str_to_str_identity` passes)
  - [x] String ordering (`<`, `>`, `<=`, `>=`) -- FIXED (`test_str_compare_less` passes)
  - [x] String methods in builtin table: `contains`, `starts_with`, `ends_with`, `trim`, `to_uppercase`, `to_lowercase`, `replace`, `split`, `repeat`, `chars` -- FIXED (tests pass)
  - [x] Struct update (`...spread`) with string fields -- FIXED (`test_struct_update_with_string` passes)

- [ ] **Subsection close-out (21.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.4 Operator Trait Dispatch (verified 2026-03-29)

- [x] **Implement**: User-defined impl blocks and associated functions
  - [x] Register user-defined struct types with LLVM
  - [x] Support user-defined `impl Type { ... }` blocks
  - [x] Generate method dispatch for user-defined methods
  - [x] Support associated functions (methods without `self` parameter)
  - [x] Enable `Type.method()` syntax for user-defined types
  - [x] **Import note**: Compile imported modules' impl blocks via `compile_impls()`
  - [x] **Import note**: `register_user_types()` processes imported modules' type declarations
  - [x] **Tests**: `tests/spec/types/associated_functions.ori` (9 tests passing)

- [x] **Implement**: User-defined operator dispatch
  - [x] **Rust Tests**: `tests/operator_trait_tests.rs`
  - [x] Detect when operand is a user-defined type (struct)
  - [x] Generate method calls to trait methods instead of direct LLVM ops
  - [x] Arithmetic operators → `.add()`, `.subtract()`, `.multiply()`, `.divide()`, `.floor_divide()`, `.remainder()`
  - [x] Unary operators → `.negate()`, `.not()`, `.bit_not()`
  - [x] Bitwise operators → `.bit_and()`, `.bit_or()`, `.bit_xor()`, `.shift_left()`, `.shift_right()`
  - [x] Comparison operators → `.equals()`, `.compare()`
  - [x] Tuple `==` equality -- FIXED (`test_tuple_equality`, `test_tuple_equality_triple` pass)
  - [ ] Handle generic operator traits with type parameters (e.g., `Mul<int>`)
  - [x] **Files**: `ori_llvm/src/operators.rs` — trait dispatch logic
  - [ ] **Tests**: `tests/spec/traits/operators/user_defined.ori` (currently skipped)

- [x] **Implement**: Overflow and panic semantics
  - [x] Integer overflow detection and panic
  - [x] Division by zero panic for integers
  - [x] Shift count validation (negative or >= bit width → panic)
  - [x] Float special cases: Inf, NaN handling
  - [x] NaN comparison semantics (NaN > all values)

- [ ] **Subsection close-out (21.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.5 Control Flow (verified 2026-03-29)

- [x] **Implement**: Basic control flow
  - [x] **Rust Tests**: `tests/control_flow_tests.rs`, `tests/advanced_control_flow_tests.rs`
  - [x] Basic block creation and linking
  - [x] Phi nodes for SSA form
  - [x] Branch and conditional instructions
  - [x] Basic break/continue in loops

- [x] **Implement**: Labeled loops
  - [x] `loop:name` syntax support
  - [x] `for:name` syntax support
  - [x] `break:name` targeting specific loop
  - [x] `continue:name` targeting specific loop
  - [x] `continue:name value` in yield context
  - [x] No label shadowing enforcement

- [x] **Implement**: Break with values
  - [x] `break value` codegen
  - [x] Phi node setup in loop exit block
  - [x] Type resolution for loop expressions with break values
  - [x] `break:name value` for labeled loops

- [x] **Implement**: For-yield expressions
  - [x] `for x in items yield expr` → list collection
  - [x] `for x in items if guard yield expr` with filtering
  - [x] Nested for-yield: `for x in xs for y in ys yield (x, y)`
  - [x] `continue value` substitution in yield
  - [x] `break value` final element addition
  - [x] `{K: V}` collection from `(K, V)` tuples
  - [x] Bool comparison stored in struct field during for-yield -- FIXED (`test_depth_combined_struct_iter_match` passes)
  - [x] Mutation of outer variables inside match arms within for-do body -- FIXED (`test_depth_combined_match_closure_result` passes)

- [x] **Implement**: Try expression and error propagation
  - [x] `?` operator on Result: unwrap or early return
  - [x] `?` operator on Option: unwrap or early return None
  - [x] Proper error propagation up call stack
  - [x] Result accumulation in try blocks

- [x] **Implement**: Catch pattern
  - [x] `catch(expr:)` → `Result<T, str>` codegen
  - [x] Panic recovery mechanism
  - [x] Error message capturing
  - [ ] BUG: `catch()` type inference returns `Result<() -> T, str>` instead of `Result<T, str>` -- blocks 12 AOT tests (type checker bug, not LLVM)
  - [ ] BUG: Inline panic in `catch` -- `invoke` only intercepts callee-function panics, not same-function inline code (e.g., `1 / 0`) -- blocks 1 AOT test

- [ ] **Subsection close-out (21.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.6 Pattern Matching (verified 2026-03-29)

- [x] **Implement**: Basic patterns
  - [x] Literal patterns
  - [x] Binding patterns
  - [x] Wildcard patterns `_`
  - [x] Basic struct destructuring
  - [x] Basic tuple destructuring

- [x] **Implement**: Advanced patterns
  - [x] Range patterns: `1..10`, `'a'..'z'`
  - [x] Or-patterns: `A | B | C`
  - [x] At-patterns: `x @ Some(_)`
  - [x] Guard patterns: `x if x > 0`
  - [x] Struct destructuring with field renaming: `{ x: px, y: py }`
  - [x] List patterns with rest: `[first, ..rest]`, `[..init, last]`
  - [x] Nested patterns

- [ ] **Implement**: Match expression improvements
  - [ ] Exhaustiveness verification in codegen
  - [ ] Never variant handling
  - [ ] Pattern matrix optimization

- [ ] **Subsection close-out (21.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.7 Function Sequences & Expressions (verified 2026-03-29)

- [x] **Implement**: Generic function monomorphization (verified 2026-03-29 -- 33 generics tests pass)
  - [x] Collect monomorphization sites (call sites with concrete type args)
  - [x] Clone generic function bodies with type substitution
  - [x] Re-type-check monomorphized bodies to get concrete expr_types
  - [x] Compile specialized instances through normal declare/define path
  - [x] Import: collect call sites from calling module and generate specialized versions of imported generic functions (e.g., `assert_eq<int>`)
  - **Implementation plan**: Monomorphization (4 sections: type checker, ARC lowering, LLVM pipeline, verification)
  - **Architecture document**: `docs/ori_lang/v2026/design/monomorphization-architecture.md`

- [x] **Implement**: Basic function codegen
  - [x] **Rust Tests**: `tests/function_tests.rs`, `tests/function_call_tests.rs`
  - [x] Function signatures and calling conventions
  - [x] Local variables
  - [x] Return statements

- [x] **Implement**: Block expressions and control flow (`{ }`, `try`, `match`)
  - [x] `{ let x = a \n result }` sequential binding via block expressions
  - [ ] `pre(cond) post(r -> cond)` function-level contract declarations
  - [x] `try { let x = f()? \n Ok(x) }` error propagation
  - [x] `match v { P -> e, _ -> d }` pattern matching

- [x] **Fix**: Block scope variable isolation (verified 2026-03-29)
  - [x] Inner block `let x` leaks to outer scope -- FIXED (`test_scope_shadow_in_nested_block` passes)
  - [ ] `let x` in for-do body overwrites outer `x` (`test_scope_for_loop_shadow`) -- no test exists to verify

- [ ] **Implement**: Recurse pattern
  - [ ] `recurse(condition:, base:, step:)` basic recursion
  - [ ] `memo:` memoization support
  - [ ] `parallel:` parallel recursion flag
  - [ ] Proper state threading

- [ ] **Implement**: With pattern (RAII)
  - [ ] `with(acquire:, use:, release:)` codegen
  - [ ] Guaranteed release even on panic
  - [ ] Resource binding in `use:` block
  - [ ] **Note**: Evaluator has a loud stub in `can_eval.rs` — replace stub with real impl when ready

- [ ] **Subsection close-out (21.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.8 Concurrency Patterns

**Related Proposals:**
- `proposals/approved/parallel-execution-guarantees-proposal.md`
- `proposals/approved/timeout-spawn-patterns-proposal.md`
- `proposals/approved/nursery-cancellation-proposal.md`
- `proposals/approved/sendable-channels-proposal.md`
- `proposals/approved/cache-pattern-proposal.md`
- `proposals/approved/task-async-context-proposal.md`

- [ ] **Implement**: Parallel pattern
  - [ ] `parallel(tasks:, max_concurrent:, timeout:)` → `[Result]`
  - [ ] Task spawning using OS threads or async runtime
  - [ ] Result collection preserving task order
  - [ ] Timeout handling with cancellation propagation
  - [ ] `uses Suspend` capability requirement
  - [ ] `Sendable` bound on task return types
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/parallel_tests.rs`
  - [ ] **Evaluator**: Replace loud stub in `can_eval.rs:eval_can_function_exp` with real parallel eval

- [ ] **Implement**: Spawn pattern
  - [ ] `spawn(tasks:, max_concurrent:)` → `void`
  - [ ] Fire-and-forget semantics (errors logged, not propagated)
  - [ ] Background execution with structured lifetime
  - [ ] Tasks must complete before parent scope exits
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/spawn_tests.rs`
  - [ ] **Evaluator**: Replace loud stub in `can_eval.rs:eval_can_function_exp` with real spawn eval

- [ ] **Implement**: Timeout pattern
  - [ ] `timeout(op:, after:)` → `Result<T, TimeoutError>`
  - [ ] Operation cancellation on timeout via `is_cancelled()` checks
  - [ ] Cleanup: resources released, destructors run
  - [ ] Cooperative cancellation model (no preemption)
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/timeout_tests.rs`
  - [ ] **Evaluator**: Replace loud stub in `can_eval.rs:eval_can_function_exp` with real timeout eval

- [ ] **Implement**: Cache pattern
  - [ ] `cache(key:, op:, ttl:)` codegen
  - [ ] Cache key hashing via `Hashable` trait
  - [ ] TTL management with Duration type
  - [ ] Thread-safe cache access
  - [ ] `uses Cache` capability requirement
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/cache_tests.rs`
  - [ ] **Evaluator**: Replace loud stub in `can_eval.rs:eval_can_function_exp` with real cache eval

- [ ] **Implement**: Nursery pattern
  - [ ] `nursery(body:, on_error:, timeout:)` codegen
  - [ ] `NurseryErrorMode.CancelRemaining`: cancel siblings on first failure
  - [ ] `NurseryErrorMode.CollectAll`: run all, collect all errors
  - [ ] `NurseryErrorMode.FailFast`: return first error immediately
  - [ ] Structured concurrency: all children complete before nursery returns
  - [ ] Cancellation propagation via `CancellationError`
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/nursery_tests.rs`

- [ ] **Implement**: Channel operations
  - [ ] `channel<T>(buffer:)` → `(Producer, Consumer)` creation
  - [ ] `T: Sendable` bound enforced at compile time
  - [ ] `channel_in`, `channel_out`, `channel_all` selection patterns
  - [ ] Blocking send/receive operations
  - [ ] Bounded buffer with backpressure
  - [ ] `CloneableProducer<T>`, `CloneableConsumer<T>` variants
  - [ ] **Rust Tests**: `ori_llvm/src/concurrency/channel_tests.rs`

- [ ] **Implement**: Sendable trait enforcement
  - [ ] Check `Sendable` bound for all cross-task data
  - [ ] Compile error for non-Sendable types in parallel contexts
  - [ ] Interior mutability rules per `sendable-interior-mutability-proposal.md`

- [ ] **Subsection close-out (21.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.9 Capabilities & With Pattern

**Related Proposals:**
- `proposals/approved/capability-composition-proposal.md`
- `proposals/approved/with-pattern-proposal.md`
- `proposals/approved/default-impl-proposal.md`
- `proposals/approved/default-impl-resolution-proposal.md`
- `proposals/approved/intrinsics-capability-proposal.md`

- [ ] **Implement**: Capability tracking
  - [ ] `uses Capability` declaration in function signatures
  - [ ] Capability propagation through call graph
  - [ ] Compile error if capability not available at call site
  - [ ] Transitive capability requirements
  - [ ] **Rust Tests**: `ori_llvm/src/capabilities/tracking_tests.rs`

- [ ] **Implement**: Capability provision
  - [ ] `with Cap = impl in expr` codegen
  - [ ] Multiple capabilities: `with A = a, B = b in expr`
  - [ ] Provider vtable generation
  - [ ] Implicit capability parameter threading
  - [ ] Scope-based resolution (innermost `with` wins)
  - [ ] **Rust Tests**: `ori_llvm/src/capabilities/provision_tests.rs`

- [ ] **Implement**: Default implementations
  - [ ] `def impl` vtable generation at module level
  - [ ] Resolution order: with scope > imported def impl > local def impl
  - [ ] Override with explicit `with` pattern
  - [ ] `without def` import to exclude default impl
  - [ ] One def impl per trait per module
  - [ ] **Rust Tests**: `ori_llvm/src/capabilities/default_impl_tests.rs`

- [ ] **Implement**: Standard capabilities
  - [ ] `Print` (default provided, writes to stdout)
  - [ ] `Http` (network requests)
  - [ ] `FileSystem` (file I/O)
  - [ ] `Clock` (time queries)
  - [ ] `Random` (random number generation)
  - [ ] `Crypto` (cryptographic operations)
  - [ ] `Cache` (memoization and caching)
  - [ ] `Logger` (structured logging)
  - [ ] `Env` (environment variables)
  - [ ] `Intrinsics` (SIMD, bit ops per intrinsics-capability-proposal)
  - [ ] `Suspend` (async/concurrency marker)
  - [ ] `FFI` (foreign function interface)
  - [ ] **Rust Tests**: `ori_llvm/src/capabilities/standard_tests.rs`

- [ ] **Subsection close-out (21.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.10 Collections & Iterators

**Related Proposals:**
- `proposals/approved/iterator-traits-proposal.md`
- `proposals/approved/iterator-performance-semantics-proposal.md`
- `proposals/approved/computed-map-keys-proposal.md`
- `proposals/approved/fixed-capacity-list-proposal.md`

- [x] **Implement**: List operations (verified 2026-03-29 -- 82 collections_ext tests pass)
  - [x] `.push(element:)` - append to end, grow if needed (`test_coll_list_push`)
  - [x] `.pop()` → `Option<T>` - remove and return last element (`test_coll_list_pop`)
  - [x] `.first()` → `Option<T>` - return first element (`test_coll_list_first`)
  - [x] `.last()` → `Option<T>` - return last element (`test_coll_list_last`)
  - [x] `.insert(at:, element:)` - insert at index, shift elements
  - [x] `.remove(at:)` → `T` - remove at index, shift elements
  - [x] `.get(index:)` → `Option<T>` - safe indexed access
  - [x] `list[index]` → `T` - direct access (panics if out of bounds) (`test_coll_list_index`)
  - [x] `list[# - 1]` - length-relative indexing
  - [x] `.reverse()` - return reversed list (`test_coll_list_reverse`)
  - [x] `.contains(element:)` → `bool` (`test_coll_list_contains`)
  - [x] `.concat(other:)` -- FIXED (`test_aot_list_concat` passes)
  - [x] Capacity management: grow by 2x when full
  - [x] List iteration: `for x in list do ...`
  - [x] List-of-tuples iteration -- FIXED (`test_tuple_in_loop` passes)
  - [x] **Rust Tests**: `ori_llvm/src/collections/list_tests.rs`

- [x] **Implement**: Map operations (verified 2026-03-29 -- collections_ext tests pass)
  - [x] Map literal: `{key: value}`, `{"string": value}`
  - [ ] Computed keys: `{[expr]: value}` where expr is evaluated
  - [x] `.get(key:)` → `Option<V>` - lookup by key (`test_coll_map_get`)
  - [x] `map[key]` → `Option<V>` - subscript access
  - [x] `.insert(key:, value:)` - insert or update (`test_coll_map_insert`)
  - [x] `.remove(key:)` → `Option<V>` - remove and return (`test_coll_map_remove`)
  - [x] `.is_empty()` → `bool` (`test_aot_map_is_empty`)
  - [x] `.contains_key(key:)` → `bool` (`test_coll_map_contains_key`)
  - [x] `.keys()` → `[K]` - return list of keys (`test_coll_map_keys`)
  - [x] `.values()` → `[V]` - return list of values (`test_coll_map_values`)
  - [x] Map iteration: yields `(K, V)` tuples
  - [x] Spread in literals: `{...base, key: value}`
  - [x] **Rust Tests**: `ori_llvm/src/collections/map_tests.rs`

- [ ] **Implement**: Set operations [partial]
  - [x] `Set<T>` type representation: `{i64 len, i64 cap, ptr data}` (same as list — sorted flat array, not hash set)
  - [x] Set creation in AOT: `__collect_set` implemented — intercepts in `emit_apply`, calls `ori_iter_collect_set` runtime (2026-03-01)
  - [x] `.len()` → `emit_set_length()` — extract field 0
  - [x] `.is_empty()` → `emit_set_is_empty()` — `len == 0`
  - [x] `.contains(element:)` → `emit_set_contains()` via `ori_set_contains` runtime
  - [x] `.insert(element:)` → `emit_set_insert()` via `ori_set_insert` runtime (returns new set via sret)
  - [x] `.remove(element:)` → `emit_set_remove()` via `ori_set_remove` runtime (returns new set via sret)
  - [x] `.union(other:)` → `emit_set_union()` via `ori_set_union` runtime (sret)
  - [x] `.intersection(other:)` → `emit_set_intersection()` via `ori_set_intersection` runtime (sret)
  - [x] `.difference(other:)` → `emit_set_difference()` via `ori_set_difference` runtime (sret)
  - [x] `.to_list()` → `emit_set_to_list()` via `ori_set_to_list` runtime (sret)
  - [ ] Set iteration: yields `T` elements (set `.iter()` codegen)
  - [x] Runtime functions: 7 functions in `ori_rt/src/lib.rs` + declarations in `runtime_decl/mod.rs`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/sets.rs` — 10 tests all passing (2026-03-01)

- [ ] **Implement**: Iterator trait codegen (partial -- 25 iterator tests pass, verified 2026-03-29) <!-- unblocks:3.8 -->
  - [x] `Iterator` trait: `type Item; @next (self) -> (Option<Self.Item>, Self)`
  - [ ] `DoubleEndedIterator` trait: `@next_back (self) -> (Option<Self.Item>, Self)`
  - [x] Fused iterator semantics: `None` stays `None`
  - [x] `.map(transform:)` - lazy transformation
  - [x] `.filter(predicate:)` - lazy filtering
  - [x] `.fold(initial:, op:)` - eager reduction
  - [x] `.collect()` into target collection via `Collect` trait
  - [ ] `.enumerate()` - yield `(index, item)` tuples
  - [ ] `.zip(other:)` - pair with another iterator
  - [x] `.chain(other:)` - concatenate iterators
  - [x] `.take(count:)`, `.skip(count:)` - limit iteration
  - [ ] `.cycle()` - repeat infinitely
  - [ ] Copy elision: avoid intermediate allocations
  - [x] **Rust Tests**: `ori_llvm/src/collections/iterator_tests.rs`

- [ ] **Implement**: DoubleEndedIterator methods
  - [ ] `.rev()` - reverse iteration order
  - [ ] `.last()` - get last element
  - [ ] `.rfind(predicate:)` - find from end
  - [ ] `.rfold(initial:, op:)` - fold from end
  - [ ] **Rust Tests**: `ori_llvm/src/collections/double_ended_tests.rs`

- [ ] **Implement**: Infinite iterators
  - [ ] `repeat(value:)` → infinite iterator yielding value
  - [ ] `(0..).iter()` → infinite range from 0
  - [ ] `(0.. by -1).iter()` → infinite descending (requires bound)
  - [ ] Must use `.take(count:)` before `.collect()`
  - [ ] **Rust Tests**: `ori_llvm/src/collections/infinite_tests.rs`

- [ ] **Subsection close-out (21.10)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.10 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.10: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.11 Lambda & Closure Support (verified 2026-03-29)

- [x] **Implement**: Basic lambdas
  - [x] Simple lambda syntax: `x -> x + 1`
  - [x] Multi-param lambdas: `(a, b) -> a + b`
  - [x] No-param lambdas: `() -> 42`

- [x] **Implement**: Advanced lambda features
  - [x] Typed lambdas: `(x: int) -> int = x * 2`
  - [x] Proper capture-by-value semantics
  - [x] Nested lambdas
  - [x] Lambda in lambda captures
  - [ ] Default parameters in lambdas

- [x] **Implement**: Function references
  - [x] Function reference as value
  - [x] Higher-order function codegen
  - [x] Passing functions to other functions
  - [x] Named function → closure coercion -- FIXED (`test_hof_apply_identity`, `test_hof_named_fn_as_arg` pass)

- [x] **Fix**: Closure ABI issues (all fixed, verified 2026-03-29)
  - [x] `(int) -> bool` as function parameter -- FIXED (`test_hof_bool_lambda` passes)
  - [x] Zero-arg closure capturing 3+ strings -- FIXED (`test_depth_closure_capturing_multiple_strings` passes)
  - [x] `.map(r -> r.score)` closure accessing struct field -- FIXED (`test_stress_combined_struct_closure_iteration` passes)

- [ ] **Subsection close-out (21.11)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.11 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.11: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.12 Built-in Functions (verified 2026-03-29)

- [x] **Implement**: Basic built-ins
  - [x] `print(msg:)`
  - [x] `panic(msg:)` → `Never`
  - [x] Basic assertions

- [x] **Implement**: Comparison built-ins
  - [x] `compare(left:, right:)` → `Ordering`
  - [x] `min(left:, right:)` with Comparable
  - [x] `max(left:, right:)` with Comparable

- [x] **Implement**: Collection built-ins
  - [x] `len(collection:)` for all collection types
  - [x] `is_empty(collection:)`
  - [ ] `repeat(value:)` → infinite iterator
  - [ ] `hash_combine(seed:, value:)` → int

- [ ] **Implement**: Developer built-ins
  - [ ] `todo()` / `todo(reason:)` → `Never`
  - [ ] `unreachable()` / `unreachable(reason:)` → `Never`
  - [ ] `dbg(value:)` / `dbg(value:, label:)` → `T`
  - [ ] `drop_early(value:)`
  - [ ] `is_cancelled()` → `bool`

- [ ] **Implement**: Assertion built-ins
  - [ ] `assert_some(option:)`, `assert_none(option:)`
  - [ ] `assert_ok(result:)`, `assert_err(result:)`
  - [ ] `assert_panics(f:)`
  - [ ] `assert_panics_with(f:, msg:)`

- [x] **Implement**: Option/Result methods
  - [x] `.map(transform:)`, `.and_then(transform:)`
  - [x] `.unwrap_or(default:)`, `.filter(predicate:)`
  - [x] `.ok_or(error:)`, `.ok()`, `.err()`
  - [x] `.context(msg:)` for Result
  - [ ] `.trace()`, `.trace_entries()`, `.has_trace()`

- [x] **Fix**: Conversion method return type tracking (all fixed, verified 2026-03-29)
  - [x] `int.f()` result type -- FIXED (`test_conv_int_f_shorthand` passes)
  - [x] `int.byte()` -- FIXED (`test_conv_int_to_byte_truncates` passes)
  - [x] `int.byte().to_int()` conversion chain -- FIXED (`test_conv_int_to_byte`, `test_conv_int_to_byte_max`, `test_conv_int_to_byte_to_int` pass)

- [ ] **Subsection close-out (21.12)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.12 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.12: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.13 FFI Support

**Related Proposals:**
- `proposals/approved/platform-ffi-proposal.md`

- [ ] **Implement**: C FFI
  - [ ] `extern "c" from "lib" { ... }` declaration codegen
  - [ ] C type bindings: `c_char`, `c_short`, `c_int`, `c_long`, `c_longlong`
  - [ ] C type bindings: `c_float`, `c_double`, `c_size`
  - [ ] Function name mapping with `as "native_name"`
  - [ ] `CPtr` opaque pointer type (size_t sized)
  - [ ] `Option<CPtr>` for nullable pointers (None = null)
  - [ ] C variadic functions: `extern "c" { @printf (fmt: CPtr, ...) -> c_int }`
    - [ ] Parse variadic `...` in extern function declarations
    - [ ] LLVM codegen: emit variadic function type (`fn_type(..., true)`)
    - [ ] Argument promotion rules: `float` → `double`, `i8`/`i16` → `i32` (C ABI)
    - [ ] Variadic calls only allowed for extern declarations (not user-defined)
  - [ ] Library linking: `-l<lib>` flag generation
  - [ ] **Rust Tests**: `ori_llvm/src/ffi/c_ffi_tests.rs`

- [ ] **Implement**: Unsafe expressions
  - [ ] `unsafe { ... }` block codegen (same as inner expr, marker only)
  - [ ] `uses FFI` capability requirement at call sites
  - [ ] Pointer operations: `ptr_read<T>(ptr:)`, `ptr_write<T>(ptr:, value:)`
  - [ ] Pointer arithmetic (future)
  - [ ] **Rust Tests**: `ori_llvm/src/ffi/unsafe_tests.rs`

- [ ] **Implement**: JavaScript FFI (WASM target)
  - [ ] `extern "js" { ... }` declaration codegen
  - [ ] `extern "js" from "./file.js"` imports with path resolution
  - [ ] `JsValue` handle type (index into JS heap slab)
  - [ ] `JsPromise<T>` async handling
  - [ ] Implicit promise resolution at `let` binding sites
  - [ ] String marshalling: Ori str ↔ JS TextEncoder/TextDecoder
  - [ ] **Rust Tests**: `ori_llvm/src/ffi/js_ffi_tests.rs`

- [ ] **Implement**: Memory layout control
  - [ ] `#repr("c")` struct attribute
  - [ ] C-compatible struct layout (field order, padding, alignment)
  - [ ] Callback support: Ori functions as C function pointers
  - [ ] **Rust Tests**: `ori_llvm/src/ffi/layout_tests.rs`

- [ ] **Subsection close-out (21.13)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.13 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.13: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.14 Conditional Compilation

**Related Proposals:**
- `proposals/approved/conditional-compilation-proposal.md`

- [ ] **Implement**: Target conditionals
  - [ ] `#target(os: "linux")` - compile only if target OS matches
  - [ ] `#target(arch: "x86_64")` - compile only if arch matches
  - [ ] `#target(family: "unix")` - compile only if family matches
  - [ ] `any_os: ["linux", "macos"]` - compile if any match
  - [ ] `not_os: "windows"` - compile if OS doesn't match
  - [ ] File-level: `#!target(...)` at top of file
  - [ ] Non-matching branches not emitted to object file
  - [ ] **Rust Tests**: `ori_llvm/src/conditional/target_tests.rs`

- [ ] **Implement**: Config conditionals
  - [ ] `#cfg(debug)` - compile only in debug builds
  - [ ] `#cfg(release)` - compile only in release builds
  - [ ] `#cfg(feature: "name")` - compile if feature enabled
  - [ ] `any_feature: ["a", "b"]` - compile if any feature enabled
  - [ ] `not_feature: "x"` - compile if feature not enabled
  - [ ] `not_debug` - compile only in release
  - [ ] **Rust Tests**: `ori_llvm/src/conditional/config_tests.rs`

- [ ] **Implement**: Compile-time constants
  - [ ] `$target_os` → `"linux"` | `"macos"` | `"windows"` | ...
  - [ ] `$target_arch` → `"x86_64"` | `"aarch64"` | `"wasm32"` | ...
  - [ ] `$target_family` → `"unix"` | `"windows"` | `"wasm"`
  - [ ] `$debug` → `true` in debug builds, `false` otherwise
  - [ ] `$release` → `true` in release builds, `false` otherwise
  - [ ] False branch not type-checked (dead code elimination)
  - [ ] **Rust Tests**: `ori_llvm/src/conditional/const_tests.rs`

- [ ] **Implement**: Compile errors
  - [ ] `compile_error("msg")` emits error at compile time
  - [ ] Used with conditionals for unsupported configurations
  - [ ] **Rust Tests**: `ori_llvm/src/conditional/error_tests.rs`

- [ ] **Subsection close-out (21.14)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.14 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.14: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.15 Memory Management (ARC) (verified 2026-03-29 -- ~290+ ARC/RC tests pass)

**Related Proposals:**
- `proposals/approved/drop-trait-proposal.md`
- `proposals/approved/memory-model-edge-cases-proposal.md`
- `proposals/approved/clone-trait-proposal.md`

- [x] **Spec**: Type classification in `15-memory-model.md` updated to type-containment model (matches `ori_arc`)

- [x] **Implement**: Reference counting (verified 2026-03-29 -- 55 arc tests, 93 iter_rc_matrix tests, 50 rc_matrix tests pass)
  - [x] Heap allocation with atomic refcount header: `{ refcount: AtomicU64, data: T }`
  - [x] `fetch_add(1, Acquire)` on clone/share
  - [x] `fetch_sub(1, Release)` on drop
  - [x] Free when refcount reaches zero (after acquire fence)
  - [x] Stack-allocated values: no refcount (moved or copied)
  - [x] **Rust Tests**: `ori_llvm/src/arc/refcount_tests.rs`

- [x] **Spec**: Drop trait in `spec/08-types.md` § Drop Trait DONE
  - [x] Trait definition, execution timing, LIFO order
  - [x] Constraints (no async, must return void, panic during unwind = abort)
  - [x] drop_early built-in function

- [x] **Implement**: Drop trait codegen (verified 2026-03-29)
  - [x] Detect types implementing `Drop` trait
  - [x] Generate destructor call when refcount reaches zero
  - [x] Destructor runs before memory reclamation
  - [x] `@drop (self) -> void` signature
  - [ ] Drop cannot declare `uses Suspend` (compile error)
  - [x] **Rust Tests**: `ori_llvm/src/arc/drop_tests.rs`

- [ ] **Implement**: Destruction ordering
  - [ ] Local bindings: reverse declaration order
  - [ ] Struct fields: reverse declaration order
  - [ ] List elements: back-to-front (last element first)
  - [ ] Tuple elements: right-to-left `(a, b, c)` → c, b, a
  - [ ] Map entries: unspecified order (no guarantees)
  - [ ] **Rust Tests**: `ori_llvm/src/arc/order_tests.rs`

- [ ] **Implement**: Panic during destruction
  - [ ] Single panic in destructor: propagate normally
  - [ ] Other destructors still run after first panic
  - [ ] Double panic (destructor panics during unwind): immediate abort
  - [ ] Abort message: "panic during panic - aborting"
  - [ ] **Rust Tests**: `ori_llvm/src/arc/panic_tests.rs`

- [ ] **Implement**: Early drop
  - [ ] `drop_early(value:)` built-in
  - [ ] Immediately decrements refcount and runs destructor if zero
  - [ ] Value cannot be used after `drop_early`
  - [ ] Compile error if value used after drop
  - [ ] **Rust Tests**: `ori_llvm/src/arc/early_drop_tests.rs`

- [ ] **Implement**: Async destructor restriction
  - [ ] Compile error if Drop.drop declares `uses Suspend`
  - [ ] Error code and message for async destructor attempt

- [ ] **Implement**: Last-reference optimization (in-place mutation when RC=1) <!-- from: aot_codegen_pipeline §11.1 -->
  - [ ] Detect `isUnique(ref)` at runtime (refcount == 1)
  - [ ] When unique, reuse allocation for same-shape value instead of alloc+free
  - [ ] Applies to struct update, map insert/remove, list push/pop
  - [ ] Reference: Perceus paper §3.3 "Reuse Analysis", Koka `CheckFBIP.hs`
  - [ ] **Rust Tests**: `ori_llvm/tests/aot/arc.rs` — last-reference reuse tests

- [ ] **Implement**: Reset/reuse optimization (constructing same-shape value after match) <!-- from: aot_codegen_pipeline §11.1 -->
  - [ ] After pattern match deconstructs a value, reuse its allocation for newly constructed value of same type
  - [ ] Emit `reset`/`reuse` instructions in canonical IR (Lean 4 / Koka pattern)
  - [ ] Only applies when matched value has RC=1 (exclusive ownership)
  - [ ] Reference: Lean 4 `ExpandResetReuse.lean`, Perceus paper §3.4
  - [ ] **Rust Tests**: `ori_llvm/tests/aot/arc.rs` — reset/reuse tests

- [ ] **Subsection close-out (21.15)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.15 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.15: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.16 Optimization Passes (verified 2026-03-29)

### 21.16.1 Representation Optimization
**Proposal**: `proposals/approved/representation-optimization-proposal.md`

Formalize the compiler's freedom to optimize machine representations while preserving semantic equivalence. Tier 1 and most Tier 2 optimizations are already implemented. All [x] items verified 2026-03-29 via 65 type_info unit tests + 36 narrowing integration tests.

- [x] **Tier 1: Type-Intrinsic Narrowing** — `type_info/mod.rs`
  - [x] `bool` → `i1`, `byte` → `i8`, `char` → `i32`, `Ordering` → `i8`
  - [x] `void` → zero-sized or `i64(0)`
  - [x] Range inclusive flag → `i1`
- [x] **Tier 2a: Enum Discriminant Narrowing** — `type_info/mod.rs`
  - [x] All enum tags use `i8`
  - [ ] Dynamic tag width for >256 variants (i16/i32)
- [x] **Tier 2b: All-Unit Enum Elimination** — `type_info/mod.rs`
  - [x] Enums with no payload variants omit payload array
- [x] **Tier 2c: Sum Type Payload Sharing** — `lower_error_handling.rs`
  - [x] Result uses max(Ok, Err) payload slot
  - [x] Alloca+store+load coercion pattern with zero-initialization
- [x] **Tier 2d: ARC Elision** — `type_info/mod.rs`
  - [x] Transitive triviality classification with cycle detection
  - [x] Two-level check: per-type conservative + transitive walk
- [x] **Tier 2e: Newtype Erasure** — already specified in Types § Newtype
- [ ] **Tier 2f: Struct Field Reordering** — not implemented
  - [ ] Alignment-aware field ordering to reduce padding
  - [ ] Respect `#repr` attribute constraints
  - [ ] Maintain declaration-order mapping for derived traits
- [ ] **Tier 3: LLVM-Delegated Value Range Narrowing** — future
  - [ ] Emit `nsw`/`nuw` flags on overflow-checked arithmetic
  - [ ] Emit `range` metadata on loads with known value ranges
  - [ ] Emit `nonnull`/`dereferenceable` attributes on pointers
- [ ] **Known Gaps**:
  - [ ] ABI size computation ignores struct field padding (`abi/mod.rs`)
  - [ ] `TypeInfo::alignment()` returns hardcoded values, not representation-aware

### 21.16.2 LLVM Optimization Pipeline

- [ ] **Implement**: Optimization pipeline
  - [ ] Configure standard optimization levels (O0, O1, O2, O3)
  - [ ] Enable inlining, dead code elimination, constant folding
  - [ ] Loop optimizations
  - [ ] Tail call optimization

- [ ] **Implement**: Debug info generation
  - [ ] Source location tracking
  - [ ] Variable debug info
  - [ ] Type debug info
  - [ ] DWARF/CodeView emission

- [ ] **Subsection close-out (21.16)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.16 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.16: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.17 Runtime Support (verified 2026-03-29 -- 360 tests pass, 0 failures)

- [x] **Implement**: Basic runtime functions
  - [x] **Rust Tests**: `tests/runtime_tests.rs`
  - [x] `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool`
  - [x] `ori_panic`, `ori_panic_cstr`
  - [x] `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`
  - [x] `ori_str_concat`, `ori_str_eq`, `ori_str_ne`
  - [x] `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float`
  - [x] `ori_list_new`, `ori_list_free`, `ori_list_len`
  - [x] `ori_compare_int`, `ori_min_int`, `ori_max_int`

- [ ] **Implement**: Extended runtime
  - [x] Map runtime functions
  - [x] Set runtime functions
  - [ ] Channel runtime functions
  - [ ] Duration/Size runtime functions
  - [ ] Stack trace capture functions

- [ ] **Subsection close-out (21.17)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.17 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.17: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.18 Architecture (Reference)

The LLVM backend follows Rust's `rustc_codegen_llvm` patterns:

### Context Hierarchy

```rust
// Simple context - just LLVM types
pub struct SimpleCx<'ll> {
    pub llcx: &'ll Context,
    pub llmod: Module<'ll>,
    pub ptr_type: PointerType<'ll>,
    pub isize_ty: IntType<'ll>,
}

// Full context - adds Ori-specific state
pub struct CodegenCx<'ll, 'tcx> {
    pub scx: SimpleCx<'ll>,
    pub interner: &'tcx StringInterner,
    pub instances: RefCell<HashMap<Name, FunctionValue<'ll>>>,
    pub type_cache: RefCell<TypeCache<'ll>>,
}
```

### Builder Pattern

```rust
pub struct Builder<'a, 'll, 'tcx> {
    llbuilder: &'a inkwell::builder::Builder<'ll>,
    cx: &'a CodegenCx<'ll, 'tcx>,
}
```

### Directory Structure

```
ori_llvm/src/
├── lib.rs              # Crate root, re-exports
├── context.rs          # SimpleCx, CodegenCx, TypeCache
├── builder.rs          # Builder + expression compilation
├── types.rs            # Type mapping helpers
├── declare.rs          # Function declaration
├── traits.rs           # BackendTypes, BuilderMethods traits
├── module.rs           # ModuleCompiler (two-phase codegen)
├── runtime.rs          # Runtime FFI functions
├── evaluator.rs        # JIT evaluator (OwnedLLVMEvaluator)
├── operators.rs        # Binary/unary operator codegen
├── control_flow.rs     # if/else, loops, break/continue
├── matching.rs         # Pattern matching codegen
├── collections/        # Collection codegen (tuples, structs, lists)
│   ├── mod.rs
│   ├── tuples.rs
│   ├── structs.rs
│   ├── lists.rs
│   └── option_result.rs
├── functions/          # Function codegen
│   ├── mod.rs
│   ├── body.rs         # Function body compilation
│   ├── calls.rs        # Function call codegen
│   ├── lambdas.rs      # Lambda/closure codegen
│   ├── builtins.rs     # Built-in function codegen
│   ├── sequences.rs    # FunctionSeq (run, try, match)
│   └── expressions.rs  # FunctionExp (recurse, print, panic)
└── tests/              # Unit tests
    ├── mod.rs
    ├── arithmetic_tests.rs
    ├── collection_tests.rs
    ├── control_flow_tests.rs
    ├── function_tests.rs
    ├── matching_tests.rs
    └── ...
```

- [ ] **Subsection close-out (21.18)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.18 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.18: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 21.19 Section Completion Checklist (verified 2026-03-29)

**Infrastructure:**
- [x] JIT compilation working
- [ ] All current Ori tests pass via LLVM
- [x] All Rust unit tests pass (466/466 lib, 0 ignored; verified 2026-03-29)
- [x] Architecture follows Rust patterns
- [x] Unified import pipeline (imports resolved once, compiled into JIT module)
- [x] Generic monomorphization (verified 2026-03-29 -- 33 generics tests pass)
- [ ] AOT compilation (see Section 21B)

**Type System:**
- [x] Primitive types
- [x] Option/Result (basic)
- [x] Lists (basic)
- [ ] Duration/Size types (partial -- basic arithmetic works, methods not implemented)
- [x] Newtypes
- [x] Sum types (general)
- [ ] Fixed-capacity lists
- [ ] Channels
- [x] Maps (full support)
- [x] Sets

**Expressions:**
- [x] Basic expressions
- [x] Binary/unary operators (primitives)
- [ ] Range with step (`by`)
- [ ] Spread operator (`...`) (struct spread works; list/map spread status unclear)
- [ ] Coalesce operator (`??`)
- [ ] Floor division (`div`)
- [x] Type conversions (`as`, `as?`)
- [x] Complex assignments

**Control Flow:**
- [x] Basic if/else, loops
- [x] Labeled loops
- [x] Break with values
- [x] For-yield expressions
- [x] Try/catch patterns

**Traits & Dispatch:**
- [x] Associated functions
- [x] Operator trait dispatch
- [x] Iterator trait methods (partial -- map/filter/fold/collect/take/chain work)
- [x] Comparison trait methods

**Concurrency:**
- [ ] Parallel pattern
- [ ] Spawn pattern
- [ ] Channels
- [ ] Nursery pattern

**Capabilities:**
- [ ] Capability tracking
- [ ] With pattern provision
- [ ] Default implementations

**FFI:**
- [ ] C FFI
- [ ] JavaScript FFI
- [ ] Unsafe blocks

**Memory:**
- [x] ARC implementation
- [x] Drop trait
- [ ] Destruction ordering

**Optimization:**
- [x] Representation optimization (Tiers 1-2e done)
- [ ] LLVM optimization pipeline
- [ ] Debug info generation

**Verified AOT Gaps** (from Section 11.1 verification, updated 2026-02-23):

*Resolved:*
- [x] Derive Eq struct codegen: `emit_comparison_via_trait` now dispatches to derived `eq` method for non-primitive types instead of falling through to scalar `icmp` <!-- test: test_aot_derive_eq_struct (un-ignored), test_aot_derive_eq_struct_not_equal, test_aot_derive_eq_struct_with_strings -->
- [x] Derive Comparable struct codegen: `emit_ordering_comparison` now dispatches to derived `compare` method and checks Ordering result (0=Less, 1=Equal, 2=Greater) for `<`/`>`/`<=`/`>=` <!-- test: test_aot_derive_comparable_struct -->
- [x] ARC enum basic drop and string-payload drop: `test_arc_enum_basic_drop` and `test_arc_enum_with_string_payload` un-ignored and passing

*Resolved (verified 2026-03-29):*
- [x] List mutation methods in AOT builtin table -- FIXED (`test_aot_list_push`, `test_aot_list_first_last` pass)
- [x] Map methods in AOT builtin table -- FIXED (`test_aot_map_is_empty` passes)
- [x] `list[index]` subscript resolved -- FIXED (`test_aot_list_index` passes)
- [x] Closure-returning-closure type inference -- FIXED (`test_aot_closure_capturing_closure` passes)
- [x] Generic monomorphization in ARC pipeline -- FIXED (33 generics tests pass)
- [x] `catch(expr:)` lowered through ARC pipeline -- FIXED (9 catch tests pass)
- [x] String interpolation -- FIXED (`test_aot_string_interpolation` passes)

*Open:*
- [ ] Enum variant constructors not declared as LLVM functions <!-- test: test_aot_enum_variant_constructors --> (no test found by listed name, but many enum tests pass via derives and pattern matching)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

Cross-references to existing items:
- Generic monomorphization → § 21.7 -- RESOLVED (verified 2026-03-29)
- Enum variant constructors → § 21.2 (blocks enums, recursive types)
- `catch(expr:)` → § 21.5 -- RESOLVED (verified 2026-03-29)
- String interpolation → § 21.3 -- RESOLVED (verified 2026-03-29)

**New AOT tests added** (2026-02-23, 13 tests, all passing):
derive_eq_struct (un-ignored), derive_eq_struct_not_equal, derive_eq_struct_with_strings,
derive_comparable_struct (4 comparisons: <, >, <=, >=), panic_basic, option_unwrap_some,
result_unwrap_ok, struct_with_list_and_string, nested_struct_with_strings,
for_yield_with_filter, for_yield_transform

**New AOT test files added** (2026-02-23, Section 11.1 verification — 5 files, 117 tests total):
- `error_handling.rs` — 21 tests (21 pass, 0 ignore): Result/Option basics, `?` operator, chaining, deep chains
- `higher_order.rs` — 27 tests (23 pass, 4 ignore): HOF apply, composition, closures with captures, nested closures, fold
- `tuples.rs` — 29 tests (23 pass, 6 ignore): construction, destructuring, field access, as param/return, nested, strings, closures
- `structs.rs` — 29 tests (28 pass, 1 ignore): construction, string fields, update syntax, nested, as param/return, closures, derived Eq
- `recursion.rs` — 22 tests (22 pass, 0 ignore): factorial, fibonacci, tail-recursive, mutual, with Result/Option/match, depth tests

**Total AOT test counts** (verified 2026-03-29): 1977 passed, 0 failed, 17 ignored

**Remaining known bugs** (verified 2026-03-29):
1. **Catch type inference**: `catch()` returns `Result<() -> T, str>` instead of `Result<T, str>` -- blocks 12 AOT tests (type checker bug, not LLVM)
2. **Inline panic in catch**: `invoke` only intercepts callee-function panics, not same-function inline code -- blocks 1 test
3. **Chained tuple field access**: `t.0.1` lexed as float by parser -- blocks 2 tests (parser bug, not LLVM; tracked in Section 05)
4. **Nounwind analysis**: does not distinguish may-unwind monomorphized callees -- blocks 1 test

**Exit Criteria**: Feature parity with evaluator for all language constructs

- [ ] **Subsection close-out (21.19)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-21.19 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 21.19: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## Running Tests

```bash
# LLVM is a default feature — no Docker required (verified 2026-03-29)
# Build all (includes LLVM)
cargo b

# Run all tests (Rust unit + AOT integration + spec)
./test-all.sh

# Run LLVM-specific tests
cargo test -p ori_llvm --lib    # 466 unit tests
cargo test -p ori_llvm --test aot_tests  # 1977 AOT integration tests

# Run spec tests only
cargo st

# Run with debug output
ORI_DEBUG_LLVM=1 ori test tests/spec/types/primitives.ori

# Legacy Docker workflow still works but is not required:
# ./docker/llvm/build.sh && ./docker/llvm/run.sh ori test
```
