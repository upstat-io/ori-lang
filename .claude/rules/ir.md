---
paths:
  - "**ori_ir**"
---

# IR (AST)

## Arena Allocation
- `ExprId(u32)` indices, not `Box<Expr>`
- Flat `Vec` storage, child references use indices
- Pre-allocate: ~1 expr per 20 bytes source

## ID Newtypes
- `#[repr(transparent)]` wrapper around `u32`
- Derive: `Copy, Clone, Eq, PartialEq, Hash, Debug`
- Sentinel: `INVALID = u32::MAX`, `.is_valid()`

## TypeId Layout (aligned with ori_types::Idx)
- Flat u32 index (no sharding)
- Primitives 0-11 match Idx: INT=0..ORDERING=11
- Markers: INFER=12, SELF_TYPE=13 (not stored in type pool)
- VOID is alias for UNIT (index 6)
- Compounds start at FIRST_COMPOUND=64

## Range Types
- `ExprRange { start: u32, len: u16 }` = 8 bytes
- `define_range!` macro: `.new()` `.is_empty()` `.len()` `EMPTY`

## Span
- 8 bytes: `start: u32, end: u32` | `Span::DUMMY` for generated code

## Name Interning
- `Name(u32)` with sharded layout | `Name::EMPTY` at (shard=0, local=0)

## Visitor
- `Visitor<'ast>` trait + `walk_*()` functions
- Visitor mutates own state; AST immutable

## DerivedTrait (Source of Truth)
- `derives/mod.rs` defines `DerivedTrait` -- canonical list of all derivable traits
- Current variants: Eq, Clone, Hashable, Printable, Debug, Default, Comparable
- **Sync points** (all must update when adding a variant):
  - `ori_types/check/registration/` -- trait + impl registration
  - `ori_eval/interpreter/derived_methods.rs` -- runtime dispatch
  - `ori_eval/derives/mod.rs` -- derive processing pipeline
  - `ori_llvm/codegen/derive_codegen.rs` -- LLVM IR generation
- **DO NOT** modify without updating all sync points | see CLAUDE.md "Adding a New Derived Trait"

## Tracing
- `ori_ir` is a data structure crate -- no direct tracing | debug through consuming crates
- Phase dumps: `ORI_DUMP_AFTER_PARSE=1` (AST) | `ORI_DUMP_AFTER_TYPECK=1` (typed IR) | `ORI_DUMP_AFTER_ARC=1` (ARC IR) | `ORI_DUMP_AFTER_LLVM=1` (LLVM IR)
- For LLVM IR debugging (especially derive codegen), see llvm.md

## Key Files
- `arena/`: ExprArena, ranges
- `type_id/`: TypeId (parser-level type index, aligned with Idx)
- `name/`: Name interning
- `ast/`: AST node definitions
- `visitor/`: Visitor trait (`mod.rs`) + `walk_expr` expression walker (`walk_expr.rs`)
- `derives/`: DerivedTrait enum (source of truth for all derivable traits)
- `builtin_methods/`: Built-in method name constants
