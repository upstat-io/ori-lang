# Journey 3: "I am generic"

**Code**:
```ori
@identity<T> (x: T) -> T = x;

type Pair<A, B> = { first: A, second: B }

@make_pair<A, B> (a: A, b: B) -> Pair<A, B> = Pair { first: a, second: b };

@first<A, B> (p: Pair<A, B>) -> A = p.first;

@main () -> int = {
    let a = identity(x: 42);
    let b = identity(x: "hello");
    let p = make_pair(a: a, b: 10);
    let f = first(p: p);
    f + 1
}
```
**Source**: 353 bytes, **Expected Result**: 43 (= 42 + 1)
**Actual**: Eval = 43 (correct), AOT = 1 (WRONG)

## Transformation Timeline

### Stage 1-2: Lexer
```
353 bytes → 163 tokens (0 errors)
```
Ratio: 2.2 bytes/token — consistent with design target. More tokens than Journey 2 (78) due to generic angle brackets and struct definition syntax.

### Stage 3: Parser
```
163 tokens → 4 functions, 1 type, 24 expressions (0 errors)
```

Parser output:
- `@identity<T>`: generic function with 1 type param, body = `Ident(x)` — 1 node
- `type Pair<A, B>`: struct type definition with 2 fields — parsed as type declaration, not a function
- `@make_pair<A, B>`: generic function with 2 type params, body = `Struct(Pair, {first: a, second: b})` — 5 nodes
- `@first<A, B>`: generic function with 2 type params, body = `Field(Ident(p), first)` — 2 nodes
- `@main`: non-generic, body = Block with 4 Let stmts + tail expr — 16 nodes

**Observation**: 4 "generic parameters" parse contexts entered (one per generic function/type). No issues here — parser handles generics cleanly.

### Stage 4: Type Checker
```
registration: 4 functions, 0 tests, 0 impls
signatures: 4 functions
body checking: 4 functions
```

**Monomorphization instances recorded:**
```
identity<int>     — args=[Type(Idx::INT)]       ✓ recorded
identity<str>     — args=[Type(Idx::STR)]        ✓ recorded
make_pair<int,int> — args=[Type(Idx::INT), Type(Idx::INT)]  ✓ recorded
first<int,int>    — FAILED TO RECORD             ✗
```

**FINDING (CRITICAL): `first<A, B>` mono instance fails to record.**

When `@main` calls `first(p: p)`:
1. The callee `first<A, B>` has 2 scheme vars (var_id 16 = A, var_id 17 = B)
2. After unification, A → int and B → int
3. But `concrete_idx` for both (Idx(214), Idx(215)) still has `Tag::Var` in the pool
4. Since `@main` is non-generic, `caller_roots` is empty (no scheme vars)
5. The mapping code in `record_deferred_mono_calls` can't find these vars in `caller_roots`
6. Result: `all_mapped = false`, no mono instance recorded

The root cause: When type variables resolve to concrete types (int) through struct fields (`Pair<A, B>`), the resolution chain goes:
- callee scheme var → instantiation var → `Pair<A=int, B=int>` field extraction → int

But the code at `calls.rs:373` checks if the `concrete_idx` has `Tag::Var`. If it does (which it shouldn't at this point — it should have been fully resolved), it tries to find it in caller scheme vars. Since @main has none, it fails.

**FINDING (MEDIUM): `record_deferred_mono_calls` runs for non-generic callers.**
The function is designed for "generic calling generic" scenarios, but it also runs when `@main` (non-generic) calls `first`. It should either: (a) not be called for non-generic callers, or (b) handle the case where caller has no scheme vars.

### Stage 5: Canonicalizer
```
canon lower_module started (functions=4, source_exprs=24)
canon lower_module complete (canon_nodes=27, roots=4, constants=6, decision_trees=0)
```
24 source exprs → 27 canon nodes. The +3 comes from desugaring (function call refs separated from call nodes, struct construction separated from literal).

### Stage 6a: Eval Path
```
@main body:
  eval Block(stmts=[4 lets], tail=Binary)
    eval Let(a, Call(identity, [42]))
      eval Call:
        eval Ident(identity) → FunctionValue
        eval Int(42) → Value::Int(42)
        → eval_call_value(identity, [42])
          eval Ident(x) → Value::Int(42)  // generic x:T, T=int
        → Value::Int(42)
      bind a = Value::Int(42)

    eval Let(b, Call(identity, ["hello"]))
      eval Call:
        eval Ident(identity) → FunctionValue
        eval Str("hello") → Value::Str("hello")
        → eval_call_value(identity, ["hello"])
          eval Ident(x) → Value::Str("hello")  // generic x:T, T=str
        → Value::Str("hello")
      bind b = Value::Str("hello")

    eval Let(p, Call(make_pair, [a, 10]))
      eval Call:
        eval Ident(make_pair) → FunctionValue
        eval Ident(a) → Value::Int(42)
        eval Int(10) → Value::Int(10)
        → eval_call_value(make_pair, [42, 10])
          eval Struct(Pair, {first: a, second: b})
            eval Ident(a) → Value::Int(42)   // param a, not local a
            eval Ident(b) → Value::Int(10)   // param b, not local b
          → Value::Struct(Pair, {first: 42, second: 10})
      bind p = Value::Struct(Pair, {first: 42, second: 10})

    eval Let(f, Call(first, [p]))
      eval Call:
        eval Ident(first) → FunctionValue
        eval Ident(p) → Value::Struct(Pair, ...)
        → eval_call_value(first, [Pair{42, 10}])
          eval Field(Ident(p), "first")
            eval Ident(p) → Value::Struct(Pair, ...)
            → field access: .first → Value::Int(42)
      bind f = Value::Int(42)

    eval Binary(Add, Ident(f), Int(1))
      eval Ident(f) → Value::Int(42)
      eval Int(1) → Value::Int(1)
      evaluate_binary(op=Add, left_type="int", right_type="int")
      → Value::Int(43) ✓
```

**Total eval_can calls**: ~27 (each canon node visited once)
**Total eval_call_value calls**: 4 (identity×2, make_pair, first)
**Total evaluate_binary calls**: 1 (main's `+`)

**Observation**: The interpreter handles generics by simple dynamic dispatch — no monomorphization needed. Generic functions work on `Value` directly, and the type parameter T is invisible at runtime. This is correct and clean for a tree-walking interpreter.

### Stage 6b: LLVM Path

#### ARC/Borrow Analysis
```
(Not instrumented — function_compiler has zero tracing despite importing tracing::{debug, trace, warn})
```

**FINDING (MEDIUM): LLVM function_compiler has dead tracing imports.** `compiler/ori_llvm/src/codegen/function_compiler/mod.rs` line 23 imports `use tracing::{debug, trace, warn}` but none are ever used. The entire two-pass declare/define pipeline runs dark.

#### Monomorphization
```
3 mono instances from type checker:
  identity<int>     → _ori_identity$24m$24int     (1 type arg)
  identity<str>     → _ori_identity$24m$24str     (1 type arg)
  make_pair<int,int> → _ori_make_pair$24m$24int_int (2 type args)
  first<int,int>    → NOT GENERATED               (missing mono instance)
```

#### Generated LLVM IR (AOT)

```llvm
define i64 @_ori_main() personality ptr @rust_eh_personality {
bb0:
  ; identity<int>(42) — correct
  %invoke = invoke fastcc i64 @"_ori_identity$24m$24int"(i64 42)
          to label %bb1 unwind label %bb2

bb1:
  ; identity<str>("hello") — correct, result is ARC-decremented
  %str.val = call { i64, ptr } @ori_str_from_raw(ptr @str, i64 5)
  %invoke1 = invoke fastcc { i64, ptr } @"_ori_identity$24m$24str"({ i64, ptr } %str.val)
          to label %bb3 unwind label %bb4

bb3:
  ; Drop the str result (b is unused, dead code eliminated)
  %rc_dec.fat_data = extractvalue { i64, ptr } %invoke1, 1
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$226")

  ; make_pair<int, int>(42, 10) — WRONG return type: i64 instead of { i64, i64 }
  %invoke2 = invoke fastcc i64 @"_ori_make_pair$24m$24int_int"(i64 %invoke, i64 10)
          to label %bb5 unwind label %bb6

bb5:
  br label %bb7  ; first() call is MISSING

bb7:
  ret i64 1      ; WRONG: returns literal 1 instead of f + 1 = 43

bb8:             ; No predecessors! Dead code.
  ...
}

; make_pair returns ONLY the first field, ignoring second
define fastcc i64 @"_ori_make_pair$24m$24int_int"(i64 %0, i64 %1) {
bb0:
  ret i64 %0     ; WRONG: should return { i64, i64 } struct
}

; identity<int> — correct (trivial)
define fastcc i64 @"_ori_identity$24m$24int"(i64 %0) {
bb0:
  ret i64 %0
}

; identity<str> — correct (passes through fat pointer)
define fastcc { i64, ptr } @"_ori_identity$24m$24str"({ i64, ptr } %0) {
bb0:
  ret { i64, ptr } %0
}
```

**FINDING (CRITICAL): Generic struct return type collapses to `i64`.**
`Pair<int, int>` should be LLVM type `{ i64, i64 }` (16 bytes). Instead, the monomorphizer produces `i64`. This cascade:
1. `make_pair` returns `i64` instead of `{ i64, i64 }` → only first field survives
2. Extract-value on the result fails (it's `i64`, not a struct): `ERROR extract_value on non-struct value`
3. Build-struct fallback produces wrong IR
4. `first()` call cannot be generated → unresolved function
5. Codegen falls through to `ret i64 1` (the literal `1` from `f + 1`)

Root cause chain:
```
type_info WARN: "Named/Applied/Alias type has no Pool resolution" (tag=Tag::applied)
  → Pair<int, int> is an Applied type in the pool
  → type_info can't resolve it to a concrete struct layout
  → falls back to i64 (the default/error type)
  → all downstream codegen is based on wrong type
```

**FINDING (CRITICAL): `first<A, B>` function is never generated.**
Warning: `ArcIrEmitter: unresolved function in invoke name="first"`. Because the mono instance was never recorded (type checker bug), the LLVM backend doesn't know to generate `_ori_first$24m$24int_int`. The call is silently dropped.

**FINDING (HIGH): 98 unconditional runtime declarations.**
The IR starts with 98 `declare` statements for runtime functions, same as Journey 2. None of `ori_list_*`, `ori_map_*`, `ori_set_*`, `ori_iter_*` are used.

**FINDING (MEDIUM): Silent error recovery produces wrong code.**
Instead of failing with a compile error when `first` is unresolved or when type resolution fails, the codegen silently falls back:
- `type_info` returns `i64` for unknown types (should error)
- `ArcIrEmitter` skips unresolved function calls (should error)
- `ir_builder` falls back when `extract_value` gets a non-struct (should error)

The result is a binary that runs but produces wrong output. **A compile error is always better than silent wrong code.**

---

## Issues Found

### CRITICAL
1. **Generic struct type resolution fails in LLVM** — `Pair<int, int>` (Tag::applied) has "no Pool resolution" in `type_info`. The Applied type → concrete struct layout mapping is broken. This causes `make_pair` to return `i64` instead of `{ i64, i64 }`, producing wrong code. **AOT exit code = 1, expected = 43.**

2. **`first<A, B>` mono instance never recorded** — Type checker's `record_deferred_mono_calls` fails when callee type vars are nested in a struct type parameter (`Pair<A, B>`). Callee var ids 16, 17 can't be mapped to caller scheme vars (caller is non-generic). Result: LLVM never generates `_ori_first`.

3. **Silent wrong code generation** — When type resolution or function resolution fails, the codegen silently produces wrong output instead of reporting a compilation error. Multiple fallback paths mask the error: `type_info → i64`, `arc_emitter → skip call`, `ir_builder → build_struct fallback`.

### HIGH
4. **98 unconditional runtime declarations** — Same as Journey 2. All runtime functions declared even when unused.

5. **Landing pads for non-panicking functions** — Same as Journey 2. `invoke` + landing pads for trivial identity/make_pair functions.

6. **Dead LLVM IR blocks** — `bb8` has "No predecessors!" — unreachable code from the failed `first()` call codegen.

### MEDIUM
7. **Function compiler has zero tracing** — `codegen/function_compiler/mod.rs` imports `use tracing::{debug, trace, warn}` but never uses them. The two-pass declare/define pipeline, ABI computation, and monomorphization scheduling are completely invisible.

8. **`record_deferred_mono_calls` called for non-generic callers** — The function is designed for generic→generic call chains but runs even when @main (non-generic) calls generic functions. The regular `record_mono_instance` path handles the direct cases, but the deferred path runs and fails with warnings.

9. **Prelude double processing** — Same as Journey 1 and 2. Still present.

### LOW
10. **`identity<str>` result correctly ARC-decremented** — The `b` variable is unused, so the LLVM optimizer correctly RC-decrements the string result immediately. This confirms ARC lifecycle management works for simple cases, even though struct types fail.

### CONFIRMED FROM PREVIOUS JOURNEYS
11. **Double prelude processing** — Present (Journey 1, 2, 3)
12. **6 prelude types registered in LLVM** — Even though unused
13. **90+ runtime declarations** — Now confirmed at 98

---

## Eval vs LLVM Behavioral Mismatch

| Aspect | Eval | LLVM (AOT) |
|--------|------|------------|
| Result | 43 (correct) | 1 (WRONG) |
| `identity<int>(42)` | 42 ✓ | 42 ✓ |
| `identity<str>("hello")` | "hello" ✓ | "hello" ✓ |
| `make_pair(42, 10)` | Pair{42, 10} ✓ | i64(42) ✗ |
| `first(p)` | 42 ✓ | NOT CALLED ✗ |
| `f + 1` | 43 ✓ | 1 ✗ |

The eval path handles generics dynamically (no monomorphization needed). The LLVM path requires monomorphization and fails at two points:
1. Type resolution: `Pair<int, int>` (Applied type) → unknown
2. Mono instance: `first<int, int>` never recorded

This is a **Phase Gap**: the type checker records mono instances for `identity` and `make_pair` (where type vars appear directly as params) but not for `first` (where type vars appear inside a struct type). The LLVM backend compounds this by silently falling back instead of erroring.
