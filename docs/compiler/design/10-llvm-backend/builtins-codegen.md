---
title: "Builtins Codegen"
description: "Ori Compiler Design — Built-in Function LLVM Generation"
order: 1005
section: "LLVM Backend"
---

# Builtins Codegen

## Overview

Built-in methods (`len`, `push`, `map`, `filter`, `to_str`, `clone`, etc.) bypass the normal method-dispatch-to-function-call path and emit inline LLVM IR directly. This provides both performance (no function call overhead, no boxing, no dynamic dispatch) and correctness (direct access to runtime memory representations that are opaque to user code).

The builtins system is the LLVM backend's equivalent of the interpreter's `method_dispatch/` module. Where the interpreter matches on method names at runtime, the builtins system matches at compile time and emits specialized IR.

## `declare_builtins!` Macro

A single-invocation macro that generates two artifacts from one declaration:

1. **`dispatch(emitter, ctx) -> Option<ValueId>`** -- A match cascade on `(type_name, method)` pairs that routes to the correct handler function. Returns `Some(value)` if the builtin was handled, `None` if it should fall through to normal method dispatch.

2. **`REGISTERED: &[BuiltinRegistration]`** -- An enumerable list of all registered builtins, used for sync tests and the `BuiltinTable`.

This dual-generation design guarantees that registration and dispatch stay synchronized. It is impossible to register a builtin without providing a dispatch handler, or to dispatch to an unregistered builtin. Adding a new built-in method requires exactly one edit in the submodule that handles it -- the macro handles the rest.

### Registration Entry

Each entry in `declare_builtins!` specifies:

```
(type_name, method_name, handler_fn, receiver_borrowed)
```

- `type_name`: The Ori type name string (e.g., `"list"`, `"str"`, `"int"`)
- `method_name`: The method name string (e.g., `"push"`, `"len"`, `"clone"`)
- `handler_fn`: The Rust function that emits LLVM IR for this builtin
- `receiver_borrowed`: Whether the receiver is borrowed (affects RC handling at call sites)

## BuiltinCtx

`BuiltinCtx` is the context struct passed to every builtin handler function. It provides everything a handler needs to emit correct LLVM IR:

| Field | Type | Purpose |
|-------|------|---------|
| `type_name` | `&str` | The Ori type being dispatched on |
| `method` | `&str` | The method name being called |
| `arg_vals` | `&[ValueId]` | LLVM values for all arguments; receiver is `arg_vals[0]` |
| `receiver_ty` | `Idx` | Pool type index for the receiver, used for type queries |
| `type_info` | `&TypeInfo` | Full type information (inner types, field layout, etc.) |
| `arc_args` | `&[ArcArg]` | ARC IR argument metadata (for `var_type` lookups) |
| `arc_func` | `&ArcFunction` | The enclosing ARC function (for additional type context) |
| `result_ty` | `Idx` | Pool type index for the expected return type |

Handlers return `Option<ValueId>` -- `Some` for successfully emitted IR, `None` to signal fallthrough (the method is not handled by this builtin module).

## Submodule Coverage

Each submodule in `builtins/` handles a coherent set of types and methods:

### `primitives.rs`

Numeric, boolean, character, byte, Duration, Size, and Ordering methods.

| Type | Methods |
|------|---------|
| `int` | `clone`, `abs`, `to_float`, `min`, `max`, `clamp`, `pow`, `is_positive`, `is_negative`, `is_zero` |
| `float` | `clone`, `abs`, `floor`, `ceil`, `round`, `to_int`, `sqrt`, `is_nan`, `is_infinite`, `is_finite`, `min`, `max` |
| `bool` | `clone`, `to_int` |
| `char` | `clone`, `to_int`, `to_str`, `is_alphabetic`, `is_numeric`, `is_whitespace`, `to_upper`, `to_lower` |
| `byte` | `clone`, `to_int`, `to_char` |
| `Duration` | `clone`, `nanoseconds`, `microseconds`, `milliseconds`, `seconds`, `minutes`, `hours` |
| `Size` | `clone`, `bytes`, `kilobytes`, `megabytes`, `gigabytes`, `terabytes` |
| `Ordering` | `clone`, `is_less`, `is_equal`, `is_greater`, `reverse`, `then`, `then_with` |

Most primitive methods emit 1-3 LLVM instructions (e.g., `int.abs` emits a comparison and select).

### `collections/string_builtins.rs`

String methods, most of which call into the runtime because strings are UTF-8 encoded and require non-trivial logic.

| Methods | Implementation |
|---------|---------------|
| `clone` | `ori_str_clone` (RC inc on data pointer) |
| `len` | Extract length field from `{ i64, ptr }` |
| `is_empty` | Compare length to 0 |
| `contains`, `starts_with`, `ends_with` | Runtime calls (`ori_str_contains`, etc.) |
| `split`, `trim`, `upper`, `lower` | Runtime calls returning new strings |
| `concat` (`+` operator) | `ori_str_concat_sso` (SSO-aware) |
| `iter` | `ori_iter_from_str` |
| `to_str` | Identity (strings are already strings) |
| `debug` | Runtime call to add quote escaping |

### `collections/list_builtins.rs`

List methods for `[T]`. Many operations delegate to runtime functions.

| Methods | Implementation |
|---------|---------------|
| `clone` | Deep clone via `ori_list_clone` + element RC inc loop |
| `len` | Extract length field from `{ i64, i64, ptr }` |
| `is_empty` | Compare length to 0 |
| `first`, `last` | Bounds check + GEP to element |
| `contains` | Linear scan with per-element equality check |
| `reverse` | Runtime call (`ori_list_reverse`) |
| `sort` | Runtime call with comparison thunk (`ori_list_sort`) |
| `iter` | `ori_iter_from_list` |
| `push`, `pop`, `insert`, `remove` | See COW operations below |

### `collections/list_cow.rs`

COW (Copy-on-Write) specialized list operations. These use the `ori_list_*_cow` family of runtime functions that check uniqueness before mutating.

| Methods | Runtime Function |
|---------|-----------------|
| `push` | `ori_list_push_cow(data, len, cap, elem, elem_size, elem_inc, elem_dec)` |
| `pop` | `ori_list_pop_cow(data, len, cap, elem_dec)` |
| `set` (index assign) | `ori_list_set_cow(data, len, cap, idx, elem, elem_size, elem_inc, elem_dec)` |
| `insert` | `ori_list_insert_cow(data, len, cap, idx, elem, elem_size, elem_inc, elem_dec)` |
| `remove` | `ori_list_remove_cow(data, len, cap, idx, elem_dec)` |
| `concat` | `ori_list_concat_cow(...)` |

Each COW function receives element-level RC callbacks (`elem_inc`, `elem_dec`) generated by `element_fn_gen.rs`. These callbacks are `extern "C" fn(*mut u8)` functions that know how to increment or decrement the RC of a single element of type `T`.

### `collections/map_set_builtins.rs`

Map (`{K: V}`) and Set (`Set<T>`) operations.

| Type | Methods |
|------|---------|
| `map` | `len`, `is_empty`, `get`, `contains_key`, `insert`, `remove`, `entries`, `keys`, `values`, `iter` |
| `Set` | `len`, `is_empty`, `contains`, `insert`, `remove`, `union`, `intersection`, `difference`, `iter` |

Map and Set operations use COW runtime functions similar to lists. The `insert` and `remove` methods pass key hash and equality callbacks for type-generic hashing.

### `option_result.rs`

Option and Result methods. Many are emitted as inline IR without runtime calls.

| Type | Methods |
|------|---------|
| `Option` | `is_some`, `is_none`, `unwrap`, `unwrap_or`, `map`, `and_then`, `filter`, `ok_or` |
| `Result` | `is_ok`, `is_err`, `unwrap`, `unwrap_or`, `ok`, `err`, `map`, `map_err`, `and_then`, `context` |

`is_some`/`is_none`/`is_ok`/`is_err` emit a single tag comparison. `unwrap_or` emits a branch on the tag with a PHI merge. `map` and `and_then` emit closure calls guarded by tag checks.

### `iterator.rs`

Iterator adapter and consumer methods.

| Methods | Implementation |
|---------|---------------|
| `next` | Runtime dispatch based on iterator variant |
| `count`, `any`, `all` | Inline loop with early exit |
| `find` | Inline loop with predicate closure call |
| `fold` | Inline loop with accumulator |
| `for_each` | Inline loop with void closure call |
| `collect` | Allocate list with `size_hint` capacity, loop `next` until `None` |
| `map`, `filter`, `take`, `skip`, `enumerate`, `zip`, `chain` | Construct new iterator adapter (runtime representation) |

### `traits.rs`

Trait method dispatch: comparison, equality, and hashing.

| Trait | Methods | Pattern |
|-------|---------|---------|
| `Eq` | `equals` | Delegates to type-specific equality (e.g., `ori_str_eq` for strings) |
| `Comparable` | `compare` | Delegates to type-specific comparison (e.g., `ori_compare_int`) |
| `Hashable` | `hash` | Delegates to type-specific hash computation |

### `compound_traits.rs`

`to_str` and `debug` formatting for compound types (structs, enums, tuples). These generate inline IR that:

1. Allocates a string buffer
2. Appends the type/variant name
3. Iterates fields, calling each field's `to_str`/`debug` recursively
4. Joins with commas and wraps in braces/parentheses

### `list_traits.rs`

List implementations of `Comparable` and `Hashable`. These emit inline loops that compare/hash elements pairwise, using element-type-specific comparison/hash thunks.

### `prelude.rs`

Prelude function builtins (`print`, `assert`, `assert_eq`, `panic`, `dbg`, etc.). These are not methods but are handled through the same builtin infrastructure for consistency.

### `trampolines.rs`

Trampoline functions for bridging between calling conventions. Used when a method reference must be passed as a function pointer (e.g., passing `list.push` as a callback). The trampoline wraps the method call with the correct receiver handling.

## BuiltinTable

`BuiltinTable` provides O(1) lookup for builtin existence and metadata. It is a two-level `FxHashMap`:

```
type_name: &str -> method_name: &str -> BuiltinRegistration
```

The table is built once from `REGISTERED` at module initialization. It serves three purposes:

1. **Early rejection**: Before attempting builtin dispatch, check if `(type, method)` is even registered. This avoids entering the match cascade for methods that will definitely fall through.

2. **Sync tests**: Test infrastructure iterates the table to verify that every registered builtin has a corresponding entry in `TYPECK_BUILTIN_METHODS` (and vice versa), preventing registration drift between the type checker and codegen.

3. **Receiver metadata**: The `receiver_borrowed` flag from registration is used by the ARC emitter to decide whether to increment the receiver's RC before a builtin call. Borrowed receivers skip the increment.

## Adding a New Builtin

To add a new built-in method (e.g., `str.repeat`):

1. **Implement the handler** in the appropriate submodule (e.g., `collections/string_builtins.rs`):
   ```rust
   fn emit_str_repeat(emitter: &mut ArcIrEmitter, ctx: &BuiltinCtx) -> Option<ValueId> {
       // ... emit LLVM IR ...
   }
   ```

2. **Register in `declare_builtins!`** within the same submodule:
   ```
   ("str", "repeat", emit_str_repeat, true)
   ```

3. **Add to `TYPECK_BUILTIN_METHODS`** in the type checker (must be alphabetically sorted by `(type, method)`).

4. **Add runtime function** (if needed) to `runtime_functions.rs` and implement in `ori_rt`.

5. **Run sync tests** -- they will catch any registration drift.

No other files need modification. The macro handles dispatch routing and table registration automatically.
