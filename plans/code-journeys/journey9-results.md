# Journey 9: "I am a string"

**Code**:
```ori
@bool_to_int (b: bool) -> int = if b then 1 else 0;

@check_logic () -> int = {
    let a = true && true;       // true  -> 1
    let b = true && false;      // false -> 0
    let c = false || true;      // true  -> 1
    let d = false || false;     // false -> 0
    bool_to_int(b: a) + bool_to_int(b: b) + bool_to_int(b: c) + bool_to_int(b: d)
    // = 1 + 0 + 1 + 0 = 2
}

@check_strings () -> int = {
    let s1 = "hello";
    let s2 = "world!";
    let s3 = "";
    s1.length() + s2.length() + s3.length()
    // = 5 + 6 + 0 = 11
}

@main () -> int = {
    let a = check_logic();      // = 2
    let b = check_strings();    // = 11
    a + b                       // = 13
}
```
**Source**: 861 bytes, **Expected Result**: 13 (= 2 + 11)
**Actual**: Eval = 13 (correct), AOT = 13 (correct)

---

## Transformation Timeline

### Stages
```
Lexer:   861 bytes → 177 tokens (12 comments, 0 errors)
Parser:  177 tokens → 4 functions, 0 types, 52 expressions, 0 errors
TypeCk:  4 functions registered, signatures collected, bodies checked — no mono instances
Canon:   4 functions, 52 source_exprs → 65 canon_nodes, 4 roots, 6 constants, 0 decision_trees
Eval:    67 eval_can calls — modest for 4 functions with boolean + string ops
```
- 25% canon expansion (52→65) — highest so far, driven by boolean && / || desugaring into if/then/else + `let` bindings for string variables
- 67 eval_can calls — reasonable for 4 `bool_to_int` calls + 3 `.length()` calls

### Stage 6b: LLVM Path

#### Generated LLVM IR (formatted)
```llvm
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [7 x i8] c"world!\00", align 1
@str.2 = private unnamed_addr constant [1 x i8] zeroinitializer, align 1

; --- bool_to_int: branch+phi pattern ---
define fastcc i64 @_ori_bool_to_int(i1 %0) #0 {
bb0:
  br i1 %0, label %bb1, label %bb2
bb1:
  br label %bb3       ; dead branch (M3)
bb2:
  br label %bb3       ; dead branch (M3)
bb3:
  %v4 = phi i64 [ 1, %bb1 ], [ 0, %bb2 ]
  ret i64 %v4
}

; --- check_logic: boolean ops constant-folded! ---
define fastcc i64 @_ori_check_logic() #0 {
bb0:
  %call = call fastcc i64 @_ori_bool_to_int(i1 true)    ; true && true → true
  br label %bb1
bb1:
  %call1 = call fastcc i64 @_ori_bool_to_int(i1 false)  ; true && false → false
  br label %bb3
bb3:
  %add = add i64 %call, %call1
  %call2 = call fastcc i64 @_ori_bool_to_int(i1 true)   ; false || true → true
  br label %bb5
bb5:
  %add3 = add i64 %add, %call2
  %call4 = call fastcc i64 @_ori_bool_to_int(i1 false)  ; false || false → false
  br label %bb7
bb7:
  %add5 = add i64 %add3, %call4
  ret i64 %add5
}

; --- check_strings: ARC lifecycle for string operations ---
define fastcc i64 @_ori_check_strings() personality ptr @rust_eh_personality {
bb0:
  %str.val = call { i64, ptr } @ori_str_from_raw(ptr @str, i64 5)
  %str.val1 = call { i64, ptr } @ori_str_from_raw(ptr @str.1, i64 6)
  %str.val2 = call { i64, ptr } @ori_str_from_raw(ptr @str.2, i64 0)
  %str.len = extractvalue { i64, ptr } %str.val, 0       ; .length() = field 0 extract!
  br label %bb1
bb1:
  %rc_dec.fat_data = extractvalue { i64, ptr } %str.val, 1
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; free s1
  %str.len3 = extractvalue { i64, ptr } %str.val1, 0
  br label %bb3

bb2:                                              ; No predecessors!  ← DEAD LANDING PAD
  %lp = landingpad { ptr, i32 } cleanup
  ; ... ARC cleanup for str.val1, str.val2 ...
  resume { ptr, i32 } %lp

bb3:
  %rc_dec.fat_data6 = extractvalue { i64, ptr } %str.val1, 1
  call void @ori_rc_dec(ptr %rc_dec.fat_data6, ptr @"_ori_drop$3")  ; free s2
  %add = add i64 %str.len, %str.len3
  %str.len7 = extractvalue { i64, ptr } %str.val2, 0
  br label %bb5

bb4:                                              ; No predecessors!  ← DEAD LANDING PAD
  %lp8 = landingpad { ptr, i32 } cleanup
  ; ... ARC cleanup for str.val2 ...
  resume { ptr, i32 } %lp8

bb5:
  %rc_dec.fat_data10 = extractvalue { i64, ptr } %str.val2, 1
  call void @ori_rc_dec(ptr %rc_dec.fat_data10, ptr @"_ori_drop$3")  ; free s3
  %add11 = add i64 %add, %str.len7
  ret i64 %add11

bb6:                                              ; No predecessors!  ← DEAD LANDING PAD
  %lp12 = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp12
}

; --- main: invoke for ARC-bearing callee ---
define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  %call = call fastcc i64 @_ori_check_logic()
  br label %bb1
bb1:
  %call1 = invoke fastcc i64 @_ori_check_strings()
          to label %bb3 unwind label %bb4
bb3:
  %add = add i64 %call, %call1
  ret i64 %add
bb4:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
}

; --- Runtime declarations ---
declare i32 @rust_eh_personality(i32) #0
declare { i64, ptr } @ori_str_from_raw(ptr, i64)
declare void @ori_rc_free(ptr, i64, i64) #0
declare void @ori_rc_dec(ptr, ptr) #2

; --- Generated drop function ---
define void @"_ori_drop$3"(ptr %0) #1 {
entry:
  call void @ori_rc_free(ptr %0, i64 16, i64 8)
  ret void
}
```

#### Key Observations
1. **Boolean `&&`/`||` constant-folded** — `true && true` → `i1 true`, `true && false` → `i1 false`. No short-circuit branches generated for constant operands. Excellent optimization.
2. **`.length()` is zero-cost** — compiles to `extractvalue { i64, ptr } %str.val, 0`. Just extracts field 0 from the fat pointer. No function call, no runtime overhead.
3. **String representation: `{ i64, ptr }`** — fat pointer: length (i64) + data pointer (ptr to RC-managed allocation). Field 0 = length enables the zero-cost `.length()`.
4. **String constants as null-terminated globals** — `c"hello\00"` with correct sizes. `ori_str_from_raw(ptr, i64 len)` wraps them in ARC-managed fat pointers.
5. **ARC lifecycle correct** — Each string is created via `ori_str_from_raw`, length extracted, then freed via `ori_rc_dec` with a drop function. Drop function calls `ori_rc_free(ptr, i64 16, i64 8)` — 16-byte allocation, 8-byte alignment.
6. **3 DEAD landing pads in `check_strings`** — bb2, bb4, bb6 all have `; No predecessors!`. The ARC emitter generates cleanup landing pads but connects all calls via `call` (not `invoke`), leaving the pads orphaned.
7. **`_ori_main` correctly uses `invoke` for `check_strings`** — because `check_strings` has personality (ARC cleanup). But its own landing pad (bb4) just resumes — no cleanup needed in main since it holds no ARC values.
8. **`check_logic()` has `nounwind`** — correct, pure boolean logic with no runtime calls.
9. **`check_strings()` lacks `nounwind`** — correct, calls `ori_str_from_raw` and `ori_rc_dec` which may panic.
10. **`_ori_main` lacks `nounwind`** — CONFIRMED M10 again. Should propagate from callees.
11. **4 runtime declarations** — `rust_eh_personality`, `ori_str_from_raw`, `ori_rc_free`, `ori_rc_dec`. First journey needing runtime ARC functions.
12. **CONFIRMED M3**: Dead branches after calls (4 instances in `check_logic`, 1 in `_ori_main`)
13. **`bool_to_int` uses branch+phi** — Could use `select i1 %0, i64 1, i64 0` instead (CONFIRMED L3)

---

## Issues Found

### CRITICAL
None.

### HIGH
None.

### MEDIUM
**M11 (NEW): Orphaned landing pads with no predecessors in ARC cleanup code**
- `check_strings()` has 3 landing pad blocks (bb2, bb4, bb6) all with `; No predecessors!`
- The ARC emitter generates cleanup blocks for each ARC-managed value scope
- But the actual runtime calls (`ori_str_from_raw`, `ori_rc_dec`) use `call` not `invoke`
- Result: landing pads are never reachable — pure dead code
- Each orphaned pad includes unnecessary `ori_rc_dec` calls and `resume` instructions
- Impact: IR bloat, potentially confuses optimization passes

**CONFIRMED M3**: Dead branches after calls (5 instances)
**CONFIRMED M10**: `_ori_main` missing `nounwind` attribute

### LOW
**CONFIRMED L3**: `bool_to_int` uses branch+phi instead of select for trivial `if b then 1 else 0`

### What Works Exceptionally Well
- **Boolean constant folding**: `&&`/`||` with constant operands fully resolved at compile time
- **Zero-cost `.length()`**: Compiled to single `extractvalue` — no function call
- **String ARC lifecycle**: Correct create/use/free pattern for all 3 strings
- **ARC-aware calling**: `_ori_main` correctly uses `invoke` for `check_strings` (has personality)
- **`nounwind` analysis**: Correctly applied to `check_logic` (pure) but not `check_strings` (runtime calls)
- **Drop function generation**: `_ori_drop$3` correctly sized for string allocation

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM |
|--------|------|------|
| Result | 13 | 13 |
| Boolean ops | Short-circuit evaluation | Constant folded (even better) |
| String length | Runtime method dispatch | Zero-cost extractvalue |
| String lifecycle | GC-free (eval manages internally) | Explicit ARC (create/dec/free) |
| Runtime declarations | 0 | 4 (ARC + EH personality) |
