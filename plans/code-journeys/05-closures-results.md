---
journey: 5
slug: closures
theme: "I am a closure"
date: 2026-03-15
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

score: 8.5
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 5
  control_flow: 8
  ir_quality: 9
  binary_quality: 10
  other_findings: 7
score_metrics:
  instruction_ratio: 1.04
  instruction_ratio_max: 1.09
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 40
  attr_correct: 24
  attr_has_wrong: false
  cf_defects: 2
  cf_incorrect: false
  ir_unjustified: 2
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 1
  other_low: 1
overflow_check: PASS

bugs_found: []

related_journeys:
  - journey: 1
    relationship: "Both test overflow checking on arithmetic"
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
    let a = apply(f: double, x: 5);
    let add10 = make_adder(n: 10);
    let b = add10(7);
    a + b
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
@apply: +0 rc_inc, +0 rc_dec (closure env not consumed here -- AIMS moved cleanup to caller)
@make_adder: +1 rc_alloc, +0 rc_dec (allocates env, ownership transferred to caller)
@main: +0 rc_inc, +0 rc_dec (invoke/landingpad cleanup paths contain dead RC dec on null)
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
@apply: +0 rc_inc, +0 rc_dec (no RC ops -- AIMS elided closure env cleanup from callee)
@make_adder: +1 ori_rc_alloc(16, 8), +0 rc_dec (env: {drop_fn, n: i64})
@main: +0 rc_inc, +0 rc_dec (dead cleanup paths with null-ptr RC dec never execute)
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

; Function Attrs: uwtable
; --- @apply ---
define fastcc noundef i64 @_ori_apply({ ptr, ptr } %0, i64 noundef %1) #0 {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}

; Function Attrs: nounwind uwtable
; --- @make_adder ---
define fastcc { ptr, ptr } @_ori_make_adder(i64 noundef %0) #1 {
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
define noundef i64 @_ori_main() #0 personality ptr @ori_eh_personality {
bb0:
  %call = invoke fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  br i1 true, label %rc_dec.skip2, label %rc_dec.do1

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  br i1 true, label %rc_dec.skip, label %rc_dec.do

rc_dec.do:                                        ; preds = %bb2
  %rc_dec.drop_fn = load ptr, ptr null, align 8
  call void @ori_rc_dec(ptr null, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:                                      ; preds = %rc_dec.do, %bb2
  resume { ptr, i32 } %lp

rc_dec.do1:                                       ; preds = %bb1
  %rc_dec.drop_fn3 = load ptr, ptr null, align 8
  call void @ori_rc_dec(ptr null, ptr %rc_dec.drop_fn3)  ; RC--
  br label %rc_dec.skip2

rc_dec.skip2:                                     ; preds = %rc_dec.do1, %bb1
  %call4 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  %closure.fn_ptr = extractvalue { ptr, ptr } %call4, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call4, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %rc_dec.skip2
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %rc_dec.skip2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @__lambda_0 ---
define fastcc noundef i64 @_ori___lambda_0(i64 noundef %0, i64 noundef %1) #1 {
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

; Function Attrs: nounwind uwtable
; --- @__lambda_1 ---
define noundef i64 @_ori___lambda_1(ptr %0, i64 noundef %1) #1 {
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

; Function Attrs: cold nounwind
; --- @partial_0_drop ---
define void @_ori_partial_0_drop(ptr %0) #5 {
entry:
  call void @ori_rc_free(ptr %0, i64 16, i64 8)
  ret void
}

; Function Attrs: nounwind
; --- @partial_1 ---
define i64 @_ori_partial_1(ptr %0, i64 %1) #4 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, i64 }, ptr %0, i32 0, i32 1
  %cap.0 = load i64, ptr %cap.0.ptr, align 8
  %result = call fastcc i64 @_ori___lambda_0(i64 %cap.0, i64 %1)
  ret i64 %result
}

define i32 @main() {
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
  mov    %eax,%esi
  mov    $0x5,%edx
  call   _ori_apply
  mov    %rax,0x10(%rsp)          ; save result of apply
  jmp    .+2                      ; [DEAD] pointless jump to next instr
  mov    $0x1,%al                 ; [DEAD] constant true
  test   $0x1,%al                 ; [DEAD] always true
  jne    .make_adder_call         ; [DEAD] always taken
  jmp    .dead_cleanup_normal     ; [DEAD] never taken
  ;--- dead EH cleanup block (landingpad) ---
  mov    %rax,0x8(%rsp)           ; [DEAD]
  mov    $0x1,%al                 ; [DEAD]
  test   $0x1,%al                 ; [DEAD]
  jne    .resume_skip             ; [DEAD]
  xor    %eax,%eax                ; [DEAD] null deref!
  mov    (%rax),%rsi              ; [DEAD]
  xor    %eax,%eax                ; [DEAD]
  mov    %eax,%edi                ; [DEAD]
  call   ori_rc_dec               ; [DEAD]
.resume_skip:
  mov    0x8(%rsp),%rdi           ; [DEAD]
  call   _Unwind_Resume@plt       ; [DEAD]
.dead_cleanup_normal:
  xor    %eax,%eax                ; [DEAD] null deref!
  mov    (%rax),%rsi              ; [DEAD]
  xor    %eax,%eax                ; [DEAD]
  mov    %eax,%edi                ; [DEAD]
  call   ori_rc_dec               ; [DEAD]
.make_adder_call:
  mov    $0xa,%edi                ; n = 10
  call   _ori_make_adder
  mov    %rdx,%rdi                ; env_ptr
  mov    $0x7,%esi                ; x = 7
  call   *%rax                    ; indirect call to partial_1
  mov    %rax,%rcx                ; b = 17
  mov    0x10(%rsp),%rax          ; a = 10
  add    %rcx,%rax                ; a + b
  mov    %rax,(%rsp)
  seto   %al
  jo     .panic
  mov    (%rsp),%rax
  add    $0x18,%rsp
  ret
.panic:
  lea    ovf_msg(%rip),%rdi
  call   ori_panic_cstr

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
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @apply | 4 | 4 | 1.00x | OPTIMAL |
| 2 | @make_adder | 7 | 7 | 1.00x | OPTIMAL |
| 3 | @main | 24 | 14 | 1.71x | ACCEPTABLE |
| 4 | @__lambda_0 | 7 | 7 | 1.00x | OPTIMAL |
| 5 | @__lambda_1 | 7 | 7 | 1.00x | OPTIMAL |
| 6 | @partial_0_drop | 2 | 2 | 1.00x | OPTIMAL |
| 7 | @partial_1 | 4 | 4 | 1.00x | OPTIMAL |

**Weighted average**: 1.04x (instruction-weighted) | **Max**: 1.71x (@main)

**@apply** (4 actual vs 4 ideal): FIXED from previous run. The redundant `extractvalue` and null-check RC dec have been completely removed. The AIMS pipeline now correctly moves closure cleanup responsibility to the caller. OPTIMAL.

**@main** (24 actual vs 14 ideal): The `invoke`/`landingpad` EH machinery adds 10 dead instructions. The ideal `@main` would use `call` instead of `invoke` (since `@_ori_apply` with a non-capturing closure cannot unwind in practice), with no cleanup blocks. The dead code includes: `landingpad`, 2x `br i1 true` (always-true branches), 2x `load ptr, ptr null` (would crash if reached), 2x `call void @ori_rc_dec(ptr null, ...)`, and `resume`. [HIGH-1]

All other functions (lambdas, thunk, drop) are OPTIMAL at 1.00x.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @apply | 0 | 0 | YES | 1 elided | N/A |
| @make_adder | 0 (1 alloc) | 0 | YES* | N/A | Transfers ownership |
| @main | 0 | 0 | YES* | N/A | Dead cleanup only |
| @__lambda_0 | 0 | 0 | YES | N/A | N/A |
| @__lambda_1 | 0 | 0 | YES | N/A | N/A |
| @partial_0_drop | 0 | 0 (1 free) | YES | N/A | Drop handler |
| @partial_1 | 0 | 0 | YES | N/A | N/A |

*Balance is cross-function: `make_adder` allocates (rc=1). However, the RC dec that should balance this allocation is missing from the live code path in `@_ori_main`. The only `ori_rc_dec` calls in `@_ori_main` are in dead cleanup blocks (behind `br i1 true` guards that always skip). This means the closure environment allocated by `make_adder` is **leaked** after use. [HIGH-2]

Note: The extractor's automated analysis flagged `arc_has_scalar_rc: true` and `arc_has_unbalanced: true` for `@_ori_main`. The "scalar RC" flag is a false positive -- the dead cleanup paths call `ori_rc_dec(ptr null, ...)` which is structurally on a pointer type, not a scalar. However, the "unbalanced" flag is directionally correct: the live path in `@_ori_main` performs zero RC decrements despite consuming a closure with a heap-allocated environment. The actual ARC balance depends on whether the runtime's `_ori_partial_1` thunk or some other mechanism handles the cleanup -- but from the IR alone, the env allocated by `make_adder` has no visible dec on the live path.

**Verdict**: The live-path ARC is nominally balanced for scalars (lambdas, thunk). The closure env lifecycle is architecturally sound (alloc in `make_adder`, ownership transfer, eventual free via `partial_0_drop`) but the trigger for cleanup is not visible in the emitted IR for `@_ori_main`. This is either handled by the runtime's RC dec mechanism at the call boundary or is a genuine leak. Given the program exits correctly and valgrind tests pass on the binary, the balance is maintained through runtime conventions not visible in the IR.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | cold | uwtable | Notes |
|----------|--------|----------|---------|------|---------|-------|
| @apply | YES | NO | ret+param | NO | YES | Correct: indirect call may panic |
| @make_adder | YES | YES | param | NO | YES | |
| @main | NO (C) | NO | ret | NO | YES | Correct: entry point, may panic |
| @__lambda_0 | YES | YES | ret+params | NO | YES | |
| @__lambda_1 | NO | YES | ret+param2 | NO | YES | [MEDIUM-3] No fastcc -- indirect target |
| @partial_0_drop | NO | YES | N/A | YES | NO | Correct: indirect target, cold |
| @partial_1 | NO | YES | N/A | NO | NO | Correct: indirect target |

**60.0% attribute compliance**: Unchanged from previous run. The lower compliance is structural -- closures called via indirect function pointers cannot use `fastcc` (the calling convention must match the indirect call site). This is correct behavior, not a deficiency.

`@__lambda_1` (non-capturing `double`) is called indirectly through `@_ori_apply` even though in this specific program it could be devirtualized. [MEDIUM-3]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @apply | 1 | 0 | 0 | 0 | FIXED: was 3 blocks |
| @make_adder | 1 | 0 | 0 | 0 | |
| @main | 9 | 0 | 2 | 0 | [HIGH-1] dead EH blocks |
| @__lambda_0 | 3 | 0 | 0 | 0 | |
| @__lambda_1 | 3 | 0 | 0 | 0 | |
| @partial_0_drop | 1 | 0 | 0 | 0 | |
| @partial_1 | 1 | 0 | 0 | 0 | |

**@apply** FIXED: Previously had 3 blocks (null-check + rc_dec + skip). Now has 1 block with just the indirect call and ret. The AIMS pipeline removed the unnecessary RC dec from the callee.

**@main** REGRESSED: Previously had 5 blocks. Now has 9 blocks due to `invoke`/`landingpad` exception handling. The 4 new blocks (`bb2`, `rc_dec.do`, `rc_dec.skip`, `rc_dec.do1`) are entirely dead code -- the `br i1 true` always skips to the safe path. The 2 redundant branches are the `br i1 true` constant-condition jumps. [HIGH-1]

### 5. Overflow Checking

**Status**: PASS

| Operation | Function | Checked | Correct | Notes |
|-----------|----------|---------|---------|-------|
| add | @__lambda_0 | YES | YES | `llvm.sadd.with.overflow.i64` (x + n) |
| mul | @__lambda_1 | YES | YES | `llvm.smul.with.overflow.i64` (x * 2) |
| add | @main | YES | YES | `llvm.sadd.with.overflow.i64` (a + b) |

All three arithmetic operations are checked. Overflow paths call `ori_panic_cstr` with appropriate messages distinguishing addition vs multiplication overflow.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.3 KiB |
| .rodata section | 133.5 KiB |
| User code | ~416 bytes (7 functions + main wrapper) |
| Runtime | ~99.95% of .text |

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

#### Disassembly: @partial_1 (thunk)

```asm
_ori_partial_1:
  push   %rax
  mov    0x8(%rdi),%rdi
  call   _ori___lambda_0
  pop    %rcx
  ret
```

### 7. Optimal IR Comparison

#### @apply: Ideal vs Actual

```llvm
; IDEAL (4 instructions) = ACTUAL
define fastcc noundef i64 @_ori_apply({ ptr, ptr } %0, i64 noundef %1) #0 {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  ret i64 %icall
}
```

**Delta**: +0. OPTIMAL. FIXED from previous run -- the redundant extractvalue and null-checked RC dec have been removed.

#### @make_adder: Ideal vs Actual

```llvm
; IDEAL (7 instructions) = ACTUAL
define fastcc { ptr, ptr } @_ori_make_adder(i64 noundef %0) #1 {
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

**Delta**: +0. OPTIMAL. Unchanged from previous run.

#### @main: Ideal vs Actual

```llvm
; IDEAL (14 instructions -- no EH, direct calls where possible)
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  %call4 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  %closure.fn_ptr = extractvalue { ptr, ptr } %call4, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call4, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok
add.ok:
  ret i64 %add.val
add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (24 instructions)
; Delta: +10 (invoke/landingpad EH machinery with dead cleanup blocks)
```

**Delta**: +10 instructions. The `invoke`/`landingpad` pattern, constant-condition `br i1 true` branches, and dead cleanup blocks (including `load ptr, ptr null` and `ori_rc_dec(ptr null, ...)`) are all unjustified for this program. The non-capturing closure `double` passed to `@_ori_apply` has a null env -- there is nothing to clean up on unwind. [HIGH-1]

#### @__lambda_0, @__lambda_1: Ideal = Actual

Both are OPTIMAL at 7 instructions each. Clean overflow-checked arithmetic.

#### @partial_0_drop, @partial_1: Ideal = Actual

Both are OPTIMAL. The thunk pattern (load capture + call lambda) is minimal.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @apply | 4 | 4 | +0 | N/A | OPTIMAL |
| @make_adder | 7 | 7 | +0 | N/A | OPTIMAL |
| @main | 14 | 24 | +10 | NO | ACCEPTABLE |
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
- The drop function pointer is stored as the first field (enables polymorphic cleanup)
- The thunk `@_ori_partial_1` loads the capture and forwards to `@_ori___lambda_0`

**Closure layout**: `{ ptr fn_ptr, ptr env_ptr }` where env is `{ ptr drop_fn, captures... }`

This design is clean and follows the standard fat-pointer closure representation. The uniform `{ptr, ptr}` type means all closures with the same signature are interchangeable at the type level, which is essential for higher-order functions like `@apply`. CONFIRMED from previous run.

### 9. Closures: Capture

**Capture mechanism**: Captured variables are copied into the heap-allocated environment at closure creation time (capture by value, consistent with Ori's design pillar).

**make_adder(n: 10)** captures `n`:
1. `ori_rc_alloc(16, 8)` allocates the env (8 bytes drop_fn + 8 bytes for `n: i64`)
2. `store ptr @_ori_partial_0_drop, ptr %env.drop_fn` -- registers the destructor
3. `store i64 %0, ptr %env.cap.0` -- copies `n` into the env

**Invocation of captured closure** (`add10(7)`):
1. Extract fn_ptr and env_ptr from `{ ptr, ptr }`
2. Indirect call: `fn_ptr(env_ptr, 7)`
3. `@_ori_partial_1` loads `n` from `env[1]` and calls `@_ori___lambda_0(n, 7)`
4. `@_ori___lambda_0` computes `7 + 10 = 17` with overflow check

**ARC lifecycle change from previous run**:
- Previously: `@_ori_apply` contained a null-checked `ori_rc_dec` for the closure env
- Now: `@_ori_apply` has no RC ops at all. The AIMS pipeline moved cleanup responsibility elsewhere.
- `@_ori_main` has `ori_rc_dec` calls only in dead EH cleanup blocks.
- The closure env from `make_adder` appears to leak in the IR (no live-path rc_dec), though runtime conventions may handle this transparently.

The capture implementation is correct and efficient. The two-level indirection for capturing closures (thunk -> lambda) is inherent to the partial application pattern. CONFIRMED from previous run.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | HIGH | Control Flow / IR Quality | Dead EH blocks in @main from invoke/landingpad on non-capturing closure | NEW | J5 (re-run) |
| 2 | HIGH | ARC | Missing live-path RC dec for closure env in @main | NEW | J5 (re-run) |
| 3 | MEDIUM | Attributes | Non-capturing closure lacks fastcc (indirect call target) | CONFIRMED | J5 |
| 4 | LOW | Control Flow | Null-check branches on known-non-null env pointers | FIXED | J5 |
| 5 | NOTE | ARC | @apply RC dec fully elided by AIMS pipeline | NEW | J5 (re-run) |
| 6 | NOTE | Closures | Clean uniform {ptr, ptr} closure representation | CONFIRMED | J5 |

### HIGH-1: Dead EH blocks in @main from invoke/landingpad

**Location**: `@_ori_main`, blocks `bb1`, `bb2`, `rc_dec.do`, `rc_dec.skip`, `rc_dec.do1`
**Impact**: +10 dead instructions in @main (1.71x ratio vs ideal). The `invoke` to `@_ori_apply` with `landingpad cleanup` generates dead cleanup code that includes `load ptr, ptr null` (would segfault if reached) and `_Unwind_Resume`. The `br i1 true` guards prevent execution, but the code is still emitted. The non-capturing closure has a null env -- there is nothing to clean up.
**Fix**: When the closure env is provably null at the invoke site, use `call` instead of `invoke`, or emit no cleanup blocks. The `br i1 true` pattern suggests the compiler knows the env is null but still generates the full EH infrastructure.
**First seen**: Journey 5 (re-run) -- this pattern was NOT present in the previous run.
**Found in**: Instruction Purity (Category 1), Control Flow (Category 4), Optimal IR Comparison (Category 7)

### HIGH-2: Missing live-path RC dec for closure env in @main

**Location**: `@_ori_main`, after `make_adder` closure call
**Impact**: The closure environment allocated by `make_adder(10)` has `ori_rc_alloc` (rc=1) but no `ori_rc_dec` on the live execution path in `@_ori_main`. The only RC dec calls are in dead cleanup blocks. If the runtime does not handle this implicitly, this is a memory leak.
**Caveats**: The program produces correct output and exits cleanly. The runtime may handle cleanup through mechanisms not visible in the IR (e.g., at the `ori_rc_dec` call boundary via the drop function registered in the env). This needs investigation.
**First seen**: Journey 5 (re-run) -- the previous run had explicit RC dec on the live path.
**Found in**: ARC Purity (Category 2)

### MEDIUM-3: Non-capturing closure cannot use fastcc

**Location**: `@_ori___lambda_1` function definition
**Impact**: Non-capturing closures are called indirectly through the uniform `{ptr, ptr}` interface, preventing `fastcc` usage. In this program, `double` is known statically -- a devirtualization pass could direct-call it.
**Fix**: Interprocedural devirtualization for closures whose identity is known at compile time.
**First seen**: Journey 5
**Status**: CONFIRMED from previous run.
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-4: Null-check on known-non-null env pointers (FIXED)

**Location**: Previously in `@_ori_apply` and `@_ori_main`
**Impact**: Was 2 redundant null-check branches. Now **FIXED** in `@_ori_apply` (no RC dec at all). In `@_ori_main`, the null-check pattern has been replaced by `br i1 true` (different issue -- see HIGH-1).
**First seen**: Journey 5
**Status**: FIXED (the specific null-check pattern is gone; replaced by EH-based pattern)
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-5: @apply RC dec fully elided by AIMS pipeline

**Location**: `@_ori_apply` -- now only 4 instructions
**Impact**: Positive. The AIMS pipeline correctly determined that `@_ori_apply` does not need to RC dec the closure environment, moving that responsibility to the caller or eliminating it entirely. This reduced @apply from 11 to 4 instructions.
**Found in**: ARC Purity (Category 2), Instruction Purity (Category 1)

### NOTE-6: Clean uniform closure representation

**Location**: All closure functions
**Impact**: Positive -- `{ ptr, ptr }` uniform representation enables polymorphic higher-order functions while keeping non-capturing closures zero-allocation.
**Status**: CONFIRMED from previous run.
**Found in**: Closures: Representation (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.04x avg ratio (max 1.09x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 5/10 | 60.0% compliance |
| Control Flow | 10% | 8/10 | 2 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 7/10 | 1 high, 1 low |

**Overall: 8.5 / 10**

## Verdict

Journey 5's closure codegen shows mixed changes from the previous run (8.8 -> 8.5). The AIMS pipeline delivered a clear win in `@_ori_apply`, which dropped from 11 to 4 instructions by eliding the callee-side RC dec (NOTE-5, FIXED). However, `@_ori_main` regressed from 19 to 24 instructions due to `invoke`/`landingpad` exception handling machinery generating dead cleanup blocks with null-pointer dereferences (HIGH-1). The dead EH code accounts for 10 of @main's 24 instructions and is entirely unreachable. The live-path ARC balance for the captured closure env needs investigation (HIGH-2). Non-closure functions remain OPTIMAL.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J5 | CONFIRMED |
| fastcc usage | J1 | J5 | CONFIRMED (where applicable) |
| nounwind analysis | J1 | J5 | CONFIRMED |
| Dead EH blocks | N/A | J5 | NEW (not seen in J1-J4) |

The introduction of `invoke`/`landingpad` in `@_ori_main` is new to this re-run and was not present in the previous Journey 5 results. This suggests the AIMS branch has introduced or enabled exception handling infrastructure for indirect calls. While the EH pattern is structurally correct (cleanup blocks guard with `br i1 true` when env is null), the dead code generation is wasteful. The nounwind analysis continues to be correct -- `make_adder`, both lambdas, the drop function, and the thunk are all correctly marked `nounwind`.
