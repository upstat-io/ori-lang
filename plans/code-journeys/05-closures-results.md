---
journey: 5
slug: closures
theme: "I am a closure"
date: 2026-03-07
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

score: 8.8
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 5
  control_flow: 8
  ir_quality: 9
  binary_quality: 10
  other_findings: 9
score_metrics:
  instruction_ratio: 1.04
  instruction_ratio_max: 1.10
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
  other_high: 0
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

> The lexer (tokenizer) breaks raw source text into a stream of tokens — the smallest
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
> (AST) — a tree structure that represents the grammatical structure of the program.

**Nodes**: 27 | **Max depth**: 4 | **Functions**: 3 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @apply
│  ├─ Params: (f: (int) -> int, x: int)
│  ├─ Return: int
│  └─ Body: Call(f)
│       └─ x
├─ FnDecl @make_adder
│  ├─ Params: (n: int)
│  ├─ Return: (int) -> int
│  └─ Body: Lambda(x)
│       └─ BinOp(+)
│            ├─ Ident(x)
│            └─ Ident(n)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let double = Lambda(x) -> BinOp(*) [x, 2]
        ├─ Let a = Call(@apply) [f: double, x: 5]
        ├─ Let add10 = Call(@make_adder) [n: 10]
        ├─ Let b = Call(add10) [7]
        └─ BinOp(+) [a, b]
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
> RC overhead — parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 3 | **Elided**: 0 | **Net ops**: 3

<details>
<summary>ARC annotations</summary>

```text
@apply: +0 rc_inc, +1 rc_dec (consumes closure env after call)
@make_adder: +1 rc_alloc, +0 rc_dec (allocates env, ownership transferred to caller)
@main: +0 rc_inc, +1 rc_dec (drops closure env from make_adder after use)
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
  ├─ let double = Lambda(__lambda_1, captures=[])
  ├─ let a = @apply(f: double, x: 5)
  │    └─ call double(5)
  │         └─ 5 * 2 = 10
  │    → 10
  ├─ let add10 = @make_adder(n: 10)
  │    └─ Lambda(__lambda_0, captures=[n=10])
  ├─ let b = add10(7)
  │    └─ call __lambda_0(x=7, n=10)
  │         └─ 7 + 10 = 17
  │    → 17
  └─ a + b = 10 + 17 = 27
→ 27
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 3 | **Elided**: 0 | **Net ops**: 3

<details>
<summary>ARC annotations</summary>

```text
@apply: +0 rc_inc, +1 rc_dec (closure env consumed — null-checked before dec)
@make_adder: +1 ori_rc_alloc(16, 8), +0 rc_dec (env: {drop_fn, n: i64})
@main: +0 rc_inc, +1 rc_dec (drops env from make_adder result)
@partial_0_drop: +1 ori_rc_free (destructor called when env refcount hits 0)
@partial_1: +0 rc_inc, +0 rc_dec (thunk — loads capture, forwards to lambda)
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
  %rc_dec.env = extractvalue { ptr, ptr } %0, 1
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.env to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do

rc_dec.do:                                        ; preds = %bb0
  %rc_dec.drop_fn = load ptr, ptr %rc_dec.env, align 8
  call void @ori_rc_dec(ptr %rc_dec.env, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:                                      ; preds = %rc_dec.do, %bb0
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
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_apply({ ptr, ptr } { ptr @_ori___lambda_1, ptr null }, i64 5)
  %call1 = call fastcc { ptr, ptr } @_ori_make_adder(i64 10)
  %closure.fn_ptr = extractvalue { ptr, ptr } %call1, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %call1, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 7)
  %rc_dec.env = extractvalue { ptr, ptr } %call1, 1
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.env to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do

rc_dec.do:                                        ; preds = %bb0
  %rc_dec.drop_fn = load ptr, ptr %rc_dec.env, align 8
  call void @ori_rc_dec(ptr %rc_dec.env, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:                                      ; preds = %rc_dec.do, %bb0
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %icall)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %rc_dec.skip
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %rc_dec.skip
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

; Function Attrs: nounwind memory(inaccessiblemem: readwrite)
declare void @ori_rc_dec(ptr, ptr) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #3

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #4

; Function Attrs: nounwind
declare noalias ptr @ori_rc_alloc(i64, i64) #5

; Function Attrs: cold nounwind
; --- @partial_0_drop ---
define void @_ori_partial_0_drop(ptr %0) #6 {
entry:
  call void @ori_rc_free(ptr %0, i64 16, i64 8)
  ret void
}

; Function Attrs: nounwind
declare void @ori_rc_free(ptr, i64, i64) #5

; Function Attrs: nounwind
; --- @partial_1 ---
define i64 @_ori_partial_1(ptr %0, i64 %1) #5 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, i64 }, ptr %0, i32 0, i32 1
  %cap.0 = load i64, ptr %cap.0.ptr, align 8
  %result = call fastcc i64 @_ori___lambda_0(i64 %cap.0, i64 %1)
  ret i64 %result
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64) #3

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nounwind memory(inaccessiblemem: readwrite) }
attributes #3 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { cold noreturn }
attributes #5 = { nounwind }
attributes #6 = { cold nounwind }
```

#### Disassembly

```asm
_ori_apply:
  sub    $0x28,%rsp
  mov    %rdx,0x8(%rsp)
  mov    %rsi,%rax
  mov    0x8(%rsp),%rsi
  mov    %rax,0x10(%rsp)
  mov    %rdi,%rax
  mov    0x10(%rsp),%rdi
  mov    %rdi,0x18(%rsp)
  call   *%rax
  mov    0x18(%rsp),%rsi
  mov    %rax,0x20(%rsp)
  cmp    $0x0,%rsi
  je     .skip
  mov    0x18(%rsp),%rdi
  mov    (%rdi),%rsi
  call   ori_rc_dec
.skip:
  mov    0x20(%rsp),%rax
  add    $0x28,%rsp
  ret

_ori_make_adder:
  push   %rax
  mov    %rdi,(%rsp)
  mov    $0x10,%edi
  mov    $0x8,%esi
  call   ori_rc_alloc
  mov    (%rsp),%rdi
  mov    %rax,%rdx
  lea    _ori_partial_0_drop(%rip),%rax
  mov    %rax,(%rdx)
  mov    %rdi,0x8(%rdx)
  lea    _ori_partial_1(%rip),%rax
  pop    %rcx
  ret

_ori_main:
  sub    $0x28,%rsp
  lea    _ori___lambda_1(%rip),%rdi
  xor    %eax,%eax
  mov    %eax,%esi
  mov    $0x5,%edx
  call   _ori_apply
  mov    %rax,0x10(%rsp)
  mov    $0xa,%edi
  call   _ori_make_adder
  mov    %rdx,%rdi
  mov    %rdi,0x18(%rsp)
  mov    $0x7,%esi
  call   *%rax
  mov    0x18(%rsp),%rdx
  mov    %rax,0x20(%rsp)
  cmp    $0x0,%rdx
  je     .skip2
  mov    0x18(%rsp),%rdi
  mov    (%rdi),%rsi
  call   ori_rc_dec
.skip2:
  mov    0x20(%rsp),%rcx
  mov    0x10(%rsp),%rax
  add    %rcx,%rax
  mov    %rax,0x8(%rsp)
  seto   %al
  jo     .panic
  mov    0x8(%rsp),%rax
  add    $0x28,%rsp
  ret
.panic:
  lea    ovf_msg(%rip),%rdi
  call   ori_panic_cstr

_ori___lambda_0:
  push   %rax
  add    %rdi,%rsi
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
  imul   %rax,%rsi
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
  mov    0x8(%rdi),%rdi
  call   _ori___lambda_0
  pop    %rcx
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @apply | 11 | 10 | 1.10x | NEAR-OPTIMAL |
| 2 | @make_adder | 7 | 7 | 1.00x | OPTIMAL |
| 3 | @main | 19 | 18 | 1.06x | NEAR-OPTIMAL |
| 4 | @__lambda_0 | 7 | 7 | 1.00x | OPTIMAL |
| 5 | @__lambda_1 | 7 | 7 | 1.00x | OPTIMAL |
| 6 | @partial_0_drop | 2 | 2 | 1.00x | OPTIMAL |
| 7 | @partial_1 | 4 | 4 | 1.00x | OPTIMAL |

**Weighted average**: 1.04x | **Max**: 1.10x

**@apply** (11 actual vs 10 ideal): The redundant `extractvalue` at `%rc_dec.env` extracts the env pointer a second time (already extracted as `%closure.env_ptr`). This is 1 unjustified instruction — LLVM may CSE it, but it's redundant in the IR. [LOW-1]

**@main** (19 actual vs 18 ideal): Same pattern — the `%rc_dec.env = extractvalue { ptr, ptr } %call1, 1` duplicates the already-extracted `%closure.env_ptr`. 1 unjustified instruction. [LOW-1]

All other functions are OPTIMAL. The lambda functions, partial application thunk, and drop function are tight and efficient.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @apply | 0 | 1 | YES* | N/A | Consumes closure |
| @make_adder | 0 (1 alloc) | 0 | YES* | N/A | Transfers ownership |
| @main | 0 | 1 | YES* | N/A | Consumes closure |
| @__lambda_0 | 0 | 0 | YES | N/A | N/A |
| @__lambda_1 | 0 | 0 | YES | N/A | N/A |
| @partial_0_drop | 0 | 0 (1 free) | YES | N/A | Drop handler |
| @partial_1 | 0 | 0 | YES | N/A | N/A |

*Balance is cross-function: `make_adder` allocates (rc=1), caller (`@main`) decrements after use. The `@apply` function's rc_dec is a no-op for the non-capturing `double` closure (null env pointer), and correctly decrements for capturing closures.

**Verdict**: All RC operations are correct and balanced across the closure lifecycle. The null-check pattern (`ptrtoint ptr to i64; icmp eq i64 ..., 0`) before rc_dec correctly handles non-capturing closures with null env pointers. Zero leaks, zero double-frees.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | cold | uwtable | Notes |
|----------|--------|----------|---------|------|---------|-------|
| @apply | YES | NO | ret+param | NO | YES | Correct: indirect call may panic |
| @make_adder | YES | YES | param | NO | YES | |
| @main | NO (C) | NO | ret | NO | YES | Correct: entry point, may panic |
| @__lambda_0 | YES | YES | ret+params | NO | YES | |
| @__lambda_1 | NO | YES | ret+param2 | NO | YES | [MEDIUM-2] No fastcc — indirect target |
| @partial_0_drop | NO | YES | N/A | YES | NO | Correct: indirect target, cold |
| @partial_1 | NO | YES | N/A | NO | NO | Correct: indirect target |

**60.0% attribute compliance**: The lower compliance is structural — closures called via indirect function pointers cannot use `fastcc` (the calling convention must match the indirect call site). This is correct behavior, not a deficiency.

`@__lambda_1` (non-capturing `double`) is called indirectly through `@_ori_apply` even though in this specific program it's always the same function. Direct-call optimization for non-capturing closures known at compile time would be beneficial but is not a correctness issue. [MEDIUM-2]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @apply | 3 | 0 | 1 | 0 | [LOW-3] null-check branch |
| @make_adder | 1 | 0 | 0 | 0 | |
| @main | 5 | 0 | 1 | 0 | [LOW-3] null-check branch |
| @__lambda_0 | 3 | 0 | 0 | 0 | |
| @__lambda_1 | 3 | 0 | 0 | 0 | |
| @partial_0_drop | 1 | 0 | 0 | 0 | |
| @partial_1 | 1 | 0 | 0 | 0 | |

The 2 "redundant" branches are the null-check patterns for closure env pointers (`br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do`). These are structurally necessary for correctness — the compiler doesn't know at codegen time whether a closure has captures. However, in `@_ori_main`, the closure from `make_adder` always has a non-null env, so the null check could theoretically be elided with interprocedural analysis. [LOW-3]

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
| .text section | 868.8 KiB |
| .rodata section | 133.5 KiB |
| User code | 361 bytes (7 functions + main wrapper) |
| Runtime | ~99.96% of .text |

#### Disassembly: @apply

```asm
_ori_apply:
  sub    $0x28,%rsp
  mov    %rdx,0x8(%rsp)           ; save x
  mov    %rsi,%rax                ; env_ptr -> rax
  mov    0x8(%rsp),%rsi           ; x -> rsi (arg 1)
  mov    %rax,0x10(%rsp)          ; save env_ptr
  mov    %rdi,%rax                ; fn_ptr -> rax
  mov    0x10(%rsp),%rdi          ; env_ptr -> rdi (arg 0)
  mov    %rdi,0x18(%rsp)          ; save env_ptr again
  call   *%rax                    ; indirect call: fn_ptr(env, x)
  mov    0x18(%rsp),%rsi          ; reload env_ptr
  mov    %rax,0x20(%rsp)          ; save result
  cmp    $0x0,%rsi                ; null check env
  je     .skip
  mov    0x18(%rsp),%rdi
  mov    (%rdi),%rsi              ; load drop_fn
  call   ori_rc_dec               ; RC-- on env
.skip:
  mov    0x20(%rsp),%rax          ; return result
  add    $0x28,%rsp
  ret
```

#### Disassembly: @make_adder

```asm
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
```

#### Disassembly: @partial_1 (thunk)

```asm
_ori_partial_1:
  push   %rax
  mov    0x8(%rdi),%rdi           ; load captured n from env
  call   _ori___lambda_0          ; tail-call-like to lambda
  pop    %rcx
  ret
```

### 7. Optimal IR Comparison

#### @apply: Ideal vs Actual

```llvm
; IDEAL (10 instructions — indirect call + null-checked RC dec)
define fastcc noundef i64 @_ori_apply({ ptr, ptr } %0, i64 noundef %1) #0 {
bb0:
  %closure.fn_ptr = extractvalue { ptr, ptr } %0, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %0, 1
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, i64 %1)
  %rc_dec.null = icmp eq ptr %closure.env_ptr, null
  br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do
rc_dec.do:
  %rc_dec.drop_fn = load ptr, ptr %closure.env_ptr, align 8
  call void @ori_rc_dec(ptr %closure.env_ptr, ptr %rc_dec.drop_fn)
  br label %rc_dec.skip
rc_dec.skip:
  ret i64 %icall
}
```

```llvm
; ACTUAL (11 instructions)
; Delta: +1 (redundant extractvalue for rc_dec.env)
```

**Delta**: +1 instruction. The `%rc_dec.env = extractvalue { ptr, ptr } %0, 1` duplicates `%closure.env_ptr`. Also uses `ptrtoint`+`icmp i64` instead of direct `icmp ptr ... null`. Minor inefficiency.

#### @make_adder: Ideal vs Actual

```llvm
; IDEAL = ACTUAL (7 instructions)
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

**Delta**: +0. OPTIMAL. Clean env allocation with structured GEP access.

#### @main: Ideal vs Actual

```llvm
; IDEAL (18 instructions)
; Same as actual minus the redundant extractvalue for rc_dec.env
```

**Delta**: +1 instruction (same redundant extractvalue pattern as @apply).

#### @__lambda_0, @__lambda_1: Ideal = Actual

Both are OPTIMAL at 7 instructions each. Clean overflow-checked arithmetic.

#### @partial_0_drop, @partial_1: Ideal = Actual

Both are OPTIMAL. The thunk pattern (load capture + call lambda) is minimal.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @apply | 10 | 11 | +1 | NO | NEAR-OPTIMAL |
| @make_adder | 7 | 7 | +0 | N/A | OPTIMAL |
| @main | 18 | 19 | +1 | NO | NEAR-OPTIMAL |
| @__lambda_0 | 7 | 7 | +0 | N/A | OPTIMAL |
| @__lambda_1 | 7 | 7 | +0 | N/A | OPTIMAL |
| @partial_0_drop | 2 | 2 | +0 | N/A | OPTIMAL |
| @partial_1 | 4 | 4 | +0 | N/A | OPTIMAL |

### 8. Closures: Representation

Ori closures are uniformly represented as `{ ptr, ptr }` pairs (function pointer + environment pointer):

**Non-capturing closures** (e.g., `double = x -> x * 2`):
- `{ ptr @_ori___lambda_1, ptr null }` — null env pointer
- No heap allocation. The function takes `(ptr, i64)` where the first arg is ignored.
- Passed directly to `@_ori_apply` as a constant aggregate.

**Capturing closures** (e.g., `make_adder(10)` captures `n`):
- Env allocated via `ori_rc_alloc(16, 8)` — 16 bytes for `{ ptr drop_fn, i64 n }`
- The drop function pointer is stored as the first field (enables polymorphic cleanup)
- The thunk `@_ori_partial_1` loads the capture and forwards to `@_ori___lambda_0`

**Closure layout**: `{ ptr fn_ptr, ptr env_ptr }` where env is `{ ptr drop_fn, captures... }`

This design is clean and follows the standard fat-pointer closure representation. The uniform `{ptr, ptr}` type means all closures with the same signature are interchangeable at the type level, which is essential for higher-order functions like `@apply`.

### 9. Closures: Capture

**Capture mechanism**: Captured variables are copied into the heap-allocated environment at closure creation time (capture by value, consistent with Ori's design pillar).

**make_adder(n: 10)** captures `n`:
1. `ori_rc_alloc(16, 8)` allocates the env (8 bytes drop_fn + 8 bytes for `n: i64`)
2. `store ptr @_ori_partial_0_drop, ptr %env.drop_fn` — registers the destructor
3. `store i64 %0, ptr %env.cap.0` — copies `n` into the env

**Invocation of captured closure** (`add10(7)`):
1. Extract fn_ptr and env_ptr from `{ ptr, ptr }`
2. Indirect call: `fn_ptr(env_ptr, 7)`
3. `@_ori_partial_1` loads `n` from `env[1]` and calls `@_ori___lambda_0(n, 7)`
4. `@_ori___lambda_0` computes `7 + 10 = 17` with overflow check

**ARC lifecycle**:
- `make_adder` creates env with rc=1 (via `ori_rc_alloc`)
- `@_ori_main` calls the closure, then rc_dec's the env (rc -> 0 -> `_ori_partial_0_drop` -> `ori_rc_free`)
- Non-capturing `double` passes through `@_ori_apply` with null env — the null check skips the rc_dec

The capture implementation is correct and efficient. The only overhead is the two-level indirection for capturing closures (thunk -> lambda), which is inherent to the partial application pattern and enables separate compilation.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | IR Quality | Redundant extractvalue for rc_dec env pointer | NEW | J5 |
| 2 | MEDIUM | Attributes | Non-capturing closure lacks fastcc (indirect call target) | NEW | J5 |
| 3 | LOW | Control Flow | Null-check branches on known-non-null env pointers | NEW | J5 |
| 4 | NOTE | ARC | Correct cross-function ARC balance for closure lifecycle | NEW | J5 |
| 5 | NOTE | Closures | Clean uniform {ptr, ptr} closure representation | NEW | J5 |

### LOW-1: Redundant extractvalue for rc_dec env pointer

**Location**: `@_ori_apply` and `@_ori_main`, `%rc_dec.env = extractvalue`
**Impact**: 1 extra instruction per function (2 total) — LLVM may CSE these away
**Fix**: Reuse `%closure.env_ptr` instead of re-extracting from the aggregate
**First seen**: Journey 5
**Found in**: Instruction Purity (Category 1), Optimal IR Comparison (Category 7)

### MEDIUM-2: Non-capturing closure cannot use fastcc

**Location**: `@_ori___lambda_1` function definition
**Impact**: Non-capturing closures are called indirectly through the uniform `{ptr, ptr}` interface, preventing `fastcc` usage. In this program, `double` is known statically — a devirtualization pass could direct-call it.
**Fix**: Interprocedural devirtualization for closures whose identity is known at compile time
**First seen**: Journey 5
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-3: Null-check on known-non-null env pointer

**Location**: `@_ori_main`, rc_dec null check after `make_adder` call
**Impact**: Unnecessary branch — `make_adder` always returns a non-null env pointer
**Fix**: Interprocedural analysis to propagate non-null guarantees from allocators
**First seen**: Journey 5
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Correct cross-function ARC balance

**Location**: Closure lifecycle across `make_adder` (alloc) -> `main` (use + dec)
**Impact**: Positive — demonstrates correct ownership transfer semantics for closures
**Found in**: ARC Purity (Category 2)

### NOTE-5: Clean uniform closure representation

**Location**: All closure functions
**Impact**: Positive — `{ ptr, ptr }` uniform representation enables polymorphic higher-order functions while keeping non-capturing closures zero-allocation
**Found in**: Closures: Representation (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.04x avg ratio (max 1.10x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 5/10 | 60.0% compliance |
| Control Flow | 10% | 8/10 | 2 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 9/10 | 1 low |

**Overall: 8.8 / 10**

## Verdict

Journey 5's closure codegen is strong. The uniform `{ptr, ptr}` representation is clean and correct, with zero-allocation non-capturing closures (null env) and properly ARC-managed capturing closures. The main score impact comes from attribute compliance (60%) — structural, not a deficiency, since indirect call targets cannot use `fastcc`. Instruction efficiency is near-optimal at 1.04x, with only 2 redundant extractvalue instructions across the entire module. ARC is perfectly balanced across the closure lifecycle.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J5 | CONFIRMED |
| fastcc usage | J1 | J5 | CONFIRMED (where applicable) |
| nounwind analysis | J1 | J5 | CONFIRMED (fixed-point analysis for closures) |

The nounwind fixed-point analysis correctly identifies that `make_adder`, `__lambda_0`, `__lambda_1`, `partial_0_drop`, and `partial_1` are nounwind, while `apply` and `main` are not (they call through function pointers or call functions that may panic). This is a more sophisticated nounwind analysis than Journey 1's simple case.
