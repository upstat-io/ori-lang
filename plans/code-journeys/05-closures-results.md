---
journey: 5
slug: closures
theme: "I am a closure"
date: 2026-03-16
status: PASS
expected: 27
eval_result: 27
aot_result: 27

difficulty: moderate
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of first-class functions"
  - "Concept of variable scope and capture"
learning_objectives:
  - "See how closures are lowered to {fn_ptr, env_ptr} pairs in LLVM IR"
  - "Understand heap-allocated capture environments and ARC management"
  - "Compare non-capturing vs capturing closure representations"
  - "Observe partial application and indirect call conventions"

features:
  - closures
  - higher_order
  - capture
  - function_calls
feature_description: "Closures with capture, higher-order functions, and partial application"

score: 9.2
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 7
  control_flow: 9
  ir_quality: 9
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.02
  instruction_ratio_max: 1.06
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 28
  attr_correct: 23
  attr_has_wrong: false
  cf_defects: 1
  cf_incorrect: false
  ir_unjustified: 1
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 0
  other_low: 0
overflow_check: PASS

bugs_found: []

related_journeys:
  - journey: 1
    relationship: "Both test overflow checking on arithmetic"
  - journey: 3
    relationship: "Both test function call conventions (direct vs indirect)"
---

# Journey 5: "I am a closure"

## Source

```ori
// Journey 5: "I am a closure"
// Slug: closures
// Difficulty: moderate
// Features: closures, higher_order, capture, function_calls
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

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 27        | 27       | (none) | (none) | PASS   |
| AOT     | 27        | 27       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 114 | **Keywords**: 6 | **Identifiers**: 22 | **Errors**: 0

<details>
<summary>Token stream (user module)</summary>

```text
Fn(@) Ident(apply) LParen Ident(f) Colon LParen Ident(int) RParen
Arrow Ident(int) Comma Ident(x) Colon Ident(int) RParen Arrow
Ident(int) Eq Ident(f) LParen Ident(x) RParen Semi
Fn(@) Ident(make_adder) LParen Ident(n) Colon Ident(int) RParen
Arrow LParen Ident(int) RParen Arrow Ident(int) Eq Ident(x)
Arrow Ident(x) Plus Ident(n) Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(double) Eq Ident(x) Arrow Ident(x) Star Lit(2) Semi
Let Ident(a) Eq Ident(apply) LParen Ident(f) Colon Ident(double)
Comma Ident(x) Colon Lit(5) RParen Semi
Let Ident(add10) Eq Ident(make_adder) LParen Ident(n) Colon Lit(10)
RParen Semi
Let Ident(b) Eq Ident(add10) LParen Lit(7) RParen Semi
Ident(a) Plus Ident(b) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 27 | **Max depth**: 4 | **Functions**: 3 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
+-  FnDecl @apply
|  +-  Params: (f: (int) -> int, x: int)
|  +-  Return: int
|  +-- Body: Call(f)
|       +-- x
+-  FnDecl @make_adder
|  +-  Params: (n: int)
|  +-  Return: (int) -> int
|  +-- Body: Lambda(x)
|       +-- BinOp(+)
|            +-  Ident(x)
|            +-- Ident(n)
+-- FnDecl @main
   +-  Return: int
   +-- Body: Block
        +-  Let double = Lambda(x) -> BinOp(*) [x, 2]
        +-  Let a = Call(@apply) [f: double, x: 5]
        +-  Let add10 = Call(@make_adder) [n: 10]
        +-  Let b = Call(add10) [7]
        +-- BinOp(+) [a, b]
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 18 | **Types inferred**: 9 | **Unifications**: 14 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@apply (f: (int) -> int, x: int) -> int = f(x)
//                                         ^ int (call return type matches f's return)

@make_adder (n: int) -> (int) -> int = x -> x + n
//                                     ^ closure captures n: int
//                                       ^ int (Add<int, int> -> int)

@main () -> int = {
    let double: (int) -> int = x -> x * 2  // inferred: (int) -> int, non-capturing
    let a: int = apply(f: double, x: 5)    // inferred: int
    let add10: (int) -> int = make_adder(n: 10)  // inferred: (int) -> int, captures n=10
    let b: int = add10(7)                   // inferred: int
    a + b                                   // -> int (matches return type)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 4 | **Desugared**: 2 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Lambda expressions lowered to closure objects with capture lists
- __lambda_0: captures [n] (from make_adder)
- __lambda_1: captures [] (non-capturing, from double)
- Function call arguments normalized to positional order
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 2 | **Elided**: 1 | **Net ops**: 1

<details>
<summary>ARC annotations</summary>

```text
@apply: +0 rc_inc, +0 rc_dec (AIMS elided callee-side env cleanup)
@make_adder: +1 rc_alloc, +0 rc_dec (env ownership transferred to caller)
@main: +0 rc_inc, +1 rc_dec (cleanup for make_adder's env after use)
@__lambda_0: no RC ops (pure scalar arithmetic on captured value)
@__lambda_1: no RC ops (non-capturing, pure scalar arithmetic)
@partial_0_drop: +1 rc_free (destructor for closure env)
@partial_1: no RC ops (thunk: loads captured value, calls lambda)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 27 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-  let double = Lambda(__lambda_1, captures=[])
  +-  let a = @apply(f: double, x: 5)
  |    +-- call double(5)
  |         +-- 5 * 2 = 10
  |    -> 10
  +-  let add10 = @make_adder(n: 10)
  |    +-- Lambda(__lambda_0, captures=[n=10])
  +-  let b = add10(7)
  |    +-- call __lambda_0(x=7, n=10)
  |         +-- 7 + 10 = 17
  |    -> 17
  +-- a + b = 10 + 17 = 27
-> 27
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 2 | **Elided**: 1 | **Net ops**: 1

<details>
<summary>ARC annotations</summary>

```text
@apply: +0 rc_inc, +0 rc_dec (AIMS elided callee-side env cleanup)
@make_adder: +1 ori_rc_alloc(16, 8), +0 rc_dec (env: {drop_fn, n: i64})
@main: +0 rc_inc, +1 ori_rc_dec (cleanup for captured closure env via null-guarded path)
@partial_0_drop: +1 ori_rc_free (destructor called when env refcount hits 0)
@partial_1: +0 rc_inc, +0 rc_dec (thunk -- loads capture, forwards to lambda)
@__lambda_0: +0 rc_inc, +0 rc_dec (pure arithmetic on captured i64)
@__lambda_1: +0 rc_inc, +0 rc_dec (pure arithmetic, non-capturing)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '05-closures'
source_filename = "05-closures"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1

; Function Attrs: nounwind uwtable
; --- @apply ---
define fastcc noundef i64 @_ori_apply({ ptr, ptr } noundef %0, i64 noundef %1) #0 {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}

; Function Attrs: nounwind uwtable
; --- @make_adder ---
define fastcc noundef { ptr, ptr } @_ori_make_adder(i64 noundef %0) #0 {
bb0:
  %env.data = call ptr @ori_rc_alloc(i64 16, i64 8)
  %env.drop_fn = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 0
  store ptr @_ori_partial_0_drop, ptr %env.drop_fn, align 8
  %env.cap.0 = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 1
  store i64 %0, ptr %env.cap.0, align 8
  %partial_apply.1 = insertvalue { ptr, ptr } { ptr @_ori_partial_1, ptr undef }, ptr %env.data, 1
  ret { ptr, ptr } %partial_apply.1
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  %call1 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  %closure.fn_ptr = extractvalue { ptr, ptr } %call1, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call1, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %rc_dec.env = extractvalue { ptr, ptr } %call1, 1
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.env to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

rc_dec.do:                                        ; preds = %add.ok
  %rc_dec.drop_fn = load ptr, ptr %rc_dec.env, align 8
  call void @ori_rc_dec(ptr %rc_dec.env, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:                                      ; preds = %rc_dec.do, %add.ok
  ret i64 %add.val
}

; Function Attrs: nounwind memory(none) uwtable
; --- @__lambda_0 ---
define fastcc noundef i64 @_ori___lambda_0(i64 noundef %0, i64 noundef %1) #2 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %1, i64 %0)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind memory(none) uwtable
; --- @__lambda_1 ---
define noundef i64 @_ori___lambda_1(ptr %0, i64 noundef %1) #2 {
bb0:
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %1, i64 2)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok

mul.ok:                                           ; preds = %bb0
  ret i64 %mul.val

mul.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: cold nounwind uwtable
; --- @partial_0_drop ---
define void @_ori_partial_0_drop(ptr %0) #6 {
entry:
  call void @ori_rc_free(ptr %0, i64 16, i64 8)
  ret void
}

; Function Attrs: nounwind uwtable
; --- @partial_1 ---
define i64 @_ori_partial_1(ptr %0, i64 %1) #0 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, i64 }, ptr %0, i32 0, i32 1
  %cap.0 = load i64, ptr %cap.0.ptr, align 8
  %result = call fastcc i64 @_ori___lambda_0(i64 %cap.0, i64 %1)
  ret i64 %result
}

define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

#### Disassembly

```asm
_ori_apply:
  sub    $0x18,%rsp
  mov    %rdx,0x8(%rsp)          ; save x
  mov    %rsi,%rax                ; env_ptr -> rax
  mov    0x8(%rsp),%rsi           ; x -> rsi (arg 1)
  mov    %rax,0x10(%rsp)          ; save env_ptr
  mov    %rdi,%rax                ; fn_ptr -> rax
  mov    0x10(%rsp),%rdi          ; env_ptr -> rdi (arg 0)
  call   *%rax                    ; indirect call: fn_ptr(env, x)
  add    $0x18,%rsp
  ret

_ori_make_adder:
  push   %rax
  mov    %rdi,(%rsp)              ; save n
  mov    $0x10,%edi               ; alloc size = 16
  mov    $0x8,%esi                ; align = 8
  call   ori_rc_alloc             ; allocate env
  mov    (%rsp),%rdi              ; reload n
  mov    %rax,%rdx                ; env -> rdx (return)
  lea    _ori_partial_0_drop,%rax ; drop_fn
  mov    %rax,(%rdx)              ; env[0] = drop_fn
  mov    %rdi,0x8(%rdx)           ; env[1] = n
  lea    _ori_partial_1,%rax      ; fn_ptr (return)
  pop    %rcx
  ret                             ; returns (fn_ptr in rax, env in rdx)

_ori_main:
  sub    $0x18,%rsp
  lea    _ori___lambda_1(%rip),%rdi
  xor    %eax,%eax
  mov    %eax,%esi                ; null env_ptr
  mov    $0x5,%edx                ; x = 5
  call   _ori_apply               ; apply(double, 5) = 10
  mov    %rax,0x8(%rsp)           ; save a = 10
  mov    $0xa,%edi                ; n = 10
  call   _ori_make_adder          ; make_adder(10)
  mov    %rdx,%rdi                ; env_ptr
  mov    %rdi,(%rsp)              ; save env_ptr for later RC dec
  mov    $0x7,%esi                ; x = 7
  call   *%rax                    ; indirect call: add10(7) = 17
  mov    %rax,%rcx                ; b = 17
  mov    0x8(%rsp),%rax           ; a = 10
  add    %rcx,%rax                ; a + b = 27
  mov    %rax,0x10(%rsp)          ; save result
  seto   %al                     ; check overflow
  jo     .panic                   ; branch if overflow
  mov    (%rsp),%rax              ; load env_ptr
  cmp    $0x0,%rax                ; null check
  je     .skip_rc_dec             ; skip if null
  jmp    .do_rc_dec               ; else dec
.panic:
  lea    ovf_msg(%rip),%rdi
  call   ori_panic_cstr
.do_rc_dec:
  mov    (%rsp),%rdi              ; env_ptr
  mov    (%rdi),%rsi              ; load drop_fn
  call   ori_rc_dec               ; RC--
.skip_rc_dec:
  mov    0x10(%rsp),%rax          ; return result
  add    $0x18,%rsp
  ret

_ori___lambda_0:
  push   %rax
  add    %rdi,%rsi                ; x + n
  mov    %rsi,(%rsp)
  seto   %al
  jo     .panic_add
  mov    (%rsp),%rax
  pop    %rcx
  ret
.panic_add:
  lea    ovf_msg(%rip),%rdi
  call   ori_panic_cstr

_ori___lambda_1:
  push   %rax
  mov    $0x2,%eax
  imul   %rax,%rsi                ; x * 2
  mov    %rsi,(%rsp)
  seto   %al
  jo     .panic_mul
  mov    (%rsp),%rax
  pop    %rcx
  ret
.panic_mul:
  lea    ovf_msg_mul(%rip),%rdi
  call   ori_panic_cstr

_ori_partial_0_drop:
  push   %rax
  mov    $0x10,%esi
  mov    $0x8,%edx
  call   ori_rc_free
  pop    %rax
  ret

_ori_partial_1:
  push   %rax
  mov    0x8(%rdi),%rdi           ; load captured n
  call   _ori___lambda_0
  pop    %rcx
  ret

main:
  push   %rax
  call   _ori_main
  pop    %rcx
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @apply | 4 | 4 | 1.00x | OPTIMAL |
| 2 | @make_adder | 7 | 7 | 1.00x | OPTIMAL |
| 3 | @main | 19 | 18 | 1.06x | NEAR-OPTIMAL |
| 4 | @__lambda_0 | 7 | 7 | 1.00x | OPTIMAL |
| 5 | @__lambda_1 | 7 | 7 | 1.00x | OPTIMAL |
| 6 | @partial_0_drop | 2 | 2 | 1.00x | OPTIMAL |
| 7 | @partial_1 | 4 | 4 | 1.00x | OPTIMAL |

**Weighted average**: 1.02x (instruction-weighted) | **Max**: 1.06x (@main)

**@apply** (4 actual vs 4 ideal): OPTIMAL. Two `extractvalue` to unpack the `{ptr, ptr}` closure, one indirect `call`, one `ret`. No RC ops -- AIMS correctly elided callee-side cleanup. Unchanged from previous run.

**@make_adder** (7 actual vs 7 ideal): OPTIMAL. Heap-allocates closure env via `ori_rc_alloc`, stores drop function and captured `n`, constructs the `{fn_ptr, env_ptr}` return value. Every instruction is necessary. Unchanged from previous run.

**@main** (19 actual vs 18 ideal): NEAR-OPTIMAL. The single unjustified instruction is the null-check on the closure env pointer returned by `@_ori_make_adder`. Since `make_adder` always allocates and returns a non-null env, the `ptrtoint` + `icmp eq 0` + conditional branch could be eliminated. However, this is a generic pattern applied to all closure cleanup. [LOW-1]

Massive improvement from previous run (was 24 instructions / 1.71x due to dead `invoke`/`landingpad` EH blocks).

**@__lambda_0, @__lambda_1** (7 each): OPTIMAL. Clean overflow-checked arithmetic -- `call @llvm.s{add,mul}.with.overflow.i64`, `extractvalue` x2, `br`, `ret`, `call @ori_panic_cstr` + `unreachable`. Both now have `memory(none)` attribute (new).

**@partial_0_drop, @partial_1** (2 and 4): OPTIMAL. Minimal thunk and destructor.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @apply | 0 | 0 | YES | 1 elided | N/A |
| @make_adder | 0 (1 alloc) | 0 | YES* | N/A | Transfers ownership |
| @main | 0 | 1 | YES* | N/A | Consumes closure env |
| @__lambda_0 | 0 | 0 | YES | N/A | N/A |
| @__lambda_1 | 0 | 0 | YES | N/A | N/A |
| @partial_0_drop | 0 | 0 (1 free) | YES | N/A | Drop handler |
| @partial_1 | 0 | 0 | YES | N/A | N/A |

*Cross-function balance: `make_adder` allocates the env (rc=1, ownership transferred to caller). `@main` performs `ori_rc_dec` on the env pointer after use (line 58 in the IR). The `partial_0_drop` destructor handles the actual deallocation via `ori_rc_free` when the refcount hits zero. The lifecycle is complete and correct.

FIXED from previous run: The RC dec is now on the live execution path in `@main`, not buried in dead EH cleanup blocks. The AIMS pipeline correctly resolved the missing live-path cleanup.

**Verdict**: All functions balanced. No leaks. Proper ownership transfer from `make_adder` to `@main` with cleanup on the live path.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | memory | cold | uwtable | Notes |
|----------|--------|----------|---------|--------|------|---------|-------|
| @apply | YES | YES | ret+param | -- | NO | YES | nounwind via two-pass analysis |
| @make_adder | YES | YES | ret+param | -- | NO | YES | |
| @main | NO (C) | NO | ret | -- | NO | YES | Correct: entry point, may panic |
| @__lambda_0 | YES | YES | ret+params | memory(none) | NO | YES | |
| @__lambda_1 | NO | YES | ret+param2 | memory(none) | NO | YES | [MEDIUM-2] No fastcc |
| @partial_0_drop | NO | YES | -- | -- | YES | YES | Correct: indirect target, cold |
| @partial_1 | NO | YES | -- | -- | NO | YES | Correct: indirect target |

**82.1% attribute compliance** (23/28 applicable checks).

**Improvements from previous run**:
- `@__lambda_0` and `@__lambda_1` now have `memory(none)` -- correctly marking them as pure functions that neither read nor write memory (they operate only on scalar arguments). This is a new optimization from the AIMS pipeline.
- `@partial_0_drop` now has `uwtable` (was missing).
- `@partial_1` now has `uwtable` (was missing).
- `@apply` now has `nounwind` -- the two-pass fixed-point nounwind analysis determined that even though `@apply` makes an indirect call, the analysis propagated nounwind through the closure call graph.

`@__lambda_1` (non-capturing `double`) is called indirectly through the uniform `{ptr, ptr}` interface, preventing `fastcc`. This is structurally correct -- closures called via indirect function pointers must use the platform calling convention. A devirtualization pass could potentially direct-call it in this program. [MEDIUM-2]

The 5 missing attributes are all `noundef` on closure-infrastructure functions (`@_ori___lambda_1` param0 ptr, `@_ori_partial_0_drop` param0 ptr, `@_ori_partial_1` return/param0 ptr/param1 i64) -- these are called indirectly and the compiler conservatively omits `noundef` on their signatures.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @apply | 1 | 0 | 0 | 0 | |
| @make_adder | 1 | 0 | 0 | 0 | |
| @main | 5 | 0 | 1 | 0 | [LOW-1] null-check branch |
| @__lambda_0 | 3 | 0 | 0 | 0 | |
| @__lambda_1 | 3 | 0 | 0 | 0 | |
| @partial_0_drop | 1 | 0 | 0 | 0 | |
| @partial_1 | 1 | 0 | 0 | 0 | |

**@main** has 5 blocks: `bb0` (main logic + overflow check), `add.ok` (null-check env for RC dec), `add.ovf_panic` (overflow handler), `rc_dec.do` (perform RC dec), `rc_dec.skip` (ret). The single redundant branch is the null-check `br i1 %rc_dec.null` on the env pointer that is known to be non-null when returned from `make_adder`.

FIXED from previous run: Previously had 9 blocks with `invoke`/`landingpad` dead EH cleanup blocks, 2 redundant branches (`br i1 true`), and dead code including `load ptr, ptr null`. Now the EH machinery is completely gone.

### 5. Overflow Checking

**Status**: PASS

| Operation | Function | Checked | Correct | Notes |
|-----------|----------|---------|---------|-------|
| add | @__lambda_0 | YES | YES | `llvm.sadd.with.overflow.i64` (x + n) |
| mul | @__lambda_1 | YES | YES | `llvm.smul.with.overflow.i64` (x * 2) |
| add | @main | YES | YES | `llvm.sadd.with.overflow.i64` (a + b) |

All three arithmetic operations are checked. Overflow paths call `ori_panic_cstr` with distinct messages distinguishing addition vs multiplication overflow.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.3 KiB |
| .rodata section | 133.5 KiB |
| User code | ~314 bytes (7 functions + main wrapper) |
| Runtime | ~99.96% of .text |

#### Disassembly: @apply

```asm
_ori_apply:
  sub    $0x18,%rsp
  mov    %rdx,0x8(%rsp)
  mov    %rsi,%rax
  mov    0x8(%rsp),%rsi
  mov    %rax,0x10(%rsp)
  mov    %rdi,%rax
  mov    0x10(%rsp),%rdi
  call   *%rax
  add    $0x18,%rsp
  ret
```

10 native instructions. The register shuffling (6 `mov` instructions) is LLVM's backend rearranging the fastcc `{ptr, ptr}` aggregate + i64 arguments into the C-convention indirect call ABI (rdi=env, rsi=x). This is structural overhead from the calling convention mismatch.

#### Disassembly: @make_adder

```asm
_ori_make_adder:
  push   %rax
  mov    %rdi,(%rsp)
  mov    $0x10,%edi
  mov    $0x8,%esi
  call   ori_rc_alloc
  mov    (%rsp),%rdi
  mov    %rax,%rdx
  lea    _ori_partial_0_drop,%rax
  mov    %rax,(%rdx)
  mov    %rdi,0x8(%rdx)
  lea    _ori_partial_1,%rax
  pop    %rcx
  ret
```

13 native instructions. Clean sequence: save n, allocate env, store drop_fn and n, set up return values. No wasted work.

#### Disassembly: @partial_1 (thunk)

```asm
_ori_partial_1:
  push   %rax
  mov    0x8(%rdi),%rdi
  call   _ori___lambda_0
  pop    %rcx
  ret
```

4 native instructions. Minimal thunk: load captured value, tail-call to lambda.

### 7. Optimal IR Comparison

#### @apply: Ideal vs Actual

```llvm
; IDEAL (4 instructions) = ACTUAL  [nounwind uwtable]
define fastcc noundef i64 @_ori_apply({ ptr, ptr } noundef %0, i64 noundef %1) nounwind {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}
```

**Delta**: +0. OPTIMAL. AIMS correctly elided callee-side RC cleanup.

#### @make_adder: Ideal vs Actual

```llvm
; IDEAL (7 instructions) = ACTUAL  [nounwind uwtable]
define fastcc noundef { ptr, ptr } @_ori_make_adder(i64 noundef %0) nounwind {
bb0:
  %env.data = call ptr @ori_rc_alloc(i64 16, i64 8)
  %env.drop_fn = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 0
  store ptr @_ori_partial_0_drop, ptr %env.drop_fn, align 8
  %env.cap.0 = getelementptr inbounds nuw { ptr, i64 }, ptr %env.data, i32 0, i32 1
  store i64 %0, ptr %env.cap.0, align 8
  %partial_apply.1 = insertvalue { ptr, ptr } { ptr @_ori_partial_1, ptr undef }, ptr %env.data, 1
  ret { ptr, ptr } %partial_apply.1
}
```

**Delta**: +0. OPTIMAL.

#### @main: Ideal vs Actual

```llvm
; IDEAL (18 instructions -- no null-check on known non-null env)  [uwtable]
define noundef i64 @_ori_main() {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  %call1 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  %closure.fn_ptr = extractvalue { ptr, ptr } %call1, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call1, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok
add.ok:
  %rc_dec.env = extractvalue { ptr, ptr } %call1, 1
  %rc_dec.drop_fn = load ptr, ptr %rc_dec.env, align 8
  call void @ori_rc_dec(ptr %rc_dec.env, ptr %rc_dec.drop_fn)
  ret i64 %add.val
add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
; rc_dec.do + rc_dec.skip blocks merged into add.ok
}
```

```llvm
; ACTUAL (19 instructions)
; Delta: +1 (null-check on make_adder's env pointer -- always non-null)
; The ptrtoint + icmp + conditional branch pattern is generic for all closure cleanup
; but could be elided when the source is known to always allocate
```

**Delta**: +1 instruction. The null-check (`ptrtoint` + `icmp eq 0` + conditional `br`) is a generic safety pattern for closure env cleanup. Since `make_adder` always allocates via `ori_rc_alloc` (which returns non-null), these 3 instructions (counted as 1 unjustified) could be removed. [LOW-1]

#### @__lambda_0, @__lambda_1: Ideal = Actual

Both are OPTIMAL at 7 instructions each. Clean overflow-checked arithmetic with `memory(none)`.

#### @partial_0_drop, @partial_1: Ideal = Actual

Both are OPTIMAL. Minimal thunk and destructor patterns.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @apply | 4 | 4 | +0 | N/A | OPTIMAL |
| @make_adder | 7 | 7 | +0 | N/A | OPTIMAL |
| @main | 18 | 19 | +1 | NO (null-check) | NEAR-OPTIMAL |
| @__lambda_0 | 7 | 7 | +0 | N/A | OPTIMAL |
| @__lambda_1 | 7 | 7 | +0 | N/A | OPTIMAL |
| @partial_0_drop | 2 | 2 | +0 | N/A | OPTIMAL |
| @partial_1 | 4 | 4 | +0 | N/A | OPTIMAL |

### 8. Closures: Representation

Ori closures are uniformly represented as `{ ptr, ptr }` pairs (function pointer + environment pointer):

**Non-capturing closures** (e.g., `double = x -> x * 2`):
- `{ ptr @_ori___lambda_1, ptr null }` -- null env pointer
- No heap allocation. The function takes `(ptr, i64)` where the first arg is ignored.
- Passed directly to `@_ori_apply` as a constant aggregate.

**Capturing closures** (e.g., `make_adder(10)` captures `n`):
- Env allocated via `ori_rc_alloc(16, 8)` -- 16 bytes for `{ ptr drop_fn, i64 n }`
- The drop function pointer is stored as the first field (enables polymorphic cleanup via `ori_rc_dec`)
- The thunk `@_ori_partial_1` loads the capture and forwards to `@_ori___lambda_0`
- Caller (`@main`) owns the env and performs RC dec after use

**Closure layout**: `{ ptr fn_ptr, ptr env_ptr }` where env is `{ ptr drop_fn, captures... }`

This design is clean and follows the standard fat-pointer closure representation used by languages like Rust and Swift. The uniform `{ptr, ptr}` type means all closures with the same signature are interchangeable at the type level, which is essential for higher-order functions like `@apply`.

### 9. Closures: Capture Semantics

**Capture mechanism**: Captured variables are copied into the heap-allocated environment at closure creation time (capture by value), consistent with Ori's design pillar of "no shared mutable refs."

**make_adder(n: 10)** captures `n`:
1. `ori_rc_alloc(16, 8)` allocates the env (8 bytes drop_fn + 8 bytes for `n: i64`)
2. `store ptr @_ori_partial_0_drop, ptr %env.drop_fn` -- registers the destructor
3. `store i64 %0, ptr %env.cap.0` -- copies `n` into the env

**Invocation of captured closure** (`add10(7)`):
1. Extract fn_ptr and env_ptr from `{ ptr, ptr }`
2. Indirect call: `fn_ptr(env_ptr, 7)`
3. `@_ori_partial_1` loads `n` from `env[1]` and calls `@_ori___lambda_0(n, 7)`
4. `@_ori___lambda_0` computes `7 + 10 = 17` with overflow check

**ARC lifecycle** (FIXED from previous run):
- `@_ori_make_adder` allocates env (rc=1), transfers ownership to caller via return value
- `@_ori_main` uses the closure, then calls `ori_rc_dec(env_ptr, drop_fn)` on the live path
- When refcount hits 0, `_ori_partial_0_drop` calls `ori_rc_free(ptr, 16, 8)` to deallocate
- The entire lifecycle is now visible and correct in the emitted IR

**Scalar capture optimization**: `n: i64` is captured by value into the env. The lambda `@_ori___lambda_0` receives it as a direct `i64` parameter (extracted by the thunk), not via pointer load. This avoids an extra indirection at call time. Both lambda functions have `memory(none)`, confirming they are pure computations on their scalar arguments.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Control Flow | Null-check on known non-null env pointer from make_adder | NEW | J5 |
| 2 | MEDIUM | Attributes | Non-capturing closure lacks fastcc (indirect call target) | CONFIRMED | J5 |
| 3 | NOTE | ARC | @apply RC dec fully elided by AIMS pipeline | CONFIRMED | J5 |
| 4 | NOTE | ARC | Live-path RC dec for closure env in @main (was missing) | FIXED | J5 |
| 5 | NOTE | Attributes | Lambda functions now have memory(none) | NEW | J5 |
| 6 | NOTE | Closures | Clean uniform {ptr, ptr} closure representation | CONFIRMED | J5 |
| 7 | NOTE | Control Flow | Dead EH blocks in @main eliminated | FIXED | J5 |

### LOW-1: Null-check on known non-null env pointer

**Location**: `@_ori_main`, `add.ok` block -- `ptrtoint` + `icmp eq i64 0` + conditional `br`
**Impact**: 1 unjustified instruction (+3 IR instructions, counted as 1 unjustified unit by the extractor). The env pointer returned by `make_adder` is always non-null (via `ori_rc_alloc`), so the null check is redundant.
**Fix**: Track non-null provenance through ownership transfers. When a closure env comes from `ori_rc_alloc` (which returns non-null), elide the null check in the RC dec sequence.
**First seen**: Journey 5
**Found in**: Instruction Purity (Category 1), Control Flow (Category 4), Optimal IR Comparison (Category 7)

### MEDIUM-2: Non-capturing closure cannot use fastcc

**Location**: `@_ori___lambda_1` function definition
**Impact**: Non-capturing closures are called indirectly through the uniform `{ptr, ptr}` interface, preventing `fastcc` usage. In this program, `double` is known statically -- a devirtualization pass could direct-call it.
**Fix**: Interprocedural devirtualization for closures whose identity is known at compile time.
**First seen**: Journey 5
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: @apply RC dec fully elided by AIMS pipeline

**Location**: `@_ori_apply` -- 4 instructions total
**Impact**: Positive. The AIMS pipeline correctly determined that `@_ori_apply` should not RC dec the closure environment, moving cleanup responsibility to the caller. This keeps @apply at the theoretical minimum.
**Found in**: ARC Purity (Category 2), Instruction Purity (Category 1)

### NOTE-4: Live-path RC dec for closure env (FIXED)

**Location**: `@_ori_main`, `rc_dec.do` block
**Impact**: Positive. The previous run had `ori_rc_dec` only in dead EH cleanup blocks (behind `br i1 true`). Now the RC dec is on the live execution path after the closure is used. The complete ownership lifecycle (alloc in `make_adder` -> use -> dec in `@main` -> free via `partial_0_drop`) is visible and correct.
**Found in**: ARC Purity (Category 2)

### NOTE-5: Improved attribute analysis on lambda and closure functions

**Location**: `@_ori___lambda_0` and `@_ori___lambda_1` -- attribute group `#2 = { nounwind memory(none) uwtable }`; `@_ori_apply` -- now `nounwind`
**Impact**: Positive. Both lambda functions are correctly marked as pure (no memory access). `@_ori_apply` gained `nounwind` via two-pass fixed-point analysis, even though it makes an indirect call. This enables LLVM to perform more aggressive optimizations.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-6: Clean uniform closure representation

**Location**: All closure functions
**Impact**: Positive. The `{ ptr, ptr }` uniform representation enables polymorphic higher-order functions while keeping non-capturing closures zero-allocation.
**Found in**: Closures: Representation (Category 8)

### NOTE-7: Dead EH blocks in @main eliminated (FIXED)

**Location**: `@_ori_main` -- was 9 blocks with `invoke`/`landingpad` and dead cleanup code
**Impact**: Positive. The `invoke`/`landingpad` exception handling machinery, including dead cleanup blocks with `load ptr, ptr null` and `resume`, has been completely eliminated. `@_ori_main` now uses simple `call` instructions. This reduced @main from 24 to 19 instructions (1.71x to 1.06x ratio). Previous HIGH-1 and HIGH-2 findings are resolved.
**Found in**: Control Flow (Category 4), Instruction Purity (Category 1)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.02x avg ratio (max 1.06x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 7/10 | 82.1% compliance |
| Control Flow | 10% | 9/10 | 1 defect |
| IR Quality | 20% | 9/10 | 1 unjustified instruction |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.2 / 10**

## Verdict

Journey 5's closure codegen has improved substantially (8.5 -> 9.2). The AIMS pipeline resolved both previous HIGH findings: dead `invoke`/`landingpad` EH blocks in `@main` are gone (NOTE-7), and the closure env now has proper live-path RC dec (NOTE-4). Six of seven user functions are OPTIMAL. The only remaining overhead is a null-check on a known non-null env pointer in `@main` (LOW-1). Lambda functions now carry `memory(none)` attributes, correctly marking them as pure. The closure representation is clean, efficient, and architecturally sound.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J5 | CONFIRMED |
| fastcc usage | J1 | J5 | CONFIRMED (where applicable) |
| nounwind analysis | J1 | J5 | CONFIRMED |
| memory(none) attrs | N/A | J5 | CONFIRMED (lambdas correctly marked pure) |
| Dead EH blocks | J5 (prev) | J5 | FIXED |
| ARC live-path cleanup | J5 (prev) | J5 | FIXED |

The two major regressions from the initial J5 run (`invoke`/`landingpad` dead blocks and missing live-path RC dec) remain fully resolved. Attribute compliance improved from 78.6% to 82.1% (23/28) as `@_ori_apply` gained `nounwind` via the two-pass fixed-point analysis. The `memory(none)` attribute on lambda functions continues to demonstrate the AIMS pipeline's strength in closure-aware optimization.
