# Registry (ori_registry)

## Purpose

**SSOT architectural center** for all builtin type behavioral specifications. The registry is one of four canonical homes in the compiler (see `impl-hygiene.md` → Paradigms → SSOT):

| Center | Domain |
|---|---|
| **Registry** (`ori_registry`) | Builtin type behavior — methods, operators, memory strategy |
| Type Pool (`ori_types`) | Type structure — interned types, type relationships |
| AIMS (`ori_arc`) | Memory analysis — ownership, borrowing, uniqueness |
| Repr-opt (`ori_llvm`) | Representation — layout, ABI, storage |

The registry's contract: **zero dependencies, zero logic, only `const` data.** It defines WHAT builtins can do. Consuming crates define HOW.

## Key Types
- `TypeDef` — type definition (tag, name, memory strategy, methods, operators)
- `MethodDef` — method specification (name, receiver ownership, params, returns, purity, kind)
- `TypeTag` — enum of all 23 builtin types
- `OpDefs` / `OpStrategy` — operator dispatch strategies per type

## Query API
- `find_type(TypeTag) -> Option<&TypeDef>` — type lookup
- `find_method(TypeTag, &str) -> Option<&MethodDef>` — method lookup
- `has_method(TypeTag, &str) -> bool` — existence check
- `find_type_by_name(&str) -> Option<&TypeDef>` — name-based lookup

## Adding a New Builtin Type

1. **`ori_registry`**: Add `TypeTag` variant in `tags/mod.rs`
2. **`ori_registry`**: Create `defs/<type_name>.rs` with `pub const TYPE_NAME: TypeDef = ...`
3. **`ori_registry`**: Add to `BUILTIN_TYPES` array in `defs/mod.rs`
4. **4 exhaustiveness guards break**: Update `_enforce_type_tag_exhaustiveness()` in:
   - `ori_types/src/infer/expr/methods/mod.rs`
   - `ori_eval/src/methods/mod.rs`
   - `ori_arc/src/borrow/mod.rs`
   - `ori_llvm/src/codegen/arc_emitter/builtins/mod.rs`
5. **Tests**: Run `cargo t -p ori_registry` (integrity), then `cargo t -p oric` (cross-phase enforcement)

## Adding a New Builtin Method

1. **`ori_registry`**: Add `MethodDef` entry to the type's `methods` slice in `defs/<type>.rs`
   - Keep methods **sorted alphabetically** (enforced by `registry_methods_sorted_per_type` test)
   - All 10 `MethodDef` fields required (no defaults)
2. **Enforcement tests guide you**: Run `cargo t -p oric` — failing tests show which phases need handlers:
   - `every_registry_method_has_typeck_handler` — add handler in `ori_types`
   - `every_registry_method_has_eval_handler` — add handler in `ori_eval` (or add to `METHODS_NOT_YET_IN_EVAL` temporarily)
   - LLVM coverage tracked by `builtin_coverage_above_threshold`
3. **ARC ownership**: If `receiver: Ownership::Borrow`, verify `every_registry_borrowing_method_in_arc_set` passes
4. **Tests**: `cargo t -p ori_registry && cargo t -p oric`

## Adding a New Field to TypeDef/MethodDef

1. Add the field to the struct in `ori_registry`
2. Compilation fails in every `defs/*.rs` file — fill in each one
3. Update consuming phases that read the field
4. Add enforcement test if the field has cross-phase implications

## Enforcement Architecture

| Test | What It Catches | Location |
|------|-----------------|----------|
| `_enforce_type_tag_exhaustiveness()` | New TypeTag variant (compile-time) | 4 consuming crates |
| `every_registry_method_has_typeck_handler` | Missing typeck handler | `oric/eval/tests/methods/consistency.rs` |
| `every_registry_method_has_eval_handler` | Missing eval handler | `oric/eval/tests/methods/consistency.rs` |
| `registry_op_strategies_cover_all_operators` | Missing LLVM operator handler | `ori_llvm/builtins/tests.rs` |
| `every_registry_borrowing_method_in_arc_set` | Missing ARC borrow annotation | `oric/eval/tests/methods/consistency.rs` |
| `backend_required_methods_fully_implemented` | Incomplete backend support | `oric/eval/tests/methods/consistency.rs` |
| `no_duplicate_methods` | Copy-paste errors | `ori_registry/defs/tests.rs` |
| `registry_methods_sorted_per_type` | Unsorted methods | `oric/eval/tests/methods/consistency.rs` |
| `purity_cargo_toml_has_no_dependencies` | Dependency creep | `ori_registry/tests.rs` |

## Key Files
- `compiler/ori_registry/src/lib.rs` — crate root, query functions
- `compiler/ori_registry/src/defs/` — type definitions (one file per type or group)
- `compiler/ori_registry/src/tags/mod.rs` — TypeTag, Ownership, OpStrategy enums
- `compiler/ori_registry/src/method/mod.rs` — MethodDef, ParamDef structs
- `compiler/ori_registry/src/query/mod.rs` — query functions
- `compiler/ori_registry/src/defs/tests.rs` — registry-level integrity tests
- `compiler/oric/src/eval/tests/methods/consistency.rs` — cross-phase enforcement tests

## Consumer Discipline — Query, Don't Copy

The registry exists so that consuming crates **never need to hardcode builtin type knowledge**. Every violation of this principle is a **LEAK:scattered-knowledge** finding.

**Consumers MUST:**
- Query `find_type()` / `find_method()` for type capabilities — never hardcode "str has method split"
- Read `MethodDef` fields (receiver ownership, purity, params) — never re-derive them
- Use `OpDefs` / `OpStrategy` for operator dispatch — never build parallel operator tables
- Use `TypeTag` exhaustive matches for type dispatch — the compiler enforces completeness

**Consumers MUST NOT:**
- Maintain local lookup tables that mirror registry data (e.g., a `HashMap<&str, ReturnType>` for method return types)
- Hardcode type-specific behavior with `if tag == TypeTag::Str { ... }` when the registry already encodes the distinction (e.g., memory strategy, method availability)
- Re-derive method signatures, parameter counts, or return types that `MethodDef` already specifies
- Add `match TypeTag { ... }` arms that encode behavioral knowledge instead of querying the registry for it

**The litmus test**: if a consuming crate's match arm would need updating because a *builtin's behavior* changed (not because the *consumer's handling* changed), the consumer has leaked registry knowledge. The fix is to query the registry and dispatch on its answer.

**Acceptable type-specific dispatch**: Consumers legitimately need per-type *implementation* logic — e.g., `ori_eval` needs different code to evaluate `str.split()` vs `list.push()`. That's implementation dispatch (HOW), not behavioral knowledge (WHAT). The registry tells you WHAT methods exist and their signatures; the consumer implements HOW to execute them.

## Purity Contract
- Zero `[dependencies]` in Cargo.toml (test enforced)
- All `TypeDef` constants are `const`-constructible (test enforced)
- Core enums (`TypeTag`, `Ownership`, `OpStrategy`) are `Copy` (test enforced)
- No IO, no allocation, no side effects, no `unsafe`
