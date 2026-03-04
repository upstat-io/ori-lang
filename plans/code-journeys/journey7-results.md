# Journey 7 Results: "I am a loop"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 30)

## Source

```ori
@sum_loop (n: int) -> int = {
    let i = 0; let total = 0;
    loop { if i >= n then break; total += i + 1; i += 1 };
    total
}
@sum_for (n: int) -> int = {
    let total = 0; for x in 1..=n do total += x; total
}
@main () -> int = { let a = sum_loop(n: 5); let b = sum_for(n: 5); a + b }
```

**Features exercised**: `loop`/`break`, `for..in..do`, inclusive range (`..=`), mutable `let`, compound assignment (`+=`), integer comparison (`>=`), named arguments, block expressions, multiple function calls.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 30        | 30       | (none) | (none) | PASS   |
| AOT     | 30        | 30       | (none) | Compile time 0.37s | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 552 bytes, 132 tokens, 0 errors
- Prelude: 10331 bytes, 1516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- 3 functions, 45 expressions, 0 errors, 0 warnings
- Correctly parsed `loop`/`break`, `for..in..do`, inclusive range `..=`, compound assignment `+=`
- Parse contexts entered: function definition, expression, if expression, for loop, function call
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors

### Canonicalization
- User module: 3 functions, 50 canon nodes, 6 constants, 0 decision trees
- Prelude: 9 functions, 46 canon nodes, 4 decision trees
- Clean pass.

### Type Checker
- Registration, signature collection, body checking: all complete, 0 errors
- Same 6 generic prelude imports with AST fallback (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`)
- Same 3 hash-first hits (`compare`, `min`, `max`)
- User module: 3 functions, 0 tests, 0 impls -- checked successfully

### ARC Pipeline
- Type registration: Ordering, PanicInfo, TraceEntry, FormatSpec-related enums/structs (same as J2)
- 3 user functions declared: `sum_loop`, `sum_for`, `main`
- `sum_loop` and `sum_for` use `fastcc`; `main` uses C ABI
- Nounwind analysis: 2 fixed-point passes, 1 function (`sum_loop`) marked nounwind; `sum_for` not marked nounwind (range zero-step panic path exists)
- Entry point wrapper: `main()` -> `_ori_main()` with `trunc i64 to i32`, `has_args=false`, `returns_int=true`

### Evaluator
- 156 trace lines total, demonstrating 5 loop iterations for each function
- `sum_loop(5)`:
  - `Let i = 0`, `Let total = 0`
  - Loop iteration 1: `GtEq(0, 5)` = false, `Assign total = 0 + (0+1) = 1`, `Assign i = 0+1 = 1`
  - Loop iteration 2: `GtEq(1, 5)` = false, `Assign total = 1 + (1+1) = 3`, `Assign i = 1+1 = 2`
  - Loop iteration 3: `GtEq(2, 5)` = false, `Assign total = 3 + (2+1) = 6`, `Assign i = 2+1 = 3`
  - Loop iteration 4: `GtEq(3, 5)` = false, `Assign total = 6 + (3+1) = 10`, `Assign i = 3+1 = 4`
  - Loop iteration 5: `GtEq(4, 5)` = false, `Assign total = 10 + (4+1) = 15`, `Assign i = 4+1 = 5`
  - Loop iteration 6: `GtEq(5, 5)` = true, `break` -> returns `total = 15`
- `sum_for(5)`:
  - `Let total = 0`, for loop over `1..=5`
  - 5 iterations: `Assign total` via `Add(total, x)` for x = 1,2,3,4,5
  - Returns `total = 15`
- `main`: `Add(15, 15)` = 30
- All operations correct, no errors.

## LLVM Deep Scrutiny (9 Categories)

### 1. Attributes & Calling Convention

| Function | fastcc | nounwind | Status |
|----------|--------|----------|--------|
| `_ori_sum_loop` | Yes | Yes | OK |
| `_ori_sum_for` | Yes | No | OK -- has panic path (zero-step range) |
| `_ori_main` | No (C ABI) | No | OK -- calls `_ori_sum_for` which may throw |
| `main` (wrapper) | No (C ABI) | No | OK |
| `ori_panic_cstr` | No | No | OK -- `cold` attr present |
| `ori_panic` | No | No | OK -- `cold` attr present |

**Verdict**: Correct. `sum_loop` can only exit via `break` or overflow, and overflow paths end in `unreachable`, so `nounwind` is appropriate. `sum_for` contains a runtime check for zero step (calls `ori_panic`), so it correctly lacks `nounwind`. Attribute hygiene is sound.

**Severity**: None.

### 2. Control Flow & Branch Structure

**`_ori_sum_loop`** (7 blocks: bb0, bb1, bb2, bb3, bb4, bb5, add.ok, add.ok4, add.ok9, plus panic blocks):
- bb0: entry, initializes `i=0`, `total=0`, branches to bb1
- bb1: loop header with phi nodes for `i` and `total`, checks `i >= n`, branches to bb3 (break) or bb4 (body)
- bb3 -> bb2: break path, returns `total`
- bb4 -> bb5: body, computes `i+1` with overflow check, then `total + (i+1)` with overflow check, then `i+1` again for increment
- add.ok9 -> bb1: loop back edge
- Correct loop structure with phi-based loop variables.

**`_ori_sum_for`** (8 blocks + panic blocks):
- bb0: constructs range tuple `{start=1, end=n, step=1, inclusive=1}`, checks step==0 -> panic or continue
- bb7 -> bb1: initializes loop with `current=start`, `total=0`
- bb1: range iteration check (complex multi-way: `step>0 && current<end`, `step<0 && current>end`, `inclusive && current==end`)
- bb2 -> bb3: body, `total += current`, `current += step` with overflow checks
- bb5 -> bb4: loop exit, returns `total`
- bb6: zero-step panic path with `ori_panic("range step cannot be zero")`
- Correct for-loop / range iteration structure.

**`_ori_main`** (linear):
- bb0: calls `_ori_sum_loop(5)`, bb1: calls `_ori_sum_for(5)`, bb3: adds results with overflow check, returns.

**Verdict**: All control flow correct. Loop back edges properly formed. No unreachable blocks (except post-panic `unreachable` terminators). Range iteration logic handles all three range bound conditions correctly.

**Severity**: None.

### 3. Phi Nodes & Value Flow

| Function | Phi Node | Values | Correctness |
|----------|----------|--------|-------------|
| `_ori_sum_loop` bb1 | `%v5 = phi [0, bb0], [%add.val7, add.ok9]` | Loop counter `i` | Correct |
| `_ori_sum_loop` bb1 | `%v6 = phi [0, bb0], [%add.val2, add.ok9]` | Accumulator `total` | Correct |
| `_ori_sum_loop` bb2 | `%v26/%v27/%v28` phis from bb3 | Break value extraction | Correct (returns `%v28 = total`) |
| `_ori_sum_for` bb1 | `%v8 = phi [%proj.0, bb7], [%add.val9, add.ok11]` | Range current value | Correct |
| `_ori_sum_for` bb1 | `%v9 = phi [0, bb7], [%v10, add.ok11]` | Accumulator `total` | Correct |
| `_ori_sum_for` bb3 | `%v10 = phi [%add.val, add.ok]` | Updated total after add | Correct |
| `_ori_sum_for` bb4 | `%v11/%v12` phis from bb5 | Loop exit values | Correct (returns `%v12 = total`) |

**Verdict**: All phi nodes correct. Loop-carried values properly threaded. Break extraction phi chain is correct (though verbose -- see finding #5).

**Severity**: None.

### 4. Overflow Checking

**`_ori_sum_loop`**: 3 overflow-checked additions per iteration:
1. `i + 1` (inner expression `i + 1`) -- checked via `@llvm.sadd.with.overflow.i64`
2. `total + (i+1)` (compound assignment) -- checked
3. `i + 1` (counter increment `i += 1`) -- checked

**Issue (LOW)**: The computation `i + 1` is performed twice per iteration (lines 40-43 and 56-59 in IR). Once for `total += i + 1` and again for `i += 1`. The expression `i + 1` could be computed once and reused, saving one `@llvm.sadd.with.overflow.i64` intrinsic call and associated overflow check per iteration. This is a CSE (common subexpression elimination) opportunity.

**`_ori_sum_for`**: 2 overflow-checked additions per iteration:
1. `total + current` -- checked
2. `current + step` -- checked

**`_ori_main`**: 1 overflow-checked addition (`a + b`).

**Range zero-step check**: `sum_for` correctly checks `step == 0` at range construction and panics with "range step cannot be zero" message. This is semantically correct -- `1..=n by 0` is undefined.

**Severity**: LOW for duplicate `i+1` computation. All overflow checks present and correct.

### 5. Redundant Blocks & Unnecessary Branches

**`_ori_sum_loop`**: bb3 -> bb2 is a two-block chain for break handling:
```llvm
bb3:                      ; from bb1 (break)
  br label %bb2
bb2:                      ; from bb3
  %v26 = phi i64 [ 0, %bb3 ]      ; unused constant
  %v27 = phi i64 [ %v5, %bb3 ]    ; unused (i value)
  %v28 = phi i64 [ %v6, %bb3 ]    ; used (total)
  ret i64 %v28
```
- bb3 is trivially foldable into bb2 (single predecessor)
- `%v26` and `%v27` are dead values (never used after phi)
- Could be: `bb3: ret i64 %v6` (single block, no phi needed)

**`_ori_sum_loop`**: bb4 -> bb5 is a trivial bridge:
```llvm
bb4:    ; from bb1 (loop body)
  br label %bb5
bb5:    ; from bb4
  %v13 = phi i64 [ 0, %bb4 ]    ; dead value
  ...
```
- Single-predecessor phi with constant -- completely dead

**`_ori_sum_for`**: Same pattern -- bb5 -> bb4 is trivial bridge for loop exit.

**`_ori_main`**: bb0 -> bb1, bb1 -> bb3 are unnecessary sequential block breaks between function calls (same pattern as J2).

**Severity**: MEDIUM -- consistent with J2 findings. The loop break path is particularly verbose (3 dead phi values for a simple `ret`). The extra blocks and dead phis increase IR size and slow down LLVM's optimization passes.

### 6. Duplicate `i + 1` Computation (New Finding)

In `_ori_sum_loop`, the expression `i + 1` appears in two contexts:
1. `total += i + 1` -- the value added to total
2. `i += 1` -- the counter increment

The LLVM IR computes this twice:
```llvm
; First computation (for total += i + 1):
%add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v5, i64 1)
%add.val = extractvalue { i64, i1 } %add, 0
...
; Second computation (for i += 1):
%add6 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v5, i64 1)
%add.val7 = extractvalue { i64, i1 } %add6, 0
```

Both compute `%v5 + 1`. The codegen should recognize that `i + 1` is the same subexpression and reuse the result. This would eliminate one intrinsic call, one overflow check branch, and one panic block per loop iteration.

**Severity**: LOW -- LLVM CSE/GVN will catch this at `-O1+`, but the unoptimized IR is doing redundant work. For debug-mode performance, this is a minor concern.

### 7. Range Iteration Complexity

The range iteration condition in `sum_for` is notably complex:

```llvm
%lt2 = icmp slt i64 %v8, %proj.1       ; current < end
%gt3 = icmp sgt i64 %v8, %proj.1       ; current > end
%eq4 = icmp eq i64 %v8, %proj.1        ; current == end
%and = and i1 %gt, %lt2                 ; step>0 && current<end
%and5 = and i1 %lt, %gt3               ; step<0 && current>end
%or = or i1 %and, %and5                ; ascending or descending in bounds
%and6 = and i1 %gt1, %eq4              ; inclusive && current==end
%or7 = or i1 %or, %and6                ; final condition
br i1 %or7, label %bb2, label %bb5
```

This is 8 instructions for the range bounds check, evaluated every iteration. The step direction (`%gt`, `%lt`, `%gt1`) are loop-invariant values computed once in bb0, which is correct. The comparison against `%proj.1` (end) is the per-iteration work.

For the common case of `1..=n by 1` (step=1, inclusive), this could be specialized to a single `icmp sle i64 %current, %end`. However, the generic range iteration supports arbitrary step and inclusive/exclusive, so the general form is necessary for correctness.

**Severity**: LOW -- correct and handles all range variants. A range specialization pass could optimize common cases but is not required for correctness.

### 8. Native Code Quality (Disassembly)

**`_ori_sum_loop`** (204 bytes, 0xcc size):
- Stack frame: 0x48 (72 bytes) for loop variables + temporaries
- Loop body uses stack-based variable storage (loads/stores each iteration) rather than register allocation
- Overflow checks use `inc` + `seto` + `jo` pattern -- functionally correct
- Loop back edge at 0x1b14b: `jmp 1b0aa` -- proper unconditional jump
- ~50 instructions for a 5-iteration loop with 2 adds per iteration

**`_ori_sum_for`** (485 bytes, 0x1e5 size):
- Stack frame: 0xa8 (168 bytes) -- notably larger due to range tuple, panic string, and SSO checks
- Range construction at entry uses `insertvalue`/`extractvalue` pattern lowered to register moves
- Bounds check (8-instruction sequence) at 0x1b1d4-0x1b1f5 -- correct
- Zero-step panic path at 0x1b25e-0x1b2b3: constructs `OriStr`, calls `ori_panic`, then SSO/RC cleanup (dead code after `ori_panic` -- see finding)
- Loop body: add + overflow check, step + overflow check, jump back

**`_ori_main`** (77 bytes, 0x4d size):
- Clean: call `_ori_sum_loop(5)`, save result, call `_ori_sum_for(5)`, add with overflow check, return
- Efficient use of call-saved registers for intermediate values

**`_ori_drop$3`** (18 bytes):
- Minimal: `call ori_rc_free(ptr, 24, 8)` -- correct drop for 24-byte str (3 x i64)

**`main` (wrapper)** (8 bytes):
- Minimal: `call _ori_main`, return truncated result

**Severity**: LOW -- debug-mode stack spills are expected. The `sum_for` function is notably larger than `sum_loop` due to the generalized range iteration machinery.

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,567,280 bytes (6.26 MiB) |
| .text | 891,025 bytes (870 KiB) |
| .rodata | 136,591 bytes (133 KiB) |
| User code | ~789 bytes (sum_loop 0xcc + sum_for 0x1e5 + main 0x4d + drop 0x12) |
| User code % | 0.089% of .text |
| Debug info | ~4.8 MiB (.debug_*) |

User code footprint grew from J2's ~307 bytes to ~789 bytes, reflecting the loop and range iteration machinery. Still negligible compared to the runtime.

**Severity**: None -- expected for debug builds.

## Findings Summary

| # | Category | Severity | Description |
|---|----------|----------|-------------|
| 1 | Duplicate computation | LOW | `i + 1` computed twice per loop iteration in `sum_loop` (CSE opportunity) |
| 2 | Redundant blocks | MEDIUM | Break path emits trivial bridge blocks with dead phi values (bb3->bb2 pattern) |
| 3 | Dead phi values | LOW | `%v26` (constant 0), `%v27` (unused i) in break path, `%v13` (dead) in body entry |
| 4 | Sequential block merging | LOW | `_ori_main` has 2 unnecessary `br` between sequential call blocks (same as J2) |
| 5 | Overflow message dedup | LOW | 6 identical `ovf.msg` constants for "integer overflow on addition" (was 2 in J2) |
| 6 | Range iteration cost | LOW | 8-instruction bounds check per iteration; correct but could specialize common cases |
| 7 | Dead code after panic | LOW | SSO/RC cleanup code after `ori_panic` call in `sum_for` bb6 is unreachable (panic never returns) |
| 8 | Nounwind on sum_for | OK | Correctly NOT marked nounwind due to zero-step panic path |

## Cross-Journey Observations (vs J2)

- **New features working**: `loop`/`break`, `for..in..do`, inclusive range `..=`, compound assignment `+=`, mutable `let` with mutation
- **Consistent patterns**: Same `fastcc` convention, same block structure verbosity, same overflow message duplication (now 6x instead of 2x)
- **Nounwind analysis improved**: J2 had all 4 functions nounwind; J7 correctly distinguishes -- `sum_loop` is nounwind (only breaks/overflows), `sum_for` is not (panic path for zero step)
- **Phi node correctness**: Loop-carried phi nodes (new pattern) are correctly formed with proper back-edge predecessors
- **Loop codegen**: Functional and correct. The phi-based loop variable threading is the standard SSA pattern. No issues with loop back edges or break semantics.
- **Range iteration**: Generalized range iteration works correctly for the `1..=n` case. The zero-step guard is a good defensive measure.

## Actionable Items

1. **Duplicate `i+1` CSE** (LOW): The codegen emits `i + 1` twice when `total += i + 1; i += 1` both reference it. A local CSE pass or smarter compound assignment lowering could eliminate the duplicate.

2. **Break path simplification** (MEDIUM): When `break` exits a loop, the break path should directly return the accumulated value rather than constructing a phi chain through bridge blocks with dead values. This is the same redundant-block pattern from J2 but now affects loop break paths.

3. **Overflow message dedup** (LOW): 6 identical string constants for addition overflow. Single global suffices. This has grown from 2 in J2 -- the duplication scales with program complexity.

4. **Dead code after panic** (LOW): The RC cleanup code after `ori_panic()` in the zero-step range path is unreachable since `ori_panic` never returns. The codegen should skip generating cleanup code after known-noreturn calls.
