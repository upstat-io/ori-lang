# Journey 3: "I am recursive"

**Code**:
```ori
@fib (n: int) -> int =
    if n <= 1 then n
    else fib(n - 1) + fib(n - 2);

@gcd (a: int, b: int) -> int =
    if b == 0 then a
    else gcd(b, a % b);

@main () -> int = {
    let f = fib(10);        // = 55
    let g = gcd(48, 18);    // = 6
    f + g                   // = 61
}
```
**Source**: 428 bytes, **Expected Result**: 61 (= 55 + 6)
**Actual**: Eval = 61 (correct), AOT = 61 (correct)

---

## Transformation Timeline

### Stage 1-2: Lexer
```
User:    428 bytes → 109 tokens (6 comments, 0 errors)
Prelude: 10,331 bytes → 1,516 tokens (unchanged)
```

### Stage 3: Parser
```
User:    109 tokens → 3 functions, 38 expressions, 0 errors
Prelude: 1,516 tokens → 9 functions, 39 traits, 46 expressions, 0 errors
```

### Stage 5: Canonicalizer
```
User:    3 functions, 38 source_exprs → 40 canon_nodes, 3 roots, 6 constants, 0 decision_trees
Prelude: 9 functions, 46 source_exprs → 46 canon_nodes, 9 roots, 6 constants, 4 decision_trees
```
- 5.3% expansion (38→40) — minimal for recursive code

### Stage 6a: Eval Path
```
Total eval_can calls:  1,813
Binary operations:     449
```
- **1,813 eval_can calls** — exponential Fibonacci. `fib(10)` makes ~177 recursive calls, each evaluating ~10 canon nodes. Plus `gcd(48,18)` with 4 recursive calls.
- 449 binary ops: comparisons (`<=`, `==`), subtractions (`n-1`, `n-2`), modulo (`a%b`), additions

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
define fastcc i64 @_ori_fib(i64 %0) personality ptr @rust_eh_personality {
bb0:
  %le = icmp sle i64 %0, 1
  br i1 %le, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  %sub = sub i64 %0, 1
  %call = invoke fastcc i64 @_ori_fib(i64 %sub)
          to label %bb4 unwind label %bb5
bb3:
  %v14 = phi i64 [ %0, %bb1 ], [ %add, %bb6 ]
  ret i64 %v14
bb4:
  %sub1 = sub i64 %0, 2
  %call2 = invoke fastcc i64 @_ori_fib(i64 %sub1)
          to label %bb6 unwind label %bb7
bb5:
  %lp = landingpad { ptr, i32 }  cleanup
  resume { ptr, i32 } %lp
bb6:
  %add = add i64 %call, %call2
  br label %bb3
bb7:
  %lp3 = landingpad { ptr, i32 }  cleanup
  resume { ptr, i32 } %lp3
}

define fastcc i64 @_ori_gcd(i64 %0, i64 %1) personality ptr @rust_eh_personality {
bb0:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %bb1, label %bb2
bb1:
  br label %bb3
bb2:
  %rem = srem i64 %0, %1
  %call = invoke fastcc i64 @_ori_gcd(i64 %1, i64 %rem)
          to label %bb4 unwind label %bb5
bb3:
  %v11 = phi i64 [ %0, %bb1 ], [ %call, %bb4 ]
  ret i64 %v11
bb4:
  br label %bb3
bb5:
  %lp = landingpad { ptr, i32 }  cleanup
  resume { ptr, i32 } %lp
}

define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  %call = invoke fastcc i64 @_ori_fib(i64 10)
          to label %bb1 unwind label %bb2
bb1:
  %call1 = invoke fastcc i64 @_ori_gcd(i64 48, i64 18)
          to label %bb3 unwind label %bb4
bb2:
  %lp = landingpad { ptr, i32 }  cleanup
  resume { ptr, i32 } %lp
bb3:
  %add = add i64 %call, %call1
  ret i64 %add
bb4:
  %lp2 = landingpad { ptr, i32 }  cleanup
  resume { ptr, i32 } %lp2
}

declare i32 @rust_eh_personality(i32) #0
```

#### Key Observations
1. **`invoke` replaces `call` for recursive functions** — J1/J2 used `call` (no unwind); J3 uses `invoke` with landing pads. The compiler detects that recursive functions might unwind (stack overflow, panic).
2. **Empty landing pads** — Every `invoke` has a `landingpad { ptr, i32 } cleanup` followed by `resume`. These do NO work — no cleanup needed for `i64` values. Pure overhead.
3. **`personality ptr @rust_eh_personality`** — Links against Rust's exception handling personality. This means AOT binaries depend on Rust's unwind infrastructure.
4. **`nounwind` attribute REMOVED** — J1/J2 had `#0 = { nounwind }` on user functions. J3 removes it because functions might unwind.
5. **`invoke` even in `@_ori_main`** — The caller also switches to `invoke` for calls to potentially-unwinding functions. Every call site in the module gets a landing pad.
6. **`srem` for modulo** — Correct signed remainder instruction for `%`
7. **`icmp sle/eq` for `<=` and `==`** — Correct comparison instructions
8. **`gcd` is tail-recursive** — The recursive call `gcd(b, a%b)` is in tail position, but NOT compiled as a tail call. LLVM could optimize this with `musttail` or loop transformation, but the codegen doesn't emit it.
9. **1 runtime declaration**: `declare i32 @rust_eh_personality(i32)` — first non-zero runtime dependency
10. **CONFIRMED M3**: Dead `br label` still present (bb1→bb3 in gcd, bb4→bb3 in main-like patterns)

---

## Issues Found

### CRITICAL
None.

### HIGH

**H1 (NEW): invoke + empty landing pads for ALL function calls when any function is recursive**
- In J1/J2 (non-recursive), functions used `call` with `nounwind`
- In J3, ALL functions use `invoke` + landing pads, even `@_ori_main`
- Landing pads just `resume` — no actual cleanup needed for `i64`
- For `fib(10)`: ~354 landing pad executions during normal execution (2 invokes × 177 calls)
- Impact: code size bloat (each invoke needs a landing pad block) and branch prediction pollution
- A smarter approach: mark leaf functions as `nounwind`, propagate up call graph

### MEDIUM

**M4 (NEW): Tail-recursive `gcd` not compiled as tail call**
- `gcd(b, a%b)` is in tail position but compiles to `invoke` (not `musttail`)
- Could be a loop instead — `srem` + conditional branch, no stack growth
- Recursive `gcd` with large inputs could overflow the stack in AOT

**CONFIRMED M2**: No `nsw` on `sub i64 %0, 1` etc. — wrapping arithmetic in AOT
**CONFIRMED M3**: Dead branches after calls (now mixed with invoke/landingpad pattern)

### LOW
**CONFIRMED L1**: Canon expansion 5.3% for recursive code
**CONFIRMED L3**: Trivial branch blocks still present

### CONFIRMED FROM PREVIOUS JOURNEYS
- M1: Prelude overhead (10,331 bytes)
- M2: No `nsw` flags
- M3: Dead branches
- L1: Canon expansion (varies: 25% → 7.5% → 5.3%)
- L2: Prelude decision trees (4)

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 61 | 61 |
| Exit code | 61 | 61 |
| Recursion | Works (1,813 eval steps) | Works (invoke-based) |
| gcd tail recursion | Stack growth | Stack growth (not optimized) |
