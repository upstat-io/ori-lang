---
section: "01"
title: Core Data Model Design
status: not-started
goal: Design all Rust types for the pure-data type strategy registry
sections:
  - id: "01.1"
    title: TypeTag Enum
    status: not-started
  - id: "01.2"
    title: MemoryStrategy Enum
    status: not-started
  - id: "01.3"
    title: Ownership Enum
    status: not-started
  - id: "01.4"
    title: OpStrategy Enum
    status: not-started
  - id: "01.5"
    title: ParamDef Struct
    status: not-started
  - id: "01.6"
    title: MethodDef Struct
    status: not-started
  - id: "01.7"
    title: OpDefs Struct
    status: not-started
  - id: "01.8"
    title: TypeDef Struct
    status: not-started
  - id: "01.8c"
    title: DeiPropagation Enum
    status: not-started
  - id: "01.9"
    title: Extensibility Design
    status: not-started
---

# Section 01: Core Data Model Design

**Why this section is first:** Nothing else can begin until these types are finalized. Sections 03-07 (type definitions) instantiate these structs/enums as `const` values. Sections 09-13 (wiring) pattern-match on these enums. A change to any type here propagates to every section, so we design once and design correctly.

---

## Const-Constructibility Constraint

Every type in this section MUST be constructible in a `const` context. This means:

- No `String`, `Vec`, `HashMap`, `Box`, or any heap-allocated container
- No `&dyn Trait` (not const-constructible in current stable Rust)
- Slices are `&'static [T]` only (pointing to `static` arrays)
- Strings are `&'static str` only
- No `Default` trait usage in construction (fields are explicit)
- All field types must themselves be `const`-constructible

This constraint ensures all registry data lives in `.rodata` (read-only data segment) with zero runtime initialization cost. The entire registry is baked into the binary at compile time.

---

## 01.1 TypeTag Enum

### Purpose

`TypeTag` is the universal type identity for all builtin types in the registry. It answers the question "which type is this?" without carrying any Pool index, type parameter, or phase-specific data. Every `TypeDef` in the registry is keyed by exactly one `TypeTag`.

### Rust Definition

```rust
/// Universal identity tag for all builtin types in the registry.
///
/// This is the registry's type discriminant. It identifies WHAT type
/// something is, independent of type parameters (List vs List<int>),
/// phase representation (Idx vs TypeInfo), or memory layout.
///
/// Exhaustive: adding a new builtin type requires a new variant here,
/// which produces compile errors in every consuming phase's match arms.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TypeTag {
    // Primitive value types (Copy semantics)
    Int,
    Float,
    Bool,
    Char,
    Byte,

    // Special value types (Copy semantics)
    Unit,
    Never,
    Duration,
    Size,
    Ordering,

    // Reference types (Arc semantics)
    Str,
    Error,

    // Generic containers (Arc semantics)
    List,
    Map,
    Set,
    Range,
    Tuple,
    Option,
    Result,
    Channel,

    // Callable/iterator types (Arc semantics)
    Function,
    Iterator,
    DoubleEndedIterator,
}
```

### Design Decisions

1. **`#[repr(u8)]`**: Guarantees a single-byte discriminant. This is important because `TypeTag` is stored in `TypeDef` and compared frequently. A `u8` repr also allows `TypeTag` to be used as an array index (up to 256 variants) for O(1) lookup tables in consuming phases.

2. **No `SelfType`**: The overview mentions `SelfType` as a possible variant, but `SelfType` is a type *variable* used during inference, not a concrete builtin type. It belongs in `TypeTag` only if the registry needs to describe methods whose return type is "the receiver's type" -- and that is handled by `ReturnTag::SelfType` in the method definition, not in the type identity enum.

3. **No `Void`**: Ori uses `Unit` for the `()` type. The name `Void` appears in the existing `ori_ir::ReturnSpec::Void`, but that represents "this method returns `()`", which maps to `TypeTag::Unit` in the registry.

4. **Separate `Iterator` and `DoubleEndedIterator`**: These are distinct types in the type system (`Tag::Iterator` vs `Tag::DoubleEndedIterator` in `ori_types`). A `DoubleEndedIterator` has a strict superset of `Iterator`'s methods. The registry must distinguish them to declare the extra methods (`next_back`, `rev`, `last`, `rfind`, `rfold`).

5. **Ordering within the enum**: Primitives first, then special value types, then reference types, then containers, then callable/iterator. This does NOT match `ori_types::Tag` discriminant ordering (Tag has `Str=3`, `Error=8` among primitives 0-11; TypeTag groups them separately). This is not load-bearing (no code should depend on discriminant order) but the divergence should be documented.

6. **No `Borrowed` variant**: `Borrowed` is a type modifier in `ori_types` (wrapping another type), not a standalone builtin type. The registry doesn't need to describe it.

7. **`Function` type has no methods in the registry**: `TypeTag::Function` exists because the ARC pipeline and LLVM backend need to classify closures/functions for memory management (`MemoryStrategy::Arc` — closures are heap-allocated). However, functions have no builtin methods (no `f.call()` — invocation is an expression form, not a method call). The `TypeDef` for `Function` will have `methods: &[]` (empty) and `operators: OpDefs::UNSUPPORTED`. Its `type_params` is `TypeParamArity::Variadic` because function signatures have arbitrary param counts. The type's value to the registry is its `MemoryStrategy::Arc` declaration and its presence in the exhaustive `TypeTag` enum — not method registration.

8. **`base_type()` for DEI aliasing**: `DoubleEndedIterator.base_type()` returns `Iterator` because the single-TypeDef model (Section 07 Decision 1) stores all iterator methods on one `TypeDef` keyed by `TypeTag::Iterator`. The query API (Section 08) uses `base_type()` to resolve the alias before looking up the `TypeDef`, then applies `dei_only` filtering to include or exclude DEI-only methods. For all other variants, `base_type()` returns `self`. This is a `const fn` (simple match with no allocations).

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_types::Tag` (37 variants) | Tag enum including type variables, schemes, projections | `TypeTag` (23 variants, concrete builtins only) |
| `ori_ir::BuiltinType` (18 variants) | Separate enum with different ordering | Consolidated into `TypeTag` (23 variants: adds Error, Tuple, Iterator, DoubleEndedIterator, Function) |
| `ori_llvm::TypeInfo` (24 variants) | LLVM-specific type classification | `TypeTag` for identity; `TypeInfo` remains for LLVM layout |
| `ori_arc::ArcClass` classification matches | `classify_primitive()`: 3-arm match on 12 `Idx` constants; `classify_by_tag()`: 11-arm match on `Tag` | `type_def.memory == MemoryStrategy::Copy` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_types** | Bridge `Tag` -> `TypeTag` for registry lookup; validate method existence |
| **ori_eval** | Bridge `Value` discriminant -> `TypeTag` for dispatch table lookup |
| **ori_arc** | Read `TypeDef.memory` keyed by `TypeTag` instead of hard-coded `ArcClass` match |
| **ori_llvm** | Bridge `TypeInfo` variant -> `TypeTag` for operator strategy and method ownership |
| **ori_ir** | Replace `BuiltinType` enum (superset of its variants) |

### Checklist

- [ ] Define `TypeTag` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Add `#[repr(u8)]` for compact representation
- [ ] Implement `TypeTag::name(&self) -> &'static str` (returns the Ori-level name: `"int"`, `"float"`, `"str"`, etc.)
- [ ] Implement `TypeTag::all() -> &'static [TypeTag]` (slice of all variants, for enumeration)
- [ ] Add `TypeTag::is_primitive(&self) -> bool` predicate
- [ ] Add `TypeTag::is_generic(&self) -> bool` predicate (types that carry type parameters: List, Map, Set, etc.)
- [ ] Implement `TypeTag::base_type(&self) -> TypeTag` (`DoubleEndedIterator` → `Iterator`, all others → `self`). Used by the query API (Section 08) for DEI aliasing: both tags resolve to the same `TypeDef`.
- [ ] Add `/// ` doc comment on every variant
- [ ] Add `//!` module doc on `tags.rs`
- [ ] Add size assertion: `const _: () = assert!(size_of::<TypeTag>() == 1);` (enforces `#[repr(u8)]`)
- [ ] Write unit test: `TypeTag::all()` returns exactly 23 variants
- [ ] Write unit test: `TypeTag::name()` returns the correct Ori-level name for every variant (e.g., `Int.name() == "int"`, `DoubleEndedIterator.name() == "DoubleEndedIterator"`)
- [ ] Write unit test: no duplicate discriminants in `TypeTag::all()`
- [ ] Write unit test: `base_type()` returns `self` for all non-DEI variants, returns `Iterator` for `DoubleEndedIterator`

---

## 01.2 MemoryStrategy Enum

### Purpose

`MemoryStrategy` declares how values of a type are managed at runtime. This is the single source of truth that replaces the scattered ARC classification logic in `ori_arc::classify::ArcClassifier::classify_primitive()` and the implicit knowledge baked into `ori_llvm::TypeInfo` storage type decisions.

### Rust Definition

```rust
/// How values of a type are managed in memory.
///
/// This determines whether the ARC pipeline inserts retain/release
/// operations, and how the LLVM backend copies/moves values.
///
/// For generic types (List, Option, etc.), the memory strategy describes
/// the container's OWN strategy, not the transitive strategy of its
/// contents. A `List` is always `Arc` even if it contains only `int`.
/// Transitive classification (does `option[int]` need RC?) is computed
/// by `ori_arc::ArcClassifier` from this base fact plus the instantiated
/// type parameters.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MemoryStrategy {
    /// Value type: bitwise copy, no reference counting.
    ///
    /// Values live in registers or on the stack. Copying is a memcpy.
    /// No destructor needed. Examples: int, float, bool, byte, char,
    /// unit, never, Duration, Size, Ordering.
    ///
    /// In LLVM: passed by value, no RC calls emitted.
    /// In ARC: `ArcClass::Scalar` (for this type alone; compound types
    /// containing only Copy children are also Scalar transitively).
    Copy,

    /// Reference-counted heap allocation.
    ///
    /// Values contain a pointer to heap-allocated memory with a reference
    /// count header. Copying increments the count (`ori_rc_inc`), dropping
    /// decrements it (`ori_rc_dec`), and when the count reaches zero the
    /// memory is freed.
    ///
    /// Examples: str, list, map, set, channel, function/closure, iterator.
    ///
    /// In LLVM: retain/release calls around copies and drops.
    /// In ARC: `ArcClass::DefiniteRef`.
    Arc,
}
```

### Design Decisions

1. **Two variants, not three**: `ori_arc::ArcClass` has a third variant `PossibleRef` for unresolved type variables. That is an *inference* artifact, not a type property. The registry describes concrete builtin types, all of which are definitively `Copy` or `Arc`. The `PossibleRef` case is handled by `ArcClassifier` at monomorphization time.

2. **On `TypeDef`, not separate**: `MemoryStrategy` is a required field on every `TypeDef`. Every builtin type MUST declare its strategy. This is not optional metadata -- it is fundamental to correctness. Making it a required field on `TypeDef` ensures new types cannot be added without deciding their memory management.

3. **Container strategy vs transitive strategy**: A `List` is `MemoryStrategy::Arc` because the list structure itself is heap-allocated. Whether `option[int]` is Scalar or DefiniteRef depends on the type parameter and is computed transitively by `ArcClassifier`. The registry declares the base fact; the classifier composes it.

4. **No `Inline` or `Stack` variant**: Some type systems distinguish stack-allocated aggregates from register scalars. Ori does not need this distinction in the registry -- both are `Copy` from the ARC perspective. The LLVM backend's distinction between register scalars and stack aggregates is a codegen concern handled by `TypeInfo`/`ValueRepr`, not a type property.

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_arc/classify/mod.rs` `classify_primitive()` | 3-arm match on 12 `Idx` constants | `type_def.memory` field lookup |
| `ori_arc/classify/mod.rs` `classify_by_tag()` | 11-arm match covering all 37 `Tag` variants | `type_def.memory` for base, `ArcClassifier` for transitive |
| `ori_llvm/type_info/mod.rs` implicit knowledge | `TypeInfo::Int` -> i64 (scalar), `TypeInfo::Str` -> struct (ref) | Explicit `MemoryStrategy` read |
| Hard-coded lists in various passes | "str is ref-counted", "int is scalar" scattered across phases | One field, one read |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_arc** | `ArcClassifier` reads `MemoryStrategy` for base classification instead of hard-coded match |
| **ori_llvm** | ARC IR emitter checks `memory` to decide retain/release emission |
| **ori_types** | Not directly used (type checker doesn't manage memory), but available for diagnostics |
| **ori_eval** | Not directly used (interpreter uses GC-less approach), but available for optimization hints |

### Checklist

- [ ] Define `MemoryStrategy` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Document each variant with Ori examples and LLVM/ARC implications
- [ ] Add size assertion: `const _: () = assert!(size_of::<MemoryStrategy>() == 1);`
- [ ] Write unit tests: each `TypeTag` has exactly one `MemoryStrategy` assignment

---

## 01.3 Ownership Enum

### Purpose

`Ownership` describes how a method parameter or receiver is passed with respect to reference counting. This is the single source of truth that replaces `receiver_borrows: bool` in `ori_ir::MethodDef`, the `borrow: true/false` syntax in `ori_llvm`'s `declare_builtins!` macro, and the `borrowing_builtins: &FxHashSet<Name>` parameter threaded through `ori_arc`'s borrow inference.

### Rust Definition

```rust
/// How a method parameter is passed with respect to reference counting.
///
/// This determines whether the ARC pipeline emits `rc_inc` at call sites
/// and whether the callee is responsible for `rc_dec` on the parameter.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ownership {
    /// Borrowed: the callee reads but does not consume.
    ///
    /// No `rc_inc` at the call site. The callee MUST NOT store, return,
    /// or pass this value to an `Owned` parameter. The caller retains
    /// ownership and handles the eventual `rc_dec`.
    ///
    /// Analogous to Lean 4's `@&` (borrow) annotation and Swift's
    /// `borrowing` parameter convention.
    ///
    /// Most builtin methods borrow their receiver: `str.len()`,
    /// `list.contains()`, `int.to_str()`, `Ordering.is_less()`.
    Borrow,

    /// Owned: the callee takes ownership.
    ///
    /// The caller emits `rc_inc` before the call. The callee is
    /// responsible for the value's lifecycle -- it may store, return,
    /// or pass it onwards. If the callee doesn't use it, it must
    /// `rc_dec` on exit.
    ///
    /// Used when the method incorporates the value into its result:
    /// `list.push(elem)` takes ownership of `elem`,
    /// `map.insert(key, value)` takes ownership of both.
    Owned,

    /// Copy: trivially copied because it's a value type.
    ///
    /// No `rc_inc` or `rc_dec` needed. The value is bitwise-copied
    /// at call sites. Semantically similar to `Borrow` (the callee
    /// reads the value), but `Copy` captures the *reason*: the type
    /// is a value type (`MemoryStrategy::Copy`), not a reference type
    /// that happens to be borrowed.
    ///
    /// Used for receivers of primitive methods: `int.abs()`,
    /// `bool.clone()`, `byte.to_int()`.
    Copy,
}
```

### Design Decisions

1. **Three variants, not two**: `Borrow` and `Owned` cover reference-counted types. `Copy` captures value types where neither `rc_inc` nor `rc_dec` is emitted. While `Borrow` and `Copy` behave identically at runtime for value types (no ARC ops), the semantic distinction matters for: (a) documentation — reading `Ownership::Copy` immediately signals "this is a value type", (b) future optimization — a phase could fast-path `Copy` receivers without consulting `MemoryStrategy`, (c) migration — `ori_ir::MethodDef.receiver_borrows: true` maps to `Borrow` for reference types and `Copy` for value types, making the mapping precise.

2. **Bool replacement**: The existing `ori_ir::MethodDef` uses `receiver_borrows: bool`. This is exactly the kind of boolean flag the coding guidelines forbid for APIs with more than trivial semantics. `Ownership` replaces it with a self-documenting enum.

3. **Applies to receiver AND parameters**: The overview's `ParamDef` includes an `ownership` field, and `MethodDef` includes a `receiver: Ownership` field. Both use the same enum. This is correct because the ARC pipeline's borrow inference treats receiver and parameter ownership uniformly.

4. **No `MutableBorrow` variant**: Ori doesn't have mutable borrows in the Rust sense. ARC memory management uses value semantics with COW (copy-on-write) for mutations. The distinction between "read-only borrow" and "mutable borrow" doesn't exist at this level. If it ever does (e.g., for future optimization), a variant can be added.

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_ir/builtin_methods/mod.rs` | `receiver_borrows: bool` field on `MethodDef` | `receiver: Ownership` on `MethodDef` |
| `ori_llvm/codegen/arc_emitter/builtins/*.rs` | `borrow: true` / `borrow: false` in `declare_builtins!` | `method_def.receiver` lookup |
| `ori_arc/borrow/builtins/mod.rs` `borrowing_builtin_names()` | Collects `BORROWING_METHOD_NAMES` into `FxHashSet<Name>` | `BUILTIN_TYPES.methods.filter(\|m\| m.receiver == Ownership::Borrow)` |
| `ori_arc/borrow/builtins/mod.rs` | `borrowing_builtins: FxHashSet<Name>` built from string list | Registry query: `find_method(tag, name).receiver` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_arc** | Borrow inference reads `method_def.receiver` to decide whether call sites need `rc_inc` |
| **ori_llvm** | ARC IR emitter reads `method_def.receiver` instead of consulting `BuiltinRegistration` |
| **ori_ir** | Migration target: `MethodDef.receiver_borrows` becomes `MethodDef.receiver: Ownership` |
| **ori_types** | Not directly used (type checker doesn't manage ownership), but available for future diagnostics |

### Checklist

- [ ] Define `Ownership` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Document each variant with concrete Ori method examples
- [ ] Add size assertion: `const _: () = assert!(size_of::<Ownership>() == 1);`
- [ ] Write unit test: `Ownership::Borrow != Ownership::Owned`, `Ownership::Owned != Ownership::Copy`, `Ownership::Copy != Ownership::Borrow`
- [ ] Write unit test: each variant is `const`-constructible

---

## 01.4 OpStrategy Enum

### Purpose

`OpStrategy` describes how a binary or unary operator is lowered to machine code for a specific type. This is the highest-value type in the registry -- it directly replaces the scattered `is_float`/`is_str` guard chains in `ori_llvm::codegen::arc_emitter::operators::emit_binary_op()` (line 21 of `operators.rs`, 363 lines total) with a single match on a strategy enum.

### Rust Definition

```rust
/// How an operator is lowered to machine code for a specific type.
///
/// Each builtin type declares an `OpStrategy` for every operator it supports.
/// The LLVM backend reads this strategy and emits the corresponding
/// instructions, eliminating the scattered `if is_float` / `if is_str`
/// guard chains that currently live in `emit_binary_op()`.
///
/// The strategy carries enough information for the backend to emit correct
/// code without further type inspection. For `RuntimeCall`, the function
/// name is included so the backend just calls it. For instruction-level
/// strategies, the backend knows the exact LLVM instruction family.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum OpStrategy {
    /// Signed integer instructions.
    ///
    /// Arithmetic: `add`, `sub`, `mul`, `sdiv`, `srem`.
    /// Comparison: `icmp slt`, `icmp sgt`, `icmp sle`, `icmp sge`.
    /// Equality: `icmp eq`, `icmp ne`.
    /// Negation: `sub 0, x`.
    ///
    /// Used by: int, Duration, Size.
    IntInstr,

    /// Floating-point instructions.
    ///
    /// Arithmetic: `fadd`, `fsub`, `fmul`, `fdiv`, `frem`.
    /// Comparison: `fcmp olt`, `fcmp ogt`, `fcmp ole`, `fcmp oge`.
    /// Equality: `fcmp oeq`, `fcmp one`.
    /// Negation: `fneg`.
    ///
    /// Used by: float.
    FloatInstr,

    /// Unsigned integer comparison.
    ///
    /// Comparison: `icmp ult`, `icmp ugt`, `icmp ule`, `icmp uge`.
    /// Equality: `icmp eq`, `icmp ne` (same as signed for equality).
    /// No arithmetic operators — byte and char don't support `+`, `-`, etc.
    ///
    /// Used by: byte, char, bool (for ordering — bool's `false < true`
    /// uses unsigned comparison since `false=0, true=1`).
    UnsignedCmp,

    /// Boolean logic instructions.
    ///
    /// And: `and`. Or: `or`. Xor: `xor`.
    /// Equality: `icmp eq`, `icmp ne`.
    /// No arithmetic, no ordering (ordering uses `UnsignedCmp`).
    ///
    /// Used by: bool (for logical operators `&&`, `||`).
    BoolLogic,

    /// Delegate to an `ori_rt` runtime function.
    ///
    /// The function name is the symbol in the runtime library that
    /// implements this operation. The LLVM backend emits a `call`
    /// instruction to this function.
    ///
    /// The `returns_bool` flag indicates whether the runtime function
    /// returns `i1` (for equality/comparison predicates like `ori_str_eq`)
    /// vs the type's own representation (for operations like `ori_str_concat`
    /// which returns a new str).
    ///
    /// Used by: str (all operators delegate to runtime).
    RuntimeCall {
        /// The runtime function symbol name (e.g., `"ori_str_concat"`).
        fn_name: &'static str,
        /// True if the function returns `i1` (bool), false if it returns
        /// the same type as the operands.
        returns_bool: bool,
    },

    /// This operator is not supported for this type.
    ///
    /// Attempting to use this operator is a type error caught by the
    /// type checker. The LLVM backend should never encounter this
    /// variant -- if it does, it's a compiler bug (the type checker
    /// failed to reject an invalid operation).
    ///
    /// Used by: most operators on most types. `str` doesn't support
    /// `sub`, `mul`, `div`, etc. `bool` doesn't support arithmetic.
    Unsupported,
}
```

### Design Decisions

1. **`RuntimeCall` carries the function name**: The backend should not need a secondary lookup to find the runtime function. `RuntimeCall { fn_name: "ori_str_concat", returns_bool: false }` gives the backend everything it needs in one read. The `&'static str` is const-constructible.

2. **`returns_bool` flag on `RuntimeCall`**: Runtime functions like `ori_str_eq` return `i1` (bool), while `ori_str_concat` returns a `str`. The backend needs to know the return type to correctly handle the result. Rather than encoding the full return type (which would require `TypeTag` and create a circular dependency within the struct), a simple boolean captures the two cases that exist in practice: "returns a bool" or "returns the same type as the operands".

3. **`BoolLogic` vs reusing `IntInstr`**: Booleans are `i1` in LLVM, not `i64`. While LLVM's `and`/`or`/`xor` work on both, the codegen context is different (no arithmetic, different comparison semantics). Keeping a separate strategy makes the backend's match arms cleaner and more explicit.

4. **No bitwise operation strategies here**: Bitwise operators (`&`, `|`, `^`, `<<`, `>>`) currently exist in `BinaryOp` but are only valid on `int`. They use the same LLVM instructions as `IntInstr` (the LLVM `and`, `or`, `xor`, `shl`, `ashr` instructions). Rather than adding separate strategies for them, they fall under `IntInstr` -- the `OpDefs` struct (01.7) will have separate fields for bitwise ops that can independently be `IntInstr` or `Unsupported`.

5. **No `Ordering`-specific strategy**: The `Ordering` type's comparison operations use unsigned comparison (`icmp eq` for equality, special logic for ordering predicates on the 3-value enum). This maps to `UnsignedCmp` for the comparison-related `OpDefs` fields. Ordering-specific method behavior (`.is_less()`, `.reverse()`) lives in `MethodDef`, not in `OpStrategy`.

6. **Why not carry LLVM opcode directly?**: The registry must have zero LLVM dependency. `OpStrategy` is a semantic description ("use integer instructions") that the LLVM backend interprets. Different backends (WASM, interpreter) would interpret the same strategy differently.

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_llvm/arc_emitter/operators.rs` `emit_binary_op()` | `if is_float => self.builder.fadd(...)` | `match type_def.operators.add { FloatInstr => ... }` |
| `ori_llvm/arc_emitter/operators.rs` `emit_binary_op()` | `if is_str => self.emit_str_runtime_call("ori_str_concat", ...)` | `match type_def.operators.add { RuntimeCall { fn_name, .. } => ... }` |
| `ori_llvm/builtins/traits.rs` `emit_equals()` | `TypeInfo::Float => fcmp_oeq, TypeInfo::Int => icmp_eq` | `match type_def.operators.eq { FloatInstr \| IntInstr \| ... }` |
| `ori_llvm/builtins/traits.rs` `emit_compare()` | `TypeInfo::Bool \| TypeInfo::Char \| TypeInfo::Byte => unsigned` | `match type_def.operators.lt { UnsignedCmp => ... }` |
| `ori_llvm/builtins/traits.rs` `emit_str_trait_method()` | `"equals" => emit_str_runtime_call("ori_str_eq", ...)` | `match STR.operators.eq { RuntimeCall { fn_name: "ori_str_eq", .. } => ... }` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_llvm** | Primary consumer: `emit_binary_op()` dispatches on `OpStrategy` instead of type guards |
| **ori_types** | Validates operator applicability: `type_def.operators.add != Unsupported` means `+` is valid |
| **ori_eval** | Could use for dispatch (currently uses direct `Value` matching, which is fine) |
| **ori_arc** | Not directly used (ARC doesn't care about operators) |

### Checklist

- [ ] Define `OpStrategy` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Include `RuntimeCall { fn_name, returns_bool }` variant with `&'static str`
- [ ] Document each variant with LLVM instruction examples
- [ ] Add size assertion: `const _: () = assert!(size_of::<OpStrategy>() == 24);` (`RuntimeCall` has `&'static str` (16) + `bool` (1) + padding to 24)
- [ ] Write unit tests: equality, hash, debug output for each variant
- [ ] Verify `RuntimeCall` is const-constructible (it is: `&'static str` and `bool` are const)

---

## 01.5 ParamDef Struct

### Purpose

`ParamDef` describes a single parameter of a builtin method (excluding the receiver). It carries the parameter's name, type, and ownership. This replaces the existing `ori_ir::ParamSpec` enum, which is too coarse (only `SelfType`, `Int`, `Str`, `Bool`, `Any`, `Closure` -- no ownership, no full type coverage).

### Rust Definition

```rust
/// A type reference in the registry, used for method parameter and return types.
///
/// Most parameters have concrete types (TypeTag), but some are generic
/// (e.g., the element type of a list method, or a closure parameter).
/// `ReturnTag` handles both cases. Used for both parameter types
/// (`ParamDef.ty`) and return types (`MethodDef.returns`).
///
/// ## Naming Convention
///
/// **Canonical name is `ReturnTag`** — not `ReturnSpec`, `ReturnType`,
/// or `ParamType`. All sections MUST use `ReturnTag`. The name applies
/// to both parameter and return positions because the same abstract
/// type references work in both contexts.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ReturnTag {
    // === Concrete types ===

    /// A concrete builtin type.
    Concrete(TypeTag),

    /// The receiver's own type (for methods that return Self).
    ///
    /// Example: `list.clone()` returns the same list type,
    /// `int.clone()` returns int.
    SelfType,

    /// Unit type (for void-returning methods like `for_each`).
    Unit,

    // === Direct type-parameter projections ===

    /// The element type of the receiver (for container methods).
    ///
    /// Example: `option.unwrap()` returns `T`,
    /// `iterator.next()` yields the element type.
    /// For Map: returns the primary element (K). Use `ValueType` for V.
    ElementType,

    /// The key type of a Map receiver (K in Map<K, V>).
    KeyType,

    /// The value type of a Map receiver (V in Map<K, V>).
    ValueType,

    /// The ok-type of Result<T, E> -> T.
    OkType,

    /// The err-type of Result<T, E> -> E.
    ErrType,

    // === Parameterized wrappers (fixed inner type) ===
    //
    // Used by primitive type methods where the return wraps a known
    // concrete type (not relative to receiver type parameters).

    /// `[T]` for a fixed `T`.
    /// Example: `str.split(sep:) -> [str]`, `str.chars() -> [char]`.
    List(TypeTag),

    /// `Option<T>` for a fixed `T`.
    /// Example: `str.to_int() -> Option<int>`.
    Option(TypeTag),

    /// `DoubleEndedIterator<T>` for a fixed `T`.
    /// Example: `str.iter() -> DoubleEndedIterator<char>`.
    DoubleEndedIterator(TypeTag),

    // === Parameterized wrappers (projection-based) ===
    //
    // Used by generic container methods where the return wraps a
    // type derived from the receiver's type parameters.

    /// `Option<P>` where P is a type projection from the receiver.
    /// Example: `list.first() -> Option<T>` = `OptionOf(Element)`.
    /// Example: `map.get(k) -> Option<V>` = `OptionOf(Value)`.
    /// Example: `result.ok() -> Option<T>` = `OptionOf(Ok)`.
    OptionOf(TypeProjection),

    /// `[P]` where P is a type projection.
    /// Example: `set.to_list() -> [T]` = `ListOf(Element)`.
    /// Example: `map.keys() -> [K]` = `ListOf(Key)`.
    ListOf(TypeProjection),

    /// `Iterator<P>` where P is a type projection.
    /// Example: `set.iter() -> Iterator<T>` = `IteratorOf(Element)`.
    IteratorOf(TypeProjection),

    /// `DoubleEndedIterator<P>` where P is a type projection.
    /// Example: `list.iter() -> DEI<T>` = `DoubleEndedIteratorOf(Element)`.
    DoubleEndedIteratorOf(TypeProjection),

    // === Composite returns ===

    /// `[(K, V)]` from a Map. Example: `map.entries()`.
    ListKeyValue,

    /// `[(int, T)]` where T is the element type.
    /// Example: `list.enumerate() -> [(int, T)]`.
    ListOfTupleIntElement,

    /// `Iterator<(K, V)>` from a Map. Example: `map.iter()`.
    MapIterator,

    /// `Iterator<(int, T)>` where T is the element type.
    /// Example: `iterator.enumerate() -> Iterator<(int, Item)>`.
    IteratorOfTupleIntElement,

    // === Tuple returns ===

    /// `(Option<T>, Self)` where T is the element type.
    /// Example: `iterator.next() -> (Option<Item>, Iterator<Item>)`.
    /// Example: `iterator.next_back() -> (Option<Item>, DoubleEndedIterator<Item>)`.
    NextResult,

    /// `Result<T, E>` where T is a projection and E is fresh.
    /// Example: `option.ok_or(err) -> Result<T, E>` (E from parameter).
    ResultOfProjectionFresh(TypeProjection),

    // === Higher-order ===

    /// A fresh type variable — return type depends on closure argument.
    /// The type checker creates a fresh variable and unifies it with
    /// the closure's return type via `unify_higher_order_constraints`.
    /// Example: `list.map(f)`, `iterator.fold(init, f)`.
    Fresh,
}

/// Which type parameter to project from the receiver.
///
/// Used by `ReturnTag::OptionOf`, `ListOf`, `IteratorOf`,
/// `DoubleEndedIteratorOf` to express generic return types.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeProjection {
    /// The single element type (T in List<T>, Option<T>, Iterator<T>, etc.)
    Element,
    /// The key type (K in Map<K, V>).
    Key,
    /// The value type (V in Map<K, V>).
    Value,
    /// The ok-type (T in Result<T, E>).
    Ok,
    /// The err-type (E in Result<T, E>).
    Err,
    /// A fixed concrete type (not a projection from receiver params).
    Fixed(TypeTag),
}

/// Definition of a method parameter (excluding the receiver).
///
/// Parameters are `const`-constructible so they can be embedded in
/// static method definitions.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParamDef {
    /// The parameter name as it appears in documentation and error messages.
    pub name: &'static str,

    /// The parameter's type.
    pub ty: ReturnTag,

    /// How the parameter is passed with respect to reference counting.
    pub ownership: Ownership,
}
```

### Design Decisions

1. **`ReturnTag` instead of `TypeTag` for parameter types**: Method parameters can reference generic type relationships ("the element type of the receiver"), not just concrete types. `ReturnTag` captures this. The name `ReturnTag` is used for both parameters and returns because the same abstract type references apply to both contexts.

2. **Separate from `TypeTag`**: `TypeTag` identifies builtin types. `ReturnTag` describes type *positions* in a method signature. These are fundamentally different -- `TypeTag::List` means "the list type itself", while `ReturnTag::ListOf(TypeProjection::Element)` means "a list whose element type matches the receiver's element type". Conflating them would either bloat `TypeTag` with positional variants or lose expressiveness. **CRITICAL: No `SelfType`, `Fresh`, or `Void` on `TypeTag`.** These are signature-level concepts that belong exclusively on `ReturnTag`. Sections that reference `TypeTag::SelfType` must use `ReturnTag::SelfType` instead. A convenience `From<TypeTag> for ReturnTag` impl wraps concrete types automatically: `TypeTag::Int.into()` produces `ReturnTag::Concrete(TypeTag::Int)`.

    ```rust
    impl From<TypeTag> for ReturnTag {
        fn from(tag: TypeTag) -> Self {
            ReturnTag::Concrete(tag)
        }
    }
    ```

3. **`Fresh` variant**: Higher-order methods like `list.map(f)` have return types that depend on the closure argument. The type checker handles this by creating fresh type variables. The registry can't specify the exact return type, so `Fresh` signals "the type checker must infer this via unification". This replaces the pattern `engine.pool_mut().fresh_var()` scattered across `resolve_*_method()` functions.

4. **No `Closure` type in `ParamDef`**: The current `ParamSpec::Closure` in `ori_ir` indicates "this parameter is a closure/function". Rather than a special variant, closure parameters will use `ReturnTag::Fresh` for the parameter type (since the exact closure signature varies). The *existence* of a closure parameter is visible from the method's higher-order nature, and the type checker's existing unification logic (`unify_higher_order_constraints` in calls.rs) handles inferring closure return types. This inference logic stays in ori_types — it's behavioral, not declarative data.

5. **`ownership` defaults to `Borrow`**: In practice, most method parameters are borrowed (the method reads them but doesn't consume them). The `ownership` field is explicit on every `ParamDef` -- no implicit defaults. This makes the registry completely self-describing.

6. **`KeyType`, `ValueType`, `ListKeyValue` for Map methods**: Map has two type parameters (K, V), and its methods return types derived from either or both. `map.keys()` returns `[K]` (a list of the key type), `map.values()` returns `[V]`, and `map.entries()` returns `[(K, V)]`. These three variants are necessary because `ElementType` alone is ambiguous for two-parameter types. Named variants are preferred over a generic `Param(u8)` because they are self-documenting, exhaustive, and const-constructible without needing a separate type parameter index scheme.

7. **`ReturnTag::Unit` convenience**: `Unit` is semantically equivalent to `Concrete(TypeTag::Unit)` but provided as a separate variant for readability. Methods like `for_each`, `channel.send()`, and `channel.close()` return void — writing `returns: ReturnTag::Unit` is clearer than `returns: ReturnTag::Concrete(TypeTag::Unit)`. The `From<TypeTag>` impl already covers `TypeTag::Unit` → `Concrete(Unit)`, so both forms work. Sections 03-07 SHOULD prefer `ReturnTag::Unit` for void-returning methods.

8. **`NextResult` for iterator protocol**: `iterator.next()` returns `(Option<T>, Self)` — a tuple of two computed types. This cannot be expressed as any single `ReturnTag` wrapper. `NextResult` is a dedicated variant that the type checker interprets as "construct a tuple of (Option<element_type>, receiver_type)". Both `next()` and `next_back()` use this pattern.

9. **Intrinsic vs derived boundary**: `ReturnTag` describes the *shape* of a return type (structural template), not the *resolved* type. The type checker interprets `OptionOf(TypeProjection::Element)` as `pool.option(elem)` — constructing a real Pool `Idx` from the template. Context-dependent facts (generic substitution, closure return type unification, trait resolution) remain in the type checker / Salsa layer. `ReturnTag::Fresh` is the explicit boundary marker: "the registry cannot specify this; the type checker must infer it."

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_ir/builtin_methods/mod.rs` | `ParamSpec` enum (6 variants) | `ParamDef` struct with `ReturnTag` |
| `ori_ir/builtin_methods/mod.rs` | `ReturnSpec` enum (7 variants) | `ReturnTag` enum (22 variants, superset) |
| `ori_types/infer/expr/methods/mod.rs` | Hard-coded return types in match arms | `method_def.returns: ReturnTag` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_types** | Reads `ParamDef.ty` to validate argument types at call sites |
| **ori_ir** | Migration: `ParamSpec` -> `ParamDef` (or re-export from registry) |
| **ori_eval** | Parameter count validation for dispatch |
| **ori_llvm** | Parameter ownership determines ARC behavior at call sites |

### Checklist

- [ ] Define `ReturnTag` enum in `ori_registry/src/tags.rs`
- [ ] Define `TypeProjection` enum in `ori_registry/src/tags.rs`
- [ ] Define `ParamDef` struct in `ori_registry/src/tags.rs`
- [ ] Add derives: `Copy, Clone, Debug, PartialEq, Eq, Hash` on all three
- [ ] Implement `From<TypeTag> for ReturnTag` convenience conversion
- [ ] Verify const-constructibility: `&'static str` + `ReturnTag` + `Ownership` are all const
- [ ] Add `/// ` doc comment on every variant and field
- [ ] Add size assertions: `const _: () = assert!(size_of::<ReturnTag>() == 2);` (discriminant u8 + largest payload u8), `const _: () = assert!(size_of::<ParamDef>() == 24);` (`&'static str` 16 + `ReturnTag` 2 + `Ownership` 1 + padding)
- [ ] Write unit tests: construct a `ParamDef` in a `const` context, verify field access
- [ ] Write unit tests: `ReturnTag::Concrete(TypeTag::Int)` equals `TypeTag::Int.into()`
- [ ] Write unit tests: every `TypeProjection` variant constructs in const context

---

## 01.6 MethodDef Struct

### Purpose

`MethodDef` is the complete specification of a single builtin method. It is the core unit of the registry -- every method on every builtin type is one `MethodDef`. It replaces the scattered method definitions in `ori_ir::builtin_methods::MethodDef`, the match arms in `ori_types::infer::expr::methods::resolve_*_method()`, and the `declare_builtins!` entries in `ori_llvm`.

### Rust Definition

```rust
/// Complete specification of a single builtin method.
///
/// Every builtin method across all phases is described by exactly one
/// `MethodDef`. The type checker reads `returns` to infer call expressions,
/// the ARC pass reads `receiver` ownership, the LLVM backend reads both,
/// and the evaluator validates dispatch coverage.
///
/// All fields are `const`-constructible. A `MethodDef` is a compile-time
/// constant embedded in the binary's `.rodata` segment.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct MethodDef {
    /// The method name (e.g., `"len"`, `"to_str"`, `"equals"`).
    pub name: &'static str,

    /// How the receiver (self) is passed.
    ///
    /// Most methods borrow: `str.len()`, `list.contains()`, `int.abs()`.
    /// Consuming methods own: `iterator.collect()`, `option.unwrap()`.
    pub receiver: Ownership,

    /// Parameters excluding the receiver.
    ///
    /// Empty slice `&[]` for zero-argument methods (e.g., `int.abs()`).
    /// The receiver is implicit and described by the owning `TypeDef`'s
    /// `tag` field plus this method's `receiver` field.
    pub params: &'static [ParamDef],

    /// The return type.
    pub returns: ReturnTag,

    /// The trait this method belongs to, if any.
    ///
    /// `Some("Eq")` for `.equals()`, `Some("Comparable")` for `.compare()`,
    /// `Some("Hashable")` for `.hash()`, etc. `None` for inherent methods
    /// like `int.abs()`, `str.len()`.
    ///
    /// This field enables the type checker to distinguish trait methods
    /// from inherent methods during resolution, and allows the registry
    /// to be queried by trait name.
    pub trait_name: Option<&'static str>,

    /// Whether this method is free of side effects and cannot panic.
    ///
    /// `true` = pure (no side effects, no panics) — e.g., `str.length()`,
    /// `int.abs()`, `list.len()`. The optimizer may reorder, eliminate,
    /// or memoize calls to pure methods. Pure methods need no `uses`
    /// capability annotation.
    ///
    /// `false` = may have side effects, may panic — e.g., `list.push()`,
    /// `option.unwrap()`, `channel.send()`.
    ///
    /// Prior art: Swift's `"n"` readnone attribute on Builtins.def flows
    /// from the registry to SIL's `getMemoryBehavior()` for optimization.
    /// Zig's `eval_to_error` flag on BuiltinFn enables pre-Sema validation.
    pub pure: bool,

    /// Whether ALL backends (eval + llvm) must implement this method.
    ///
    /// `true` = both eval and llvm must have a handler. Section 14
    /// enforcement tests verify this. Most user-facing methods are
    /// backend_required.
    ///
    /// `false` = may be eval-only, llvm-only, or internal. Examples:
    /// - `__iter_next` (llvm-only ARC lowering protocol)
    /// - `__collect_set` (eval-only type-directed rewrite)
    /// - Methods not yet implemented in LLVM (tracked, not enforced)
    ///
    /// Prior art: Rust's `must_be_overridden` flag on `IntrinsicDef`
    /// forces every codegen backend to implement the intrinsic.
    pub backend_required: bool,

    /// Whether this is an instance method or associated function.
    ///
    /// `Instance` (default) = called as `value.method(args)`.
    /// `Associated` = called as `Type.method(args)` (e.g.,
    /// `Duration.from_seconds(ns:)`, `Size.from_bytes(b:)`).
    ///
    /// For associated functions, `receiver` is irrelevant (no self).
    pub kind: MethodKind,

    /// Whether this method is only available on `DoubleEndedIterator`.
    ///
    /// `true` = DEI-only (e.g., `next_back`, `rev`, `last`, `rfind`, `rfold`).
    /// `false` = available on both `Iterator` and `DoubleEndedIterator`.
    ///
    /// Only meaningful for methods on the Iterator/DEI TypeDef. For all other
    /// types, this is always `false`. The query API uses this to filter:
    /// `methods_for(Iterator)` excludes `dei_only: true` methods;
    /// `methods_for(DoubleEndedIterator)` includes all methods.
    ///
    /// See Section 07 Decision 1: single TypeDef with `dei_only` flag.
    pub dei_only: bool,

    /// How this method propagates DoubleEndedIterator capability.
    ///
    /// Only meaningful for iterator adapter methods. Consumers and
    /// non-iterator methods use `NotApplicable`.
    ///
    /// See Section 07 Decision 2: explicit field, not derived from return type.
    pub dei_propagation: DeiPropagation,
}
```

### Design Decisions

1. **No `type_flow` field — intentionally excluded**: Higher-order method type inference (how closure arguments constrain return types) is behavioral logic that belongs in the type checker, not declarative data in the registry. `returns: ReturnTag::Fresh` signals "the type checker must infer this via unification." The specific unification logic (`unify_higher_order_constraints` in `calls/method_call.rs`) stays in ori_types where it can reason about type variables, closure signatures, and unification constraints — none of which are expressible as pure const data.

2. **`trait_name: Option<&'static str>` not `Option<TraitTag>`**: Creating a `TraitTag` enum for all traits would couple the registry to the trait system's evolution. Using a plain string keeps it simple and matches how traits are identified throughout the codebase. The string is `&'static str` so it's const-constructible.

3. **`pure: bool` for side-effect annotation**: Inspired by Swift's `"n"` (readnone) attribute on `Builtins.def` and Zig's `eval_to_error` flag on `BuiltinFn`. A single boolean captures the most useful distinction for the optimizer and effect system. More granular effect levels (e.g., "may panic but no IO", "reads memory but doesn't write") can be added later if needed by replacing `bool` with an enum — the consuming phases already handle `pure` as a field, so widening the type is non-breaking. The binary `pure/impure` covers ~90% of the optimization benefit.

4. **`backend_required: bool` for cross-backend enforcement**: Inspired by Rust's `must_be_overridden` flag on `IntrinsicDef`. Without this, a method can exist in the registry and be implemented in eval but not llvm (or vice versa), with the gap only caught by test-time enforcement in Section 14. The flag makes the expectation explicit: `backend_required: true` means Section 14 tests WILL fail if any backend is missing a handler. `backend_required: false` means the method is intentionally backend-specific (e.g., `__iter_next` is llvm-only).

5. **No mutation strategy on MethodDef — intentionally excluded**: Whether a method can mutate its receiver in-place (vs. clone-then-mutate) is a per-call-site decision, not a per-method property. `list.push(x)` may be in-place at one call site (sole reference) and clone at another (shared reference). Roc validates this: `LowLevel::ListReplaceUnsafe` carries a single enum variant, and each call site gets a separate `UpdateMode` from alias analysis. The registry declares `receiver: Ownership::Borrow` (the semantic contract) — the ARC pass decides the optimization.

6. **`params` is `&'static [ParamDef]`**: Method parameters are a fixed set known at compile time. Using a static slice avoids heap allocation and is const-constructible. The owning `static` arrays live next to the `MethodDef` const declarations (Sections 03-07).

7. **`MethodDef` derives `Copy`**: The struct is exactly 56 bytes (verified: two fat pointers 32 + `Option<&'static str>` 16 + small enum/bool fields 8 with padding). All fields are `Copy`. At 56 bytes it's within reasonable bounds (less than a cache line). Since the data lives in static storage and is always accessed by reference (`&MethodDef`), adding `Copy` avoids `.clone()` noise without risk of accidental large copies in hot paths.

8. **`dei_only` and `dei_propagation` on every MethodDef**: These fields are only meaningful for iterator methods (~24 entries) but live on all MethodDefs (~200+ entries). The cost is 2 bytes per entry. Alternatives considered: (a) `Option<IteratorMetadata>` wrapper — adds 1 byte for `Option` discriminant + 16 bytes for the pointer/padding, actually MORE expensive; (b) separate `IteratorMethodDef` — breaks uniform `&[MethodDef]` slicing for all types. The 2-byte overhead with clear defaults (`false` / `NotApplicable`) is the simplest design. Section 07 Decision 1 chose the single-TypeDef model specifically because `dei_only: bool` is cheap and directly queryable.

9. **Matches but improves on `ori_ir::MethodDef`**: The existing `ori_ir::MethodDef` has `receiver: BuiltinType`, `name`, `params: &'static [ParamSpec]`, `returns: ReturnSpec`, `trait_name: Option<&'static str>`, `receiver_borrows: bool`. The registry's `MethodDef` drops `receiver: BuiltinType` (the owning type is the `TypeDef` that contains this method), replaces `receiver_borrows: bool` with `receiver: Ownership`, and uses the richer `ReturnTag` instead of `ReturnSpec`.

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_ir/builtin_methods/mod.rs` | `MethodDef { receiver, name, params, returns, trait_name, receiver_borrows }` | `MethodDef { name, receiver, params, returns, trait_name, pure, backend_required, kind, dei_only, dei_propagation }` |
| `ori_types/infer/expr/methods/resolve_by_type.rs` | `"to_str" => Some(Idx::STR)` (per method, per type) | `method_def.returns` on the queried `MethodDef` |
| `ori_types/infer/expr/methods/mod.rs` | `TYPECK_BUILTIN_METHODS` (390 entries: `(type, method)` pairs) | `TypeDef.methods` iteration |
| `ori_eval/methods/helpers/mod.rs` | `EVAL_BUILTIN_METHODS` | `TypeDef.methods` iteration |
| `ori_llvm/builtins/*.rs` | `declare_builtins!` entries with `borrow: true/false` | `method_def.receiver: Ownership` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_types** | `find_method(tag, name).returns` replaces `resolve_*_method()` match arms |
| **ori_eval** | Validates dispatch coverage: every `MethodDef` must have a handler |
| **ori_arc** | `method_def.receiver == Ownership::Borrow` replaces `borrowing_builtins` set |
| **ori_llvm** | `method_def.receiver` for ARC decisions; `method_def.params` for codegen |
| **ori_ir** | Migration: `MethodDef` consolidation (Section 13) |

### Checklist

- [ ] Define `MethodDef` struct in `ori_registry/src/method.rs`
- [ ] Add derives: `Copy, Clone, Debug, PartialEq, Eq, Hash`
- [ ] Verify const-constructibility: all fields are `Copy`/`&'static`
- [ ] Add `/// ` doc comment on every field
- [ ] Add `//!` module doc on `method.rs`
- [ ] Add size assertion: `const _: () = assert!(size_of::<MethodDef>() <= 64);` (two fat pointers 32 + small fields + padding; actual ~56 bytes)
- [ ] Write unit tests: construct in `const` context, verify field access
- [ ] Test: `MethodDef` in a `&'static [MethodDef]` slice compiles as const

---

## 01.7 OpDefs Struct

### Purpose

`OpDefs` holds the `OpStrategy` for every operator that can be applied to a type. It is a fixed-field struct (not a map) so that Rust's exhaustiveness checking catches missing operator entries when a new operator is added. Every `TypeDef` contains exactly one `OpDefs`.

### Rust Definition

```rust
/// Operator strategies for a single type.
///
/// Every field corresponds to one operator or operator group. Each field
/// is an `OpStrategy` declaring how that operation is lowered.
/// `Unsupported` means the operator is invalid for this type (caught
/// by the type checker, never reaches codegen).
///
/// Adding a new field here is a compile error in every `TypeDef`
/// definition (Sections 03-07) and every backend match arm that reads
/// `OpDefs`, enforcing full coverage.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct OpDefs {
    // Arithmetic operators
    /// `+` (binary addition, or string concatenation for str).
    pub add: OpStrategy,
    /// `-` (binary subtraction).
    pub sub: OpStrategy,
    /// `*` (binary multiplication).
    pub mul: OpStrategy,
    /// `/` (binary division).
    pub div: OpStrategy,
    /// `%` (remainder/modulo).
    pub rem: OpStrategy,

    /// `div` (integer division via `sdiv`).
    pub floor_div: OpStrategy,

    // Comparison operators (expanded — one field per operator)
    /// `==` (equality).
    pub eq: OpStrategy,
    /// `!=` (inequality).
    pub neq: OpStrategy,
    /// `<` (less than).
    pub lt: OpStrategy,
    /// `>` (greater than).
    pub gt: OpStrategy,
    /// `<=` (less than or equal).
    pub lt_eq: OpStrategy,
    /// `>=` (greater than or equal).
    pub gt_eq: OpStrategy,

    // Unary operators
    /// `-x` (unary negation).
    pub neg: OpStrategy,
    /// `!x` (logical NOT).
    pub not: OpStrategy,

    // Bitwise operators
    /// `&` (bitwise AND).
    pub bit_and: OpStrategy,
    /// `|` (bitwise OR).
    pub bit_or: OpStrategy,
    /// `^` (bitwise XOR).
    pub bit_xor: OpStrategy,
    /// `~` (bitwise NOT / complement).
    pub bit_not: OpStrategy,
    /// `<<` (left shift).
    pub shl: OpStrategy,
    /// `>>` (right shift, arithmetic for signed types).
    pub shr: OpStrategy,
}
```

### Design Decisions

1. **Include bitwise operators**: After examining `ori_ir::BinaryOp`, bitwise ops (`BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`) are real operators in the language. They are currently only valid for `int` (using `IntInstr` strategy). Including them in `OpDefs` means:
   - The type checker can validate bitwise operator applicability from the registry
   - If bitwise ops are ever extended to `byte` (common in other languages), the registry just changes the `byte` type's `OpDefs`
   - No separate mechanism needed to track "which types support bitwise ops"

2. **Expanded comparison fields (`eq`/`neq`/`lt`/`gt`/`lt_eq`/`gt_eq`)**: Each comparison operator gets its own field rather than compact `eq` + `cmp` groupings. This eliminates ambiguity for types where equality and ordering use different strategies (e.g., `str` uses `RuntimeCall("ori_str_eq")` for `eq` but `RuntimeCall("ori_str_compare")` for `lt`). The backend reads the exact field for the exact operator — no inversion or predicate selection logic needed at the `OpDefs` level.

3. **`floor_div` separate from `div`**: `BinaryOp::FloorDiv` (`div` keyword in Ori) uses `sdiv` for integers but has different semantics than `/` for floats (floor-truncated vs true division). Keeping a separate field lets float define `div: FloatInstr` and `floor_div: Unsupported` (or vice versa) independently.

4. **`bit_not` separate from `neg`**: Unary `~` (bitwise complement) uses `xor -1` while unary `-` (negation) uses `sub 0, x`. Different LLVM instructions, different semantics. Separate fields.

5. **No `and`/`or` logical operator fields**: Logical `&&` and `||` are short-circuiting — the control flow is lowered before the final merge. However, `BinaryOp::And` and `BinaryOp::Or` DO still reach `emit_binary_op()` in the primitive dispatch, where they emit LLVM `and` / `or` instructions (same as `BoolLogic`). Since these only apply to `bool`, they are subsumed by `bool`'s `BoolLogic` strategy on the existing `bit_and`/`bit_or` fields (LLVM `and` = `and` regardless of type). No separate `logical_and`/`logical_or` fields are needed — the backend maps `BinaryOp::And` to `bit_and` strategy, `BinaryOp::Or` to `bit_or` strategy, relying on the fact that `bool` defines both as `BoolLogic`.

6. **No `Coalesce` or `Range` operators**: `??` (coalesce) and `..` (range) are desugared before reaching operator dispatch. They don't need registry entries.

7. **No `pow`/`**` operator**: The `**` operator (exponentiation) exists in Ori syntax at precedence 2 with a `Pow` trait, but it is desugared to a `Pow.power()` trait method call BEFORE reaching `BinaryOp` IR. `BinaryOp` has no `Pow` variant. Since `OpDefs` mirrors `BinaryOp` + `UnaryOp`, there is no `pow` field. Exponentiation is handled entirely through trait dispatch.

8. **No `matmul`/`@` operator field**: `BinaryOp::MatMul` exists in `ori_ir`, but in `emit_binary_op()` it is handled as a desugared/trait-dispatched op (falls through to `emit_binary_op_via_trait()` with `trait_method_name()` returning `"mat_mul"`). The ARC IR emitter logs a warning if `MatMul` reaches the primitive dispatch. Since it is always trait-dispatched, no `OpStrategy` field is needed — `MatMul` is an operator trait, not a builtin type operation.

9. **No `as`/`as?` conversion operators**: Type conversions (`42 as float`, `"42" as? int`) are `Expr::Cast` nodes in the AST, not binary or unary operators. They are handled by the type checker (`infer_cast()`) and codegen (`emit_cast()`) as special expression forms, not by operator dispatch. The `As`/`TryAs` traits exist for user-defined types but builtins have hard-coded conversion logic. These do not belong in `OpDefs`.

10. **`not` is separate from `bit_not`**: Logical NOT (`!x`, `UnaryOp::Not`) uses `builder.not()` (LLVM `xor x, true` for `i1`). Bitwise NOT (`~x`, `UnaryOp::BitNot`) uses `xor x, -1` for `i64`. Different semantics, different types: `not` applies to `bool`, `bit_not` applies to `int`. Separate fields in `OpDefs`.

### Convenience Constructor

```rust
impl OpDefs {
    /// All operators unsupported (for types with no operator support).
    ///
    /// Useful as a starting point: construct `UNSUPPORTED` then override
    /// specific fields with struct update syntax.
    pub const UNSUPPORTED: OpDefs = OpDefs {
        add: OpStrategy::Unsupported,
        sub: OpStrategy::Unsupported,
        mul: OpStrategy::Unsupported,
        div: OpStrategy::Unsupported,
        rem: OpStrategy::Unsupported,
        floor_div: OpStrategy::Unsupported,
        eq: OpStrategy::Unsupported,
        neq: OpStrategy::Unsupported,
        lt: OpStrategy::Unsupported,
        gt: OpStrategy::Unsupported,
        lt_eq: OpStrategy::Unsupported,
        gt_eq: OpStrategy::Unsupported,
        neg: OpStrategy::Unsupported,
        not: OpStrategy::Unsupported,
        bit_and: OpStrategy::Unsupported,
        bit_or: OpStrategy::Unsupported,
        bit_xor: OpStrategy::Unsupported,
        bit_not: OpStrategy::Unsupported,
        shl: OpStrategy::Unsupported,
        shr: OpStrategy::Unsupported,
    };
}
```

This allows type definitions to write:

```rust
pub const INT_OPS: OpDefs = OpDefs {
    add: OpStrategy::IntInstr,
    sub: OpStrategy::IntInstr,
    mul: OpStrategy::IntInstr,
    div: OpStrategy::IntInstr,
    rem: OpStrategy::IntInstr,
    floor_div: OpStrategy::IntInstr,
    eq: OpStrategy::IntInstr,
    neq: OpStrategy::IntInstr,
    lt: OpStrategy::IntInstr,
    gt: OpStrategy::IntInstr,
    lt_eq: OpStrategy::IntInstr,
    gt_eq: OpStrategy::IntInstr,
    neg: OpStrategy::IntInstr,
    not: OpStrategy::Unsupported, // int does not support `!`
    bit_and: OpStrategy::IntInstr,
    bit_or: OpStrategy::IntInstr,
    bit_xor: OpStrategy::IntInstr,
    bit_not: OpStrategy::IntInstr,
    shl: OpStrategy::IntInstr,
    shr: OpStrategy::IntInstr,
};
```

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_llvm/arc_emitter/operators.rs` `emit_binary_op()` | 40+ lines of `match op { BinaryOp::Add if is_float => ..., if is_str => ... }` | `match type_def.operators.add { IntInstr \| FloatInstr \| RuntimeCall { .. } \| ... }` |
| `ori_llvm/builtins/traits.rs` `emit_equals()` | `match type_info { TypeInfo::Float => fcmp_oeq, ... }` | `match type_def.operators.eq { ... }` |
| `ori_llvm/builtins/traits.rs` `emit_compare()` | Separate signed/unsigned/float dispatch | `match type_def.operators.lt { IntInstr \| FloatInstr \| UnsignedCmp \| ... }` |
| `ori_types` (implicit) | Type checker knows `int + int` is valid but `bool + bool` is not | `type_def.operators.add != Unsupported` |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_llvm** | Primary consumer: `emit_binary_op()` reads `OpDefs` fields for dispatch |
| **ori_types** | Operator validation: is `+` valid on this type? |
| **ori_eval** | Could validate operator dispatch coverage (currently uses Value matching) |
| **ori_arc** | Not directly used |

### Checklist

- [ ] Define `OpDefs` struct in `ori_registry/src/operator.rs`
- [ ] Add derives: `Copy, Clone, Debug, PartialEq, Eq, Hash`
- [ ] Define `OpDefs::UNSUPPORTED` const for convenience
- [ ] Verify const-constructibility: all fields are `OpStrategy` which is `Copy`
- [ ] Add `/// ` doc comment on every field
- [ ] Add `//!` module doc on `operator.rs`
- [ ] Add size assertions: `const _: () = assert!(size_of::<OpStrategy>() == 24);` and `const _: () = assert!(size_of::<OpDefs>() == 480);` (20 fields x 24 bytes)
- [ ] Write unit tests: `UNSUPPORTED` has all fields `Unsupported`, field access works
- [ ] Test: construct an `OpDefs` in a `const` context with mixed strategies

---

## 01.8 TypeDef Struct

### Purpose

`TypeDef` is the top-level registry entry for a single builtin type. It is the complete behavioral specification: identity, memory strategy, methods, and operators. Every builtin type has exactly one `TypeDef`. The entire registry is a `&'static [&'static TypeDef]`.

### Rust Definition

```rust
/// Complete behavioral specification of a single builtin type.
///
/// This is the single source of truth for everything the compiler needs
/// to know about a builtin type's behavior across all phases. One
/// `TypeDef` per type, all phases read from it, no phase hard-codes
/// type knowledge independently.
///
/// Stored as `static` constants in the binary's `.rodata` segment.
/// Zero runtime cost to access.
///
/// # Extensibility
///
/// New required fields produce compile errors in every `TypeDef`
/// definition (Sections 03-07), enforcing immediate full coverage.
/// Optional future fields use `Option<NewDef>` (see Section 01.9).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TypeDef {
    /// The type's identity tag.
    pub tag: TypeTag,

    /// The type's name as it appears in Ori source code.
    ///
    /// Lowercase for primitives (`"int"`, `"str"`), proper-case for
    /// special types (`"Duration"`, `"Ordering"`).
    pub name: &'static str,

    /// How values of this type are managed in memory.
    pub memory: MemoryStrategy,

    /// Number of type parameters for generic types.
    ///
    /// `0` for primitives (int, str, bool, etc.) and non-generic compound
    /// types (Duration, Size, Ordering, Error).
    /// `1` for single-param generics (List<T>, Set<T>, Option<T>,
    /// Iterator<T>, Range<T>, Channel<T>).
    /// `2` for two-param generics (Map<K, V>, Result<T, E>).
    /// `TypeParamArity::Variadic` for tuples (0-N elements).
    pub type_params: TypeParamArity,

    /// All methods defined on this type.
    ///
    /// Includes inherent methods, trait method implementations, and
    /// associated functions. The full method set: every method the type
    /// checker accepts, the evaluator dispatches, and the LLVM backend
    /// emits.
    pub methods: &'static [MethodDef],

    /// Operator lowering strategies for this type.
    pub operators: OpDefs,
}
```

### Design Decisions

1. **No `Copy` derive**: `TypeDef` is ~520 bytes (verified on x86_64 stable Rust): `OpDefs` (20 fields x 24 bytes = 480) + `&'static str` name (16) + `&'static [MethodDef]` methods (16) + `TypeTag` (1) + `MemoryStrategy` (1) + `TypeParamArity` (2) + padding (4). Too large for `Copy`. It is always accessed via `&'static TypeDef` references. This is fine -- the data lives in static storage.

2. **`methods: &'static [MethodDef]`**: A static slice pointing to a static array. This is the natural const-constructible collection. Each type's methods are defined as a `static` array in their section file (e.g., `static INT_METHODS: [MethodDef; N] = [...]`) and the `TypeDef` points to it.

3. **`type_params: TypeParamArity`**: Generic types need their arity declared so downstream sections (05, 06, 07) can express type-parameter-relative return types (e.g., `List.first() -> Option<T>`). The registry does NOT resolve concrete instantiations (`List<int>` vs `List<str>`) — that remains in the type pool. It declares the *arity* so `ReturnTag` variants like `ElementType`, `KeyType`, `ValueType` are semantically grounded. `TypeParamArity` is `Fixed(u8)` for 0-2 params, `Variadic` for tuples.

4. **No `llvm_layout` field**: LLVM representation (i64, f64, struct types) is a backend-specific concern. `TypeInfo` in `ori_llvm` continues to own this. The registry describes *semantic* behavior (operators, methods, memory strategy), not *physical* layout.

5. **No `Display` name vs internal name split**: The `name` field serves both display (in error messages) and lookup. Currently, all type names used in error messages match the Ori source names (`"int"`, `"str"`, `"Duration"`). If display names ever diverge from internal names, a `display_name` field can be added.

### What It Replaces

| Current Location | Current Form | Registry Form |
|---|---|---|
| `ori_types/infer/expr/methods/resolve_by_type.rs` | 20 `resolve_*_method()` functions | `TypeDef.methods` lookup by name |
| `ori_types/infer/expr/methods/mod.rs` | `TYPECK_BUILTIN_METHODS` (390 entries) | `BUILTIN_TYPES.flat_map(\|td\| td.methods.iter().map(\|m\| (td.name, m.name)))` |
| `ori_eval/methods/helpers/mod.rs` | `EVAL_BUILTIN_METHODS` | Same enumeration |
| `ori_ir/builtin_methods/mod.rs` | `BUILTIN_METHODS` (123 entries) | Consolidated `TypeDef.methods` |
| `ori_llvm/codegen/arc_emitter/builtins/*.rs` | `declare_builtins!` entries | `TypeDef.methods` with `receiver: Ownership` |
| `ori_llvm/codegen/arc_emitter/operators.rs` | `emit_binary_op()` type guard chains | `TypeDef.operators` field dispatch |
| `ori_arc/classify/mod.rs` | `ArcClassifier::classify_primitive()` | `TypeDef.memory` field |

### Consuming Phases

| Phase | Usage |
|---|---|
| **ori_types** | Method resolution: `find_method(tag, name)` returns `&MethodDef` |
| **ori_eval** | Dispatch validation: iterate all methods, verify handler exists |
| **ori_arc** | Memory classification: `type_def.memory`; borrow inference: `method_def.receiver` |
| **ori_llvm** | Operator codegen: `type_def.operators`; method ownership: `method_def.receiver` |
| **ori_ir** | Migration: `BUILTIN_METHODS` becomes re-export of registry data |

### Checklist

- [ ] Define `TypeDef` struct in `ori_registry/src/type_def.rs`
- [ ] Add derives: `Clone, Debug, PartialEq, Eq, Hash` (no `Copy` -- too large)
- [ ] Verify const-constructibility: all fields are `const`-constructible
- [ ] Add `/// ` doc comment on every field
- [ ] Add `//!` module doc on `type_def.rs`
- [ ] Write unit tests: construct a `TypeDef` in a `const` context, access all fields
- [ ] Test: a `&'static TypeDef` pointing to static data compiles cleanly
- [ ] Test: `TypeDef` in a `&'static [&'static TypeDef]` slice compiles as const

---

## 01.8a TypeParamArity Enum

### Purpose

`TypeParamArity` declares how many type parameters a builtin type has. This is needed by Sections 05-07 to validate that `ReturnTag` variants like `ElementType`, `KeyType` are used on types with the correct arity.

### Rust Definition

```rust
/// How many type parameters a builtin type expects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeParamArity {
    /// Fixed number of type parameters (0, 1, or 2).
    ///
    /// `Fixed(0)` — primitives (int, str, bool, byte, char, float),
    ///               non-generic compounds (Duration, Size, Ordering, Error).
    /// `Fixed(1)` — List<T>, Set<T>, Option<T>, Iterator<T>, Range<T>, Channel<T>.
    /// `Fixed(2)` — Map<K, V>, Result<T, E>.
    Fixed(u8),

    /// Variadic type parameters (Tuple: 0-N elements).
    Variadic,
}
```

### Design Notes

- `Fixed(u8)` covers all current builtin types. No builtin has more than 2 type parameters.
- `Variadic` is only needed for Tuple. If other variadic types are ever added, this enum already handles them.
- `const`-constructible: `u8` and fieldless variants are trivially const.
- `Function` also uses `Variadic` — function signatures have arbitrary parameter counts. See TypeTag design decision 8.

### Checklist

- [ ] Define `TypeParamArity` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Add `/// ` doc comment on every variant
- [ ] Write unit tests: `Fixed(0)` for int, `Fixed(1)` for List, `Fixed(2)` for Map, `Variadic` for Tuple/Function

---

## 01.8b MethodKind Enum

### Purpose

`MethodKind` distinguishes instance methods (which take `self`) from associated functions (which do not). Duration and Size have static constructors (`Duration.from_seconds(ns:)`, `Size.from_bytes(b:)`) that are associated functions, not instance methods. Without this field, the registry cannot express them.

### Rust Definition

```rust
/// Whether a method is called on an instance or on the type itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum MethodKind {
    /// Instance method: called as `value.method(args)`.
    /// Has an implicit `self` receiver.
    Instance,

    /// Associated function: called as `Type.method(args)`.
    /// No `self` receiver. Examples: `Duration.from_seconds(ns:)`,
    /// `Size.from_bytes(b:)`.
    Associated,
}
```

### Impact on MethodDef

The `MethodDef` struct (01.6) includes a `kind` field:

```rust
pub struct MethodDef {
    // ... existing fields ...

    /// Whether this is an instance method or associated function.
    /// Default: `MethodKind::Instance` (the common case).
    pub kind: MethodKind,
}
```

For associated functions, `receiver: Ownership` is irrelevant (there is no receiver). By convention, associated functions use `receiver: Ownership::Copy` as a placeholder.

### Design Notes

- Only Duration and Size currently need `MethodKind::Associated` (for factory functions like `from_seconds`, `from_bytes`). All other builtin methods are `Instance`.
- Adding `kind` to `MethodDef` is a required field, not optional. This ensures every method declaration is explicit about its calling convention.

### Checklist

- [ ] Define `MethodKind` enum in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Add `/// ` doc comment on every variant
- [ ] Write unit tests: `MethodKind::Instance != MethodKind::Associated`

---

## 01.8c DeiPropagation Enum

### Purpose

`DeiPropagation` describes how an iterator adapter method affects `DoubleEndedIterator` capability. This is used exclusively by the Iterator/DEI type's method definitions (Section 07) and consumed by the query API (Section 08) to determine whether a chained adapter preserves reversibility.

### Rust Definition

```rust
/// How an iterator adapter method affects DoubleEndedIterator capability.
///
/// Only meaningful for iterator adapter methods (map, filter, take, etc.).
/// Consumer methods (fold, count, collect, etc.) and non-iterator methods
/// use `NotApplicable`.
///
/// The type checker uses this to determine whether `iter.map(f).rev()` is
/// valid: if `map`'s propagation is `Propagate`, and the source iterator
/// is DEI, then the result is also DEI.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum DeiPropagation {
    /// Adapter preserves DEI capability.
    ///
    /// If the input is DEI, the output is also DEI.
    /// Examples: `map`, `filter`, `enumerate`, `chain` (if both inputs are DEI).
    Propagate,

    /// Adapter downgrades DEI to plain Iterator.
    ///
    /// Even if the input is DEI, the output is only Iterator.
    /// Examples: `take`, `skip`, `flatten`, `flat_map`, `cycle`.
    /// Rationale: bounded iteration, nested iteration, or infinite repetition
    /// breaks the ability to efficiently iterate from the back.
    Downgrade,

    /// Not an adapter — this is a consumer or non-iterator method.
    ///
    /// Used for terminal operations (`fold`, `count`, `collect`, `for_each`)
    /// and all non-iterator methods. The field is meaningless for these
    /// methods; `NotApplicable` makes that explicit.
    NotApplicable,
}
```

### Design Notes

- The enum has 3 variants, not 2, to distinguish "this method doesn't participate in DEI propagation" from "this method actively destroys DEI capability." Without `NotApplicable`, consumer methods would need an arbitrary choice between `Propagate` and `Downgrade`.
- Prior art: Rust's `Iterator` and `DoubleEndedIterator` traits have separate impls for each adapter, making DEI propagation implicit in the type system. Ori's registry makes it explicit data because the single-TypeDef model (Section 07 Decision 1) merges both traits.

### Checklist

- [ ] Define `DeiPropagation` in `ori_registry/src/tags.rs`
- [ ] Add `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`
- [ ] Document all 3 variants
- [ ] Verify: `DeiPropagation` does NOT implement `Default` (no derive or manual impl -- forces explicit choice at every use site)
- [ ] Write unit test: all 3 variants are distinct and `const`-constructible

---

## 01.9 Extensibility Design

### Purpose

The registry must be evolvable. New fields will be added as the compiler grows: iteration definitions, hash strategies, display strategies, desugaring rules. The extensibility design governs how new fields are added without breaking existing type definitions or creating optional-field sprawl.

### Principle: Required By Default, Optional By Exception

Every new field should be **required** unless there's a strong reason to make it optional. Required fields produce compile errors in every `TypeDef` when added, which is the primary enforcement mechanism. Optional fields (`Option<T>`) silently default to `None` and can be forgotten.

**When to use required fields:**
- The field is meaningful for every type (even if the value is "not applicable")
- There's a natural "not applicable" variant in the field's enum (e.g., `OpStrategy::Unsupported`)
- Forgetting to set it would be a bug

**When to use `Option<T>`:**
- The field is genuinely irrelevant for most types (e.g., an iteration strategy for types that are not iterable)
- The default (`None`) is always correct when the feature doesn't apply
- The cost of requiring every type to explicitly set `None` exceeds the safety benefit

### Future Candidate Fields

These fields are NOT part of the initial registry but are anticipated for future sections:

#### `IterationDef` (strong candidate for `Option<IterationDef>`)

```rust
/// How a type produces an iterator.
///
/// Only meaningful for iterable types (list, str, map, set, range).
/// `None` for non-iterable types (int, float, bool, etc.).
pub struct IterationDef {
    /// The element type produced by iteration.
    pub element: ReturnTag,
    /// Whether the iterator is double-ended.
    pub double_ended: bool,
}
```

**Justification for `Option`**: Most types (11 of 23) are not iterable. Requiring `IterationDef` on `int`, `float`, `bool`, etc. would add noise without safety benefit. `None` is always correct for non-iterable types.

#### `HashStrategy` (strong candidate for required field via enum)

```rust
/// How values of this type are hashed.
pub enum HashStrategy {
    /// Identity hash (value IS its hash). Used by: int, Duration, Size.
    Identity,
    /// Bitcast to i64 with normalization. Used by: float (±0 normalization).
    Bitcast,
    /// Zero-extend to i64. Used by: bool, byte.
    ZeroExtend,
    /// Sign-extend to i64. Used by: char.
    SignExtend,
    /// Runtime function call. Used by: str (ori_str_hash).
    RuntimeCall { fn_name: &'static str },
    /// Not hashable. Used by: function, channel, iterator.
    NotHashable,
}
```

**Justification for required**: Every type either is hashable (with a strategy) or is not (`NotHashable`). There's a natural "not applicable" variant. Forgetting to set this would be a bug when adding a new type.

#### `DisplayStrategy` (candidate for `Option<DisplayDef>`)

```rust
/// How values of this type are displayed (for debug/to_str).
pub struct DisplayDef {
    /// Runtime function for display. None if handled inline.
    pub runtime_fn: Option<&'static str>,
}
```

**Justification for `Option`**: This is more of an optimization hint than a correctness concern. The evaluator handles display via direct pattern matching; the LLVM backend calls runtime functions. The registry could centralize the runtime function names, but it's lower priority than methods and operators.

#### `DesugarRule` (speculative, probably NOT on `TypeDef`)

Desugaring rules (how `for x in list` becomes `list.iter()` calls) are more about syntax transformations than type behavior. They would likely live in a separate part of the registry or in the parser/type checker, not on `TypeDef`.

### Rust Exhaustiveness as Enforcement

The key insight: `TypeDef` is a struct, not a trait. Adding a field to a struct is a **hard compile error** in every location that constructs the struct without the field. This is strictly stronger than:

- Adding a method to a trait (which can have a default impl, hiding the addition)
- Adding a variant to an enum (which is caught by `match` but not by construction)
- Adding an entry to a `HashMap` (which has no compile-time enforcement)

Every `TypeDef` constant in Sections 03-07 uses named field syntax:

```rust
pub static INT: TypeDef = TypeDef {
    tag: TypeTag::Int,
    name: "int",
    memory: MemoryStrategy::Copy,
    type_params: TypeParamArity::Fixed(0),
    methods: &INT_METHODS,
    operators: INT_OPS,
    // Adding a new required field here → compile error until filled in
};
```

This means adding `hash_strategy: HashStrategy` to `TypeDef` produces a compile error in EVERY type definition file. The developer MUST set the field for all 23 types before the compiler accepts the change. This is the structural guarantee.

### Enforcement Boundary: Construction vs Consumption

Struct construction exhaustiveness is the **primary** enforcement mechanism — it guarantees that every type definition includes every field. However, it has a known limitation: **consuming phases that read registry data are not forced to handle new fields.** A phase that reads `type_def.operators.add` won't get a compile error when `type_def.operators.floor_div` is added — it simply won't read the new field.

This is addressed by **two complementary mechanisms**:

1. **Primary (compile-time): Struct construction.** Adding a field to `TypeDef`, `OpDefs`, or `MethodDef` forces every type definition to be updated. This catches the "declaration side" — no type can exist without specifying the new fact.

2. **Secondary (test-time): Section 14 enforcement tests.** These iterate the registry and verify every consuming phase handles every declared fact:
   - Every `OpStrategy` that's not `Unsupported` must be matched in the LLVM backend
   - Every `MethodDef` must have a dispatch handler in the evaluator
   - Every method must be recognized by the type checker
   - Every `Ownership::Borrow` annotation must be consumed by the ARC pass

Neither mechanism alone is sufficient. Construction exhaustiveness catches missing *declarations*. Enforcement tests catch missing *consumption*. Together they close the loop.

### Version Strategy

The registry does NOT need versioning. It is a compile-time artifact consumed by crates in the same workspace. All consumers are recompiled together. There is no binary compatibility concern. The Rust compiler's type system IS the versioning mechanism.

### Checklist

- [ ] Document the "required by default, optional by exception" principle in `ori_registry/src/lib.rs` module docs
- [ ] Document each future candidate field with its justification for required vs optional
- [ ] Write a compile-time test that constructs a `TypeDef` with all fields (verifies constructibility)
- [ ] Add a section in the crate docs showing how to add a new field (workflow for future developers)

---

## Sync Points — Adding a New TypeTag Variant

When a new `TypeTag` variant is added, the following locations MUST ALL be updated in the same commit:

| Location | What to update |
|---|---|
| `ori_registry/src/tags.rs` | Add variant to `TypeTag` enum |
| `TypeTag::name()` | Add match arm returning Ori-level type name |
| `TypeTag::all()` | Add variant to the static slice |
| `TypeTag::is_primitive()` / `is_generic()` | Add to the appropriate predicate |
| `TypeTag::base_type()` | Add match arm (returns `self` unless DEI-like alias) |
| `ori_registry/src/defs/<type>.rs` (new file) | Define `TypeDef` const with all fields |
| `BUILTIN_TYPES` array (Section 08) | Add `&<TYPE>` entry |
| `ori_types` bridge function | Add `Tag::NewType => TypeTag::NewType` mapping |
| `ori_eval` bridge function | Add `Value::NewType => TypeTag::NewType` mapping |
| `ori_llvm` bridge function | Add `TypeInfo::NewType => TypeTag::NewType` mapping |
| `ori_arc` bridge function | Add classification mapping |
| `_enforce_exhaustiveness()` (Section 14) | All 4 consuming crates get compile error automatically |

This is structurally enforced by Rust exhaustiveness: adding a `TypeTag` variant is a compile error in every `match` on `TypeTag` across all consuming crates.

## Sync Points — Adding a New OpDefs Field

When a new operator field is added to `OpDefs`:

| Location | What to update |
|---|---|
| `ori_registry/src/operator.rs` | Add field to `OpDefs` struct |
| `OpDefs::UNSUPPORTED` | Add `field: OpStrategy::Unsupported` |
| All `TypeDef` definitions (Sections 03-07) | Add field to every type's `OpDefs` const |
| `ori_llvm` operator emission | Add match arm for the new field in `emit_binary_op()` or `emit_unary_op()` |
| `ori_types` operator validation | Add validation for the new operator |
| Section 14 enforcement test | Verify all non-`Unsupported` strategies have backend handlers |

This is structurally enforced by Rust struct construction: every `OpDefs` literal must include all fields.

---

## Exit Criteria

Section 01 is complete when ALL of the following are true:

1. **All types are finalized**: `TypeTag`, `MemoryStrategy`, `Ownership`, `OpStrategy`, `ReturnTag`, `TypeProjection`, `ParamDef`, `MethodDef`, `OpDefs`, `TypeDef`, `TypeParamArity`, `MethodKind`, `DeiPropagation` -- each has an exact Rust definition with derive macros, documentation, and design rationale.

2. **Const-constructibility verified**: Every type can be instantiated in a `const` or `static` context. This is verified by writing `const _: TypeDef = TypeDef { ... }` test expressions.

3. **No LLVM/Pool/Arena dependency**: None of the types reference `inkwell`, `ori_types::Idx`, `ori_types::Pool`, `ori_ir::ExprId`, or any phase-specific type. They use only primitive Rust types, `&'static str`, `&'static [T]`, and other registry types.

4. **Design decisions documented**: Every choice (why two `MemoryStrategy` variants not three, why three `Ownership` variants not two, why `ReturnTag` is separate from `TypeTag`, why `OpDefs` has 20 expanded fields) is recorded with rationale.

5. **Replacement mapping complete**: Every type has a table showing what it replaces in the current codebase (file, current form, registry form).

6. **Consuming phase usage documented**: Every type has a table showing which phases read it and how.

7. **Extensibility design documented**: The principle for adding new fields (required vs optional), future candidate fields, and the Rust exhaustiveness enforcement mechanism are all recorded.

8. **Documentation complete**: Every pub type has `///` doc comments. Every module file has `//!` module docs. This is per coding guidelines (impl-hygiene.md).

9. **Size assertions specified**: `TypeTag` (1 byte), `MemoryStrategy` (1 byte), `Ownership` (1 byte), `OpStrategy` (24 bytes), `ReturnTag` (2 bytes), `TypeProjection` (1 byte), `ParamDef` (24 bytes), `MethodDef` (<=64 bytes, actual ~56), `OpDefs` (480 bytes), `TypeDef` (~520 bytes, no `Copy`), `MethodKind` (1 byte), `DeiPropagation` (1 byte), `TypeParamArity` (2 bytes). All verified by compiling with `std::mem::size_of`.

10. **Operator coverage verified**: Every variant in `BinaryOp` and `UnaryOp` (from `ori_ir::ast::operators`) is accounted for in `OpDefs` — either as a field, or with a documented design decision explaining exclusion (e.g., `Pow` is desugared, `MatMul` is trait-dispatched, `And`/`Or` are short-circuit control flow, `Range`/`RangeInclusive`/`Coalesce` are desugared, `Try` is desugared).

11. **ReturnTag coverage verified**: Every distinct return type pattern in the existing `resolve_*_method()` functions is expressible as a `ReturnTag` variant. Patterns checked: concrete Idx, Self, fresh var, Option<element>, List<element>, Iterator<element>, DEI<element>, (Option<T>, Self) next protocol, Result<T, fresh>, Map iterator, enumerate tuple.

12. **No implementation started**: This section is design-only. No `.rs` files are created (that's Section 02). The output is this document, reviewed and approved.
