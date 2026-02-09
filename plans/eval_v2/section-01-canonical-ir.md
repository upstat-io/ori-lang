---
section: "01"
title: Canonical IR
status: complete
goal: Define CanExpr, CanArena, CanId types in ori_ir and relocate decision tree types from ori_arc to ori_ir
sections:
  - id: "01.1"
    title: CanExpr Type Definition
    status: complete
  - id: "01.2"
    title: CanArena and Index Types
    status: complete
  - id: "01.3"
    title: Decision Tree Type Relocation
    status: complete
  - id: "01.4"
    title: ori_canon Crate Skeleton
    status: complete
  - id: "01.5"
    title: Completion Checklist
    status: complete
---

# Section 01: Canonical IR

**Status:** Complete (2026-02-09)
**Goal:** Define the canonical IR types that both backends will consume. This section is pure type definitions — no logic, no behavioral changes.

**Crate:** `ori_ir` (types) + `ori_canon` (crate skeleton)

**Prior art:**
- **Roc** `crates/compiler/can/src/expr.rs` — `can::Expr` enum, separate from `ast::Expr`, with `Symbol` (resolved names) and type variables on every node
- **Elm** `compiler/src/AST/Optimized.hs` — `Opt.Expr` with `Decider`, `Destructor`, `Path` — distinct from `Can.Expr_`
- **Ori** `compiler/ori_ir/src/ast/expr.rs` — Existing `ExprKind` (52 variants, 24 bytes, arena-allocated)

---

## 01.1 CanExpr Type Definition

Define `CanExpr` in `ori_ir/src/canon/mod.rs`. This is the canonical expression type — sugar-free, with decision trees and constants baked in.

- [x] Define `CanExpr` enum (46 variants, 24 bytes — verified by `static_assert_size!`)
  - [x] All 44 primitive `ExprKind` variants mapped to `CanExpr` equivalents using `CanId`/`CanRange`
  - [x] 7 sugar variants **absent** (CallNamed, MethodCallNamed, TemplateLiteral, TemplateFull, ListWithSpread, MapWithSpread, StructWithSpread)
  - [x] Added: `Constant(ConstantId)` for compile-time-folded values
  - [x] Added: `DecisionTreeId` field on `Match` for pre-compiled patterns
  - [x] Uses `CanId` (not `ExprId`) for child references — distinct index space
- [x] Define `CanNode` struct (like `Expr` but for canonical form)
  - [x] `kind: CanExpr`
  - [x] `span: Span` (preserved from source for error reporting)
  - [x] `ty: TypeId` (resolved type — uses `TypeId` from `ori_ir`, same index layout as `ori_types::Idx`)
- [x] Size assertion: `CanExpr` = 24 bytes (verified). `CanNode` = 36 bytes (struct-of-arrays in `CanArena`, not stored contiguously)
- [x] Derive: `Copy, Clone, Eq, PartialEq, Hash` (Salsa-compatible). Custom `Debug` impl for readability.
- [x] Define `ConstantId(u32)` newtype for indexing into constant pool
- [x] Define `DecisionTreeId(u32)` newtype for indexing into decision tree storage

**Implementation note — `ty: TypeId` instead of `ty: Idx`:** The plan specified `Idx` from `ori_types`, but `ori_ir` cannot depend on `ori_types` (dependency flows the other direction). `TypeId` in `ori_ir` shares the same u32 index layout as `Idx`, so both backends can convert freely. This is the correct architectural choice.

**Implementation note — 46 variants (not ~42):** The final variant count is 46, slightly higher than the estimate. The extra variants come from `FunctionSeq`, `FunctionExp`, `Duration`, `Size`, and `HashLength` which were not counted in the initial estimate but are needed for complete coverage.

---

## 01.2 CanArena and Index Types

Define the arena and index types following Ori's existing patterns (`ExprArena`/`ExprId`).

- [x] Define `CanId(u32)` newtype — index into `CanArena`
  - [x] `#[repr(transparent)]`, `Copy, Clone, Eq, PartialEq, Hash`
  - [x] Sentinel: `CanId::INVALID = u32::MAX`, `.is_valid()`
  - [x] Custom `Debug` impl (shows `CanId::INVALID` for sentinel)
  - [x] `Default` impl returns `INVALID`
- [x] Define `CanRange { start: u32, len: u16 }` — contiguous range in arena
  - [x] Same layout as `ExprRange` (8 bytes)
  - [x] `EMPTY`, `.is_empty()`, `.len()`
- [x] Define `CanMapEntryRange`, `CanFieldRange` — specialized ranges (8 bytes each)
- [x] Define `CanArena` — struct-of-arrays storage (not flat `Vec<CanNode>`)
  - [x] Parallel arrays: `kinds: Vec<CanExpr>`, `spans: Vec<Span>`, `types: Vec<TypeId>`
  - [x] Auxiliary storage: `expr_lists: Vec<CanId>`, `map_entries: Vec<CanMapEntry>`, `fields: Vec<CanField>`
  - [x] `push(node: CanNode) -> CanId`
  - [x] `get(id: CanId) -> CanNode` (reconstructs from parallel arrays)
  - [x] Individual accessors: `kind()`, `span()`, `ty()`
  - [x] `push_expr_list(ids: &[CanId]) -> CanRange`
  - [x] Incremental list building: `start_expr_list()` / `push_expr_list_item()` / `finish_expr_list()`
  - [x] Pre-allocate with `with_capacity(source_len / 20)` (same heuristic as `ExprArena`)
  - [x] Uses shared `to_u32()`/`to_u16()` overflow-checking helpers from `arena.rs`
- [x] Define `ConstantPool` — stores folded constant values
  - [x] `Vec<ConstValue>` indexed by `ConstantId`
  - [x] `intern(value: ConstValue) -> ConstantId` (dedup via `FxHashMap`)
  - [x] Pre-intern sentinels: UNIT, TRUE, FALSE, ZERO, ONE, EMPTY_STR
- [x] Define `DecisionTreePool` — stores compiled decision trees
  - [x] `Vec<DecisionTree>` indexed by `DecisionTreeId`
  - [x] `push(tree: DecisionTree) -> DecisionTreeId`
- [x] Define `CanonResult` — output of the canonicalization pass
  - [x] `arena: CanArena`, `constants: ConstantPool`, `decision_trees: DecisionTreePool`, `root: CanId`
  - [x] Derive `Clone, Debug, PartialEq, Eq` for Salsa
  - [x] `CanonResult::empty()` factory for error recovery

**Implementation note — struct-of-arrays:** The plan suggested `Vec<CanNode>`, but we used struct-of-arrays (separate `Vec<CanExpr>`, `Vec<Span>`, `Vec<TypeId>`) following the existing `ExprArena` pattern. This gives better cache locality when iterating only kinds or only types, and avoids padding waste.

**Implementation note — no `CanStmtRange`:** Canonical blocks use `CanRange` for statements (which are just `CanId` lists), unlike the source AST where `StmtRange` indexes into a separate `stmts` array. This simplification is possible because canonical statements don't need the `Stmt` wrapper.

---

## 01.3 Decision Tree Type Relocation

Move decision tree types from `ori_arc` to `ori_ir/src/canon/tree.rs`. The types are pure data definitions with no logic — they belong in the shared types crate.

- [x] Move from `ori_arc/src/decision_tree/mod.rs` to `ori_ir/src/canon/tree.rs`:
  - [x] `DecisionTree` enum (`Switch`, `Leaf`, `Guard`, `Fail`)
  - [x] `ScrutineePath` = `Vec<PathInstruction>` (changed from `SmallVec` — `ori_ir` doesn't depend on `smallvec`)
  - [x] `PathInstruction` enum (`TagPayload`, `TupleIndex`, `StructField`, `ListElement`)
  - [x] `TestKind` enum (`EnumTag`, `IntEq`, `StrEq`, `BoolEq`, `FloatEq`, `IntRange`, `ListLen`)
  - [x] `TestValue` enum (`Tag`, `Int`, `Str`, `Bool`, `Float`, `IntRange`, `ListLen`)
  - [x] `FlatPattern` enum (14 variants — also relocated as shared type)
  - [x] `PatternRow`, `PatternMatrix` (also relocated)
- [x] Update `ori_arc` to re-export these types from `ori_ir` instead of defining them
  - [x] `ori_arc/src/decision_tree/mod.rs` — replaced ~800 lines with re-exports
  - [x] All `ori_arc` submodules (emit, compile, flatten) access via existing paths unchanged
- [x] No `smallvec` dependency change needed (changed `ScrutineePath` to `Vec<PathInstruction>`)
- [x] `FlatPattern::collect_bindings` made `pub` (was private, needed cross-crate from compile.rs)
- [x] All `ori_arc` tests pass after relocation (20 tests moved to `ori_ir`)
- [x] All LLVM tests pass (534 passed, 0 failed)

**Implementation note — FlatPattern/PatternRow/PatternMatrix also relocated:** The plan only mentioned the 5 core decision tree types, but `FlatPattern`, `PatternRow`, and `PatternMatrix` were also moved because `FlatPattern` depends on `PathInstruction`/`ScrutineePath` (which are now in `ori_ir`) and these types are needed by both `ori_canon` (will build pattern matrices in Section 03) and `ori_arc` (currently builds them).

---

## 01.4 ori_canon Crate Skeleton

Create the new `ori_canon` crate with module structure but no logic yet.

- [x] Create `compiler/ori_canon/Cargo.toml`
  - [x] Dependencies: `ori_ir`, `ori_types`, `ori_arc`, `rustc-hash`, `tracing`
  - [x] Add to workspace in root `Cargo.toml` (members + workspace.dependencies)
- [x] Create module structure:
  ```
  compiler/ori_canon/src/
    lib.rs          — pub mod lower, desugar, patterns, const_fold, validate
    lower.rs        — pub fn lower() -> CanonResult (stub returning empty)
    desugar.rs      — sugar elimination helpers (TODO stubs)
    patterns.rs     — pattern compilation (TODO stubs)
    const_fold.rs   — constant folding (TODO stubs)
    validate.rs     — debug_assert! validation of canonical invariants
  ```
- [x] Implement `lower()` as a stub returning `CanonResult::empty()` — actual lowering in Section 02
- [x] Implement `validate()` — full implementation walking arena, asserting all invariants:
  - [x] All `CanId` references within arena bounds
  - [x] All `CanRange` references valid (each ID in range within bounds)
  - [x] All `ConstantId` references within constant pool
  - [x] All `DecisionTreeId` references within decision tree pool
  - [x] All types resolved (not `TypeId::INFER`)
  - [x] Map entry key/value references validated
  - [x] Struct field value references validated
- [x] Re-exports: `pub use lower::lower; pub use validate::validate;`
- [x] Convenience re-exports: all canonical IR types from `ori_ir::canon`

---

## 01.5 Completion Checklist

- [x] `CanExpr` enum defined with 46 variants, = 24 bytes
- [x] `CanNode` struct with kind + span + resolved type
- [x] `CanId`, `CanRange`, `CanArena` following existing Ori patterns
- [x] `ConstantPool` and `DecisionTreePool` defined
- [x] `CanonResult` struct defined (Salsa-compatible)
- [x] Decision tree types relocated from `ori_arc` to `ori_ir`
- [x] `ori_canon` crate created with module skeleton
- [x] `ori_arc` updated to import from `ori_ir` (tests pass)
- [x] `./test-all.sh` passes — 8,322 tests, 0 failures
- [x] `./clippy-all.sh` passes — workspace + LLVM clean
- [x] WASM playground build passes

**Exit Criteria:** All canonical IR types exist and compile. Decision tree types are in `ori_ir` (shared). `ori_canon` crate exists with stub lowering function. No behavior changes — this is pure infrastructure. ✅ Met (2026-02-09)

### Files Created/Modified

| File | Action | Purpose |
|------|--------|---------|
| `compiler/ori_ir/src/canon/mod.rs` | Created | Core canonical IR types (48 tests) |
| `compiler/ori_ir/src/canon/tree.rs` | Created | Decision tree types (20 tests) |
| `compiler/ori_ir/src/lib.rs` | Modified | Added `pub mod canon` |
| `compiler/ori_ir/src/arena.rs` | Modified | Made `to_u32`/`to_u16` `pub(crate)` |
| `compiler/ori_arc/src/decision_tree/mod.rs` | Modified | Replaced definitions with re-exports |
| `compiler/ori_canon/Cargo.toml` | Created | Crate configuration |
| `compiler/ori_canon/src/lib.rs` | Created | Module declarations + re-exports |
| `compiler/ori_canon/src/lower.rs` | Created | Stub (Section 02) |
| `compiler/ori_canon/src/desugar.rs` | Created | Stub (Section 02.3) |
| `compiler/ori_canon/src/patterns.rs` | Created | Stub (Section 03) |
| `compiler/ori_canon/src/const_fold.rs` | Created | Stub (Section 04) |
| `compiler/ori_canon/src/validate.rs` | Created | Full validation implementation |
| `Cargo.toml` | Modified | Workspace members + deps |
