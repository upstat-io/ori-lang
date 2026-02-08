---
section: "07"
title: RC Insertion via Liveness Analysis
status: complete
goal: Precisely insert inc/dec operations into ARC IR based on variable liveness so every heap value is freed exactly when its last use ends
sections:
  - id: "07.1"
    title: Liveness Analysis on ARC IR
    status: complete
    note: liveness.rs — backward dataflow, gen/kill, postorder traversal
  - id: "07.2"
    title: RC Operation Insertion
    status: complete
    note: rc_insert.rs — Perceus algorithm, backward pass, borrowed-derived tracking
  - id: "07.3"
    title: Runtime Integration
    status: complete
    note: ori_rt redesigned — 8-byte header, ori_rc_alloc/inc/dec/free implemented
  - id: "07.4"
    title: Specialized Drop Functions
    status: complete
    note: drop.rs — DropKind/DropInfo descriptors, per-type drop analysis, 38 tests
  - id: "07.5"
    title: Early Exit & Panic Cleanup
    status: complete
    note: rc_insert.rs — edge cleanup pass, predecessor computation, trampoline blocks for edge splitting, 6 tests
  - id: "07.6"
    title: Reset/Reuse Detection
    status: complete
    note: reset_reuse.rs — intra-block RcDec+Construct→Reset/Reuse replacement
---

# Section 07: RC Insertion via Liveness Analysis

**Status:** Complete — 07.1 liveness analysis complete. 07.2 RC insertion complete. 07.3 runtime integration complete (ori_rt redesigned with 8-byte header, V2 API). 07.4 specialized drop descriptors complete (DropKind/DropInfo, 38 tests). 07.5 early exit/edge cleanup complete (edge gap Decs, predecessor computation, trampoline blocks for edge splitting). 07.6 Reset/Reuse detection complete.
**Goal:** After borrow inference (Section 06), insert precise reference counting operations into the **ARC IR** (Section 06.0). Every non-scalar, non-borrowed value gets `inc` at duplication and `dec` at last use. This is the "Perceus" approach from Koka.

**Crate:** `ori_arc` (no LLVM dependency). Operates on ARC IR basic blocks.

**Reference compilers:**
- **Koka** `src/Backend/C/Parc.hs` -- Backward traversal inserting dup/drop based on live variables
- **Lean 4** `src/Lean/Compiler/IR/RC.lean` -- `addInc`/`addDec` based on VarInfo liveness
- **Roc** `crates/compiler/mono/src/code_gen_help/refcount.rs` -- `ModifyRc::Inc`/`Dec`/`Free` IR statements

---

## 07.1 Liveness Analysis on ARC IR

**Backward dataflow analysis** over ARC IR basic blocks to compute live variable sets. Because the ARC IR has explicit control flow (basic blocks, terminators, block parameters), liveness analysis is a standard compiler algorithm — no need to reinvent control flow traversal on the expression tree.

**Block ordering:** Process blocks in **postorder** (not reverse postorder) for backward liveness analysis. Postorder ensures that when processing a block, its successors have already been visited (except for back-edges in loops, which are handled by fixed-point iteration). Block ordering must be computed from CFG edges, not assumed from `Vec` storage order — blocks may not be stored in any particular traversal order.

```rust
/// Live variable set at a program point.
///
/// A variable is "live" if it will be used again in the future.
/// A variable is "dead" if its last use has already occurred.
pub type LiveSet = FxHashSet<ArcVarId>;

/// Compute liveness for an ARC IR function.
///
/// Returns live-in and live-out sets for each block.
///
/// Algorithm: Standard backward dataflow on basic blocks.
///   gen(B)  = upward-exposed uses (used before defined in block)
///   kill(B) = variables defined in block
///   live_in(B)  = gen(B) ∪ (live_out(B) - kill(B))
///   live_out(B) = ∪ adjusted_live_in(S) for each successor S of B
/// Iterate until fixed point (needed for loops/back-edges).
pub fn compute_liveness(
    func: &ArcFunction,
    pool: &Pool,
) -> BlockLiveness {
    let num_blocks = func.blocks.len();
    let mut live_in: Vec<LiveSet> = vec![LiveSet::default(); num_blocks];
    let mut live_out: Vec<LiveSet> = vec![LiveSet::default(); num_blocks];

    // Precompute gen and kill sets for each block.
    // Process instructions in FORWARD order to correctly identify
    // upward-exposed uses (variables used before being defined in the block).
    let mut gen: Vec<LiveSet> = vec![LiveSet::default(); num_blocks];
    let mut kill: Vec<LiveSet> = vec![LiveSet::default(); num_blocks];
    for block in &func.blocks {
        let bid = block.id.0 as usize;
        let mut block_kill = LiveSet::default();
        let mut block_gen = LiveSet::default();

        // Block parameters are definitions at block entry
        for &(param_var, _) in &block.params {
            block_kill.insert(param_var);
        }

        // Forward scan: a use is "gen" only if not already killed in this block
        for instr in &block.body {
            for &used in instr.used_vars() {
                if pool.needs_rc(func.var_type(used)) && !block_kill.contains(&used) {
                    block_gen.insert(used);
                }
            }
            if let Some(dst) = instr.defined_var() {
                block_kill.insert(dst);
            }
        }
        // Terminator uses
        for &used in block.terminator.used_vars() {
            if pool.needs_rc(func.var_type(used)) && !block_kill.contains(&used) {
                block_gen.insert(used);
            }
        }

        gen[bid] = block_gen;
        kill[bid] = block_kill;
    }

    // Compute postorder from CFG edges (not from Vec storage order)
    let postorder = compute_postorder(func);

    let mut changed = true;
    while changed {
        changed = false;

        // Process blocks in postorder for backward analysis
        for &bid in &postorder {
            let block = &func.blocks[bid];

            // live_out = union of adjusted live_in of all successors.
            // When a successor has block parameters, replace them with
            // the corresponding jump arguments from this block's terminator.
            let mut new_out = LiveSet::default();
            for (succ_id, args) in successor_args(&block.terminator) {
                let succ_bid = succ_id.0 as usize;
                let succ_block = &func.blocks[succ_bid];

                // Start with successor's live_in
                let succ_live_in = &live_in[succ_bid];
                for &var in succ_live_in {
                    // If var is a block parameter of the successor,
                    // substitute with the corresponding jump argument
                    if let Some(arg_idx) = succ_block.params.iter()
                        .position(|&(p, _)| p == var)
                    {
                        if let Some(&arg) = args.get(arg_idx) {
                            if pool.needs_rc(func.var_type(arg)) {
                                new_out.insert(arg);
                            }
                        }
                    } else {
                        new_out.insert(var);
                    }
                }
            }

            // live_in = gen ∪ (live_out - kill)
            let mut new_in = gen[bid].clone();
            for &var in &new_out {
                if !kill[bid].contains(&var) {
                    new_in.insert(var);
                }
            }

            if new_in != live_in[bid] || new_out != live_out[bid] {
                live_in[bid] = new_in;
                live_out[bid] = new_out;
                changed = true;
            }
        }
    }

    BlockLiveness { live_in, live_out }
}

/// Liveness results for all blocks in a function.
pub struct BlockLiveness {
    pub live_in: Vec<LiveSet>,
    pub live_out: Vec<LiveSet>,
}
```

The key advantage of operating on ARC IR: branching, loops, and early exits are just edges between blocks. The standard dataflow algorithm handles all of these uniformly. No special cases for `if`/`match`/`loop`/`break`.

> **Import note:** The `pool.needs_rc()` calls above require `use ori_arc::ArcClassification;` to be in scope. The `needs_rc()` method is defined on the `ArcClassification` trait (Section 05.3), which is implemented for `Pool` in the `ori_arc` crate. Without this import, the method is not available on `Pool`.

**Key details:**
- **Gen/kill computation** uses forward instruction order to correctly identify upward-exposed uses (variables used before being defined within the block). A naive approach that computes `used` and `defined` independently gives wrong results when a variable is both defined and used in the same block — the order matters.
- **Block parameters are definitions** at block entry. They must be included in the `kill` set and removed from `live_in`. When computing `live_out` from a successor's `live_in`, block parameters in the successor are replaced with the corresponding jump arguments from the predecessor's terminator.
- **Postorder traversal** ensures efficient convergence: in the absence of loops, one pass suffices. With loops, the back-edges require additional iterations, but postorder minimizes the number of iterations needed.

- [ ] Implement backward dataflow liveness on ARC IR blocks
- [ ] Compute gen/kill sets with forward instruction scan (upward-exposed uses)
- [ ] Compute postorder traversal from CFG edges
- [ ] Implement `successor_args()` to extract jump arguments per successor
- [ ] Implement `used_vars()` and `defined_var()` on `ArcInstr` and `ArcTerminator`
- [ ] Handle block parameters as definitions (include in kill set)
- [ ] Handle block parameter substitution in live_out computation
- [ ] Handle loops: fixed-point iteration converges naturally via back-edges
- [ ] Handle closures: captured variables live until closure value is dead
- [ ] Test liveness on simple ARC IR functions (verify last-use detection)

## 07.2 RC Operation Insertion

Using liveness results, insert `RcInc`/`RcDec` instructions directly into the ARC IR blocks. This mutates the ARC IR in place — the RC operations are first-class instructions in the ARC IR (see `ArcInstr::RcInc` and `ArcInstr::RcDec` in Section 06.0).

**Backward pass ordering:** The insertion pass iterates backward through each block's instructions, building up the new instruction list. The key correctness requirement is that after reversal, the final order must be: `[Inc ops] instruction [Dec ops]`. That is, increments for consumed arguments appear *before* the instruction, and decrements for dead results appear *after* the instruction.

When iterating backward and pushing to a Vec that will be reversed:
1. **Dec first** (for dead definitions) — these end up *after* the instruction post-reverse
2. **Instruction** — the original instruction
3. **Inc last** (for duplicated arguments) — these end up *before* the instruction post-reverse

This follows Lean 4's continuation-passing approach in `addInc`/`addDec`: conceptually, for each instruction we wrap it as `[Inc for consumed args] instruction [Dec for dead result]`.

```rust
/// Insert RC operations into an ARC IR function.
///
/// Rules (from Koka Perceus):
/// 1. Variable defined and used once -> no inc, no dec (ownership transfers)
/// 2. Variable used multiple times -> inc before each use except the last
/// 3. Variable defined but never used -> dec immediately after definition
/// 4. Borrowed parameter -> no inc/dec at all (caller manages)
/// 5. Owned parameter, last use -> dec after last use
/// 6. Return value -> no dec (ownership transfers to caller)
pub fn insert_rc_ops(
    func: &mut ArcFunction,
    pool: &Pool,
    borrow_info: &AnnotatedSig,
    liveness: &BlockLiveness,
) {
    for block in &mut func.blocks {
        let bid = block.id.0 as usize;
        let mut live = liveness.live_out[bid].clone();
        let mut new_body = Vec::with_capacity(block.body.len() * 2);

        // Backward pass through instructions.
        // Push order: Dec, instruction, Inc — so after reverse: Inc, instruction, Dec.
        for instr in block.body.iter().rev() {
            // 1. At each definition: remove from live set.
            //    If variable was NOT live (never used) -> emit Dec after definition.
            //    Push Dec FIRST (it will appear AFTER instruction post-reverse).
            if let Some(dst) = instr.defined_var() {
                if pool.needs_rc(func.var_type(dst)) && !live.remove(&dst) {
                    // Defined but never used -> drop immediately after definition
                    new_body.push(ArcInstr::RcDec { var: dst });
                }
            }

            // 2. The instruction itself.
            new_body.push(instr.clone());

            // 3. At each variable use: check if variable is still live after this use.
            //    If live after -> emit Inc before this use (dup).
            //    Push Inc LAST (it will appear BEFORE instruction post-reverse).
            //
            //    Correctness for duplicate uses: if a variable appears N times in
            //    used_vars() (e.g., `Apply { args: [x, x] }`), two sub-cases
            //    both produce the correct N-1 Inc's:
            //
            //    (a) x is in live_out (used later beyond this instruction):
            //        Every iteration finds x in `live`, emits Inc. After N
            //        iterations, N Inc's are emitted. But x was already in live
            //        from live_out, so one of those N uses is "free" (ownership
            //        carried forward). Net: N-1 extra Inc's. Correct.
            //
            //    (b) x is NOT in live_out (this is the last instruction using x,
            //        but x appears multiple times within it):
            //        First iteration: x not in `live`, no Inc. `live.insert(x)`.
            //        Second through Nth iterations: x is in `live`, emit Inc.
            //        Net: N-1 Inc's. The first use consumes the existing
            //        ownership, each additional use needs its own Inc. Correct.
            for &used in instr.used_vars() {
                if pool.needs_rc(func.var_type(used)) {
                    if live.contains(&used) {
                        // Variable used again later -> dup before this use
                        new_body.push(ArcInstr::RcInc { var: used, count: 1 });
                    }
                    // Mark as live (backward pass — this use makes it live above)
                    live.insert(used);
                }
            }
        }

        // Dec owned parameters that are live-in but not borrowed
        // (they enter the block with ownership, and if still live at entry,
        // the caller has already transferred ownership)

        new_body.reverse();
        block.body = new_body;
    }
}
```

**Derived value optimization** (from Lean 4's `LiveVars.borrows`):

When projecting a field from a Borrowed parameter, the projected value is implicitly kept alive by the parent — the parent's lifetime encompasses the field's lifetime. No Inc/Dec is needed for the projected value as long as the parent remains live. Track a `borrows` set alongside the `live` set:

```rust
/// Extended live variable tracking with borrow relationships.
pub struct LiveVars {
    pub live: LiveSet,
    /// Variables that are derived from (projected out of) a borrowed value.
    /// These don't need their own RC ops — the parent keeps them alive.
    pub borrows: FxHashSet<ArcVarId>,
}
```

When a `Project` instruction extracts a field from a variable that is a Borrowed parameter (or itself derived from one), the result is added to `borrows` instead of the normal `live` set. This avoids unnecessary RC operations for common struct field access patterns like `point.x + point.y`.

**Ownership transition for borrowed values:** When a value in the `borrows` set is used in an Owned position (e.g., passed to a function parameter that expects Owned, stored into a Construct, or returned), it must be:
1. Removed from the `borrows` set
2. Added to the `live` set (it now has its own ownership)
3. Inc'd (the parent still needs its copy; the owned position needs an independent reference)

This ensures that values derived from borrowed parameters correctly transition to independent ownership when needed.

- [ ] Implement RC insertion on ARC IR blocks using liveness results
- [ ] Implement correct backward push order: Dec, instruction, Inc (reversed to Inc, instruction, Dec)
- [ ] Handle Borrowed parameters (skip entirely per borrow_info)
- [ ] Handle multiple uses (inc before each non-last use)
- [ ] Handle dead variables (dec immediately after definition)
- [ ] Handle block boundaries (variables live-out but not used in successors)
- [ ] Implement derived value (borrows) tracking for projection optimization
- [ ] Test: verify RC instructions on simple ARC IR functions

## 07.3 Runtime Integration

> **Prerequisite: ori_rt Redesign Required**
>
> The current `ori_rt` uses a **16-byte `RcHeader(refcount, size)`** layout where the pointer points to the header. V2 requires a fundamentally different layout:
>
> | | Current `ori_rt` | V2 Required |
> |---|---|---|
> | Header | 16 bytes: `{ refcount: i64, size: usize }` | 8 bytes: `{ strong_count: i64 }` (0.1-alpha) |
> | Pointer | Points to `RcHeader` | Points to **data** (`ptr - 8` = strong_count) |
> | Size tracking | In header at runtime | Compile-time via `TypeInfo` (no runtime size field) |
> | Alloc API | `ori_rc_alloc(size) -> *header` | `ori_rc_alloc(size, align) -> *data` |
> | Inc API | `ori_rc_inc(*header)` | `ori_rc_inc(*data)` |
> | Dec API | `ori_rc_dec(*header)` | `ori_rc_dec(*data, drop_fn)` |
> | Free API | (implicit in dec) | `ori_rc_free(*data, size, align)` |
>
> **BLOCKING: This redesign must complete before any V2 RC codegen is possible.** All existing `ori_rc_*` call sites in `ori_llvm` must be updated to the new API. There is no incremental migration path — the old and new layouts are incompatible.
>
> **OriStr note:** The current `OriStr` has no RC header. All existing string creation paths (`ori_str_concat`, `ori_str_from_int`, etc.) must be rewritten to use `ori_rc_alloc` and the 8-byte header layout (strong_count at `ptr - 8`, data pointer points to string bytes).
>
> The new API surface (0.1-alpha, 8-byte header):
> - `ori_rc_alloc(size: usize, align: usize) -> *mut u8` — allocates `size + 8` bytes with `align` alignment, sets strong_count to 1, returns pointer to data (at `base + 8`)
> - `ori_rc_inc(data_ptr: *mut u8)` — increments strong_count at `data_ptr - 8`
> - `ori_rc_dec(data_ptr: *mut u8, drop_fn: fn(*mut u8))` — decrements strong_count at `data_ptr - 8`; if zero, calls `drop_fn(data_ptr)`, then frees
> - `ori_rc_free(data_ptr: *mut u8, size: usize, align: usize)` — unconditional deallocation from `data_ptr - 8` with total size `size + 8`
>
> Size is tracked at compile-time via `TypeInfo` (Section 01). Each type knows its own size and alignment, so the runtime never needs to store or retrieve size from the heap. The 8-byte header stores only strong_count — no size field, no weak_count (deferred to post-0.1-alpha). The `drop_fn` parameter to `ori_rc_dec` is a pointer to the type's specialized drop function (Section 07.4).

RC operations map to `ori_rt` functions:

```
Inc(var)  -> call ori_rc_inc(data_ptr)
Dec(var)  -> call ori_rc_dec(data_ptr, drop_fn_for_type)
Free(var) -> call ori_rc_free(data_ptr, size, align)   // unconditional deallocation
```

**RC heap layout with 8-byte header (0.1-alpha)** (see Section 01.6 for full documentation):

```
Heap allocation:
  +──────────────────+───────────────────────+
  | strong_count: i64| data bytes ...        |
  +──────────────────+───────────────────────+
  ^                  ^
  ptr - 8            ptr (data pointer, stored on stack)
```

- Data pointer (`ptr`) points directly to user data, NOT to the header
- Strong refcount is at `ptr - 8`
- Advantage: data pointer can be passed directly to C FFI without adjustment
- `emit_retain`: loads strong_count from `ptr - 8`, increments, stores back
- `emit_release`: loads strong_count from `ptr - 8`, decrements; if zero, call specialized drop function, then free from `ptr - 8`
- Allocation: `ori_rc_alloc(size, align)` allocates `size + 8` bytes, returns `base + 8`

**Stack representations for reference-counted types:**

| Type | Stack Layout | Heap Data |
|------|-------------|-----------|
| `str` | `{i64, ptr}` | `[rc \| utf8_bytes...]` |
| `[T]` | `{i64, i64, ptr}` | `[rc \| elements...]` |
| `{K: V}` | `{i64, i64, ptr, ptr}` | `[rc \| keys...]`, `[rc \| vals...]` |
| `set[T]` | `{i64, i64, ptr}` | `[rc \| elements...]` |

- [ ] **Prerequisite:** Redesign `ori_rt` to Roc-style layout before V2 codegen
- [ ] Implement new `ori_rt` API: `ori_rc_alloc`, `ori_rc_inc`, `ori_rc_dec`, `ori_rc_free`
- [ ] Implement RC op emission in codegen (IrBuilder emits calls to ori_rt)
- [ ] Emit correct pointer arithmetic: strong_count at `ptr - 8`, data at `ptr`
- [ ] Pass specialized drop function pointer to `ori_rc_dec`

### Weak References for Cycle Safety

> **Deferred to post-0.1-alpha.** 0.1-alpha uses an **8-byte header** with `strong_count` only. Weak references, `weak_count`, and deferred deallocation are future work. Ori's "no shared mutable refs" design pillar makes reference cycles unlikely, but if cycles become possible in future versions, a cycle detector or weak reference system will be added.

**0.1-alpha heap layout (8-byte header):**

```
Heap allocation:
  +──────────────────+───────────────────────+
  | strong_count: i64| data bytes ...        |
  +──────────────────+───────────────────────+
  ^                  ^
  ptr - 8            ptr (data pointer, stored on stack)
```

- `strong_count` at `ptr - 8`: tracks strong references (normal ownership)
- Data pointer (`ptr`) points directly to user data
- Total header size: 8 bytes (1 x i64)
- Allocation: `ori_rc_alloc(size, align)` allocates `size + 8` bytes, returns `base + 8`
- `emit_retain`: loads strong_count from `ptr - 8`, increments, stores back
- `emit_release`: loads strong_count from `ptr - 8`, decrements; if zero, calls drop function, then frees from `ptr - 8`
- Free: deallocate from `ptr - 8` with total size `size + 8`

**0.1-alpha Runtime API:**
- `ori_rc_alloc(size: usize, align: usize) -> *mut u8` — allocates `size + 8` bytes, sets strong_count to 1, returns `base + 8`
- `ori_rc_inc(data_ptr: *mut u8)` — increments strong_count at `data_ptr - 8`
- `ori_rc_dec(data_ptr: *mut u8, drop_fn: fn(*mut u8))` — decrements strong_count at `data_ptr - 8`; if zero, calls `drop_fn(data_ptr)`, then frees
- `ori_rc_free(data_ptr: *mut u8, size: usize, align: usize)` — unconditional deallocation from `data_ptr - 8` with total size `size + 8`

**Future work (post-0.1-alpha):** The full weak reference system would extend the header to 16 bytes (`strong_count` + `weak_count`), add `ori_rc_weak_inc`, `ori_rc_weak_dec`, `ori_rc_weak_lock` APIs, and implement deferred deallocation (tombstoning when strong_count reaches zero but weak_count > 0).

## 07.4 Specialized Drop Functions

**Problem:** When a refcount reaches zero, all RC'd children must be decremented before freeing the memory. A generic "recursive dec" approach requires runtime type information to know which fields are RC'd. Instead, we generate **compile-time specialized drop functions** per type — each knows exactly which fields to Dec.

**Reference compilers:**
- **Lean 4** `src/Lean/Compiler/IR/RC.lean` — `addDec` generates type-specific cleanup
- **Roc** `crates/compiler/mono/src/code_gen_help/refcount.rs` — per-layout refcount helpers

For each type with `ArcClass::DefiniteRef` or `ArcClass::PossibleRef`, generate a drop function:

```rust
/// Generate a specialized drop function for a type.
///
/// The drop function is called when an RC'd value's refcount reaches zero.
/// It decrements each RC'd child field, then frees the memory.
///
/// Generated function signature: fn drop_<Type>(data_ptr: *mut u8)
fn generate_drop_function(ty: Idx, pool: &Pool) -> ArcFunction {
    // The function receives a data pointer to the object being freed.
    // It must:
    // 1. Dec each RC'd child field
    // 2. Free the memory (ori_rc_free with compile-time known size and align)
    todo!()
}
```

**Drop function patterns by type kind:**

| Type Kind | Drop Function Body |
|-----------|-------------------|
| **Struct** | Dec each field with `ArcClass != Scalar`, then `ori_rc_free(ptr, size, align)` |
| **Enum** | Switch on tag; for each variant, Dec that variant's RC'd fields, then free |
| **`[T]`** (list) | Load length; loop over elements, Dec each if `T` is RC'd; free buffer |
| **`{K: V}`** (map) | Loop over keys (Dec if RC'd), loop over values (Dec if RC'd); free both buffers |
| **`set[T]`** | Loop over elements, Dec each if `T` is RC'd; free buffer |
| **Closure env** | Dec each captured variable in the env struct; free env struct |
| **`str`** | No children to Dec (bytes are not RC'd); just `ori_rc_free(ptr, size, align)` |

**Naming convention:** `_ori_drop$<mangled_type>` — e.g., `_ori_drop$MyStruct`, `_ori_drop$List$Str`. These are internal symbols, not user-visible.

**Closure environments** (per Q3 answer): Each closure type gets its own drop function for its env struct. The env struct captures are known at compile time, so the drop function Decs each captured variable. Example:

```
// Closure capturing (name: str, count: int)
fn _ori_drop$__lambda_3_env(data_ptr: *mut u8) {
    let env = data_ptr as *mut Lambda3Env;
    ori_rc_dec((*env).name, _ori_drop$Str);   // Dec the captured str
    // (*env).count is Scalar (int) — no Dec needed
    ori_rc_free(data_ptr, size_of::<Lambda3Env>(), align_of::<Lambda3Env>());
}
```

- [x] Implement drop descriptor generation per type kind (struct, enum, list, map, set, closure, str) — `drop.rs`
- [x] Compute `DropKind::Collection` for variable-length types (list, set) with RC elements
- [x] Compute `DropKind::Map` for map types (dec_keys/dec_values flags)
- [x] Compute `DropKind::Enum` for enum types (per-variant field cleanup, including option/result)
- [x] Compute `DropKind::ClosureEnv` for closure environments (Dec each captured variable)
- [x] Handle nested RC types (recursive drop via field_type references in descriptors)
- [x] Handle named type resolution (Named/Alias indirection)
- [x] Batch collection via `collect_drop_infos()` (scans RcDec instructions, deduplicates)
- [x] Test: 38 tests covering all type kinds, edge cases, collection, and batch API
- [ ] LLVM codegen: generate actual drop functions from DropInfo descriptors (in ori_llvm)
- [ ] LLVM codegen: register drop function pointers with `ori_rc_dec` calls (in ori_llvm)

## 07.5 Early Exit & Panic Cleanup

**Problem:** Early exits (`break`, `continue`, `return` in non-tail position) and panics can leave RC'd variables live without a Dec. Without cleanup, these paths leak memory.

### Early Exit (break/continue/return)

The per-block backward RC insertion pass (07.2) handles **intra-block** cleanup (dead definitions, unused params). But **cross-block edge gaps** require a separate pass:

When a predecessor P branches to successors B1 and B2, `live_out[P]` is the **union** of what all successors need. Variables in `live_out[P]` that are NOT in `live_in[B]` for a specific successor B are "stranded" on that edge — they're alive when entering B but B doesn't use them. These need `RcDec` at B's entry.

**Algorithm (`insert_edge_cleanup`):**
1. Compute predecessors for each block (deduplicated, from successor edges)
2. For each block B, compute per-predecessor gap: `gap(P→B) = live_out[P] - live_in[B]` (filtered for RC-trackable, non-borrowed vars)
3. **Single-predecessor B**: insert Dec at B's start
4. **Multi-predecessor B, all gaps identical**: insert Dec at B's start
5. **Multi-predecessor B, gaps differ**: edge split — create trampoline blocks with Dec + Jump

Trampoline blocks contain only `RcDec` instructions and a `Jump` to the original target. The predecessor's terminator is redirected to the trampoline.

**Borrowed var filtering:** Gap computation excludes borrowed params and globally-tracked borrow-derived vars (computed across all blocks) to prevent double-free on never-Inc'd values.

References:
- Lean 4: `addDecForDeadParams` in `src/Lean/Compiler/IR/RC.lean`
- Appel: "Modern Compiler Implementation" §10.2 (live range splitting)

### Panic Cleanup (Full Cleanup Blocks)

> **0.1-alpha scope**: Use `Call` everywhere (no `Invoke`/landingpad). Memory leaks on panic are accepted as a known limitation. Full panic cleanup via invoke + cleanup blocks is future work. The design below describes the target architecture for post-0.1-alpha.

Every potentially-panicking call must ensure all live RC'd variables are decremented if a panic occurs. This uses the `Invoke` terminator (Section 06.0) with cleanup blocks.

**Mechanism:**
1. Potentially-panicking calls (array bounds checks, arithmetic overflow, explicit `panic`, user function calls that might panic) use `ArcTerminator::Invoke` instead of `ArcInstr::Apply`
2. The `Invoke` terminator has two successors: `normal` (continue on success) and `unwind` (jump here on panic)
3. The `unwind` block is a **cleanup block**: it Decs all RC'd variables that were live at the point of the invoke, then resumes unwinding

```
// Before: simple apply
block_3:
    %result = Apply { func: might_panic, args: [x, y] }
    Jump { target: block_4, args: [%result] }

// After: invoke with cleanup
block_3:
    Invoke {
        dst: %result,
        func: might_panic,
        args: [x, y],
        normal: block_4,      // success path
        unwind: cleanup_3,    // panic path
    }

cleanup_3:  // cleanup block — Dec all live RC vars
    RcDec { var: a }     // 'a' was live at invoke point
    RcDec { var: b }     // 'b' was live at invoke point
    Resume               // continue unwinding
```

**LLVM emission:**
- `Invoke` terminator -> LLVM `invoke` instruction
- Cleanup block -> LLVM basic block starting with `landingpad` instruction (personality function: `__ori_personality`)
- After cleanup Decs -> `resume` instruction to continue unwinding
- Personality function determines which landing pads to run during stack unwinding

**When to use Invoke vs Apply:**
- `Apply` (no unwind): calls to functions proven to never panic (pure arithmetic on scalars, field access on owned values, known-safe builtins)
- `Invoke` (with unwind): all other calls, including user-defined functions, collection access, string operations, any function that might transitively panic

> **Implementation note:** Panic cleanup is the most complex part of the RC system. Consider implementing it as a later phase — start with Apply everywhere (leak on panic), then add Invoke + cleanup blocks as a refinement pass. The correctness of non-panicking paths does not depend on cleanup blocks.

**0.1-alpha:**
- [x] Implement edge cleanup pass (`insert_edge_cleanup` in `rc_insert.rs`)
- [x] Compute predecessors, per-edge gaps, trampoline block creation
- [x] Filter borrowed params and globally-derived borrow vars from gaps
- [x] Test: early exit (branch with unused var), break from loop, switch asymmetric, multiple stranded vars, multi-pred same gap, symmetric no-cleanup

**Post-0.1-alpha:**
- [ ] Identify potentially-panicking calls (conservative: all user function calls)
- [ ] Convert panicking `Apply` instructions to `Invoke` terminators with cleanup blocks
- [ ] Generate cleanup blocks with Dec for all live RC'd variables at invoke point
- [ ] Add `Resume` variant to `ArcTerminator` for cleanup block termination
- [ ] LLVM emission: `invoke` instruction, `landingpad`, `resume`
- [ ] Implement `__ori_personality` function (or reuse existing unwinding ABI)
- [ ] Test: verify no leaks on panic paths (leak-free panic)

## 07.6 Reset/Reuse Detection

> **0.1-alpha scope**: Only intra-block Reset/Reuse detection is implemented. Cross-block detection (requiring dominator trees and all-paths verification) is future work.

After RC operations are inserted, scan for patterns where a `RcDec` on a variable is followed by a `Construct` of the **same type**. This pattern indicates an opportunity for constructor reuse (FBIP). Replace the `RcDec`/`Construct` pair with `Reset`/`Reuse` intermediate instructions (defined in Section 06.0 as `ArcInstr::Reset` and `ArcInstr::Reuse`).

**Detection rule:**

```
Pattern:
    RcDec { var: x }
    ... (intervening instructions that do not alias x) ...
    Construct { dst: y, ty: T, ctor: C, args: [w0, w1, ...] }

Where: typeof(x) == T  (same concrete type — constructor-identity rule)

Replace with:
    Reset { var: x, token: t }    // Replaces the RcDec
    ... (intervening instructions unchanged) ...
    Reuse { token: t, dst: y, ty: T, ctor: C, args: [w0, w1, ...] }  // Replaces the Construct
```

The `RcDec` is removed because the `Reset` subsumes it: on the slow path (expanded by Section 09), the Reset will emit a `dec`; on the fast path, the memory is reused instead of freed.

**Constraints:**
- Same-type only (Lean 4 constructor-identity rule, not Koka's size-based matching)
- The variable `x` must not be used between the `RcDec` and the `Construct` (no aliasing)
- The `Construct` must be the next allocation of the same type (no intervening allocations of that type)

**Detection scope -- intra-block and cross-block:**

The initial implementation uses **intra-block** detection: the `RcDec` and `Construct` must be in the same basic block. This is simpler and covers the most common cases (match arms that destructure and reconstruct within a single block).

The design supports **cross-block** detection via dominance analysis for future optimization:
- If a `RcDec { var: x }` in block A **dominates** a `Construct { ty: T }` in block B (where `typeof(x) == T`), and the variable `x` is not used on any path from A to B, the pair is a reuse candidate.
- Cross-block detection requires: (1) computing a dominator tree over the ARC IR CFG, (2) verifying the no-alias constraint holds on ALL paths from the Dec to the Construct (not just one path), and (3) ensuring no intervening same-type allocations on any path.
- The all-paths verification is critical: unlike intra-block where there is exactly one execution path between the two instructions, cross-block reuse must ensure that every control flow path from the Dec to the Construct satisfies the constraints. This can be checked by verifying that the Dec dominates the Construct and no path from Dec to Construct contains a use of `x` or another same-type allocation.

Cross-block detection is not required for the initial implementation but the ARC IR structure (basic blocks with explicit CFG) makes it straightforward to add.

**Section 09** expands `Reset`/`Reuse` pairs into `isShared` + fast/slow conditional paths. After Section 09, no `Reset` or `Reuse` instructions remain in the ARC IR.

- [ ] Scan for `RcDec` + `Construct same_type` patterns after RC insertion
- [ ] Verify same-type constraint (constructor-identity, not size-based)
- [ ] Verify no aliasing of the dec'd variable between dec and construct
- [ ] Replace `RcDec`/`Construct` with `Reset`/`Reuse` pairs
- [ ] Test: `map` over a list produces `Reset`/`Reuse` pairs

---

**Exit Criteria:** Every non-scalar value in the ARC IR has correct `RcInc`/`RcDec` instructions. No leaks (every inc has a matching dec). No use-after-free (dec only at last use). Borrowed parameters have zero RC overhead. Specialized drop functions exist for every RC'd type. Panic paths clean up all live RC'd variables. Constructor reuse opportunities are detected and marked with `Reset`/`Reuse` pairs (expanded by Section 09). The codegen layer (`ori_llvm`) reads the ARC IR's RC instructions and emits corresponding `ori_rt` calls.
