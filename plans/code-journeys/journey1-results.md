# Journey 1: "I am bare arithmetic"

**Code**:
```ori
// Journey 1: Bare arithmetic + let bindings
// Expected: (3 + 4) * 5 - 2 = 33
@main () -> int = {
  let a = 3;
  let b = 4;
  let c = a + b;   // 7
  let d = c * 5;   // 35
  d - 2             // 33
};
```
**Source**: 203 bytes, **Expected Result**: 33 (= (3+4)*5-2)
**Actual**: Eval = 33 (correct), AOT = 33 (correct)

## Transformation Timeline

### Stage 1-2: Lexer
```
User: 203 bytes → 47 tokens (5 comments, 0 errors) — 4.3 bytes/token
Prelude: 10,331 bytes → 1,516 tokens (126 comments, 0 errors) — 6.8 bytes/token
```
Prelude is 51x the user code by bytes, 32x by tokens.

### Stage 3: Parser
```
User: 47 tokens → 1 function, 0 tests, 0 types, 0 traits, 0 impls, 12 expressions, 0 errors
Prelude: 1,516 tokens → 9 functions, 0 tests, 0 types, 39 traits, 0 impls, 46 expressions, 0 errors
```

### Stage 4: Type Checker
```
User: registration=1 functions, 0 tests, 0 impls; signatures=1; body checking=1
Prelude: registration=9 functions, 0 tests, 0 impls; signatures=9; body checking=9
```
No mono instances recorded (no generics).

### Stage 5: Canonicalizer
```
User: canon lower_module (functions=1, source_exprs=12) → (canon_nodes=16, roots=1, constants=6, decision_trees=0)
Prelude: canon lower_module (functions=9, source_exprs=46) → (canon_nodes=46, roots=9, constants=6, decision_trees=4)
```
12 source expressions → 16 canon nodes (4 extra from block/let structure).

### Stage 6a: Eval Path
```
18 eval_can calls total:
  - 4 Int literals (3, 4, 5, 2)
  - 4 Ident lookups (a, b, c, d)
  - 3 Binary ops (Add, Mul, Sub) — all int×int
  - 4 Let bindings
  - 1 Block
```
Minimal, clean execution. No unnecessary overhead.

### Stage 6b: LLVM Path

#### Generated LLVM IR (user functions only)
```llvm
; Function Attrs: nounwind
define i64 @_ori_main() #1 {
bb0:
  ret i64 33
}

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

#### Key Observations
1. **Constant folding**: All arithmetic computed at compile time — `_ori_main` returns `33` directly
2. **Nounwind**: `_ori_main` correctly marked `nounwind` (no panic possible)
3. **98 runtime declarations** for a program that calls zero runtime functions
4. **2 function definitions** (`_ori_main` + `main` wrapper)
5. **No ARC ops**: No `rc_inc`/`rc_dec` — correct for int-only code

---

## Issues Found

### HIGH
1. **[NEW] 98 eager runtime declarations for zero-usage program** — Every runtime function is declared even though `_ori_main` calls none of them. LLVM's linker will strip them, but it's unnecessary IR bloat and compile-time overhead. This was addressed by Task #5 (lazy runtime declarations) in the previous session but the fix appears incomplete — all 98 are still emitted.

### MEDIUM
2. **[CONFIRMED] Prelude overhead ratio** — For a 203-byte program, the compiler also processes 10,331 bytes of prelude (51:1 ratio). Prelude: 1,516 tokens, 9 functions, 39 traits, 46 expressions, 46 canon nodes. This is unavoidable for correctness but worth noting as baseline.

3. **[NEW] `cargo run` silently strips LLVM feature** — Running `cargo run -- run` rebuilds `oric` WITHOUT `--features llvm`, overwriting the LLVM-enabled binary at `target/debug/ori`. The symlink at `~/.local/bin/ori` then points to a non-LLVM binary. This is a developer experience trap — any `cargo run` invocation silently breaks AOT until next `cargo bl`.

### LOW
4. **[NEW] Canon node expansion ratio** — 12 source expressions → 16 canon nodes (+33%). The 4 extra nodes come from block structure and let binding wrappers. Minimal overhead, not actionable.

---

## Eval vs LLVM Behavioral Mismatch
None — both produce exit code 33.
