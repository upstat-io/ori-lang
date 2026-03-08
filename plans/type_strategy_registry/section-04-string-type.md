---
plan: "type_strategy_registry"
section: "04"
title: "String Type Definition"
status: complete
depends_on:
  - "01"
  - "02"
estimated_lines: 450
complexity: medium-high
---

# Section 04: String Type Definition

## Overview

STR is the most complex primitive type in the registry. Unlike int/float/bool/byte/char (all `MemoryStrategy::Copy`), str uses `MemoryStrategy::Arc` -- it is reference-counted with Small String Optimization (SSO). Strings <= 23 bytes are stored inline (no heap, no RC); longer strings use a heap-allocated RC-managed buffer. Its operators use `RuntimeCall` to delegate to `ori_rt` functions rather than emitting native LLVM instructions. It has the largest method surface of any primitive type (38 methods across ori_types, 25 in ori_eval, 18 in ori_ir, and 27 in ori_llvm across collections + traits).

This section defines the complete `STR` `TypeDef` constant with every method, operator, and ownership annotation, producing the single source of truth that all four compiler phases will consume.

---

## 04.1 STR Method Inventory

### Complete method list from resolve_str_method (ori_types)

Source: `compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs` (str methods section).

| Method | Parameters | Return Type | Category |
|--------|-----------|-------------|----------|
| `len` | `()` | `int` | Query |
| `byte_len` | `()` | `int` | Query |
| `length` | `()` | `int` | Query (alias of `len`) |
| `is_empty` | `()` | `bool` | Predicate |
| `contains` | `(substr: str)` | `bool` | Predicate |
| `starts_with` | `(prefix: str)` | `bool` | Predicate |
| `ends_with` | `(suffix: str)` | `bool` | Predicate |
| `to_uppercase` | `()` | `str` | Transform |
| `to_lowercase` | `()` | `str` | Transform |
| `trim` | `()` | `str` | Transform |
| `trim_start` | `()` | `str` | Transform |
| `trim_end` | `()` | `str` | Transform |
| `escape` | `()` | `str` | Transform |
| `concat` | `(other: str)` | `str` | Combine |
| `repeat` | `(count: int)` | `str` | Combine |
| `replace` | `(pattern: str, replacement: str)` | `str` | Transform |
| `slice` | `(start: int, end: int)` | `str` | Extract |
| `substring` | `(start: int, end: int)` | `str` | Extract (alias of `slice`) |
| `pad_start` | `(width: int, fill: str)` | `str` | Transform |
| `pad_end` | `(width: int, fill: str)` | `str` | Transform |
| `split` | `(sep: str)` | `[str]` | Decompose |
| `lines` | `()` | `[str]` | Decompose |
| `chars` | `()` | `[char]` | Decompose |
| `bytes` | `()` | `[byte]` | Decompose |
| `iter` | `()` | `DoubleEndedIterator<char>` | Iteration |
| `index_of` | `(substr: str)` | `Option<int>` | Search |
| `last_index_of` | `(substr: str)` | `Option<int>` | Search |
| `to_int` / `parse_int` | `()` | `Option<int>` | Conversion |
| `to_float` / `parse_float` | `()` | `Option<float>` | Conversion |
| `into` | `()` | `Error` | Conversion (str -> Error) |
| `clone` | `()` | `str` | Trait: Clone |
| `to_str` | `()` | `str` | Trait: Printable |
| `debug` | `()` | `str` | Trait: Debug |
| `equals` | `(other: str)` | `bool` | Trait: Eq |
| `compare` | `(other: str)` | `Ordering` | Trait: Comparable |
| `hash` | `()` | `int` | Trait: Hashable |

Note: `to_int` and `parse_int` are aliases; `to_float` and `parse_float` are aliases. Both appear in the type checker match arm together.

### Spec-Defined Methods Not Yet in Type Checker

The following methods are defined in the Ori spec (§8.1.6 String Byte Access) but are NOT yet implemented in `resolve_str_method`. The registry MUST include them to be the complete specification. They will need to be added to the type checker during the wiring phase (Section 09).

| Method | Parameters | Return Type | Category | Spec Reference |
|--------|-----------|-------------|----------|----------------|
| `as_bytes` | `()` | `[byte]` | Byte Access | §8.1.6 — zero-copy view via seamless slice |
| `to_bytes` | `()` | `[byte]` | Byte Access | §8.1.6 — independent copy of UTF-8 bytes |

**Note:** `as_bytes()` has special ownership semantics — it returns a `[byte]` seamless slice that shares the underlying allocation with the source `str`. COW semantics apply. `to_bytes()` returns an independent copy. Both are `pure: true`.

**Note:** `bytes()` (already in the type checker) returns `[byte]` like `to_bytes()`, but the spec defines `as_bytes()` and `to_bytes()` separately with distinct ownership semantics (zero-copy vs copy). The registry should include all three.

### Associated Functions

The spec (§8.1.6) defines two associated functions on `str`. These are NOT instance methods — they are called as `str.from_utf8(bytes:)`, not `s.from_utf8()`.

| Function | Parameters | Return Type | Category | Spec Reference |
|----------|-----------|-------------|----------|----------------|
| `from_utf8` | `(bytes: [byte])` | `Result<str, Error>` | Construction | §8.1.6 — validates UTF-8 encoding |
| `from_utf8_unchecked` | `(bytes: [byte])` | `str` | Construction | §8.1.6 — unsafe, skips validation |

These require `MethodKind::Associated` (from frozen decision 9) and must be included in the `STR` `TypeDef`. `from_utf8_unchecked` additionally requires the `Unsafe` capability annotation; the registry does not currently model capability requirements on individual methods (a future `requires_unsafe: bool` field on `MethodDef` could address this). See Section 05 for precedent on associated functions from `Duration`/`Size`.

**Note:** These are not yet in the type checker (`resolve_str_method` only handles instance methods). The wiring phase (Section 09) must add associated function resolution for str, following the pattern already established for Duration/Size associated functions.

### Alias Representation Strategy

Several str methods are aliases of each other:
- `length` aliases `len`
- `substring` aliases `slice`
- `parse_int` aliases `to_int`
- `parse_float` aliases `to_float`

**Registry representation:** Aliases are represented as **separate `MethodDef` entries** with identical signatures. The registry does NOT have an `alias_of` field on `MethodDef`. This is intentional:

1. Each alias is independently resolvable by name — the query API returns the same signature for both names.
2. The evaluator and LLVM backend may route aliases to the same implementation, but that is a backend concern, not a registry concern.
3. Adding an `alias_of: Option<&'static str>` field would add complexity for marginal benefit — the consuming phases already handle aliases via their dispatch logic.

If alias deduplication becomes important for diagnostics (e.g., "did you mean `len` instead of `length`?"), it can be added as a query API helper in Section 08 without changing the data model.

### Cross-Phase Reconciliation Table

| Method | ori_types | ori_eval | ori_ir | ori_llvm | Status |
|--------|:---------:|:--------:|:------:|:--------:|--------|
| `add` | - | Y | Y | - (operator) | **Operator alias** -- `+` desugars to `ori_str_concat` in LLVM |
| `byte_len` | Y | - | - | - | **Typeck-only** |
| `bytes` | Y | - | - | - | **Typeck-only** |
| `chars` | Y | - | - | Y | **Partial** (typeck + LLVM) |
| `clone` | Y | Y | Y | Y | Complete |
| `compare` | Y | Y | Y | Y | Complete |
| `concat` | Y | Y | Y | Y | Complete |
| `contains` | Y | Y | Y | Y | Complete |
| `debug` | Y | Y | Y | - | **Missing LLVM** |
| `ends_with` | Y | Y | Y | Y | Complete |
| `equals` | Y | Y | Y | Y | Complete |
| `escape` | Y | Y | Y | - | **Missing LLVM** |
| `hash` | Y | Y | Y | Y | Complete |
| `index_of` | Y | - | - | - | **Typeck-only** |
| `into` | Y | Y | - | - | **Missing IR/LLVM** |
| `is_empty` | Y | Y | Y | Y | Complete |
| `is_equal` | - | - | - | Y | **LLVM alias** of `equals` |
| `is_greater` | - | - | - | Y | **LLVM trait predicate** |
| `is_greater_or_equal` | - | - | - | Y | **LLVM trait predicate** |
| `is_less` | - | - | - | Y | **LLVM trait predicate** |
| `is_less_or_equal` | - | - | - | Y | **LLVM trait predicate** |
| `iter` | Y | Y | - | Y | **Missing IR** |
| `last_index_of` | Y | - | - | - | **Typeck-only** |
| `len` | Y | Y | Y | Y | Complete |
| `length` | Y | Y | - | Y | **Partial** (eval dispatches via `n.length`, LLVM has entry) |
| `lines` | Y | - | - | - | **Typeck-only** |
| `pad_end` | Y | - | - | - | **Typeck-only** |
| `pad_start` | Y | - | - | - | **Typeck-only** |
| `parse_float` | Y | - | - | - | **Typeck-only** |
| `parse_int` | Y | - | - | - | **Typeck-only** |
| `repeat` | Y | Y | Y | Y | Complete |
| `replace` | Y | Y | Y | Y | Complete |
| `slice` | Y | Y | - | Y | **Missing IR** |
| `split` | Y | Y | - | Y | **Missing IR** |
| `starts_with` | Y | Y | Y | Y | Complete |
| `substring` | Y | Y | - | Y | **Missing IR** |
| `to_float` | Y | - | - | - | **Typeck-only** |
| `to_int` | Y | - | - | - | **Typeck-only** |
| `to_lowercase` | Y | Y | Y | Y | Complete |
| `to_str` | Y | Y | - | Y | **Missing IR** |
| `to_uppercase` | Y | Y | Y | Y | Complete |
| `trim` | Y | Y | Y | Y | Complete |
| `trim_end` | Y | - | - | - | **Typeck-only** |
| `trim_start` | Y | - | - | - | **Typeck-only** |
| `as_bytes` | - | - | - | - | **Spec-only** (§8.1.6, not yet implemented) |
| `to_bytes` | - | - | - | - | **Spec-only** (§8.1.6, not yet implemented) |
| `from_utf8` | - | - | - | - | **Spec-only** (§8.1.6, associated fn, not yet implemented) |
| `from_utf8_unchecked` | - | - | - | - | **Spec-only** (§8.1.6, associated fn, not yet implemented) |

### Gap Summary

- **Complete across all 4 phases (15):** `clone`, `compare`, `concat`, `contains`, `ends_with`, `equals`, `hash`, `is_empty`, `len`, `repeat`, `replace`, `starts_with`, `to_lowercase`, `to_uppercase`, `trim`
- **Missing IR only (5):** `iter`, `slice`, `split`, `substring`, `to_str` (present in eval + LLVM, but not in ori_ir BUILTIN_METHODS)
- **Typeck-only (13):** `byte_len`, `bytes`, `index_of`, `last_index_of`, `lines`, `pad_end`, `pad_start`, `parse_float`, `parse_int`, `to_float`, `to_int`, `trim_end`, `trim_start`
- **Missing LLVM (2):** `debug`, `escape`
- **LLVM-only comparison predicates (5):** `is_equal`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal` -- these are generated from the `Comparable` trait and only exist at the LLVM level as lowered dispatch targets.
- **Spec-defined, not yet implemented (2):** `as_bytes`, `to_bytes` — defined in spec §8.1.6, not in any compiler phase yet. Must be added during wiring (Section 09).
- **Spec-defined associated functions, not yet implemented (2):** `str.from_utf8`, `str.from_utf8_unchecked` — defined in spec §8.1.6, not in any compiler phase yet.

### Traits Not Covered by the Registry (str-specific)

The following traits apply to `str` but are NOT represented as `MethodDef` entries:

1. **`Formattable`** — `str` implements `Printable`, which provides a blanket `Formattable` impl. The `format(spec:)` method is resolved through trait dispatch, not through `resolve_str_method()`. It does NOT appear in `TYPECK_BUILTIN_METHODS` for str (only `Duration` and `Size` have explicit `format` entries). The registry does NOT include a `format` MethodDef for str.

2. **`Iterable`** — `str` implements `Iterable` (spec §8.13.1). The `iter()` method IS included as a MethodDef (returning `DoubleEndedIterator<char>`). The `Iterable` trait itself is satisfied through the well_known bitfield system in `ori_types`, not the registry.

3. **`DoubleEndedIterator` (on str's iterator)** — spec §8.13.1 says `str` supports `DoubleEndedIterator`. This means `str.iter()` returns a `DoubleEndedIterator`, which is captured by the `ReturnTag::DoubleEndedIterator(TypeTag::Char)` return type on the `iter` method. The DEI methods (`next_back`, `rev`, etc.) live on the Iterator TypeDef (Section 07), not on str.

4. **`Default`** — `str` implements `Default` (default is `""`). This is handled by the well_known bitfield, not the method registry. `default()` is an associated function.

5. **`Sendable`** — `str` is NOT `Sendable` per spec §8.14 (heap-allocated, reference-counted).

6. **`Into`** — `str` has `into()` returning `Error` (spec §8.11). This IS included as a direct `MethodDef` with `trait_name: None` because the builtin `into()` is hardcoded in the type checker, not resolved through trait dispatch. The `Into` trait exists in the stdlib for user-defined types, but builtin `into()` bypasses it. This mirrors the pattern used for `int.into()` -> `float` (Section 03.1).

---

## 04.2 STR Operator Strategies

### Operator Table

| Operator | Ori Syntax | OpStrategy | Runtime Function | Notes |
|----------|-----------|------------|-----------------|-------|
| `add` | `a + b` | `RuntimeCall("ori_str_concat")` | `ori_str_concat(*const OriStr, *const OriStr) -> OriStr` | COW-optimized: SSO merge, in-place append, or new alloc |
| `eq` | `a == b` | `RuntimeCall("ori_str_eq")` | `ori_str_eq(*const OriStr, *const OriStr) -> bool` | Byte-level comparison |
| `neq` | `a != b` | `RuntimeCall("ori_str_ne")` | `ori_str_ne(*const OriStr, *const OriStr) -> bool` | Negation of `ori_str_eq` |
| `lt` | `a < b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(*const OriStr, *const OriStr) -> i8` | Result == 0 (Less) |
| `gt` | `a > b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(*const OriStr, *const OriStr) -> i8` | Result == 2 (Greater) |
| `lt_eq` | `a <= b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(*const OriStr, *const OriStr) -> i8` | Result != 2 |
| `gt_eq` | `a >= b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(*const OriStr, *const OriStr) -> i8` | Result != 0 |
| `sub` | `a - b` | `Unsupported` | - | - |
| `mul` | `a * b` | `Unsupported` | - | - |
| `div` | `a / b` | `Unsupported` | - | - |
| `rem` | `a % b` | `Unsupported` | - | - |
| `floor_div` | `a div b` | `Unsupported` | - | - |
| `neg` | `-a` | `Unsupported` | - | - |
| `not` | `!a` | `Unsupported` | - | - |
| `bit_and` | `a & b` | `Unsupported` | - | - |
| `bit_or` | `a \| b` | `Unsupported` | - | - |
| `bit_xor` | `a ^ b` | `Unsupported` | - | - |
| `bit_not` | `~a` | `Unsupported` | - | - |
| `shl` | `a << b` | `Unsupported` | - | - |
| `shr` | `a >> b` | `Unsupported` | - | - |

### Why RuntimeCall?

String operators cannot use native LLVM instructions because:

1. **Strings are variable-length structures with SSO.** The LLVM `{i64, i64, ptr}` representation (a 24-byte SSO union: heap `{len, cap, data}` or SSO `{bytes[23], flags}`) cannot be compared or concatenated with a single instruction -- it requires dispatching on the SSO flag, dereferencing the correct data source, iterating over bytes, and potentially allocating new memory.

2. **`ori_str_concat` must handle COW.** Concatenation uses a 4-case COW strategy: (1) both SSO and combined <= 23 bytes -> SSO result, (2) heap unique with capacity -> in-place append, (3) heap unique without capacity -> fresh allocation, (4) shared/SSO -> new allocation. This is fundamentally different from `add` on `i64` which is a single ALU instruction.

3. **`ori_str_compare` does byte-level lexicographic ordering.** This cannot be expressed as a single `icmp` -- it requires looping over bytes with length awareness. The runtime function returns an `i8` Ordering tag (Less=0, Equal=1, Greater=2), which the LLVM backend then checks against the expected value.

4. **The comparison bug.** The string ordering operators (`<`, `>`, `<=`, `>=`) were broken before commit `0bed4d75` because `emit_binary_op` lacked `is_str` guards -- it fell through to `icmp_slt`/`icmp_sgt` which compared raw `{i64, i64, ptr}` struct values instead of string content. The `OpStrategy::RuntimeCall` pattern in the registry makes this impossible by design: if the strategy says `RuntimeCall`, the backend *must* call the runtime function.

### ABI Convention

All `ori_str_*` runtime functions take `*const OriStr` (pointer to the 24-byte SSO union `{i64 len, i64 cap, ptr data}`). The LLVM backend creates entry-block allocas, stores the `{i64, i64, ptr}` value, and passes the alloca pointer. This is implemented in `emit_str_runtime_call` (`arc_emitter/apply.rs`).

Functions returning `OriStr` use the `sret` (struct-return) convention -- the caller allocates stack space and passes a pointer as the first argument, and the callee writes the result there. Functions returning `bool` or `i8` return scalars directly.

---

## 04.3 STR Ownership Semantics

### Memory Strategy

```
MemoryStrategy::Arc
```

The `str` type in Ori is an immutable string with Small String Optimization (SSO). At the runtime level it is represented as a 24-byte union:

```rust
// ori_rt/src/string/mod.rs
#[repr(C)]
pub union OriStr {
    pub heap: OriStrHeap,  // {len: i64, cap: i64, data: *mut u8}
    pub sso: OriStrSSO,    // {bytes: [u8; 23], flags: u8}
}
```

- **SSO** (strings <= 23 bytes): stored inline in `bytes[0..len]`. The `flags` byte (byte 23) has high bit set (`0x80`) as the SSO discriminator; low 7 bits encode the length. No heap allocation, no RC.
- **Heap** (strings > 23 bytes): `data` pointer points into an `ori_rc_alloc`-managed allocation with a hidden reference count header (8 bytes before the data). The `cap` field tracks allocated capacity for COW growth.

### SSO Implications on Ownership Model

SSO means the registry's `MemoryStrategy::Arc` is a simplification. In reality:

1. **`clone`**: SSO strings are bitwise-copied (24 bytes, no `ori_rc_inc`). Heap strings get `ori_rc_inc` on the data pointer. The `MethodDef` declares `receiver: Ownership::Borrow` and `returns: ReturnTag::SelfType` — the implementation details of how the clone is performed (bitwise vs RC inc) are backend concerns, not registry concerns. The registry's `MemoryStrategy::Arc` correctly signals that the type MAY require RC management.

2. **`as_bytes`** (spec §8.1.6): Returns a seamless slice (`SLICE_FLAG` in cap field) sharing the same allocation. For SSO strings, this requires materializing the inline bytes to a heap buffer first (the slice must have a stable pointer). This implementation detail does NOT affect the registry's method signature but DOES affect the LLVM codegen and runtime.

3. **Return values**: Transform methods (`to_uppercase`, `trim`, etc.) may return either SSO or heap strings depending on result length. The registry's `ReturnTag::SelfType` is correct regardless — the caller always owns the return value.

4. **ARC pipeline**: The `MemoryStrategy::Arc` classification causes the ARC pass to conservatively insert RC operations for str values. The runtime's SSO check (`flags & 0x80`) makes SSO RC ops into no-ops at negligible cost. This is acceptable — the registry captures the WORST CASE memory strategy, and the runtime optimizes the common case.

### Receiver Ownership

**All str methods borrow their receiver.** String is immutable -- every method reads the content without modifying or consuming it. Methods that return `str` (e.g., `to_uppercase`, `concat`, `trim`) allocate a *new* string with RC=1; the original is untouched.

This is encoded as `receiver_borrows: true` on every `MethodDef` in ori_ir's `BUILTIN_METHODS` for `BuiltinType::Str` (see `compiler/ori_ir/src/builtin_methods/mod.rs`), and as `borrow: true` in every `declare_builtins!` entry in ori_llvm's `collections/mod.rs` and `traits.rs`.

### Parameter Ownership

- **`str` parameters also borrow.** Methods like `contains(substr)`, `starts_with(prefix)`, `concat(other)` take their `str` arguments by borrow. The callee reads but does not consume the argument. No RC increment is needed at the call site for borrowed arguments.
- **`int` parameters are Copy.** Methods like `slice(start, end)`, `repeat(count)`, `pad_start(width, fill)` take `int` args which are trivially copied.

### Return Ownership

| Return Type | Ownership | RC Implication |
|-------------|-----------|----------------|
| `str` (from transform/combine) | New allocation (heap RC=1) or SSO (no alloc) | Caller owns the return value |
| `str` (from `clone`) | RC increment on heap data | `ori_rc_inc` on data pointer (heap only; SSO is a bitwise copy) |
| `str` (from `to_str`) | Identity return (self) | No allocation, no RC change (LLVM returns receiver directly) |
| `int`, `bool` | Copy | No RC involvement |
| `Ordering` | Copy (i8) | No RC involvement |
| `[str]`, `[char]`, `[byte]` | New list allocation | Caller owns the list; elements may be RC'd |
| `Iterator<char>` | New iterator | Iterator holds reference to source string data |
| `Option<int>`, `Option<float>` | Stack value | No RC involvement |
| `Error` (from `into`) | New allocation | Caller owns the error |

### ARC Pipeline Implications

1. **Borrow inference recognizes all str method calls as borrowing.** The `borrowing_builtin_names()` function in `ori_arc/src/borrow/builtins/mod.rs` includes all str method names in the `BORROWING_METHOD_NAMES` array (excluding `iter`).

2. **`iter()` is excluded from borrow set.** Although `iter()` borrows its receiver, the iterator it creates holds a hidden reference to the string's data. The ARC pipeline cannot model this dependency, so `iter()` uses Owned semantics and the runtime manages internal RC.

3. **Operator calls (`+`, `==`, `<`, etc.) pass through `emit_binary_op`.** The receiver is always borrowed (passed by pointer). The `ori_str_concat` return value is a new RC=1 string owned by the caller.

---

## 04.4 Full STR TypeDef Definition

> **WARNING (BLOAT risk):** STR has 43 methods. At ~10 lines per MethodDef struct literal (all 10 frozen fields), the methods alone consume ~430 lines. With OpDefs (25 lines), module docs, TypeDef wrapper, and section comments, `str.rs` will exceed the 500-line file size limit. **A `const fn` helper is REQUIRED** — either:
> 1. `MethodDef::str_instance(name, params, returns, trait_name)` — fills `receiver: Borrow`, `pure: true`, `backend_required: true`, `kind: Instance`, `dei_only: false`, `dei_propagation: NotApplicable` (6 constant fields, keeping each method at ~1 line), OR
> 2. Split into `defs/str/mod.rs` (TypeDef shell + OpDefs) + `defs/str/methods.rs` (method array).
>
> Define the helper in `method.rs` (Section 01/02) BEFORE implementing Section 04. Section 03 establishes the precedent with `MethodDef::primitive()`.

This is the planned `const` Rust definition for the registry, referencing the data model types from Section 01. The first `MethodDef` entry shows all 10 frozen fields; subsequent entries abbreviate fields that share the documented defaults (per frozen decision 13). The final implementation MUST fill in all fields on every entry.

```rust
pub const STR: TypeDef = TypeDef {
    tag: TypeTag::Str,
    name: "str",
    type_params: TypeParamArity::Fixed(0),
    memory: MemoryStrategy::Arc,
    operators: OpDefs {
        add:       OpStrategy::RuntimeCall { fn_name: "ori_str_concat", returns_bool: false },
        sub:       OpStrategy::Unsupported,
        mul:       OpStrategy::Unsupported,
        div:       OpStrategy::Unsupported,
        rem:       OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq:        OpStrategy::RuntimeCall { fn_name: "ori_str_eq", returns_bool: true },
        neq:       OpStrategy::RuntimeCall { fn_name: "ori_str_ne", returns_bool: true },
        lt:        OpStrategy::RuntimeCall { fn_name: "ori_str_compare", returns_bool: true },
        gt:        OpStrategy::RuntimeCall { fn_name: "ori_str_compare", returns_bool: true },
        lt_eq:     OpStrategy::RuntimeCall { fn_name: "ori_str_compare", returns_bool: true },
        gt_eq:     OpStrategy::RuntimeCall { fn_name: "ori_str_compare", returns_bool: true },
        neg:       OpStrategy::Unsupported,
        not:       OpStrategy::Unsupported,
        bit_and:   OpStrategy::Unsupported,
        bit_or:    OpStrategy::Unsupported,
        bit_xor:   OpStrategy::Unsupported,
        bit_not:   OpStrategy::Unsupported,
        shl:       OpStrategy::Unsupported,
        shr:       OpStrategy::Unsupported,
    },
    methods: &[
        // ── Query ──────────────────────────────────────────────────────
        //
        // All str MethodDefs share these defaults (per frozen decision 13):
        //   receiver: Ownership::Borrow,
        //   pure: true,
        //   backend_required: true,
        //   kind: MethodKind::Instance,
        //   dei_only: false,
        //   dei_propagation: DeiPropagation::NotApplicable,
        // First two entries shown in full; remaining instance method
        // entries abbreviate to the 5 per-method fields (name, params,
        // returns, trait_name, receiver). Associated functions show all 10.
        MethodDef {
            name: "len",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
            pure: true,
            backend_required: true,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        },
        MethodDef {
            name: "length",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
            pure: true,
            backend_required: true,
            kind: MethodKind::Instance,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        },
        // ── Remaining entries abbreviate frozen-default fields ─────────
        // All str instance methods below share these frozen defaults:
        //   pure: true,              (all str methods are side-effect free)
        //   backend_required: true,  (unless marked otherwise in coverage matrix)
        //   kind: MethodKind::Instance,
        //   dei_only: false,
        //   dei_propagation: DeiPropagation::NotApplicable,
        //
        // NOTE: Abbreviated entries below omit these 5 constant fields for
        // PLAN READABILITY ONLY. The implementation MUST either (a) use a
        // const fn helper like MethodDef::str_instance() that fills the
        // constant fields, or (b) write out all 10 fields on every entry.
        // Abbreviated entries as shown below WILL NOT COMPILE.
        MethodDef {
            name: "byte_len",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Predicates ─────────────────────────────────────────────────
        MethodDef {
            name: "is_empty",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Bool),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "contains",
            params: &[ParamDef { name: "substr", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::Concrete(TypeTag::Bool),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "starts_with",
            params: &[ParamDef { name: "prefix", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::Concrete(TypeTag::Bool),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "ends_with",
            params: &[ParamDef { name: "suffix", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::Concrete(TypeTag::Bool),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Transform ──────────────────────────────────────────────────
        MethodDef {
            name: "to_uppercase",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "to_lowercase",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "trim",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "trim_start",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "trim_end",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "escape",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "replace",
            params: &[
                ParamDef { name: "pattern", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow },
                ParamDef { name: "replacement", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow },
            ],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "pad_start",
            params: &[
                ParamDef { name: "width", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
                ParamDef { name: "fill", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow },
            ],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "pad_end",
            params: &[
                ParamDef { name: "width", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
                ParamDef { name: "fill", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow },
            ],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Combine ────────────────────────────────────────────────────
        MethodDef {
            name: "concat",
            params: &[ParamDef { name: "other", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "repeat",
            params: &[ParamDef { name: "count", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy }],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "add",
            params: &[ParamDef { name: "other", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::SelfType,
            trait_name: Some("Add"),
            receiver: Ownership::Borrow,
        },

        // ── Extract ────────────────────────────────────────────────────
        MethodDef {
            name: "slice",
            params: &[
                ParamDef { name: "start", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
                ParamDef { name: "end", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
            ],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "substring",
            params: &[
                ParamDef { name: "start", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
                ParamDef { name: "end", ty: ReturnTag::Concrete(TypeTag::Int), ownership: Ownership::Copy },
            ],
            returns: ReturnTag::SelfType,
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Decompose ──────────────────────────────────────────────────
        MethodDef {
            name: "split",
            params: &[ParamDef { name: "sep", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::List(TypeTag::Str),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "lines",
            params: &[],
            returns: ReturnTag::List(TypeTag::Str),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "chars",
            params: &[],
            returns: ReturnTag::List(TypeTag::Char),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "bytes",
            params: &[],
            returns: ReturnTag::List(TypeTag::Byte),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Byte Access (spec §8.1.6) ───────────────────────────────────
        MethodDef {
            name: "as_bytes",
            params: &[],
            returns: ReturnTag::List(TypeTag::Byte),
            trait_name: None,
            receiver: Ownership::Borrow,
            // NOTE: returns seamless slice (zero-copy). Implementation detail,
            // not representable in ReturnTag. LLVM codegen must handle specially.
        },
        MethodDef {
            name: "to_bytes",
            params: &[],
            returns: ReturnTag::List(TypeTag::Byte),
            trait_name: None,
            receiver: Ownership::Borrow,
            // NOTE: returns independent copy (not seamless slice).
        },

        // ── Iteration ──────────────────────────────────────────────────
        MethodDef {
            name: "iter",
            params: &[],
            returns: ReturnTag::DoubleEndedIterator(TypeTag::Char),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Search ─────────────────────────────────────────────────────
        MethodDef {
            name: "index_of",
            params: &[ParamDef { name: "substr", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::Option(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "last_index_of",
            params: &[ParamDef { name: "substr", ty: ReturnTag::Concrete(TypeTag::Str), ownership: Ownership::Borrow }],
            returns: ReturnTag::Option(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Conversion ─────────────────────────────────────────────────
        MethodDef {
            name: "to_int",
            params: &[],
            returns: ReturnTag::Option(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "parse_int",
            params: &[],
            returns: ReturnTag::Option(TypeTag::Int),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "to_float",
            params: &[],
            returns: ReturnTag::Option(TypeTag::Float),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        MethodDef {
            name: "parse_float",
            params: &[],
            returns: ReturnTag::Option(TypeTag::Float),
            trait_name: None,
            receiver: Ownership::Borrow,
        },
        // NOTE: trait_name is None despite implementing the Into<Error> trait (spec §8.11).
        // Builtin into() is hardcoded in the type checker, not resolved via trait dispatch.
        // See "Traits Not Covered by the Registry" section above.
        MethodDef {
            name: "into",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Error),
            trait_name: None,
            receiver: Ownership::Borrow,
        },

        // ── Trait: Eq ──────────────────────────────────────────────────
        MethodDef {
            name: "equals",
            params: &[ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }],
            returns: ReturnTag::Concrete(TypeTag::Bool),
            trait_name: Some("Eq"),
            receiver: Ownership::Borrow,
        },

        // ── Trait: Comparable ──────────────────────────────────────────
        MethodDef {
            name: "compare",
            params: &[ParamDef { name: "other", ty: ReturnTag::SelfType, ownership: Ownership::Borrow }],
            returns: ReturnTag::Concrete(TypeTag::Ordering),
            trait_name: Some("Comparable"),
            receiver: Ownership::Borrow,
        },

        // ── Trait: Clone ───────────────────────────────────────────────
        MethodDef {
            name: "clone",
            params: &[],
            returns: ReturnTag::SelfType,
            trait_name: Some("Clone"),
            receiver: Ownership::Borrow,
        },

        // ── Trait: Hashable ────────────────────────────────────────────
        MethodDef {
            name: "hash",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Int),
            trait_name: Some("Hashable"),
            receiver: Ownership::Borrow,
        },

        // ── Trait: Printable ───────────────────────────────────────────
        MethodDef {
            name: "to_str",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Str),
            trait_name: Some("Printable"),
            receiver: Ownership::Borrow,
        },

        // ── Trait: Debug ───────────────────────────────────────────────
        MethodDef {
            name: "debug",
            params: &[],
            returns: ReturnTag::Concrete(TypeTag::Str),
            trait_name: Some("Debug"),
            receiver: Ownership::Borrow,
        },

        // ── Associated Functions (spec §8.1.6) ──────────────────────────
        // Called as str.from_utf8(bytes:), not s.from_utf8().
        // Uses MethodKind::Associated (frozen decision 9).
        MethodDef {
            name: "from_utf8",
            params: &[ParamDef { name: "bytes", ty: ReturnTag::List(TypeTag::Byte), ownership: Ownership::Owned }],
            returns: ReturnTag::ResultOfProjectionFresh(TypeProjection::Fixed(TypeTag::Str)),
            // NOTE: returns Result<str, Error>. The ReturnTag encoding may need
            // refinement depending on Section 01's final ResultOfProjectionFresh design.
            trait_name: None,
            receiver: Ownership::Borrow, // irrelevant for Associated
            pure: true,
            backend_required: true,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
        },
        MethodDef {
            name: "from_utf8_unchecked",
            params: &[ParamDef { name: "bytes", ty: ReturnTag::List(TypeTag::Byte), ownership: Ownership::Owned }],
            returns: ReturnTag::SelfType, // returns str
            trait_name: None,
            receiver: Ownership::Borrow, // irrelevant for Associated
            pure: true,
            backend_required: true,
            kind: MethodKind::Associated,
            dei_only: false,
            dei_propagation: DeiPropagation::NotApplicable,
            // NOTE: requires `uses Unsafe` capability. The registry does not
            // currently model capability requirements per method. This may need
            // a future field or annotation.
        },
    ],
};
```

### Method Count Summary

| Category | Count | Methods |
|----------|-------|---------|
| Query | 3 | `len`, `length`, `byte_len` |
| Predicate | 4 | `is_empty`, `contains`, `starts_with`, `ends_with` |
| Transform | 9 | `to_uppercase`, `to_lowercase`, `trim`, `trim_start`, `trim_end`, `escape`, `replace`, `pad_start`, `pad_end` |
| Combine | 3 | `concat`, `repeat`, `add` |
| Extract | 2 | `slice`, `substring` |
| Decompose | 4 | `split`, `lines`, `chars`, `bytes` |
| Byte Access | 2 | `as_bytes`, `to_bytes` |
| Iteration | 1 | `iter` |
| Search | 2 | `index_of`, `last_index_of` |
| Conversion | 5 | `to_int`, `parse_int`, `to_float`, `parse_float`, `into` |
| Trait | 6 | `equals`, `compare`, `clone`, `hash`, `to_str`, `debug` |
| Associated | 2 | `from_utf8`, `from_utf8_unchecked` |
| **Total** | **43** | |

### Data Model Requirements for ReturnTag

The STR type definition requires the following `ReturnTag` variants beyond what primitive types need:

- `ReturnTag::SelfType` -- for methods returning `str` (same as receiver type)
- `ReturnTag::Concrete(TypeTag)` -- for `int`, `bool`, `Ordering`, `Error`
- `ReturnTag::List(TypeTag)` -- for `split` -> `[str]`, `chars` -> `[char]`, `bytes` -> `[byte]`, `as_bytes` -> `[byte]`, `to_bytes` -> `[byte]`
- `ReturnTag::Option(TypeTag)` -- for `index_of` -> `Option<int>`, `to_float` -> `Option<float>`
- `ReturnTag::DoubleEndedIterator(TypeTag)` -- for `iter` -> `DoubleEndedIterator<char>`
- `ReturnTag::ResultOfProjectionFresh(TypeProjection)` -- for `from_utf8` -> `Result<str, Error>`

These must be defined in Section 01's data model. If the data model uses a simpler enum without parameterized variants, the parameterized types (`List`, `Option`, `DoubleEndedIterator`) can be encoded as a `ReturnTag::Generic { constructor: TypeTag, element: TypeTag }` variant or similar.

**Const-constructibility check:** All `ReturnTag` variants used in `STR.methods` must be constructible in a `const` context (Section 01 constraint). The first 5 variants above are trivially const-constructible (enum variants with `Copy` payloads like `TypeTag`). The `ResultOfProjectionFresh(TypeProjection)` variant requires that `TypeProjection` itself be const-constructible — verify that `TypeProjection::Fixed(TypeTag)` contains no heap types (`String`, `Vec`, `Box`, `HashMap`) and is `Copy`. If `TypeProjection` cannot be made const-constructible, `from_utf8`'s return type encoding may need a dedicated `ReturnTag::Result(TypeTag, TypeTag)` variant instead.

The STR type definition also requires `MethodKind::Associated` (frozen decision 9) for the `from_utf8` and `from_utf8_unchecked` associated functions. This is the first primitive type in the registry that has associated functions (Duration and Size have them in Section 05, but str is defined in Section 04).

> **COMPLEXITY WARNING:** `ReturnTag::ResultOfProjectionFresh(TypeProjection::Fixed(TypeTag::Str))` is the most complex return type encoding in the registry so far. It requires `TypeProjection` to be defined in Section 01 and to be const-constructible. If Section 01 has not yet designed `TypeProjection`, this encoding will block `from_utf8`. Consider a simpler fallback: `ReturnTag::Result(TypeTag::Str, TypeTag::Error)` — a dedicated two-payload variant that directly encodes `Result<T, E>` without the indirection through `TypeProjection`. This is simpler, const-constructible by construction, and sufficient for all current uses.

---

## 04.5 Validation

### Cross-Reference: Registry vs resolve_str_method

Every arm in `resolve_str_method` (`compiler/ori_types/src/infer/expr/methods/resolve_by_type.rs`) must have a corresponding `MethodDef` in the registry's `STR.methods` array.

- [x] `into` -> `ReturnTag::Concrete(TypeTag::Error)` -- matches `Some(Idx::ERROR)`
- [x] `len` / `byte_len` / `hash` / `length` -> `ReturnTag::Concrete(TypeTag::Int)` -- matches `Some(Idx::INT)`
- [x] `iter` -> `ReturnTag::DoubleEndedIterator(TypeTag::Char)` -- matches `engine.pool_mut().double_ended_iterator(Idx::CHAR)`
- [x] `is_empty` / `starts_with` / `ends_with` / `contains` / `equals` -> `ReturnTag::Concrete(TypeTag::Bool)` -- matches `Some(Idx::BOOL)`
- [x] `to_uppercase` / `to_lowercase` / `trim` / `trim_start` / `trim_end` / `replace` / `repeat` / `pad_start` / `pad_end` / `slice` / `substring` / `clone` / `debug` / `escape` / `concat` / `to_str` -> `ReturnTag::SelfType` or `ReturnTag::Concrete(TypeTag::Str)` -- matches `Some(Idx::STR)`
- [x] `chars` -> `ReturnTag::List(TypeTag::Char)` -- matches `engine.pool_mut().list(Idx::CHAR)`
- [x] `bytes` -> `ReturnTag::List(TypeTag::Byte)` -- matches `engine.pool_mut().list(Idx::BYTE)`
- [x] `split` / `lines` -> `ReturnTag::List(TypeTag::Str)` -- matches `engine.pool_mut().list(Idx::STR)`
- [x] `index_of` / `last_index_of` / `to_int` / `parse_int` -> `ReturnTag::Option(TypeTag::Int)` -- matches `engine.pool_mut().option(Idx::INT)`
- [x] `to_float` / `parse_float` -> `ReturnTag::Option(TypeTag::Float)` -- matches `engine.pool_mut().option(Idx::FLOAT)`
- [x] `compare` -> `ReturnTag::Concrete(TypeTag::Ordering)` -- matches `Some(Idx::ORDERING)`

**Result: 38/38 type-checker methods covered. No gaps.**

**Registry additions beyond resolve_str_method (4):**
- `as_bytes` -> `ReturnTag::List(TypeTag::Byte)` — spec §8.1.6, not yet in type checker
- `to_bytes` -> `ReturnTag::List(TypeTag::Byte)` — spec §8.1.6, not yet in type checker
- `from_utf8` -> `ReturnTag::ResultOfProjectionFresh(...)` — spec §8.1.6, associated function, not yet in type checker
- `from_utf8_unchecked` -> `ReturnTag::SelfType` — spec §8.1.6, associated function, not yet in type checker

These will be added to `resolve_str_method` (or a new `resolve_str_associated`) during the wiring phase (Section 09).

### Cross-Reference: Registry vs ori_ir BUILTIN_METHODS (str section)

The ori_ir `BUILTIN_METHODS` array for `BuiltinType::Str` (`compiler/ori_ir/src/builtin_methods/mod.rs`) contains 18 entries: 5 via trait helper constructors (`comparable`, `eq_trait`, `clone_trait`, `hash_trait`, `debug_trait`) and 13 via explicit `MethodDef::new()`:

| ori_ir Entry | Registry Entry | Match? |
|-------------|----------------|--------|
| `compare` (Comparable) | `compare` (Some("Comparable")) | Y |
| `equals` (Eq) | `equals` (Some("Eq")) | Y |
| `clone` (Clone) | `clone` (Some("Clone")) | Y |
| `hash` (Hashable) | `hash` (Some("Hashable")) | Y |
| `debug` (Debug) | `debug` (Some("Debug")) | Y |
| `len` (no trait) | `len` (None) | Y |
| `is_empty` (no trait) | `is_empty` (None) | Y |
| `contains` (Str param) | `contains` (Str param) | Y |
| `starts_with` (Str param) | `starts_with` (Str param) | Y |
| `ends_with` (Str param) | `ends_with` (Str param) | Y |
| `to_uppercase` (SelfType return) | `to_uppercase` (SelfType) | Y |
| `to_lowercase` (SelfType return) | `to_lowercase` (SelfType) | Y |
| `trim` (SelfType return) | `trim` (SelfType) | Y |
| `escape` (SelfType return) | `escape` (SelfType) | Y |
| `add` (Str param, Add trait) | `add` (Some("Add")) | Y |
| `concat` (Str param) | `concat` (Str param) | Y |
| `replace` (2 Str params) | `replace` (2 Str params) | Y |
| `repeat` (Int param) | `repeat` (Int param) | Y |

**Note:** The ori_ir `BUILTIN_METHODS` does NOT include `to_str` (Printable) for str. This is tracked in `EVAL_METHODS_NOT_IN_IR` (`compiler/oric/src/eval/tests/methods/consistency.rs`). The registry includes it because the registry is the COMPLETE specification, not limited by ori_ir's current coverage.

**Result: All 18 ori_ir entries are present in the registry. The registry adds 25 additional methods (typeck-only, eval-only, and spec-only ones) beyond what ori_ir covers.**

### Cross-Reference: Registry vs ori_llvm str builtins

The ori_llvm phase handles str methods across two submodules:

**collections/mod.rs (19 entries):** `clone`, `length`, `len`, `is_empty`, `concat`, `to_str`, `contains`, `starts_with`, `ends_with`, `trim`, `substring`, `slice`, `to_uppercase`, `to_lowercase`, `replace`, `repeat`, `chars`, `split`, `iter`
**traits.rs (8 entries):** `equals`, `is_equal`, `compare`, `hash`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`

All 27 LLVM entries correspond to registry methods. The 5 comparison predicates (`is_equal`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`) are derived from the `Comparable` trait's `compare` method and exist only at the LLVM codegen level. They do not need explicit `MethodDef` entries because they are lowered from operator syntax and trait dispatch, not from user-visible method calls.

**Missing from LLVM (2):** `debug`, `escape` -- these str methods have ori_ir entries and eval implementations but no LLVM codegen yet.

### Cross-Reference: Registry vs ori_eval dispatch_string_method

The evaluator's `dispatch_string_method` (`compiler/ori_eval/src/methods/collections.rs`) handles 25 distinct method names:

`len`, `length`, `is_empty`, `to_uppercase`, `to_lowercase`, `trim`, `contains`, `starts_with`, `ends_with`, `add`, `concat`, `substring`, `slice`, `compare`, `equals`, `iter`, `clone`, `to_str`, `escape`, `debug`, `hash`, `replace`, `split`, `repeat`, `into`

All 25 are present in the registry. The registry has 18 additional methods not in the evaluator: 14 typeck-only methods (`byte_len`, `bytes`, `chars`, `index_of`, `last_index_of`, `lines`, `pad_end`, `pad_start`, `parse_float`, `parse_int`, `to_float`, `to_int`, `trim_end`, `trim_start`) and 4 spec-only methods (`as_bytes`, `to_bytes`, `from_utf8`, `from_utf8_unchecked`).

### Runtime Functions Cross-Reference

| Registry OpStrategy / Method | Runtime Function | ori_rt Location | ori_llvm Declaration |
|------------------------------|-----------------|-----------------|---------------------|
| `add` operator | `ori_str_concat` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `eq` operator | `ori_str_eq` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `neq` operator | `ori_str_ne` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `lt`/`gt`/`lt_eq`/`gt_eq` operators | `ori_str_compare` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `hash` method | `ori_str_hash` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `len` method (internal) | `ori_str_len` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `data` access (internal) | `ori_str_data` | `string/ops.rs` | `runtime_decl/runtime_functions.rs` |
| `iter` method (internal) | `ori_str_next_char` | `string/methods/mod.rs` | `runtime_decl/runtime_functions.rs` |
| `to_str` (on int) | `ori_str_from_int` | `string/convert.rs` | `runtime_decl/runtime_functions.rs` |
| `to_str` (on bool) | `ori_str_from_bool` | `string/convert.rs` | `runtime_decl/runtime_functions.rs` |
| `to_str` (on float) | `ori_str_from_float` | `string/convert.rs` | `runtime_decl/runtime_functions.rs` |
| literal construction | `ori_str_from_raw` | `string/convert.rs` | `runtime_decl/runtime_functions.rs` |

---

## Implementation Checklist

### Prerequisites (upstream, must complete before str.rs)

- [x] Ensure Section 01 data model supports `ReturnTag::List(TypeTag)`, `ReturnTag::Option(TypeTag)`, `ReturnTag::DoubleEndedIterator(TypeTag)` variants — **added to Section 01 ReturnTag enum**
- [x] Ensure Section 01 data model supports `ReturnTag::ResultOfProjectionFresh(TypeProjection)` for `from_utf8` -> `Result<str, Error>`. **Verified: `TypeProjection` is `Copy` + const-constructible.**
- [x] Section 01/02: `MethodDef::primitive()` already serves as the helper — same signature, `Ownership::Borrow` passed explicitly. No dedicated `str_instance()` needed. `str.rs` = 193 lines (well under 500).

### Definition

- [x] Define `STR` const in `ori_registry/src/defs/str.rs`
- [x] Include all 43 methods with exact parameter and return types (38 from resolve_str_method + `add` from Add trait + 2 spec byte access + 2 spec associated functions)
- [x] Include all 20 operator strategy entries (7 active RuntimeCall + 13 Unsupported)
- [x] Set `memory: MemoryStrategy::Arc`
- [x] Set `receiver: Ownership::Borrow` on every instance method
- [x] Set `kind: MethodKind::Associated` on `from_utf8` and `from_utf8_unchecked`
- [x] Verify every `MethodDef` entry has all 10 frozen fields (per frozen decision 13) — no abbreviated struct literals

### Compilation

- [x] Verify `cargo c -p ori_registry` compiles
- [x] Verify no source file exceeds 500 lines (excluding test files) — str.rs = 193 lines

### Tests
**Test file location:** `ori_registry/src/defs/tests.rs` (shared with section-03 primitive tests, str.rs is a single file).

- [x] Write unit test: `str_method_count()` asserts exactly 43 methods
- [x] Write unit test: `str_all_instance_methods_borrow_receiver()` asserts every instance method has `Ownership::Borrow`
- [x] Write unit test: `str_operators_all_runtime_call_or_unsupported()` asserts no `IntInstr`/`FloatInstr` strategies
- [x] Write unit test: `str_runtime_call_names_are_valid()` asserts all `fn_name` values start with `ori_str_`
- [x] Write unit test: `str_trait_methods_have_trait_name()` asserts `equals`/`compare`/`clone`/`hash`/`to_str`/`debug`/`add` have non-None `trait_name`
- [x] Write unit test: `str_associated_functions()` asserts `from_utf8`/`from_utf8_unchecked` have `kind: MethodKind::Associated`
- [x] Write unit test: `str_alias_pairs_have_matching_signatures()` asserts `length` == `len`, `substring` == `slice`, `parse_int` == `to_int`, `parse_float` == `to_float` (same params + returns)
- [x] Verify plan against Ori spec — `as_bytes`, `to_bytes`, `from_utf8`, `from_utf8_unchecked` included per §8.1.6; `into` per §8.11; `iter` returning DEI<char> per §8.13.1
- [x] Write unit test: `str_opdefs_has_all_20_fields()` — covered by existing `opdefs_has_all_20_fields` (tests all BUILTIN_TYPES)
- [x] Additional tests: `str_is_arc`, `str_comparison_operators_use_ori_str_compare`, `str_neq_uses_ori_str_ne`
- [x] Run `./test-all.sh` and verify no regressions — 12,235 tests pass, 0 failures

## Exit Criteria

1. `STR` const compiles as part of `ori_registry`
2. Every method in `resolve_str_method` has a corresponding `MethodDef` in `STR.methods`
3. Every entry in ori_ir `BUILTIN_METHODS` for `BuiltinType::Str` has a corresponding `MethodDef`
4. Every `("str", ...)` entry in ori_llvm `declare_builtins!` has a corresponding `MethodDef`
5. All unit tests pass
6. The `STR` definition is the single source of truth for the string type's complete behavioral contract
7. Every spec-defined str method (§8.1.6) has a corresponding `MethodDef`, even if not yet implemented in compiler phases
8. Associated functions use `MethodKind::Associated` and are distinguishable from instance methods via the query API
9. Alias pairs have identical signatures (enforced by unit test)
10. `./test-all.sh` passes (no regressions in existing crates)
11. No source file exceeds 500 lines (excluding test files)
12. Test file uses sibling `tests.rs` convention
13. Every `MethodDef` entry has all 10 frozen fields (per frozen decision 13) — no abbreviated struct literals
