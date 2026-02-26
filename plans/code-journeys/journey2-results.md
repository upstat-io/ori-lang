# Journey 2: "I am arithmetic"

**Code**:
```ori
@add (a: int, b: int) -> int = a + b;
@multiply (x: int, y: int) -> int = x * y;
@main () -> int = {
    let sum = add(3, 4);
    let product = multiply(sum, 5);
    product + 1
}
```
**Source**: 182 bytes, **Result**: 36 (= (3+4)*5+1)

## Transformation Timeline

### Stage 1-2: Lexer
```
182 bytes → 78 tokens (0 errors)
```
Ratio: 2.3 bytes/token — consistent with the ~2-3 bytes/token design target.

### Stage 3: Parser
```
78 tokens → 3 functions, 18 expressions (0 errors)
```

ExprArena (18 nodes):
- `@add` body: `Binary(Add, Ident(a), Ident(b))` — 3 nodes
- `@multiply` body: `Binary(Mul, Ident(x), Ident(y))` — 3 nodes
- `@main` body: Block with 2 Let stmts + tail expr — 12 nodes:
  - `Let(sum, Call(Ident(add), [Int(3), Int(4)]))`
  - `Let(product, Call(Ident(multiply), [Ident(sum), Int(5)]))`
  - `Binary(Add, Ident(product), Int(1))`

### Stage 4: Type Checker
```
registration: 3 functions, 0 tests, 0 impls
signatures: 3 functions — all explicit (int, int) -> int
body checking: 3 functions — unification trivial (int + int → int)
```

No inference needed — all types are explicit. The type checker still runs 3 full passes (registration, signatures, bodies) even when types are fully annotated.

**FINDING: No fast path for fully-annotated functions.** When all parameter and return types are explicit and the body is trivially typed (e.g., `int + int`), the type checker could skip inference entirely. Currently runs full HM inference for `a + b` even when `a: int, b: int` is declared.

### Stage 5: Canonicalizer
```
canon lower_module started (functions=3, source_exprs=18)
canon lower_module complete (canon_nodes=20, roots=3, constants=6, decision_trees=0)
```

18 source exprs → 20 canon nodes. The +2 comes from desugaring (each function call has its function reference separated from the call node).

### Stage 6a: Eval Path
```
@main body:
  eval Block(stmts=[Let(sum,...), Let(product,...)], tail=Binary)
    eval Let(sum, Call(add, [3, 4]))
      eval Call:
        eval Ident(add) → FunctionValue
        eval Int(3) → Value::Int(3)
        eval Int(4) → Value::Int(4)
        → eval_call_value(add, [3, 4])
          eval Binary(Add, Ident(a), Ident(b))
            eval Ident(a) → Value::Int(3)
            eval Ident(b) → Value::Int(4)
            evaluate_binary(op=Add, left_type="int", right_type="int")
            → Value::Int(7)
      bind sum = Value::Int(7)
    eval Let(product, Call(multiply, [7, 5]))
      eval Call:
        eval Ident(multiply) → FunctionValue
        eval Ident(sum) → Value::Int(7)
        eval Int(5) → Value::Int(5)
        → eval_call_value(multiply, [7, 5])
          eval Binary(Mul, Ident(x), Ident(y))
            eval Ident(x) → Value::Int(7)
            eval Ident(y) → Value::Int(5)
            evaluate_binary(op=Mul, left_type="int", right_type="int")
            → Value::Int(35)
      bind product = Value::Int(35)
    eval Binary(Add, Ident(product), Int(1))
      eval Ident(product) → Value::Int(35)
      eval Int(1) → Value::Int(1)
      evaluate_binary(op=Add, left_type="int", right_type="int")
      → Value::Int(36)
```

**Total eval_can calls**: ~20 (each expression node visited once)
**Total evaluate_binary calls**: 3 (add's `+`, multiply's `*`, main's `+`)
**Total eval_call_value calls**: 2 (add, multiply)

**FINDING: Function call overhead in interpreter.** Each function call requires:
1. Eval the function identifier → look up in environment → get FunctionValue
2. Eval each argument
3. Create a new environment scope (push)
4. Bind parameters to argument values
5. Set up canon context (switch canon IR pointer)
6. Eval the function body
7. Pop the environment scope

For `add(3, 4)`, step 1 (environment lookup) + step 3-7 (scope management) dominate over the actual computation (one `add` instruction). This is expected for a tree-walking interpreter but worth noting as the baseline.

### Stage 6b: LLVM Path

#### ARC/Borrow Analysis
```
function_count=3
SCC decomposition: 3 SCCs (no recursion between add/multiply/main)
Borrow inference: 3 SCCs × 1 member each, none recursive
```

**FINDING: SCC decomposition is correct** — each function is its own SCC because there are no mutual/self-recursive calls. The cost is O(V+E) which is trivial here but confirms the graph analysis works.

#### Code Generation (Two-pass)
**Pass 1 — Declare:**
```
declare "add"      → _ori_add,      params=2, call_conv=Fast, return=Direct
declare "multiply" → _ori_multiply,  params=2, call_conv=Fast, return=Direct
declare "main"     → _ori_main,      params=0, call_conv=C,    return=Direct
```

**FINDING: call_conv=Fast for user functions, C for main.** `add` and `multiply` use LLVM's `fastcc` (register-passing, tail-call capable), while `main` uses the C calling convention for OS interop. This is correct architecture.

**Pass 2 — Define:**
```
define "add"      → ARC pre-lowered, tier=2
define "multiply" → ARC pre-lowered, tier=2
define "main"     → ARC pre-lowered, tier=2 + C main wrapper
```

#### Generated LLVM IR

```llvm
define fastcc i64 @_ori_add(i64 %0, i64 %1) {
bb0:
  %add = add i64 %0, %1
  ret i64 %add
}

define fastcc i64 @_ori_multiply(i64 %0, i64 %1) {
bb0:
  %mul = mul i64 %0, %1
  ret i64 %mul
}

define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  %invoke = invoke fastcc i64 @_ori_add(i64 3, i64 4)
          to label %bb1 unwind label %bb2
bb1:
  %invoke1 = invoke fastcc i64 @_ori_multiply(i64 %invoke, i64 5)
          to label %bb3 unwind label %bb4
bb3:
  %add = add i64 %invoke1, 1
  ret i64 %add
bb2:
  %lp = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp
bb4:
  %lp2 = landingpad { ptr, i32 } cleanup
  resume { ptr, i32 } %lp2
}

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

**Key observations:**

1. **`add` and `multiply` are optimal** — single instruction + ret. No stack allocation, no prologue overhead. LLVM will trivially inline these.

2. **`invoke` instead of `call`** — All user function calls use `invoke` with landing pads, even for functions that cannot panic (`add`, `multiply`). This is the panic/error propagation infrastructure.

3. **FINDING: Landing pads for non-panicking functions.** Each `invoke` generates a landing pad (`landingpad { ptr, i32 } cleanup / resume`). For `add` and `multiply` which just do arithmetic and can never panic, these are dead code. The LLVM optimizer should remove them, but they increase IR size and codegen time. A "nothrow" analysis could mark pure arithmetic functions as non-panicking and use `call` instead of `invoke`.

4. **FINDING: 90+ runtime declarations.** The IR starts with ~90 `declare` statements for runtime functions (`ori_print`, `ori_list_*`, `ori_map_*`, `ori_set_*`, `ori_str_*`, `ori_rc_*`, `ori_iter_*`, etc.) even though `journey2.ori` uses NONE of them. These are unconditionally declared. Dead declaration elimination by the linker removes them from the final binary, but they add ~2KB to the IR text.

5. **`personality ptr @rust_eh_personality`** on `_ori_main` — enables Rust-compatible exception handling for panic propagation.

6. **`trunc i64 to i32` in main wrapper** — the exit code is truncated from Ori's 64-bit int to the OS's 32-bit exit code. Correct but worth documenting that exit codes > 255 or < 0 will be truncated.

---

## Issues Found

### HIGH
1. **Landing pads for non-panicking functions** — `invoke` + landing pads generated for every user function call, even trivial arithmetic. A "nothrow" attribute based on body analysis could use `call` instead, reducing IR size and improving codegen.

2. **90+ unconditional runtime declarations** — All runtime functions declared even when unused. Lazy declaration (only declare what's called) would reduce IR size significantly.

### MEDIUM
3. **No fast path for fully-annotated type checking** — When all types are explicit, HM inference still runs. A quick check "are all types annotated?" could skip inference.

4. **Interpreter function call overhead** — 7 steps per call for what amounts to a single instruction. Relevant for tight loops calling small functions. Could benefit from inlining in the canonical IR.

### LOW
5. **18 AST nodes → 20 canon nodes** — Slight expansion from desugaring. Not a problem, but shows the canon IR is not always smaller.

### CONFIRMED FROM JOURNEY 1
6. **Double prelude processing** — Still present (type-checked and canonicalized twice).
7. **6 prelude types registered in LLVM** — Even though unused by this program.
