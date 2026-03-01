---
title: "Lowering"
description: "Ori Compiler Design — CanExpr to ARC IR Lowering"
order: 902
section: "ARC System"
---

# Lowering

The lowering pass converts canonical IR (typed expression tree with implicit
control flow) into ARC IR (explicit basic blocks with SSA variables). This is
the first step of the ARC pipeline and produces the IR that all subsequent
passes (borrow inference, RC insertion, RC elimination, constructor reuse)
operate on.

**Source**: `compiler/ori_arc/src/lower/`

## Overview

The entry point is `lower_function_can()`, which takes a canonical IR body
(`CanId` + `CanonResult`) and produces an `ArcFunction` plus any lambda bodies
as additional `ArcFunction`s.

```
lower_function_can(name, params, return_type, body, canon, ...)
    -> (ArcFunction, Vec<ArcFunction>)
```

The returned tuple contains:
- The main function's ARC IR
- A `Vec<ArcFunction>` of all lambda bodies extracted during lowering

Each lambda body becomes a separate `ArcFunction` with captured variables as
leading parameters and user parameters following. The call site emits
`PartialApply` to pack the captured outer variables into a closure.

## Key Components

### ArcIrBuilder

Owns the in-progress function state during lowering: blocks, variables, and
the current insertion point. Follows the same "position at a block, emit
instructions, terminate" pattern as LLVM's `IRBuilder`.

**Source**: `compiler/ori_arc/src/lower/builder.rs`

Key operations:
- `new()` -- creates a builder with an entry block (block 0) already allocated
- `new_block()` -- allocates an empty block and returns its `ArcBlockId`
- `position_at(block)` -- sets the current insertion point
- `fresh_var(ty)` -- allocates a new `ArcVarId` with the given type
- `add_block_param(block, ty)` -- adds a block parameter (for phi merge)
- `emit_let()`, `emit_apply()`, `emit_construct()`, etc. -- emit instructions
- `emit_invoke(ty, func, args, span)` -- emits an `Invoke` terminator with normal and unwind continuations, positions the builder at the normal block
- `terminate_return()`, `terminate_jump()`, `terminate_branch()`, `terminate_switch()` -- set block terminators
- `finish(name, params, return_type, entry, is_fbip)` -- consumes the builder and produces the final `ArcFunction`

The builder uses block parameters instead of phi nodes for SSA merge. This is
the key structural difference from LLVM IR, where phi nodes are instructions
at the start of a block. In ARC IR, blocks declare parameters and `Jump`
terminators pass arguments.

The `finish()` method validates that every block has a terminator. Unterminated
blocks receive `Unreachable` as a fallback (with a tracing warning).

### ArcLowerer

Walks the canonical expression tree and emits ARC IR instructions via the
builder. Borrows the builder and all contextual data needed to lower each
expression variant.

**Source**: `compiler/ori_arc/src/lower/expr/mod.rs`

Key fields:
- `builder` -- mutable reference to the `ArcIrBuilder`
- `arena` -- the canonical expression arena (`CanArena`)
- `canon` -- the full canonicalization result (`CanonResult`), including decision trees and constants
- `interner` -- string interner for resolving `Name` values
- `pool` -- the type pool for type queries
- `scope` -- current `ArcScope` for name-to-variable bindings
- `loop_ctx` -- optional `LoopContext` for `break`/`continue` targets
- `problems` -- accumulator for lowering diagnostics
- `lambdas` -- accumulator for extracted lambda bodies
- `hash_length` -- resolved `#` (hash length) value for index expressions
- `block_let_names` -- names freshly `let`-bound in the current block (for shadow vs reassignment tracking)
- `variant_ctors` -- reverse lookup from variant name to `(enum_name, variant_index, field_count)`
- `type_subst` -- optional type substitution map for monomorphized generic functions

The core method is `lower_expr(id: CanId) -> ArcVarId`, which dispatches on
the `CanExpr` variant and returns the SSA variable holding the result.

### ArcScope

Tracks `Name` to `ArcVarId` bindings with mutable variable tracking for SSA
merge.

**Source**: `compiler/ori_arc/src/lower/scope/mod.rs`

- `bind(name, var)` -- bind an immutable variable
- `bind_mutable(name, var)` -- bind a mutable variable and track it for SSA merge
- `lookup(name)` -- resolve a name to its current `ArcVarId`
- `is_mutable(name)` -- check whether a name refers to a mutable variable
- `mutable_bindings()` -- iterate over all mutable bindings (name, current var)

Child scopes are created via `clone()`. The `merge_mutable_vars()` free
function compares branch scopes against a pre-branch snapshot to find
mutable variables that were reassigned, and adds block parameters to the
merge block for each divergent variable.

### VariantCtors

A `FxHashMap<Name, (Name, u32, usize)>` mapping variant name to
`(enum_name, variant_index, field_count)`. Built once per function from the
Pool's enum type data by `build_variant_ctors()`, then shared by reference
with the expression lowerer and any inner lambda lowerers.

Used to intercept enum variant constructor calls and emit `Construct`
instructions instead of function calls.

## Submodule Organization

| Submodule | What it lowers |
|-----------|---------------|
| `expr/` | Expression dispatch: the main `match` on `CanExpr` variants. Handles literals, identifiers, binary/unary ops, and routes to specialized submodules. |
| `calls/` | Function calls (direct, method, indirect) and the nounwind classification that determines `Apply` vs `Invoke`. |
| `calls/lambda.rs` | Lambda expression lowering: capture analysis, body extraction into separate `ArcFunction`, and `PartialApply` emission. |
| `collections/` | Tuple, list, map, set, struct, enum construction. Field access (`Project`), index access, range literals. Also handles `Ok`/`Err`/`Some`/`None`, `try` (`?`), and casts. |
| `constructs.rs` | Special expression forms: `FunctionExp` dispatch (`print`, `panic`, `todo`, `unreachable`, `recurse`, `cache`, `catch`), and `FormatWith` (template string format specs). |
| `control_flow/` | Block expressions, `let` bindings, `if`/`else`, `match`, `break`, `continue`, assignment. These are the expression variants that create multiple basic blocks. |
| `control_flow/for_loops.rs` | `for`-`in` loop lowering (imperative iteration with `do`). |
| `control_flow/for_yield.rs` | `for`-`yield` (list comprehension) lowering. |
| `control_flow/loops.rs` | Infinite `loop` lowering with `break`/`continue` support. |
| `patterns/` | Pattern binding destructuring for `let` and `for` expressions. Handles simple names, tuples, structs, enum variants, wildcards, and nested patterns. Match pattern compilation uses the separate decision tree pipeline. |
| `scope/` | `ArcScope` name bindings and the `merge_mutable_vars` function for SSA merge at control flow join points. |

## SSA Form and Mutable Variables

Ori's ARC IR uses SSA form for all values. Immutable `let` bindings map
directly to a single `ArcVarId`. Mutable variables require special handling.

### Rebinding

Each assignment to a mutable variable creates a fresh `ArcVarId`. The scope
tracks the *current* SSA variable for each mutable name. Reading a mutable
variable looks up the current binding; writing creates a new one.

```
let x = 1       // scope: x -> v0
x = x + 1       // scope: x -> v1 (fresh ArcVarId)
x = x * 2       // scope: x -> v2 (fresh ArcVarId)
```

### Merge at Join Points

At control flow join points (if/else, match, loops), mutable variables that
were reassigned in divergent branches must be merged. The merge produces a
fresh `ArcVarId` via a block parameter on the merge block.

The process:
1. Before the branch, snapshot the scope (`pre_scope = scope.clone()`)
2. Lower each branch with a clone of the pre-scope
3. Call `merge_mutable_vars(builder, merge_block, pre_scope, branch_scopes, var_types)`
4. For each mutable variable where ANY branch changed the `ArcVarId`, add a block parameter to the merge block
5. Each branch's `Jump` to the merge block passes its version of the variable as an argument
6. After the merge, rebind each merged variable to its merge-block parameter

This SSA merge pattern is used consistently by `lower_if`, `lower_match`,
and the loop lowerers.

### Block Scope Propagation

Block expressions (`{ let x = 1; ... }`) create child scopes. Local `let`
bindings die with the block, but mutable variable reassignments (`x = expr`)
propagate to the parent scope. The `block_let_names` set tracks which names
were freshly introduced by `let` in the current block to distinguish shadows
from reassignments.

## Lambda Lowering

Lambda bodies are extracted into separate `ArcFunction`s. The process:

1. **Capture analysis**: `collect_captures()` walks the lambda body's canonical
   expression tree. For each `Ident(name)` not in the lambda's parameter list
   and present in the outer scope, it records `(name, outer_arc_var_id)`.
   A `HashSet` prevents duplicate captures.

2. **Build lambda function**: A new `ArcIrBuilder` and `ArcScope` are created.
   Capture parameters are added first (with the outer variable's type), followed
   by user parameters (with types from the Pool's function type data). A nested
   `ArcLowerer` lowers the body expression.

3. **Name assignment**: The lambda receives a unique name (`__lambda_N` where N
   is the index into the lambdas accumulator). The `num_captures` field on the
   resulting `ArcFunction` records how many leading parameters are captures.

4. **PartialApply at call site**: Back in the outer lowerer, a `PartialApply`
   instruction is emitted with the lambda name and the outer capture variable
   IDs as arguments. This creates the closure object.

Nested lambdas are handled recursively: the inner lambda lowerer shares the
same `lambdas` accumulator, so deeply nested lambdas all end up in the same
flat list.

## Call Lowering

Function calls are classified into three categories:

### Direct Calls (Apply / Invoke)

When the callee is a `FunctionRef` or an unbound `Ident` (top-level function),
the lowerer checks if it is a variant constructor (emits `Construct`) or a
regular function. Regular functions are further classified:

- **Nounwind**: runtime functions (`ori_*`) and compiler helpers (`__*`) are
  known to never unwind. These emit `Apply` instructions.
- **May-unwind**: user-defined functions may panic. These emit `Invoke`
  terminators with normal and unwind continuation blocks. The unwind block
  is terminated with `Resume` (or `Jump` to a catch handler inside
  `catch(expr:)` blocks).

### Indirect Calls (ApplyIndirect)

When the callee is a local variable holding a closure (or any non-name
expression), the lowerer emits `ApplyIndirect` with the closure variable.

### Inline Tag Checks

Calls to `is_ok`, `is_err`, `is_some`, `is_none` are lowered inline as
`Project(tag field) == constant` rather than function calls, because these
are compiled inline by LLVM codegen and do not participate in Perceus
ownership transfer.

## Control Flow Lowering

### If/Else

Produces four blocks: entry (condition evaluation), then, else, merge.
The condition is evaluated in the entry block, which terminates with `Branch`.
Each branch lowers its body in a clone of the pre-branch scope. At the merge
block, SSA merge adds block parameters for divergent mutable variables plus
the result value. Both branches `Jump` to the merge block with their
result and mutable variable values.

### Match

Uses pre-compiled decision trees from the canonicalization pass. The
`DecisionTree` is read from `CanonResult.decision_trees` and walked by
`decision_tree::emit::emit_tree()` to produce ARC IR blocks. Mutable variable
merge follows the same pattern as if/else, with merge-block parameters for
each mutable variable in scope.

### Loops

Infinite loops (`loop { body }`) produce a header block and an exit block.
The header block receives mutable variable values as parameters. `break`
jumps to the exit block; `continue` jumps back to the header. The
`LoopContext` struct tracks exit/continue targets and the ordered list of
mutable variable names for consistent block-parameter ordering.

### For Loops

`for`-`in` loops lower the iterator expression, then emit a loop structure
that calls `next()` on each iteration, binds the pattern, evaluates the body,
and jumps back to the header. `for`-`yield` (list comprehensions) builds a
list by appending each yielded value.

## Pattern Binding

Pattern destructuring for `let` and `for` bindings is handled by
`bind_pattern()` in `patterns/mod.rs`. It walks the `CanBindingPattern`
tree and emits `Project` instructions to extract fields, binding each
extracted variable in the scope.

- Simple name patterns bind directly
- Tuple/struct patterns emit `Project` for each field
- Nested patterns recurse
- Wildcard patterns (`_`) are skipped
- Mutable bindings use `scope.bind_mutable()`; immutable use `scope.bind()`
- For-loop patterns are always immutable (via `bind_for_pattern()`)

Match pattern compilation uses the separate decision tree pipeline
(`decision_tree::flatten` -> `decision_tree::compile` -> `decision_tree::emit`),
not the binding pattern infrastructure.

## Key Invariants

1. **All parameters start as `Ownership::Owned`**: refined by borrow inference
   in a later pass. The lowerer does not make ownership decisions.

2. **Synthetic instructions have `None` spans**: the `spans` parallel array
   tracks source locations per instruction. Instructions inserted by the
   lowerer that have no direct source correspondence use `None`.

3. **Value representations populated immediately after lowering**: both the
   main function and all lambda bodies get `var_reprs` computed via
   `compute_var_reprs` at the end of `lower_function_can()`, before returning
   to the caller. The ARC pipeline re-computes them as a consistency check.

4. **Lambda bodies returned separately, not inlined**: the caller receives the
   flat list of all lambda `ArcFunction`s and is responsible for running the
   ARC pipeline on each one independently.

5. **Unterminated blocks receive `Unreachable`**: if a block is not terminated
   during lowering (e.g., dead code after `break`), the builder's `finish()`
   method adds `Unreachable` as a fallback with a tracing warning.

6. **`VariantCtors` built once per function**: the reverse lookup from variant
   name to enum constructor info is computed from the Pool at the start of
   lowering and shared immutably with all nested lowerers.

7. **Block-parameter order is deterministic**: mutable variable merge uses
   `Vec` (not `HashMap`) for the mutable variable list in `LoopContext`, and
   `merge_mutable_vars` iterates the pre-scope's mutable bindings in a
   consistent order. This ensures `Jump` argument order matches
   `add_block_param` order.

8. **Type substitution for generics**: when lowering a monomorphized function,
   `type_subst` maps generic `Idx` to concrete `Idx`. The `expr_type()` method
   applies this substitution transparently, so all emitted instructions carry
   concrete types even when the canonical IR uses generic ones.
