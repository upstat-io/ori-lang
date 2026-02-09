---
section: "02"
title: AST Lowering
status: complete
goal: Implement the ExprArena → CanArena transformation that maps all 52 ExprKind variants to canonical form, desugaring 7 sugar variants
sections:
  - id: "02.1"
    title: Lowering Infrastructure
    status: complete
  - id: "02.2"
    title: Primitive Variant Mapping
    status: complete
  - id: "02.3"
    title: Sugar Desugaring
    status: complete
  - id: "02.4"
    title: Type Attachment
    status: complete
  - id: "02.5"
    title: Completion Checklist
    status: complete
---

# Section 02: AST Lowering

**Status:** Complete (2026-02-09)
**Goal:** Implement the `ExprArena → CanArena` transformation. Every `ExprKind` variant is either mapped directly to its `CanExpr` equivalent (44 variants) or desugared into primitive operations (7 sugar variants). The error variant maps to `CanExpr::Error`.

**File:** `compiler/ori_canon/src/lower.rs` + `compiler/ori_canon/src/desugar.rs`

**Prior art:**
- **Roc** `crates/compiler/can/src/expr.rs` — `canonicalize_expr()` transforms `ast::Expr` → `can::Expr` with name resolution, operator desugaring, reference tracking
- **Elm** `compiler/src/Canonicalize/Expression.hs` — `canonicalize` transforms Source → Canonical with binop resolution, let-rec detection

---

## 02.1 Lowering Infrastructure

```rust
/// The lowering context: holds source arena, type info, and builds the canonical arena.
pub(crate) struct Lowerer<'a> {
    src: &'a ExprArena,
    typed: &'a TypedModule,
    arena: CanArena,
    constants: ConstantPool,
    decision_trees: DecisionTreePool,
    // Pre-interned method names for desugaring
    name_to_str: Name,
    name_concat: Name,
    name_merge: Name,
}
```

- [x] Implement `Lowerer::new(src, typed, interner)` — initializes with pre-allocated arena (25% headroom over source count)
- [x] Implement `Lowerer::lower_expr(expr_id: ExprId) -> CanId` — main recursive dispatch
  - [x] Copies `ExprKind` out of source arena (ExprKind is Copy, avoids borrow conflicts)
  - [x] Dispatches to variant-specific lowering (exhaustive match on all 52 variants)
  - [x] Attaches resolved type from `typed.expr_type(id.index())` → `TypeId::from_raw(idx.raw())`
- [x] Implement `Lowerer::finish(self, root) -> CanonResult` — produces the final result
- [x] Implement top-level `pub fn lower(src: &ExprArena, type_result: &TypeCheckResult, root: ExprId, interner: &SharedInterner) -> CanonResult`
- [x] Helper: `lower_optional(id)` — handles `ExprId::INVALID` sentinel → `CanId::INVALID`
- [x] Helper: `push(kind, span, ty) -> CanId` — push canonical node into arena
- [x] Debug validation: `validate(&result)` called automatically in `#[cfg(debug_assertions)]`

**Implementation note — no `id_map`:** The plan suggested an `ExprId → CanId` mapping vector, but this was skipped because each `ExprId` is lowered exactly once (tree structure, not DAG). If caching becomes needed later, it can be added.

**Implementation note — pre-interned names:** `to_str`, `concat`, and `merge` are pre-interned in `Lowerer::new()` to avoid repeated hash lookups during desugaring.

---

## 02.2 Primitive Variant Mapping

44 variants map directly from `ExprKind` to `CanExpr` with child references remapped from `ExprId` to `CanId`.

**Mapping rules:**
- Leaf nodes (Int, Float, Bool, Str, Char, Unit, Duration, Size, Ident, Const, SelfRef, FunctionRef, HashLength, None, Error) → copy directly
- Unary nodes (Unary, Try, Await, Some, Ok, Err, Break, Continue, Loop) → lower child, construct `CanExpr` with `CanId`
- Binary/ternary nodes (Binary, Field, Index, Assign, If, For, Range, WithCapability, Cast) → lower children, construct `CanExpr` with `CanId`s
- Container nodes (Block, Let, Lambda, List, Tuple, Map, Struct, Match, Call, MethodCall) → lower children and ranges, construct `CanExpr`
- Special forms (FunctionSeq, FunctionExp) → pass through IDs (these reference separate arenas)

- [x] Implement lowering for all leaf nodes (15 variants) — trivial copy
- [x] Implement lowering for all unary nodes (9 variants) — lower child, wrap
- [x] Implement lowering for all binary/ternary nodes (9 variants) — lower children, wrap
- [x] Implement lowering for container nodes (10 variants) — lower ranges, wrap
  - [x] `List(ExprRange)` → lower each element via `lower_expr_range`, build `CanRange`
  - [x] `Block { stmts, result }` → lower statements via `lower_stmt_range`, lower optional result
  - [x] `Match { scrutinee, arms }` → lower scrutinee and arm bodies, placeholder `DecisionTree::Fail` (Section 03 will compile patterns)
  - [x] `Call { func, args }` → lower func and args via `lower_expr_range`
  - [x] `MethodCall { receiver, method, args }` → lower receiver and args
  - [x] `Map(entries)` → lower key/value pairs via `lower_map_entries`
  - [x] `Struct { name, fields }` → lower field inits via `lower_field_inits`, handle shorthand (`value: None` → synthesize `Ident(name)`)
  - [x] `Tuple(exprs)` → lower each element via `lower_expr_range`
  - [x] `Let { pattern, ty, init, mutable }` → lower init expression
  - [x] `Lambda { params, ret_ty, body }` → lower body
  - [x] `Range { start, end, step, inclusive }` → lower optional children
- [x] Implement pass-through for special forms (2 variants)
  - [x] `FunctionSeq(id)` → `CanExpr::FunctionSeq(id)` (references separate arena)
  - [x] `FunctionExp(id)` → `CanExpr::FunctionExp(id)` (references separate arena)

**Implementation note — `StmtKind` lowering:** `lower_stmt_range` converts `StmtKind::Expr(id)` to a lowered expression and `StmtKind::Let { .. }` to `CanExpr::Let` with `TypeId::UNIT` (let bindings produce unit in Ori's expression-based semantics). Canonical blocks use `CanRange` for statements (not a separate stmt array), simplifying the representation.

---

## 02.3 Sugar Desugaring

7 variants are desugared into compositions of primitive `CanExpr` nodes during lowering. All desugar methods live in `desugar.rs` as `impl Lowerer<'_>` methods.

### Named Arguments → Positional Calls

```
CallNamed { func, args: [(name, expr), ...] }
  → Call { func, args: [reordered exprs...] }

MethodCallNamed { receiver, method, args: [(name, expr), ...] }
  → MethodCall { receiver, method, args: [reordered exprs...] }
```

- [x] Implement `desugar_call_named(func, args, span, ty) -> CanId`
  - [x] Look up function signature from `TypedModule` via `resolve_func_param_names`
  - [x] Reorder named arguments to match signature via `reorder_and_lower_args`
  - [x] Unnamed/positional arguments fill remaining slots left-to-right
  - [x] Fallback: source order if signature unavailable (error recovery, lambdas)
  - [x] Build `CanExpr::Call` with positional `CanRange`
- [x] Implement `desugar_method_call_named(receiver, method, args, span, ty) -> CanId`
  - [x] Same reordering logic, looks up method signature from `typed.impl_sigs`

### Template Literals → String Concatenation

```
TemplateFull(name)
  → Str(name)  (handled inline in lower.rs)

TemplateLiteral { head, parts: [(expr, fmt, text), ...] }
  → "head".concat(expr.to_str()).concat("mid")...
```

- [x] `TemplateFull(name)` → `CanExpr::Str(name)` (handled inline in `lower_expr`)
- [x] Implement `desugar_template_literal(head, parts, span, ty) -> CanId`
  - [x] Start with `CanExpr::Str(head)` as accumulator
  - [x] For each part: lower expr → `.to_str()` if not already STR → `.concat()` to accumulator
  - [x] If `text_after != Name::EMPTY`: chain `.concat(Str(text_after))`
  - [x] Return final accumulator `CanId`

### Spread Operators → Collection Operations

```
ListWithSpread([1, 2, ...existing, 3])
  → [1, 2].concat(existing).concat([3])

MapWithSpread({...defaults, "k": v})
  → {}.merge(defaults).merge({"k": v})

StructWithSpread { name, fields: [...base, x: 10] }
  → Struct { name, fields: { x: base.x, y: base.y, ..., x: 10 } }
```

- [x] Implement `desugar_list_with_spread(elements, span, ty) -> CanId`
  - [x] Group consecutive non-spread elements into `CanExpr::List` nodes
  - [x] Chain spread elements via `.concat()` method calls
  - [x] Shared helper: `chain_method_calls(segments, method, span, ty)` — left-fold via method calls
- [x] Implement `desugar_map_with_spread(elements, span, ty) -> CanId`
  - [x] Group consecutive non-spread entries into `CanExpr::Map` nodes
  - [x] Chain via `.merge()` method calls
  - [x] Uses same `chain_method_calls` helper
- [x] Implement `desugar_struct_with_spread(name, fields, span, ty) -> CanId`
  - [x] Look up struct type definition via `resolve_struct_fields(name)` for all field names in order
  - [x] Explicit fields: lower provided expression (shorthand: synthesize `Ident(name)`)
  - [x] Spread fields: generate `CanExpr::Field { receiver: spread_expr, field: name }` for ALL fields
  - [x] "Later wins" — explicit fields after a spread override the spread
  - [x] Fallback: source-order lowering if struct definition unavailable (error recovery)

---

## 02.4 Type Attachment

Every `CanNode` carries a resolved type (`ty: TypeId`). During lowering, types are attached from the type checker's `expr_types` map.

- [x] For directly-mapped expressions: `typed.expr_type(id.index())` → `TypeId::from_raw(idx.raw())`
- [x] Fallback to `TypeId::ERROR` if type unavailable (error recovery — not `INFER`, since `ERROR` is semantically correct for unresolved types)
- [x] For synthesized expressions (from desugaring): compute type from context
  - [x] `.to_str()` call → `TypeId::STR`
  - [x] `.concat()` call on string → `TypeId::STR`
  - [x] `.concat()` call on list → same type as list (`ty` from parent)
  - [x] `.merge()` call on map → same type as map (`ty` from parent)
  - [x] Field access from spread → `TypeId::ERROR` (placeholder — precise type available in type checker but not needed during lowering; backends resolve from type environment)
  - [x] Shorthand field init `Point { x }` → `TypeId::ERROR` (same reasoning — backends resolve from variable binding)

**Implementation note — `TypeId::ERROR` vs `TypeId::INFER`:** The plan suggested `INFER` as default, but `ERROR` is more appropriate. `INFER` implies the type is unknown and should be inferred, while `ERROR` signals that the type couldn't be determined at this stage. For well-typed programs, all directly-mapped expressions have types from the type checker. Synthesized nodes from desugaring use domain-specific types (`STR` for string operations) or `ERROR` as a placeholder.

---

## 02.5 Completion Checklist

- [x] All 52 `ExprKind` variants handled in lowering (44 mapped, 7 desugared, 1 error)
- [x] Lowering produces valid `CanArena` — all `CanId` references resolve
- [x] `validate()` passes in debug builds (no sugar, no dangling refs)
- [x] Type attached to every `CanNode` from type checker's `expr_types`
- [x] Synthesized nodes from desugaring have correct types
- [x] `./test-all.sh` passes — 8,336 tests, 0 failures
- [x] `./clippy-all.sh` passes — workspace + LLVM clean
- [x] 14 unit tests covering leaf, unary, binary, container lowering and 3 desugaring paths

**Deferred to Section 07:** Round-trip integration test (`parse → typecheck → lower → eval` produces same results as current path). Requires wiring canonicalization into the Salsa pipeline.

**Exit Criteria:** The lowering pass transforms every `ExprKind` variant to its canonical equivalent. Sugar is eliminated. Types are attached. The canonical arena is self-consistent. ✅ Met (2026-02-09)

### Files Modified

| File | Action | Purpose |
|------|--------|---------|
| `compiler/ori_canon/src/lower.rs` | Rewritten | Core lowering: `Lowerer` struct, `lower_expr` dispatch (all 52 arms), container helpers, 11 unit tests |
| `compiler/ori_canon/src/desugar.rs` | Rewritten | Sugar elimination: 7 desugar functions, arg reordering, spread resolution, 3 unit tests |

### Key Design Decisions

| Decision | Choice | Why |
|----------|--------|-----|
| `ExprKind` is Copy | Copy kind out before recursing | No borrow conflicts with mutable `self` |
| No `id_map` caching | Skip (each ExprId lowered once) | ExprIds are unique tree nodes; add if needed later |
| Pre-interned names | `to_str`, `concat`, `merge` in `Lowerer::new()` | Avoids repeated hash lookups during desugaring |
| Match placeholder | `DecisionTree::Fail` | Section 03 replaces; arm bodies still lowered |
| Field shorthand | Synthesize `CanExpr::Ident(name)` | Parser stores `value: None`; lowerer creates implicit ref |
| `Idx` → `TypeId` | `TypeId::from_raw(idx.raw())` | Same u32 layout, zero-cost conversion |
| Struct spread | "Later wins" (unconditional overwrite) | Matches evaluator semantics |
| Error fallback | `TypeId::ERROR` not `TypeId::INFER` | Semantically correct for unresolved types |
