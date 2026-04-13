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
- Rules are numbered `CATEGORY-N`. Categories: `TY` (type pool / storage), `TK` (type kinds / tag catalog), `TI` (type identity / interning / hashing), `TF` (type flags), `TL` (type surface per spec), `PT` (properties of types), `TR` (trait interface — builtin traits), `SC` (scheme / quantification), `RG` (registries), `PC` (phase contracts), `SL` (Salsa / caching), `DI` (diagnostics produced by type storage itself)
- Cross-references: `typeck.md` rules prefixed with `CHK:` (e.g., `CHK:UN-1`), `parse.md` with `PARSE:`, `aims-rules.md` with `AIMS:`, `codegen-rules.md` with `CG:`, `impl-hygiene.md` with `HYG:`, spec clauses with `Spec:` (e.g., `Spec: Clause 8.1`)

---

## §1 Pool Architecture

The type pool is the single source of truth for type structure in the compiler. Every type that participates in checking, evaluation, ARC, or codegen exists as exactly one `Idx` in exactly one pool. The pool is append-only during a module's checking session; `Idx` values never move and are stable for the life of the pool.

Source: `ori_types/src/pool/`.

### TY-1 — Idx as the Canonical Type Handle

`Idx(u32)` SHALL be the canonical representation of a type in the compiler outside the source AST. Consumers SHALL NOT construct types as trees, `Box<Type>` graphs, or strings. Every type reaches consumers as an `Idx` to be queried against a pool.

Rationale: A single 32-bit handle makes type equality O(1) (integer compare), enables hash-keyed caching (`Idx` hashes in one instruction), and eliminates recursive type traversal at use sites. This is the Rust `Ty<'tcx>` / Zig `InternPool.Index` / Swift `CanType` pattern applied to Ori.

### TY-2 — Pool Storage Layout

The pool SHALL store types in parallel column arrays keyed by `Idx.raw() as usize`. The columns:

| Column | Type | Purpose |
|--------|------|---------|
| `items` | `Vec<Item>` | (tag, data) pair — 5 bytes per entry |
| `extra` | `Vec<u32>` | Variable-length data for types that need more than 4 bytes |
| `flags` | `Vec<TypeFlags>` | Pre-computed metadata (TF-1) |
| `hashes` | `Vec<u64>` | Structural Merkle hash for deduplication |
| `interner` | `HashMap<u64, Idx>` | Hash → Idx map for O(1) re-interning |

The `items` column is the primary index. All other columns are parallel: `items.len() == flags.len() == hashes.len()` SHALL hold as a pool invariant.

Rationale: Column storage beats struct-of-arrays for cache behavior on the common access pattern (tag-dispatch then optional data read). Items at 5 bytes keeps the primary column dense.

### TY-3 — Item Representation

`Item` SHALL be exactly 5 bytes: 1 byte `Tag` + 4 bytes `data: u32`. The interpretation of `data` is tag-dependent, documented per-tag in `TK-1`. Adding a new variant SHALL NOT grow `Item` beyond 5 bytes without updating this rule.

A compile-time size assertion (`const _: () = assert!(size_of::<Item>() == 5);`) SHALL guard this invariant.

Rationale: 5-byte items fit 12+ per cache line. Widening the item doubles pool memory for every interned type.

### TY-4 — Extra Array Discipline

Tags whose payload exceeds 4 bytes (two-child containers, complex types, named types, schemes, projections) SHALL store `data = extra_offset` where `extra[offset..offset+len]` contains the payload. The payload length is:

- **Two-child tags** (Map, Result, Borrowed): 2 consecutive `Idx` values
- **Complex tags** (Function, Tuple, Struct, Enum): length prefix followed by child `Idx`s and/or extra fields per-tag
- **Named tags** (Named, Applied, Alias): `Name` + generic args
- **Scheme** (Scheme): bound-var count + body `Idx`
- **Projection**: receiver `Idx` + `Name` of associated type

`Tag::uses_extra()` SHALL return `true` iff the tag reads from `extra`.

Rationale: Uniform 5-byte items with variable-length extra keeps the hot column dense and pushes variable cost to cold paths.

### TY-5 — Pre-Interned Primitives

Primitive types SHALL occupy `Idx(0)` through `Idx(11)` in every pool, in this fixed order:

| Idx | Type | Tag |
|-----|------|-----|
| 0 | `int` | `Tag::Int` |
| 1 | `float` | `Tag::Float` |
| 2 | `bool` | `Tag::Bool` |
| 3 | `str` | `Tag::Str` |
| 4 | `char` | `Tag::Char` |
| 5 | `byte` | `Tag::Byte` |
| 6 | `()` (unit) | `Tag::Unit` |
| 7 | `Never` | `Tag::Never` |
| 8 | `<error>` | `Tag::Error` |
| 9 | `Duration` | `Tag::Duration` |
| 10 | `Size` | `Tag::Size` |
| 11 | `Ordering` | `Tag::Ordering` |

Pre-interning SHALL happen during `Pool::new()`. Pool users SHALL NOT construct primitive `Idx` values manually; they SHALL use the named constants (`Idx::INT`, `Idx::FLOAT`, …) or `Pool::primitive(tag)`.

Rationale: Fixed indices allow `TypeId` (parser-level) and `Idx` (pool-level) to be identical for primitives, eliminating a lookup step on every primitive type reference. Tags 12–15 are reserved for future primitives.

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

Spec: Clause 8.1.3 (Never type), Clause 8.2 (compound types — no-field rule).

### TK-5 — Infer vs Var

`Tag::Infer` SHALL represent a placeholder awaiting type resolution in the AST (producer: parser). `Tag::Var` SHALL represent an active unification variable allocated by `InferEngine::fresh_var()` (producer: type checker). `Tag::Infer` SHALL NOT reach the pool used by unification; the checker converts each AST `Infer` into a fresh `Var` during entry (`CHK:CK-3`).

Rationale: Two separate tags make the producer explicit — `Infer` is a syntactic marker, `Var` is a semantic hole. Mixing them loses that provenance.

### TK-6 — Rigid Variables

`Tag::RigidVar` SHALL represent a user-annotated generic parameter (e.g., `@f<T>`). Rigid variables SHALL NOT unify with concrete types; attempting to unify a `RigidVar` with a non-variable type produces `E2001` (type mismatch) with the rigid name in the diagnostic.

Rationale: Rigid variables preserve parametricity — `@f<T> (x: T) -> T` must typecheck as identity, not as any specific `T`. The rigid tag makes the "can't narrow" property a tag-level invariant, not an annotation-tracking side table.

Spec: Clause 8.3 (generic types).

### TK-7 — Self Type Tag

`Tag::SelfType` SHALL represent the `Self` reference inside `trait` or `impl` blocks. The checker substitutes `SelfType` with the current implementing type at each use site. `SelfType` reaching codegen is a phase contract violation (`PC-2`).

Spec: Clause 8.8 (trait objects), Clause 8.6 (user-defined types).

### TK-8 — Projection for Associated Types

`Tag::Projection` SHALL represent an unresolved associated type reference `T.Item` where `T: Iterable`. Extra stores `[receiver_idx, name_id]`. Projections are normalized by the checker when the receiver resolves to a concrete type that implements the trait; an unresolved projection at codegen time is a phase contract violation (`PC-2`).

Spec: Clause 8.13 (Iterator traits), Clause 8.6.3 (user-defined traits).

### TK-9 — Alias and Applied

- `Tag::Alias` SHALL represent a transparent type alias `type N = Existing`. Aliases SHALL NOT introduce a new identity; unification treats `Alias(N)` and the underlying type as equal.
- `Tag::Named` SHALL represent a reference to a user-defined nominal type by name, pre-resolution.
- `Tag::Applied` SHALL represent `Named(args...)` — a nominal type instantiated with generic arguments.

A `Named` that is a newtype (`type UserId = int`) introduces a fresh nominal identity under `TI-5`; an alias does not. The distinction is recorded in the `TypeRegistry`, not in the tag itself.

Spec: Clauses 8.6 (user-defined types), 8.7 (nominal typing).

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
| 24 | `HAS_CAPABILITY` | Capability | Contains a function type that `uses` at least one capability |
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

Spec: Clauses 8.1.1 through 8.1.11.

### TL-2 — Compound Types

The following compound types SHALL be representable:

| Ori syntax | Tag | Pool encoding |
|------------|-----|---------------|
| `(T1, T2, ...)` | `Tuple` | extra: `[len, T1, T2, ...]` |
| `(T) -> R uses Caps` | `Function` | extra: `[arity, param_tys..., return_ty, capability_set]` |
| `[T]` | `List` | data: `T.raw()` |
| `[T, max N]` | `List` + ReprPlan hint (target-only for fixed-capacity fast path) | data: `T.raw()` + capacity in layout |
| `{K: V}` | `Map` | extra: `[K, V]` |
| `Set<T>` | `Set` | data: `T.raw()` |
| struct | `Struct` | extra: `[field_count, name_id, field_name_id_1, field_ty_1, ...]` |
| enum | `Enum` | extra: `[variant_count, name_id, variant_name_id_1, variant_payload_1, ...]` |
| trait object | `Applied` over a `Named` trait | extra: `[trait_name, concrete_args]` |

Spec: Clause 8.2.

### TL-3 — Generic Types

Generic parameters SHALL be represented as `RigidVar` inside a surrounding `Scheme` (SC-1). Applied generics (`Option<int>`, `Result<T, Error>`) SHALL be represented as `Applied(named, [args...])` where `Applied`'s `extra` lists the argument `Idx`s in declaration order. Bounds (`T: Eq + Clone`) SHALL be stored in the `TraitRegistry` associated with the scheme's binding site, not in the pool.

Const generics (`$N: int`) SHALL be represented as bound values carried alongside rigid type vars; the pool representation records the kind (type vs const) per bound var.

Spec: Clause 8.3.

### TL-4 — Built-in Generic Types

The following spec-defined generic types SHALL be representable as prelude types whose trait implementations are registered by `check/registration/derived.rs`:

| Type | Tag | Notes |
|------|-----|-------|
| `Option<T>` | `Option` | Simple container, child = `T` |
| `Result<T, E>` | `Result` | Two-child container |
| `Range<T>` | `Range` | Simple container; `T` is always `int` in shipped surface |
| `Iterator<T>` | `Iterator` | Object-unsafe (TR-6) |
| `DoubleEndedIterator<T>` | `DoubleEndedIterator` | Subtype of `Iterator` in the trait hierarchy |

Spec: Clause 8.4.

### TL-5 — Channel Types

`Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>` SHALL be representable as `Channel`-tagged types, with an extra field discriminating producer/consumer and cloneability. The element type `T` SHALL satisfy the `Sendable` marker trait (TR-7) — the checker enforces this bound at channel construction sites.

Spec: Clause 8.5.

### TL-6 — User-Defined Types

The three user-defined shapes SHALL be:

1. **Struct** (`type N = { field: T, ... }`) — Tag `Struct`, nominal identity (TI-5), field order preserved from source.
2. **Sum** (`type N = A | B(T)`) — Tag `Enum`, nominal identity, variant order preserved from source.
3. **Newtype** (`type N = Existing`) — Tag `Named` wrapping the underlying type, distinct identity from the underlying.

A transparent alias (`type Alias = Existing` registered as an alias, not a newtype) SHALL use `Tag::Alias` and SHALL NOT introduce a new identity.

Spec: Clauses 8.6 (user-defined types), 8.7 (nominal typing).

### TL-7 — Trait Objects

Trait object types SHALL be representable as `Applied(trait_name, concrete_args)` when used at value positions (argument, return, field). Trait objects SHALL be rejected for traits that are not object-safe (TR-6); the check runs at registration time, not at use time.

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
- `T = [V, max N]` and `U = [V]` (fixed-capacity widens to dynamic), OR
- A user-defined `Into<U>` impl exists on `T` and the conversion is explicit at the call site (the checker SHALL NOT insert implicit `.into()` calls).

The checker SHALL reject all other assignments with `E2001`. Ori SHALL NOT perform implicit numeric widening (`int` to `float`), implicit pointer decay, or implicit trait-object coercion.

Spec: Clause 9.2.

### PT-3 — Variance

All Ori type constructors SHALL be **invariant** in their type parameters. `List<Dog>` is NOT a subtype of `List<Animal>` even if `Dog <: Animal` existed; Ori does not have nominal subtyping between user types. The only subtyping in the language is `Never <: T` (bottom).

Rationale: Invariance is sound, simple, and matches the spec. Adding variance requires a proposal.

Spec: Clause 9.3.

### PT-4 — Type Constraints

Trait bounds on type parameters SHALL be stored in the `TraitRegistry` indexed by the binding site. The checker SHALL verify, at instantiation time, that the supplied concrete type satisfies every declared bound (`CHK:TR-3`).

A bound failure SHALL produce `E2001` (type mismatch) or the trait-specific code (e.g., `E2029` for derive-supertrait missing) depending on the context.

Spec: Clause 9.4.

### PT-5 — Default Values

Types satisfying the `Default` trait SHALL have a compiler-queryable default value. The default for builtin types is specified by Spec Clause 9.5; the default for user types SHALL come from a user-provided `Default` impl or a derive.

`Ordering::default() == Equal`, `Duration::default() == 0ns`, `Size::default() == 0b` are examples mandated by the spec; the `ori_registry` holds these defaults for builtin types.

Spec: Clause 9.5.

---

## §7 Builtin Trait Interface

Traits defined in the prelude (Spec Clauses 9.6 through 9.14 and scattered through Clause 8) have a fixed method shape that the type system encodes as registry entries. This section states the canonical signatures and their invariants; the actual dispatch is in `typeck.md`.

### TR-1 — Canonical Trait Methods

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
| `Iterator` | `next` | `(self) -> (Option<Item>, Self)` | 8.13 |
| `DoubleEndedIterator: Iterator` | `next_back` | `(self) -> (Option<Item>, Self)` | 8.13 |
| `Iterable` | `iter` | `(self) -> impl Iterator` | 8.13 |
| `Into<T>` | `into` | `(self) -> T` | 8.11 |
| `Sendable` | *(marker — no methods)* | — | 8.14 |
| `Value: Clone, Eq` | *(marker — no methods)* | — | 8.14 |

Deviations from the canonical signature SHALL be rejected with `E2001` or `E2010` at registration time.

### TR-2 — Derive Sync Points

The derivable traits (`Eq`, `Clone`, `Debug`, `Printable`, `Default`, `Comparable`, `Hashable`) SHALL be enumerated in a single canonical list in `ori_ir::DerivedTrait`. Every consumer crate (`ori_types`, `ori_eval`, `ori_llvm`, `library/std`) SHALL iterate that list to drive registration/evaluation/codegen. Parallel hand-maintained lists SHALL be a DRIFT finding per `HYG:§Registration Sync Points`.

Cross-reference: `ir.md` §DerivedTrait holds the canonical checklist. Full derive workflow is in CLAUDE.md §"Adding a New Derived Trait".

### TR-3 — Structural Defaults

`Eq`, `Clone`, `Debug`, `Printable` SHALL have structural default implementations that fire without a declaration on `type T = { ... }`. A user declaration SHALL override the structural default. `Comparable`, `Hashable`, `Default` SHALL require explicit declaration — there is no structural default for ordering, hashing, or default values.

Rationale: Structural equality / cloning / formatting is unambiguous; ordering and hashing are not (Swift and Rust both require opt-in for the same reason).

Spec: Clauses 8.9 (Clone), 8.12 (Debug), 9.6 (Printable), 9.12 (Comparable), 9.13 (Hashable).

### TR-4 — Operator Traits

Arithmetic (`+`, `-`, `*`, `/`, `**`, `%`, `div`), bitwise (`&`, `|`, `^`, `<<`, `>>`, `~`), comparison (`<`, `<=`, `>`, `>=`), equality (`==`, `!=`), unary (`-`, `!`, `~`), matmul (`@`), and conversion (`as`, `as?`) operators SHALL desugar to canonical trait methods as enumerated in `spec/operator-rules.md`. The registry holds the trait-to-method mapping; the checker consults it rather than hardcoding operator knowledge.

Rationale: Operator-to-method mapping is language-wide knowledge; encoding it in the checker per-operator would be a LEAK (scattered knowledge).

Spec: `spec/operator-rules.md`.

### TR-5 — Iterator Trait Shape

`Iterator::next` SHALL return `(Option<Self.Item>, Self)` — a tuple of the next element (None on exhaustion) and the *new* iterator state. Ori iterators are **fused** (after the first `None`, all subsequent `next` calls return `None`) and **value-returning** (the iterator value itself is consumed and produced fresh, not mutated in place).

Rationale: Value-returning iterators are sound under ARC — no aliased mutation hazard. See `aims-rules.md` §1 for how the checker's iterator facts feed into AIMS.

Spec: Clause 8.13.

### TR-6 — Object Safety

A trait SHALL be *object-safe* iff all methods satisfy:

- No `Self` in return type (except as receiver)
- No `Self` in non-receiver parameter
- No generic type parameter on the method

Object safety SHALL be checked at trait-registration time (`CHK:TR-6`). Non-object-safe traits (`Clone`, `Eq`, `Iterator`, `Comparable`, `Hashable`, `Into`) SHALL NOT be usable at trait-object positions (`E2024`).

Rationale: Object-safe traits correspond to vtable-compatible interfaces. Rust and Swift both enforce the same rules.

Spec: Clause 8.8 (trait objects).

### TR-7 — Sendable and Value Markers

`Sendable` and `Value` SHALL be **compiler-auto-derived** marker traits. Users SHALL NOT impl them manually. The checker derives:

- `Sendable` iff all fields are `Sendable` AND the type has no interior mutability AND captures no non-`Sendable` values
- `Value` iff all fields are `Value` AND the type is ≤ 512 bytes (warn at > 256 bytes)

A manual `impl` of `Sendable` or `Value` SHALL be rejected with `E2033` ("trait not derivable").

Spec: Clause 8.14.

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

The registry SHALL be populated by `check/registration/user_types.rs` during Phase 1 of `check_module` (`CHK:CK-1`). It SHALL be frozen at Phase 2 entry — later phases query, never mutate.

Rationale: Freezing after registration prevents Phase 3 signature-check or Phase 4 body-check from accidentally introducing new nominal identities via side effects.

### RG-2 — TraitRegistry

`TraitRegistry` SHALL store trait definitions and trait implementations. Trait entries record:

- Method signatures (names, types, default bodies)
- Associated types
- Supertrait constraints
- Object safety (TR-6)

Impl entries record:

- The `(trait, impl_type)` pair
- Method implementations (override or default-inherited)
- Generic parameters and bounds
- Coherence metadata (orphan status, overlap group)

Coherence SHALL be checked at registration time (`CHK:TR-5`). Two non-overlapping impls for the same `(trait, impl_type)` SHALL produce `E2010` (conflicting impl / coherence violation).

### RG-3 — MethodRegistry

`MethodRegistry` SHALL provide unified method lookup across three dispatch tiers, in priority order:

1. **Builtin methods** — registered from `ori_registry` at module startup (e.g., `str.len`, `int.to_str`).
2. **Inherent impls** — methods defined in `impl Type { ... }` blocks.
3. **Trait impls** — methods from `impl Type: Trait { ... }` blocks.

A call `receiver.method(args)` SHALL resolve to the first matching entry in priority order. Ambiguity between trait impls produces `E2023` (ambiguous method); a user disambiguates with qualified syntax (`Trait.method(receiver, ...)`).

The registry SHALL expose method entries with stable, alphabetically-ordered method lists per type (cross-reference: the `registry_methods_sorted_per_type` test enforces this).

### RG-4 — Registry as SSOT for Builtin Behavior

Knowledge about builtin type behavior (method presence, method signatures, trait impls for primitives, operator desugaring) SHALL live exclusively in `ori_registry` and be read through `MethodRegistry`. Any consumer (`ori_types`, `ori_eval`, `ori_llvm`) that hardcodes knowledge like `if type == str { special_case }` outside `ori_registry` is a LEAK per `HYG:§Side Logic`.

Cross-reference: `.claude/rules/registry.md`.

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

Type errors SHALL be accumulated via Salsa's diagnostic accumulator, not returned through `Result`. Phase 1 / 2 / 3 / 4 of `check_module` each append to the accumulator; callers collect at the end.

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

`E2033` — user attempted to manually `impl` a marker trait (`Sendable`, `Value`) or attempted to derive a trait that is not derivable (e.g., `Iterator`).

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
| **Koka type rep.** | Effect-typed functions with capability sets | `Function` tag carrying capability set (TL-2) |
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
| 0 | `Int` | `int` | Clause 8.1.1 |
| 1 | `Float` | `float` | Clause 8.1.2 |
| 2 | `Bool` | `bool` | Clause 8.1.3 |
| 3 | `Str` | `str` | Clause 8.1.4 |
| 4 | `Char` | `char` | Clause 8.1.5 |
| 5 | `Byte` | `byte` | Clause 8.1.6 |
| 6 | `Unit` | `()` | Clause 8.1.7 |
| 7 | `Never` | `Never` | Clause 8.1.8 |
| 8 | `Error` | `<error>` | Internal (poison) |
| 9 | `Duration` | `Duration` | Clause 8.1.9 |
| 10 | `Size` | `Size` | Clause 8.1.10 |
| 11 | `Ordering` | `Ordering` | Clause 8.1.11 |

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
| IS_COPYABLE | AND across children AND parent obeys `Value` (TR-7) |
| Capability (HAS_*) | OR across function-type children (non-function children contribute nothing) |
