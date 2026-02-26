# Journey 3: "I am generics"

**Code**:
```ori
// Journey 3: Generics + type inference
// Expected: identity(7) + first(10, 20) = 7 + 10 = 17
@identity<T> (x: T) -> T = x;
@first<A, B> (a: A, b: B) -> A = a;
@main () -> int = identity(7) + first(10, 20);
```
**Source**: 210 bytes, **Expected Result**: 17 (= 7 + 10)
**Actual**: Eval = 17 (correct), AOT = 17 (correct)

## Transformation Timeline

### Stage 1-2: Lexer
```
User: 210 bytes → 63 tokens (2 comments, 0 errors) — 3.3 bytes/token
Prelude: 10,331 bytes → 1,516 tokens (unchanged)
```

### Stage 3: Parser
```
User: 63 tokens → 3 functions, 10 expressions, 0 errors
```

### Stage 4: Type Checker
```
User: registration=3; signatures=3; body checking=3
Mono instances recorded:
  - identity<int> (1 type arg)
  - first<int, int> (2 type args)
```
Type inference correctly resolves `T=int`, `A=int, B=int` from call sites.

### Stage 5: Canonicalizer
```
User: functions=3, source_exprs=10 → canon_nodes=10, roots=3, constants=6, decision_trees=0
```

### Stage 6a: Eval Path
Correct. Generics are erased at eval time — `identity(7)` just returns `7`, `first(10, 20)` returns `10`.

### Stage 6b: LLVM Path

#### Generated LLVM IR
```llvm
define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  %call = invoke fastcc i64 @"_ori_identity$24m$24int"(i64 7)
          to label %bb1 unwind label %bb2
bb1:
  %call1 = invoke fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)
          to label %bb3 unwind label %bb4
bb2:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
bb3:
  %add = add i64 %call, %call1
  ret i64 %add
bb4:
  %lp2 = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp2
}

; Function Attrs: nounwind
define fastcc i64 @"_ori_first$24m$24int_int"(i64 %0, i64 %1) #1 {
bb0:
  ret i64 %0
}

; Function Attrs: nounwind
define fastcc i64 @"_ori_identity$24m$24int"(i64 %0) #1 {
bb0:
  ret i64 %0
}
```

#### Key Observations
1. **Monomorphization works**: `identity<T>` → `_ori_identity$24m$24int`, `first<A,B>` → `_ori_first$24m$24int_int`
2. **Monomorphized functions are nounwind**: Both callees marked `#1` (nounwind)
3. **But main still uses invoke**: `_ori_main` uses `invoke` with landing pads despite callees being nounwind
4. **Compilation order issue**: `_ori_main` is compiled BEFORE the monomorphized functions. At that point, monomorphized names aren't in `nounwind_functions` yet, so the nounwind analysis conservatively uses `invoke`.
5. **Name mangling**: `$24m$24` encodes the monomorphization parameters

---

## Issues Found

### HIGH
1. **[NEW] Nounwind analysis doesn't cover monomorphized callees** — `_ori_main` calls `identity<int>` and `first<int,int>`, both of which are trivially nounwind (just `ret`). But `_ori_main` still uses `invoke` with landing pads because the monomorphized functions are compiled AFTER `_ori_main`, so they're not in `nounwind_functions` when `_ori_main`'s nounwind check runs. This means any caller of a generic function always gets unnecessary landing pads.
   - **Impact**: All generic function calls pay the `invoke` + landing pad overhead unnecessarily
   - **Potential fix**: Two-pass approach — compile all functions once, analyze nounwind, then re-emit callers that could benefit. Or: topological ordering so callees are compiled before callers.

### MEDIUM
2. **[CONFIRMED] 98 eager runtime declarations** — Still all present for zero runtime usage.
3. **[CONFIRMED] Dead blocks in nounwind functions** — Not applicable here since main doesn't use nounwind calls, but the pattern persists.

---

## Eval vs LLVM Behavioral Mismatch
None — both produce 17.
