---
journey: 2
slug: branching
theme: "I am a branch"
date: 2026-03-16
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
score: 9.3
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 10
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
  attr_applicable: 20
  attr_correct: 20
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
    relationship: "Both test scalar-only codegen; same attribute patterns confirmed"
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

**Tokens**: 142 | **Keywords**: 12 | **Identifiers**: 21 | **Errors**: 0

The source file is 593 bytes. The lexer produces 142 tokens with zero errors. Keywords include `if` (x3), `then` (x3), `else` (x3), `let` (x3), and `@` function markers (x4). The `if/then/else` keywords are first-class tokens rather than identifier-based -- the lexer distinguishes them from user identifiers at the tokenization stage.

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen Arrow
Ident(int) Eq If Ident(n) Lt Int(0) Then Minus Ident(n) Else Ident(n) Semi

Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma Ident(b) Colon
Ident(int) RParen Arrow Ident(int) Eq If Ident(a) Gt Ident(b) Then
Ident(a) Else Ident(b) Semi

Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen Arrow
Ident(int) Eq If Ident(n) Gt Int(0) Then Int(1) Else LParen If
Ident(n) Lt Int(0) Then Minus Int(1) Else Int(0) RParen Semi

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
> Conditional expressions become `If(condition, then_branch, else_branch)` nodes.

**Nodes**: 40 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

The parser produces 40 expression nodes across 4 function declarations. The nested `if/then/else` in `@my_sign` produces an `If` node with another `If` node as its else branch -- this is the deepest nesting at depth 5. Named arguments are preserved in the AST.

<details>
<summary>AST (simplified)</summary>

```text
Module
+-- FnDecl @my_abs
|  +-- Params: (n: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(<) [n, 0]
|       +-- Then: Unary(-) [n]
|       +-- Else: Ident(n)
+-- FnDecl @my_max
|  +-- Params: (a: int, b: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(>) [a, b]
|       +-- Then: Ident(a)
|       +-- Else: Ident(b)
+-- FnDecl @my_sign
|  +-- Params: (n: int)
|  +-- Return: int
|  +-- Body: If
|       +-- Cond: BinOp(>) [n, 0]
|       +-- Then: Lit(1)
|       +-- Else: If
|            +-- Cond: BinOp(<) [n, 0]
|            +-- Then: Unary(-) [Lit(1)]
|            +-- Else: Lit(0)
+-- FnDecl @main
   +-- Return: int
   +-- Body: Block
        +-- Let a = Call(@my_abs) [n: Unary(-) Lit(7)]
        +-- Let b = Call(@my_max) [a: Lit(3), b: Lit(10)]
        +-- Let c = Call(@my_sign) [n: Lit(0)]
        +-- BinOp(+) [BinOp(+) [a, b], c]
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit annotations everywhere.

**Constraints**: 18 | **Types inferred**: 9 | **Unifications**: 16 | **Errors**: 0

All types resolve to `int`. The comparison operators (`<`, `>`) resolve via the `Comparable` trait to produce `bool` conditions. The unary negation `-n` resolves via `Neg<int> -> int`. The type checker confirms that all three user functions return `int`, and the three calls in `@main` with named arguments match the declared parameter types.

<details>
<summary>Inferred types</summary>

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n
//                           ^ bool (Comparable<int, int> -> bool)
//                                   ^ int (Neg<int> -> int)
//                                          ^ int (identity)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool
//                                            ^ int  ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1
//     ^ bool      ^ int
    else (if n < 0 then -1 else 0)
//          ^ bool       ^ int   ^ int

@main () -> int = {
    let a: int = my_abs(n: -7)    // inferred from @my_abs return type
    let b: int = my_max(a: 3, b: 10)  // inferred from @my_max return type
    let c: int = my_sign(n: 0)    // inferred from @my_sign return type
    a + b + c  // -> int (Add<int, int> -> int, twice)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form -- a flat
> sequence of operations suitable for backend consumption. It desugars syntactic sugar,
> lowers complex expressions, and resolves named arguments to positional order.

**Transforms**: 4 | **Desugared**: 0 | **Errors**: 0

The canonicalizer produces 43 canon nodes from 40 AST nodes. The `if/then/else` expressions become `CanIf(condition, then_branch, else_branch)` canon nodes. Named arguments are resolved to positional order. The nested if in `@my_sign` is preserved as-is -- no desugaring needed since `if/then/else` is already a primitive expression.

<details>
<summary>Key transformations</summary>

```text
- 43 canon nodes from 40 AST nodes (let bindings create extra pattern nodes)
- 4 roots: @my_abs, @my_max, @my_sign, @main
- 6 constants: int literals -7, 3, 10, 0, 1, -1
- 0 decision trees (no pattern matching)
- Named arguments resolved to positional order
- If/then/else preserved as CanIf nodes (primitive expression)
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and inserts
> reference counting operations. It performs borrow inference to minimize RC overhead --
> parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

This program uses only `int` scalars (i64), which are value types stored directly in registers. No heap allocation occurs, so no reference counting is needed. The AIMS unified lattice correctly identifies all values as scalar.

<details>
<summary>ARC annotations</summary>

```text
@my_abs: no heap values -- pure scalar comparison + negation
@my_max: no heap values -- pure scalar comparison
@my_sign: no heap values -- pure scalar comparison chain
@main: no heap values -- all let bindings hold int scalars
Total RC ops: 0 (optimal for scalar-only program)
AIMS lattice: all values classified as scalar -- no RC analysis needed
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without compilation.
> It serves as the reference implementation for correctness testing -- if eval and AOT
> disagree, the bug is in LLVM codegen.

**Result**: 17 | **Status**: PASS

The eval trace shows the execution order: `@main` evaluates three calls in sequence. `@my_abs(-7)`: condition `-7 < 0` is true, so `-(-7) = 7`. `@my_max(3, 10)`: condition `3 > 10` is false, so `b = 10`. `@my_sign(0)`: condition `0 > 0` is false, inner condition `0 < 0` is false, so `0`. Final: `7 + 10 + 0 = 17`.

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-- let a = @my_abs(n: -7)
  |    +-- -7 < 0 = true
  |    +-- -(-7) = 7
  +-- let b = @my_max(a: 3, b: 10)
  |    +-- 3 > 10 = false
  |    +-- b = 10
  +-- let c = @my_sign(n: 0)
  |    +-- 0 > 0 = false
  |    +-- 0 < 0 = false
  |    +-- 0
  +-- a + b + c = 7 + 10 + 0 = 17
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
@_ori_my_abs: +0 rc_inc, +0 rc_dec (pure scalar)
@_ori_my_max: +0 rc_inc, +0 rc_dec (pure scalar)
@_ori_my_sign: +0 rc_inc, +0 rc_dec (pure scalar)
@_ori_main: +0 rc_inc, +0 rc_dec (pure scalar)
Nounwind analysis: 2 passes (fixed-point), all 4 functions marked nounwind
Memory analysis: @_ori_my_abs, @_ori_my_max, @_ori_my_sign marked memory(none)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '02-branching'
source_filename = "02-branching"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on negation\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind memory(none) uwtable
; --- @my_abs ---
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

; Function Attrs: nounwind memory(none) uwtable
; --- @my_max ---
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}

; Function Attrs: nounwind memory(none) uwtable
; --- @my_sign ---
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

bb3:                                              ; preds = %bb1, %bb2
  %v11 = phi i64 [ %sel, %bb2 ], [ 1, %bb1 ]
  ret i64 %v11
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
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
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64) #2

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: nounwind uwtable
define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind memory(none) uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
```

#### Disassembly

```asm
000000000001b100 <_ori_my_abs>:                  ; 76 bytes
   1b100:  sub    $0x18,%rsp             ; stack frame
   1b104:  mov    %rdi,0x10(%rsp)        ; spill n
   1b109:  cmp    $0x0,%rdi              ; n < 0?
   1b10d:  jge    1b125                  ; if n >= 0, skip negation
   1b10f:  mov    0x10(%rsp),%rcx        ; reload n
   1b114:  xor    %eax,%eax              ; 0
   1b116:  sub    %rcx,%rax              ; 0 - n = -n
   1b119:  mov    %rax,0x8(%rsp)         ; spill result
   1b11e:  seto   %al                    ; overflow check
   1b121:  jo     1b144                  ; branch to panic on overflow
   1b123:  jmp    1b139                  ; jump to merge (neg.ok -> bb3)
   1b125:  mov    0x10(%rsp),%rax        ; reload n (else branch)
   1b12a:  mov    %rax,(%rsp)            ; spill to merge slot
   1b12e:  jmp    1b130                  ; jump to merge (bb2 -> bb3)
   1b130:  mov    (%rsp),%rax            ; load merged result (phi)
   1b134:  add    $0x18,%rsp             ; restore stack
   1b138:  ret                           ; return
   1b139:  mov    0x8(%rsp),%rax         ; reload neg result
   1b13e:  mov    %rax,(%rsp)            ; store to merge slot
   1b142:  jmp    1b130                  ; jump to merge
   1b144:  lea    ovf.msg(%rip),%rdi     ; overflow panic
   1b14b:  call   ori_panic_cstr

000000000001b150 <_ori_my_max>:                  ; 11 bytes
   1b150:  mov    %rsi,%rax              ; result = b
   1b153:  cmp    %rax,%rdi              ; a > b?
   1b156:  cmovg  %rdi,%rax             ; if a > b, result = a
   1b15a:  ret

000000000001b160 <_ori_my_sign>:                 ; 56 bytes
   1b160:  mov    %rdi,-0x8(%rsp)        ; spill n
   1b165:  cmp    $0x0,%rdi              ; n > 0?
   1b169:  jle    1b177                  ; if n <= 0, else branch
   1b16b:  mov    $0x1,%eax              ; result = 1
   1b170:  mov    %rax,-0x10(%rsp)       ; store to merge slot
   1b175:  jmp    1b192                  ; jump to merge
   1b177:  mov    -0x8(%rsp),%rdx        ; reload n
   1b17c:  xor    %eax,%eax              ; result = 0
   1b17e:  mov    $0xffffffffffffffff,%rcx ; -1
   1b185:  cmp    $0x0,%rdx              ; n < 0?
   1b189:  cmovl  %rcx,%rax             ; if n < 0, result = -1
   1b18d:  mov    %rax,-0x10(%rsp)       ; store to merge slot
   1b192:  mov    -0x10(%rsp),%rax       ; load merged result (phi)
   1b197:  ret

000000000001b1a0 <_ori_main>:                    ; 138 bytes
   1b1a0:  sub    $0x28,%rsp             ; stack frame
   1b1a4:  mov    $0xfffffffffffffff9,%rdi ; n = -7
   1b1ab:  call   1b100 <_ori_my_abs>    ; a = my_abs(-7)
   1b1b0:  mov    %rax,0x10(%rsp)        ; spill a
   1b1b5:  mov    $0x3,%edi              ; first arg = 3
   1b1ba:  mov    $0xa,%esi              ; second arg = 10
   1b1bf:  call   1b150 <_ori_my_max>    ; b = my_max(3, 10)
   1b1c4:  mov    %rax,0x8(%rsp)         ; spill b
   1b1c9:  xor    %eax,%eax
   1b1cb:  mov    %eax,%edi              ; n = 0
   1b1cd:  call   1b160 <_ori_my_sign>   ; c = my_sign(0)
   1b1d2:  mov    0x8(%rsp),%rcx         ; reload b
   1b1d7:  mov    %rax,%rdx              ; save c
   1b1da:  mov    0x10(%rsp),%rax        ; reload a
   1b1df:  mov    %rdx,0x18(%rsp)        ; spill c
   1b1e4:  add    %rcx,%rax              ; a + b (overflow checked)
   1b1e7:  mov    %rax,0x20(%rsp)        ; spill sum
   1b1ec:  seto   %al                    ; overflow check
   1b1ef:  jo     1b209                  ; branch to panic
   1b1f1:  mov    0x18(%rsp),%rcx        ; reload c
   1b1f6:  mov    0x20(%rsp),%rax        ; reload a+b
   1b1fb:  add    %rcx,%rax              ; (a+b) + c (overflow checked)
   1b1fe:  mov    %rax,(%rsp)            ; spill final result
   1b202:  seto   %al                    ; overflow check
   1b205:  jo     1b21e                  ; branch to panic
   1b207:  jmp    1b215                  ; jump to epilogue
   1b209:  lea    ovf.msg.1(%rip),%rdi   ; addition overflow panic
   1b210:  call   ori_panic_cstr
   1b215:  mov    (%rsp),%rax            ; load final result
   1b219:  add    $0x28,%rsp             ; restore stack
   1b21d:  ret                           ; return 17

000000000001b230 <main>:                         ; 8 bytes
   1b230:  push   %rax
   1b231:  call   1b1a0 <_ori_main>
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

**@my_abs (12 instructions, ideal 11)**: The single extra instruction is the unconditional `br label %bb3` in the `bb2` block (the else branch). In the ideal IR, the else branch would fall through directly to the phi node merge block without an intervening empty block. This is a consequence of how the if/then/else codegen emits separate blocks for both branches before creating a merge point. The other 11 instructions are all justified: `icmp` (1), conditional `br` (1), overflow-checked negation via `llvm.ssub.with.overflow` (call + 2 extractvalue + conditional br = 4), the `neg.ok` to merge `br` (1, also extra but counted in the other block), phi merge + ret (2), and panic path (2). The `neg.ok` block's `br label %bb3` is the other unjustified instruction but the tool counts only 1 as unjustified because of how block-level analysis works.

**@my_max (3 instructions)**: Perfect. The `icmp sgt` + `select` + `ret` pattern is exactly optimal. The codegen correctly recognizes that a simple `if a > b then a else b` with no side effects in either branch can be lowered to a branchless `select` instruction. This avoids the overhead of phi nodes and separate blocks entirely. **OPTIMAL.**

**@my_sign (8 instructions, ideal 7)**: The single extra instruction is the `br label %bb3` in `bb1` (the `then` branch for `n > 0`). In the ideal IR, the constant `1` would be placed directly in the phi node without an intervening empty block. The codegen correctly optimizes the inner `if n < 0 then -1 else 0` to `icmp slt` + `select` (branchless), but the outer if/then/else still generates a branch with an empty then-block. **NEAR-OPTIMAL.**

**@main (16 instructions)**: All 16 instructions are justified. Three `call fastcc` instructions for the three helper functions (3), two overflow-checked additions via `llvm.sadd.with.overflow` (2 x (call + 2 extractvalue + br) = 8), the final `ret` (1), and two panic paths (2 x (call + unreachable) = 4). **OPTIMAL.**

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Correct -- this program uses only `int` scalars (i64), which are value types requiring no reference counting. No `ori_rc_inc`, `ori_rc_dec`, or any RC-related calls in the IR. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | memory | noundef | noreturn | cold | Notes |
|----------|--------|----------|---------|--------|---------|----------|------|-------|
| @my_abs  | YES    | YES      | YES     | none   | YES (param + ret) | N/A | N/A | [NOTE-1] |
| @my_max  | YES    | YES      | YES     | none   | YES (params + ret) | N/A | N/A | [NOTE-1] |
| @my_sign | YES    | YES      | YES     | none   | YES (param + ret) | N/A | N/A | [NOTE-1] |
| @main    | NO (C) | YES      | YES     | N/A    | YES (ret) | N/A  | N/A  | C conv for entry point -- correct |
| main wrapper | NO (C) | YES | YES     | N/A    | YES (ret) | N/A  | N/A  | |
| ori_panic_cstr | N/A | N/A | N/A     | N/A    | N/A     | YES      | YES  | Both noreturn and cold present |

**100% attribute compliance (20/20)**. All user functions have `fastcc` (internal calling convention), `nounwind` (no exception unwinding), `uwtable` (unwind table generation), and `noundef` on parameters and returns. The three helper functions additionally have `memory(none)` -- correct, since they are pure functions that neither read from nor write to memory. `@_ori_main` does not have `memory(none)` because it calls other functions -- correct, since the memory attribute on non-leaf functions requires interprocedural analysis and `@_ori_main` calls `ori_panic_cstr` (indirectly via its callees). The main wrapper `@main` now has `noundef` on its `i32` return value (fixed from J1's LOW-1).

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 6      | 2           | 1                 | 1         | [MEDIUM-1] |
| @my_max  | 1      | 0           | 0                 | 0         | Optimal -- select used |
| @my_sign | 4      | 1           | 1                 | 1         | [LOW-2] |
| @main    | 5      | 0           | 0                 | 0         | Optimal layout |
| main     | 1      | 0           | 0                 | 0         | Optimal |

**@my_abs block layout**: 6 blocks with 2 empty blocks and 1 redundant branch. Block `bb2` (else branch) contains only `br label %bb3` -- this is an empty trampoline to the merge point. Block `neg.ok` similarly contains only `br label %bb3`. Ideally, `bb2` would directly predecessor `bb3` without the unconditional branch, and the phi node in `bb3` would reference `bb0` instead of `bb2`. Similarly, `neg.ok` could be eliminated by having `bb1`'s overflow check branch directly to `bb3` on the non-overflow path. The phi node itself is correctly formed with `[ %0, %bb2 ], [ %neg.val, %neg.ok ]`.

**@my_max block layout**: 1 block with zero overhead. The `select` instruction avoids all branching entirely. This is the ideal codegen for a simple conditional with no side effects.

**@my_sign block layout**: 4 blocks with 1 empty block. Block `bb1` (the `n > 0` then branch) contains only `br label %bb3` -- a trampoline to the merge point. The inner `if n < 0` is correctly lowered to `icmp slt` + `select` in `bb2`, avoiding a second level of branching. The phi node in `bb3` correctly merges `[ %sel, %bb2 ], [ 1, %bb1 ]`.

**@main block layout**: 5 blocks with optimal layout. The three calls are in `bb0`, followed by overflow-checked additions. Panic blocks are placed at the end, separated from the happy path.

### 5. Overflow Checking

**Status**: PASS

| Operation | Intrinsic | Checked | Correct | Panic Message |
|-----------|-----------|---------|---------|---------------|
| `-n` (negation) | `llvm.ssub.with.overflow.i64(0, n)` | YES | YES | "integer overflow on negation" |
| `a + b` (first add) | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |
| `(a+b) + c` (second add) | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |

All arithmetic operations are overflow-checked. The negation uses `ssub.with.overflow(0, n)` which correctly detects overflow when `n = INT_MIN` (since `-INT_MIN` overflows). The two additions use `sadd.with.overflow`. The panic messages are operation-specific. The `icmp` comparisons (`slt`, `sgt`) do not need overflow checking since they do not produce arithmetic results.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (6,556,736 bytes, debug) |
| .text section | 869 KiB (890,057 bytes) |
| .rodata section | 133 KiB (136,657 bytes) |
| .debug_info | 1.56 MiB (1,639,950 bytes) |
| User code (@my_abs) | 76 bytes |
| User code (@my_max) | 11 bytes |
| User code (@my_sign) | 56 bytes |
| User code (@main) | 138 bytes |
| User code (main wrapper) | 8 bytes |
| User code total | 289 bytes |
| User code % of .text | 0.032% |
| Runtime % of binary | ~99.97% |

The binary size is essentially the same as Journey 1 -- the runtime dominates. User code grew from 132 bytes (J1, 2 functions) to 289 bytes (J2, 4 functions + wrapper) which is proportional to the function count. `@my_max` at 11 bytes is remarkably compact thanks to the `select` lowering to `cmovg`.

#### Disassembly: @my_abs

```asm
_ori_my_abs:                     ; 76 bytes, 18 instructions
  sub    $0x18,%rsp              ; stack frame
  mov    %rdi,0x10(%rsp)         ; spill n (O0 regalloc)
  cmp    $0x0,%rdi               ; n < 0?
  jge    .else                   ; skip negation if n >= 0
  mov    0x10(%rsp),%rcx         ; reload n (O0)
  xor    %eax,%eax               ; 0
  sub    %rcx,%rax               ; 0 - n = -n
  mov    %rax,0x8(%rsp)          ; spill neg result (O0)
  seto   %al                     ; overflow check
  jo     .panic                  ; branch if overflow
  jmp    .merge_neg              ; jump to merge (from neg.ok)
  ; --- else path ---
  mov    0x10(%rsp),%rax         ; reload n
  mov    %rax,(%rsp)             ; store to merge slot
  jmp    .merge                  ; jump to merge (from bb2)
  ; --- merge ---
  mov    (%rsp),%rax             ; load merged result (phi)
  add    $0x18,%rsp              ; restore stack
  ret
  ; --- neg.ok -> merge ---
  mov    0x8(%rsp),%rax          ; reload neg result
  mov    %rax,(%rsp)             ; store to merge slot
  jmp    .merge
  ; --- overflow panic ---
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr
```

#### Disassembly: @my_max

```asm
_ori_my_max:                     ; 11 bytes, 4 instructions
  mov    %rsi,%rax               ; result = b (default)
  cmp    %rax,%rdi               ; a > b?
  cmovg  %rdi,%rax               ; if a > b, result = a
  ret
```

#### Disassembly: @my_sign

```asm
_ori_my_sign:                    ; 56 bytes, 13 instructions
  mov    %rdi,-0x8(%rsp)         ; spill n (O0)
  cmp    $0x0,%rdi               ; n > 0?
  jle    .else_outer             ; if n <= 0, check inner
  mov    $0x1,%eax               ; result = 1
  mov    %rax,-0x10(%rsp)        ; store to merge slot
  jmp    .merge                  ; jump to merge
  ; --- inner if ---
  mov    -0x8(%rsp),%rdx         ; reload n
  xor    %eax,%eax               ; result = 0 (default)
  mov    $0xffffffffffffffff,%rcx ; -1
  cmp    $0x0,%rdx               ; n < 0?
  cmovl  %rcx,%rax               ; if n < 0, result = -1
  mov    %rax,-0x10(%rsp)        ; store to merge slot
  ; --- merge ---
  mov    -0x10(%rsp),%rax        ; load merged result (phi)
  ret
```

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (11 instructions)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %n) #0 {
entry:
  %lt = icmp slt i64 %n, 0
  br i1 %lt, label %neg, label %merge
neg:
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %n)
  %val = extractvalue { i64, i1 } %sub, 0
  %ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %ovf, label %panic, label %merge
merge:
  %r = phi i64 [ %n, %entry ], [ %val, %neg ]
  ret i64 %r
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
bb2:
  br label %bb3               ; <-- EXTRA: empty trampoline block
bb3:
  %v7 = phi i64 [ %0, %bb2 ], [ %neg.val, %neg.ok ]
  ret i64 %v7
neg.ok:
  br label %bb3               ; <-- routes through trampoline
neg.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +1 instruction (empty `bb2` trampoline block -- unjustified). In the ideal IR, the else branch of the entry block branches directly to the merge block, and the `neg.ok` block also branches directly to merge. The actual IR interposes `bb2` and `neg.ok` as separate trampoline blocks, adding one unconditional branch. The phi node references `bb2` and `neg.ok` instead of `bb0` and `bb1` directly.

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc noundef i64 @_ori_my_max(i64 noundef %a, i64 noundef %b) #0 {
entry:
  %gt = icmp sgt i64 %a, %b
  %r = select i1 %gt, i64 %a, i64 %b
  ret i64 %r
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

**Delta**: 0 instructions. **OPTIMAL.** The codegen correctly recognizes a simple if/then/else with pure scalar values on both branches and lowers it to a branchless `select` instruction.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %n) #0 {
entry:
  %gt = icmp sgt i64 %n, 0
  br i1 %gt, label %merge, label %inner
inner:
  %lt = icmp slt i64 %n, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %merge
merge:
  %r = phi i64 [ 1, %entry ], [ %sel, %inner ]
  ret i64 %r
}
```

```llvm
; ACTUAL (8 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) #0 {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb1, label %bb2
bb1:
  br label %bb3               ; <-- EXTRA: empty trampoline block
bb2:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3
bb3:
  %v11 = phi i64 [ %sel, %bb2 ], [ 1, %bb1 ]
  ret i64 %v11
}
```

**Delta**: +1 instruction (empty `bb1` trampoline block). In the ideal IR, the `entry` block branches directly to `merge` on the true path, carrying the constant `1` into the phi. The actual IR interposes `bb1` as an empty block containing only `br label %bb3`. The inner `if n < 0 then -1 else 0` is correctly lowered to `select` (no extra blocks).

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions)
define noundef i64 @_ori_main() #1 {
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

```llvm
; ACTUAL (16 instructions)
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_my_abs(i64 -7)
  %call1 = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %call2 = call fastcc i64 @_ori_my_sign(i64 0)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok
add.ok:
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  %add.val4 = extractvalue { i64, i1 } %add3, 0
  %add.ovf5 = extractvalue { i64, i1 } %add3, 1
  br i1 %add.ovf5, label %add.ovf_panic7, label %add.ok6
add.ok6:
  ret i64 %add.val4
add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
add.ovf_panic7:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: 0 instructions. **OPTIMAL.** All let bindings eliminated. Constants inlined directly as call arguments. The two additions chain correctly with overflow checking on each.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 11    | 12     | +1    | NO (empty trampoline block) | NEAR-OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 8      | +1    | NO (empty trampoline block) | NEAR-OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |
| **Total** | **37** | **39** | **+2** | | |

### 8. Branching: Select vs Branch Lowering

The codegen demonstrates two distinct lowering strategies for `if/then/else`:

**Strategy 1 -- Select (branchless)**: Used when both branches produce simple values with no side effects. `@my_max` uses `icmp sgt` + `select`, producing a single basic block with zero control flow overhead. The inner branch of `@my_sign` (`if n < 0 then -1 else 0`) also uses `select` since both branches are integer constants. This is OPTIMAL -- equivalent to C's ternary operator lowering.

**Strategy 2 -- Branch + Phi**: Used when a branch has side effects (overflow-checked negation in `@my_abs`) or when the codegen has not yet determined that both branches are simple enough for select. The outer branch of `@my_sign` uses this strategy because one branch produces a constant `1` while the other produces a computed `select` value.

The select optimization is a significant quality indicator. Many compilers at `-O0` would emit branches even for simple cases like `max(a, b)`. Ori's codegen correctly identifies selectability, producing LLVM `select` instructions that lower to x86 `cmov` -- avoiding branch misprediction entirely.

**Selectability criteria** (observed):
- Both branches must be side-effect-free (no function calls, no overflow-checked ops)
- Both branches must produce scalar values
- `@my_abs` cannot use select because the then-branch (`-n`) involves overflow-checked subtraction

### 9. Branching: Phi Node Quality

| Function | Phi Nodes | Sources | Correct | Notes |
|----------|-----------|---------|---------|-------|
| @my_abs  | 1         | 2       | YES     | Merges `[%0, %bb2]` and `[%neg.val, %neg.ok]` |
| @my_sign | 1         | 2       | YES     | Merges `[%sel, %bb2]` and `[1, %bb1]` |

Both phi nodes are well-formed and semantically correct. The values are properly routed from their defining blocks. The phi in `@my_abs` correctly receives the original parameter `%0` from the else path and the negated value from the negation path. The phi in `@my_sign` correctly receives the inner select result from the else path and the constant `1` from the then path.

The only deficiency is that the phi nodes reference intermediate trampoline blocks (`bb2`, `bb1`, `neg.ok`) rather than the logically meaningful blocks where the values are defined. This is a block layout issue (Category 4), not a phi correctness issue.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM   | Control Flow | Empty trampoline blocks in @my_abs (bb2 + neg.ok) | NEW | J2 |
| 2 | LOW      | Control Flow | Empty trampoline block in @my_sign (bb1) | NEW | J2 |
| 3 | NOTE     | Branching | Select optimization for @my_max -- branchless codegen | NEW | J2 |
| 4 | NOTE     | Branching | Inner select in @my_sign -- partial branchless optimization | NEW | J2 |
| 5 | NOTE     | Attributes | 100% attribute compliance -- memory(none) on pure functions | CONFIRMED | J1 |
| 6 | NOTE     | Attributes | noundef now present on main wrapper i32 return | FIXED | J1 |
| 7 | NOTE     | Instruction Purity | Let bindings eliminated to direct SSA in @main | CONFIRMED | J1 |

### MEDIUM-1: Empty trampoline blocks in @my_abs

**Location**: `@_ori_my_abs`, blocks `bb2` (1 instruction: `br label %bb3`) and `neg.ok` (1 instruction: `br label %bb3`)
**Impact**: 2 empty blocks with unconditional branches that could be eliminated. Adds control flow overhead and makes the phi node reference intermediate blocks instead of logically meaningful ones. At the native code level, this produces extra `jmp` instructions in the `-O0` output.
**Fix**: When lowering `if/then/else`, if a branch produces a simple value (no side effects), route the predecessor block directly to the merge block. Alternatively, run a block-merging pass after codegen to eliminate empty trampoline blocks.
**First seen**: Journey 2
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-2: Empty trampoline block in @my_sign

**Location**: `@_ori_my_sign`, block `bb1` (1 instruction: `br label %bb3`)
**Impact**: 1 empty block for the `n > 0` then-branch. The constant `1` is carried through the phi node via this empty block. Could be eliminated by having `bb0`'s conditional branch target `bb3` directly with `1` as the phi value from `bb0`.
**Fix**: Same as MEDIUM-1 -- eliminate empty trampoline blocks in if/then/else lowering.
**First seen**: Journey 2
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-3: Select optimization for @my_max

**Location**: `@_ori_my_max` -- entire function is 3 instructions (`icmp` + `select` + `ret`)
**Impact**: Positive. The codegen correctly identifies that `if a > b then a else b` can be lowered to a branchless `select` instruction. This produces optimal code: 3 LLVM IR instructions lowering to 4 x86 instructions (`mov` + `cmp` + `cmovg` + `ret`), with zero control flow overhead.
**Found in**: Branching: Select vs Branch Lowering (Category 8)

### NOTE-4: Inner select in @my_sign

**Location**: `@_ori_my_sign`, block `bb2` -- inner `if n < 0 then -1 else 0` lowered to `icmp slt` + `select`
**Impact**: Positive. The codegen correctly optimizes the nested conditional's inner branch to branchless code. The outer branch still uses a phi node (because one branch produces a constant while the other produces a computed value from a different block), but the inner branch avoids additional blocks entirely.
**Found in**: Branching: Select vs Branch Lowering (Category 8)

### NOTE-5: 100% attribute compliance

**Location**: All user functions
**Impact**: Positive. All applicable attributes are correctly present: `fastcc`, `nounwind`, `uwtable`, `noundef`, and `memory(none)` on pure functions. The nounwind analysis uses a fixed-point algorithm (2 passes) to determine that all 4 user functions cannot unwind. The memory analysis correctly identifies `@_ori_my_abs`, `@_ori_my_max`, and `@_ori_my_sign` as pure (no memory effects).
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-6: `noundef` now present on main wrapper i32 return

**Location**: `define noundef i32 @main() #1`
**Impact**: Positive. Previously LOW-1 in Journey 1 -- the C entry point wrapper lacked `noundef` on its `i32` return. Now fixed, bringing attribute compliance to 100%.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-7: Let bindings eliminated to direct SSA in @main

**Location**: `@_ori_main` -- all three `let` bindings compiled to SSA registers
**Impact**: Positive. No `alloca`/`store`/`load` chains. Constants `-7`, `3`, `10`, `0` inlined directly as call arguments. Call results flow directly into the addition chain.
**Found in**: Instruction Purity (Category 1)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.05x avg ratio (max 1.14x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 10/10 | 100.0% compliance |
| Control Flow | 10% | 7/10 | 5 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.3 / 10**

## Verdict

Journey 2's branching codegen is strong overall, with a standout result in `@my_max` where the `select` optimization produces branchless, OPTIMAL code in just 3 instructions. The main overhead comes from empty trampoline blocks in `@my_abs` and `@my_sign` -- a systematic pattern where the if/then/else lowering creates intermediate blocks that could be merged away. Attributes are at 100% compliance (improved from J1), with `memory(none)` correctly identifying all three helper functions as pure. ARC is irrelevant for scalar arithmetic -- zero RC operations. The 2 unjustified instructions (empty trampoline blocks) and 5 control flow defects (empty blocks + redundant branches) are the primary areas for improvement.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED (add + negation) |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind analysis | J1 | J2 | CONFIRMED |
| noundef on params | J1 | J2 | CONFIRMED |
| memory(none) on pure functions | J1 | J2 | CONFIRMED |
| Let binding elimination | J1 | J2 | CONFIRMED |
| Missing noundef on main wrapper | J1 | J2 | FIXED |
| Select optimization | J2 | J2 | NEW |
| Empty trampoline blocks | J2 | J2 | NEW |

**New in Journey 2**: The `select` optimization for simple conditionals is a significant codegen quality feature first observed here. The empty trampoline block pattern is a new systematic issue -- both `@my_abs` and `@my_sign` exhibit it, suggesting it is inherent to the if/then/else lowering strategy rather than a one-off case. The attribute compliance improvement from J1 (90.9% to 100%) is confirmed, with `noundef` now present on the main wrapper's `i32` return.
