---
journey: 2
slug: branching
theme: "I am a branch"
date: 2026-03-15
status: PASS
expected: 17
eval_result: 17
aot_result: 17
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of conditionals and comparisons"
learning_objectives:
  - "Understand how if/then/else lowers to LLVM IR branches, selects, and phi nodes"
  - "See overflow checking on negation (ssub.with.overflow from 0)"
  - "Compare branch-based vs select-based codegen for simple conditionals"
  - "Observe how nested if/else chains produce cascading block structures"
features:
  - branching
  - comparison
  - function_calls
  - multiple_functions
feature_description: "Branching with if/then/else, comparison operators, and multiple function calls"
score: 9.2
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 9
  control_flow: 7
  ir_quality: 9
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.05
  instruction_ratio_max: 1.14
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 23
  attr_correct: 22
  attr_has_wrong: false
  cf_defects: 5
  cf_incorrect: false
  ir_unjustified: 2
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
    relationship: "Same missing uwtable on main wrapper; same attribute quality"
---

# Journey 2: "I am a branch"

## Source

```ori
// Journey 2: "I am a branch"
// Slug: branching
// Difficulty: simple
// Features: branching, comparison, function_calls, multiple_functions
// Expected: my_abs(-7) + my_max(3, 10) + my_sign(0) = 7 + 10 + 0 = 17

@my_abs (n: int) -> int = if n < 0 then -n else n;

@my_max (a: int, b: int) -> int = if a > b then a else b;

@my_sign (n: int) -> int =
    if n > 0 then 1
    else (if n < 0 then -1 else 0);

@main () -> int = {
    let a = my_abs(n: -7);       // = 7
    let b = my_max(a: 3, b: 10); // = 10
    let c = my_sign(n: 0);       // = 0
    a + b + c                    // = 17
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 17        | 17       | (none) | (none) | PASS   |
| AOT     | 17        | 17       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals. This is the first
> stage of every compiler.

**Tokens**: 142 | **Keywords**: 10 | **Identifiers**: 20 | **Errors**: 0

The source file is 593 bytes. The lexer produces 142 tokens with zero errors. Keywords include `if` (x4), `then` (x4), `else` (x3), `let` (x3), and `@` function markers (x4). The increased token count compared to Journey 1 (77 tokens) reflects the branching syntax overhead.

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Lt Int(0) Then Minus Ident(n)
Else Ident(n) Semi

Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(int) Eq
If Ident(a) Gt Ident(b) Then Ident(a) Else Ident(b) Semi

Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Gt Int(0) Then Int(1) Else
LParen If Ident(n) Lt Int(0) Then Minus Int(1) Else Int(0) RParen Semi

Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus Int(7) RParen Semi
Let Ident(b) Eq Ident(my_max) LParen Ident(a) Colon Int(3) Comma
  Ident(b) Colon Int(10) RParen Semi
Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon Int(0) RParen Semi
Ident(a) Plus Ident(b) Plus Ident(c) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.
> If/then/else expressions become ternary nodes with condition, consequent, and alternative.

**Nodes**: 40 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

The parser produces 40 expression nodes across 4 function declarations. The nested `if` in `my_sign` produces an AST depth of 5. The `else (if ...)` is correctly parsed as an `if` expression nested inside the `else` branch, not as a special `else if` construct.

<details>
<summary>AST (simplified)</summary>

```text
Module
+-- FnDecl @my_abs
|  +-- Params: (n: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(<)
|       |    +-- Ident(n)
|       |    +-- Lit(0)
|       +-- Then: Unary(-)
|       |    +-- Ident(n)
|       +-- Else: Ident(n)
+-- FnDecl @my_max
|  +-- Params: (a: int, b: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(>)
|       |    +-- Ident(a)
|       |    +-- Ident(b)
|       +-- Then: Ident(a)
|       +-- Else: Ident(b)
+-- FnDecl @my_sign
|  +-- Params: (n: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(>)
|       |    +-- Ident(n)
|       |    +-- Lit(0)
|       +-- Then: Lit(1)
|       +-- Else: If (nested)
|            +-- Cond: BinOp(<)
|            |    +-- Ident(n)
|            |    +-- Lit(0)
|            +-- Then: Unary(-)
|            |    +-- Lit(1)
|            +-- Else: Lit(0)
+-- FnDecl @main
   +-- Return: int
   +-- Body: Block
        +-- Let a = Call(@my_abs, n: Unary(-) Lit(7))
        +-- Let b = Call(@my_max, a: Lit(3), b: Lit(10))
        +-- Let c = Call(@my_sign, n: Lit(0))
        +-- BinOp(+)
             +-- BinOp(+)
             |    +-- Ident(a)
             |    +-- Ident(b)
             +-- Ident(c)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit annotations everywhere.

**Constraints**: 18 | **Types inferred**: 9 | **Unifications**: 14 | **Errors**: 0

All types resolve to `int` or `bool`. The comparison operators (`<`, `>`) produce `bool` values used as `if` conditions. The type checker confirms that `Neg<int> -> int`, `Lt<int, int> -> bool`, `Gt<int, int> -> bool`, and `Add<int, int> -> int` are all valid. The nested `if` in `my_sign` is correctly typed: both branches (`-1` and `0`) are `int`, and the outer branches (`1` and inner `if`) are also `int`.

<details>
<summary>Inferred types</summary>

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n
//                           ^ bool (Lt<int, int> -> bool)
//                                   ^ int (Neg<int> -> int)
//                                          ^ int

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int> -> bool)
//                                                ^ int    ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1
    //  ^ bool    ^ int
    else (if n < 0 then -1 else 0)
    //      ^ bool      ^ int   ^ int

@main () -> int = {
    let a: int = my_abs(n: -7)     // inferred from @my_abs return
    let b: int = my_max(a: 3, b: 10)  // inferred from @my_max return
    let c: int = my_sign(n: 0)     // inferred from @my_sign return
    a + b + c  // -> int (Add<int, int> -> int, twice)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form -- a flat
> sequence of operations suitable for backend consumption. It desugars syntactic sugar,
> lowers complex expressions, and resolves named arguments to positional order.

**Transforms**: 4 | **Desugared**: 0 | **Errors**: 0

The canonicalizer produces 43 canon nodes from 40 AST nodes. Named arguments in function calls are resolved to positional order. The `if/then/else` expressions are lowered to canonical `If(cond, then, else)` nodes. The nested `if` in `my_sign` becomes two canonical `If` nodes.

<details>
<summary>Key transformations</summary>

```text
- 43 canon nodes from 40 AST nodes
- 4 roots: @my_abs, @my_max, @my_sign, @main
- 6 constants: int literals -7, 0, 1, 3, 10, -1
- 0 decision trees (if/then/else, not pattern matching)
- Named arguments resolved to positional order
- Unary negation preserved as Unary(Neg, expr)
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and inserts
> reference counting operations. It performs borrow inference to minimize RC overhead --
> parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

This program uses only `int` scalars (i64), which are value types stored directly in registers. No heap allocation occurs, so no reference counting is needed. This is the optimal outcome for a scalar-only program.

<details>
<summary>ARC annotations</summary>

```text
@my_abs: no heap values -- pure scalar (int param, int return)
@my_max: no heap values -- pure scalar (int params, int return)
@my_sign: no heap values -- pure scalar (int param, int return)
@main: no heap values -- all let bindings hold int scalars
Total RC ops: 0 (optimal for scalar-only program)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without compilation.
> It serves as the reference implementation for correctness testing -- if eval and AOT
> disagree, the bug is in LLVM codegen, not the interpreter.

**Result**: 17 | **Status**: PASS

The eval trace shows correct execution: `my_abs(-7)` evaluates `n < 0` as `true`, then computes `-(-7) = 7`. `my_max(3, 10)` evaluates `a > b` as `false` (3 > 10 is false), returning `b = 10`. `my_sign(0)` evaluates `n > 0` as `false`, then `n < 0` as `false`, returning `0`. Final result: `7 + 10 + 0 = 17`.

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-- let a = @my_abs(n: -7)
  |    +-- -7 < 0 = true (Lt)
  |    +-- -(-7) = 7 (Neg)
  +-- let b = @my_max(a: 3, b: 10)
  |    +-- 3 > 10 = false (Gt)
  |    +-- else: b = 10
  +-- let c = @my_sign(n: 0)
  |    +-- 0 > 0 = false (Gt)
  |    +-- 0 < 0 = false (Lt)
  |    +-- else: 0
  +-- a + b + c
  |    +-- 7 + 10 = 17
  |    +-- 17 + 0 = 17
-> 17
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled to native
> machine code via LLVM's optimization and code generation pipeline. This path produces
> ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@_ori_my_abs: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
@_ori_my_max: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
@_ori_my_sign: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
@_ori_main: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
Nounwind analysis: 2 passes (fixed-point), all 4 functions marked nounwind
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '02-branching'
source_filename = "02-branching"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on negation\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) #0 {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb2

bb1:                                              ; preds = %bb0
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %neg.ovf_panic, label %neg.ok

bb2:                                              ; preds = %bb0
  br label %bb3

bb3:                                              ; preds = %neg.ok, %bb2
  %v7 = phi i64 [ %0, %bb2 ], [ %neg.val, %neg.ok ]
  ret i64 %v7

neg.ok:                                           ; preds = %bb1
  br label %bb3

neg.ovf_panic:                                    ; preds = %bb1
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}

; Function Attrs: nounwind uwtable
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) #0 {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb1, label %bb2

bb1:                                              ; preds = %bb0
  br label %bb3

bb2:                                              ; preds = %bb0
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3

bb3:                                              ; preds = %bb2, %bb1
  %v11 = phi i64 [ %sel, %bb2 ], [ 1, %bb1 ]
  ret i64 %v11
}

; Function Attrs: nounwind uwtable
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_my_abs(i64 -7)
  %call1 = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %call2 = call fastcc i64 @_ori_my_sign(i64 0)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  %add.val4 = extractvalue { i64, i1 } %add3, 0
  %add.ovf5 = extractvalue { i64, i1 } %add3, 1
  br i1 %add.ovf5, label %add.ovf_panic7, label %add.ok6

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable

add.ok6:                                          ; preds = %add.ok
  ret i64 %add.val4

add.ovf_panic7:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64) #1

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #1

; Function Attrs: nounwind
define i32 @main() #3 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind uwtable }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { cold noreturn }
attributes #3 = { nounwind }
```

#### Disassembly

```asm
000000000001b100 <_ori_my_abs>:                    ; 80 bytes
   1b100:  sub    $0x18,%rsp              ; stack frame (24 bytes)
   1b104:  mov    %rdi,0x10(%rsp)         ; spill param (O0)
   1b109:  cmp    $0x0,%rdi               ; n < 0?
   1b10d:  jge    .else                   ; branch to else (n >= 0)
   ; --- then: negate ---
   1b10f:  mov    0x10(%rsp),%rcx         ; reload n (O0)
   1b114:  xor    %eax,%eax               ; 0
   1b116:  sub    %rcx,%rax               ; 0 - n = -n
   1b119:  mov    %rax,0x8(%rsp)          ; spill result (O0)
   1b11e:  seto   %al                     ; overflow check (negation)
   1b121:  jo     .neg_panic              ; branch if overflow
   1b123:  jmp    .merge                  ; jump to phi merge
   ; --- else: identity ---
   1b125:  mov    0x10(%rsp),%rax         ; reload n (O0)
   1b12a:  mov    %rax,(%rsp)             ; store to phi slot (O0)
   1b12e:  jmp    .ret                    ; jump over (O0 redundant)
   ; --- return ---
   1b130:  mov    (%rsp),%rax             ; load phi result (O0)
   1b134:  add    $0x18,%rsp              ; restore stack
   1b138:  ret
   ; --- neg ok path ---
   1b139:  mov    0x8(%rsp),%rax          ; reload neg result (O0)
   1b13e:  mov    %rax,(%rsp)             ; store to phi slot (O0)
   1b142:  jmp    .ret                    ; jump to return
   ; --- neg overflow panic ---
   1b144:  lea    ovf.msg(%rip),%rdi
   1b14b:  call   ori_panic_cstr

000000000001b150 <_ori_my_max>:                    ; 11 bytes
   1b150:  mov    %rsi,%rax               ; rax = b
   1b153:  cmp    %rax,%rdi               ; a > b?
   1b156:  cmovg  %rdi,%rax               ; if a > b, rax = a
   1b15a:  ret

000000000001b160 <_ori_my_sign>:                   ; 56 bytes
   1b160:  mov    %rdi,-0x8(%rsp)         ; spill param (O0)
   1b165:  cmp    $0x0,%rdi               ; n > 0?
   1b169:  jle    .else                   ; branch to else (n <= 0)
   1b16b:  mov    $0x1,%eax               ; result = 1
   1b170:  mov    %rax,-0x10(%rsp)        ; store to phi slot (O0)
   1b175:  jmp    .ret                    ; jump to return
   ; --- else: nested if ---
   1b177:  mov    -0x8(%rsp),%rdx         ; reload n (O0)
   1b17c:  xor    %eax,%eax               ; result = 0
   1b17e:  mov    $0xffffffffffffffff,%rcx ; -1
   1b185:  cmp    $0x0,%rdx               ; n < 0?
   1b189:  cmovl  %rcx,%rax               ; if n < 0, result = -1
   1b18d:  mov    %rax,-0x10(%rsp)        ; store to phi slot (O0)
   ; --- return ---
   1b192:  mov    -0x10(%rsp),%rax        ; load phi result (O0)
   1b197:  ret

000000000001b1a0 <_ori_main>:                      ; 138 bytes
   1b1a0:  sub    $0x28,%rsp              ; stack frame (40 bytes)
   1b1a4:  mov    $0xfffffffffffffff9,%rdi ; arg n = -7
   1b1ab:  call   _ori_my_abs             ; a = my_abs(-7) -> 7
   1b1b0:  mov    %rax,0x10(%rsp)         ; spill a (O0)
   1b1b5:  mov    $0x3,%edi               ; arg a = 3
   1b1ba:  mov    $0xa,%esi               ; arg b = 10
   1b1bf:  call   _ori_my_max             ; b = my_max(3, 10) -> 10
   1b1c4:  mov    %rax,0x8(%rsp)          ; spill b (O0)
   1b1c9:  xor    %eax,%eax               ; arg n = 0
   1b1cb:  mov    %eax,%edi
   1b1cd:  call   _ori_my_sign            ; c = my_sign(0) -> 0
   1b1d2:  mov    0x8(%rsp),%rcx          ; reload b (O0)
   1b1d7:  mov    %rax,%rdx              ; save c
   1b1da:  mov    0x10(%rsp),%rax         ; reload a (O0)
   1b1df:  mov    %rdx,0x18(%rsp)         ; spill c (O0)
   1b1e4:  add    %rcx,%rax              ; a + b (overflow check)
   1b1e7:  mov    %rax,0x20(%rsp)         ; spill sum (O0)
   1b1ec:  seto   %al                     ; overflow flag
   1b1ef:  jo     .add_panic              ; branch if overflow
   1b1f1:  mov    0x18(%rsp),%rcx         ; reload c (O0)
   1b1f6:  mov    0x20(%rsp),%rax         ; reload a+b (O0)
   1b1fb:  add    %rcx,%rax              ; (a+b) + c (overflow check)
   1b1fe:  mov    %rax,(%rsp)             ; spill result (O0)
   1b202:  seto   %al                     ; overflow flag
   1b205:  jo     .add_panic2             ; branch if overflow
   1b207:  jmp    .epilogue               ; jump to return
   ; --- overflow panic 1 ---
   1b209:  lea    ovf.msg.1(%rip),%rdi
   1b210:  call   ori_panic_cstr
   ; --- epilogue ---
   1b215:  mov    (%rsp),%rax             ; reload result (O0)
   1b219:  add    $0x28,%rsp              ; restore stack
   1b21d:  ret
   ; --- overflow panic 2 ---
   1b21e:  lea    ovf.msg.1(%rip),%rdi
   1b225:  call   ori_panic_cstr

000000000001b230 <main>:                           ; 8 bytes
   1b230:  push   %rax
   1b231:  call   _ori_main
   1b236:  pop    %rcx
   1b237:  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual (IR) | Ideal (IR) | Ratio | Verdict |
|---|----------|-------------|------------|-------|---------|
| 1 | @my_abs  | 12          | 11         | 1.09x | NEAR-OPTIMAL |
| 2 | @my_max  | 3           | 3          | 1.00x | OPTIMAL |
| 3 | @my_sign | 8           | 7          | 1.14x | NEAR-OPTIMAL |
| 4 | @main    | 16          | 16         | 1.00x | OPTIMAL |
| 5 | main wrapper | 3      | 3          | 1.00x | OPTIMAL |

**@my_abs (12 instructions)**: The function has 1 unjustified instruction. The `bb2` block (`br label %bb3`) is an unconditional branch from the else path into the phi merge block. Ideally, `bb2` would not exist and `bb0` would branch directly to `bb3` for the else case. The phi node, overflow-checked negation, and panic path are all justified. [LOW-1]

**@my_max (3 instructions)**: OPTIMAL. The compiler correctly recognized this as a simple select pattern (`icmp sgt` + `select` + `ret`). No branches, no phi nodes -- the best possible codegen for a two-way conditional returning one of two existing values. This is excellent: the compiler lowered `if a > b then a else b` directly to a branchless `select`.

**@my_sign (8 instructions)**: The function has 1 unjustified instruction. The `bb1` block (`br label %bb3`) is an empty unconditional branch -- the "then 1" path jumps to the merge block instead of being folded. The inner `if n < 0 then -1 else 0` is correctly lowered to a branchless `select`. [LOW-2]

**@main (16 instructions)**: OPTIMAL. Three function calls, two overflow-checked additions (4 instructions each: call + 2 extractvalue + br), one return, and two panic paths (2 x (call + unreachable)). Let bindings are eliminated to direct SSA -- no alloca/store/load chains.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Correct -- this program uses only `int` scalars (i64), which are value types requiring no reference counting. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | noundef | noreturn | cold | Notes |
|----------|--------|----------|---------|---------|----------|------|-------|
| @my_abs  | YES    | YES      | YES     | YES (param + ret) | N/A | N/A |       |
| @my_max  | YES    | YES      | YES     | YES (params + ret) | N/A | N/A |       |
| @my_sign | YES    | YES      | YES     | YES (param + ret) | N/A | N/A |       |
| @main    | NO (C) | YES      | YES     | YES (ret) | N/A  | N/A  | C conv for entry point -- correct |
| main wrapper | NO (C) | YES | NO      | N/A     | N/A      | N/A  | Missing uwtable [LOW-3] |
| ori_panic_cstr | N/A | N/A | N/A     | N/A     | YES      | YES  | Both noreturn and cold present |

**All 4 user functions have `fastcc` or correct C convention**: Internal functions (`@my_abs`, `@my_max`, `@my_sign`) use `fastcc`. The entry point `@_ori_main` uses C convention for compatibility with the `main()` wrapper. All 4 are marked `nounwind` (fixed-point analysis, 2 passes) and `uwtable`. The `noundef` attribute is correctly applied to all integer parameters and return values.

**`ori_panic_cstr` has both `cold` and `noreturn`**: Correct. Matches Journey 1.

**Attribute compliance**: 23 applicable attributes checked. 22 of 23 correct (95.7%) -- only `uwtable` on the main wrapper is missing. This matches Journey 1's pattern.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 6      | 2           | 1                 | 1         | [LOW-1] [LOW-2] |
| @my_max  | 1      | 0           | 0                 | 0         | Branchless -- OPTIMAL |
| @my_sign | 4      | 1           | 1                 | 1         | [LOW-2] |
| @main    | 5      | 0           | 0                 | 0         | Clean layout |
| main wrapper | 1  | 0           | 0                 | 0         | OPTIMAL |

**@my_abs block structure**: 6 blocks with 2 empty blocks and 1 redundant branch. The `bb2` block (else path) contains only `br label %bb3` -- a bridge block that could be eliminated. Similarly, `neg.ok` contains only `br label %bb3`. Both could be merged directly into the predecessor/successor. The phi node in `bb3` is justified (merging the else value with the negated value). [LOW-1] [LOW-2]

**@my_max block structure**: OPTIMAL. Single block with `icmp` + `select` + `ret`. The compiler recognized this as a select-eligible pattern and emitted branchless code. This is the best possible lowering for a simple two-way conditional.

**@my_sign block structure**: 4 blocks with 1 empty block (`bb1` contains only `br label %bb3`). The inner `if/else` is correctly lowered to a branchless `select` in `bb2`. The outer `if` requires a branch because the then-path and else-path have different computational structure. The phi node in `bb3` is justified. [LOW-2]

**@main block structure**: 5 blocks with zero redundant branches. The three function calls and two overflow-checked additions are laid out linearly on the happy path. Panic blocks are placed at the end. Clean layout.

### 5. Overflow Checking

**Status**: PASS

| Operation | Intrinsic | Checked | Correct | Panic Message |
|-----------|-----------|---------|---------|---------------|
| `-n` (negation) | `llvm.ssub.with.overflow.i64(0, n)` | YES | YES | "integer overflow on negation" |
| `a + b` (first add) | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |
| `(a+b) + c` (second add) | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |

Negation is correctly implemented as `0 - n` using `ssub.with.overflow` -- this catches the edge case where `n = INT_MIN` (-9223372036854775808) which has no valid negation in two's complement. Both additions use `sadd.with.overflow` with appropriate panic messages. Each panic message is a distinct global constant string.

The comparison operators (`<`, `>`) in `@my_abs`, `@my_max`, and `@my_sign` correctly do NOT have overflow checking -- integer comparison cannot overflow.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (6,556,736 bytes, debug) |
| .text section | 869 KiB (890,057 bytes) |
| .rodata section | 133 KiB (136,657 bytes) |
| .debug_info | 1.56 MiB (1,639,950 bytes) |
| .debug_str | 1.72 MiB (1,804,139 bytes) |
| .eh_frame | 109 KiB (112,008 bytes) |
| User code (@my_abs) | 80 bytes |
| User code (@my_max) | 11 bytes |
| User code (@my_sign) | 56 bytes |
| User code (@main) | 138 bytes |
| User code (main wrapper) | 8 bytes |
| User code total | 293 bytes |
| User code % of .text | 0.033% |
| Runtime % of binary | ~99.97% |

The binary is essentially the same size as the previous run (6,556,736 vs 6,554,664 bytes -- 2,072 bytes larger, attributable to minor runtime/debug info changes on the AIMS branch). The user code total is identical at 293 bytes for 4 user functions. `@my_max` remains particularly compact at 11 bytes (4 instructions: `mov` + `cmp` + `cmovg` + `ret`) thanks to branchless codegen.

#### Disassembly: @my_abs

```asm
_ori_my_abs:                     ; 80 bytes, 18 instructions
  sub    $0x18,%rsp              ; stack frame
  mov    %rdi,0x10(%rsp)         ; spill param (O0)
  cmp    $0x0,%rdi               ; n < 0?
  jge    .else                   ; branch to else
  mov    0x10(%rsp),%rcx         ; reload n (O0)
  xor    %eax,%eax               ; 0
  sub    %rcx,%rax               ; 0 - n = -n
  mov    %rax,0x8(%rsp)          ; spill (O0)
  seto   %al                     ; overflow check
  jo     .panic                  ; branch if overflow
  jmp    .merge                  ; to phi merge
  ; --- else ---
  mov    0x10(%rsp),%rax         ; reload n (O0)
  mov    %rax,(%rsp)             ; to phi slot (O0)
  jmp    .ret                    ; to return (O0 redundant)
  ; --- return ---
  mov    (%rsp),%rax             ; load phi result (O0)
  add    $0x18,%rsp              ; restore stack
  ret
  ; --- neg ok ---
  mov    0x8(%rsp),%rax          ; reload (O0)
  mov    %rax,(%rsp)             ; to phi slot (O0)
  jmp    .ret                    ; to return
  ; --- panic ---
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr
```

#### Disassembly: @my_max

```asm
_ori_my_max:                     ; 11 bytes, 4 instructions
  mov    %rsi,%rax               ; rax = b
  cmp    %rax,%rdi               ; a > b?
  cmovg  %rdi,%rax               ; if a > b, rax = a
  ret                            ; return max(a, b)
```

#### Disassembly: @my_sign

```asm
_ori_my_sign:                    ; 56 bytes, 13 instructions
  mov    %rdi,-0x8(%rsp)         ; spill param (O0)
  cmp    $0x0,%rdi               ; n > 0?
  jle    .else                   ; branch to else
  mov    $0x1,%eax               ; result = 1
  mov    %rax,-0x10(%rsp)        ; to phi slot (O0)
  jmp    .ret                    ; to return
  ; --- else ---
  mov    -0x8(%rsp),%rdx         ; reload n (O0)
  xor    %eax,%eax               ; result = 0
  mov    $0xffffffffffffffff,%rcx ; -1
  cmp    $0x0,%rdx               ; n < 0?
  cmovl  %rcx,%rax               ; if n < 0, result = -1
  mov    %rax,-0x10(%rsp)        ; to phi slot (O0)
  ; --- return ---
  mov    -0x10(%rsp),%rax        ; load phi result (O0)
  ret
```

#### Disassembly: @main

```asm
_ori_main:                       ; 138 bytes, 27 instructions
  sub    $0x28,%rsp              ; stack frame (40 bytes)
  mov    $0xfffffffffffffff9,%rdi ; n = -7
  call   _ori_my_abs             ; a = 7
  mov    %rax,0x10(%rsp)         ; spill a (O0)
  mov    $0x3,%edi               ; a = 3
  mov    $0xa,%esi               ; b = 10
  call   _ori_my_max             ; b = 10
  mov    %rax,0x8(%rsp)          ; spill b (O0)
  xor    %eax,%eax               ; n = 0
  mov    %eax,%edi
  call   _ori_my_sign            ; c = 0
  mov    0x8(%rsp),%rcx          ; reload b (O0)
  mov    %rax,%rdx               ; save c
  mov    0x10(%rsp),%rax         ; reload a (O0)
  mov    %rdx,0x18(%rsp)         ; spill c (O0)
  add    %rcx,%rax               ; a + b (overflow check)
  mov    %rax,0x20(%rsp)         ; spill sum (O0)
  seto   %al                     ; overflow flag
  jo     .add_panic              ; branch if overflow
  mov    0x18(%rsp),%rcx         ; reload c (O0)
  mov    0x20(%rsp),%rax         ; reload a+b (O0)
  add    %rcx,%rax               ; (a+b) + c (overflow check)
  mov    %rax,(%rsp)             ; spill result (O0)
  seto   %al                     ; overflow flag
  jo     .add_panic2             ; branch if overflow
  jmp    .epilogue               ; jump to return
  ; --- overflow panic 1 ---
  lea    ovf.msg.1(%rip),%rdi
  call   ori_panic_cstr
  ; --- epilogue ---
  mov    (%rsp),%rax             ; reload result (O0)
  add    $0x28,%rsp              ; restore stack
  ret
  ; --- overflow panic 2 ---
  lea    ovf.msg.1(%rip),%rdi
  call   ori_panic_cstr
```

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (11 instructions)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %n) #0 {
entry:
  %lt = icmp slt i64 %n, 0
  br i1 %lt, label %then, label %merge

then:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %n)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %panic, label %merge

merge:
  %result = phi i64 [ %n, %entry ], [ %neg.val, %then ]
  ret i64 %result

panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (12 instructions)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) #0 {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb2

bb1:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %neg.ovf_panic, label %neg.ok

bb2:                           ; REDUNDANT: only contains br to bb3
  br label %bb3

bb3:
  %v7 = phi i64 [ %0, %bb2 ], [ %neg.val, %neg.ok ]
  ret i64 %v7

neg.ok:                        ; REDUNDANT: only contains br to bb3
  br label %bb3

neg.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +1 instruction (2 empty bridge blocks `bb2` and `neg.ok`, but the net effect is 1 extra instruction because the ideal IR merges entry's else branch directly into `merge` and the then's overflow-ok branch directly into `merge`).

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc noundef i64 @_ori_my_max(i64 noundef %a, i64 noundef %b) #0 {
entry:
  %gt = icmp sgt i64 %a, %b
  %sel = select i1 %gt, i64 %a, i64 %b
  ret i64 %sel
}
```

```llvm
; ACTUAL (3 instructions)
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}
```

**Delta**: 0 instructions. **OPTIMAL.** The compiler correctly lowered the simple if/then/else to a branchless `select`.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %n) #0 {
entry:
  %gt = icmp sgt i64 %n, 0
  br i1 %gt, label %merge, label %else

else:
  %lt = icmp slt i64 %n, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %merge

merge:
  %result = phi i64 [ 1, %entry ], [ %sel, %else ]
  ret i64 %result
}
```

```llvm
; ACTUAL (8 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) #0 {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb1, label %bb2

bb1:                           ; REDUNDANT: only contains br to bb3
  br label %bb3

bb2:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3

bb3:
  %v11 = phi i64 [ %sel, %bb2 ], [ 1, %bb1 ]
  ret i64 %v11
}
```

**Delta**: +1 instruction. The `bb1` block is an empty bridge -- the then-path could branch directly from `bb0` to `bb3` with the constant `1` phi value. The inner `if/else` is correctly lowered to a branchless `select` (matching the ideal). [LOW-2]

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions)
define noundef i64 @_ori_main() #0 {
entry:
  %a = call fastcc i64 @_ori_my_abs(i64 -7)
  %b = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %c = call fastcc i64 @_ori_my_sign(i64 0)
  %add1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %add1.v = extractvalue { i64, i1 } %add1, 0
  %add1.o = extractvalue { i64, i1 } %add1, 1
  br i1 %add1.o, label %panic1, label %ok1
ok1:
  %add2 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add1.v, i64 %c)
  %add2.v = extractvalue { i64, i1 } %add2, 0
  %add2.o = extractvalue { i64, i1 } %add2, 1
  br i1 %add2.o, label %panic2, label %ok2
ok2:
  ret i64 %add2.v
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: 0 instructions. **OPTIMAL.** The actual IR matches the ideal exactly. Let bindings eliminated, constants inlined, calls and overflow-checked additions laid out linearly.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 11    | 12     | +1    | NO (bridge blocks) | NEAR-OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 8      | +1    | NO (bridge block)  | NEAR-OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |
| main wrapper | 3 | 3      | +0    | N/A       | OPTIMAL |
| **Total** | **40** | **42** | **+2** | | |

### 8. Branching: Select vs Branch Lowering

The compiler uses two strategies for `if/then/else`:

| Pattern | Lowering | Example |
|---------|----------|---------|
| Both arms are simple values | `select` (branchless) | `@my_max`: `if a > b then a else b` |
| Inner else of nested if with simple values | `select` (branchless) | `@my_sign`: `if n < 0 then -1 else 0` |
| One arm has side effects (overflow check) | `br` + phi (branching) | `@my_abs`: `if n < 0 then -n else n` |
| Outer if with asymmetric arms | `br` + phi (branching) | `@my_sign`: `if n > 0 then 1 else (...)` |

This is intelligent lowering. The compiler detects when both arms of a conditional are simple SSA values and uses `select` instead of branching. The `select` instruction avoids branch prediction overhead and reduces block count. The `@my_max` function is the clearest win: 3 instructions total, 11 bytes of native code, fully branchless.

The `@my_sign` function demonstrates mixed strategy: the outer `if` uses branching (because the else arm has more complex structure), while the inner `if` uses `select` (because both arms are simple constants). This is the correct optimization choice.

### 9. Branching: Negation Overflow Safety

The negation in `@my_abs` (`-n`) is implemented as `0 - n` using `llvm.ssub.with.overflow.i64`. This is safety-critical because:

- For `n = INT_MIN` (-9223372036854775808), `-n` would overflow to `INT_MIN` in unchecked arithmetic
- The `ssub.with.overflow` correctly detects this case and panics with "integer overflow on negation"
- The panic message is specific to negation (not reusing the addition message)

This matches the Ori spec: "overflow panics." Comparison operators (`<`, `>`) correctly skip overflow checking since integer comparison cannot overflow.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW      | Control Flow | Empty bridge blocks in @my_abs (bb2 and neg.ok) | CONFIRMED | J2 |
| 2 | LOW      | Control Flow | Empty bridge block in @my_sign (bb1) | CONFIRMED | J2 |
| 3 | LOW      | Attributes | Missing `uwtable` on main wrapper | CONFIRMED | J1 |
| 4 | NOTE     | Branching | Branchless `select` for @my_max -- OPTIMAL codegen | CONFIRMED | J2 |
| 5 | NOTE     | Branching | Mixed select/branch strategy in @my_sign -- correct | CONFIRMED | J2 |
| 6 | NOTE     | Attributes | `nounwind`, `uwtable`, `noundef` on all user functions | CONFIRMED | J1 |
| 7 | NOTE     | Instruction Purity | Let bindings eliminated to direct SSA | CONFIRMED | J1 |

### LOW-1: Empty bridge blocks in @my_abs

**Location**: `@_ori_my_abs`, blocks `bb2` and `neg.ok` -- each contains only `br label %bb3`
**Impact**: 2 extra LLVM IR instructions (unconditional branches). At the native level, LLVM's register allocator generates spill/reload through a stack slot for the phi merge, adding ~4 native instructions per bridge block. In optimized builds (`-O1`+), LLVM would merge these blocks.
**Fix**: When lowering `if/then/else` where one or both arms are simple values, emit the branch directly to the merge block instead of creating an intermediate block. When the overflow-checked negation succeeds, branch directly to the merge instead of through `neg.ok`.
**First seen**: Journey 2
**Status**: CONFIRMED -- identical to previous run on AIMS branch.
**Found in**: Control Flow & Block Layout (Category 4), Optimal IR Comparison (Category 7)

### LOW-2: Empty bridge block in @my_sign

**Location**: `@_ori_my_sign`, block `bb1` -- contains only `br label %bb3`
**Impact**: 1 extra LLVM IR instruction. At the native level, LLVM generates a `jmp` through the bridge block. Minor overhead.
**Fix**: When the then-branch of an `if/then/else` is a simple constant (like `1`), branch directly from the conditional to the merge block with the constant as a phi value.
**First seen**: Journey 2
**Status**: CONFIRMED -- identical to previous run on AIMS branch.
**Found in**: Control Flow & Block Layout (Category 4), Optimal IR Comparison (Category 7)

### LOW-3: Missing `uwtable` on main wrapper

**Location**: `define i32 @main() #3` where `#3 = { nounwind }` -- missing `uwtable`
**Impact**: Without `uwtable`, LLVM may not generate a proper `.eh_frame` unwind table entry for the C entry point wrapper. Practical impact minimal since the function is trivial (3 instructions).
**Fix**: Add `uwtable` to the main wrapper's attribute group in `compiler/ori_llvm/src/codegen/function_compiler/entry_point.rs`.
**First seen**: Journey 1
**Status**: CONFIRMED -- still present on AIMS branch.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-4: Branchless `select` for @my_max

**Location**: `@_ori_my_max` -- `icmp sgt` + `select` + `ret`
**Impact**: Positive. The compiler recognized that `if a > b then a else b` can be lowered to a branchless `select` instruction, producing OPTIMAL codegen (3 IR instructions, 11 bytes native, 4 native instructions including `cmovg`). This avoids branch prediction overhead entirely.
**Found in**: Branching: Select vs Branch Lowering (Category 8)

### NOTE-5: Mixed select/branch strategy in @my_sign

**Location**: `@_ori_my_sign` -- outer `if` uses branches, inner `if` uses `select`
**Impact**: Positive. The compiler correctly uses different lowering strategies for different conditional patterns: branching for the outer `if` (where arms have different computational structure) and branchless `select` for the inner `if` (where both arms are simple constants). This is the optimal mixed strategy.
**Found in**: Branching: Select vs Branch Lowering (Category 8)

### NOTE-6: nounwind, uwtable, noundef on all user functions

**Location**: All 4 user functions
**Impact**: Positive. Same quality as Journey 1. The fixed-point nounwind analysis correctly determines all functions are non-unwinding (overflow panics use `noreturn` + `unreachable`). `noundef` on all `int` parameters and returns is correct.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-7: Let bindings eliminated to direct SSA

**Location**: `@_ori_main` -- all 3 `let` bindings eliminated
**Impact**: Positive. Same quality as Journey 1. Constants `-7`, `3`, `10`, `0` are inlined directly as call arguments. Call results flow directly into subsequent operations as SSA values.
**Found in**: Instruction Purity (Category 1)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.05x avg ratio (max 1.14x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 9/10 | 95.7% compliance |
| Control Flow | 10% | 7/10 | 5 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.2 / 10**

## Verdict

Journey 2's branching codegen on the AIMS branch is identical to the previous run. The compiler correctly uses branchless `select` for simple two-way conditionals (`@my_max`: 3 instructions, OPTIMAL) and mixed select/branch strategies for nested conditionals (`@my_sign`). The main overhead comes from empty bridge blocks in phi-merge patterns -- `@my_abs` has 2 and `@my_sign` has 1 -- accounting for the only 2 unjustified instructions. Overflow checking on negation is correct and safety-critical. ARC is irrelevant for scalar-only code. The AIMS unified lattice introduces no regressions for this journey.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED (+ negation via ssub) |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind analysis | J1 | J2 | CONFIRMED |
| noundef on params/returns | J1 | J2 | CONFIRMED |
| Missing uwtable on main wrapper | J1 | J2 | CONFIRMED |
| Let binding elimination | J1 | J2 | CONFIRMED |
| Branchless select lowering | -- | J2 | CONFIRMED |
| Phi-node merge blocks | -- | J2 | CONFIRMED |

Journey 2 re-run on the AIMS branch (`experiment/aims`) produces byte-identical LLVM IR compared to the previous run (2026-03-07). All metrics are unchanged: instruction ratio 1.05x, 0 ARC violations, 95.7% attribute compliance, 5 control flow defects (3 empty blocks + 2 redundant branches), 2 unjustified instructions. The AIMS unified lattice has zero impact on scalar-only branching codegen. Score: 9.2/10 (unchanged).
