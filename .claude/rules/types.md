---
paths:
  - "**ori_types**"
---

# Type System (V2)

## Pool Architecture
- **`Idx(u32)`**: Universal type handle — THE canonical representation
- **`Pool`**: Unified storage (items + extra arrays + flags + hashes)
- **`Tag(u8)`**: Type kind discriminant for tag-driven dispatch
- **`Item`**: Compact 5-byte storage (1 tag + 4 data)
- Pre-interned primitives at fixed indices: INT=0..ORDERING=11
- O(1) equality: `idx1 == idx2` (interning deduplication)

## TypeId vs Idx
- `TypeId` (`ori_ir`): Parser-level type index | same layout as Idx for primitives 0-11
- `Idx` (`ori_types`): Type checker pool handle | used by unification, inference, registries
- Bridge: `resolve_type_id()` maps TypeId -> Idx (identity for primitives)

## Type Variants (via Tag)
- Primitives: `Int` `Float` `Bool` `Str` `Char` `Byte` `Unit` `Never` `Duration` `Size` `Ordering`
- Simple containers: `List` `Option` `Set` `Channel` `Range` (data = child Idx)
- Two-child: `Map` `Result` (data = index into extra array)
- Complex: `Function` `Tuple` `Struct` `Enum` (extra array with length prefix)
- Named: `Named` `Applied` `Alias`
- Variables: `Var` `BoundVar` `RigidVar`
- Schemes: `Scheme`
- Special: `Projection` `ModuleNs` `Infer` `SelfType`

## TypeFlags (Pre-Computed Metadata)
- Computed once at interning time, O(1) queries
- Presence: `HAS_VAR` `HAS_ERROR` `HAS_INFER` etc.
- Category: `IS_PRIMITIVE` `IS_CONTAINER` `IS_FUNCTION` etc.
- Optimization: `NEEDS_SUBST` `IS_RESOLVED` `IS_MONO`

## Inference Engine
- `InferEngine`: Mutable state wrapping `Pool`
- `fresh_var()`: Counter-based, rank-aware
- Path-compressed union-find unification
- Rank-based generalization for let-polymorphism
- Bidirectional checking via `Expected`/`ExpectedOrigin`

## Registries
- `TypeRegistry`: User-defined types (struct, enum, newtype, alias)
- `TraitRegistry`: Traits and implementations
- `MethodRegistry`: Unified lookup (builtin -> inherent -> trait)

## Module Checker
- `check_module()`: Full module-level type checking
- Registration passes -> signature collection -> body checking
- Salsa-compatible via `TypeCheckResult`

## Salsa Compatibility
- All types: `Clone, Eq, PartialEq, Hash, Debug`
- No `Arc<Mutex<T>>` or fn pointers | deterministic (no random/time/IO)

## Tracing
- Target: `ori_types` | `ORI_LOG=ori_types=debug` (phases) | `=trace` (per-expression) | `ORI_LOG_TREE=1` for call tree
- Phase dump: `ORI_DUMP_AFTER_TYPECK=1` | see compiler.md for full debugging reference

## Key Files
- `pool/mod.rs`: Pool, interning, query methods
- `pool/construct/`: Type construction helpers
- `idx.rs`: Idx type handle
- `tag.rs`: Tag enum
- `flags.rs`: TypeFlags bitflags
- `infer/mod.rs`: InferEngine
- `infer/expr/`: Expression inference (identifiers, calls, operators, etc.)
- `unify/`: Unification engine
- `check/mod.rs`: Module-level type checker
- `check/registration/`: Type/trait/impl registration passes
- `check/bodies/`: Function body checking
- `registry/`: Type (`types/`), trait (`traits/`), method (`methods/`) registries
