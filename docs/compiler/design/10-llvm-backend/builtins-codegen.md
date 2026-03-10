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

Ori uses a **declarative registration system** where each built-in method is registered with a handler function that emits LLVM IR. Simple methods emit inline instructions; complex methods emit calls to runtime functions. The `declare_builtins!` macro generates both the dispatch logic and the registration table from a single declaration, guaranteeing that registration and dispatch stay synchronized.

## What Makes Ori's Builtins System Distinctive

### Macro-Generated Dual Artifacts

A single `declare_builtins!` invocation in each submodule generates two artifacts simultaneously:

1. **`dispatch(emitter, ctx) -> Option<ValueId>`** — a match cascade on `(type_name, method)` pairs that routes to the correct handler. Returns `Some(value)` if the builtin was handled, `None` if it should fall through to normal method dispatch.

2. **`REGISTERED: &[BuiltinRegistration]`** — an enumerable list of all registered builtins, used for sync tests and the `BuiltinTable`.

This dual-generation design makes an entire class of bugs impossible: you cannot register a builtin without providing a dispatch handler, and you cannot dispatch to an unregistered builtin. The macro is the single source of truth for what builtins exist and how they are dispatched.

### Sync Testing Against the Type Checker

The `ori_registry` crate maintains the single source of truth for built-in methods (`ori_registry::BUILTIN_TYPES`) — the methods that can be called on built-in types without requiring an explicit `impl` block. The type checker, evaluator, and builtins codegen system each consume this registry. If any consumer drifts — the type checker allows `str.reverse()` but codegen doesn't handle it, or codegen handles `int.clamp()` but the type checker doesn't know about it — the program will either fail at codegen (missing handler) or silently fall through to runtime dispatch (unnecessary overhead).

Sync tests in each consumer automatically iterate `ori_registry::BUILTIN_TYPES` and verify that every registered method is handled. Adding a new built-in method to the registry without updating all consumers triggers a test failure.

### Receiver Borrowing Metadata

Each builtin registration includes a `receiver_borrowed` flag that the ARC emitter uses to decide whether to increment the receiver's reference count before the call. Borrowed receivers (`.len()`, `.is_empty()`, `.contains()`) skip the increment — the method only reads the receiver, so there is no need to extend its lifetime. Owned receivers (methods that consume or modify the receiver) get the normal RC treatment.

This per-method borrowing information is not available from the type system alone — it is a codegen-level optimization that depends on the method's implementation, not its signature.

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

The Copy-on-Write list operations are the most complex builtins. Each COW function receives the list's components (data pointer, length, capacity), the element to operate on, the element size, and **element RC callbacks** — function pointers that know how to increment or decrement the reference count of a single element.

| Method | Runtime Function | Element Callbacks |
|--------|-----------------|-------------------|
| `push` | `ori_list_push_cow` | inc + dec |
| `pop` | `ori_list_pop_cow` | dec only |
| `set` | `ori_list_set_cow` | inc + dec |
| `insert` | `ori_list_insert_cow` | inc + dec |
| `remove` | `ori_list_remove_cow` | dec only |
| `concat` | `ori_list_concat_cow` | inc + dec |

The element callbacks are generated by `element_fn_gen.rs` — one `extern "C" fn(*mut u8)` for increment, one for decrement, per element type. For `[int]`, both callbacks are null (integers need no RC). For `[str]`, the callbacks increment/decrement the string's data pointer. For `[[int]]`, the callbacks handle the nested list's RC. These are cached per type to avoid regeneration.

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

The `BuiltinTable` provides O(1) lookup for builtin existence and metadata. It is a two-level `FxHashMap`: type name → method name → `BuiltinRegistration`. Built once from `REGISTERED` at module initialization, it serves three purposes:

1. **Early rejection** — before attempting builtin dispatch, check if `(type, method)` is registered. This avoids entering the match cascade for methods that will definitely fall through.

2. **Sync tests** — test infrastructure iterates the table to verify parity with `ori_registry::BUILTIN_TYPES`.

3. **Receiver metadata** — the `receiver_borrowed` flag drives ARC behavior at call sites.

## Adding a New Built-in Method

Adding a new built-in method (for example, `str.repeat`) requires exactly four changes:

1. **Implement the handler** in the appropriate submodule. The handler receives a `BuiltinCtx` and returns `Option<ValueId>`.

2. **Register in `declare_builtins!`** within the same submodule: `("str", "repeat", emit_str_repeat, true)`. The macro handles dispatch routing and table registration.

3. **Add to `ori_registry::BUILTIN_TYPES`** in the registry crate. Add a `MethodDef` entry to the appropriate `TypeDef`.

4. **Add runtime function** (if needed) to `RT_FUNCTIONS` in `runtime_functions.rs` and implement it in `ori_rt`.

No other files need modification. The sync tests verify that all registrations match. The macro handles dispatch and table population.

## Prior Art

**[rustc](https://github.com/rust-lang/rust)** — Rust's codegen does not have a comparable "builtins" system because Rust's built-in methods are implemented as actual trait impls (e.g., `impl Add for i32`) that go through normal monomorphization and codegen. LLVM intrinsics are used for specific operations (`@llvm.ctpop` for `count_ones`, `@llvm.fabs` for `f64::abs`), but there is no dispatch table for method-level builtin handling. Ori's approach is necessary because Ori's interpreter and LLVM backend share a language-level method interface that doesn't map directly to trait impls.

**[V8](https://github.com/nicknisi/v8)** (JavaScript) — V8's "Torque" language defines built-in functions in a domain-specific language that generates both the interpreter bytecode handlers and the optimizing compiler's inline code. This is structurally similar to Ori's `declare_builtins!` macro — a single source generates both dispatch and implementation. V8's approach is more sophisticated (Torque generates multiple tiers of code), but the principle of single-source builtin definition is the same.

**[GHC](https://gitlab.haskell.org/ghc/ghc)** (Haskell) — GHC's "primops" are built-in operations that bypass normal function call overhead. Each primop has a codegen handler that emits specialized assembly or LLVM IR. GHC's `primops.txt.pp` file serves a similar role to Ori's `declare_builtins!` — it is a single source that generates both the type checker's knowledge of primops and the codegen handlers.

**[Zig](https://github.com/ziglang/zig)** — Zig's built-in functions (`@addWithOverflow`, `@memcpy`, `@typeInfo`) are handled directly by the compiler's codegen. Each builtin has a dedicated handler in `Sema.zig` or `codegen/llvm.zig`. Zig's approach is simpler (no macro or registration table) but less systematic — adding a new builtin requires editing multiple files manually.

## Design Tradeoffs

**Macro registration vs. manual dispatch.** The `declare_builtins!` macro adds compile-time complexity (macro expansion, generated code) in exchange for eliminating registration drift. A manual approach — separate dispatch function and registration array — would be simpler to read but would require developers to update two places when adding a builtin. The macro's enforcement guarantee is worth the abstraction cost.

**Inline codegen vs. runtime calls.** Each builtin handler must decide whether to emit inline instructions or call a runtime function. The heuristic is: if the operation can be expressed in 1–5 LLVM instructions with no loops, inline it. If it requires loops, heap allocation, or complex logic (UTF-8 string processing, hash table operations), call the runtime. The boundary is fuzzy — `contains` on lists uses an inline loop (it's a performance-critical path), while `sort` uses a runtime call (the sorting algorithm is complex).

**Per-method borrowing vs. type-level borrowing.** The `receiver_borrowed` flag is per-method, not per-type. An alternative — marking entire types as "always borrowed for reads" — would be simpler but incorrect for methods like `clone()` that need ownership semantics even though they only "read" the value. Per-method granularity trades registration verbosity for correct RC behavior.

**Single BuiltinCtx vs. method-specific contexts.** All handlers receive the same `BuiltinCtx`, even though some handlers use only a few fields. A method-specific context (different struct for collection methods vs. primitive methods) would be more precise but would fragment the API and make the macro more complex. The uniform context trades a few unused fields for API simplicity.
