---
section: "02"
title: "ARC Lowering Integration"
status: complete
goal: "Enable the ARC lowerer to substitute generic types with concrete types during lowering"
sections:
  - id: "02.1"
    title: "Add type_subst to lower_function_can()"
    status: complete
  - id: "02.2"
    title: "ArcLowerer resolve_body_type()"
    status: complete
---

# Section 02: ARC Lowering Integration

**Goal:** The ARC lowerer (`ori_arc`) produces `ArcFunction` values with ownership annotations and RC operations. For monomorphized functions, the lowerer must substitute generic types with concrete types so that RC inc/dec/drop target the right runtime operations (e.g., `ori_rc_dec` for heap-allocated types vs no-op for scalars).

**Key insight (from Swift):** The canonical IR body is shared — not cloned. The `body_type_map` from `MonoInstance` is passed as a substitution map, and the lowerer applies it when reading expression types from the canonical IR. This is the "clone-and-substitute" model but without actually cloning the IR.

---

## 02.1 Add `type_subst` to `lower_function_can()`

**File:** `compiler/ori_arc/src/lower/mod.rs`

Add an optional type substitution map parameter. All existing callers pass `None` — zero behavioral change for non-generic functions.

```rust
pub fn lower_function_can(
    name: Name,
    params: &[(Name, Idx)],
    return_type: Idx,
    body: CanId,
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
    problems: &mut Vec<ArcProblem>,
    is_fbip: bool,
    type_subst: Option<&FxHashMap<Idx, Idx>>,  // NEW
) -> (ArcFunction, Vec<ArcFunction>)
```

- [x] Add `type_subst` parameter to `lower_function_can()` signature
- [x] Pass `type_subst` through to `ArcLowerer` construction
- [x] Update all existing callers to pass `None`
- [x] Compilation clean, all tests pass (pure refactor, no behavior change)

---

## 02.2 ArcLowerer `resolve_body_type()`

**File:** `compiler/ori_arc/src/lower/expr/mod.rs`

Add `type_subst: Option<&FxHashMap<Idx, Idx>>` to `ArcLowerer`. Provide a `resolve_body_type()` method that transparently applies substitution:

```rust
fn resolve_body_type(&self, ty: Idx) -> Idx {
    match &self.type_subst {
        Some(map) => map.get(&ty).copied().unwrap_or(ty),
        None => ty,
    }
}
```

Call `resolve_body_type()` everywhere the lowerer reads `CanNode.ty` — expression type lookups, constructor types, call return types in the body.

- [x] Add `type_subst` field to `ArcLowerer`
- [x] Implement `resolve_body_type()` method
- [x] Audit all `CanNode.ty` reads in the lowerer and wrap with `resolve_body_type()`
- [x] Unit test: lowering a function body with substitution map produces concrete types in ArcFunction
- [x] Verify: existing non-generic lowering unchanged (all `None` callers still work)
