---
paths:
  - "**/ori_ir/**"
---

**NO WORKAROUNDS/HACKS/SHORTCUTS.** Proper fixes only. When unsure, STOP and ask. Fact-check against spec. Consult `~/projects/reference_repos/lang_repos/` (includes Swift for ARC, Koka for effects, Lean 4 for RC).

**Ori tooling is under construction** — bugs are usually in compiler, not user code. This is one system: every piece must fit for any piece to work. Fix every issue you encounter — no "unrelated", no "out of scope", no "pre-existing." If it's broken, research why and fix it.

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
- `define_range!` macro: `.new()`, `.is_empty()`, `.len()`, `EMPTY`

## Span
- 8 bytes: `start: u32, end: u32`
- `Span::DUMMY` for generated code

## Name Interning
- `Name(u32)` with sharded layout
- `Name::EMPTY` at (shard=0, local=0)

## Visitor
- `Visitor<'ast>` trait + `walk_*()` functions
- Visitor mutates own state; AST immutable

## Debugging / Tracing

The `ori_ir` crate does not use tracing directly (it's a data structure crate). Debug IR issues through consuming crates:

```bash
ORI_LOG=oric=debug ori check file.ori               # See Salsa query flow (tokens→parsed→typed)
ORI_LOG=ori_types=trace ori check file.ori          # See how IR nodes are consumed by type checker
ORI_LOG=ori_eval=trace ori run file.ori             # See how IR nodes are consumed by evaluator
```

### Phase Dumps (inspect IR at each stage)
```bash
ORI_DUMP_AFTER_PARSE=1 ori check file.ori           # AST after parsing
ORI_DUMP_AFTER_TYPECK=1 ori check file.ori          # Typed IR after type checking
ORI_DUMP_AFTER_ARC=1 ori build file.ori             # ARC IR with RC strategies
ORI_DUMP_AFTER_LLVM=1 ori build file.ori            # Annotated LLVM IR
```

**Tips**:
- TypeId mismatch? Check `type_id.rs` alignment with `ori_types::Idx` (primitives 0-11 must match)
- Wrong ExprId? Use `ori_types=trace` to see which expression IDs the type checker processes
- Arena issue? Add temporary `tracing::debug!` calls to the IR code you're debugging

## DerivedTrait (Source of Truth)

`derives/mod.rs` defines `DerivedTrait` — the **canonical list of all derivable traits**. This enum is consumed by 4 downstream crates. It is the single source of truth.

**Current variants**: Eq, Clone, Hashable, Printable, Debug, Default, Comparable

**Sync points** (all must be updated when adding a variant):
- `ori_types/check/registration/` — trait + impl registration
- `ori_eval/interpreter/derived_methods.rs` — runtime dispatch
- `ori_eval/derives/mod.rs` — derive processing pipeline
- `ori_llvm/codegen/derive_codegen.rs` — LLVM IR generation

**DO NOT** modify `DerivedTrait` without updating all sync points. See CLAUDE.md "Adding a New Derived Trait" checklist.

## Key Files
- `arena/`: ExprArena, ranges
- `type_id/`: TypeId (parser-level type index, aligned with Idx)
- `name/`: Name interning
- `ast/`: AST node definitions
- `visitor.rs`: Visitor trait
- `derives/`: DerivedTrait enum (source of truth for all derivable traits)
- `builtin_methods/`: Built-in method name constants

## LLVM Debugging

For LLVM IR debugging workflow (especially derive codegen issues that originate in `ori_ir`), see @llvm.md
