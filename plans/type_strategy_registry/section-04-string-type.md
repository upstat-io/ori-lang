---
plan: "type_strategy_registry"
section: "04"
title: "String Type Definition"
status: not-started
depends_on:
  - "01"
  - "02"
estimated_lines: 150
complexity: medium
---

# Section 04: String Type Definition

## Overview

STR is the most complex primitive type in the registry. Unlike int/float/bool/byte/char (all `MemoryStrategy::Copy`), str uses `MemoryStrategy::Arc` -- it is reference-counted, heap-allocated, and immutable. Its operators use `RuntimeCall` to delegate to `ori_rt` functions rather than emitting native LLVM instructions. It has the largest method surface of any primitive type (38 methods across ori_types, 19 in ori_eval, 13 inherent + 6 trait in ori_ir, and 19 in ori_llvm).

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

### Cross-Phase Reconciliation Table

| Method | ori_types | ori_eval | ori_ir | ori_llvm | Status |
|--------|:---------:|:--------:|:------:|:--------:|--------|
| `add` | - | Y | Y | - (operator) | **Operator alias** -- `+` desugars to `ori_str_concat` in LLVM |
| `byte_len` | Y | - | - | - | **Typeck-only** |
| `bytes` | Y | - | - | - | **Typeck-only** |
| `chars` | Y | - | - | - | **Typeck-only** |
| `clone` | Y | Y | Y | Y | Complete |
| `compare` | Y | Y | Y | Y | Complete |
| `concat` | Y | Y | Y | Y | Complete |
| `contains` | Y | Y | Y | - | **Missing LLVM** |
| `debug` | Y | Y | Y | - | **Missing LLVM** |
| `ends_with` | Y | Y | Y | - | **Missing LLVM** |
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
| `length` | Y | - | - | Y | **Partial** (typeck + LLVM, alias of `len`) |
| `lines` | Y | - | - | - | **Typeck-only** |
| `pad_end` | Y | - | - | - | **Typeck-only** |
| `pad_start` | Y | - | - | - | **Typeck-only** |
| `parse_float` | Y | - | - | - | **Typeck-only** |
| `parse_int` | Y | - | - | - | **Typeck-only** |
| `repeat` | Y | - | - | - | **Typeck-only** |
| `replace` | Y | - | - | - | **Typeck-only** |
| `slice` | Y | - | - | - | **Typeck-only** |
| `split` | Y | - | - | - | **Typeck-only** |
| `starts_with` | Y | Y | Y | - | **Missing LLVM** |
| `substring` | Y | - | - | - | **Typeck-only** |
| `to_float` | Y | - | - | - | **Typeck-only** |
| `to_int` | Y | - | - | - | **Typeck-only** |
| `to_lowercase` | Y | Y | Y | - | **Missing LLVM** |
| `to_str` | Y | Y | - | Y | **Missing IR** |
| `to_uppercase` | Y | Y | Y | - | **Missing LLVM** |
| `trim` | Y | Y | Y | - | **Missing LLVM** |
| `trim_end` | Y | - | - | - | **Typeck-only** |
| `trim_start` | Y | - | - | - | **Typeck-only** |

### Gap Summary

- **Complete across all 4 phases (8):** `clone`, `compare`, `concat`, `equals`, `hash`, `is_empty`, `len`, `iter` (missing IR but present in eval+LLVM)
- **Typeck-only (18):** `byte_len`, `bytes`, `chars`, `index_of`, `last_index_of`, `length`, `lines`, `pad_end`, `pad_start`, `parse_float`, `parse_int`, `repeat`, `replace`, `slice`, `split`, `substring`, `to_float`, `to_int`, `trim_end`, `trim_start`
- **Missing LLVM (6):** `contains`, `debug`, `ends_with`, `escape`, `starts_with`, `to_lowercase`, `to_uppercase`, `trim`
- **LLVM-only comparison predicates (5):** `is_equal`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal` -- these are generated from the `Comparable` trait and only exist at the LLVM level as lowered dispatch targets.

---

## 04.2 STR Operator Strategies

### Operator Table

| Operator | Ori Syntax | OpStrategy | Runtime Function | Notes |
|----------|-----------|------------|-----------------|-------|
| `add` | `a + b` | `RuntimeCall("ori_str_concat")` | `ori_str_concat(ptr, ptr) -> OriStr` | Allocates new string; RC=1 |
| `eq` | `a == b` | `RuntimeCall("ori_str_eq")` | `ori_str_eq(ptr, ptr) -> bool` | Byte-level comparison |
| `neq` | `a != b` | `RuntimeCall("ori_str_ne")` | `ori_str_ne(ptr, ptr) -> bool` | Negation of `ori_str_eq` |
| `lt` | `a < b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(ptr, ptr) -> i8` | Result == 0 (Less) |
| `gt` | `a > b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(ptr, ptr) -> i8` | Result == 2 (Greater) |
| `lt_eq` | `a <= b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(ptr, ptr) -> i8` | Result != 2 |
| `gt_eq` | `a >= b` | `RuntimeCall("ori_str_compare")` + check | `ori_str_compare(ptr, ptr) -> i8` | Result != 0 |
| `sub` | `a - b` | `Unsupported` | - | - |
| `mul` | `a * b` | `Unsupported` | - | - |
| `div` | `a / b` | `Unsupported` | - | - |
| `mod` | `a % b` | `Unsupported` | - | - |
| `neg` | `-a` | `Unsupported` | - | - |

### Why RuntimeCall?

String operators cannot use native LLVM instructions because:

1. **Strings are variable-length, heap-allocated structures.** The LLVM `{i64 len, ptr data}` representation cannot be compared or concatenated with a single instruction -- it requires dereferencing the data pointer, iterating over bytes, and potentially allocating new memory.

2. **`ori_str_concat` must allocate.** Concatenation creates a new string with a new RC-managed heap allocation (`ori_rc_alloc`). This is fundamentally different from `add` on `i64` which is a single ALU instruction.

3. **`ori_str_compare` does byte-level lexicographic ordering.** This cannot be expressed as a single `icmp` -- it requires looping over bytes with length awareness. The runtime function returns an `i8` Ordering tag (Less=0, Equal=1, Greater=2), which the LLVM backend then checks against the expected value.

4. **The comparison bug.** The string ordering operators (`<`, `>`, `<=`, `>=`) were broken before commit `0bed4d75` because `emit_binary_op` lacked `is_str` guards -- it fell through to `icmp_slt`/`icmp_sgt` which compared raw `{i64, ptr}` struct values instead of string content. The `OpStrategy::RuntimeCall` pattern in the registry makes this impossible by design: if the strategy says `RuntimeCall`, the backend *must* call the runtime function.

### ABI Convention

All `ori_str_*` runtime functions take `*const OriStr` (pointer to `{i64 len, ptr data}`). The LLVM backend creates entry-block allocas, stores the `{i64, ptr}` value, and passes the alloca pointer. This is documented in `emit_str_runtime_call` (arc_emitter/mod.rs, line 2497).

Functions returning `OriStr` return the struct by value (`{i64, ptr}`). Functions returning `bool` or `i8` return scalars directly.

---

## 04.3 STR Ownership Semantics

### Memory Strategy

```
MemoryStrategy::Arc
```

The `str` type in Ori is a reference-counted, immutable string. At the runtime level it is represented as:

```rust
// ori_rt/src/lib.rs, line 78
pub struct OriStr {
    pub len: i64,       // byte length
    pub data: *const u8, // pointer to RC-managed heap data
}
```

The `data` pointer points into an `ori_rc_alloc`-managed allocation with a hidden reference count header (8 bytes before the data).

### Receiver Ownership

**All str methods borrow their receiver.** String is immutable -- every method reads the content without modifying or consuming it. Methods that return `str` (e.g., `to_uppercase`, `concat`, `trim`) allocate a *new* string with RC=1; the original is untouched.

This is encoded as `receiver_borrows: true` on every `MethodDef` in ori_ir's `BUILTIN_METHODS` for `BuiltinType::Str`, and as `borrow: true` in every `declare_builtins!` entry in ori_llvm.

### Parameter Ownership

- **`str` parameters also borrow.** Methods like `contains(substr)`, `starts_with(prefix)`, `concat(other)` take their `str` arguments by borrow. The callee reads but does not consume the argument. No RC increment is needed at the call site for borrowed arguments.
- **`int` parameters are Copy.** Methods like `slice(start, end)`, `repeat(count)`, `pad_start(width, fill)` take `int` args which are trivially copied.

### Return Ownership

| Return Type | Ownership | RC Implication |
|-------------|-----------|----------------|
| `str` (from transform/combine) | New allocation, RC=1 | Caller owns the return value |
| `str` (from `clone`) | RC increment on original | `ori_rc_inc` on data pointer |
| `str` (from `to_str`) | Identity return (self) | No allocation, no RC change (LLVM returns receiver directly) |
| `int`, `bool` | Copy | No RC involvement |
| `Ordering` | Copy (i8) | No RC involvement |
| `[str]`, `[char]`, `[byte]` | New list allocation | Caller owns the list; elements may be RC'd |
| `Iterator<char>` | New iterator | Iterator holds reference to source string data |
| `Option<int>`, `Option<float>` | Stack value | No RC involvement |
| `Error` (from `into`) | New allocation | Caller owns the error |

### ARC Pipeline Implications

1. **Borrow inference recognizes all str method calls as borrowing.** The `borrowing_builtin_names()` function in `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs` includes every str method with `receiver_borrowed: true` (excluding `iter`).

2. **`iter()` is excluded from borrow set.** Although `iter()` borrows its receiver, the iterator it creates holds a hidden reference to the string's data. The ARC pipeline cannot model this dependency, so `iter()` uses Owned semantics and the runtime manages internal RC.

3. **Operator calls (`+`, `==`, `<`, etc.) pass through `emit_binary_op`.** The receiver is always borrowed (passed by pointer). The `ori_str_concat` return value is a new RC=1 string owned by the caller.

---

## 04.4 Full STR TypeDef Definition

This is the exact `const` Rust definition for the registry. It references the data model types from Section 01.

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
        //   kind: MethodKind::Instance,
        //   dei_only: false,
        //   dei_propagation: DeiPropagation::NotApplicable,
        // Only `pure` and `backend_required` vary per method.
        // First entry shown in full; remaining entries abbreviate to
        // the 5 fields that vary (name, params, returns, receiver, trait_name,
        // pure, backend_required).
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
            // kind, dei_only, dei_propagation: same defaults as above
        },
        // ── Remaining entries abbreviate frozen-default fields ─────────
        // All str methods below share these frozen defaults:
        //   pure: true,              (all str methods are side-effect free)
        //   backend_required: true,  (unless marked otherwise in coverage matrix)
        //   kind: MethodKind::Instance,
        //   dei_only: false,
        //   dei_propagation: DeiPropagation::NotApplicable,
        // Implementation MUST fill in all 10 MethodDef fields per frozen decision 13.
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
    ],
};
```

### Method Count Summary

| Category | Count | Methods |
|----------|-------|---------|
| Query | 3 | `len`, `length`, `byte_len` |
| Predicate | 4 | `is_empty`, `contains`, `starts_with`, `ends_with` |
| Transform | 8 | `to_uppercase`, `to_lowercase`, `trim`, `trim_start`, `trim_end`, `escape`, `replace`, `pad_start`, `pad_end` |
| Combine | 3 | `concat`, `repeat`, `add` |
| Extract | 2 | `slice`, `substring` |
| Decompose | 4 | `split`, `lines`, `chars`, `bytes` |
| Iteration | 1 | `iter` |
| Search | 2 | `index_of`, `last_index_of` |
| Conversion | 5 | `to_int`, `parse_int`, `to_float`, `parse_float`, `into` |
| Trait | 6 | `equals`, `compare`, `clone`, `hash`, `to_str`, `debug` |
| **Total** | **38** | |

### Data Model Requirements for ReturnTag

The STR type definition requires the following `ReturnTag` variants beyond what primitive types need:

- `ReturnTag::SelfType` -- for methods returning `str` (same as receiver type)
- `ReturnTag::Concrete(TypeTag)` -- for `int`, `bool`, `Ordering`, `Error`
- `ReturnTag::List(TypeTag)` -- for `split` -> `[str]`, `chars` -> `[char]`, `bytes` -> `[byte]`
- `ReturnTag::Option(TypeTag)` -- for `index_of` -> `Option<int>`, `to_float` -> `Option<float>`
- `ReturnTag::DoubleEndedIterator(TypeTag)` -- for `iter` -> `DoubleEndedIterator<char>`

These must be defined in Section 01's data model. If the data model uses a simpler enum without parameterized variants, the parameterized types (`List`, `Option`, `DoubleEndedIterator`) can be encoded as a `ReturnTag::Generic { constructor: TypeTag, element: TypeTag }` variant or similar.

---

## 04.5 Validation

### Cross-Reference: Registry vs resolve_str_method

Every arm in `resolve_str_method` (ori_types, lines 589-611) must have a corresponding `MethodDef` in the registry's `STR.methods` array.

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

**Result: 38/38 methods covered. No gaps.**

### Cross-Reference: Registry vs ori_ir BUILTIN_METHODS (str section)

The ori_ir `BUILTIN_METHODS` array for `BuiltinType::Str` (lines 462-554) contains 13 entries:

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

**Note:** The ori_ir `BUILTIN_METHODS` does NOT include `to_str` (Printable). This is tracked in `EVAL_METHODS_NOT_IN_IR` (consistency.rs, line 65). The registry includes it because the registry is the COMPLETE specification, not limited by ori_ir's current coverage.

**Result: All 16 ori_ir entries are present in the registry. The registry adds 22 additional methods (the typeck-only and eval-only ones).**

### Cross-Reference: Registry vs ori_llvm str builtins

The ori_llvm phase handles str methods across two submodules:

**collections.rs (7 entries):** `clone`, `length`, `len`, `is_empty`, `concat`, `to_str`, `iter`
**traits.rs (8 entries):** `equals`, `is_equal`, `compare`, `hash`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`

All 15 LLVM entries correspond to registry methods. The 5 comparison predicates (`is_equal`, `is_less`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`) are derived from the `Comparable` trait's `compare` method and exist only at the LLVM codegen level. They do not need explicit `MethodDef` entries because they are lowered from operator syntax and trait dispatch, not from user-visible method calls.

### Cross-Reference: Registry vs ori_eval dispatch_string_method

The evaluator's `dispatch_string_method` (ori_eval, collections.rs lines 93-177) handles 19 methods:

`len`, `is_empty`, `to_uppercase`, `to_lowercase`, `trim`, `contains`, `starts_with`, `ends_with`, `add`, `concat`, `compare`, `equals`, `iter`, `clone`, `to_str`, `escape`, `debug`, `hash`, `into`

All 19 are present in the registry. The registry adds 19 additional methods that type-check but are not yet implemented in the evaluator (tracked in `TYPECK_METHODS_NOT_IN_EVAL`, consistency.rs lines 612-632).

### Runtime Functions Cross-Reference

| Registry OpStrategy / Method | Runtime Function | ori_rt Location | ori_llvm Declaration |
|------------------------------|-----------------|-----------------|---------------------|
| `add` operator | `ori_str_concat` | lib.rs:944 | runtime_decl/mod.rs:97 |
| `eq` operator | `ori_str_eq` | lib.rs:961 | runtime_decl/mod.rs:98 |
| `neq` operator | `ori_str_ne` | lib.rs:978 | runtime_decl/mod.rs:99 |
| `lt`/`gt`/`lt_eq`/`gt_eq` operators | `ori_str_compare` | lib.rs:986 | runtime_decl/mod.rs:100 |
| `hash` method | `ori_str_hash` | lib.rs:1011 | runtime_decl/mod.rs:101 |
| `iter` method (internal) | `ori_str_next_char` | lib.rs:1281 | runtime_decl/mod.rs:118 |
| `to_str` (on int) | `ori_str_from_int` | lib.rs:1037 | runtime_decl/mod.rs:125 |
| `to_str` (on bool) | `ori_str_from_bool` | lib.rs:1073 | runtime_decl/mod.rs:126 |
| `to_str` (on float) | `ori_str_from_float` | lib.rs:1080 | runtime_decl/mod.rs:127 |
| literal construction | `ori_str_from_raw` | lib.rs:1046 | runtime_decl/mod.rs:124 |

---

## Implementation Checklist

- [x] Ensure Section 01 data model supports `ReturnTag::List(TypeTag)`, `ReturnTag::Option(TypeTag)`, `ReturnTag::DoubleEndedIterator(TypeTag)` variants — **added to Section 01 ReturnTag enum**
- [ ] Define `STR` const in `ori_registry/src/defs/str.rs`
- [ ] Include all 38 methods with exact parameter and return types
- [ ] Include all 14 operator strategy entries
- [ ] Set `memory: MemoryStrategy::Arc`
- [ ] Set `receiver: Ownership::Borrow` on every method
- [ ] Verify `cargo c -p ori_registry` compiles
- [ ] Write unit test: `str_method_count()` asserts exactly 38 methods
- [ ] Write unit test: `str_all_methods_borrow_receiver()` asserts every method has `Ownership::Borrow`
- [ ] Write unit test: `str_operators_all_runtime_call_or_unsupported()` asserts no `IntInstr`/`FloatInstr` strategies
- [ ] Write unit test: `str_runtime_call_names_are_valid()` asserts all `fn_name` values start with `ori_str_`
- [ ] Write unit test: `str_trait_methods_have_trait_name()` asserts `equals`/`compare`/`clone`/`hash`/`to_str`/`debug` have non-None `trait_name`

## Exit Criteria

1. `STR` const compiles as part of `ori_registry`
2. Every method in `resolve_str_method` has a corresponding `MethodDef` in `STR.methods`
3. Every entry in ori_ir `BUILTIN_METHODS` for `BuiltinType::Str` has a corresponding `MethodDef`
4. Every `("str", ...)` entry in ori_llvm `declare_builtins!` has a corresponding `MethodDef`
5. All unit tests pass
6. The `STR` definition is the single source of truth for the string type's complete behavioral contract
