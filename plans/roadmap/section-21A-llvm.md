---
section: "21A"
title: LLVM Backend
status: in-progress
reviewed: false
tier: 8
goal: JIT compilation and LLVM codegen for Ori language
sections:
  - id: "21.1"
    title: LLVM Setup & Infrastructure
    status: in-progress
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
    status: not-started
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

## Current Test Results

| Test Suite | Passed | Failed | Skipped | LCFail | Total |
|------------|--------|--------|---------|--------|-------|
| Ori spec (evaluator) | 3035 | 0 | 42 | - | 3077 |
| Ori spec (LLVM backend) | 1082 | 1 | 9 | 1985 | 3077 |
| Rust unit tests (LLVM) | 527 | 0 | 15 | - | 542 |

## Import Resolution (Unified Pipeline)

The JIT test runner resolves imports via `oric::imports::resolve_imports()` and compiles
imported function bodies directly into the JIT module. This uses the same `declare_all` /
`define_all` path as main module functions, so **most new codegen features automatically
work for imported functions too**.

### Features that need import-aware changes

When implementing these features, ensure they also work across module boundaries:

- **Generic monomorphization**: `declare_all` skips generics (`sig.is_generic()`). To
  compile `assert_eq<T: Eq>(actual: int, expected: int)`, the monomorphization pass must
  collect concrete instantiation sites from the *calling* module and generate specialized
  versions of imported generic functions. This is the single largest gap — `assert_eq` is
  used in 2,472 test call sites.

- **Impl blocks from imported modules**: Currently only top-level functions are compiled
  from imports. If an imported module has `impl Type { @method ... }`, those methods need
  `compile_impls()` processing with the imported module's impl_sigs.

- **Type declarations from imported modules**: `register_user_types()` only processes the
  main module's types. Imported struct/sum type definitions need registration so the LLVM
  type resolver can compute their layouts.

- **Prelude functions**: Currently skipped in JIT mode because some prelude functions
  (e.g., `compare` returning `Ordering`) use types the codegen can't handle yet. Once sum
  type codegen (21.2) works, prelude compilation should be re-enabled.

---

## 21.1 LLVM Setup & Infrastructure

- [ ] **Setup**: LLVM development environment
  - [ ] Docker container with LLVM 21 and development headers
  - [ ] Add `inkwell` crate to `compiler/ori_llvm/Cargo.toml`
  - [ ] Verify LLVM bindings compile and link correctly
  - [ ] Create `compiler/ori_llvm/src/` module structure

- [ ] **Implement**: LLVM context and module initialization
  - [ ] **Rust Tests**: `context.rs` — SimpleCx, CodegenCx, TypeCache
  - [ ] Create LLVM context, module, and builder abstractions

- [ ] **Implement**: Basic target configuration
  - [ ] Support native target detection (JIT)
  - [ ] Configure data layout and target features (AOT)

---

## 21.2 Type Lowering

- [ ] **Implement**: Primitive type mapping
  - [ ] **Rust Tests**: `types.rs`, `context.rs` — type mapping
  - [ ] Map Ori primitives (int → i64, float → f64, bool → i1, char → i32, byte → i8)
  - [ ] Map strings to `{ i64 len, ptr data }` struct
  - [ ] Handle function types

- [ ] **Implement**: Option/Result types
  - [ ] Map Option/Result to `{ i8 tag, i64 payload }` tagged unions
  - [ ] Tag values: None=0, Some=1; Err=0, Ok=1
  - [ ] Proper payload handling for non-primitive types

- [ ] **Implement**: Collection type representations in `ori_llvm/src/codegen/` — list/map/set LLVM struct layouts
  - [ ] Map lists to `{ i64 len, i64 cap, ptr data }` struct
  - [ ] Map maps to appropriate hash table representation
  - [ ] Map sets to appropriate hash set representation

- [ ] **Implement**: Duration & Size types
  - [ ] Map Duration to i64 (nanoseconds)
  - [ ] Map Size to i64 (bytes)
  - [ ] Duration arithmetic operations (add, sub, mul, div, mod)
  - [ ] Size arithmetic operations (add, sub, mul, div, mod)
  - [ ] Size cross-unit normalization — `512kb + 512kb == 1mb` equality fails in AOT (`test_op_size_arithmetic`)
  - [ ] Mixed-type arithmetic (Duration * int, int * Duration, Duration / Duration → int)
  - [ ] Size unary negation compile error
  - [ ] Overflow panic semantics for Duration
  - [ ] Division by zero panics
  - [ ] Duration methods: `.nanoseconds()`, `.microseconds()`, `.milliseconds()`, `.seconds()`, `.minutes()`, `.hours()`
  - [ ] Size methods: `.bytes()`, `.kilobytes()`, `.megabytes()`, `.gigabytes()`, `.terabytes()`
  - [ ] Factory functions: `Duration.from_nanoseconds(ns:)`, `Size.from_bytes(b:)`

- [ ] **Implement**: Newtype codegen in `ori_llvm/src/codegen/` — transparent wrapper representation
  - [ ] Newtype type representation (same as wrapped type)
  - [ ] Constructor codegen: `TypeName(value)`
  - [ ] `.inner` field access
  - [ ] Proper type distinction for trait dispatch

- [ ] **Implement**: Sum type codegen (beyond Option/Result)
  - [ ] User-defined sum type representation (tagged union)
  - [ ] Variant constructor codegen
  - [ ] Tag-based dispatch in match
  - [ ] Multi-field variant payloads
  - [ ] **Import note**: Once sum types work, re-enable prelude function compilation in
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

---

## 21.3 Expression Codegen

- [ ] **Implement**: Core expression codegen — literals, binary/unary ops, calls, field access in `ori_llvm/src/codegen/`
  - [ ] **Rust Tests**: `tests/arithmetic_tests.rs`, `tests/operator_tests.rs`
  - [ ] Literals (int, float, bool, string, char, byte)
  - [ ] Binary ops (add, sub, mul, div, mod, comparisons, logical)
  - [ ] Unary ops (neg, not)
  - [ ] Function calls and method dispatch
  - [ ] Field access and basic indexing

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

- [ ] **Implement**: String operation codegen — indexing, interpolation, escapes, methods via `ori_rt` calls
  - [ ] String indexing with Unicode handling (`str[i]` → single-codepoint `str`)
  - [ ] String interpolation: `` `Hello {name}` ``
  - [ ] All escape sequences: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`
  - [ ] `str.concat()` return type tracking — result not tracked as str, downstream comparison uses icmp (`test_str_concat_basic`)
  - [ ] `str.to_str()` identity return type tracking — same icmp issue (`test_str_to_str_identity`)
  - [ ] String ordering (`<`, `>`, `<=`, `>=`) — uses icmp instead of compare runtime call (`test_str_compare_less`)
  - [ ] String methods not in builtin table: `contains`, `starts_with`, `ends_with`, `trim`, `to_uppercase`, `to_lowercase`, `replace`, `split`, `repeat`, `chars` (`test_str_contains` et al.)
  - [ ] Struct update (`...spread`) with string fields — GEP on non-pointer value causes segfault (`test_struct_update_with_string`)

---

## 21.4 Operator Trait Dispatch

- [ ] **Implement**: User-defined impl blocks and associated functions
  - [ ] Register user-defined struct types with LLVM
  - [ ] Support user-defined `impl Type { ... }` blocks
  - [ ] Generate method dispatch for user-defined methods
  - [ ] Support associated functions (methods without `self` parameter)
  - [ ] Enable `Type.method()` syntax for user-defined types
  - [ ] **Import note**: When impl blocks work, also compile imported modules' impl blocks
        via `compile_impls()`. Currently only top-level functions are compiled from imports.
  - [ ] **Import note**: `register_user_types()` must also process imported modules' type
        declarations so the LLVM type resolver can compute their layouts.
  - [ ] **Tests**: `tests/spec/types/associated_functions.ori` (9 tests passing)

- [ ] **Implement**: User-defined operator dispatch
  - [ ] **Rust Tests**: `tests/operator_trait_tests.rs`
  - [ ] Detect when operand is a user-defined type (struct)
  - [ ] Generate method calls to trait methods instead of direct LLVM ops
  - [ ] Arithmetic operators → `.add()`, `.subtract()`, `.multiply()`, `.divide()`, `.floor_divide()`, `.remainder()`
  - [ ] Unary operators → `.negate()`, `.not()`, `.bit_not()`
  - [ ] Bitwise operators → `.bit_and()`, `.bit_or()`, `.bit_xor()`, `.shift_left()`, `.shift_right()`
  - [ ] Comparison operators → `.equals()`, `.compare()`
  - [ ] Tuple `==` equality — compiles but returns wrong result; comparison codegen incorrect (`test_tuple_equality`, `test_tuple_equality_triple`)
  - [ ] Handle generic operator traits with type parameters (e.g., `Mul<int>`)
  - [ ] **Files**: `ori_llvm/src/operators.rs` — add trait dispatch logic
  - [ ] **Tests**: `tests/spec/traits/operators/user_defined.ori` (currently skipped)

- [ ] **Implement**: Overflow and panic semantics
  - [ ] Integer overflow detection and panic
  - [ ] Division by zero panic for integers
  - [ ] Shift count validation (negative or >= bit width → panic)
  - [ ] Float special cases: Inf, NaN handling
  - [ ] NaN comparison semantics (NaN > all values)

---

## 21.5 Control Flow

- [ ] **Implement**: Basic control flow
  - [ ] **Rust Tests**: `tests/control_flow_tests.rs`, `tests/advanced_control_flow_tests.rs`
  - [ ] Basic block creation and linking
  - [ ] Phi nodes for SSA form
  - [ ] Branch and conditional instructions
  - [ ] Basic break/continue in loops

- [ ] **Implement**: Labeled loops
  - [ ] `loop:name` syntax support
  - [ ] `for:name` syntax support
  - [ ] `break:name` targeting specific loop
  - [ ] `continue:name` targeting specific loop
  - [ ] `continue:name value` in yield context
  - [ ] No label shadowing enforcement

- [ ] **Implement**: Break with values
  - [ ] `break value` codegen
  - [ ] Phi node setup in loop exit block
  - [ ] Type resolution for loop expressions with break values
  - [ ] `break:name value` for labeled loops

- [ ] **Implement**: For-yield expressions
  - [ ] `for x in items yield expr` → list collection
  - [ ] `for x in items if guard yield expr` with filtering
  - [ ] Nested for-yield: `for x in xs for y in ys yield (x, y)`
  - [ ] `continue value` substitution in yield
  - [ ] `break value` final element addition
  - [ ] `{K: V}` collection from `(K, V)` tuples
  - [ ] Bool comparison stored in struct field during for-yield always produces false (`test_depth_combined_struct_iter_match`)
  - [ ] Mutation of outer variables inside match arms within for-do body — ArcIrEmitter: variable not yet defined (`test_depth_combined_match_closure_result`)

- [ ] **Implement**: Try expression and error propagation
  - [ ] `?` operator on Result: unwrap or early return
  - [ ] `?` operator on Option: unwrap or early return None
  - [ ] Proper error propagation up call stack
  - [ ] Result accumulation in try blocks

- [ ] **Implement**: Catch pattern
  - [ ] `catch(expr:)` → `Result<T, str>` codegen
  - [ ] Panic recovery mechanism
  - [ ] Error message capturing

---

## 21.6 Pattern Matching

- [ ] **Implement**: Basic patterns
  - [ ] Literal patterns
  - [ ] Binding patterns
  - [ ] Wildcard patterns `_`
  - [ ] Basic struct destructuring
  - [ ] Basic tuple destructuring

- [ ] **Implement**: Advanced patterns
  - [ ] Range patterns: `1..10`, `'a'..'z'`
  - [ ] Or-patterns: `A | B | C`
  - [ ] At-patterns: `x @ Some(_)`
  - [ ] Guard patterns: `x if x > 0`
  - [ ] Struct destructuring with field renaming: `{ x: px, y: py }`
  - [ ] List patterns with rest: `[first, ..rest]`, `[..init, last]`
  - [ ] Nested patterns

- [ ] **Implement**: Match expression improvements
  - [ ] Exhaustiveness verification in codegen
  - [ ] Never variant handling
  - [ ] Pattern matrix optimization

---

## 21.7 Function Sequences & Expressions

- [ ] **Implement**: Generic function monomorphization
  - [ ] Collect monomorphization sites (call sites with concrete type args)
  - [ ] Clone generic function bodies with type substitution
  - [ ] Re-type-check monomorphized bodies to get concrete expr_types
  - [ ] Compile specialized instances through normal declare/define path
  - [ ] **Import note**: Must collect call sites from the *calling* module and generate
        specialized versions of imported generic functions (e.g., `assert_eq<int>`).
        Currently `declare_all` skips all generics — this is the single biggest gap
        for LLVM test coverage (affects 2,472+ `assert_eq` call sites).
  - **Implementation plan**: Monomorphization (4 sections: type checker, ARC lowering, LLVM pipeline, verification)
  - **Architecture document**: `docs/ori_lang/v2026/design/monomorphization-architecture.md`

- [ ] **Implement**: Basic function codegen
  - [ ] **Rust Tests**: `tests/function_tests.rs`, `tests/function_call_tests.rs`
  - [ ] Function signatures and calling conventions
  - [ ] Local variables
  - [ ] Return statements

- [ ] **Implement**: Block expressions and control flow (`{ }`, `try`, `match`)
  - [ ] `{ let x = a \n result }` sequential binding via block expressions
  - [ ] `pre(cond) post(r -> cond)` function-level contract declarations
  - [ ] `try { let x = f()? \n Ok(x) }` error propagation
  - [ ] `match v { P -> e, _ -> d }` pattern matching

- [ ] **Fix**: Block scope variable isolation
  - [ ] Inner block `let x` leaks to outer scope — shadow bindings overwrite outer variables instead of creating block-local scope (`test_scope_shadow_in_nested_block`, `test_scope_shadow_in_nested_block_2`)
  - [ ] `let x` in for-do body overwrites outer `x` (`test_scope_for_loop_shadow`)

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

---

## 21.10 Collections & Iterators

**Related Proposals:**
- `proposals/approved/iterator-traits-proposal.md`
- `proposals/approved/iterator-performance-semantics-proposal.md`
- `proposals/approved/computed-map-keys-proposal.md`
- `proposals/approved/fixed-capacity-list-proposal.md`

- [ ] **Implement**: List operations
  - [ ] `.push(element:)` - append to end, grow if needed (`test_coll_list_push`)
  - [ ] `.pop()` → `Option<T>` - remove and return last element (`test_coll_list_pop`)
  - [ ] `.first()` → `Option<T>` - return first element (`test_coll_list_first`)
  - [ ] `.last()` → `Option<T>` - return last element (`test_coll_list_last`)
  - [ ] `.insert(at:, element:)` - insert at index, shift elements
  - [ ] `.remove(at:)` → `T` - remove at index, shift elements
  - [ ] `.get(index:)` → `Option<T>` - safe indexed access
  - [ ] `list[index]` → `T` - direct access (panics if out of bounds) (`test_coll_list_index`)
  - [ ] `list[# - 1]` - length-relative indexing
  - [ ] `.reverse()` - return reversed list (`test_coll_list_reverse`)
  - [ ] `.contains(element:)` → `bool` (`test_coll_list_contains`)
  - [ ] `.concat(other:)` — produces wrong result (`test_aot_list_concat`)
  - [ ] Capacity management: grow by 2x when full
  - [ ] List iteration: `for x in list do ...`
  - [ ] List-of-tuples iteration — heap corruption when iterating `[(1, 10), (2, 20)]` (`test_tuple_in_loop`)
  - [ ] **Rust Tests**: `ori_llvm/src/collections/list_tests.rs`

- [ ] **Implement**: Map operations
  - [ ] Map literal: `{key: value}`, `{"string": value}`
  - [ ] Computed keys: `{[expr]: value}` where expr is evaluated
  - [ ] `.get(key:)` → `Option<V>` - lookup by key (`test_coll_map_get`)
  - [ ] `map[key]` → `Option<V>` - subscript access
  - [ ] `.insert(key:, value:)` - insert or update (`test_coll_map_insert`)
  - [ ] `.remove(key:)` → `Option<V>` - remove and return (`test_coll_map_remove`)
  - [ ] `.is_empty()` → `bool` (`test_aot_map_is_empty`)
  - [ ] `.contains_key(key:)` → `bool` (`test_coll_map_contains_key`)
  - [ ] `.keys()` → `[K]` - return list of keys (`test_coll_map_keys`)
  - [ ] `.values()` → `[V]` - return list of values (`test_coll_map_values`)
  - [ ] Map iteration: yields `(K, V)` tuples
  - [ ] Spread in literals: `{...base, key: value}`
  - [ ] **Rust Tests**: `ori_llvm/src/collections/map_tests.rs`

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

- [ ] **Implement**: Iterator trait codegen <!-- unblocks:3.8 -->
  - [ ] `Iterator` trait: `type Item; @next (self) -> (Option<Self.Item>, Self)`
  - [ ] `DoubleEndedIterator` trait: `@next_back (self) -> (Option<Self.Item>, Self)`
  - [ ] Fused iterator semantics: `None` stays `None`
  - [ ] `.map(transform:)` - lazy transformation
  - [ ] `.filter(predicate:)` - lazy filtering
  - [ ] `.fold(initial:, op:)` - eager reduction
  - [ ] `.collect()` into target collection via `Collect` trait
  - [ ] `.enumerate()` - yield `(index, item)` tuples
  - [ ] `.zip(other:)` - pair with another iterator
  - [ ] `.chain(other:)` - concatenate iterators
  - [ ] `.take(count:)`, `.skip(count:)` - limit iteration
  - [ ] `.cycle()` - repeat infinitely
  - [ ] Copy elision: avoid intermediate allocations
  - [ ] **Rust Tests**: `ori_llvm/src/collections/iterator_tests.rs`

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

---

## 21.11 Lambda & Closure Support

- [ ] **Implement**: Basic lambdas
  - [ ] Simple lambda syntax: `x -> x + 1`
  - [ ] Multi-param lambdas: `(a, b) -> a + b`
  - [ ] No-param lambdas: `() -> 42`

- [ ] **Implement**: Advanced lambda features
  - [ ] Typed lambdas: `(x: int) -> int = x * 2`
  - [ ] Proper capture-by-value semantics
  - [ ] Nested lambdas
  - [ ] Lambda in lambda captures
  - [ ] Default parameters in lambdas

- [ ] **Implement**: Function references
  - [ ] Function reference as value
  - [ ] Higher-order function codegen
  - [ ] Passing functions to other functions
  - [ ] Named function → closure coercion (named fns are `i64` function pointers, closure params expect `{ ptr, ptr }` struct) — `test_hof_apply_identity`, `test_hof_apply_named_function`, `test_hof_named_fn_as_arg`

- [ ] **Fix**: Closure ABI issues
  - [ ] `(int) -> bool` as function parameter — bool return ABI unresolved in closure-typed params (`test_hof_bool_lambda`)
  - [ ] Zero-arg closure capturing 3+ strings causes heap corruption — `malloc(): corrupted top size` (`test_depth_closure_capturing_multiple_strings`)
  - [ ] `.map(r -> r.score)` closure accessing struct field produces invalid LLVM IR (`test_stress_combined_struct_closure_iteration`)

---

## 21.12 Built-in Functions

- [ ] **Implement**: Basic built-ins
  - [ ] `print(msg:)`
  - [ ] `panic(msg:)` → `Never`
  - [ ] Basic assertions

- [ ] **Implement**: Comparison built-ins
  - [ ] `compare(left:, right:)` → `Ordering`
  - [ ] `min(left:, right:)` with Comparable
  - [ ] `max(left:, right:)` with Comparable

- [ ] **Implement**: Collection built-ins
  - [ ] `len(collection:)` for all collection types
  - [ ] `is_empty(collection:)`
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

- [ ] **Implement**: Option/Result methods
  - [ ] `.map(transform:)`, `.and_then(transform:)`
  - [ ] `.unwrap_or(default:)`, `.filter(predicate:)`
  - [ ] `.ok_or(error:)`, `.ok()`, `.err()`
  - [ ] `.context(msg:)` for Result
  - [ ] `.trace()`, `.trace_entries()`, `.has_trace()`

- [ ] **Fix**: Conversion method return type tracking
  - [ ] `int.f()` result type not tracked as float — `icmp` used instead of `fcmp` for comparison (`test_conv_int_f_shorthand`)
  - [ ] `int.byte()` — byte type not fully supported in AOT pipeline (`test_conv_int_to_byte_truncates`)
  - [ ] `int.byte().to_int()` — type tracking lost through conversion chain (`test_conv_int_to_byte`, `test_conv_int_to_byte_max`, `test_conv_int_to_byte_to_int`)

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

---

## 21.15 Memory Management (ARC)

**Related Proposals:**
- `proposals/approved/drop-trait-proposal.md`
- `proposals/approved/memory-model-edge-cases-proposal.md`
- `proposals/approved/clone-trait-proposal.md`

- [x] **Spec**: Type classification in `15-memory-model.md` updated to type-containment model (matches `ori_arc`)

- [ ] **Implement**: Reference counting
  - [ ] Heap allocation with atomic refcount header: `{ refcount: AtomicU64, data: T }`
  - [ ] `fetch_add(1, Acquire)` on clone/share
  - [ ] `fetch_sub(1, Release)` on drop
  - [ ] Free when refcount reaches zero (after acquire fence)
  - [ ] Stack-allocated values: no refcount (moved or copied)
  - [ ] **Rust Tests**: `ori_llvm/src/arc/refcount_tests.rs`

- [ ] **Spec**: Drop trait in `spec/08-types.md` § Drop Trait DONE
  - [ ] Trait definition, execution timing, LIFO order
  - [ ] Constraints (no async, must return void, panic during unwind = abort)
  - [ ] drop_early built-in function

- [ ] **Implement**: Drop trait codegen
  - [ ] Detect types implementing `Drop` trait
  - [ ] Generate destructor call when refcount reaches zero
  - [ ] Destructor runs before memory reclamation
  - [ ] `@drop (self) -> void` signature
  - [ ] Drop cannot declare `uses Suspend` (compile error)
  - [ ] **Rust Tests**: `ori_llvm/src/arc/drop_tests.rs`

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

- [ ] **Implement**: Panic during destruction
  - [ ] **Rust Tests**: `tests/destructor_panic_tests.rs`
  - [ ] Single panic in destructor: propagate normally
  - [ ] Other destructors still run after panic
  - [ ] Double panic (destructor panics during unwind): abort

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

---

## 21.16 Optimization Passes

### 21.16.1 Representation Optimization
**Proposal**: `proposals/approved/representation-optimization-proposal.md`

Formalize the compiler's freedom to optimize machine representations while preserving semantic equivalence. Tier 1 and most Tier 2 optimizations are already implemented.

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

### 21.16.3 Function Attribute Compliance (from AIMS)

Codegen quality items deferred from the AIMS codegen quality plan (2026-03-15). All completed work is in `experiment/aims` branch. Items below are self-contained with full implementation guidance.

**Context:** AIMS Section 01 (regressions) is complete. Section 02 (attributes) is ~65% done. Sections 03-05 are not started. The `/code-journey` skill scores IR quality on 13 journey programs in `plans/code-journeys/*.ori`.

**What's already done (2026-03-15):**
- `noundef` on ALL `ParamPassing::Direct` params/returns (not just scalars) — `function_compiler/mod.rs:247-263`
- `uwtable` on C main wrapper — `function_compiler/entry_point.rs:65`
- `memory(none)` on pure scalar functions — `is_arc_function_pure()` in `define_phase.rs`, integrated into `compute_nounwind_set()` in `nounwind.rs`
- Runtime function nounwind declarations verified correct
- Nounwind two-pass analysis audited and working correctly
- 27 function_compiler tests pass including `direct_aggregate_params_have_noundef`

- [ ] **`readonly` on Indirect read-only params**: For `ParamPassing::Indirect` params where the callee only reads (never mutates), add LLVM `readonly` attribute. Enables LLVM to hoist loads and CSE across calls.
  - **Why deferred**: Requires architectural change — threading `Ownership` annotation from `ori_arc` borrow inference through `ParamAbi` to the attribute application site. Currently, ABI computation is size-first: `Borrowed` + large type → `Indirect` (losing the ownership signal). `ParamAbi` only has `{ name, ty, passing }`, not ownership.
  - **Implementation approach**: (a) Add `readonly: bool` field to `ParamAbi`. (b) In `compute_function_abi_with_ownership()`, set `readonly=true` when `Ownership::Borrowed`. (c) In `declare_function_llvm_with_extra_params()`, apply `builder.add_readonly_param_attribute()` for Indirect params with `readonly=true`. (d) Add `add_readonly_param_attribute` to `ir_builder/attributes.rs` (same pattern as `add_noalias_attribute`).
  - **Alternative**: Mark ALL `ParamPassing::Reference` params as `readonly` (they're already semantically borrowed/read-only). This is simpler but doesn't cover large borrowed params that fall through to `Indirect`.
  - **Files**: `codegen/abi/mod.rs`, `codegen/function_compiler/mod.rs`, `codegen/ir_builder/attributes.rs`
- [ ] **Fix impl method nounwind gap**: Impl methods compiled via `compile_impls()` in `impls.rs` use the immediate-emit path (`emit_arc_function()`) BEFORE the two-pass nounwind analysis runs. This means impl methods calling nounwind callees still use `invoke` instead of `call`.
  - **Approach (a) — correct**: Fold impl methods into the two-pass batch: modify `compile_impls()` to use `prepare_all_cached()` + `compute_nounwind_set()` + `emit_prepared_functions()` instead of direct `emit_arc_function()`. Requires refactoring the impl compilation path.
  - **Approach (b) — simple**: Post-hoc nounwind pass: after ALL functions are emitted, walk LLVM functions and retroactively add `nounwind` to those that contain no `invoke` instructions. Simpler but less precise (misses transitive nounwind — A calls B calls C, where C is nounwind, B becomes nounwind, but post-hoc only sees B has no invoke because C was nounwind).
  - **Impact**: Affects journeys with derived trait methods and impl methods. Currently derives are handled separately via `DerivedTrait::is_nounwind_derived()` in `derive_codegen`.
  - **Files**: `codegen/function_compiler/impls.rs`, `codegen/function_compiler/nounwind.rs`
- [ ] **`memory(read)` annotation**: For functions that read from struct fields but don't write/allocate. LLVM encoding: `create_enum_attribute(kind, 21)` (already implemented as `add_memory_read_attribute()` in `ir_builder/attributes.rs`). Detection criteria: all ARC IR instructions are `Let`, `Select`, or `Project` (no `Apply`/`Construct`/`Set`/RC ops), AND function may have Indirect params (reads via pointer are OK). Lower priority than `memory(none)` — fewer qualifying functions.
- [ ] **`nonnull` + `dereferenceable` on Indirect params**: Ori pointer params are always non-null and point to valid memory of known size. Add `nonnull` and `dereferenceable(N)` attributes to `ParamPassing::Indirect` and `ParamPassing::Reference` params. Requires knowing the byte size of the pointed-to type (available from `TypeInfoStore`). Overlaps with Tier 3 pointer attributes above.

### 21.16.4 CFG Cleanup (from AIMS)

Post-emission LLVM IR contains unnecessary blocks from the ARC pipeline. These inflate code journey scores and inhibit LLVM optimization at O0.

- [ ] **Empty trampoline block elimination**: The ARC emitter generates `br`-only blocks (empty block → single successor). These should be eliminated by a post-emission simplification pass. Common in J2, J3, J5, J7, J9, J10, J12.
  - **Implementation**: Walk all blocks in a function after `emit_function()`. For each block with only a `br` instruction and no phi predecessors, redirect all predecessors to the successor and delete the block. Handle phi-node rewrites when the successor has phis.
  - **CONSTRAINT**: SimplifyCFG only runs at O1+. Code journey scoring operates on O0 IR, so a pre-optimization simplification is needed.
  - **BLOAT prerequisite**: `codegen/arc_emitter/emit_function.rs` is at 506 lines. Extract dead-unwind detection logic (lines 96-167) into `detect_dead_unwind_blocks()` to reclaim ~70 lines before adding simplification code.
  - **Files**: `codegen/arc_emitter/emit_function.rs`
- [ ] **Redundant entry block elimination**: Functions with empty entry blocks that just `br` to a header. Common in TCO functions (J3 `@gcd`, J7 `@sum_loop`).
  - **CONSTRAINT**: An empty entry block that branches to a header with phi nodes CANNOT be simply eliminated — the phis need the entry block as an incoming edge. When merging `entry → header`: if entry is the ONLY predecessor, replace phis with their single incoming value. If header has OTHER predecessors (back-edges from tail calls), the entry block cannot be merged without rewriting phis.
  - **Test**: Verify TCO still works in J3 `@gcd` after simplification (`ORI_DUMP_AFTER_LLVM=1`, check for `musttail call` or phi-based TCO pattern).

### 21.16.5 IR Quality Polish (from AIMS)

Specific IR patterns that inflate instruction count without semantic purpose.

- [ ] **Range construct-then-destructure** (J7): Range iteration creates a full range struct then immediately destructures it. Investigate whether the pattern is in ARC IR lowering or LLVM emission. Fix: optimize to use scalar SSA values directly.
- [ ] **SSO gating code redundancy** (J9): String operations emit SSO vs heap branching where both arms produce identical code. Fix: merge redundant branches (or detect at emission time).
- [ ] **List struct extract/repack** (J10 `@count_items`): List (24 bytes) passed Indirect generates extract/repack at callee entry. This is correct behavior for the FastISel safety pattern (per-field GEP+load+insert_value). Verify ABI threshold — list structs (3x i64 = 24 bytes) should be Indirect on x86-64. If ABI is correct, this is expected and the instruction count is justified.

### 21.16.6 Code Journey Score Baseline (from AIMS, 2026-03-15)

Pre-AIMS baseline scores for the 13 code journeys. Use `/code-journey` skill to re-score after implementing items above. Journey source files: `plans/code-journeys/*.ori`. Scoring tool: `.claude/skills/code-journey/extract-metrics.py`.

| Journey | Pre-AIMS | Target | Blocking Categories |
|---------|----------|--------|-------------------|
| J1 Arithmetic | 9.8 | 10.0 | Attr (uwtable on main — FIXED) |
| J2 Branching | 9.2 | 9.8+ | CF (empty blocks), Attr |
| J3 Recursion | 8.9 | 9.8+ | CF (entry blocks), Attr (nounwind) |
| J4 Structs | 9.7 | 10.0 | Attr (noundef — FIXED) |
| J5 Closures | 8.5 | 9.8+ | Attr (indirect call), CF |
| J6 Patterns | 9.7 | 10.0 | Attr (noundef — FIXED) |
| J7 Loops | 9.2 | 9.8+ | CF (entry blocks), IR (range construct) |
| J8 Generics | 9.8 | 10.0 | Attr (noundef — FIXED) |
| J9 Strings | 8.8 | 9.8+ | CF (SSO blocks), IR (SSO redundancy) |
| J10 Lists | 8.7 | 9.8+ | CF (empty blocks), Attr (nounwind) |
| J11 Derives | 9.7 | 10.0 | Attr (noundef — FIXED) |
| J12 Error | 9.2 | 9.8+ | CF (empty blocks) |
| J13 Iterators | 9.4 | 9.8+ | Attr (indirect call) |

**Verification checklist** (run after completing 21.16.3-5):
- [ ] Re-score all 13 journeys with `/code-journey`
- [ ] All journeys score >= 9.8/10, simple journeys (J1, J4, J6, J8, J11) = 10.0
- [ ] `ORI_CHECK_LEAKS=1` clean on heap-allocating journeys (J5, J9, J10, J13)
- [ ] `diagnostics/dual-exec-verify.sh` passes (eval == AOT for all tests)
- [ ] `cargo b --release && timeout 150 ./test-all.sh` green (release build — FastISel differs)
- [ ] Update `plans/code-journeys/*-results.md` with new IR dumps and scores

---

## 21.17 Runtime Support

- [ ] **Implement**: Basic runtime functions
  - [ ] **Rust Tests**: `tests/runtime_tests.rs`
  - [ ] `ori_print`, `ori_print_int`, `ori_print_float`, `ori_print_bool`
  - [ ] `ori_panic`, `ori_panic_cstr`
  - [ ] `ori_assert`, `ori_assert_eq_int`, `ori_assert_eq_bool`, `ori_assert_eq_str`
  - [ ] `ori_str_concat`, `ori_str_eq`, `ori_str_ne`
  - [ ] `ori_str_from_int`, `ori_str_from_bool`, `ori_str_from_float`
  - [ ] `ori_list_new`, `ori_list_free`, `ori_list_len`
  - [ ] `ori_compare_int`, `ori_min_int`, `ori_max_int`

- [ ] **Implement**: Extended runtime
  - [ ] Map runtime functions
  - [ ] Set runtime functions
  - [ ] Channel runtime functions
  - [ ] Duration/Size runtime functions
  - [ ] Stack trace capture functions

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
└── tests/              # Unit tests (206 tests)
    ├── mod.rs
    ├── arithmetic_tests.rs
    ├── collection_tests.rs
    ├── control_flow_tests.rs
    ├── function_tests.rs
    ├── matching_tests.rs
    └── ...
```

---

## 21.19 Section Completion Checklist

**Infrastructure:**
- [x] JIT compilation working
- [ ] All current Ori tests pass via LLVM (1082/3077 passing, 1985 LCFail)
- [x] All Rust unit tests pass (527/527, 15 skipped)
- [x] Architecture follows Rust patterns
- [x] Unified import pipeline (imports resolved once, compiled into JIT module)
- [ ] Generic monomorphization (biggest remaining gap for test coverage)
- [ ] AOT compilation (see Section 21B)

**Type System:**
- [ ] Primitive types
- [ ] Option/Result (basic)
- [ ] Lists (basic)
- [ ] Duration/Size types
- [ ] Newtypes
- [ ] Sum types (general)
- [ ] Fixed-capacity lists
- [ ] Channels
- [ ] Maps (full support)
- [ ] Sets

**Expressions:**
- [ ] Basic expressions
- [ ] Binary/unary operators (primitives)
- [ ] Range with step (`by`)
- [ ] Spread operator (`...`)
- [ ] Coalesce operator (`??`)
- [ ] Floor division (`div`)
- [ ] Type conversions (`as`, `as?`)
- [ ] Complex assignments

**Control Flow:**
- [ ] Basic if/else, loops
- [ ] Labeled loops
- [ ] Break with values
- [ ] For-yield expressions
- [ ] Try/catch patterns

**Traits & Dispatch:**
- [ ] Associated functions
- [ ] Operator trait dispatch
- [ ] Iterator trait methods
- [ ] Comparison trait methods

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
- [ ] ARC implementation
- [ ] Drop trait
- [ ] Destruction ordering

**Optimization:**
- [ ] Optimization passes
- [ ] Debug info generation
- [ ] 80+% test coverage (currently ~68%)

**Verified AOT Gaps** (from Section 11.1 verification, updated 2026-02-23):

*Resolved:*
- [x] Derive Eq struct codegen: `emit_comparison_via_trait` now dispatches to derived `eq` method for non-primitive types instead of falling through to scalar `icmp` <!-- test: test_aot_derive_eq_struct (un-ignored), test_aot_derive_eq_struct_not_equal, test_aot_derive_eq_struct_with_strings -->
- [x] Derive Comparable struct codegen: `emit_ordering_comparison` now dispatches to derived `compare` method and checks Ordering result (0=Less, 1=Equal, 2=Greater) for `<`/`>`/`<=`/`>=` <!-- test: test_aot_derive_comparable_struct -->
- [x] ARC enum basic drop and string-payload drop: `test_arc_enum_basic_drop` and `test_arc_enum_with_string_payload` un-ignored and passing

*Open:*
- [ ] List mutation methods not in AOT builtin table: `.push()`, `.first()`, `.last()`, `.is_empty()`, `.concat()` <!-- test: test_aot_list_push, test_aot_list_first_last, test_aot_list_empty_operations, test_aot_list_concat -->
- [ ] Map methods not in AOT builtin table: `.is_empty()`, `.get()`, `.insert()`, `.remove()`, `.keys()`, `.values()` <!-- test: test_aot_map_is_empty -->
- [ ] `list[index]` subscript not resolved in AOT <!-- test: test_aot_list_index -->
- [ ] Closure-returning-closure type inference: `let f = (n: int) -> (int) -> int = { (x: int) -> int = ... }` infers `()` return instead of `(int) -> int` <!-- test: test_aot_closure_capturing_closure, section: 02 -->
- [ ] Enum variant constructors not declared as LLVM functions <!-- test: test_aot_enum_variant_constructors -->
- [ ] Generic monomorphization not in ARC pipeline <!-- test: test_aot_generic_identity, test_aot_generic_pair -->
- [ ] `catch(expr:)` not yet lowered through ARC pipeline <!-- test: test_aot_catch_basic, test_aot_catch_no_panic, test_aot_catch_nested -->
- [ ] String interpolation produces wrong result <!-- test: test_aot_string_interpolation -->

Cross-references to existing items:
- Generic monomorphization → § 21.7 (**CRITICAL** — blocks 2,472+ test call sites)
- Enum variant constructors → § 21.2 (blocks enums, recursive types)
- `catch(expr:)` → § 21.5 (blocks panic recovery in AOT)
- String interpolation → § 21.3

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

**Total AOT test counts** (2026-02-23): 850 passed, 0 failed, 70 ignored

**Exit Criteria**: Feature parity with evaluator for all language constructs

---

## Running Tests

```bash
# Build Docker container (first time only)
./docker/llvm/build.sh

# Run all Ori tests via LLVM
./docker/llvm/run.sh ori test

# Run spec tests only
./docker/llvm/run.sh ori test tests/spec

# Run Rust unit tests
./docker/llvm/run.sh cargo test -p ori_llvm --lib

# Run with debug output
ORI_DEBUG_LLVM=1 ./docker/llvm/run.sh ori test tests/spec/types/primitives.ori
```
