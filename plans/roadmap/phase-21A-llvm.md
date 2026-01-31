# Phase 21A: LLVM Backend

**Status:** 🔶 Partial — JIT working, basic codegen functional, many features missing

## Current Test Results (2026-01-31)

| Test Suite | Passed | Failed | Skipped | Total |
|------------|--------|--------|---------|-------|
| All Ori tests | 1572 | 0 | 39 | 1611 |
| Rust unit tests | 206 | 0 | 1 | 207 |

---

## 21.1 LLVM Setup & Infrastructure

- [x] **Setup**: LLVM development environment
  - [x] Docker container with LLVM 17 and development headers
  - [x] Add `inkwell` crate to `compiler/ori_llvm/Cargo.toml`
  - [x] Verify LLVM bindings compile and link correctly
  - [x] Create `compiler/ori_llvm/src/` module structure

- [x] **Implement**: LLVM context and module initialization
  - [x] **Rust Tests**: `context.rs` — SimpleCx, CodegenCx, TypeCache
  - [x] Create LLVM context, module, and builder abstractions

- [x] **Implement**: Basic target configuration
  - [x] Support native target detection (JIT)
  - [ ] Configure data layout and target features (AOT)

---

## 21.2 Type Lowering

- [x] **Implement**: Primitive type mapping
  - [x] **Rust Tests**: `types.rs`, `context.rs` — type mapping
  - [x] Map Ori primitives (int → i64, float → f64, bool → i1, char → i32, byte → i8)
  - [x] Map strings to `{ i64 len, ptr data }` struct
  - [x] Handle function types

- [x] **Implement**: Option/Result types
  - [x] Map Option/Result to `{ i8 tag, i64 payload }` tagged unions
  - [x] Tag values: None=0, Some=1; Err=0, Ok=1
  - [ ] Proper payload handling for non-primitive types

- [x] **Implement**: Collection types
  - [x] Map lists to `{ i64 len, i64 cap, ptr data }` struct
  - [ ] Map maps to appropriate hash table representation
  - [ ] Map sets to appropriate hash set representation

- [ ] **Implement**: Duration & Size types
  - [ ] Map Duration to i64 (nanoseconds)
  - [ ] Map Size to i64 (bytes)
  - [ ] Duration arithmetic operations (add, sub, mul, div, mod)
  - [ ] Size arithmetic operations (add, sub, mul, div, mod)
  - [ ] Mixed-type arithmetic (Duration * int, int * Duration, Duration / Duration → int)
  - [ ] Size unary negation compile error
  - [ ] Overflow panic semantics for Duration
  - [ ] Division by zero panics
  - [ ] Duration methods: `.nanoseconds()`, `.microseconds()`, `.milliseconds()`, `.seconds()`, `.minutes()`, `.hours()`
  - [ ] Size methods: `.bytes()`, `.kilobytes()`, `.megabytes()`, `.gigabytes()`, `.terabytes()`
  - [ ] Factory functions: `Duration.from_nanoseconds(ns:)`, `Size.from_bytes(b:)`

- [ ] **Implement**: Newtype codegen
  - [ ] Newtype type representation (same as wrapped type)
  - [ ] Constructor codegen: `TypeName(value)`
  - [ ] `.inner` field access
  - [ ] Proper type distinction for trait dispatch

- [ ] **Implement**: Sum type codegen (beyond Option/Result)
  - [ ] User-defined sum type representation (tagged union)
  - [ ] Variant constructor codegen
  - [ ] Tag-based dispatch in match
  - [ ] Multi-field variant payloads

- [ ] **Implement**: Fixed-capacity lists `[T, max N]`
  - [ ] Inline allocation strategy
  - [ ] Compile-time capacity tracking
  - [ ] `.capacity()`, `.is_full()`, `.remaining()` methods
  - [ ] `.push()` with panic on full
  - [ ] `.try_push()` → `bool`
  - [ ] `.push_or_drop()`, `.push_or_oldest()`
  - [ ] `.to_dynamic()` conversion
  - [ ] `.to_fixed<$N>()` and `.try_to_fixed<$N>()`

- [ ] **Implement**: Channel types
  - [ ] `Producer<T>` and `Consumer<T>` type representation
  - [ ] `CloneableProducer<T>` and `CloneableConsumer<T>`
  - [ ] `Sendable` trait constraint checking
  - [ ] Channel buffer management

---

## 21.3 Expression Codegen

- [x] **Implement**: Basic expressions
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

- [ ] **Implement**: Bitwise operators
  - [ ] `~` (BitNot) codegen
  - [ ] `&`, `|`, `^` for int and byte types
  - [ ] `<<`, `>>` shift operators
  - [ ] Shift overflow panics (count < 0 or count >= bit width)

- [ ] **Implement**: Type conversions
  - [ ] `expr as Type` infallible conversion codegen
  - [ ] `expr as? Type` fallible conversion → `Option<T>`
  - [ ] Standard conversions: int→float, str→Error, Set<T>→[T]
  - [ ] Into trait method dispatch

- [ ] **Implement**: Assignment to complex targets
  - [ ] Field assignment: `point.x = 10`
  - [ ] Index assignment: `list[0] = value`
  - [ ] Nested assignments: `a.b.c = value`

- [ ] **Implement**: String operations
  - [ ] String indexing with Unicode handling (`str[i]` → single-codepoint `str`)
  - [ ] String interpolation: `` `Hello {name}` ``
  - [ ] All escape sequences: `\n`, `\t`, `\r`, `\0`, `\\`, `\"`

---

## 21.4 Operator Trait Dispatch

- [x] **Implement**: User-defined impl blocks and associated functions
  - [x] Register user-defined struct types with LLVM
  - [x] Support user-defined `impl Type { ... }` blocks
  - [x] Generate method dispatch for user-defined methods
  - [x] Support associated functions (methods without `self` parameter)
  - [x] Enable `Type.method()` syntax for user-defined types
  - [x] **Tests**: `tests/spec/types/associated_functions.ori` (9 tests passing)

- [ ] **Implement**: User-defined operator dispatch
  - [ ] **Rust Tests**: `tests/operator_trait_tests.rs`
  - [ ] Detect when operand is a user-defined type (struct)
  - [ ] Generate method calls to trait methods instead of direct LLVM ops
  - [ ] Arithmetic operators → `.add()`, `.subtract()`, `.multiply()`, `.divide()`, `.floor_divide()`, `.remainder()`
  - [ ] Unary operators → `.negate()`, `.not()`, `.bit_not()`
  - [ ] Bitwise operators → `.bit_and()`, `.bit_or()`, `.bit_xor()`, `.shift_left()`, `.shift_right()`
  - [ ] Comparison operators → `.equals()`, `.compare()`
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

- [x] **Implement**: Basic control flow
  - [x] **Rust Tests**: `tests/control_flow_tests.rs`, `tests/advanced_control_flow_tests.rs`
  - [x] Basic block creation and linking
  - [x] Phi nodes for SSA form
  - [x] Branch and conditional instructions
  - [x] Basic break/continue in loops

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

- [x] **Implement**: Basic patterns
  - [x] Literal patterns
  - [x] Binding patterns
  - [x] Wildcard patterns `_`
  - [x] Basic struct destructuring
  - [x] Basic tuple destructuring

- [ ] **Implement**: Advanced patterns
  - [ ] Range patterns: `1..10`, `'a'..'z'`
  - [ ] Or-patterns: `A | B | C`
  - [ ] At-patterns: `x @ Some(_)`
  - [ ] Guard patterns: `x if x > 0`
  - [ ] Guard with `.match(condition)` method
  - [ ] Struct destructuring with field renaming: `{ x: px, y: py }`
  - [ ] List patterns with rest: `[first, ..rest]`, `[..init, last]`
  - [ ] Nested patterns

- [ ] **Implement**: Match expression improvements
  - [ ] Exhaustiveness verification in codegen
  - [ ] Never variant handling
  - [ ] Pattern matrix optimization

---

## 21.7 Function Sequences & Expressions

- [x] **Implement**: Basic function codegen
  - [x] **Rust Tests**: `tests/function_tests.rs`, `tests/function_call_tests.rs`
  - [x] Function signatures and calling conventions
  - [x] Local variables
  - [x] Return statements

- [ ] **Implement**: Function sequences (`run`, `try`, `match`)
  - [ ] `run(let x = a, result)` sequential binding
  - [ ] `run(pre_check:, body, post_check:)` with panic semantics
  - [ ] `try(let x = f()?, Ok(x))` error propagation
  - [ ] `match(v, P -> e, _ -> d)` pattern matching

- [ ] **Implement**: Recurse pattern
  - [ ] `recurse(condition:, base:, step:)` basic recursion
  - [ ] `memo:` memoization support
  - [ ] `parallel:` parallel recursion flag
  - [ ] Proper state threading

- [ ] **Implement**: With pattern (RAII)
  - [ ] `with(acquire:, use:, release:)` codegen
  - [ ] Guaranteed release even on panic
  - [ ] Resource binding in `use:` block

---

## 21.8 Concurrency Patterns

- [ ] **Implement**: Parallel pattern
  - [ ] `parallel(tasks:, max_concurrent:, timeout:)` → `[Result]`
  - [ ] Task spawning and scheduling
  - [ ] Result collection
  - [ ] Timeout handling
  - [ ] `uses Suspend` capability requirement

- [ ] **Implement**: Spawn pattern
  - [ ] `spawn(tasks:, max_concurrent:)` → `void`
  - [ ] Fire-and-forget semantics
  - [ ] Background execution

- [ ] **Implement**: Timeout pattern
  - [ ] `timeout(op:, after:)` → `Result<T, TimeoutError>`
  - [ ] Operation cancellation on timeout
  - [ ] Proper cleanup

- [ ] **Implement**: Cache pattern
  - [ ] `cache(key:, op:, ttl:)` codegen
  - [ ] Cache key handling
  - [ ] TTL management
  - [ ] Cache invalidation

- [ ] **Implement**: Nursery pattern
  - [ ] `nursery(body:, on_error:, timeout:)` codegen
  - [ ] `NurseryErrorMode` handling (CancelRemaining, CollectAll, FailFast)
  - [ ] Structured concurrency guarantees

- [ ] **Implement**: Channel operations
  - [ ] `channel<T>(buffer:)` → `(Producer, Consumer)` creation
  - [ ] `channel_in`, `channel_out`, `channel_all` selection
  - [ ] Send/receive operations
  - [ ] Buffer management

---

## 21.9 Capabilities & With Pattern

- [ ] **Implement**: Capability tracking
  - [ ] `uses Capability` declaration checking
  - [ ] Capability propagation through calls
  - [ ] Missing capability errors

- [ ] **Implement**: Capability provision
  - [ ] `with Cap = impl in expr` codegen
  - [ ] Multiple capabilities: `with A = a, B = b in expr`
  - [ ] Provider instance binding
  - [ ] Scope-based capability resolution

- [ ] **Implement**: Default implementations
  - [ ] `def impl` resolution
  - [ ] Override with `with` pattern
  - [ ] `without def` import handling

- [ ] **Implement**: Standard capabilities
  - [ ] `Print` (default provided)
  - [ ] `Http`, `FileSystem`, `Clock`, `Random`
  - [ ] `Crypto`, `Cache`, `Logger`, `Env`
  - [ ] `Intrinsics`, `Suspend`, `FFI`

---

## 21.10 Collections & Iterators

- [ ] **Implement**: List operations
  - [ ] `.push()`, `.pop()` methods
  - [ ] `.insert()`, `.remove()` methods
  - [ ] `.get()` → `Option<T>`
  - [ ] Capacity management
  - [ ] List iteration

- [ ] **Implement**: Map operations
  - [ ] Map literal codegen with computed keys: `{[expr]: value}`
  - [ ] `.get()` → `Option<V>`
  - [ ] `.insert()`, `.remove()` methods
  - [ ] `.contains_key()` method
  - [ ] Map iteration (key-value pairs)

- [ ] **Implement**: Set operations
  - [ ] Set type representation
  - [ ] Set literal support
  - [ ] `.insert()`, `.remove()`, `.contains()` methods
  - [ ] Set operations: union, intersection, difference

- [ ] **Implement**: Iterator trait codegen
  - [ ] `Iterator.next()` method dispatch
  - [ ] `DoubleEndedIterator.next_back()` dispatch
  - [ ] `.map()`, `.filter()`, `.fold()` method chains
  - [ ] `.collect()` into target collection
  - [ ] `.enumerate()`, `.zip()`, `.chain()`
  - [ ] `.take()`, `.skip()`, `.cycle()`
  - [ ] `.rev()`, `.last()`, `.rfind()`, `.rfold()`

- [ ] **Implement**: Infinite iterators
  - [ ] `repeat(value:)` codegen
  - [ ] `(0..).iter()` infinite range
  - [ ] Proper bounding with `.take(count:)`

---

## 21.11 Lambda & Closure Support

- [x] **Implement**: Basic lambdas
  - [x] Simple lambda syntax: `x -> x + 1`
  - [x] Multi-param lambdas: `(a, b) -> a + b`
  - [x] No-param lambdas: `() -> 42`

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

---

## 21.12 Built-in Functions

- [x] **Implement**: Basic built-ins
  - [x] `print(msg:)`
  - [x] `panic(msg:)` → `Never`
  - [x] Basic assertions

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

---

## 21.13 FFI Support

- [ ] **Implement**: C FFI
  - [ ] `extern "c" from "lib" { ... }` declaration parsing
  - [ ] C type bindings: `c_char`, `c_int`, `c_float`, `c_double`, `c_size`, etc.
  - [ ] Function name mapping with `as "name"`
  - [ ] `CPtr` opaque pointer type
  - [ ] `Option<CPtr>` for nullable pointers

- [ ] **Implement**: Unsafe blocks
  - [ ] `unsafe { ... }` block codegen
  - [ ] `uses FFI` capability requirement
  - [ ] Pointer operations: `ptr_read`, `ptr_write`

- [ ] **Implement**: JavaScript FFI (WASM target)
  - [ ] `extern "js" { ... }` declaration
  - [ ] `extern "js" from "./file.js"` imports
  - [ ] `JsValue` handle type
  - [ ] `JsPromise<T>` async handling
  - [ ] Implicit promise resolution at binding sites

- [ ] **Implement**: Memory layout control
  - [ ] `#repr("c")` struct representation
  - [ ] C-compatible struct layout
  - [ ] Callback support

---

## 21.14 Conditional Compilation

- [ ] **Implement**: Target conditionals
  - [ ] `#target(os: "linux")` codegen (only compile matching branch)
  - [ ] `#target(arch:)`, `#target(family:)`
  - [ ] `any_os:`, `not_os:` predicates
  - [ ] File-level: `#!target(...)`

- [ ] **Implement**: Config conditionals
  - [ ] `#cfg(debug)`, `#cfg(release)`
  - [ ] `#cfg(feature: "name")`, `any_feature:`, `not_feature:`

- [ ] **Implement**: Compile-time constants
  - [ ] `$target_os`, `$target_arch`, `$target_family`
  - [ ] `$debug`, `$release`
  - [ ] False branch not type-checked

- [ ] **Implement**: Compile errors
  - [ ] `compile_error("msg")` codegen

---

## 21.15 Memory Management (ARC)

- [ ] **Implement**: Reference counting
  - [ ] Atomic refcount allocation for heap types
  - [ ] `fetch_add` on clone/share
  - [ ] `fetch_sub` on drop
  - [ ] Free when refcount reaches zero

- [ ] **Implement**: Drop trait codegen
  - [ ] **Rust Tests**: `tests/drop_tests.rs`
  - [ ] Detect types implementing Drop trait
  - [ ] Generate destructor calls when refcount reaches zero
  - [ ] Destructor called before memory reclamation

- [ ] **Implement**: Destruction ordering
  - [ ] **Rust Tests**: `tests/destruction_order_tests.rs`
  - [ ] Reverse declaration order for local bindings
  - [ ] Reverse declaration order for struct fields
  - [ ] Back-to-front for list elements
  - [ ] Right-to-left for tuple elements

- [ ] **Implement**: Panic during destruction
  - [ ] **Rust Tests**: `tests/destructor_panic_tests.rs`
  - [ ] Single panic in destructor: propagate normally
  - [ ] Other destructors still run after panic
  - [ ] Double panic (destructor panics during unwind): abort

- [ ] **Implement**: Async destructor restriction
  - [ ] Compile error if Drop.drop declares `uses Suspend`
  - [ ] Error code and message for async destructor attempt

---

## 21.16 Optimization Passes

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

---

## 21.17 Runtime Support

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

## 21.19 Phase Completion Checklist

**Infrastructure:**
- [x] JIT compilation working
- [x] All current Ori tests pass (1572/1572, 39 skipped)
- [x] All Rust unit tests pass (206/206)
- [x] Architecture follows Rust patterns
- [ ] AOT compilation (see Phase 21B)

**Type System:**
- [x] Primitive types
- [x] Option/Result (basic)
- [x] Lists (basic)
- [ ] Duration/Size types
- [ ] Newtypes
- [ ] Sum types (general)
- [ ] Fixed-capacity lists
- [ ] Channels
- [ ] Maps (full support)
- [ ] Sets

**Expressions:**
- [x] Basic expressions
- [x] Binary/unary operators (primitives)
- [ ] Range with step (`by`)
- [ ] Spread operator (`...`)
- [ ] Coalesce operator (`??`)
- [ ] Floor division (`div`)
- [ ] Type conversions (`as`, `as?`)
- [ ] Complex assignments

**Control Flow:**
- [x] Basic if/else, loops
- [ ] Labeled loops
- [ ] Break with values
- [ ] For-yield expressions
- [ ] Try/catch patterns

**Traits & Dispatch:**
- [x] Associated functions
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

**Exit Criteria**: Feature parity with interpreter for all language constructs

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
