---
title: "ARC IR"
description: "Ori Compiler Design — ARC IR Definitions and Type Classification"
order: 901
section: "ARC System"
---

# ARC IR

The ARC IR is a basic-block intermediate representation used by all ARC analysis
passes: borrow inference, RC insertion, RC elimination, and constructor reuse.
It is lowered from the canonical IR (typed expression tree with implicit control
flow) into explicit basic blocks with SSA-like variables.

The design follows the same structural pattern as LLVM IR, Lean 4's LCNF, and
Rust's MIR: functions contain blocks, blocks contain sequential instructions and
a terminator, and values are referenced by SSA variable IDs.

**Source**: `compiler/ori_arc/src/ir/mod.rs`, `compiler/ori_arc/src/ir/instr.rs`,
`compiler/ori_arc/src/ir/repr.rs`, `compiler/ori_arc/src/classify/mod.rs`,
`compiler/ori_arc/src/lib.rs`

## ARC IR Types

### ArcFunction

A complete function body in the ARC IR. Contains everything needed for ARC
analysis: the function signature with ownership annotations, basic blocks, and
metadata mapping variables back to types and source spans.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `Name` | The function's mangled name (interned) |
| `params` | `Vec<ArcParam>` | Function parameters with ownership annotations |
| `return_type` | `Idx` | The return type (Pool index) |
| `blocks` | `Vec<ArcBlock>` | Basic blocks in definition order |
| `entry` | `ArcBlockId` | The entry block ID (`blocks[entry.index()]` is entry) |
| `var_types` | `Vec<Idx>` | Type of each variable, indexed by `ArcVarId::index()` |
| `var_reprs` | `Vec<ValueRepr>` | Machine-level representation per variable (populated by `compute_var_reprs`) |
| `spans` | `Vec<Vec<Option<Span>>>` | Source spans indexed by `[block_index][instr_index]`; `None` for synthetic instructions |
| `is_fbip` | `bool` | Whether annotated `#fbip` for constructor-reuse enforcement |
| `num_captures` | `usize` | Number of leading parameters that are captures (0 for top-level functions) |

`ArcFunction` provides `fresh_var(ty)` and `fresh_var_repr(ty, repr)` for passes
that introduce synthetic variables, and `push_block(block)` for appending blocks
post-lowering. The `var_reprs` field is empty after lowering and populated by
`compute_var_reprs` at the start of the ARC pipeline.

### ArcBlock

A basic block: optional parameters (for phi-like merge values), a sequential
instruction body, and a terminator.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `ArcBlockId` | This block's identifier |
| `params` | `Vec<(ArcVarId, Idx)>` | Block parameters passed from predecessors via `Jump` |
| `body` | `Vec<ArcInstr>` | Sequential instructions executed in order |
| `terminator` | `ArcTerminator` | How control leaves this block |

Block parameters are the ARC IR's phi-node mechanism. When mutable variables
diverge across branches (if/else, match), merge blocks receive the divergent
values as block parameters, and the `Jump` terminators from each branch pass
their version as arguments.

### ArcVarId

A `#[repr(transparent)]` newtype over `u32`. Identifies a unique SSA-like value
within a single `ArcFunction`. IDs are allocated sequentially starting from 0.
Provides `raw() -> u32` and `index() -> usize` for indexing into parallel arrays
like `var_types` and `var_reprs`.

### ArcBlockId

A `#[repr(transparent)]` newtype over `u32`. Identifies a basic block within a
single `ArcFunction`. IDs are allocated sequentially starting from 0.

### ArcParam

A function parameter annotated with ownership.

| Field | Type | Description |
|-------|------|-------------|
| `var` | `ArcVarId` | The variable ID bound to this parameter |
| `ty` | `Idx` | The parameter's type in the Pool |
| `ownership` | `Ownership` | `Owned` or `Borrowed` (set by borrow inference) |

All parameters start as `Ownership::Owned` during lowering and are refined to
`Borrowed` by the borrow inference pass.

## Instruction Set (ArcInstr)

All instructions are defined in `compiler/ori_arc/src/ir/instr.rs`. Most
produce a value bound to a `dst` variable. RC operations are inserted by
the RC insertion pass and optimized by subsequent passes.

| Instruction | Fields | Semantics |
|-------------|--------|-----------|
| `Let` | `dst`, `ty`, `value: ArcValue` | Bind a value (Var, Literal, or PrimOp) to a variable |
| `Apply` | `dst`, `ty`, `func`, `args`, `arg_ownership` | Direct function call |
| `ApplyIndirect` | `dst`, `ty`, `closure`, `args` | Call through a closure fat pointer (`{fn_ptr, env_ptr}`) |
| `PartialApply` | `dst`, `ty`, `func`, `args` | Capture args into a closure object |
| `Project` | `dst`, `ty`, `value`, `field: u32` | Extract a field from a struct/enum/tuple |
| `Construct` | `dst`, `ty`, `ctor: CtorKind`, `args` | Build a struct, enum variant, tuple, list, map, set, or closure |
| `RcInc` | `var`, `count: u32`, `strategy: RcStrategy` | Increment reference count (`count` allows batched increments) |
| `RcDec` | `var`, `strategy: RcStrategy` | Decrement reference count and free if zero |
| `IsShared` | `dst`, `var` | Test whether `var`'s refcount > 1 (result is `bool`) |
| `Set` | `base`, `field: u32`, `value` | In-place field mutation (only valid when uniquely owned) |
| `SetTag` | `base`, `tag: u64` | In-place enum tag update (only valid when uniquely owned) |
| `Reset` | `var`, `token` | Intermediate: mark a value for potential reuse (expanded by Section 09) |
| `Reuse` | `token`, `dst`, `ty`, `ctor`, `args` | Intermediate: construct using a reuse token's memory (expanded by Section 09) |
| `Select` | `dst`, `ty`, `cond`, `true_val`, `false_val` | Conditional value selection (maps to LLVM `select`) |

**Value-producing instructions** (`Let`, `Apply`, `ApplyIndirect`, `PartialApply`,
`Project`, `Construct`, `IsShared`, `Reuse`, `Select`) return `Some(dst)` from
`defined_var()`. `Reset` returns `Some(token)`. Side-effect-only instructions
(`RcInc`, `RcDec`, `Set`, `SetTag`) return `None`.

### ArcValue

The right-hand side of `Let` instructions. Side-effect-free (except for
primitive operations that may trap on overflow).

- `Var(ArcVarId)` -- reference to an existing variable
- `Literal(LitValue)` -- a literal constant (`Int`, `Float`, `Bool`, `String`, `Char`, `Duration`, `Size`, `Unit`)
- `PrimOp { op: PrimOp, args }` -- a primitive operation (wraps `BinaryOp`/`UnaryOp` from `ori_ir`)

### CtorKind

Distinguishes the kind of constructor for a `Construct` instruction:

- `Struct(Name)` -- named struct (`Point { x: 1, y: 2 }`)
- `EnumVariant { enum_name, variant: u32 }` -- enum variant by index
- `Tuple` -- tuple (`(1, "hello")`)
- `ListLiteral` -- list literal (`[1, 2, 3]`)
- `MapLiteral` -- map literal (`{"a": 1}`)
- `SetLiteral` -- set literal (`{1, 2, 3}`)
- `Closure { func }` -- closure capture (packages captured variables)

### ArgOwnership

Per-argument ownership at a call site. Mirrors `Ownership` but scoped to a
specific `Apply` or `Invoke` instruction rather than a function signature.
Populated by the RC insertion pass.

- `Owned` -- callee consumes; caller emits `RcInc` if live-after
- `Borrowed` -- callee borrows; caller must `RcDec` at last use

## Terminators (ArcTerminator)

Every block ends with exactly one terminator. Terminators reference successor
blocks by `ArcBlockId`.

| Terminator | Fields | Semantics |
|------------|--------|-----------|
| `Return` | `value` | Return a value from the function |
| `Jump` | `target`, `args` | Unconditional jump, passing arguments as block parameters (phi merge) |
| `Branch` | `cond`, `then_block`, `else_block` | Conditional branch on a boolean |
| `Switch` | `scrutinee`, `cases: Vec<(u64, ArcBlockId)>`, `default` | Multi-way branch on an integer discriminant |
| `Invoke` | `dst`, `ty`, `func`, `args`, `arg_ownership`, `normal`, `unwind` | Call that may unwind; success jumps to `normal`, panic jumps to `unwind` |
| `Resume` | (none) | Resume unwinding (re-raise after cleanup) |
| `Unreachable` | (none) | Marks a block as provably unreachable |

Terminators provide `used_vars()`, `uses_var(target)`, and
`substitute_var(old, new)` for liveness analysis and reuse expansion.

## Type Classification (ArcClass)

Three-way classification that drives all RC decisions. Defined in
`compiler/ori_arc/src/lib.rs`.

| Class | RC Behavior | Examples |
|-------|-------------|---------|
| `Scalar` | No RC needed; register-width value | `int`, `float`, `bool`, `char`, `byte`, `unit`, `never`, `duration`, `size`, `ordering`, `Option<int>`, `(int, float)` |
| `DefiniteRef` | Always needs RC; contains a heap pointer | `str`, `[T]`, `{K: V}`, `Set<T>`, `Channel<T>`, `(P) -> R`, `Option<str>`, `(int, str)` |
| `PossibleRef` | Conservative fallback; might need RC | Unresolved type variables before monomorphization |

Classification is **monomorphized**: it operates on concrete types after type
parameter substitution. `PossibleRef` should never appear after monomorphization;
encountering it post-mono is a compiler bug.

**Misclassification is catastrophic:**
- `Scalar` as `DefiniteRef` produces unnecessary RC ops (performance bug)
- `DefiniteRef` as `Scalar` produces missing RC ops (use-after-free or leak)

### ArcClassifier

The `ArcClassifier` wraps a `Pool` reference with a memoization cache
(`RefCell<FxHashMap<Idx, ArcClass>>`) and a cycle-detection set
(`RefCell<FxHashSet<Idx>>`). It implements the `ArcClassification` trait.

**Fast path**: pre-interned primitives (Pool indices 0-11: `int`, `float`,
`bool`, `char`, `byte`, `unit`, `never`, `error`, `duration`, `size`,
`ordering`, `str`) are classified by raw index without hash map lookup.
`str` (index 11) classifies as `DefiniteRef`; all others as `Scalar`.

**Transitive classification**: compound types are classified by their children.
If ANY child is `DefiniteRef`, the compound is `DefiniteRef`. If ANY child is
`PossibleRef` (and none is `DefiniteRef`), the compound is `PossibleRef`.
Otherwise the compound is `Scalar`.

**Cycle detection**: if classification encounters an `Idx` already in the
`classifying` set, the type is recursive and requires heap indirection,
producing `DefiniteRef`.

**Tag-specific rules**: `Iterator` and `DoubleEndedIterator` are classified as
`Scalar` because they are runtime-managed (heap-allocated with `Box::new`, not
`ori_rc_alloc`, so they lack RC headers). `Named`, `Applied`, and `Alias` tags
resolve through `Pool::resolve_fully()` and recursively classify the resolved
type.

## Value Representation (ValueRepr)

Bridges `ArcClass` to LLVM concrete types. Computed once per variable by
`compute_var_reprs` at the start of the ARC pipeline. Defined in
`compiler/ori_arc/src/ir/repr.rs`.

| Repr | Layout | Examples |
|------|--------|---------|
| `Scalar` | Register-width, no RC | `int`, `float`, `bool`, `char`, `byte`, `unit` |
| `RcPointer` | Single heap pointer | `[T]`, `{K: V}`, `Set<T>`, `Channel<T>`, `Iterator` |
| `Aggregate` | Multi-field value | `(T, U)` tuple, struct, enum, `Result<T, E>`, `Option<T>` |
| `FatValue` | Two-word value (ptr + metadata) | `str` (ptr + len), closure (`fn_ptr` + `env_ptr`) |

`ValueRepr::from_arc_class(class, pool, idx)` derives the representation: for
`Scalar` classes, the result is always `Scalar`. For ref-containing classes, the
Pool tag disambiguates via `from_ref_tag()`:
- `Tag::Str | Tag::Function` produce `FatValue`
- `Tag::Tuple | Tag::Struct | Tag::Enum | Tag::Result | Tag::Option` produce `Aggregate`
- `Tag::List | Tag::Map | Tag::Set | Tag::Channel | Tag::Iterator | ...` produce `RcPointer`

## RC Strategy (RcStrategy)

Refines `ValueRepr` for RC operations. Computed during RC insertion from
`ValueRepr` + Pool structure. Embedded in `RcInc`/`RcDec` instructions so the
LLVM emitter can pattern-match directly without Pool queries at emission time.

| Strategy | Inc Behavior | Dec Behavior |
|----------|-------------|-------------|
| `HeapPointer` | `ori_rc_inc(data_ptr)` | `ori_rc_dec(data_ptr, drop_fn)` |
| `FatPointer` | Extract field 1, `ori_rc_inc(data_ptr)` | Extract field 1, `ori_rc_dec(data_ptr, drop_fn)` |
| `Closure` | Extract `env_ptr`, null check, `ori_rc_inc(env_ptr)` | Extract `env_ptr`, null check, load `drop_fn`, `ori_rc_dec(env_ptr, drop_fn)` |
| `AggregateFields` | Traverse RC fields, Inc each recursively | Call generated drop function that traverses fields |
| `InlineEnum` | **No-op** (stack-allocated container) | Tag-switch, per-variant field Dec |

`RcStrategy::from_var(repr, pool, ty)` computes the strategy:
- `RcPointer` maps to `HeapPointer`
- `FatValue` maps to `Closure` (for `Tag::Function`) or `FatPointer` (for `Tag::Str`)
- `Aggregate` maps to `InlineEnum` (for `Tag::Result | Tag::Enum | Tag::Option`) or `AggregateFields`
- Calling on `Scalar` is a debug assertion failure (scalars never get RC ops)

## ARC IR vs AST Comparison

| Property | Canonical IR (AST) | ARC IR |
|----------|-------------------|--------|
| Control flow | Implicit (nested if/match/loop expressions) | Explicit basic blocks with terminators |
| Names | Scoped lexical names (`Name`) | SSA variables (`ArcVarId`) |
| Phi nodes | N/A (expression nesting handles merge) | Block parameters on `Jump` |
| Mutable variables | Rebinding in scope | Fresh `ArcVarId` per assignment; merge via block params |
| Function calls | Nested expression | `Apply` (direct), `ApplyIndirect` (closure), `Invoke` (may-unwind) |
| RC operations | None (implicit in value semantics) | Explicit `RcInc`/`RcDec` with strategy |
| Reuse | None | `Reset`/`Reuse` intermediates, expanded to `IsShared` + conditional |
| Spans | Per-expression | Per-instruction (`None` for synthetic ops) |
| Types | Per-expression via arena | Parallel `var_types` array indexed by `ArcVarId` |
| Ownership | None | Per-parameter `Ownership`, per-argument `ArgOwnership` |
