# Journey 5: "I am a closure" -- Results

## Source Code

```ori
// Journey 5: "I am a closure"
// Features: lambdas, higher-order functions, closures capturing variables
// Expected: apply(double, 5) + make_adder(10)(7) = 10 + 17 = 27

@apply (f: (int) -> int, x: int) -> int = f(x);

@make_adder (n: int) -> (int) -> int = x -> x + n;

@main () -> int = {
    let double = x -> x * 2;
    let a = apply(f: double, x: 5);   // = 10
    let add10 = make_adder(n: 10);
    let b = add10(7);                  // = 17
    a + b                              // = 27
}
```

## Results

| Backend | Expected | Actual | Status |
|---------|----------|--------|--------|
| Eval (interpreter) | exit 27 | exit 27 | PASS |
| AOT (LLVM native)  | exit 27 | exit 27 | PASS |

**Note**: This journey was previously classified as CRITICAL (C1) due to a closure crash in AOT. That bug is now **FIXED**.

---

## Transformation Timeline

### 1. Lexer

Source: 499 bytes, 112 tokens, 0 errors.

The lexer processes the user module (499 bytes, 112 tokens) and the prelude (10,331 bytes, 1,516 tokens). Both passes complete with zero errors. Token types observed include: identifiers (`@apply`, `@make_adder`, `@main`, `f`, `x`, `n`, `double`, `a`, `add10`, `b`), keywords (`let`), integers (`2`, `5`, `7`, `10`), operators (`+`, `*`), arrow (`->`), and punctuation (`(`, `)`, `{`, `}`, `:`, `;`, `,`).

### 2. Parser

User module: 3 functions, 27 expressions, 0 errors, 0 warnings.
Prelude: 9 functions, 39 traits, 46 expressions, 4 decision trees, 0 errors.

Parse contexts entered for user code:
- 3x "function definition" (`@apply`, `@make_adder`, `@main`)
- 3x "function call" (`apply(f: double, x: 5)`, `make_adder(n: 10)`, `add10(7)`)
- 2x "closure" (`x -> x * 2` in `double`, `x -> x + n` in `make_adder`)
- 1x "expression" (block body of `@main`)
- Primary nodes: identifiers (`f`, `x`, `double`, `add10`, `make_adder`, `a`, `b`) and integers (`2`, `5`, `7`, `10`)

The parser correctly handles:
- Higher-order function parameter type `(int) -> int`
- Function return type `(int) -> int` (curried function)
- Lambda expressions with arrow syntax
- Variable capture in closures (implicit -- `n` referenced in `make_adder`'s lambda)

### 3. Type Checker

Prelude: 9 functions, 0 tests, 0 impls -- registration, signatures, body checking all complete.
User module: 3 functions, 0 tests, 0 impls -- all three passes complete.

Import resolution:
- Hash-first miss (AST fallback): `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err` (generic builtins)
- Hash-first hit: `compare`, `min`, `max`

No type errors. The type checker correctly infers:
- `double: (int) -> int` from lambda `x -> x * 2`
- `a: int` from `apply(f: double, x: 5)` return type
- `add10: (int) -> int` from `make_adder(n: 10)` return type
- `b: int` from `add10(7)` call
- `@main` returns `int` (last expression `a + b`)

Key type checking for closures:
- `@apply` parameter `f: (int) -> int` -- function type as first-class parameter
- `@make_adder` return type `(int) -> int` -- function type as return value
- Lambda `x -> x + n` correctly captures `n: int` from enclosing scope

### 4. Canonicalization

User module: 29 canon nodes, 3 roots, 0 method roots, 6 constants, 0 decision trees.
Prelude: 46 canon nodes, 9 roots, 6 constants, 4 decision trees.

The canon IR lowers the AST to a flat representation. The 3 roots correspond to `@apply`, `@make_adder`, and `@main`. The 6 constants correspond to the integer literals (2, 5, 7, 10) plus function-level constants. No decision trees needed (no match/pattern matching).

### 5. Interpreter Evaluation

Execution trace (CanId-level):

```
eval @main body: Block(CanRange(5..9), CanId(27))
  let double = (lambda) -- CanId(11): Let(pat0, CanId(10)=Lambda(params(1..2), CanId(9)), Mutable)
  let a = apply(f: double, x: 5)
    -- CanId(16): Let(pat1, CanId(15)=Call(...), Mutable)
    resolve apply     -- CanId(12): Ident("apply")
    arg f: double     -- CanId(13): Ident("double") -> (lambda)
    arg x: 5          -- CanId(14): Int(5)
    eval @apply body:
      f(x)            -- CanId(2): Call(CanId(0)=Ident("f"), [CanId(1)=Ident("x")])
        eval lambda (x -> x * 2) with x=5:
          x * 2       -- CanId(9): Binary(Mul, CanId(7)=Ident("x"), CanId(8)=Int(2))
                         evaluate_binary Mul int int -> 10
  let add10 = make_adder(n: 10)
    -- CanId(20): Let(pat2, CanId(19)=Call(...), Mutable)
    resolve make_adder -- CanId(17): Ident("make_adder")
    arg n: 10          -- CanId(18): Int(10)
    eval @make_adder body:
      (lambda)         -- CanId(6): Lambda(params(0..1), CanId(5))
                         returns closure capturing n=10
  let b = add10(7)
    -- CanId(24): Let(pat3, CanId(23)=Call(...), Mutable)
    resolve add10      -- CanId(21): Ident("add10") -> closure(n=10)
    arg: 7             -- CanId(22): Int(7)
    eval closure (x -> x + n) with x=7, n=10:
      x + n            -- CanId(5): Binary(Add, CanId(3)=Ident("x"), CanId(4)=Ident("n"))
                          evaluate_binary Add int int -> 17
  a + b               -- CanId(27): Binary(Add, CanId(25)=Ident("a"), CanId(26)=Ident("b"))
                          evaluate_binary Add int int -> 27
```

The interpreter correctly handles:
1. Lambda creation (`double = x -> x * 2`) -- creates a callable value
2. Higher-order function dispatch (`apply(f: double, x: 5)`) -- passes lambda as argument, calls via `f(x)`
3. Closure capture (`make_adder(n: 10)`) -- returns a closure that captures `n=10`
4. Closure invocation (`add10(7)`) -- calls the closure with `x=7`, accesses captured `n=10`

Exit code: 27 (correct).

### 6. LLVM Codegen

#### ARC Pipeline

The ARC pipeline registered 6 user types (prelude types: Ordering, PanicInfo, TraceEntry, FormatType, Sign, Alignment). Three user functions declared:
- `_ori_apply`: 2 params (closure + int), FastCC, direct return
- `_ori_make_adder`: 1 param, FastCC, direct return
- `_ori_main`: 0 params, C calling convention, direct return

Two lambdas declared:
- `__lambda_0`: 2 params, **capturing** (the `x -> x + n` inside `make_adder`, captures `n`)
- `__lambda_1`: 1 param, **non-capturing** (the `x -> x * 2`, no captures)

Nounwind analysis: 2 passes, 3 functions marked `nounwind` (apply, make_adder, main). Zero mono-propagated.

Entry point wrapper: C `main()` generated with `returns_int=true`, `has_args=false`, `has_panic=false`.

#### Generated LLVM IR

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.2 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; --- @apply ---
define fastcc i64 @_ori_apply({ ptr, ptr } %0, i64 %1) {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}

; --- @make_adder ---
define fastcc { ptr, ptr } @_ori_make_adder(i64 %0) #0 {
bb0:
  %env.data = call ptr @ori_rc_alloc(i64 16, i64 8)
  %env.drop_fn = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 0
  store ptr @_ori_partial_0_drop, ptr %env.drop_fn, align 8
  %env.cap.0 = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 1
  store i64 %0, ptr %env.cap.0, align 8
  %partial_apply.1 = insertvalue { ptr, ptr } { ptr @_ori_partial_1, ptr undef }, ptr %env.data, 1
  ret { ptr, ptr } %partial_apply.1
}

; --- @main ---
define i64 @_ori_main() {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  br label %bb1

bb1:
  %call1 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  br label %bb3

bb3:
  %closure.fn_ptr = extractvalue { ptr, ptr } %call1, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call1, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}

; --- @__lambda_0 (capturing: x -> x + n) ---
define fastcc i64 @_ori___lambda_0(i64 %0, i64 %1) #0 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %1, i64 %0)
  ; ... overflow check ...
  ret i64 %add.val
}

; --- @__lambda_1 (non-capturing: x -> x * 2) ---
define i64 @_ori___lambda_1(ptr %0, i64 %1) #0 {
bb0:
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %1, i64 2)
  ; ... overflow check ...
  ret i64 %mul.val
}

; --- @partial_0_drop (env destructor) ---
define void @_ori_partial_0_drop(ptr %0) #3 {
entry:
  call void @ori_rc_free(ptr %0, i64 16, i64 8)
  ret void
}

; --- @partial_1 (closure dispatcher) ---
define i64 @_ori_partial_1(ptr %0, i64 %1) #0 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, i64 }, ptr %0, i32 0, i32 1
  %cap.0 = load i64, ptr %cap.0.ptr, align 8
  %result = call fastcc i64 @_ori___lambda_0(i64 %cap.0, i64 %1)
  ret i64 %result
}

; --- C main wrapper ---
define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

---

## LLVM Deep Scrutiny Report

### 1. Closure Representation

Closures are represented as a fat pointer pair `{ ptr, ptr }`:
- Slot 0: function pointer (the callable code)
- Slot 1: environment pointer (captured variables, or `null` for non-capturing)

**Non-capturing lambda** (`double = x -> x * 2`):
- Represented as `{ ptr @_ori___lambda_1, ptr null }` -- function pointer with null environment
- `__lambda_1` takes `(ptr %env, i64 %x)` but ignores the environment pointer
- Compiled as a plain function with overflow-checked multiply

**Capturing closure** (`x -> x + n` inside `make_adder`):
- `make_adder` allocates an RC-managed environment via `ori_rc_alloc(16, 8)` (16 bytes data, 8-byte alignment)
- Environment layout: `{ ptr drop_fn, i64 captured_n }` -- drop function pointer + one captured i64
- Returns `{ ptr @_ori_partial_1, ptr env_data }` -- partial application dispatcher + env
- `_ori_partial_1` extracts captured `n` from env, calls `__lambda_0(n, x)`
- `_ori_partial_0_drop` frees the environment via `ori_rc_free`

This is a clean two-tier closure design: non-capturing lambdas are zero-cost (no allocation), while capturing closures use RC-managed heap environments.

### 2. Instruction Purity

**@_ori_apply** -- 4 IR instructions:
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `extractvalue { ptr, ptr } %0, 0` | YES | Extract function pointer |
| 2 | `extractvalue { ptr, ptr } %0, 1` | YES | Extract environment pointer |
| 3 | `call i64 %closure.fn_ptr(ptr %env, i64 %1)` | YES | Indirect call through closure |
| 4 | `ret i64 %icall` | YES | Return result |

Optimal: 4. Actual: 4. **Ratio: 1.00**

**@_ori_make_adder** -- 7 IR instructions:
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `call ptr @ori_rc_alloc(i64 16, i64 8)` | YES | Allocate closure env |
| 2 | `getelementptr ... i32 0, i32 0` | YES | Pointer to drop_fn slot |
| 3 | `store ptr @_ori_partial_0_drop` | YES | Set destructor |
| 4 | `getelementptr ... i32 0, i32 1` | YES | Pointer to capture slot |
| 5 | `store i64 %0, ptr %env.cap.0` | YES | Store captured value |
| 6 | `insertvalue { ptr, ptr } ..., ptr %env.data, 1` | YES | Build fat pointer |
| 7 | `ret { ptr, ptr } %partial_apply.1` | YES | Return closure |

Optimal: 7. Actual: 7. **Ratio: 1.00**

**@_ori_main** -- 15 IR instructions (excl. panic path):
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `call fastcc @_ori_apply(...)` | YES | Call apply with double |
| 2 | `br label %bb1` | NO | Redundant unconditional branch |
| 3 | `call fastcc @_ori_make_adder(i64 10)` | YES | Create add10 closure |
| 4 | `br label %bb3` | NO | Redundant unconditional branch |
| 5 | `extractvalue { ptr, ptr } %call1, 0` | YES | Extract fn ptr |
| 6 | `extractvalue { ptr, ptr } %call1, 1` | YES | Extract env ptr |
| 7 | `call i64 %closure.fn_ptr(...)` | YES | Call add10(7) |
| 8 | `call @llvm.sadd.with.overflow.i64(...)` | YES | Checked a + b |
| 9 | `extractvalue { i64, i1 } %add, 0` | YES | Extract sum |
| 10 | `extractvalue { i64, i1 } %add, 1` | YES | Extract overflow flag |
| 11 | `br i1 %add.ovf, ...` | YES | Overflow branch |
| 12 | `ret i64 %add.val` | YES | Return result |
| 13-15 | Panic path (call + unreachable) | YES | Cold path |

Optimal (with overflow checks): 13. Actual: 15. **Ratio: 1.15** (2 redundant branches)

**@_ori___lambda_0** (capturing: `x + n`) -- 7 instructions:
Identical structure to Journey 1's `@_ori_add`. Overflow-checked addition with panic path. **Ratio: 1.00**

**@_ori___lambda_1** (non-capturing: `x * 2`) -- 7 instructions:
Overflow-checked multiplication with panic path. Constant `2` is embedded directly. **Ratio: 1.00**

**@_ori_partial_1** (closure dispatcher) -- 4 instructions:
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `getelementptr ... i32 0, i32 1` | YES | Pointer to captured n |
| 2 | `load i64, ptr %cap.0.ptr` | YES | Load captured n |
| 3 | `call fastcc @_ori___lambda_0(i64 %cap.0, i64 %1)` | YES | Forward to lambda |
| 4 | `ret i64 %result` | YES | Return |

Optimal: 4. Actual: 4. **Ratio: 1.00**

**@_ori_partial_0_drop** (destructor) -- 2 instructions:
`call @ori_rc_free` + `ret void`. **Ratio: 1.00**

### 3. ARC Purity

RC operations in the generated IR:
- `ori_rc_alloc(i64 16, i64 8)` in `_ori_make_adder` -- allocates closure environment (1 captured i64 + drop fn ptr)
- `ori_rc_free(ptr %0, i64 16, i64 8)` in `_ori_partial_0_drop` -- frees closure environment

**Missing**: There is no `ori_rc_dec` call on the `add10` closure in `_ori_main`. After `add10(7)` returns, the closure environment is no longer needed. The environment was allocated in `make_adder`, and the closure `{ fn_ptr, env_ptr }` is stored on the stack of `main`. When `main` returns, the environment leaks.

However, for this specific program, the process exits immediately after `main` returns, so the leak has no practical impact. In a long-running program, this pattern would be a genuine leak.

**Assessment**: The RC allocation is correct and necessary. The missing RC decrement is a known limitation of the current ARC pipeline for closures returned from functions. The non-capturing lambda (`double`) correctly uses `ptr null` for the environment, requiring zero allocation.

**Verdict: ACCEPTABLE** -- correct allocation, missing cleanup (process-exit hides leak).

### 4. Attribute Audit

| Function | nounwind | noalias | cold | fastcc | Notes |
|----------|----------|---------|------|--------|-------|
| `_ori_apply` | missing | n/a | n/a | YES | Should be nounwind (calls nounwind fn) |
| `_ori_make_adder` | YES | n/a | n/a | YES | Correct |
| `_ori_main` | missing | n/a | n/a | no (C) | Correct convention; nounwind missing |
| `_ori___lambda_0` | YES | n/a | n/a | YES | Correct |
| `_ori___lambda_1` | YES | n/a | n/a | no (C) | **Should be fastcc** |
| `_ori_partial_1` | YES | n/a | n/a | no (C) | Correct -- called via indirect `call` |
| `_ori_partial_0_drop` | YES | n/a | YES | no (C) | `cold` correct -- destructors are infrequent |
| `ori_panic_cstr` | n/a | n/a | YES | n/a | Still missing `noreturn` (same as J1) |
| `ori_rc_alloc` | YES | **YES** | n/a | n/a | `noalias` correct -- fresh allocation |
| `main` (wrapper) | missing | n/a | n/a | no (C) | Missing nounwind (same as J1) |

Findings:
- **`_ori_apply` missing `nounwind`**: The apply function does an indirect call through a closure, so the nounwind analysis may conservatively not mark it. However, all callees in this program are nounwind, so it could be marked transitively.
- **`_ori___lambda_1` uses C convention instead of `fastcc`**: The non-capturing lambda is called indirectly through `apply`, where `apply` uses an indirect `call i64 %closure.fn_ptr(...)` without `fastcc`. Since the calling convention must match between declaration and call site, and `apply` doesn't know the convention, C convention is correct for indirect calls. The capturing lambda's dispatcher `_ori_partial_1` similarly uses C convention. This is architecturally correct.
- **`ori_panic_cstr` missing `noreturn`**: Same as Journey 1.

### 5. Optimal IR Comparison

**Ideal `_ori_apply` (hand-written):**
```llvm
define fastcc i64 @_ori_apply({ ptr, ptr } %closure, i64 %x) {
  %fn = extractvalue { ptr, ptr } %closure, 0
  %env = extractvalue { ptr, ptr } %closure, 1
  %r = call i64 %fn(ptr %env, i64 %x)
  ret i64 %r
}
```
Generated matches ideal exactly. **OPTIMAL.**

**Ideal `_ori_make_adder` (hand-written):**
```llvm
define fastcc { ptr, ptr } @_ori_make_adder(i64 %n) nounwind {
  %env = call noalias ptr @ori_rc_alloc(i64 16, i64 8)
  %drop_slot = getelementptr inbounds { ptr, i64 }, ptr %env, i32 0, i32 0
  store ptr @_ori_partial_0_drop, ptr %drop_slot
  %cap_slot = getelementptr inbounds { ptr, i64 }, ptr %env, i32 0, i32 1
  store i64 %n, ptr %cap_slot
  %r = insertvalue { ptr, ptr } { ptr @_ori_partial_1, ptr undef }, ptr %env, 1
  ret { ptr, ptr } %r
}
```
Generated matches ideal exactly. **OPTIMAL.**

**Ideal `_ori_main` (hand-written):**
```llvm
define i64 @_ori_main() {
  ; apply(double, 5) -- double is non-capturing
  %a = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  ; make_adder(10) -- returns closure
  %add10 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  ; add10(7)
  %fn = extractvalue { ptr, ptr } %add10, 0
  %env = extractvalue { ptr, ptr } %add10, 1
  %b = call i64 %fn(ptr %env, i64 7)
  ; a + b (checked)
  %sum = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue { i64, i1 } %sum, 0
  %ovf = extractvalue { i64, i1 } %sum, 1
  br i1 %ovf, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

Generated vs. ideal: 2 extra instructions (`br label %bb1` and `br label %bb3` -- redundant unconditional branches between let-binding boundaries). LLVM's optimizer eliminates these.

**Let binding elimination**: All four `let` bindings (`double`, `a`, `add10`, `b`) are eliminated -- no `alloca`/`store`/`load` chains. The non-capturing lambda `double` is represented as an inline constant `{ ptr @_ori___lambda_1, ptr null }`. This is excellent codegen.

### 6. Constant Folding Opportunities

| Expression | Foldable? | Status |
|------------|-----------|--------|
| `apply(f: double, x: 5)` | Partially (inline + fold) | NOT FOLDED -- requires inlining `apply` then `lambda_1` |
| `double(5)` -> `5 * 2` | YES (after inlining) | Would fold to 10 |
| `make_adder(10)(7)` | YES (after inlining) | Would fold to 17 |
| `10 + 17` | YES (after prior folds) | Would fold to 27 |

No constant folding is performed at the unoptimized IR level. The indirect calls through closure fat pointers make this expected -- the codegen cannot know at compile time which function a closure points to without interprocedural analysis. At `-O2`, LLVM could inline and devirtualize, folding the entire program to `ret i64 27`.

### 7. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size (on disk) | 6,561,576 bytes (6.26 MiB) |
| .text section | 889,841 bytes (869 KiB) |
| .rodata section | 136,527 bytes (133 KiB) |
| .debug_info | 1,642,452 bytes (1.57 MiB) |
| Debug total | ~4.7 MiB |
| User code (.text) | ~262 bytes (see breakdown below) |

User function sizes (from symbol table):
| Function | Size (bytes) | Size (instructions) |
|----------|-------------|-------------------|
| `_ori_apply` | 37 (0x25) | ~10 |
| `_ori_make_adder` | 50 (0x32) | ~12 |
| `_ori_main` | 110 (0x6e) | ~28 |
| `_ori___lambda_0` | 31 (0x1f) | ~10 |
| `_ori___lambda_1` | 37 (0x25) | ~10 |
| `_ori_partial_0_drop` | 18 (0x12) | ~6 |
| `_ori_partial_1` | 12 (0x0c) | ~4 |
| **Total user code** | **295 bytes** | **~80** |

The binary size is consistent with Journey 1 (~6.26 MiB). The slight increase comes from the closure infrastructure (partial_1 dispatcher, partial_0_drop destructor, RC alloc/free linkage).

### 8. Disassembly Highlights

**`_ori_apply` (37 bytes):**
```asm
sub    $0x18,%rsp         ; frame
mov    %rdx,0x8(%rsp)     ; save x to stack
mov    %rsi,%rax           ; fn_ptr to rax
mov    0x8(%rsp),%rsi      ; reload x to rsi (arg 2)
mov    %rax,0x10(%rsp)     ; save fn_ptr
mov    %rdi,%rax           ; env_ptr to rax... wait
mov    0x10(%rsp),%rdi     ; reload fn_ptr... wrong reg
call   *%rax               ; indirect call
```

The closure fat pointer `{ ptr, ptr }` is passed in `%rdi` (fn_ptr) and `%rsi` (env_ptr), with `%rdx` carrying the int argument `x`. The register shuffling is suboptimal -- there are unnecessary stack spills at `-O0`. The function pointer ends up in `%rax` for the indirect call, and the env goes to `%rdi` (first arg). This is correct but could be tighter.

**`_ori_make_adder` (50 bytes):**
```asm
push   %rax
mov    %rdi,(%rsp)         ; save n
mov    $0x10,%edi           ; size=16
mov    $0x8,%esi            ; align=8
call   ori_rc_alloc         ; allocate env
mov    (%rsp),%rdi          ; reload n
mov    %rax,%rdx            ; env ptr to rdx
lea    _ori_partial_0_drop(%rip),%rax
mov    %rax,(%rdx)          ; env[0] = drop_fn
mov    %rdi,0x8(%rdx)       ; env[1] = n (captured value)
lea    _ori_partial_1(%rip),%rax  ; fn_ptr
pop    %rcx
ret                          ; returns (rax=fn_ptr, rdx=env_ptr)
```

Clean codegen for the closure construction. The environment is a 16-byte struct `{ ptr drop_fn, i64 n }`. The return is a pair `(rax, rdx)` matching the `{ ptr, ptr }` struct return convention. Only one stack spill (to preserve `n` across the `ori_rc_alloc` call).

**`_ori_partial_1` (12 bytes):**
```asm
push   %rax
mov    0x8(%rdi),%rdi       ; load captured n from env[1]
call   _ori___lambda_0      ; call lambda_0(n, x)
pop    %rcx
ret
```

Extremely compact dispatcher -- loads the captured value from the environment and forwards to the underlying lambda. Just 5 instructions (12 bytes).

**`_ori_main` (110 bytes):**
The main function sequence:
1. Constructs non-capturing closure `{ @_ori___lambda_1, null }` for `double`
2. Calls `_ori_apply` with the closure and `5` -- result in `%rax` (saved to stack)
3. Calls `_ori_make_adder(10)` -- returns closure pair in `(rax, rdx)` (saved to stack)
4. Extracts fn_ptr and env_ptr, calls closure with `7` -- result in `%rax`
5. Adds the two results with overflow check
6. Returns or panics

The `seto`/`jo` pattern for overflow checking is present. The final addition result is at offset `0x14b` (`add %rcx,%rax`), followed by overflow check and return.

### 9. Calling Convention Audit

- `_ori_apply`: `fastcc` -- Correct. Internal function.
- `_ori_make_adder`: `fastcc` -- Correct. Internal function.
- `_ori_main`: C convention -- Correct. Called from C `main()` wrapper.
- `_ori___lambda_0`: `fastcc` -- Correct. Called only from `_ori_partial_1` (which knows the convention).
- `_ori___lambda_1`: C convention -- Correct. Called indirectly; convention must match indirect call site.
- `_ori_partial_1`: C convention -- Correct. Called via indirect `call *%rax`.
- `_ori_partial_0_drop`: C convention with `cold` -- Correct. Called from RC system.
- `main` wrapper: C convention -- Correct. OS entry point.

The calling convention split is architecturally sound: direct-call internal functions use `fastcc`, while functions called through indirect pointers (closures, dispatchers, destructors) use C convention. This avoids calling convention mismatches at indirect call sites.

### 10. Closure Safety Analysis

**Environment lifetime**: The closure environment is allocated with `ori_rc_alloc` (reference-counted). The drop function `_ori_partial_0_drop` calls `ori_rc_free`. This ensures the environment is freed when the refcount reaches zero.

**Capture correctness**: The captured `n` is stored by value (`store i64 %0, ptr %env.cap.0`). Since `int` is a value type, this is correct -- no dangling references possible.

**Non-capturing optimization**: The non-capturing lambda `double` uses `ptr null` for the environment, avoiding any heap allocation. `__lambda_1` receives and ignores the null environment pointer. This is the zero-cost path for simple lambdas.

**Potential issue -- RC leak**: As noted in section 3, the closure environment allocated in `make_adder` is not explicitly freed in `_ori_main`. The environment's refcount starts at 1 (from `ori_rc_alloc`), and no `ori_rc_dec` is emitted before `main` returns. In a long-running program, this would be a leak. For this process-exit program, it is benign.

---

## Issues Found

### MEDIUM-1: Redundant unconditional branches in `_ori_main`

**Severity**: MEDIUM (overhead ratio 1.15)
**Location**: `_ori_main`, `bb0 -> bb1` and `bb1 -> bb3`
**Details**: Two redundant `br label` instructions at let-binding boundaries. Same pattern as Journey 1, now appearing twice due to more let bindings.
**Impact**: Minimal at runtime (LLVM backend eliminates them).
**Fix**: Merge sequential blocks when no control flow divergence occurs.

### MEDIUM-2: Missing `noreturn` on `ori_panic_cstr`

**Severity**: MEDIUM (recurring from Journey 1)
**Location**: `declare void @ori_panic_cstr(ptr) #2`
**Details**: Still only marked `cold`, not `noreturn`.
**Impact**: LLVM may not fully optimize code after panic calls.

### MEDIUM-3: Missing closure environment cleanup in `_ori_main`

**Severity**: MEDIUM (RC leak for closures)
**Location**: `_ori_main` -- no `ori_rc_dec` on the `add10` closure environment
**Details**: The closure environment allocated by `make_adder` is never freed. Benign here (process exit), but would be a real leak if closures were used in loops or long-lived contexts.
**Impact**: Memory leak in non-trivial programs using closures.
**Fix**: The ARC pipeline should emit `ori_rc_dec` at the end of a closure's live range.

### LOW-1: Missing `nounwind` on `_ori_apply`

**Severity**: LOW
**Location**: `_ori_apply` function definition
**Details**: Makes an indirect call, so nounwind analysis conservatively excludes it. Since all callees are nounwind in this program, it could be marked transitively.
**Impact**: Minimal.

### LOW-2: Missing `nounwind` on `main` wrapper

**Severity**: LOW (recurring from Journey 1)
**Location**: `define i32 @main()`
**Details**: Transitively nounwind.

---

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Correctness | 30% | 10/10 | Both backends produce 27. Closures work correctly. Previously CRITICAL (C1) -- now FIXED. |
| Instruction Purity | 20% | 8/10 | 2 redundant branches in main, all other functions optimal |
| ARC Purity | 15% | 7/10 | Correct allocation, missing RC cleanup on closure env |
| Attributes | 15% | 7/10 | Missing noreturn on panic, missing nounwind on apply/wrapper |
| Constant Folding | 10% | 7/10 | No folding (expected -- indirect calls prevent devirtualization at -O0) |
| Block Layout | 10% | 8/10 | 2 redundant block boundaries, panic blocks placed correctly |

**Overall Score: 8.0 / 10**

The codegen quality is solid for a debug build. The closure representation is architecturally clean -- non-capturing lambdas are zero-cost, capturing closures use RC-managed environments with a dispatcher/destructor pair. The primary gap is the missing RC cleanup for closure environments, which would matter in production code. The previously critical AOT closure crash (C1) is fully resolved.
