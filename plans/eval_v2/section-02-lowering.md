---
section: "02"
title: AST Lowering
status: not-started
goal: Implement the ExprArena → CanArena transformation that maps all 52 ExprKind variants to canonical form, desugaring 7 sugar variants
sections:
  - id: "02.1"
    title: Lowering Infrastructure
    status: not-started
  - id: "02.2"
    title: Primitive Variant Mapping
    status: not-started
  - id: "02.3"
    title: Sugar Desugaring
    status: not-started
  - id: "02.4"
    title: Type Attachment
    status: not-started
  - id: "02.5"
    title: Completion Checklist
    status: not-started
---

# Section 02: AST Lowering

**Status:** Not Started
**Goal:** Implement the `ExprArena → CanArena` transformation. Every `ExprKind` variant is either mapped directly to its `CanExpr` equivalent (44 variants) or desugared into primitive operations (7 sugar variants). The error variant maps to `CanExpr::Error`.

**File:** `compiler/ori_canon/src/lower.rs` + `compiler/ori_canon/src/desugar.rs`

**Prior art:**
- **Roc** `crates/compiler/can/src/expr.rs` — `canonicalize_expr()` transforms `ast::Expr` → `can::Expr` with name resolution, operator desugaring, reference tracking
- **Elm** `compiler/src/Canonicalize/Expression.hs` — `canonicalize` transforms Source → Canonical with binop resolution, let-rec detection

---

## 02.1 Lowering Infrastructure

```rust
/// The lowering context: holds source arena, type info, and builds the canonical arena.
pub struct Lowerer<'a> {
    /// Source AST (read-only)
    src: &'a ExprArena,
    /// Type check results (for signatures, pattern resolutions, expr types)
    types: &'a TypeCheckResult,
    /// Target canonical arena (being built)
    dst: CanArena,
    /// Constant pool (for folded constants)
    constants: ConstantPool,
    /// Decision tree pool (for compiled patterns)
    decision_trees: DecisionTreePool,
    /// ID mapping: ExprId → CanId (for cross-references)
    id_map: Vec<CanId>,
}
```

- [ ] Implement `Lowerer::new(src, types)` — initializes with pre-allocated arena
- [ ] Implement `Lowerer::lower_expr(expr_id: ExprId) -> CanId` — main recursive entry point
  - [ ] Looks up `ExprKind` from source arena
  - [ ] Dispatches to variant-specific lowering
  - [ ] Records `ExprId → CanId` mapping
  - [ ] Attaches resolved type from `types.expr_types[expr_id]`
- [ ] Implement `Lowerer::finish() -> CanonResult` — produces the final result
- [ ] Implement top-level `pub fn lower(parse: &ParseResult, types: &TypeCheckResult) -> CanonResult`

---

## 02.2 Primitive Variant Mapping

44 variants map directly from `ExprKind` to `CanExpr` with child references remapped from `ExprId` to `CanId`.

**Mapping rules:**
- Leaf nodes (Int, Float, Bool, Str, Char, Unit, Duration, Size, Ident, Const, SelfRef, FunctionRef, HashLength, None, Error) → copy directly
- Unary nodes (Unary, Try, Await, Some, Ok, Err, Break, Continue, Loop) → lower child, construct `CanExpr` with `CanId`
- Binary/ternary nodes (Binary, Field, Index, Assign, If, For, Range, WithCapability, Cast) → lower children, construct `CanExpr` with `CanId`s
- Container nodes (Block, Let, Lambda, List, Tuple, Map, Struct, Match, Call, MethodCall) → lower children and ranges, construct `CanExpr`
- Special forms (FunctionSeq, FunctionExp) → pass through IDs (these reference separate arenas)

- [ ] Implement lowering for all leaf nodes (15 variants) — trivial copy
- [ ] Implement lowering for all unary nodes (9 variants) — lower child, wrap
- [ ] Implement lowering for all binary/ternary nodes (9 variants) — lower children, wrap
- [ ] Implement lowering for container nodes (10 variants) — lower ranges, wrap
  - [ ] `List(ExprRange)` → lower each element, build `CanRange`, construct `CanExpr::List`
  - [ ] `Block { stmts, result }` → lower statements and result, build `CanStmtRange`
  - [ ] `Match { scrutinee, arms }` → lower scrutinee, compile patterns (Section 03), construct `CanExpr::Match` with `DecisionTreeId`
  - [ ] `Call { func, args }` → lower func and args
  - [ ] `MethodCall { receiver, method, args }` → lower receiver and args
- [ ] Implement pass-through for special forms (2 variants)
  - [ ] `FunctionSeq(id)` → `CanExpr::FunctionSeq(id)` (references separate arena)
  - [ ] `FunctionExp(id)` → `CanExpr::FunctionExp(id)` (references separate arena)

---

## 02.3 Sugar Desugaring

7 variants are desugared into compositions of primitive `CanExpr` nodes during lowering.

### Named Arguments → Positional Calls

```
CallNamed { func, args: [(name, expr), ...] }
  → Call { func, args: [reordered exprs...] }

MethodCallNamed { receiver, method, args: [(name, expr), ...] }
  → MethodCall { receiver, method, args: [reordered exprs...] }
```

- [ ] Implement `lower_call_named(expr_id) -> CanId`
  - [ ] Look up function signature from `TypeCheckResult`
  - [ ] Extract parameter names and positional order
  - [ ] Reorder named arguments to match signature
  - [ ] Lower each argument expression
  - [ ] Build `CanExpr::Call` with positional `CanRange`
- [ ] Implement `lower_method_call_named(expr_id) -> CanId`
  - [ ] Same logic, preserving receiver

### Template Literals → String Concatenation

```
TemplateFull(name)
  → Str(name)

TemplateLiteral { head, parts: [(expr, fmt, text), ...] }
  → MethodCall(MethodCall(Str(head), "concat", [MethodCall(expr, "to_str", [])]), "concat", [Str(text)])...
```

- [ ] Implement `lower_template_full(name) -> CanId` — trivial: emit `CanExpr::Str(name)`
- [ ] Implement `lower_template_literal(head, parts) -> CanId`
  - [ ] Start with `CanExpr::Str(head)` as accumulator
  - [ ] For each part: lower expr → `.to_str()` → `.concat()` to accumulator → `.concat(text)` if non-empty
  - [ ] Handle format specs (future: may desugar to `.format(spec)` instead of `.to_str()`)
  - [ ] Return final accumulator `CanId`

### Spread Operators → Collection Operations

```
ListWithSpread([1, 2, ...existing, 3])
  → MethodCall(List([1, 2]), "concat", [existing]).concat(List([3]))

MapWithSpread({...defaults, "k": v})
  → MethodCall(defaults, "merge", [Map({"k": v})])

StructWithSpread { name, fields: [...base, x: 10] }
  → Struct { name, fields: { x: 10, y: Field(base, "y"), z: Field(base, "z") } }
```

- [ ] Implement `lower_list_with_spread(elements) -> CanId`
  - [ ] Group consecutive non-spread elements into `CanExpr::List` nodes
  - [ ] Chain spread elements via `.concat()` method calls
  - [ ] Handle: spread at start, spread at end, multiple spreads, only spreads
- [ ] Implement `lower_map_with_spread(elements) -> CanId`
  - [ ] Group consecutive non-spread entries into `CanExpr::Map` nodes
  - [ ] Chain via `.merge()` method calls
  - [ ] Preserve "later wins" ordering
- [ ] Implement `lower_struct_with_spread(name, fields, type_result) -> CanId`
  - [ ] Look up struct type definition for all field names
  - [ ] Explicit fields: use provided expression
  - [ ] Spread fields: generate `CanExpr::Field { receiver: spread_expr, field: name }`
  - [ ] Construct `CanExpr::Struct` with all fields resolved
  - [ ] Handle multiple spreads (later wins per field)

---

## 02.4 Type Attachment

Every `CanNode` carries a resolved type (`ty: Idx`). During lowering, types are attached from the type checker's `expr_types` map.

- [ ] For directly-mapped expressions: `types.expr_types.get(expr_id)` → attach to `CanNode`
- [ ] For synthesized expressions (from desugaring): compute type from context
  - [ ] `.to_str()` call → type is `Idx::STR`
  - [ ] `.concat()` call on string → type is `Idx::STR`
  - [ ] `.concat()` call on list → type is same as list type
  - [ ] `.merge()` call on map → type is same as map type
  - [ ] Field access from spread → type is field's declared type
- [ ] Type defaults to `Idx::INFER` if unavailable (should not happen for well-typed programs)

---

## 02.5 Completion Checklist

- [ ] All 52 `ExprKind` variants handled in lowering (44 mapped, 7 desugared, 1 error)
- [ ] Lowering produces valid `CanArena` — all `CanId` references resolve
- [ ] `validate_canonical()` passes in debug builds (no sugar, no dangling refs)
- [ ] Type attached to every `CanNode` from type checker's `expr_types`
- [ ] Synthesized nodes from desugaring have correct types
- [ ] Round-trip test: `parse → typecheck → lower → eval` produces same results as `parse → typecheck → eval` for all spec tests
- [ ] `./test-all.sh` passes (run through old path; new path validated separately)

**Exit Criteria:** The lowering pass transforms every `ExprKind` variant to its canonical equivalent. Sugar is eliminated. Types are attached. The canonical arena is self-consistent. Round-trip testing shows identical behavior.
