---
paths:
  - "**ori_types**"
---

# Type System Formal Ruleset

This document defines the **laws** of the Ori type system — the interned representation of Ori types, their structural properties, their relationships, and the registries that back builtin behavior. The spec (`docs/ori_lang/v2026/spec/`) defines **what** types exist and how they behave in the language; this document defines **how** the compiler's type representation implements that spec faithfully. If the code violates a rule stated here, the code has a bug.

**Relationship to other rulesets**: The parser (`parse.md`) produces an AST carrying `TypeId`s that reference spec-visible type syntax. The type checker (`typeck.md`) consumes that AST, converts `TypeId` into pool-interned `Idx`, performs inference/unification against the rules in this document, and emits typed IR whose type handles obey the invariants defined here. Downstream phases — evaluation (`ori_eval`), ARC analysis (`aims-rules.md`), codegen (`codegen-rules.md`) — SHALL NOT re-derive type structure. They query the pool defined here. `types.md` is **about the type representation**; `typeck.md` is **about the algorithm that fills it in**. The two are the pair of files that govern the `ori_types` crate.

**Relationship to compiler.md and impl-hygiene.md**: Those files are *operational* guides (architecture, hygiene, Salsa). This document is *normative* (what the type system must guarantee). When they conflict, this document is authoritative for type-system-specific rules.

**Scope**: This ruleset covers the type pool (`ori_types::pool`), type kind tags (`ori_types::tag`), type flags (`ori_types::flags`), type identity (interning, Merkle hashing), the spec-visible type surface, property traits, type schemes, the three registries (type / trait / method), and the phase contracts that govern how types enter and leave the crate. Inference, unification, trait resolution, and capability checking live in `typeck.md`.

**Target-only rules**: Rules marked **(target-only)** describe the COMPLETE target system per the spec. The implementation may not have shipped them yet. The spec is authoritative; code divergences are bugs to file, not spec inaccuracies. These annotations prevent reviewers from re-flagging known implementation gaps as spec issues.

---

## Notation

- **SHALL** = mandatory requirement (violation = implementation bug)
- **SHOULD** = recommended practice (violation = design smell, may be justified)
- **Idx** = `ori_types::idx::Idx` — 32-bit handle into the type pool
- **TypeId** = `ori_ir::TypeId` — parser-level type reference; identity with `Idx` for pre-interned primitives
- **Item** = `ori_types::item::Item` — compact 5-byte pool entry (1 tag + 4 data)
- **Pool** = `ori_types::pool::Pool` — the interning store; every type has exactly one `Idx` in exactly one pool
- **Tag** = `ori_types::tag::Tag` — 1-byte kind discriminant
- Rules are numbered `CATEGORY-N`. Categories: `TY` (type pool / storage), `TK` (type kinds / tag catalog), `TI` (type identity / interning / hashing), `TF` (type flags), `TL` (type surface per spec), `PT` (properties of types), `BI` (builtin trait interface), `SC` (scheme / quantification), `RG` (registries), `PC` (phase contracts), `SL` (Salsa / caching), `DI` (diagnostics produced by type storage itself), `TRG` (tracing). `typeck.md`'s `TR-*` (Trait Resolution) is a distinct category living in a different file; this file's `BI-*` defines the trait INTERFACE (canonical methods / derivability / object safety rules). Cross-file references always use `TYPES:BI-*` (into this file) or `CHK:TR-*` (into typeck.md).
- Cross-references: `typeck.md` rules prefixed with `CHK:` (e.g., `CHK:UN-1`), `parse.md` with `PARSE:`, `aims-rules.md` with `AIMS:`, `codegen-rules.md` with `CG:`, `impl-hygiene.md` with `HYG:`, spec clauses with `Spec:` (e.g., `Spec: Clause 8.1`)

---

## §1 Pool Architecture

The type pool is the single source of truth for type structure in the compiler. Every type that participates in checking, evaluation, ARC, or codegen exists as exactly one `Idx` in exactly one pool. The pool is append-only during a module's checking session; `Idx` values never move and are stable for the life of the pool.

Source: `ori_types/src/pool/`.

### TY-1 — Idx as the Canonical Type Handle

`Idx(u32)` SHALL be the canonical representation of a type in the compiler outside the source AST. Consumers SHALL NOT construct types as trees, `Box<Type>` graphs, or strings. Every type reaches consumers as an `Idx` to be queried against a pool.

Rationale: A single 32-bit handle makes type equality O(1) (integer compare), enables hash-keyed caching (`Idx` hashes in one instruction), and eliminates recursive type traversal at use sites. This is the Rust `Ty<'tcx>` / Zig `InternPool.Index` / Swift `CanType` pattern applied to Ori.

### TY-2 — Pool Storage Layout

The pool SHALL store types in parallel column arrays plus auxiliary maps/vectors. The parallel columns — one entry per `Idx.raw() as usize` — are:

| Column | Type | Purpose |
|--------|------|---------|
| `items` | `Vec<Item>` | `(tag, data)` pair per type |
| `flags` | `Vec<TypeFlags>` | Pre-computed metadata (TF-1) |
| `hashes` | `Vec<u64>` | Structural Merkle hash for deduplication |

The auxiliary (non-parallel) state:

| Field | Type | Purpose |
|-------|------|---------|
| `extra` | `Vec<u32>` | Variable-length payload for types that need more than 4 bytes of data |
| `intern_map` | `FxHashMap<u64, Idx>` | Hash → Idx map for O(1) re-interning |
| `resolutions` | `FxHashMap<Idx, Idx>` | Named/Applied → concrete Struct/Enum mapping populated during registration |
| `var_states` | `Vec<VarState>` | Per-variable state (Unbound / Link / Rigid / Generalized); indexed by `var_id`, NOT by `Idx` |
| `next_var_id` | `u32` | Counter for fresh type-variable ids (`CHK:EN-3`) |

`items.len() == flags.len() == hashes.len()` SHALL hold as a pool invariant. `var_states` is NOT parallel to `items` — variable state is indexed by `var_id` carried in a variable item's `data` field (`TK-1` Type Variables row).

Rationale: Parallel columns give cache-friendly tag-dispatch-then-data reads. Auxiliary maps (intern, resolutions) are rebuilt in place; variable state lives in its own vector because the `var_id` identity is rank-scoped rather than pool-scoped.

### TY-3 — Item Representation

`Item` SHALL carry a 5-byte logical payload: 1 byte `Tag` + 4 bytes `data: u32`. The struct is `#[repr(C)]`; the ABI size is 5 to 8 bytes depending on target alignment. Adding a new variant SHALL NOT grow the logical payload beyond 5 bytes — widening the logical payload invalidates all fixed-width pool assumptions downstream.

A test-time assertion (`const _: () = { assert!(size_of::<Item>() >= 5); assert!(size_of::<Item>() <= 8); };`) SHALL guard the bounded ABI range.

Rationale: 5-byte logical payload keeps item data dense while `#[repr(C)]` gives a predictable (possibly padded) ABI layout. A strict `== 5` assertion would require `repr(packed)`, which the pool does not use.

### TY-4 — Extra Array Discipline

Tags whose payload exceeds 4 bytes (two-child containers, complex types, named types, schemes, projections) SHALL store `data = extra_offset` where `extra[offset..offset+len]` contains the payload. The payload length is:

- **Two-child tags** (Map, Result, Borrowed): 2 consecutive `Idx` values
- **Complex tags** (Function, Tuple, Struct, Enum): length prefix followed by child `Idx`s and/or extra fields per-tag
- **Named tags** (Named, Applied, Alias): `Name` + generic args
- **Scheme** (Scheme): bound-var count + body `Idx`
- **Projection**: receiver `Idx` + `Name` of associated type

`Tag::uses_extra()` SHALL return `true` iff the tag reads from `extra`.

Rationale: Uniform 5-byte items with variable-length extra keeps the hot column dense and pushes variable cost to cold paths.

### TY-5 — Pre-Interned Primitives and Reserved Range

The primitive *named constants* SHALL occupy `Idx(0)` through `Idx(11)` in every pool, in this fixed order:

| Idx | Type | Tag | `Idx` constant |
|-----|------|-----|----------------|
| 0 | `int` | `Tag::Int` | `Idx::INT` |
| 1 | `float` | `Tag::Float` | `Idx::FLOAT` |
| 2 | `bool` | `Tag::Bool` | `Idx::BOOL` |
| 3 | `str` | `Tag::Str` | `Idx::STR` |
| 4 | `char` | `Tag::Char` | `Idx::CHAR` |
| 5 | `byte` | `Tag::Byte` | `Idx::BYTE` |
| 6 | `()` (unit) | `Tag::Unit` | `Idx::UNIT` |
| 7 | `Never` | `Tag::Never` | `Idx::NEVER` |
| 8 | `<error>` | `Tag::Error` | `Idx::ERROR` |
| 9 | `Duration` | `Tag::Duration` | `Idx::DURATION` |
| 10 | `Size` | `Tag::Size` | `Idx::SIZE` |
| 11 | `Ordering` | `Tag::Ordering` | `Idx::ORDERING` |

The entire pre-interned range is `Idx(0)` through `Idx(Idx::FIRST_DYNAMIC - 1)` with `FIRST_DYNAMIC = 64`. Indices 12..64 are RESERVED for future primitives and are padded at pool construction with `Item::primitive(Tag::Error)` placeholders carrying `TypeFlags::HAS_ERROR`. Dynamically-interned types begin at `Idx::FIRST_DYNAMIC`.

`Idx::is_primitive()` SHALL be implemented as `self.0 < Idx::FIRST_DYNAMIC` — any index below the boundary is treated as pre-reserved, including the padded slots.

Pre-interning SHALL happen in `Pool::intern_primitives()` during `Pool::new()`. Pool users SHALL NOT construct primitive `Idx` values manually; they SHALL use the named `Idx::*` constants. There is no `Pool::primitive(tag)` public API — primitive items are constructed internally via `Item::primitive(tag)` at pool initialization.

Rationale: Fixed indices allow `TypeId` (parser-level) and `Idx` (pool-level) to be identical for primitives, eliminating a lookup step on every primitive type reference. Padding the reserved range keeps `FIRST_DYNAMIC` stable even when new primitives are added — consumers of dynamic indices never see a discontinuity.

Spec: Clause 8.1 (primitive types).

### TY-6 — Pool Append-Only During Check

During a single `check_module()` session, the pool SHALL be append-only: existing `Idx` values do not move, existing `Item`/extra/flags/hashes entries do not mutate. New types are appended; re-interning returns a prior `Idx`.

Exception: `pool/re_intern/` performs cross-pool migration (reinterning a tree of types from one pool into another). Migration constructs fresh `Idx` values in the destination; it does NOT mutate the source.

Rationale: Append-only guarantees `Idx` stability — consumers may hold `Idx` values across any pool operation without invalidation risk. This is load-bearing for Salsa memoization (SL-1) and for the typed IR produced by the checker.

---

## §2 Type Kinds — Tag Catalog

Every type has a `Tag: u8` that determines how its `data` field is interpreted and what operations are legal on it. The tag space is partitioned into semantic ranges with reserved slots for future growth.

Source: `ori_types/src/tag/mod.rs`.

### TK-1 — Tag Range Partition

Tags SHALL be partitioned into the following ranges. Adding a new tag SHALL place it within the correct range and update this table. Ranges with reserved slots SHALL NOT overflow into adjacent ranges.

| Range | Category | Data interpretation | Shipped tags |
|-------|----------|---------------------|--------------|
| `0..16` | Primitives | unused (data = 0) | Int, Float, Bool, Str, Char, Byte, Unit, Never, Error, Duration, Size, Ordering |
| `16..32` | Simple containers | `data = child_idx.raw()` | List, Option, Set, Channel, Range, Iterator, DoubleEndedIterator |
| `32..48` | Two-child containers | `data = extra_offset` (2 consecutive Idx) | Map, Result, Borrowed (reserved, not yet constructed) |
| `48..80` | Complex types | `data = extra_offset` (length prefix) | Function, Tuple, Struct, Enum |
| `80..96` | Named types | `data = extra_offset` | Named, Applied, Alias |
| `96..112` | Type variables | `data = var_id` (into `var_states`) | Var, BoundVar, RigidVar |
| `112..128` | Schemes | `data = extra_offset` | Scheme |
| `128..240` | *(reserved)* | — | — |
| `240..256` | Special | tag-specific | Projection, ModuleNs, Infer, SelfType |

The following classification predicates SHALL be provided and stable:

- `Tag::is_primitive()` — discriminant `< 16`
- `Tag::is_container()` — `16 ≤ discriminant < 48`
- `Tag::is_iterator()` — `Iterator | DoubleEndedIterator`
- `Tag::is_type_variable()` — `Var | BoundVar | RigidVar`
- `Tag::uses_extra()` — tag reads from the extra array
- `Tag::has_child_in_data()` — tag encodes a single child `Idx` directly in `data` (simple containers only)
- `Tag::is_merkle_leaf()` — hash depends only on `(tag, data)`, no child lookup

Spec: Clause 8 (types), Clause 8.13 (Iterator traits — distinguishes `Iterator` from `DoubleEndedIterator`).

### TK-2 — Tag Size Assertion

`size_of::<Tag>()` SHALL be exactly 1 byte. A compile-time assertion SHALL guard this. Widening `Tag` past 1 byte invalidates the 5-byte `Item` layout (TY-3).

### TK-3 — Error Tag as Poison Type

`Tag::Error` SHALL represent a type error placeholder. It unifies with anything (`CHK:UN-4`), propagates the `HAS_ERROR` flag (TF-3), and suppresses cascading diagnostics per `HYG:Error Recovery Monotonicity`. Any operation on `Tag::Error` SHALL succeed silently without emitting a new diagnostic.

Rationale: Poisoning keeps error recovery monotone — a single user mistake does not produce N downstream errors.

### TK-4 — Never as Bottom Type

`Tag::Never` SHALL represent the bottom type `Never`. `Never` coerces into any other type (`CHK:UN-3`), is uninhabited at runtime, and may appear as a sum-variant payload but never as a struct field.

Spec: Clause 8.1 primitive table, Clause 8.1.1 (Never Semantics — coercion, producers, inference), Clause 8.6 (user-defined types — struct fields SHALL NOT be `Never` per uninhabited-field rule surfaced as `E2019`).

### TK-5 — Infer vs Var

`Tag::Infer` SHALL represent a placeholder awaiting type resolution in the AST (producer: parser). `Tag::Var` SHALL represent an active unification variable allocated by `InferEngine::fresh_var()` (producer: type checker). `Tag::Infer` SHALL NOT reach the pool used by unification; the checker converts each AST `Infer` into a fresh `Var` during entry (`CHK:CK-3`).

Rationale: Two separate tags make the producer explicit — `Infer` is a syntactic marker, `Var` is a semantic hole. Mixing them loses that provenance.

### TK-6 — Rigid Variables

`Tag::RigidVar` SHALL represent a user-annotated generic parameter (e.g., `@f<T>`). Rigid variables SHALL NOT unify with concrete types; attempting to unify a `RigidVar` with a non-variable type produces `E2003` (via `TypeErrorKind::RigidMismatch`) with the rigid name in the diagnostic. See `CHK:UN-6` for the checker-side enforcement and `CHK:DI-1` for the complete error-code mapping.

Rationale: Rigid variables preserve parametricity — `@f<T> (x: T) -> T` must typecheck as identity, not as any specific `T`. The rigid tag makes the "can't narrow" property a tag-level invariant, not an annotation-tracking side table.

Spec: Clause 8.3 (generic types).

### TK-7 — Self Type Tag

`Tag::SelfType` SHALL represent the `Self` reference inside `trait` or `impl` blocks. The checker substitutes `SelfType` with the current implementing type at each use site. `SelfType` reaching codegen is a phase contract violation (`PC-2`).

Spec: Clause 8.8 (trait objects), Clause 8.6 (user-defined types).

### TK-8 — Projection for Associated Types

`Tag::Projection` SHALL represent an unresolved associated type reference `T.Item` where `T: Iterable`. Extra stores `[receiver_idx, name_id]`. Projections are normalized by the checker when the receiver resolves to a concrete type that implements the trait; an unresolved projection at codegen time is a phase contract violation (`PC-2`).

Spec: Clause 8.13 (Iterator traits), Clause 8.6.3 (user-defined traits).

### TK-9 — Named, Applied, and Alias

- `Tag::Named` SHALL represent a reference to a user-defined nominal type by name, pre-resolution. Every `type N = { ... }` struct, `type N = A | B` sum, and `type N = ExistingType` newtype produces a `Tag::Named` entry — all three are the parser's `TypeDeclKind::Struct | Sum | Newtype` variants, all landing in the pool as `Named`.
- `Tag::Applied` SHALL represent `Named(args...)` — a nominal type instantiated with generic arguments.
- `Tag::Alias` is reserved in the tag catalog for compiler-internal transparent references (import aliases, well-known-type re-exposures). The Ori surface syntax `type N = ExistingType` is parsed as `TypeDeclKind::Newtype` — NOT as a transparent alias — so `Tag::Alias` SHALL NOT be produced from the user-writable `type` declaration surface.

Nominal identity comes from the `Tag::Named` entry under `TI-5`; the distinction between Struct/Sum/Newtype shape is recorded in the `TypeRegistry`, not in the tag itself.

Spec: Clauses 8.6 (user-defined types, including 8.6.3 newtype section), 8.7 (nominal typing).

---

## §3 Type Identity & Interning

Type equality in Ori is **structural identity under interning**: two types are equal iff their `Idx` values are equal. The pool guarantees this by hashing and deduplicating every constructed type.

Source: `ori_types/src/pool/mod.rs` (interning), `ori_types/src/pool/construct/` (hash computation).

### TI-1 — Structural Interning

The pool SHALL deduplicate types by structural hash. `Pool::intern(item, extra)` SHALL:

1. Compute a Merkle hash over `(tag, data, extra_payload)`, recursing via `hashes[child_idx]` for any child type referenced in `data` or `extra`.
2. Probe the `interner` map: if the hash matches an existing `Idx`, return that `Idx`.
3. Otherwise, append to `items`/`flags`/`hashes`, record the mapping, and return the new `Idx`.

Rationale: Structural hashing + probe gives content-based deduplication — the same type constructed by two different callers returns the same `Idx`. This is the basis for O(1) equality (TI-2).

### TI-2 — O(1) Type Equality

`Idx` equality SHALL be type equality. `a == b` on `Idx` values SHALL be sound and complete for any two types interned in the same pool.

Consumers SHALL NOT walk type structure to compare equality. Consumers SHALL NOT compare `Item` fields directly outside the pool itself — such a comparison is a LEAK against this rule per `HYG:§Interning Discipline`.

Rationale: Interning concentrates the cost of equality at construction time (once per unique type) rather than at every comparison (once per use).

### TI-3 — Merkle Hash Classification

Every tag SHALL fall into exactly one of three Merkle hash classes:

| Class | Predicate | Hashing rule |
|-------|-----------|--------------|
| Leaf | `is_merkle_leaf()` | `hash(tag, data)` — no child lookup |
| Child-in-data | `has_child_in_data()` | `hash(tag, pool.hashes[data as usize])` — one child lookup |
| Extra-backed | `uses_extra()` | `hash(tag, extra_payload_with_child_hashes)` — walks extra |

The three predicates SHALL partition the tag space with no overlap. `is_merkle_leaf()` SHALL be defined as `!has_child_in_data() && !uses_extra()`.

Rationale: Uniform hash classification lets `Pool::intern` choose the right hashing strategy from the tag alone — no per-tag special cases leak into the interner.

### TI-4 — Cross-Pool Identity is Undefined

`Idx` values from different pools SHALL NOT be compared. Consumers holding an `Idx` SHALL also hold a reference to the `Pool` that produced it (or a `PoolId` identifying it).

Exception: The first 12 slots are pre-interned primitives (TY-5); their `Idx` values coincide across every pool. This is documented and is the only cross-pool stable identity.

Rationale: Cross-pool comparison is a silent miscompilation waiting to happen — two pools may assign different `Idx` values to structurally equal types.

### TI-5 — Newtype Identity Is Nominal, Not Structural

A newtype (`type UserId = int`) SHALL intern as a distinct `Idx` from its underlying type, even though its payload is structurally identical. The pool encodes this as a `Named` tag wrapping the underlying type, so the structural hash differs.

Consequence: `UserId(42)` has type `UserId` (one `Idx`); `42` has type `int` (a different `Idx`). `UserId` does NOT implicitly satisfy `int`'s bounds — trait impls on `int` do not apply unless also declared on `UserId` (or `UserId` explicitly derives / extends the behavior).

Spec: Clause 8.7 (nominal typing).

---

## §4 Type Flags

`TypeFlags` are pre-computed type metadata — a single `u32` bitset packed with presence, category, optimization, and capability information. Flags are computed once at interning time and cached in the parallel `flags` column; queries are O(1) bit tests.

Source: `ori_types/src/flags/mod.rs`.

### TF-1 — Flag Catalog

The complete `TypeFlags` bitset SHALL be:

| Bit | Flag | Group | Meaning |
|-----|------|-------|---------|
| 0 | `HAS_VAR` | Presence | Contains at least one unbound `Tag::Var` |
| 1 | `HAS_BOUND_VAR` | Presence | Contains at least one `Tag::BoundVar` (under a scheme) |
| 2 | `HAS_RIGID_VAR` | Presence | Contains at least one `Tag::RigidVar` (user-annotated generic) |
| 3 | `HAS_ERROR` | Presence | Contains `Tag::Error` — poison propagation |
| 4 | `HAS_INFER` | Presence | Contains `Tag::Infer` — AST-level placeholder |
| 5 | `HAS_SELF` | Presence | Contains `Tag::SelfType` |
| 6 | `HAS_PROJECTION` | Presence | Contains `Tag::Projection` |
| 8 | `IS_PRIMITIVE` | Category | Type is a primitive |
| 9 | `IS_CONTAINER` | Category | Type is a generic container (List/Option/Set/etc.) |
| 10 | `IS_FUNCTION` | Category | Type is a function type |
| 11 | `IS_COMPOSITE` | Category | Type is Struct/Enum/Tuple |
| 12 | `IS_NAMED` | Category | Type is Named/Applied/Alias |
| 13 | `IS_SCHEME` | Category | Type is a quantified scheme |
| 16 | `NEEDS_SUBST` | Optimization | Has unresolved variables requiring substitution |
| 17 | `IS_RESOLVED` | Optimization | Fully resolved — no variables, no projections |
| 18 | `IS_MONO` | Optimization | Monomorphic — no generic parameters |
| 19 | `IS_COPYABLE` | Optimization | Known to be bit-copyable (Value trait) |
| 24 | `HAS_CAPABILITY` | Capability | **(Target-only)** Bit reserved for "contains a function type that `uses` at least one capability". Today `Tag::Function` pool entries do not store capability sets — capabilities are carried on `FunctionSig.capabilities` alongside the pool `Idx` (see `CHK:CP-1`). Propagation via this flag activates when in-pool capability encoding ships. |
| 25 | `IS_PURE` | Capability | Guaranteed pure (no effects) |
| 26 | `HAS_IO` | Capability | Contains a function with IO effects |
| 27 | `HAS_ASYNC` | Capability | Contains a function with async / `Suspend` effects |

Bits 7, 14–15, 20–23, 28–31 are RESERVED. New flags SHALL be added in the same group band.

### TF-2 — Flag Computation at Interning Time

`TypeFlags` SHALL be computed during `Pool::intern` and cached in the parallel `flags` column. Consumers SHALL NOT recompute flags by walking the type structure. `Pool::flags(idx)` SHALL return the cached value in O(1).

Rationale: Flag computation amortizes one traversal across all future queries. Recomputation at consumption time is a WASTE finding per `HYG:§Data Flow`.

### TF-3 — Flag Propagation From Children

For compound types (simple containers, two-child, complex, named, scheme), the `PROPAGATE_MASK` flags SHALL be OR'd from every child into the parent:

`PROPAGATE_MASK = HAS_VAR | HAS_BOUND_VAR | HAS_RIGID_VAR | HAS_ERROR | HAS_INFER | HAS_SELF | HAS_PROJECTION | NEEDS_SUBST | HAS_CAPABILITY | HAS_IO | HAS_ASYNC`

Category flags (`IS_PRIMITIVE`, `IS_FUNCTION`, etc.) SHALL NOT propagate — they are set by the parent tag alone. Optimization flags other than `NEEDS_SUBST` SHALL NOT propagate.

Rationale: Propagation makes "does this type contain any unresolved variable?" an O(1) query on the parent, not a recursive walk.

### TF-4 — Category Dispatch

`TypeFlags::category()` SHALL return a single `TypeCategory` enum value derived from the category bits, in this priority order: Primitive > Function > Container > Composite > Scheme > Named > Variable > Unknown.

Consumers SHALL use `category()` for coarse-grained dispatch rather than repeating the priority logic.

Rationale: One canonical priority means consumers cannot disagree on how to classify overlapping categorizations.

### TF-5 — Fast-Path Gates

The following optimizations SHALL use flag bits as early-exit gates:

| Operation | Gate | Reason to skip |
|-----------|------|----------------|
| Substitution | `!NEEDS_SUBST` | No variables to substitute |
| Occurs check | `!HAS_VAR` | No variables to check |
| Trait dispatch (fast path) | `!is_primitive() && !is_type_variable()` | Primitives route through `ori_registry`; vars defer |
| Generalization scan | `HAS_VAR` | No variables to generalize |

Consumers that skip these gates (e.g., walking types unconditionally) are LEAKs against the "don't recompute at consumption time" rule.

---

## §5 Spec Type Surface

The compiler's internal representation (TY-/TK-/TI-/TF-) SHALL model the complete user-visible type surface described in `docs/ori_lang/v2026/spec/08-types.md`. This section maps the spec surface onto the tag catalog. Every user-writable type in Ori has an exact tag / pool encoding; there is no "unknown" type shape at the language level.

### TL-1 — Primitive Types

The following primitive types SHALL be supported with the tag mapping from TY-5:

| Ori type | Tag | LLVM canonical type (see `CG:TR-1`) |
|----------|-----|-------------------------------------|
| `int` | `Int` | `i64` |
| `float` | `Float` | `f64` |
| `bool` | `Bool` | `i1` |
| `str` | `Str` | `{ i64, i64, ptr }` fat pointer |
| `char` | `Char` | `i32` (Unicode scalar value) |
| `byte` | `Byte` | `i8` |
| `()` / `void` | `Unit` | `i64` sentinel |
| `Never` | `Never` | `i64` sentinel (TK-4) |
| `Duration` | `Duration` | `i64` (nanoseconds) |
| `Size` | `Size` | `i64` (bytes, non-negative) |
| `Ordering` | `Ordering` | `i8` (Less=0, Equal=1, Greater=2) |

Spec: Clause 8.1 (primitive table) plus subclauses 8.1.1 (Never Semantics), 8.1.2 (Duration), 8.1.3 (Size).

### TL-2 — Compound Types

The following compound types SHALL be representable:

| Ori syntax | Tag | Pool encoding |
|------------|-----|---------------|
| `(T1, T2, ...)` | `Tuple` | extra: `[len, T1, T2, ...]` |
| `(T) -> R` | `Function` | extra: `[param_count, param_tys..., return_ty]` |
| `[T]` | `List` | data: `T.raw()` |
| `[T, max N]` | Erased to `Tag::List` with no capacity payload (shipped) | See `PT-2` target-only note — capacity-preserving encoding not yet shipped |
| `{K: V}` | `Map` | extra: `[K, V]` |
| `Set<T>` | `Set` | data: `T.raw()` |
| struct | `Struct` | extra: `[field_count, name_id, field_name_id_1, field_ty_1, ...]` |
| enum | `Enum` | extra: `[variant_count, name_id, variant_name_id_1, variant_payload_1, ...]` |
| trait object | shipped as first-bound placeholder `Idx` | See TL-7 — placeholder representation today; dedicated pool encoding is target-only |

Spec: Clause 8.2.

### TL-3 — Generic Types

Generic parameters SHALL be represented as `RigidVar` inside a surrounding `Scheme` (SC-1). Applied generics (`Option<int>`, `Result<T, Error>`) SHALL be represented as `Applied(named, [args...])` where `Applied`'s `extra` lists the argument `Idx`s in declaration order. Bounds (`T: Eq + Clone`) SHALL be stored in the `TraitRegistry` associated with the scheme's binding site, not in the pool.

Const generics (`$N: int`) SHALL be tracked separately from type parameters. The shipped checker filters const params out of type-parameter collection (`compiler/ori_types/src/check/signatures/mod.rs`, `compiler/ori_types/src/check/registration/type_resolution.rs`) and stores them on `FunctionSig.const_params` as a metadata sidecar. The pool's `Tag::Scheme` layout carries plain type-variable ids only — `[var_count, var_id_1, ..., var_id_N, body_idx]` — with no kind discriminator for const-vs-type. **(Target-only)** A kind-tagged scheme-binder representation would unify type and const generics in one pool layout, but is not yet shipped.

Spec: Clause 8.3.

### TL-4 — Built-in Generic Types

The following spec-defined generic types SHALL be representable. Iterator-family traits (`Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect`) use associated types, not generic parameters — the tag catalog's `Iterator` / `DoubleEndedIterator` entries are *container shapes* carrying a single child element type, distinct from the trait definitions.

| Ori surface | Tag (when used as container) | Notes |
|-------------|------------------------------|-------|
| `Option<T>` | `Option` | Simple container, child = `T` |
| `Result<T, E>` | `Result` | Two-child container |
| `Range<T>` | `Range` | Simple container; `T` is `int` in shipped surface (`Range<float>` does NOT impl `Iterable` per spec 8.13) |
| `impl Iterator` (container shape) | `Iterator` | Opaque iterator handle carrying the element type `T` as a child `Idx`; realises `trait Iterator { type Item; }` with the container's child as `Self.Item` |
| `impl DoubleEndedIterator` | `DoubleEndedIterator` | Subtype of `Iterator`; registered with supertrait-inherited methods |

Iterator trait *definitions* are:

```ori
trait Iterator { type Item; @next (self) -> (Option<Self.Item>, Self); }
trait DoubleEndedIterator: Iterator { @next_back (self) -> (Option<Self.Item>, Self); }
trait Iterable { type Item; @iter (self) -> impl Iterator where Item == Self.Item; }
trait Collect<T> { @from_iter<I: Iterator> (iter: I) -> Self where I.Item == T; }
```

The associated-type model is what the trait registry records (`TYPES:RG-2`, populated by `CHK:CK-1` pass 0c `register_traits`): `Iterator` and `Iterable` register `type Item;`; `Collect<T>` registers a type parameter. `next` returns `(Option<Self.Item>, Self)` — the value and the *new* iterator state (`BI-5`).

Spec: Clauses 8.4, 8.13.

### TL-5 — Channel Types

The shipped pool representation SHALL be a single-child `Tag::Channel` constructed via `Pool::channel(elem)` — one channel type per element type, with no role discriminator or cloneability flag in the pool item.

**(Target-only)** Spec Clause 8.5 mandates that the channel element type `T` satisfy the `Sendable` marker trait (BI-7). The shipped checker does not yet enforce this bound: channel-construction expression forms (`FunctionExpKind::Channel`, `ChannelIn`, `ChannelOut`, `ChannelAll`) are currently rejected as unsupported (`E2040`) before any `Sendable`-bound check runs. When channel construction ships, bound enforcement will occur at the construction site.

**(Target-only)** The spec defines four distinct channel role types in Clause 8.5: `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>`. Role discrimination and cloneability are represented at the Ori trait level (each role type implements a different capability-trait interface), NOT in the pool tag. Distinct role tags or an in-tag discriminator are not yet shipped; until they are, role-specific behavior SHALL be handled by trait dispatch on the container, not by tag inspection.

Spec: Clause 8.5.

### TL-6 — User-Defined Types

The three user-defined shapes SHALL be:

1. **Struct** (`type N = { field: T, ... }`) — Tag `Struct`, nominal identity (TI-5), field order preserved from source.
2. **Sum** (`type N = A | B(T)`) — Tag `Enum`, nominal identity, variant order preserved from source.
3. **Newtype** (`type N = Existing`) — Tag `Named` wrapping the underlying type, distinct identity from the underlying.

The user-writable surface does NOT include a transparent alias — the parser represents every `type N = ...` declaration as one of the three shapes above via `TypeDeclKind::{Struct, Sum, Newtype}` (`compiler/ori_ir/src/ast/items/types.rs`). `Tag::Alias` exists in the tag catalog (`TK-9`) for compiler-internal transparent references (import aliases, well-known-type re-exposures), but is never produced from user-writable `type` declarations.

Spec: Clauses 8.6 (user-defined types, including 8.6.3 newtype), 8.7 (nominal typing).

### TL-7 — Trait Objects

Trait objects at value positions (argument, return, field) are written in source as `ParsedType::TraitBounds` carrying one or more bound trait names. Object-safety enforcement runs in two stages:

1. **Registration (Phase 0c)**: `check/registration/traits.rs` computes and stores an `object_safety_violations` field on every `TraitEntry` when the trait definition is first registered. The three violation conditions (`Self` in return, `Self` in non-receiver parameter, generic method) are checked once and cached per trait.
2. **Use site (Signatures / Bodies)**: at every trait-object use position in signatures (`check/signatures/mod.rs`) and in-body type annotations (`infer/expr/type_resolution.rs`), the type-resolution pass consults the stored `object_safety_violations` and emits `E2024` (not-object-safe) on the use site's span. Type-resolution returns the FIRST bound as a placeholder `Idx` when the check passes; that placeholder is the shipped representation of a trait-object typed position today.

**(Target-only)** Per Spec Clause 8.8, the target pool SHALL distinguish a trait-object type from any concrete type that satisfies the trait. The current pool does not yet have a dedicated trait-object encoding — downstream consumers rely on the use-site `E2024` emission rather than a distinct runtime representation. The target encoding's exact tag shape is not yet specified at the rule-file level.

Spec: Clause 8.8 (trait objects).

### TL-8 — Existential Types

`impl Trait where Assoc == Type` SHALL be representable as a fresh `Projection`-bound type with trait constraints recorded against the opaque identity. The opaque identity SHALL NOT be structurally equal to its underlying concrete type — the point of existential types is to hide that identity from callers. (Target-only: the shipped checker partially normalizes `impl Trait` returns today; the full opaque-identity rule is the target.)

Spec: Clause 8.15.

### TL-9 — Borrowed References (Target-Only)

`Tag::Borrowed` SHALL be reserved for future `&T` / `Slice<T>` support per the low-level future-proofing proposal. The tag exists in the tag catalog (TK-1) but SHALL NOT be constructed by the current surface syntax. Spec references to borrowed values are target-only until the proposal ships.

### TL-10 — Inference and Opacity

Types appearing without a concrete identity at their construction site SHALL be one of:

- `Tag::Infer` — parser-emitted `_` or omitted annotation awaiting resolution (TK-5)
- `Tag::Var` — checker-allocated unification variable (TK-5)
- `Tag::Projection` — associated type awaiting trait resolution (TK-8)

All three SHALL be resolved to concrete types before typed IR leaves the checker (`PC-2`).

Spec: Clause 8.16 (type inference).

---

## §6 Properties of Types

Type-level predicates that the checker consults during unification, trait dispatch, and operator resolution. The authoritative behavior is Spec Clause 9; this section states the compiler-side invariants that realize that behavior.

### PT-1 — Type Identity

Two types SHALL be considered identical iff their `Idx` values are equal (TI-2). Nominal types (TI-5) are never identical to structurally equal non-nominal types. Aliases are identical to their underlying type.

Spec: Clause 9.1.

### PT-2 — Assignability

A value of type `T` is assignable to type `U` iff:

- `T == U` (identical), OR
- `T = Never` (bottom coerces to anything — TK-4), OR
- A user-defined `Into<U>` impl exists on `T` and the conversion is explicit at the call site via `.into()` (the checker SHALL NOT insert implicit `.into()` calls).

The checker SHALL reject all other assignments with `E2001`. Ori SHALL NOT perform implicit numeric widening (`int` to `float`), implicit pointer decay, or implicit trait-object coercion.

**(Target-only)** Spec Clause 8.2.2 specifies a one-way widening from fixed-capacity lists `[V, max N]` to dynamic `[V]`. The shipped checker ERASES `ParsedType::FixedList` to `[V]` during registration / signature / expression type resolution (`compiler/ori_types/src/check/registration/type_resolution.rs`, `compiler/ori_types/src/check/signatures/mod.rs`, `compiler/ori_types/src/infer/expr/type_resolution.rs`), so no capacity-aware subtyping rule is enforced today — both `[V, max N]` and `[V]` hit the pool as the same `[V]` shape. A capacity-preserving pool encoding and the one-way widening assignability rule are target-only until fixed-capacity erasure is lifted.

Spec: Clause 9.2.

### PT-3 — Variance

All Ori type constructors SHALL be **invariant** in their type parameters. `List<Dog>` is NOT a subtype of `List<Animal>` even if `Dog <: Animal` existed; Ori does not have nominal subtyping between user types. The only subtyping in the language is `Never <: T` (bottom).

Rationale: Invariance is sound, simple, and matches the spec. Adding variance requires a proposal.

Spec: Clause 9.3.

### PT-4 — Type Constraints

Trait bounds on type parameters SHALL be stored in the `TraitRegistry` indexed by the binding site. The checker SHALL verify, at instantiation time, that the supplied concrete type satisfies every declared bound (`CHK:TR-4`).

A bound failure SHALL produce `E2001` (type mismatch) or the trait-specific code (e.g., `E2029` for derive-supertrait missing) depending on the context.

Spec: Clause 9.4.

### PT-5 — Default Values

Types satisfying the `Default` trait SHALL have a compiler-queryable default value. The default for builtin types is specified by Spec Clause 9.5; the default for user types SHALL come from a user-provided `Default` impl or a derive.

`Ordering::default() == Equal`, `Duration::default() == 0ns`, `Size::default() == 0b` are examples mandated by the spec; the `ori_registry` holds these defaults for builtin types.

Spec: Clause 9.5.

---

## §7 Builtin Trait Interface

Traits defined in the prelude (Spec Clauses 9.6 through 9.14 and scattered through Clause 8) have a fixed method shape that the type system encodes as registry entries. This section states the canonical signatures and their invariants; the actual dispatch is in `typeck.md`.

### BI-1 — Canonical Trait Methods

The following traits SHALL have the canonical method shapes registered in `ori_registry` and cross-checked by `ori_types::check::registration`:

| Trait | Method | Signature | Spec |
|-------|--------|-----------|------|
| `Eq` | `equals` | `(self, other: Self) -> bool` | Operator-rules §==/!= |
| `Comparable: Eq` | `compare` | `(self, other: Self) -> Ordering` | 9.12 |
| `Hashable: Eq` | `hash` | `(self) -> int` | 9.13 |
| `Printable` | `to_str` | `(self) -> str` | 9.6 |
| `Formattable: Printable` | `format` | `(self, spec: FormatSpec) -> str` | 9.7 |
| `Debug` | `debug` | `(self) -> str` | 8.12 |
| `Clone` | `clone` | `(self) -> Self` | 8.9 |
| `Default` | `default` | `() -> Self` | 9.8 |
| `Drop` | `drop` | `(self) -> void` | 8.10 |
| `Len` | `len` | `(self) -> int` | 9.10 |
| `IsEmpty` | `is_empty` | `(self) -> bool` | 9.11 |
| `Iterator` | `next` | `(self) -> (Option<Self.Item>, Self)` (associated type `Item`) | 8.13 |
| `DoubleEndedIterator: Iterator` | `next_back` | `(self) -> (Option<Self.Item>, Self)` | 8.13 |
| `Iterable` | `iter` | `(self) -> impl Iterator where Item == Self.Item` (associated type `Item`) | 8.13 |
| `Collect<T>` | `from_iter` | `<I: Iterator> (iter: I) -> Self where I.Item == T` | 8.13 |
| `Into<T>` | `into` | `(self) -> T` | 8.11 |
| `As<T>` **(target-only)** | `as` | `(self) -> T` — desugar target for infallible `e as T` casts | 8.11 |
| `TryAs<T>` **(target-only)** | `try_as` | `(self) -> Option<T>` — desugar target for fallible `e as? T` casts | 8.11 |
| `Traceable` | `with_trace`, `trace`, `trace_entries`, `has_trace` | `(self, entry: TraceEntry) -> Self`, `(self) -> str`, `(self) -> [TraceEntry]`, `(self) -> bool` | 9.9 |
| `Sendable` | *(marker — no methods)* | — | 8.14 |
| `Value: Clone, Eq` | *(marker — no methods)* | — | 8.14 |

Deviations from the canonical signature SHALL be rejected with `E2001` or `E2010` at registration time.

### BI-2 — Derive Sync Points

The derivable traits (`Eq`, `Clone`, `Debug`, `Printable`, `Default`, `Comparable`, `Hashable`) SHALL be enumerated in a single canonical list in `ori_ir::DerivedTrait`. Every consumer crate (`ori_types`, `ori_eval`, `ori_llvm`, `library/std`) SHALL iterate that list to drive registration/evaluation/codegen. Parallel hand-maintained lists SHALL be a DRIFT finding per `HYG:§Registration Sync Points`.

Cross-reference: `ir.md` §DerivedTrait holds the canonical checklist. Full derive workflow is in CLAUDE.md §"Adding a New Derived Trait".

### BI-3 — Derivable Traits

The following traits SHALL be derivable on user-defined types via `#derive(Trait)` (pre-proposal syntax) or `type T: Trait = { ... }` (post-proposal syntax): `Eq`, `Clone`, `Debug`, `Printable`, `Default`, `Comparable`, `Hashable`. A derive is generated at registration time (`CHK:TR-8`) with canonical componentwise semantics. A user `impl` overrides the derived form.

Non-derivable: traits whose method bodies depend on user intent (`Drop`, `Iterator`, `Into<T>`, `Iterable`). Derive of a non-derivable trait produces `E2033`.

Spec: Clauses 8.9 (Clone — 8.9.2 Derivable), 8.12 (Debug — 8.12.2 Derivable), 9.6 (Printable), 9.8 (Default), 9.12 (Comparable), 9.13 (Hashable).

Rationale: Derivation is opt-in via `#derive` (or the post-proposal `type T: Trait` form). There is no structural auto-derivation that fires without a declaration — an Ori type declared as a bare `type T = { ... }` does NOT silently acquire `Eq`/`Clone`/`Debug`/`Printable` impls.

### BI-4 — Operator Traits

Arithmetic (`+`, `-`, `*`, `/`, `**`, `%`, `div`), bitwise (`&`, `|`, `^`, `<<`, `>>`, `~`), comparison (`<`, `<=`, `>`, `>=`), equality (`==`, `!=`), unary (`-`, `!`, `~`), matmul (`@`), and conversion (`as`, `as?`) operators SHALL desugar to canonical trait methods as enumerated in `spec/operator-rules.md`. The registry holds the trait-to-method mapping; the checker consults it rather than hardcoding operator knowledge.

Rationale: Operator-to-method mapping is language-wide knowledge; encoding it in the checker per-operator would be a LEAK (scattered knowledge).

Spec: `spec/operator-rules.md`.

### BI-5 — Iterator Trait Shape

`Iterator::next` SHALL return `(Option<Self.Item>, Self)` — a tuple of the next element (None on exhaustion) and the *new* iterator state. Ori iterators are **fused** (after the first `None`, all subsequent `next` calls return `None`) and **value-returning** (the iterator value itself is consumed and produced fresh, not mutated in place).

Rationale: Value-returning iterators are sound under ARC — no aliased mutation hazard. See `aims-rules.md` §1 for how the checker's iterator facts feed into AIMS.

Spec: Clause 8.13.

### BI-6 — Object Safety

A trait SHALL be *object-safe* iff all methods satisfy:

- No `Self` in return type (except as receiver)
- No `Self` in non-receiver parameter
- No generic type parameter on the method

Object safety SHALL be checked at trait-registration time (`CHK:TR-6`). Non-object-safe traits (`Clone`, `Eq`, `Iterator`, `Comparable`, `Hashable`) SHALL NOT be usable at trait-object positions (`E2024`). `Into<T>` satisfies all three rules (`into(self) -> T` — `self` receiver, `T` return is not `Self`, no method-level generic parameters) and IS object-safe.

Rationale: Object-safe traits correspond to vtable-compatible interfaces. Rust and Swift both enforce the same rules.

Spec: Clause 8.8 (trait objects).

### BI-7 — Sendable and Value Markers

`Sendable` and `Value` SHALL be **compiler-auto-derived** marker traits (Spec Clause 8.14). Users SHALL NOT impl them manually. The checker derives:

- `Sendable` iff all fields are `Sendable` AND the type has no interior mutability AND captures no non-`Sendable` values.
- `Value` iff all fields are `Value` AND the type is ≤ 512 bytes (warn at > 256 bytes) AND has no `Drop` impl AND is non-recursive (Spec Clause 8.14.2).

`E2033` ("trait not derivable") SHALL be emitted by the derive-registration path when a user attempts to derive `Sendable` or `Value` explicitly via `#derive(Sendable)` (pre-proposal) or `type T: Sendable = { ... }` (post-proposal). **(Target-only)** Rejection of bare `impl T: Sendable { ... }` (i.e., a manual hand-written impl) is specified but not yet enforced by the shipped checker — today the derive path is where `E2033` surfaces.

Spec: Clause 8.14 (Sendable / Value — marker semantics, forbidden manual impls).

---

## §8 Type Schemes & Quantification

Polymorphic types are represented by `Tag::Scheme` wrapping a body with explicitly bound rigid variables. The scheme is the only polymorphic shape in the pool; every non-generic type is a scheme-free monotype.

Source: `ori_types/src/unify/generalization.rs`, `ori_types/src/unify/rank/`, `ori_types/src/pool/substitute/`.

### SC-1 — Scheme Layout

A scheme SHALL be `Tag::Scheme` with extra `[var_count, var_id_1, ..., var_id_N, body_idx]`. The body SHALL reference the bound variables via `Tag::BoundVar` with `data = var_id` matching one of the scheme's declared var ids.

Free variables in the body (not declared by this scheme) SHALL be `Tag::Var` (unbound in the enclosing scope) or `Tag::RigidVar` (user-annotated from an outer binder).

Rationale: Explicit bound-var ids allow substitution to rename without risking capture.

### SC-2 — Rank-Based Generalization

Every unification variable SHALL carry a rank indicating the nesting depth at which it was introduced. A variable SHALL be generalizable at scope exit iff its rank is strictly greater than the current outer rank AND it appears in the type being generalized AND it does not escape via the enclosing environment.

Under-generalization (ranks too low) produces monomorphic inferred signatures where polymorphism was intended. Over-generalization (ranks too high) is unsoundness — a skolem escaping its binder.

Source: `ori_types/src/unify/rank/`. Cross-reference: `CHK:GN-1`.

Rationale: Rank-based generalization is standard HM. The scope entry/exit points SHALL push and pop ranks; see `CHK:CK-2`.

### SC-3 — Substitution

Substituting a scheme `∀α. body` at a use site SHALL:

1. Allocate one fresh `Var` per bound variable.
2. Replace every `BoundVar(α_i)` in the body with the corresponding fresh `Var`.
3. Preserve `RigidVar` nodes — they belong to an outer binder and SHALL NOT be substituted.

Substitution walks SHALL be gated by `TF-5` (skip when `!NEEDS_SUBST`).

### SC-4 — Occurs Check

Unification of a `Var` with a type that transitively contains the same `Var` SHALL fail with `E2008` (infinite type). The occurs check SHALL be gated by `TF-5` (skip when the candidate type has `!HAS_VAR`). Path compression during unification SHALL NOT bypass the occurs check.

Rationale: Without the occurs check, the unification engine builds a cyclic type that crashes later phases. This is a correctness invariant, not a performance hint.

---

## §9 Registries

Three registries hold the crate's name-indexed type knowledge. They are the SSOT homes for "what types exist", "what traits exist", and "what methods are callable on what". Consumers SHALL query the registries rather than rediscover the data.

Source: `ori_types/src/registry/`.

### RG-1 — TypeRegistry

`TypeRegistry` SHALL store user-defined nominal types (struct / sum / newtype / alias) indexed by `Name`. Entries record:

- Source location (span)
- Parameter list (rigid vars + bounds)
- Body shape (field list for struct; variant list for sum; target type for newtype/alias)
- Visibility
- Attributes (`#repr`, `#derive`, etc.)

The registry SHALL be populated during the Registration-group passes (0a–0e) of `check_module_impl` (`CHK:CK-1`). It SHALL be frozen at Signatures-pass entry — later passes query, never mutate.

Rationale: Freezing after registration prevents later passes (signature collection, body checking) from accidentally introducing new nominal identities via side effects. See `CHK:CK-1` for the full pass order.

### RG-2 — TraitRegistry

`TraitRegistry` SHALL store trait definitions and trait implementations. Trait entries record:

- Method signatures (names, types, default bodies)
- Associated types
- Supertrait constraints
- Object safety (BI-6)

Impl entries record:

- The `(trait, impl_type)` pair
- Method implementations (override or default-inherited)
- Generic parameters and bounds
- Coherence metadata (orphan status, overlap group)

Coherence SHALL be checked at registration time (`CHK:TR-5`). Two impls with identical `(trait, impl_type)` keys (duplicates) produce `E2010` (duplicate implementation). Two distinct impls whose domains overlap without specificity ranking — blanket vs specific — produce `E2021` (overlapping implementations).

### RG-3 — Method Lookup Partition

Method lookup in `ori_types` SHALL be partitioned across two distinct resolution paths, each with a single canonical entry point:

1. **Builtin method resolution** — `resolve_builtin_method()` in `infer/expr/methods/` consults `ori_registry::BUILTIN_TYPES` (pure-data crate; see `.claude/rules/registry.md`). This path answers "does this primitive / built-in container have a method named `m`?" and produces a method descriptor in `ori_registry`'s vocabulary (`MethodDef`).
2. **User-defined method resolution** — `TraitRegistry::lookup_method()` in `registry/traits/mod.rs` answers "does `receiver_ty` have an `impl` or trait-impl entry for `m`?". The registry SHALL check *inherent* impls first, then trait impls; ambiguous trait-impl matches produce `E2023`.

`MethodRegistry` is a thin wrapper reserved for a future unified entry point. In the shipped surface, `MethodRegistry::lookup_trait_method()` delegates directly to `TraitRegistry::lookup_method()`; builtin lookup is NOT routed through `MethodRegistry`.

A call `receiver.method(args)` SHALL dispatch in this order at the checker call-site (`CHK:TR-1`): builtin-first via `resolve_builtin_method()`, then user-defined via the trait registry (inherent-then-trait). The aggregate order — builtin → inherent → trait — SHALL be consistent across every call site; diverging orderings are a LEAK per `HYG:§Side Logic`.

The registry SHALL expose method entries with stable, alphabetically-ordered method lists per type (the `registry_methods_sorted_per_type` test enforces this for `ori_registry`).

**(Target-only)** `MethodRegistry` is reserved for a future unified-dispatch implementation that would integrate builtin + inherent + trait lookup behind one entry point. Until that ships, the two paths remain distinct.

### RG-4 — Registry as SSOT for Builtin Behavior

Knowledge about builtin type behavior (method presence, method signatures, trait impls for primitives, operator desugaring) SHALL live exclusively in `ori_registry` (the pure-data crate) and be read via `find_type(TypeTag)`, `find_method(TypeTag, name)`, and `OpDefs`. Any consumer (`ori_types`, `ori_eval`, `ori_llvm`) that hardcodes knowledge like `if type == str { special_case }` outside `ori_registry` is a LEAK per `HYG:§Side Logic`.

There is no extension-dispatch path in the shipped `ori_types` method-resolution surface. Spec-level `extend T { @m }` extensions are a language feature; their registration in the trait / method registries has not yet been threaded through `ori_types::registry`, so method-resolution `CHK:TR-9` rules about extension dispatch are **(target-only)** until the registration pass ships.

Cross-reference: `.claude/rules/registry.md` for the full `ori_registry` data-model surface.

---

## §10 Phase Contracts

The type system sits between parsing and everything downstream. Every downstream phase (eval, ARC, codegen, diagnostics) relies on invariants that `ori_types` guarantees on its output. Those invariants SHALL be explicit and validated.

### PC-1 — Input Contract (from Parser)

The checker's input SHALL be an AST with:

- Every type reference carrying a `TypeId` — either pre-interned (primitives) or pending resolution (Named)
- Error nodes marked (the checker skips them; see `HYG:§Error Recovery Monotonicity`)
- `TypeId::INFER` for user-elided annotations (`let x = ...`)

The checker SHALL NOT re-parse source text. Parsing is complete before type checking begins.

### PC-2 — Output Contract (to Eval / ARC / Codegen)

On successful type check, the typed IR SHALL satisfy:

1. **No `Tag::Var`** in any type-bearing IR position. Unification variables are internal to the checker; they SHALL be resolved before emission.
2. **No `Tag::Infer`** — AST placeholders are eliminated during entry (`CHK:CK-3`).
3. **No `Tag::Projection`** — associated types are normalized.
4. **No `Tag::SelfType`** — `Self` is substituted with the implementing type at every use site.
5. **No `Tag::Named` awaiting resolution** — every `Named` reference resolves to a registered `TypeRegistry` entry or is rejected with `E2003`.

Violation of any clause above is a phase contract bug (`HYG:§Cross-Phase Invariant Contracts`). Consumers SHALL `debug_assert!` on entry; release builds SHALL surface a clear internal compiler error rather than emit wrong code.

### PC-3 — Error-Typed Output

If the check fails, the checker SHALL still produce a typed IR with `Tag::Error` filling positions that could not be resolved. Downstream phases SHALL skip error-typed nodes silently (no cascading diagnostics). `Tag::Error` in the output is NOT a contract violation — it is the documented error-recovery carrier.

### PC-4 — Stable Idx Across Phases

An `Idx` emitted by the checker SHALL be interpretable by any downstream consumer holding a reference to the same `Pool`. Pool identity SHALL NOT change between checker exit and codegen entry (TY-6).

---

## §11 Salsa & Caching

The type pool and its queries participate in Salsa's incremental computation model. Determinism and input hygiene are non-negotiable.

### SL-1 — Query Purity

Every `#[salsa::tracked]` query returning pool-dependent data SHALL be pure: same inputs ⇒ same outputs. No global mutable state, no thread locals, no clock reads, no filesystem side effects.

Cross-reference: `HYG:§Salsa & Caching`.

### SL-2 — Idx Stability Across Revisions

An `Idx` from a prior revision SHALL remain valid as a pool key so long as the pool itself is the same Salsa input. If the source input changes such that the pool is rebuilt, old `Idx` values SHALL NOT be read — they key into a stale pool.

Salsa memoization handles the invalidation: tracked queries that take `Idx` as a key are automatically invalidated when the pool input changes.

### SL-3 — Hashable, Eq, Clone

All types exposed as Salsa query outputs SHALL derive `Clone`, `Eq`, `PartialEq`, `Hash`, `Debug`. `Idx`, `TypeFlags`, `Tag`, `Pool`, and all public pool-derived types SHALL satisfy this.

No `Arc<Mutex<T>>`, no `fn` pointers, no `dyn Trait` in Salsa output types.

### SL-4 — Error Accumulation

Type errors SHALL be accumulated via Salsa's diagnostic accumulator, not returned through `Result`. Every pass of `check_module_impl` (`CHK:CK-1`) appends to the accumulator; the driver collects them via `finish()` / `finish_with_pool()`.

Rationale: Accumulation preserves error-recovery behavior — a single mistake does not abort the whole module check.

---

## §12 Diagnostics (Pool-Level)

Diagnostics emitted by the pool itself (as opposed to the checker) are rare but not empty. The small set of pool-specific diagnostics:

### DI-1 — Hash Invariant Violation

`E2030` — a user-supplied `Hashable` impl was observed to produce unequal hashes for equal values. This is detectable only at runtime today and is flagged as warning-severity. (Target-only: a static lint may catch common shapes in the future.)

### DI-2 — Non-Hashable Map Key

`E2031` — a map literal with a key type that does not satisfy `Hashable`. The pool computes `Hashable` impl availability from the registry (RG-2); the checker emits the diagnostic at the map-literal use site.

### DI-3 — Field Missing Trait in Derive

`E2032` — a `#derive(Trait)` (pre-proposal syntax) or `type T: Trait = {...}` (post-proposal syntax) where one field lacks `Trait`. The pool sees the field types; the derive-validator iterates and emits the diagnostic.

### DI-4 — Trait Not Derivable

`E2033` — the derive-registration path rejected an attempted derive. Fires when the user writes `#derive(Trait)` or `type T: Trait = { ... }` for a trait that is not derivable — this includes marker traits (`Sendable`, `Value`) and traits whose bodies depend on user intent (`Iterator`, `Into`, `Drop`, `Iterable`). **(Target-only)** Rejection of bare manual `impl Type: Sendable { ... }` forms is specified but not yet emitted from a separate impl-registration path.

Additional diagnostics produced by the checker live in `typeck.md §Diagnostics` with the full `E2001..E2041` catalog. This section covers only the subset whose root cause is a pool-level query.

---

## §13 Tracing

Source: `compiler/ori_types/src/lib.rs` (target registration), `compiler/oric/src/tracing_setup.rs` (initialization).

### TRG-1 — Tracing Target

Pool and checker events SHALL trace under target `ori_types`. Consumers enable:

- `ORI_LOG=ori_types=debug` — phase boundaries, type errors, registration events
- `ORI_LOG=ori_types=trace ORI_LOG_TREE=1` — per-expression inference call tree (hierarchical)

### TRG-2 — Phase Dump

`ORI_DUMP_AFTER_TYPECK=1` SHALL dump the typed IR to stderr after the checker exits, for debugging. The dump SHALL NOT leak type variables (PC-2) — if `Tag::Var` appears in the dump, the check silently miscompiled.

Cross-reference: `compiler.md §Phase Dumps` for the complete list.

---

## §14 Prior Art Cross-Reference

| System | Relevant Pattern | Ori Correspondence |
|--------|-----------------|-------------------|
| **Rust `rustc_type_ir`** | `Ty<'tcx>` interned in `TyCtxt` with pre-computed `TypeFlags` | `Idx` + `Pool` + `TypeFlags` (TY-1, TF-1) |
| **Rust `rustc_type_ir::INT_TY`** | Pre-interned primitives | Pre-interned `Idx(0..12)` (TY-5) |
| **Zig `InternPool`** | Column-oriented interner with fixed-width items | `items` / `extra` / `flags` columns (TY-2) |
| **Swift `CanType`** | Canonicalized type handle, O(1) equality | `Idx` with structural interning (TI-1, TI-2) |
| **TypeScript `Type` interface** | `flags: TypeFlags` bitset on every type | `TypeFlags` on every pool entry (TF-1) |
| **Roc `Symbol = (ModuleId, IdentId)`** | Packed symbol for O(1) equality | Aspirational — `HYG:§Aspirational Patterns` |
| **Gleam `Type`** | Enum-based type representation, structural HM | `Tag` discriminated union (TK-1) |
| **Koka type rep.** | Effect-typed functions with capability sets | Capabilities tracked on `FunctionSig` metadata (not in `Tag::Function` pool entry) |
| **Lean 4 `InternPool`** | Interned types with Merkle hashing | TI-3 Merkle classification |

### Interface with typeck.md

| typeck.md rule | Uses types.md rule | Interface |
|----------------|--------------------|-----------|
| `CHK:UN-*` (unification) | TI-2, TF-5, SC-4 | Unification compares Idx (TI-2), gates on NEEDS_SUBST / HAS_VAR (TF-5), enforces occurs check (SC-4) |
| `CHK:GN-*` (generalization) | SC-1, SC-2 | Scheme construction follows SC-1 layout, rank rules in SC-2 |
| `CHK:TR-*` (trait resolution) | RG-2, RG-3 | Dispatches through `TraitRegistry`/`MethodRegistry` |
| `CHK:CP-*` (capability checking) | TL-2, TF-1 `HAS_CAPABILITY` | Function types carry capability set; flag propagation tracks presence |
| `CHK:PC-*` (phase contracts) | PC-1, PC-2 | Input/output invariants produced in types.md, consumed in typeck.md |

---

## §15 Key Files

| Path | Role |
|------|------|
| `ori_types/src/pool/mod.rs` | `Pool` struct, interning, public query API |
| `ori_types/src/pool/construct/` | Helpers for constructing compound types with correct hashing |
| `ori_types/src/pool/substitute/` | Scheme substitution (SC-3) |
| `ori_types/src/pool/re_intern/` | Cross-pool migration (TY-6 exception) |
| `ori_types/src/pool/collection_surface/` | Surface type construction (List, Map, etc.) |
| `ori_types/src/pool/format/` | Display/Debug for pool types |
| `ori_types/src/item/` | `Item` 5-byte storage (TY-3) |
| `ori_types/src/idx/` | `Idx` newtype + pre-interned primitive constants (TY-1, TY-5) |
| `ori_types/src/tag/mod.rs` | `Tag` enum, range predicates (TK-1, TK-2) |
| `ori_types/src/flags/mod.rs` | `TypeFlags` bitflags, category dispatch (TF-1 through TF-4) |
| `ori_types/src/registry/types/` | `TypeRegistry` (RG-1) |
| `ori_types/src/registry/traits/` | `TraitRegistry` (RG-2) |
| `ori_types/src/registry/methods/` | `MethodRegistry` (RG-3) |
| `ori_types/src/triviality/` | `IS_COPYABLE` / value-trait classification |
| `ori_types/src/value_category/` | Lvalue / rvalue classification |
| `ori_types/src/lifetime/` | Lifetime scaffolding for `Tag::Borrowed` (target-only, TL-9) |
| `ori_types/src/output/` | Typed IR emission (PC-2) |
| `ori_types/src/lib.rs` | Crate root; tracing target registration |

---

## Appendix A: Pre-Interned Primitive Idx Table

| Idx | Tag | Ori name | Spec reference |
|-----|-----|----------|----------------|
| 0 | `Int` | `int` | Clause 8.1 primitive table |
| 1 | `Float` | `float` | Clause 8.1 primitive table |
| 2 | `Bool` | `bool` | Clause 8.1 primitive table |
| 3 | `Str` | `str` | Clause 8.1 primitive table |
| 4 | `Char` | `char` | Clause 8.1 primitive table |
| 5 | `Byte` | `byte` | Clause 8.1 primitive table |
| 6 | `Unit` | `()` | Clause 8.1 primitive table (`void` alias) |
| 7 | `Never` | `Never` | Clause 8.1.1 Never Semantics |
| 8 | `Error` | `<error>` | Internal (poison) — no spec surface |
| 9 | `Duration` | `Duration` | Clause 8.1.2 Duration |
| 10 | `Size` | `Size` | Clause 8.1.3 Size |
| 11 | `Ordering` | `Ordering` | Clause 9 Ordering (via Comparable — see Clause 9.12) |
| 12..63 | reserved | — | Padded with `Item::primitive(Tag::Error)` per TY-5 |

## Appendix B: Tag-to-Data Decoding Table

| Tag range | Predicate | `data` interpretation |
|-----------|-----------|-----------------------|
| 0–15 | `is_primitive()` | unused (0) |
| 16–31 | `is_container() && has_child_in_data()` | `child_idx.raw()` |
| 32–47 | `uses_extra()`, two-child | `extra_offset` → `[Idx, Idx]` |
| 48–79 | `uses_extra()`, complex | `extra_offset` → length-prefixed payload |
| 80–95 | `uses_extra()`, named | `extra_offset` → `(Name, [args])` |
| 96–111 | `is_type_variable()` | `var_id` |
| 112–127 | `uses_extra()`, scheme | `extra_offset` → `(var_count, vars, body)` |
| 240–255 | special | tag-specific; see TK-7 / TK-8 |

## Appendix C: TypeFlags Propagation Decision Table

For a compound type constructed from children `c_1 .. c_n`:

| Flag group | Propagation rule |
|------------|-----------------|
| Presence (HAS_*) | OR across children AND current tag's own contribution |
| Category (IS_*) | Set by parent tag alone; children do NOT contribute |
| NEEDS_SUBST | OR across children (children with vars → parent needs subst) |
| IS_RESOLVED | AND across children AND current tag is fully concrete |
| IS_MONO | AND across children AND parent is not a scheme |
| IS_COPYABLE | AND across children AND parent obeys `Value` (BI-7) |
| Capability (HAS_*) | OR across function-type children (non-function children contribute nothing) |
