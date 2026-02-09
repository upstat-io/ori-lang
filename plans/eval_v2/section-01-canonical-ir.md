---
section: "01"
title: Canonical IR
status: not-started
goal: Define CanExpr, CanArena, CanId types in ori_ir and relocate decision tree types from ori_arc to ori_ir
sections:
  - id: "01.1"
    title: CanExpr Type Definition
    status: not-started
  - id: "01.2"
    title: CanArena and Index Types
    status: not-started
  - id: "01.3"
    title: Decision Tree Type Relocation
    status: not-started
  - id: "01.4"
    title: ori_canon Crate Skeleton
    status: not-started
  - id: "01.5"
    title: Completion Checklist
    status: not-started
---

# Section 01: Canonical IR

**Status:** Not Started
**Goal:** Define the canonical IR types that both backends will consume. This section is pure type definitions — no logic, no behavioral changes.

**Crate:** `ori_ir` (types) + `ori_canon` (crate skeleton)

**Prior art:**
- **Roc** `crates/compiler/can/src/expr.rs` — `can::Expr` enum, separate from `ast::Expr`, with `Symbol` (resolved names) and type variables on every node
- **Elm** `compiler/src/AST/Optimized.hs` — `Opt.Expr` with `Decider`, `Destructor`, `Path` — distinct from `Can.Expr_`
- **Ori** `compiler/ori_ir/src/ast/expr.rs` — Existing `ExprKind` (52 variants, 24 bytes, arena-allocated)

---

## 01.1 CanExpr Type Definition

Define `CanExpr` in `ori_ir/src/canon/mod.rs`. This is the canonical expression type — sugar-free, with decision trees and constants baked in.

- [ ] Define `CanExpr` enum (~42 variants, target ≤ 24 bytes like `ExprKind`)
  - [ ] All 44 primitive `ExprKind` variants mapped to `CanExpr` equivalents using `CanId`/`CanRange`
  - [ ] 7 sugar variants **absent** (CallNamed, MethodCallNamed, TemplateLiteral, TemplateFull, ListWithSpread, MapWithSpread, StructWithSpread)
  - [ ] Added: `Constant(ConstantId)` for compile-time-folded values
  - [ ] Added: `DecisionTreeId` field on `Match` for pre-compiled patterns
  - [ ] Uses `CanId` (not `ExprId`) for child references — distinct index space
- [ ] Define `CanNode` struct (like `Expr` but for canonical form)
  - [ ] `kind: CanExpr`
  - [ ] `span: Span` (preserved from source for error reporting)
  - [ ] `ty: Idx` (resolved type from type checker — attached, not inferred)
- [ ] Size assertion: `CanExpr` ≤ 24 bytes, `CanNode` ≤ 32 bytes
- [ ] Derive: `Copy, Clone, Eq, PartialEq, Hash, Debug` (Salsa-compatible)
- [ ] Define `ConstantId(u32)` newtype for indexing into constant pool
- [ ] Define `DecisionTreeId(u32)` newtype for indexing into decision tree storage

**Design note — `CanNode` carries resolved type (`ty: Idx`):** Unlike `Expr` (which has no type information), each `CanNode` carries the resolved type from the type checker's `expr_types` map. This means backends don't need to look up types separately — they're right there on the node. This follows Roc's pattern where every `can::Expr` carries type variables.

---

## 01.2 CanArena and Index Types

Define the arena and index types following Ori's existing patterns (`ExprArena`/`ExprId`).

- [ ] Define `CanId(u32)` newtype — index into `CanArena`
  - [ ] `#[repr(transparent)]`, `Copy, Clone, Eq, PartialEq, Hash, Debug`
  - [ ] Sentinel: `CanId::INVALID = u32::MAX`, `.is_valid()`
- [ ] Define `CanRange { start: u32, len: u16 }` — contiguous range in arena
  - [ ] Same layout as `ExprRange` (8 bytes)
  - [ ] `EMPTY`, `.is_empty()`, `.len()`, `.iter()` (like existing range types)
- [ ] Define `CanStmtRange`, `CanMapEntryRange`, `CanFieldRange` — specialized ranges
- [ ] Define `CanArena` — flat `Vec<CanNode>` storage
  - [ ] `push(node: CanNode) -> CanId`
  - [ ] `get(id: CanId) -> &CanNode`
  - [ ] `push_range(nodes: impl Iterator<Item = CanNode>) -> CanRange`
  - [ ] Pre-allocate with `with_capacity(source_len / 20)` (same heuristic as `ExprArena`)
- [ ] Define `ConstantPool` — stores folded constant values
  - [ ] `Vec<Value>` indexed by `ConstantId`
  - [ ] `intern(value: Value) -> ConstantId` (dedup via content hash)
  - [ ] Pre-intern sentinels: void, true, false, 0, 1, empty string
- [ ] Define `DecisionTreePool` — stores compiled decision trees
  - [ ] `Vec<DecisionTree>` indexed by `DecisionTreeId`
  - [ ] `push(tree: DecisionTree) -> DecisionTreeId`
- [ ] Define `CanonResult` — output of the canonicalization pass
  - [ ] `arena: CanArena`
  - [ ] `constants: ConstantPool`
  - [ ] `decision_trees: DecisionTreePool`
  - [ ] `root: CanId` (entry point expression)
  - [ ] Derive `Clone, Debug` for Salsa

---

## 01.3 Decision Tree Type Relocation

Move decision tree types from `ori_arc` to `ori_ir/src/canon/tree.rs`. The types are pure data definitions with no logic — they belong in the shared types crate.

- [ ] Move from `ori_arc/src/decision_tree/mod.rs` to `ori_ir/src/canon/tree.rs`:
  - [ ] `DecisionTree` enum (`Switch`, `Leaf`, `Guard`, `Fail`)
  - [ ] `ScrutineePath` = `SmallVec<[PathInstruction; 4]>`
  - [ ] `PathInstruction` enum (`TagPayload`, `TupleIndex`, `StructField`, `ListElement`)
  - [ ] `TestKind` enum (`EnumTag`, `IntEq`, `StrEq`, `BoolEq`, `FloatEq`, `IntRange`, `ListLen`)
  - [ ] `TestValue` enum (`Tag`, `Int`, `Str`, `Bool`, `Float`, `IntRange`, `ListLen`)
- [ ] Update `ori_arc` to import these types from `ori_ir` instead of defining them
  - [ ] `ori_arc/src/decision_tree/emit.rs` — update imports
  - [ ] `ori_arc/src/decision_tree/compile.rs` — update imports
  - [ ] `ori_arc/src/decision_tree/flatten.rs` — update imports
- [ ] Update `ori_arc/Cargo.toml` if `smallvec` dependency needs to move to `ori_ir`
- [ ] Verify `ori_arc` tests still pass after relocation

**The construction algorithm stays in `ori_arc`** (or moves to `ori_canon`). Only the TYPE DEFINITIONS move to `ori_ir`. This is the minimal change to break the circular dependency.

---

## 01.4 ori_canon Crate Skeleton

Create the new `ori_canon` crate with module structure but no logic yet.

- [ ] Create `compiler/ori_canon/Cargo.toml`
  - [ ] Dependencies: `ori_ir`, `ori_types`, `ori_arc` (for pattern compilation algorithm), `rustc-hash`, `tracing`
  - [ ] Add to workspace in root `Cargo.toml`
- [ ] Create module structure:
  ```
  compiler/ori_canon/src/
    lib.rs          — pub mod lower, desugar, patterns, const_fold, validate
    lower.rs        — pub fn lower(parse: &ParseResult, types: &TypeCheckResult) -> CanonResult
    desugar.rs      — sugar elimination helpers (called by lower)
    patterns.rs     — pattern compilation (calls ori_arc algorithm, stores in CanArena)
    const_fold.rs   — constant folding (called by lower)
    validate.rs     — debug_assert! validation of canonical invariants
  ```
- [ ] Implement `lower()` as a stub that panics — actual lowering in Section 02
- [ ] Implement `validate()` — walks arena, asserts invariants (enabled in debug builds)

---

## 01.5 Completion Checklist

- [ ] `CanExpr` enum defined with ~42 variants, ≤ 24 bytes
- [ ] `CanNode` struct with kind + span + resolved type
- [ ] `CanId`, `CanRange`, `CanArena` following existing Ori patterns
- [ ] `ConstantPool` and `DecisionTreePool` defined
- [ ] `CanonResult` struct defined (Salsa-compatible)
- [ ] Decision tree types relocated from `ori_arc` to `ori_ir`
- [ ] `ori_canon` crate created with module skeleton
- [ ] `ori_arc` updated to import from `ori_ir` (tests pass)
- [ ] `./test-all.sh` passes — no behavioral changes

**Exit Criteria:** All canonical IR types exist and compile. Decision tree types are in `ori_ir` (shared). `ori_canon` crate exists with stub lowering function. No behavior changes — this is pure infrastructure.
