---
title: "Builtins Codegen"
description: "Ori Compiler Design — Built-in Method LLVM Generation"
order: 1005
section: "LLVM Backend"
---

# Builtins Codegen

## Why Inline Codegen for Built-in Methods?

When a programmer writes `list.len()`, the simplest implementation would be to compile it as a function call: emit a call to `ori_list_len(list)`, let the runtime extract the length field, and return it. This is correct, but it is also wasteful — `list.len()` is a single field extraction from a known struct layout. The "function call" crosses the compiler-runtime boundary, pushes arguments, saves registers, and returns through the call stack, all to read one integer from a fixed offset.

The alternative is **inline codegen**: instead of calling a function, the compiler emits the LLVM instructions directly. For `list.len()`, this means a single `extractvalue` instruction that pulls the length field from the list's `{ i64, i64, ptr }` representation. No function call, no ABI overhead, no register spilling. The result is the same, but the generated code is dramatically faster.

This trade-off — inline code generation versus runtime function calls — applies to hundreds of built-in methods across all of Ori's types. Some methods are simple enough to inline entirely (`.len()`, `.is_empty()`, `.is_some()`). Others require complex logic that belongs in the runtime (`.split()`, `.sort()`, `.format()`). The builtins codegen system manages this decision for every built-in method in the language.

### Classical Approaches

**Everything as runtime calls** (the CPython approach) — every operation, including `len()`, `+`, and `[]`, dispatches through the runtime. This is simple to implement but slow. CPython's method dispatch goes through `PyObject_GetAttr`, hash table lookups, and descriptor protocol handling for every single method call.

**Everything inlined** (the early C compiler approach) — the compiler knows about every built-in type and operation, generating specialized code for each. This produces fast code but makes the compiler monolithic — adding a new type or method requires modifying the compiler's codegen.

**Intrinsics** (the LLVM/GCC approach) — certain functions are recognized by the compiler and replaced with specialized instruction sequences. LLVM's `@llvm.memcpy`, `@llvm.sqrt`, and `@llvm.ctpop` are examples. The compiler pattern-matches on function names and replaces them with target-specific instruction sequences.

**Tiered dispatch** (the V8/JavaScriptCore approach) — hot methods get specialized inline implementations, warm methods use runtime calls, and cold methods use generic dispatch. The tier boundaries are determined by profiling data.

### Where Ori Sits

Ori's LLVM backend uses a **declarative physical-handler system**. Simple methods emit inline instructions; complex methods emit calls to runtime functions. The `declare_builtins!` macro generates LLVM dispatch logic and an enumerable handler table from a single declaration. This table owns LLVM coverage only. Language semantics, parameter ownership, and runtime identity belong to `ori_registry` and the shared executable carrier.

## What Makes Ori's Builtins System Distinctive

### Macro-Generated Dual Artifacts

A single `declare_builtins!` invocation in each submodule generates two artifacts simultaneously:

1. **`dispatch(emitter, ctx) -> Option<ValueId>`** — a match cascade on `(type_name, method)` pairs that routes to the correct handler. Returns `Some(value)` if the builtin was handled, `None` if it should fall through to normal method dispatch.

2. **`REGISTERED: &[BuiltinRegistration]`** — an enumerable list of all registered builtins, used for sync tests and the `BuiltinTable`.

This dual-generation design keeps LLVM registration and dispatch synchronized: an LLVM handler cannot be registered without a dispatch arm. It is not the single source of truth for which language methods exist or how ownership transfers.

### Sync Testing Against the Type Checker

The `ori_registry` crate maintains the single source of truth for built-in methods (`ori_registry::BUILTIN_TYPES`) — the methods that can be called without an explicit `impl` block. The type checker, evaluator, AIMS, executable compiler, VM, and LLVM projection consume typed portions of this registry. `MethodDef` owns semantic signature and ownership; `MethodRuntime` identifies shared runtime operations. A physical backend may implement an identity inline or through a helper, but cannot infer its contract from spelling.

Sync tests in each consumer automatically iterate `ori_registry::BUILTIN_TYPES` and verify that every registered method is handled. Adding a new built-in method to the registry without updating all consumers triggers a test failure.

### Ownership Is Upstream

LLVM builtin registrations contain only the physical `(type_name, method_name)` handler key. Receiver and parameter ownership come from the registry-backed AIMS contract and realized call instruction. The emitter must not add or remove RC operations based on whether a handler happens to inline the method.

> **Current gap — spelling-based physical dispatch.** `BuiltinCtx` and `declare_builtins!` still select many LLVM handlers by type and method strings, and not every operation carries a closed `MethodRuntime` identity yet. This is acceptable only as a documented migration state. Production execution must carry typed operation identity through the shared artifact, reject missing identities, and make both VM and LLVM dispatch exhaustive over that identity.

## BuiltinCtx

Every handler function receives a `BuiltinCtx` containing everything needed to emit correct LLVM IR:

| Field | Type | Purpose |
|-------|------|---------|
| `type_name` | `&str` | Ori type being dispatched on (e.g., `"list"`, `"str"`) |
| `method` | `&str` | Method name being called (e.g., `"len"`, `"push"`) |
| `arg_vals` | `&[ValueId]` | LLVM values for all arguments; receiver is `arg_vals[0]` |
| `receiver_ty` | `Idx` | Pool type index for the receiver |
| `type_info` | `&TypeInfo` | Full type information (inner types, field layout) |
| `arc_args` | `&[ArcArg]` | ARC IR argument metadata (for type lookups) |
| `arc_func` | `&ArcFunction` | Enclosing ARC function (for additional type context) |
| `result_ty` | `Idx` | Pool type index for the expected return type |

Handlers return `Option<ValueId>`: `Some(value)` for successfully emitted IR, `None` to signal fallthrough to normal method dispatch.

## Built-in Method Coverage

The builtins system covers every type in the language. Each submodule handles a coherent group of types and methods.

### Primitives

Numeric, boolean, character, byte, Duration, Size, and Ordering methods. Most emit 1–3 LLVM instructions.

| Type | Methods | Typical Emission |
|------|---------|-----------------|
| `int` | `clone`, `abs`, `to_float`, `min`, `max`, `clamp`, `pow`, `is_positive`, `is_negative`, `is_zero` | `icmp` + `select` for `abs`; `sitofp` for `to_float` |
| `float` | `clone`, `abs`, `floor`, `ceil`, `round`, `to_int`, `sqrt`, `is_nan`, `is_infinite`, `is_finite`, `min`, `max` | LLVM intrinsics: `@llvm.fabs`, `@llvm.floor`, `@llvm.sqrt` |
| `bool` | `clone`, `to_int` | `zext i1 to i64` for `to_int` |
| `char` | `clone`, `to_int`, `to_str`, `is_alphabetic`, `is_numeric`, `is_whitespace`, `to_upper`, `to_lower` | `zext i32 to i64` for `to_int`; runtime calls for Unicode |
| `byte` | `clone`, `to_int`, `to_char` | Zero-extension instructions |
| `Duration` | `clone`, `nanoseconds`, `microseconds`, `milliseconds`, `seconds`, `minutes`, `hours` | Division by duration constants |
| `Size` | `clone`, `bytes`, `kilobytes`, `megabytes`, `gigabytes`, `terabytes` | Division by size constants |
| `Ordering` | `clone`, `is_less`, `is_equal`, `is_greater`, `reverse`, `then`, `then_with` | `icmp eq i8 %v, -1` for `is_less` |

`int.clone()` and `bool.clone()` are identity operations — the value is already in a register, and "copying" it means returning the same register. These exist so that generic code that calls `.clone()` on any `T: Clone` works uniformly for primitives.

### Strings

String methods balance inline emission and runtime calls. Layout access (length, empty check) is inlined; content manipulation (split, trim, case conversion) delegates to the runtime because UTF-8 string processing requires non-trivial logic.

| Pattern | Methods | Implementation |
|---------|---------|---------------|
| Layout access | `len`, `is_empty` | `extractvalue` from `{ i64, ptr }` |
| Identity | `to_str` | Return receiver unchanged |
| Content queries | `contains`, `starts_with`, `ends_with` | Runtime: `ori_str_contains`, etc. |
| Transformation | `split`, `trim`, `upper`, `lower` | Runtime: returns new strings |
| Concatenation | `+` operator | `ori_str_concat_sso` (SSO-aware) |
| Debug | `debug` | Runtime: adds quote escaping |
| Iteration | `iter` | `ori_iter_from_str` |

### Lists

List operations span the full range from simple inline code to complex runtime interactions:

| Pattern | Methods | Implementation |
|---------|---------|---------------|
| Layout access | `len`, `is_empty` | `extractvalue` from `{ i64, i64, ptr }` |
| Element access | `first`, `last` | Bounds check + GEP to element |
| Search | `contains` | Inline linear scan with equality check loop |
| Order | `reverse`, `sort` | Runtime calls; `sort` uses comparison thunks |
| COW mutations | `push`, `pop`, `insert`, `remove`, `set` | Runtime COW functions (see below) |
| Iteration | `iter` | `ori_iter_from_list` |
| Deep copy | `clone` | `ori_list_clone` + element RC inc loop |

### COW List Operations

- Copy-on-Write list operations are the most complex LLVM builtins.
- Each current COW helper receives the list's physical components, element
  size, and **element RC callbacks**.
- Production `CompiledLayoutPlan` construction selects and binds these LLVM
  callback projections to upstream logical retain/release identities.
- AIMS does not generate callbacks.

| Method | Runtime Function | Element Callbacks |
|--------|-----------------|-------------------|
| `push` | `ori_list_push_cow` | inc + dec |
| `pop` | `ori_list_pop_cow` | dec only |
| `set` | `ori_list_set_cow` | inc + dec |
| `insert` | `ori_list_insert_cow` | inc + dec |
| `remove` | `ori_list_remove_cow` | dec only |
| `concat` | `ori_list_concat_cow` | inc + dec |

- `element_fn_gen.rs` generates the current LLVM callback projection: one
  `extern "C" fn(*mut u8)` for increment and one for decrement per element layout.
- `[int]` uses null callbacks; `[str]` callbacks operate on the compiled string
  reference; `[[int]]` callbacks project the nested list's bound drop plan.
- Production caches by compiled-plan identity and keeps each callback bound to
  the validated plan that selected it.
- Direct callback derivation from `TypeInfo` is a current migration gap.

### Maps and Sets

Map and set operations follow the same pattern as lists — layout access is inlined, mutations use COW runtime functions with key/value RC callbacks:

| Type | Methods |
|------|---------|
| `map` | `len`, `is_empty`, `get`, `contains_key`, `insert`, `remove`, `entries`, `keys`, `values`, `iter` |
| `Set` | `len`, `is_empty`, `contains`, `insert`, `remove`, `union`, `intersection`, `difference`, `iter` |

Map operations additionally pass key hash and equality callbacks for type-generic hashing — the runtime's hash map implementation doesn't know how to hash an arbitrary key type, so the compiler generates key-specific hash and equality functions.

### Option and Result

Option and Result methods are almost entirely inlined, leveraging the `{ i8, T }` tagged representation:

| Pattern | Methods | Implementation |
|---------|---------|---------------|
| Tag check | `is_some`, `is_none`, `is_ok`, `is_err` | Single `icmp` on tag byte |
| Unwrap | `unwrap`, `unwrap_or` | Branch on tag, extract payload or use default |
| Transform | `map`, `and_then`, `filter` | Branch on tag, call closure in `Some`/`Ok` arm |
| Convert | `ok_or`, `ok`, `err` | Construct new tagged value |
| Context | `context` | Wrap error with context string |

`is_some`/`is_none` emit exactly one LLVM instruction: `icmp eq i8 %tag, 1` (or 0). These are among the simplest builtins in the system.

### Iterators

Iterator methods split into adapters (which construct new iterator representations) and consumers (which drive the iteration loop):

| Pattern | Methods | Implementation |
|---------|---------|---------------|
| Adapters | `map`, `filter`, `take`, `skip`, `enumerate`, `zip`, `chain`, `flatten`, `flat_map`, `cycle` | Construct runtime iterator adapter |
| Consumers | `count`, `any`, `all`, `find`, `fold`, `for_each`, `collect` | Inline loop with early exit |
| Next | `next` | Runtime dispatch on iterator variant |
| Double-ended | `rev`, `last`, `rfind`, `rfold` | Runtime adapter or consumer |

Consumer methods emit inline loops rather than function calls. A `list.iter().count()` emits a loop that calls `ori_iter_next` repeatedly, counting non-None results, with early exit optimization for iterators with known size hints.

### Traits

Trait method dispatch for comparison, equality, and hashing delegates to type-specific implementations:

| Trait | Method | Dispatch |
|-------|--------|----------|
| `Eq` | `equals` | Type-specific: `ori_str_eq` for strings, `icmp` for ints |
| `Comparable` | `compare` | Type-specific: `ori_compare_int`, string comparison |
| `Hashable` | `hash` | Type-specific hash computation |

### Compound Types

`to_str` and `debug` for structs, enums, and tuples generate inline IR that:

1. Allocates a string buffer
2. Appends the type/variant name
3. Iterates fields, calling each field's `to_str`/`debug` recursively
4. Joins with commas and wraps in appropriate delimiters

### Prelude Functions

Prelude functions (`print`, `assert`, `assert_eq`, `panic`, `dbg`, etc.) are not methods but use the same builtin infrastructure. They emit calls to runtime functions (`ori_print`, `ori_assert`, `ori_panic`) with type-specific argument preparation.

## BuiltinTable

`BuiltinTable` is a test-only, two-level `FxHashMap` from type name to method name to `BuiltinRegistration`. Test infrastructure builds it from the `REGISTERED` arrays to enumerate LLVM coverage and compare that coverage with `ori_registry::BUILTIN_TYPES`. Production dispatch uses the generated submodule match chain directly; the table does not own semantics or runtime routing.

## Adding a New Built-in Method

Adding a new built-in method (for example, `str.repeat`) starts at the semantic registry and then supplies each applicable physical projection:

1. **Add the `MethodDef`** to `ori_registry::BUILTIN_TYPES`, including parameter ownership and a `MethodRuntime` identity when execution uses a shared runtime operation.

2. **Teach AIMS and executable lowering to consume the typed registry facts**. Do not add a name-based ownership exception.

3. **Implement the VM operation or adapter** when the method is available to VM execution.

4. **Implement and register the LLVM physical handler** in `declare_builtins!`, or map the shared runtime identity to `RT_FUNCTIONS` and `ori_rt`.

Coverage tests must compare each physical consumer against the registry and shared operation identity. The LLVM macro checks only its own handler table; it cannot certify cross-backend semantic or ownership parity.

## Prior Art

**[rustc](https://github.com/rust-lang/rust)** — Rust's codegen does not have a comparable "builtins" system because Rust's built-in methods are implemented as actual trait impls (e.g., `impl Add for i32`) that go through normal monomorphization and codegen. LLVM intrinsics are used for specific operations (`@llvm.ctpop` for `count_ones`, `@llvm.fabs` for `f64::abs`), but there is no dispatch table for method-level builtin handling. Ori instead gives evaluator, VM, and compiled execution one language-level method interface whose typed semantic and ownership facts live above every physical handler.

**[V8](https://github.com/nicknisi/v8)** (JavaScript) — V8's "Torque" language defines built-in functions in a domain-specific language that generates both the interpreter bytecode handlers and the optimizing compiler's inline code. This is structurally similar to Ori's `declare_builtins!` macro — a single source generates both dispatch and implementation. V8's approach is more sophisticated (Torque generates multiple tiers of code), but the principle of single-source builtin definition is the same.

**[GHC](https://gitlab.haskell.org/ghc/ghc)** (Haskell) — GHC's "primops" are built-in operations that bypass normal function call overhead. Each primop has a codegen handler that emits specialized assembly or LLVM IR. GHC's `primops.txt.pp` file serves a similar role to Ori's `declare_builtins!` — it is a single source that generates both the type checker's knowledge of primops and the codegen handlers.

**[Zig](https://github.com/ziglang/zig)** — Zig's built-in functions (`@addWithOverflow`, `@memcpy`, `@typeInfo`) are handled directly by the compiler's codegen. Each builtin has a dedicated handler in `Sema.zig` or `codegen/llvm.zig`. Zig's approach is simpler (no macro or registration table) but less systematic — adding a new builtin requires editing multiple files manually.

## Design Tradeoffs

**Macro registration vs. manual dispatch.** The `declare_builtins!` macro adds compile-time complexity (macro expansion, generated code) in exchange for eliminating registration drift. A manual approach — separate dispatch function and registration array — would be simpler to read but would require developers to update two places when adding a builtin. The macro's enforcement guarantee is worth the abstraction cost.

**Inline codegen vs. runtime calls.** Each builtin handler must decide whether to emit inline instructions or call a runtime function. The heuristic is: if the operation can be expressed in 1–5 LLVM instructions with no loops, inline it. If it requires loops, heap allocation, or complex logic (UTF-8 string processing, hash table operations), call the runtime. The boundary is fuzzy — `contains` on lists uses an inline loop (it's a performance-critical path), while `sort` uses a runtime call (the sorting algorithm is complex).

**Per-method ownership vs. type-level defaults.** Receiver and parameter ownership are typed per method in `MethodDef`; they are not inferred by a backend from the receiver type or method spelling. A type-level default would be simpler but incorrect for methods such as `clone()` whose ownership contract differs from superficially read-only operations. Per-method contracts add registry detail in exchange for correct, shared RC behavior.

**Single BuiltinCtx vs. method-specific contexts.** All handlers receive the same `BuiltinCtx`, even though some handlers use only a few fields. A method-specific context (different struct for collection methods vs. primitive methods) would be more precise but would fragment the API and make the macro more complex. The uniform context trades a few unused fields for API simplicity.
