---
section: "09"
title: Constructor Reuse (FBIP)
status: complete
goal: Reuse memory from dropped constructors for new allocations, avoiding heap allocation in functional patterns
sections:
  - id: "09.1"
    title: Reuse-Eligible Ori Patterns
    status: complete
    note: Patterns identified in plan; detection handled by Section 07.6
  - id: "09.2"
    title: Reset/Reuse IR Operations
    status: complete
    note: ArcInstr::Reset, ArcInstr::Reuse defined in ir.rs; detection in reset_reuse.rs
  - id: "09.3"
    title: Two-Path Expansion Algorithm
    status: complete
    note: expand_reuse.rs — IsShared+Branch, fast/slow paths, merge blocks
  - id: "09.4"
    title: Projection-Increment Erasure
    status: complete
    note: erase_proj_increments() in expand_reuse.rs
  - id: "09.5"
    title: Self-Set Elimination
    status: complete
    note: is_self_set() in expand_reuse.rs
---

# Section 09: Constructor Reuse (FBIP)

**Status:** Complete (09.1-09.5 all implemented) — 208 tests in ori_arc crate.
**Goal:** When pattern matching drops a constructor and immediately constructs a new one of the same type, reuse the dropped memory. This is the "Functional But In-Place" optimization, using Lean 4's constructor-identity approach: only same-type constructors can reuse each other. This is safer and simpler than Koka's size-based approach, and correctness comes first.

**Crate:** `ori_arc` (no LLVM dependency). Operates on ARC IR basic blocks.

**Reference compilers:**
- **Lean 4** `src/Lean/Compiler/IR/ExpandResetReuse.lean` -- Explicit `reset`/`reuse` IR operations expanded into `isShared` + fast/slow paths. **We follow this approach.** Constructor-identity reuse: `reset` and `reuse` are restricted to the same type. The expansion algorithm produces a conditional branch on the refcount, with in-place mutation on the fast path and fresh allocation on the slow path.
- **Koka** `src/Backend/C/ParcReuse.hs` -- Size-based reuse with an available-pool mechanism. Forward analysis tracks reuse tokens (available allocations of known sizes). `genAllocAt` checks whether a reuse token of matching size is available; `genReuseAddress` extracts the actual address from the token. Reuse is opportunistic: any allocation of matching size can satisfy any constructor of that size. **We chose Lean 4's constructor-identity approach instead** for safety and simplicity -- size-matching across unrelated types risks subtle bugs and complicates verification.
- **Roc** `crates/compiler/mono/src/code_gen_help/mod.rs` -- `HelperOp::Reset`/`ResetRef`/`Reuse` helper operations

**Relationship to other sections:**
- **Section 06** defines `ArcInstr::Reset` and `ArcInstr::Reuse` in the ARC IR enum.
- **Section 07.6** (a post-pass after RC insertion) identifies Reset/Reuse candidates by scanning for `RcDec` followed by `Construct` of the same type, and replaces those pairs with `Reset`/`Reuse` intermediate instructions.
- **Section 09** (this section) expands `Reset`/`Reuse` into `isShared` + fast/slow conditional paths. After expansion, no `Reset` or `Reuse` instructions remain in the ARC IR. They are intermediate representations that exist only between Section 07 and Section 09.

**Example:**
```ori
// Before FBIP:
match list {
    Cons(head, tail) -> Cons(transform(head), tail)
    //  ^^^^ dropped     ^^^^ new allocation
}

// After Section 07 (reset/reuse insertion):
// reset list
// ... Cons via reuse list ...

// After Section 09 (expansion):
match list {
    Cons(head, tail) -> {
        c := isShared(list)
        if c then {
            // SLOW PATH (shared — refcount > 1)
            dec list
            inc tail          // retained because still referenced
            Cons(transform(head), tail)  // fresh allocation
        } else {
            // FAST PATH (unique — refcount == 1, common case, fall-through)
            // In-place field mutation, no allocation
            list[0] = transform(head)
            // list[1] is already tail — self-set eliminated (see 09.5)
            list  // Return same memory, no allocation!
        }
    }
}
```

**Uniqueness test convention:** We use `isShared(x)` which returns `true` when the refcount is greater than 1 (the value is shared, NOT unique). The `then` branch is the **slow path** (shared, must allocate fresh). The `else` branch is the **fast path** (unique, can reuse in-place). This means the fast/common case (unique values) is the fall-through branch, which is better for branch prediction.

**IsShared is inlined, not a runtime call.** `IsShared` compiles to: load strong_count from ptr-8 (0.1-alpha 8-byte header), `icmp sgt 1`. This is emitted as inline LLVM IR instructions, not a runtime function call. There is no `ori_rc_is_unique` runtime function.

---

## 09.1 Reuse-Eligible Ori Patterns

Constructor reuse applies when a pattern match destructures a value and an arm reconstructs a value of the **same type**. Lean 4's constructor-identity rule restricts reuse to same-type constructors only. The following concrete Ori patterns are reuse-eligible:

### Match arms reconstructing the same type

```ori
fn transform(opt: option[int]) -> option[int] =
    match opt {
        Some(x) -> Some(f(x))   // Same type: option[int] → option[int]
        None -> None             // Same type: trivial (no allocation either way)
    }
```

The `Some(x)` destructure drops the old `Some` cell. The `Some(f(x))` constructs a new one. With reuse, the old cell's memory is reused for the new `Some`.

### Recursive data structure transformations

```ori
fn map(list: List[a], f: (a) -> b) -> List[b] =
    match list {
        Cons(head, tail) -> Cons(f(head), map(tail, f))
        Nil -> Nil
    }
```

Each recursive step destructures a `Cons` cell and constructs a new `Cons` cell. With reuse, the entire list is transformed in-place when uniquely owned.

### Struct-with-spread (destructure old, construct same type)

```ori
fn move_right(p: Point) -> Point =
    Point { ...p, x: p.x + 10 }

// Desugars to: destructure p, construct Point with new x, reuse p's memory
```

The spread operation destructures the old `Point` and constructs a new one of the same type. This is a direct reuse candidate.

### Enum variant transforms (changing payload, preserving structure)

```ori
fn increment_leaf(tree: Tree) -> Tree =
    match tree {
        Leaf(n) -> Leaf(n + 1)          // Same type, same variant
        Node(l, r) -> Node(increment_leaf(l), increment_leaf(r))  // Same type, same variant
    }
```

Both arms destructure and reconstruct the same variant of the same type. Each cell can be reused in-place.

### NOT reuse-eligible (different types)

```ori
fn to_string(opt: option[int]) -> option[str] =
    match opt {
        Some(x) -> Some(str(x))   // Different type: option[int] → option[str]
        None -> None
    }
```

Even though `option[int]` and `option[str]` may have the same layout, we do NOT reuse across different concrete types. Correctness first.

- [x] Identify reuse-eligible patterns during RC insertion (Section 07)
- [x] Restrict reuse to same-type constructors (Lean 4 constructor-identity rule)
- [x] Handle spread-struct patterns as reuse candidates
- [x] Handle recursive data structure patterns (list map, tree map)

### Future work

Additional reuse-eligible patterns to investigate in future versions:

- **For-yield (list comprehension):** `for x in list yield transform(x)` creates a new list of the same type and length as the input. When the input list is uniquely owned, the output list could reuse the input's backing buffer in-place, element by element. This requires element-level reuse within the iterator loop rather than constructor-level reuse.
- **Or-patterns in match arms:** When multiple patterns in a match arm destructure the same type and all produce the same-type result, the reuse token from any matched pattern can be used. This requires the pattern compiler (Section 10) to propagate reuse tokens through or-pattern alternatives.

## 09.2 Reset/Reuse IR Operations

`Reset` and `Reuse` are **intermediate ARC IR operations** inserted by Section 07 and expanded away by Section 09. They do NOT exist alongside `RcInc`/`RcDec` in the final IR -- they are transient instructions that get replaced with concrete `isShared` + branch + field-mutation sequences during the expansion pass.

```rust
/// Intermediate ARC IR operations for constructor reuse.
///
/// These are variants of `ArcInstr` (defined in Section 06.0).
/// They are inserted by Section 07 (RC insertion) when a dec+Construct
/// pattern is detected, and expanded by Section 09 into conditional
/// fast/slow paths.
///
/// After Section 09 runs, NO Reset or Reuse instructions remain in
/// the ARC IR. They are fully expanded into Branch terminators,
/// Project/Construct/RcInc/RcDec instructions, and field mutations.
///
/// In the ArcInstr enum (Section 06.0):
ArcInstr::Reset {
    /// The variable being tested for uniqueness and prepared for reuse.
    /// Must be a heap-allocated constructor value.
    var: ArcVarId,
    /// Result variable: holds a reuse token (conceptually, "can I reuse this?").
    /// Used by the corresponding `Reuse` instruction.
    token: ArcVarId,
}

ArcInstr::Reuse {
    /// The reuse token from a preceding `Reset` instruction.
    token: ArcVarId,
    /// Destination variable for the new value.
    dst: ArcVarId,
    /// Type of the constructed value (must match the Reset'd value's type).
    ty: Idx,
    /// Which constructor to apply.
    ctor: CtorKind,
    /// Fields for the new constructor.
    args: Vec<ArcVarId>,
}
```

**Detection rule (Section 07.6):** In the post-pass after RC insertion, when the scanner encounters a `RcDec { var: x }` followed (possibly with intervening non-aliasing instructions) by `Construct { dst: y, ty, ctor, args }` where `ty` matches the type of `x`, replace the pair with:

```
Reset { var: x, token: t }
... (intervening instructions unchanged) ...
Reuse { token: t, dst: y, ty, ctor, args }
```

The `RcDec` is removed (it is subsumed by the Reset's slow-path dec).

- [x] Define `Reset` and `Reuse` as `ArcInstr` variants (Section 06.0)
- [x] Implement detection in Section 07: scan for `dec x` + `Construct same_type` patterns
- [x] Generate `Reset`/`Reuse` pairs, removing the original `RcDec`
- [x] Verify same-type constraint (constructor-identity rule)

## 09.3 Two-Path Expansion Algorithm

The core algorithm, adapted from Lean 4's `ExpandResetReuse.lean`. This pass runs after RC insertion (Section 07) and before RC elimination (Section 08). It replaces every `Reset`/`Reuse` pair with a conditional branch on the refcount.

### Expansion of `Reset`

```
Input:
    Reset { var: y, token: t }

Output:
    c := isShared(y)
    Branch { cond: c, then_block: slow_block, else_block: fast_block }
```

- `isShared(y)` returns `true` when the refcount of `y` is greater than 1.
- The **then branch** (slow path) is taken when the value is shared.
- The **else branch** (fast path, fall-through) is taken when the value is unique and reusable.

### Expansion of `Reuse` in fast path (unique)

```
Input (fast path):
    Reuse { token: t, dst: z, ty, ctor, args: [w0, w1, ...] }

Output (fast path — in-place mutation, using ArcInstr variants):
    // For each field i being replaced (not self-set, not claimed by projection erasure):
    //   If old field value is RC'd: Dec the old value before overwriting
    RcDec { var: old_field_0 }               // Dec old field 0 if RC'd and being replaced
    Set { base: y, field: 0, value: w0 }     // Overwrite field 0 of the original object y
    RcDec { var: old_field_1 }               // Dec old field 1 if RC'd and being replaced
    Set { base: y, field: 1, value: w1 }     // Overwrite field 1
    ...
    SetTag { base: y, tag: tag }             // Update tag if constructor variant differs
                                             // (uses ArcInstr::SetTag from Section 06.0)
    z := y                                   // z IS y — same memory, no allocation
```

The fast path performs in-place field mutation on the original object `y`. No allocation occurs. If the constructor variant is the same as the original, no tag update is needed. If it differs (e.g., one enum variant reusing another's memory), the tag field is overwritten.

**Additionally**, the fast path must release fields that are no longer referenced. Any field of `y` that is being replaced (not retained) and whose old value is RC'd must be decremented before the overwrite. The `RcDec` for each replaced field uses the value obtained via `Project` before the Reset. Fields that are self-set (Section 09.5) or claimed by projection-increment erasure (Section 09.4) do NOT need a Dec on the fast path. See Section 09.4 for the full claimed/unclaimed field analysis.

### Expansion of `Reuse` in slow path (shared)

```
Input (slow path):
    Reuse { token: t, dst: z, ty, ctor, args: [w0, w1, ...] }

Output (slow path — fresh allocation):
    dec y                          // Decrement the shared original
    z := Construct { ctor, args: [w0, w1, ...] }  // Fresh allocation
```

The slow path decrements the shared original (which won't reach zero because other references exist) and allocates fresh memory for the new constructor.

**Additionally**, the slow path must increment fields that were projected from `y` and are being retained. See Section 09.4 for the projection-increment erasure optimization that determines exactly which fields need `inc`.

### Complete expansion example

```
// Input ARC IR:
block_5:
    %head = Project { value: %list, field: 0 }
    %tail = Project { value: %list, field: 1 }
    %new_head = Apply { func: transform, args: [%head] }
    Reset { var: %list, token: %t }
    Reuse { token: %t, dst: %result, ty: List, ctor: Cons, args: [%new_head, %tail] }
    Jump { target: merge, args: [%result] }

// Output ARC IR (after expansion):
block_5:
    %head = Project { value: %list, field: 0 }
    %tail = Project { value: %list, field: 1 }
    %new_head = Apply { func: transform, args: [%head] }
    %c = IsShared { var: %list }
    Branch { cond: %c, then_block: slow_5, else_block: fast_5 }

fast_5:
    // In-place: overwrite field 0 (head) with %new_head.
    // Field 1 (tail) is %tail which came from Project { %list, 1 } — self-set, skipped (09.5).
    // Release old field 0 (%head) if not claimed by projection erasure (09.4).
    // In this case, %head was passed to transform and consumed — no release needed.
    Set { base: %list, field: 0, value: %new_head }
    // %list[1] already IS %tail — no write needed (self-set elimination)
    Jump { target: merge, args: [%list] }

slow_5:
    RcDec { var: %list }
    RcInc { var: %tail }       // %tail was projected from %list; after dec, need inc to keep it alive
    %result = Construct { ty: List, ctor: Cons, args: [%new_head, %tail] }
    Jump { target: merge, args: [%result] }
```

- [x] Implement `expand_reset_reuse` pass over ARC IR functions
- [x] Split each `Reset` into `IsShared` + `Branch` (then=slow, else=fast)
- [x] Expand fast-path `Reuse` into `Set` instructions (in-place field mutation)
- [x] Expand slow-path `Reuse` into `RcDec` + `Construct` (fresh allocation)
- [x] Handle tag updates for different enum variants via `ArcInstr::SetTag { base, tag }` (defined in Section 06.0)
- [x] Verify no `Reset` or `Reuse` instructions remain after expansion
- [x] Add `ArcInstr::IsShared`, `ArcInstr::Set`, and `ArcInstr::SetTag` variants for the expanded operations (defined in Section 06.0)

## 09.4 Projection-Increment Erasure

Adapted from Lean 4's `eraseProjIncFor` optimization. When fields are projected from the scrutinee before the `Reset`, some projections exist solely to feed back into the `Reuse` as retained fields. The increment for these retained fields can be avoided on the fast path (where we own the object exclusively), and must only be added on the slow path.

### Algorithm

**Backward scan** from the `Reset` instruction, looking for `Project`/`RcInc` patterns:

```
For each field i of the Reset'd variable y:
    Scan backward to find:  %fi = Project { value: y, field: i }
    Then scan for:          RcInc { var: %fi }
    If both found:
        1. Erase the RcInc (it is unnecessary on the fast path because we own y exclusively)
        2. Mark field i as "claimed" in a bitmask

Build claimed_fields bitmask from the above scan.

Fast path (unique):
    For each field i NOT in claimed_fields:
        RcDec the old field value (it is being replaced and no one claimed it)
    For claimed fields: nothing — the caller already has a reference via the projection

Slow path (shared):
    RcDec { var: y }                  // Dec the shared original
    For each field i IN claimed_fields:
        RcInc the projected value     // Restore the inc we erased above
    For unclaimed fields: nothing — they go down with the dec of y
```

The intuition: on the fast path, we exclusively own the object, so projected fields don't need to be incremented (they are part of our exclusive ownership). On the slow path, the `dec y` will recursively decrement all fields, so fields we want to keep must be incremented to prevent them from being freed.

### Example

```
// Before erasure:
%head = Project { value: %list, field: 0 }
%tail = Project { value: %list, field: 1 }
RcInc { var: %tail }           // tail is retained in the new Cons
%new_head = Apply { func: f, args: [%head] }
Reset { var: %list, token: %t }
Reuse { token: %t, dst: %r, ctor: Cons, args: [%new_head, %tail] }

// After erasure:
%head = Project { value: %list, field: 0 }
%tail = Project { value: %list, field: 1 }
// RcInc { var: %tail } -- ERASED
%new_head = Apply { func: f, args: [%head] }
Reset { var: %list, token: %t }
Reuse { token: %t, dst: %r, ctor: Cons, args: [%new_head, %tail] }

// claimed_fields = { 1 } (tail was claimed)
// Fast path: nothing to release for field 1; field 0 (%head) was consumed by f
// Slow path: dec %list, then inc %tail (restore the erased inc)
```

- [x] Implement backward scan for `Project`/`RcInc` patterns before `Reset`
- [x] Build claimed-fields bitmask per Reset
- [x] Fast path: emit `RcDec` for unclaimed replaced fields only
- [x] Slow path: emit `RcInc` for claimed fields (restoring erased increments)
- [x] Handle multi-field constructors with mixed claimed/unclaimed fields

## 09.5 Self-Set Elimination

Adapted from Lean 4's `removeSelfSet` optimization. After expansion, the fast path may contain `Set` instructions that write a field back to its original position -- a no-op.

### Rule

If the fast path would emit:

```
Set { base: y, field: i, value: wi }
```

and `wi` was obtained via:

```
%wi = Project { value: y, field: i }
```

(i.e., the value being written is the same field from the same object at the same index), then the `Set` is a no-op and can be eliminated.

### Detection

During fast-path emission, for each `Set` instruction, check whether the source value `wi` was the result of `Project { value: y, field: i }` where `y` is the base object and `i` is the same field index. If so, skip emitting the `Set`.

This is common in recursive data structure transformations where one field changes (e.g., `head`) and others are preserved (e.g., `tail`):

```ori
Cons(head, tail) -> Cons(f(head), tail)
//                                ^^^^ tail is field 1, projected from Cons at field 1
//                                     Set { base: list, field: 1, value: tail } is a no-op
```

- [x] Implement self-set detection during fast-path emission
- [x] Track `Project` origins to identify self-referential writes
- [x] Skip `Set` instructions that are no-ops

---

**Exit Criteria:** Functional patterns like map/filter on lists reuse memory when uniquely owned. The expansion produces correct two-path code: fast path (in-place mutation) for unique values, slow path (fresh allocation) for shared values. Projection-increment erasure and self-set elimination reduce unnecessary RC operations and writes. Allocation counts measurably decrease for common recursive data structure transformations.
