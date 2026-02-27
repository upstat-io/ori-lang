---
paths:
  - "**/ori_types/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

# Type System (V2)

## Pool Architecture
- **`Idx(u32)`**: Universal type handle — THE canonical representation
- **`Pool`**: Unified storage (items + extra arrays + flags + hashes)
- **`Tag(u8)`**: Type kind discriminant for tag-driven dispatch
- **`Item`**: Compact 5-byte storage (1 tag + 4 data)
- Pre-interned primitives at fixed indices: INT=0..ORDERING=11
- O(1) equality: `idx1 == idx2` (interning deduplication)

## TypeId vs Idx
- `TypeId` (`ori_ir`): Parser-level type index. Same layout as Idx for primitives 0-11.
- `Idx` (`ori_types`): Type checker pool handle. Used by unification, inference, registries.
- Bridge: `resolve_type_id()` maps TypeId→Idx (identity for primitives)

## Type Variants (via Tag)
- Primitives: `Int`, `Float`, `Bool`, `Str`, `Char`, `Byte`, `Unit`, `Never`, `Duration`, `Size`, `Ordering`
- Simple containers: `List`, `Option`, `Set`, `Channel`, `Range` (data = child Idx)
- Two-child: `Map`, `Result` (data = index into extra array)
- Complex: `Function`, `Tuple`, `Struct`, `Enum` (extra array with length prefix)
- Named: `Named`, `Applied`, `Alias`
- Variables: `Var`, `BoundVar`, `RigidVar`
- Schemes: `Scheme`
- Special: `Projection`, `ModuleNs`, `Infer`, `SelfType`

## TypeFlags (Pre-Computed Metadata)
- Computed once at interning time, O(1) queries
- Presence: `HAS_VAR`, `HAS_ERROR`, `HAS_INFER`, etc.
- Category: `IS_PRIMITIVE`, `IS_CONTAINER`, `IS_FUNCTION`, etc.
- Optimization: `NEEDS_SUBST`, `IS_RESOLVED`, `IS_MONO`

## Inference Engine
- `InferEngine`: Mutable state wrapping `Pool`
- `fresh_var()`: Counter-based, rank-aware
- Path-compressed union-find unification
- Rank-based generalization for let-polymorphism
- Bidirectional checking via `Expected`/`ExpectedOrigin`

## Registries
- `TypeRegistry`: User-defined types (struct, enum, newtype, alias)
- `TraitRegistry`: Traits and implementations
- `MethodRegistry`: Unified lookup (builtin → inherent → trait)

## Module Checker
- `check_module()`: Full module-level type checking
- Registration passes → signature collection → body checking
- Salsa-compatible via `TypeCheckResult`

## Salsa Compatibility
- All types: `Clone, Eq, PartialEq, Hash, Debug`
- No `Arc<Mutex<T>>` or fn pointers
- Deterministic (no random/time/IO)

## Debugging / Tracing

**Always use `ORI_LOG` first when debugging type issues.** Tracing target: `ori_types`.

```bash
ORI_LOG=ori_types=debug ori check file.ori          # Phase boundaries, module checking
ORI_LOG=ori_types=trace ori check file.ori          # Per-expression inference (hot path)
ORI_LOG=ori_types=trace ORI_LOG_TREE=1 ori check file.ori  # Hierarchical call tree
ORI_LOG=ori_types=debug,oric=debug ori check file.ori      # Types + Salsa query execution
```

**Instrumented functions** (`debug` level): `check_module()`, `check_module_impl()`, `collect_signatures()`, `check_function_bodies()`, `check_test_bodies()`, `check_impl_bodies()`
**Instrumented functions** (`trace` level): `infer_expr()`, `check_expr()` — per-expression, very verbose
**Manual events**: `push_error()` logs type errors at debug level

### Phase Dump
```bash
ORI_DUMP_AFTER_TYPECK=1 ori check file.ori          # Dump typed IR (type pool state after checking)
```

**Tips**:
- Type mismatch? Use `ORI_LOG=ori_types=debug` to see which function body triggers the error
- Inference wrong? Use `ORI_LOG=ori_types=trace ORI_LOG_TREE=1` to trace the full inference chain
- Salsa re-execution? Add `oric=debug` to see `WillExecute` events for the `typed()` query

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
