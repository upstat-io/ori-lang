# Journey 16: COW Sharing Semantics — Results

**Date**: 2026-03-02
**Status**: PASS (both paths)
**Eval exit code**: 23 (expected 23)
**AOT exit code**: 23 (expected 23)

## Journey Code

```ori
@main () -> int = {
    let $original = [1, 2, 3];
    let modified = original;
    let modified = modified.push(4);

    let $orig_len = original.length();
    let $mod_len = modified.length();

    let orig_sum = 0;
    for x in original do {
        orig_sum += x;
    };

    let mod_sum = 0;
    for x in modified do {
        mod_sum += x;
    };

    orig_len + mod_len + orig_sum + mod_sum
}
```

**Expected**: 3 + 4 + 6 + 10 = 23

## Phase Analysis

### 1. Lexer

- Source: 731 bytes, 124 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors
- Clean. No issues.

### 2. Parser

- User module: 1 function, 38 expressions, 0 errors
- Prelude: 9 functions, 39 traits, 46 expressions, 4 decision trees, 0 errors
- Two `for` loops parsed correctly as `for...do` blocks.
- Two `.push()` and two `.length()` method calls parsed correctly.
- Clean. No issues.

### 3. Type Checker

- Registration: 1 function, 0 tests, 0 impls
- Signature collection + body checking: complete
- Prelude registration: 9 functions (hash-first hits for `compare`, `min`, `max`; AST fallback for generic builtins `len`, `is_empty`, etc.)
- No type errors. Mutable `let` (`$original`, `orig_sum`, `mod_sum`) and immutable `let` (`modified`, `$orig_len`, `$mod_len`) correctly distinguished.

### 4. Canonicalizer

- User module: 38 source expressions -> 45 canon nodes (18.4% expansion)
- 6 constants, 0 decision trees
- Prelude: 46 source expressions -> 46 canon nodes, 4 decision trees
- Canon expansion (18.4%) is slightly above average (typical: 10-25%). The additional nodes come from desugaring `for..do` loops and compound assignment `+=`.

### 5. Eval (Interpreter)

The eval trace shows correct execution:

1. **List creation** (CanId 3): `[1, 2, 3]` -> List literal with `Int(1)`, `Int(2)`, `Int(3)`
2. **Let binding** (CanId 4): `$original` bound to the list (Immutable)
3. **Shadowed let** (CanId 6): `modified` bound to `original` (Mutable) -- creates shared reference
4. **Method call** (CanId 9): `modified.push(4)` -- COW push in interpreter (clones because RC > 1)
5. **Re-let** (CanId 10): `modified` re-bound to push result (Mutable)
6. **Length calls** (CanId 12, 15): `.length()` on `original` (Name shard=1,local=3) and `modified` (Name shard=9,local=5)
7. **For loop 1** (CanId 26): iterates `original` (3 iterations), accumulating `orig_sum` via `Add` operations: 0+1=1, 1+2=3, 3+3=6
8. **For loop 2** (CanId 36): iterates `modified` (4 iterations), accumulating `mod_sum` via `Add` operations: 0+1=1, 1+2=3, 3+3=6, 6+4=10
9. **Final sum** (CanId 43): `3 + 4 + 6 + 10 = 23`

Value semantics verified in eval: `original` remained `[1,2,3]` (sum=6, length=3) after `modified.push(4)`.

### 6. LLVM IR Analysis

#### 6.1 List Representation

Lists use the standard fat-pointer representation `{ i64, i64, ptr }` = `{ len, cap, data }`.

#### 6.2 List Construction (bb0)

```llvm
%list.data = call ptr @ori_list_alloc_data(i64 3, i64 8)
; Store elements 1, 2, 3 via GEP
%list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2
```

List allocated with capacity 3, length 3. Elements stored via `getelementptr inbounds`. The triple `%list.2` = `{ len=3, cap=3, data=ptr }` represents `original`.

#### 6.3 COW Push -- THE CRITICAL OPERATION

```llvm
; RC increment for shared reference: original -> modified alias
%rc_inc.data = extractvalue { i64, i64, ptr } %list.2, 2
%rc_inc.cap = extractvalue { i64, i64, ptr } %list.2, 1
call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)

; Extract fields for push
%list.data3 = extractvalue { i64, i64, ptr } %list.2, 2
%list.len = extractvalue { i64, i64, ptr } %list.2, 0
%list.cap = extractvalue { i64, i64, ptr } %list.2, 1

; Push element 4
store i64 4, ptr %push.elem, align 4
call void @ori_list_push_cow(
    ptr %list.data3, i64 %list.len, i64 %list.cap,
    ptr %push.elem, i64 8, i64 8,
    ptr null, i32 0, ptr %push.out)
```

**COW mechanism analysis**:

1. **RC increment** (`ori_list_rc_inc`): Before push, the original list's RC is incremented from 1 to 2, because `modified = original` creates a shared reference. This is CORRECT.

2. **`ori_list_push_cow` call**: The COW push is called with `cow_mode=0` (dynamic RC check). Inside the runtime:
   - `ori_rc_is_unique(data)` will return `false` (RC=2)
   - The SLOW PATH executes: allocates new buffer, copies [1,2,3], appends 4
   - Old buffer RC is decremented (back to 1 for `original`)
   - Result written to `push.out`: new `{ len=4, cap=4, data=new_ptr }`

3. **The COW check is NOT in the LLVM IR itself** -- it is inside the `ori_list_push_cow` runtime function. The IR delegates the uniqueness check to the runtime. This is architecturally correct: `cow_mode=0` means "do the dynamic check at runtime." The `ori_rc_is_unique` function IS linked in the binary (symbol at `0x29f60`, 228 bytes) and IS called from within `ori_list_push_cow` (symbol at `0x1f740`, 2897 bytes).

4. **No inlined uniqueness check in LLVM IR**: The IR does NOT generate a branch with fast-path (unique) and slow-path (shared) inline. Instead, the entire COW decision is encapsulated in the runtime call. This is a design choice -- the runtime handles the branching internally.

#### 6.4 ARC Balance Analysis

Tracking all RC operations in the LLVM IR for `%list.2` (original):

| Location | Operation | Target | Purpose |
|----------|-----------|--------|---------|
| bb0 | `ori_list_rc_inc` | `%list.2` | Creating shared ref for `modified = original` (RC: 1->2) |
| bb0 | `ori_list_push_cow` | consumes `%list.2` data ref | COW push consumes one ref (RC: 2->1 for original's buffer) |
| bb1 | `ori_list_rc_inc` | `%list.2` | Borrowing for `.length()` on `original` |
| bb3 | `ori_buffer_rc_dec` | `%list.2` | Release after length extraction |
| bb5 | `ori_iter_from_list` | `%list.2` fields | Create iterator for first `for` loop (borrows the data) |
| bb9 | `ori_iter_drop` | first iterator | Drop iterator after first loop completes |

For `%push.val.s2` (modified):

| Location | Operation | Target | Purpose |
|----------|-----------|--------|---------|
| bb3 | `ori_list_rc_inc` | `%push.val.s2` | Borrowing for `.length()` on `modified` |
| bb5 | `ori_buffer_rc_dec` | `%push.val.s2` | Release after length extraction |
| bb9 | `ori_list_rc_inc` | `%v24` (= `%push.val.s2` via phi) | Create iterator for second `for` loop |
| bb9 | `ori_iter_from_list` | `%v24` fields | Create iterator |
| bb13 | `ori_buffer_rc_dec` | `%v48` (= `%v24` via phi) | Final cleanup of `modified` |
| bb13 | `ori_iter_drop` | second iterator | Drop iterator after second loop |

**ARC balance verdict**: The RC operations are balanced. Every `ori_list_rc_inc` has a corresponding `ori_buffer_rc_dec` or is consumed by a runtime function that handles decrement internally. The `ori_list_push_cow` function handles the consumption of one reference to the original buffer on the slow (shared) path.

#### 6.5 Landing Pads

Three pairs of landing pads (bb2/bb4/bb6) exist with cleanup code:

```llvm
bb2:     ; cleanup for original list only
bb4:     ; cleanup for both original and modified lists
bb6:     ; cleanup for both lists (after modified length extracted)
```

All landing pads have "No predecessors!" comments -- they are orphaned (no `invoke` instructions target them since all calls use `call` not `invoke`). This is CONFIRMED M11 from previous journeys.

#### 6.6 Iterator Loop Structure

Both `for` loops use the same pattern:
1. `ori_iter_from_list` creates an iterator handle
2. Loop header calls `ori_iter_next` returning `{ tag, value }` pair
3. `icmp ne tag, 0` branches: nonzero=more elements, zero=done
4. Loop body accumulates sum via `add i64`
5. Phi nodes carry the accumulator across iterations
6. After loop, `ori_iter_drop` releases the iterator

The phi-based SSA loop structure is correct and matches J7/J10 patterns.

## LLVM Deep Scrutiny (9 Categories)

### S1: Correctness

**PASS**. Both paths produce 23. The COW semantics are correct:
- `original` is never mutated (its buffer has RC=1 after push completes)
- `modified` gets a fresh buffer with [1,2,3,4]
- Both lists are independently iterable with correct sums

### S2: ARC/RC Safety

**PASS with observations**.
- ARC balance is correct -- every increment has a matching decrement
- The `ori_list_push_cow` runtime function handles COW correctly via `ori_rc_is_unique`
- `cow_mode=0` means dynamic check -- correct for this case where sharing is not statically determined
- Landing pads are orphaned (M11, CONFIRMED) but would be correct if invoked

**Observation**: The codegen does NOT attempt static uniqueness analysis here. Since `modified = original` is a trivial alias, a smarter compiler could emit `cow_mode=2` (force slow path) since sharing is statically known. Currently, the runtime pays for the dynamic `ori_rc_is_unique` check. This is a missed optimization, not a bug.

### S3: Memory Safety

**PASS**. No use-after-free, no double-free patterns visible. The consumption semantics of `ori_list_push_cow` (takes ownership of caller's reference) are correctly maintained by the RC inc before the call.

### S4: Type Representation

**PASS**. Lists correctly represented as `{ i64, i64, ptr }`. Elements are `i64` (for `int`). Push element stored via `store i64 4, ptr %push.elem, align 4`.

**Note**: `align 4` on `i64` store to `%push.elem` -- this is M5 (CONFIRMED). Should be `align 8` for i64.

### S5: Control Flow

**PASS**. Two for-loops compile to correct phi-based SSA loops. The `br label %bb1` after `ori_list_push_cow` in bb0 is M3 (CONFIRMED, unconditional branch to next sequential block).

### S6: Calling Convention

**PASS**. All calls use `call` (not `invoke`) since `_ori_main` is not marked `nounwind`. Entry point wrapper correctly truncates i64 to i32 for process exit code.

The `nounwind` analysis reports 0 nounwind functions -- correct, since the function calls runtime functions (`ori_list_push_cow`, `ori_iter_next`, etc.) that may throw.

### S7: Code Quality

**MEDIUM issues**:
- Orphaned landing pads (M11, CONFIRMED)
- Unconditional branches to next block (M3, CONFIRMED)
- `align 4` on i64 operations (M5, CONFIRMED)
- Single-predecessor phi nodes at loop exit (L4, CONFIRMED) -- bb9 phis from bb10, bb13 phis from bb14
- The push output is loaded field-by-field from `%push.out` alloca using GEP+load+insertvalue -- verbose but correct (avoids full struct load for JIT compatibility, per llvm-codegen.md rules)

### S8: Binary Analysis

- Binary size: 6,723,440 bytes (6.4 MB)
- .text: 949,034 bytes (929 KB)
- `_ori_main`: starts at `0x1eb00`, `main` wrapper at `0x1f050` -- function is ~1,360 bytes of native code
- Stack frame: `sub $0x1c8, %rsp` = 456 bytes (significant, but handles two list triples, push output, two iterator scratch spaces, and loop state)
- Runtime functions linked: `ori_list_alloc_data`, `ori_list_rc_inc`, `ori_list_push_cow`, `ori_buffer_rc_dec`, `ori_iter_from_list`, `ori_iter_next`, `ori_iter_drop`
- `ori_rc_is_unique` present in binary (228 bytes at `0x29f60`) -- called from within `ori_list_push_cow`, not from generated code

### S9: Comparison with Eval

Both paths produce identical results. Key semantic differences:
- **Eval**: COW handled in Rust `Value::List` clone-on-write via `Rc` refcount check
- **AOT**: COW handled in `ori_list_push_cow` runtime via `ori_rc_is_unique` check on raw pointer RC header
- Both implement the same COW invariant: clone when shared, mutate when unique

## COW-Specific Analysis

### Is `ori_rc_is_unique` called before mutation?

**YES** -- but indirectly. The LLVM IR calls `ori_list_push_cow` which internally calls `ori_rc_is_unique`. The check is NOT inlined into the generated IR. The runtime function at `ori_list_push_cow` (2,897 bytes, symbol at `0x1f740`) contains the full COW logic:

```rust
let is_unique = !data.is_null()
    && !is_slice_cap(cap)
    && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
```

### Does the IR show fast path and slow path?

**NO** -- the branching is inside the runtime function, not in the generated IR. The IR has a single `call void @ori_list_push_cow(...)`. Both paths are handled within the runtime. This is an architectural decision: the COW complexity is encapsulated in the runtime library rather than being generated inline by the compiler.

### Are RC operations correct?

**YES**. The sequence is:
1. `ori_list_rc_inc` on `original.data` (RC 1->2 for sharing with `modified`)
2. `ori_list_push_cow` consumes one ref (RC 2->1 on slow path, creates new buffer at RC=1)
3. Additional inc/dec pairs for `.length()` borrows and iterator creation
4. Final dec to free `modified`'s buffer

### Is original provably unmodified?

**YES** in the runtime. The `ori_rc_is_unique` check returns `false` (RC=2), forcing the slow path which allocates a new buffer. The original buffer is never written to after construction.

**NOT provable from IR alone** -- since the COW logic is in the runtime, the LLVM optimizer cannot prove that `original` is unmodified. This means LLVM cannot optimize reads from `original` after the push call (e.g., cannot hoist length extraction before push). A future optimization could emit `cow_mode=2` (force-shared) when the compiler statically knows a reference is shared.

### ARC balance: every inc has matching dec?

**YES**. See section 6.4 above for the complete tracking table.

## Findings

### New Findings

| ID | Severity | Description | Category |
|----|----------|-------------|----------|
| (none) | -- | No new findings | -- |

### Confirmed Findings

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| M1 | MEDIUM | Prelude overhead: 10,331 bytes constant (13/13 journeys) | CONFIRMED |
| M3 | MEDIUM | Unnecessary `br label` after calls (13/13 journeys) | CONFIRMED |
| M5 | MEDIUM | `align 4` on i64 operations -- push.elem store, list element stores | CONFIRMED |
| M11 | MEDIUM | Orphaned landing pads with no predecessors (3 in this journey) | CONFIRMED |
| L1 | LOW | Canon expansion 18.4% (within 0-25% range) | CONFIRMED |
| L2 | LOW | 4 prelude decision trees | CONFIRMED |
| L4 | LOW | Single-predecessor phi nodes at loop exit (bb9 from bb10, bb13 from bb14) | CONFIRMED |

### Optimization Opportunities (not bugs)

| Observation | Severity | Description |
|-------------|----------|-------------|
| OPT-1 | LOW | Static uniqueness analysis could emit `cow_mode=2` when sharing is provable, avoiding runtime `ori_rc_is_unique` call |
| OPT-2 | LOW | Inlining the COW fast-path check into IR (branch on `ori_rc_is_unique` then mutate-in-place vs call slow path) would let LLVM optimize around it |

## Summary

Journey 16 is the most important COW test -- it validates the fundamental value semantics invariant that modifying a shared list does not affect the original. Both the interpreter and AOT compiler handle this correctly.

The COW mechanism works by:
1. Incrementing RC when creating a shared reference (`modified = original`)
2. Calling `ori_list_push_cow` with `cow_mode=0` (dynamic check)
3. The runtime's `ori_rc_is_unique` check detects RC=2, takes the slow path
4. Slow path: allocate new buffer, copy elements, append new element, dec old buffer's RC
5. Result: `original` untouched at `[1,2,3]` (RC=1), `modified` is new `[1,2,3,4]` (RC=1)

No new critical, high, or medium findings. All previously identified patterns (M1, M3, M5, M11, L1, L2, L4) are confirmed. The ARC balance is correct. Value semantics are preserved.

This is the first journey that exercises the COW runtime path and the ARC lifecycle for mutating operations on shared collections. It passes cleanly with no regressions.
