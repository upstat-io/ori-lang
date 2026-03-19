---
journey: 2
slug: branching
theme: "I am a branch"
date: 2026-03-19
status: PASS
expected: 17
eval_result: 17
aot_result: 17
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of conditionals and comparison operators"
learning_objectives:
  - "See how if/then/else compiles to LLVM branch and phi instructions"
  - "Understand the difference between branch and select lowering strategies"
  - "Observe overflow checking on negation as well as addition"
  - "Learn how nested conditionals compile to chained basic blocks"
features:
  - branching
  - comparison
  - function_calls
  - multiple_functions
feature_description: "If/then/else conditionals with comparison operators and multiple function calls"
score: 10.0
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 10
  control_flow: 10
  ir_quality: 10
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.00
  instruction_ratio_max: 1.00
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 20
  attr_correct: 20
  attr_has_wrong: false
  cf_defects: 0
  cf_incorrect: false
  ir_unjustified: 0
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
    relationship: "Same overflow checking pattern, same attribute quality"
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

**Tokens**: 142 | **Keywords**: 12 | **Identifiers**: 24 | **Errors**: 0

The source file is 593 bytes. The lexer produces 142 tokens with zero errors. Keywords include `if` (x4), `then` (x4), `else` (x3), `let` (x3), and `@` function markers (x4). Identifiers cover function names, parameter names, type names, and local bindings. Compared to Journey 1's 77 tokens, the increase comes from conditional keywords and the nested if/else in `@my_sign`.

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen Arrow
Ident(int) Eq If Ident(n) Lt Int(0) Then Minus Ident(n) Else Ident(n) Semi

Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma Ident(b)
Colon Ident(int) RParen Arrow Ident(int) Eq If Ident(a) Gt Ident(b)
Then Ident(a) Else Ident(b) Semi

Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen Arrow Ident(int) Eq
If Ident(n) Gt Int(0) Then Int(1)
Else LParen If Ident(n) Lt Int(0) Then Minus Int(1) Else Int(0) RParen Semi

Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus Int(7) RParen Semi
Let Ident(b) Eq Ident(my_max) LParen Ident(a) Colon Int(3) Comma Ident(b) Colon Int(10) RParen Semi
Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon Int(0) RParen Semi
Ident(a) Plus Ident(b) Plus Ident(c) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.
> Conditional expressions are parsed as `if condition then consequent else alternate`.

**Nodes**: 40 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

The parser produces 40 expression nodes across 4 function declarations. The `@my_sign` function has a nested `if` expression (depth 5) -- the inner `if n < 0 then -1 else 0` is wrapped in parentheses, parsed as the `else` branch of the outer `if n > 0 then 1`. The `-n` in `@my_abs` and `-7` in `@main` are parsed as unary negation.

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
        +-- Let a = Call(@my_abs)
        |    +-- n: Unary(-)
        |         +-- Lit(7)
        +-- Let b = Call(@my_max)
        |    +-- a: Lit(3)
        |    +-- b: Lit(10)
        +-- Let c = Call(@my_sign)
        |    +-- n: Lit(0)
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

**Constraints**: 18 | **Types inferred**: 8 | **Unifications**: 14 | **Errors**: 0

All types resolve to `int`. The 8 inferred bindings are: `a`, `b`, `c` in `@main`, plus the intermediate results of the comparison operators and arithmetic in each function. The type checker verifies that `Lt<int, int> -> bool`, `Gt<int, int> -> bool`, `Neg<int> -> int`, and `Add<int, int> -> int` are all valid. The `if/then/else` branches unify to `int` in all three helper functions.

<details>
<summary>Inferred types</summary>

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n
//                           ^ bool (Lt<int, int>)
//                                   ^ int (Neg<int>)
//                                          ^ int
//                        ^ int (both branches unified)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int>)
//                                              ^ int   ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1               // int
    else (if n < 0 then -1 else 0) // int (nested branches unified)

@main () -> int = {
    let a: int = my_abs(n: -7)     // inferred from @my_abs return
    let b: int = my_max(a: 3, b: 10) // inferred from @my_max return
    let c: int = my_sign(n: 0)    // inferred from @my_sign return
    a + b + c  // -> int (Add<int, int> -> int, twice)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form -- a flat
> sequence of operations suitable for backend consumption. It desugars syntactic sugar,
> lowers complex expressions, and resolves named arguments to positional order.

**Transforms**: 4 | **Desugared**: 0 | **Errors**: 0

The canonicalizer produces 43 canon nodes from 40 AST nodes. Named arguments are resolved to positional order. Each `if/then/else` expression becomes a `CanIf(condition, then_branch, else_branch)` triple. The nested if in `@my_sign` becomes two nested CanIf nodes. Six constants are registered (integer literals 0, 1, -1, -7, 3, 10).

<details>
<summary>Key transformations</summary>

```text
- 43 canon nodes from 40 AST nodes (let bindings create extra pattern nodes)
- 4 roots: @my_abs, @my_max, @my_sign, @main
- 6 constants: int literals 0, 1, -1, -7, 3, 10
- 0 decision trees (if/then/else is direct, not pattern matching)
- Named arguments (n:, a:, b:) resolved to positional order
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and inserts
> reference counting operations. It performs borrow inference to minimize RC overhead --
> parameters that are only read can be borrowed rather than owned. On the AIMS branch,
> the unified lattice subsumes the previous multi-pass approach.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

This program uses only `int` scalars (i64), which are value types stored directly in registers. No heap allocation occurs, so no reference counting is needed. The AIMS unified lattice correctly identifies all values as scalar.

<details>
<summary>ARC annotations</summary>

```text
@my_abs: no heap values -- pure scalar arithmetic (int param, int return)
@my_max: no heap values -- pure scalar arithmetic (int params, int return)
@my_sign: no heap values -- pure scalar arithmetic (int param, int return)
@main: no heap values -- all let bindings hold int scalars
Total RC ops: 0 (optimal for scalar-only program)
AIMS lattice: all values classified as scalar -- no RC analysis needed
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without compilation.
> It serves as the reference implementation for correctness testing -- if eval and AOT
> disagree, the bug is in LLVM codegen, not the interpreter.

**Result**: 17 | **Status**: PASS

The eval trace shows the execution flow: `@main` calls `my_abs(n: -7)` which evaluates `if -7 < 0` (true) then `-(-7) = 7`. Then `my_max(a: 3, b: 10)` evaluates `if 3 > 10` (false) else `10`. Then `my_sign(n: 0)` evaluates `if 0 > 0` (false) then `if 0 < 0` (false) else `0`. Final result: `7 + 10 + 0 = 17`.

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-- let a = @my_abs(n: -7)
  |    +-- if -7 < 0 -> true
  |    +-- -(-7) = 7
  +-- let b = @my_max(a: 3, b: 10)
  |    +-- if 3 > 10 -> false
  |    +-- else: 10
  +-- let c = @my_sign(n: 0)
  |    +-- if 0 > 0 -> false
  |    +-- else: if 0 < 0 -> false
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
Nounwind analysis: 2 passes (fixed-point), 4 functions marked nounwind
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
  br i1 %lt, label %bb1, label %bb3

bb1:                                              ; preds = %bb0
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %neg.ovf_panic, label %bb3

bb3:                                              ; preds = %bb1, %bb0
  %v712 = phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]
  ret i64 %v712

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
  br i1 %gt, label %bb3, label %bb2

bb2:                                              ; preds = %bb0
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3

bb3:                                              ; preds = %bb0, %bb2
  %v111 = phi i64 [ %sel, %bb2 ], [ 1, %bb0 ]
  ret i64 %v111
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
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}

; Function Attrs: nounwind
declare i32 @ori_check_leaks() #4

attributes #0 = { nounwind memory(none) uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
attributes #4 = { nounwind }
```

#### Disassembly

```asm
000000000001b100 <_ori_my_abs>:                 ; 65 bytes
   1b100:  sub    $0x18,%rsp              ; stack frame
   1b104:  mov    %rdi,0x8(%rsp)          ; spill n (O0)
   1b109:  cmp    $0x0,%rdi               ; n < 0?
   1b10d:  mov    %rdi,0x10(%rsp)         ; spill result slot (O0)
   1b112:  jge    bb3                     ; n >= 0: skip negation
   1b114:  mov    0x8(%rsp),%rcx          ; reload n (O0)
   1b119:  xor    %eax,%eax              ; 0
   1b11b:  sub    %rcx,%rax              ; 0 - n = -n
   1b11e:  seto   %cl                    ; overflow check
   1b121:  test   $0x1,%cl               ; test overflow flag
   1b124:  mov    %rax,0x10(%rsp)        ; store negated value
   1b129:  jne    panic                  ; panic if overflow
   ; --- merge ---
   1b12b:  mov    0x10(%rsp),%rax        ; reload result
   1b130:  add    $0x18,%rsp             ; restore stack
   1b134:  ret
   ; --- overflow path ---
   1b135:  lea    ovf.msg(%rip),%rdi
   1b13c:  call   ori_panic_cstr

000000000001b150 <_ori_my_max>:                 ; 11 bytes
   1b150:  mov    %rsi,%rax              ; result = b
   1b153:  cmp    %rax,%rdi              ; a > b?
   1b156:  cmovg  %rdi,%rax             ; if a > b: result = a
   1b15a:  ret

000000000001b160 <_ori_my_sign>:                ; 54 bytes
   1b160:  mov    %rdi,-0x10(%rsp)       ; spill n (O0)
   1b165:  mov    $0x1,%eax             ; result = 1
   1b16a:  cmp    $0x0,%rdi             ; n > 0?
   1b16e:  mov    %rax,-0x8(%rsp)       ; spill result (O0)
   1b173:  jg     bb3                   ; if n > 0: return 1
   1b175:  mov    -0x10(%rsp),%rdx      ; reload n (O0)
   1b17a:  xor    %eax,%eax            ; result = 0
   1b17c:  mov    $0xffffffffffffffff,%rcx  ; -1
   1b183:  cmp    $0x0,%rdx            ; n < 0?
   1b187:  cmovl  %rcx,%rax            ; if n < 0: result = -1
   1b18b:  mov    %rax,-0x8(%rsp)      ; store result
   ; --- merge ---
   1b190:  mov    -0x8(%rsp),%rax      ; reload result
   1b195:  ret

000000000001b1a0 <_ori_main>:                   ; 134 bytes
   1b1a0:  sub    $0x28,%rsp             ; stack frame (40 bytes)
   1b1a4:  mov    $0xfffffffffffffff9,%rdi  ; arg n = -7
   1b1ab:  call   _ori_my_abs            ; a = 7
   1b1b0:  mov    %rax,0x10(%rsp)       ; spill a (O0)
   1b1b5:  mov    $0x3,%edi             ; arg a = 3
   1b1ba:  mov    $0xa,%esi             ; arg b = 10
   1b1bf:  call   _ori_my_max           ; b = 10
   1b1c4:  mov    %rax,0x8(%rsp)       ; spill b (O0)
   1b1c9:  xor    %eax,%eax            ; arg n = 0
   1b1cb:  mov    %eax,%edi
   1b1cd:  call   _ori_my_sign          ; c = 0
   1b1d2:  mov    0x8(%rsp),%rcx       ; reload b
   1b1d7:  mov    %rax,%rdx            ; c
   1b1da:  mov    0x10(%rsp),%rax      ; reload a
   1b1df:  mov    %rdx,0x18(%rsp)      ; spill c (O0)
   1b1e4:  add    %rcx,%rax            ; a + b (overflow checked)
   1b1e7:  mov    %rax,0x20(%rsp)      ; spill (O0)
   1b1ec:  seto   %al                  ; overflow check
   1b1ef:  jo     add_panic1           ; panic if overflow
   1b1f1:  mov    0x18(%rsp),%rcx      ; reload c
   1b1f6:  mov    0x20(%rsp),%rax      ; reload (a+b)
   1b1fb:  add    %rcx,%rax            ; (a+b) + c (overflow checked)
   1b1fe:  mov    %rax,(%rsp)          ; spill (O0)
   1b202:  seto   %al                  ; overflow check
   1b205:  jo     add_panic2           ; panic if overflow
   1b207:  jmp    epilogue
   ; --- (panic + epilogue) ---
   1b209:  lea    ovf.msg.1(%rip),%rdi
   1b210:  call   ori_panic_cstr
   1b215:  mov    (%rsp),%rax
   1b219:  add    $0x28,%rsp
   1b21d:  ret
   1b21e:  lea    ovf.msg.1(%rip),%rdi
   1b225:  call   ori_panic_cstr

000000000001b230 <main>:                        ; 29 bytes
   1b230:  push   %rax                   ; align stack
   1b231:  call   _ori_main              ; call Ori main
   1b236:  mov    %eax,0x4(%rsp)         ; save exit code
   1b23a:  call   ori_check_leaks        ; RC leak detection
   1b23f:  mov    %eax,%ecx              ; leak result -> ecx
   1b241:  mov    0x4(%rsp),%eax         ; reload exit code
   1b245:  cmp    $0x0,%ecx              ; check if leaks detected
   1b248:  cmovne %ecx,%eax             ; use leak code if nonzero
   1b24b:  pop    %rcx                   ; restore stack
   1b24c:  ret                           ; return final exit code
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual (IR) | Ideal (IR) | Ratio | Verdict |
|---|----------|-------------|------------|-------|---------|
| 1 | @my_abs  | 10          | 10         | 1.00x | OPTIMAL |
| 2 | @my_max  | 3           | 3          | 1.00x | OPTIMAL |
| 3 | @my_sign | 7           | 7          | 1.00x | OPTIMAL |
| 4 | @main    | 16          | 16         | 1.00x | OPTIMAL |

**@my_abs (10 instructions)**: `icmp slt` for the condition (1), `br` to branch (1), overflow-checked negation via `ssub.with.overflow(0, n)` (1 call + 2 extractvalue = 3), `br` for overflow check (1), `phi` to merge the result from both branches (1), `ret` (1), and the cold panic path (call + unreachable = 2). Every instruction is justified -- negation of `INT_MIN` overflows, requiring the check. **OPTIMAL.**

**@my_max (3 instructions)**: `icmp sgt` for the comparison (1), `select` for the conditional result (1), `ret` (1). The compiler recognized that `@my_max` has no side-effecting branches and lowered the `if/then/else` to a branchless `select`. This is superior to a branch+phi pattern for simple value selection. **OPTIMAL.**

**@my_sign (7 instructions)**: Outer `icmp sgt` (1), `br` (1) for the outer if. Inner `icmp slt` (1), `select` (1) for the inner if (lowered branchlessly). Unconditional `br` (1) to the merge block. `phi` to merge outer then (1) and inner result (1), `ret` (1). The compiler uses a hybrid strategy: branch for the outer if (which has different branch targets), select for the inner if (which is a simple value choice). **OPTIMAL.**

**@main (16 instructions)**: 3 function calls (3), 2 overflow-checked additions (2 x (call + 2 extractvalue + br) = 8), ret (1), and 2 panic paths (2 x (call + unreachable) = 4). **OPTIMAL.**

**Let binding elimination**: All three `let` bindings (`a`, `b`, `c`) in `@main` are correctly eliminated -- no `alloca`/`store`/`load` chains. Function call results flow directly as SSA registers into the overflow-checked additions. This is `-O1` quality codegen in a debug build, continuing the pattern from Journey 1.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Correct -- this program uses only `int` scalars (i64), which are value types requiring no reference counting. No `ori_rc_inc`, `ori_rc_dec`, or any RC-related calls present in the IR or disassembly. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | memory | noundef | noreturn | cold | Notes |
|----------|--------|----------|---------|--------|---------|----------|------|-------|
| @my_abs  | YES    | YES      | YES     | none   | YES (param + ret) | N/A | N/A | [NOTE-1] |
| @my_max  | YES    | YES      | YES     | none   | YES (params + ret) | N/A | N/A | [NOTE-1] |
| @my_sign | YES    | YES      | YES     | none   | YES (param + ret) | N/A | N/A | [NOTE-1] |
| @main (ori) | NO (C) | YES | YES     | N/A    | YES (ret) | N/A  | N/A  | C conv for entry point -- correct |
| main wrapper | NO (C) | YES | YES    | N/A    | YES (ret) | N/A  | N/A  | All attributes present |
| ori_panic_cstr | N/A | N/A | N/A    | N/A    | N/A     | YES      | YES  | Both noreturn and cold present [NOTE-2] |
| ori_check_leaks | N/A | YES | N/A   | N/A    | N/A     | N/A      | N/A  | Leak detection runtime function |

All three helper functions (`@my_abs`, `@my_max`, `@my_sign`) correctly have `fastcc`, `nounwind`, `memory(none)`, `uwtable`, and `noundef`. The `memory(none)` attribute is correct because these are pure arithmetic functions with no observable side effects -- the overflow panic path is modeled as a noreturn function, which does not violate memory purity. The `@_ori_main` correctly uses C calling convention since it is called from the C `main()` wrapper.

**Attribute compliance**: 20 applicable attributes checked (per extract-metrics.py). 20 of 20 correct. 100% compliance.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 4      | 0           | 0                 | 1         | [NOTE-3] |
| @my_max  | 1      | 0           | 0                 | 0         | [NOTE-4] |
| @my_sign | 3      | 0           | 0                 | 1         | [NOTE-5] |
| @main    | 5      | 0           | 0                 | 0         | |
| main wrapper | 1  | 0           | 0                 | 0         | |

**@my_abs block layout**: 4 blocks -- `bb0` (entry with comparison), `bb1` (negation with overflow check), `bb3` (phi merge + return), `neg.ovf_panic` (cold panic). The phi merges the original parameter (from `bb0`) with the negated value (from `bb1`). Panic block placed at the end. **Optimal layout.**

**@my_max block layout**: Single block -- `icmp sgt` + `select` + `ret`. The compiler lowered the entire if/then/else to a branchless `select` instruction, eliminating all branches. **Optimal -- best possible layout.**

**@my_sign block layout**: 3 blocks -- `bb0` (outer comparison + branch), `bb2` (inner comparison as `select` + unconditional branch to merge), `bb3` (phi merge + return). The unconditional `br` from `bb2` to `bb3` is structurally necessary for the phi node. **Optimal layout.**

**@main block layout**: 5 blocks with zero redundant branches. Three function calls reside in `bb0`, followed by two overflow-checked additions each with their own `ok` and `panic` blocks. **Optimal layout.**

### 5. Overflow Checking

**Status**: PASS

| Operation | Intrinsic | Checked | Correct | Panic Message |
|-----------|-----------|---------|---------|---------------|
| `-n` in @my_abs | `llvm.ssub.with.overflow.i64(0, n)` | YES | YES | "integer overflow on negation" |
| `a + b` in @main | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |
| `(a+b) + c` in @main | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |

All arithmetic operations use the correct LLVM signed overflow intrinsics. The negation `-n` is correctly implemented as `0 - n` using `ssub.with.overflow` rather than a raw `sub` instruction -- this catches the edge case where `n = INT_MIN` and `-n` would overflow. Each operation has a descriptive panic message. Note that `@my_max` has no arithmetic (just comparison and selection) and `@my_sign` has no arithmetic (just comparison and constant selection), so they correctly have no overflow checks.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (6,559,072 bytes, debug) |
| .text section | 869 KiB (890,393 bytes) |
| .rodata section | 134 KiB (136,705 bytes) |
| .debug_info | 1.56 MiB (1,640,996 bytes) |
| User code (@my_abs) | 65 bytes |
| User code (@my_max) | 11 bytes |
| User code (@my_sign) | 54 bytes |
| User code (@main) | 134 bytes |
| User code (main wrapper) | 29 bytes |
| User code total | 293 bytes |
| User code % of .text | 0.033% |
| Runtime % of binary | ~99.97% |

The binary is the same size as Journey 1 (6.25 MiB debug, static linking of `ori_rt`). User code grew from 153 bytes (J1) to 293 bytes (J2) due to 2 additional user functions and conditional branching logic. `@my_max` is remarkably compact at 11 bytes (4 native instructions) thanks to the `select` -> `cmovg` lowering that avoids any branching.

#### Disassembly: @my_abs

```asm
_ori_my_abs:                     ; 65 bytes, 17 instructions
  sub    $0x18,%rsp              ; stack frame
  mov    %rdi,0x8(%rsp)          ; spill n (O0 regalloc)
  cmp    $0x0,%rdi               ; n < 0?
  mov    %rdi,0x10(%rsp)         ; spill result slot (O0)
  jge    bb3                     ; n >= 0: skip negation
  mov    0x8(%rsp),%rcx          ; reload n (O0)
  xor    %eax,%eax               ; 0
  sub    %rcx,%rax               ; 0 - n = -n
  seto   %cl                     ; overflow check
  test   $0x1,%cl                ; test overflow flag
  mov    %rax,0x10(%rsp)         ; store negated value
  jne    panic                   ; panic if overflow
  ; --- merge ---
  mov    0x10(%rsp),%rax         ; reload result
  add    $0x18,%rsp              ; restore stack
  ret
  ; --- overflow path ---
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr
```

#### Disassembly: @my_max

```asm
_ori_my_max:                     ; 11 bytes, 4 instructions
  mov    %rsi,%rax               ; result = b
  cmp    %rax,%rdi               ; a > b?
  cmovg  %rdi,%rax               ; if a > b: result = a
  ret
```

#### Disassembly: @my_sign

```asm
_ori_my_sign:                    ; 54 bytes, 14 instructions
  mov    %rdi,-0x10(%rsp)        ; spill n (O0)
  mov    $0x1,%eax               ; result = 1 (optimistic)
  cmp    $0x0,%rdi               ; n > 0?
  mov    %rax,-0x8(%rsp)         ; spill result (O0)
  jg     bb3                     ; if n > 0: return 1
  mov    -0x10(%rsp),%rdx        ; reload n (O0)
  xor    %eax,%eax               ; result = 0
  mov    $0xffffffffffffffff,%rcx  ; -1
  cmp    $0x0,%rdx               ; n < 0?
  cmovl  %rcx,%rax               ; if n < 0: result = -1
  mov    %rax,-0x8(%rsp)         ; store result
  ; --- merge ---
  mov    -0x8(%rsp),%rax         ; reload result
  ret
```

#### Disassembly: @main

```asm
_ori_main:                       ; 134 bytes, 27 instructions
  sub    $0x28,%rsp              ; stack frame (40 bytes)
  mov    $0xfffffffffffffff9,%rdi  ; arg n = -7
  call   _ori_my_abs             ; a = 7
  mov    %rax,0x10(%rsp)         ; spill a (O0)
  mov    $0x3,%edi               ; arg a = 3
  mov    $0xa,%esi               ; arg b = 10
  call   _ori_my_max             ; b = 10
  mov    %rax,0x8(%rsp)          ; spill b (O0)
  xor    %eax,%eax               ; arg n = 0
  mov    %eax,%edi
  call   _ori_my_sign            ; c = 0
  mov    0x8(%rsp),%rcx          ; reload b
  mov    %rax,%rdx               ; c
  mov    0x10(%rsp),%rax         ; reload a
  mov    %rdx,0x18(%rsp)         ; spill c (O0)
  add    %rcx,%rax               ; a + b (overflow checked)
  mov    %rax,0x20(%rsp)         ; spill (O0)
  seto   %al                     ; overflow check
  jo     add_panic1              ; panic if overflow
  mov    0x18(%rsp),%rcx         ; reload c
  mov    0x20(%rsp),%rax         ; reload (a+b)
  add    %rcx,%rax               ; (a+b) + c (overflow checked)
  mov    %rax,(%rsp)             ; spill (O0)
  seto   %al                     ; overflow check
  jo     add_panic2              ; panic if overflow
  jmp    epilogue                ; jump over panic blocks
  ; --- (panic + epilogue omitted for brevity) ---
```

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (10 instructions -- negation overflow check mandatory)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %n) #0 {
entry:
  %lt = icmp slt i64 %n, 0
  br i1 %lt, label %neg, label %merge
neg:
  %r = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %n)
  %val = extractvalue { i64, i1 } %r, 0
  %ovf = extractvalue { i64, i1 } %r, 1
  br i1 %ovf, label %panic, label %merge
merge:
  %result = phi i64 [ %n, %entry ], [ %val, %neg ]
  ret i64 %result
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (10 instructions)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) #0 {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb3
bb1:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %neg.ovf_panic, label %bb3
bb3:
  %v712 = phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]
  ret i64 %v712
neg.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: 0 instructions. **OPTIMAL.** The actual IR matches the ideal IR exactly. The phi node correctly merges the original value (from `bb0` when `n >= 0`) with the negated value (from `bb1` when `n < 0`). The negation uses `ssub.with.overflow(0, n)` to catch `INT_MIN` overflow.

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions -- branchless select)
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

**Delta**: 0 instructions. **OPTIMAL.** The compiler recognized that both branches of the `if/then/else` return a scalar without side effects and lowered to a single `select` instruction. This compiles to a `cmovg` on x86_64 -- zero branches, zero branch mispredictions.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions -- hybrid branch + select)
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
; ACTUAL (7 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) #0 {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb3, label %bb2
bb2:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3
bb3:
  %v111 = phi i64 [ %sel, %bb2 ], [ 1, %bb0 ]
  ret i64 %v111
}
```

**Delta**: 0 instructions. **OPTIMAL.** The compiler uses a hybrid strategy: the outer `if` uses a branch (since the `then` path returns a constant 1, making the `else` block entirely skippable), while the inner `if` uses a branchless `select` (since it chooses between two constant values -1 and 0). This is the ideal lowering.

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions)
define noundef i64 @_ori_main() #1 {
entry:
  %a = call fastcc i64 @_ori_my_abs(i64 -7)
  %b = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %c = call fastcc i64 @_ori_my_sign(i64 0)
  %r1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %r1.v = extractvalue { i64, i1 } %r1, 0
  %r1.o = extractvalue { i64, i1 } %r1, 1
  br i1 %r1.o, label %panic1, label %ok1
ok1:
  %r2 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %r1.v, i64 %c)
  %r2.v = extractvalue { i64, i1 } %r2, 0
  %r2.o = extractvalue { i64, i1 } %r2, 1
  br i1 %r2.o, label %panic2, label %ok2
ok2:
  ret i64 %r2.v
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: 0 instructions. **OPTIMAL.** The actual IR matches the ideal IR exactly in structure and instruction count. All three function calls are in the entry block, followed by two overflow-checked additions. Each addition has its own panic path with the shared overflow message.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 10    | 10     | +0    | N/A       | OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |
| **Total** | **36** | **36** | **+0** | | |

### 8. Branching: Lowering Strategy Selection

The compiler demonstrates intelligent if/then/else lowering with three distinct strategies:

| Function | Strategy | Why |
|----------|----------|-----|
| @my_abs | Branch + phi | The `then` branch has a side-effecting operation (overflow-checked negation), so it must be guarded by a branch. The phi merges the result from both paths. |
| @my_max | Branchless select | Both branches return a simple scalar value (parameter) with no side effects. A `select` (compiled to `cmovg`) is strictly better -- no branch misprediction penalty. |
| @my_sign | Hybrid: branch + select + phi | The outer if uses a branch (to skip the inner comparison entirely when `n > 0`). The inner if uses a `select` (choosing between constant -1 and 0). The phi merges the outer `then` (1) with the inner result. |

This is excellent codegen. The compiler does not blindly use one strategy for all if/then/else -- it selects the best lowering per function based on the branch contents. The `@my_max` `select` lowering is particularly notable: it produces 3 IR instructions and 4 native instructions (11 bytes total), which is as compact as hand-written assembly.

### 9. Branching: Negation via Subtraction

The compiler implements unary negation `-n` as `0 - n` using `llvm.ssub.with.overflow.i64(0, n)`. This is correct and important:

- **Correctness**: `-INT_MIN` (i.e., `-(-9223372036854775808)`) overflows because `INT_MAX = 9223372036854775807`. Using `ssub.with.overflow` catches this.
- **Alternative**: LLVM has a `sub nsw i64 0, %n` instruction, but the `nsw` flag would make the overflow behavior undefined rather than trapping. The Ori spec requires overflow to panic, not be UB.
- **Message**: The panic message "integer overflow on negation" is operation-specific, not a generic "arithmetic overflow".

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE     | Branching | Intelligent lowering strategy selection (branch vs select vs hybrid) | NEW | J2 |
| 2 | NOTE     | Attributes | `memory(none)` on all three pure helper functions | CONFIRMED | J1 |
| 3 | NOTE     | Attributes | `noreturn` + `cold` present on `ori_panic_cstr` | CONFIRMED | J1 |
| 4 | NOTE     | Overflow | Negation uses `ssub.with.overflow(0, n)` with operation-specific message | NEW | J2 |
| 5 | NOTE     | Branching | @my_max compiles to 3 IR instructions / 11 bytes native (branchless) | NEW | J2 |

### NOTE-1: `memory(none)` on pure helper functions

**Location**: `@_ori_my_abs`, `@_ori_my_max`, `@_ori_my_sign` -- all have `#0 = { nounwind memory(none) uwtable }`
**Impact**: Positive. All three helper functions are correctly identified as pure -- they have no observable memory effects. This enables LLVM to reorder, deduplicate, or eliminate calls to these functions. Continues the pattern from Journey 1 where `@_ori_add` was also marked `memory(none)`.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-2: `noreturn` + `cold` on `ori_panic_cstr`

**Location**: `declare void @ori_panic_cstr(ptr) #3` where `#3 = { cold noreturn }`
**Impact**: Positive. Both attributes present, enabling dead code elimination and branch prediction heuristics.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Phi node in @my_abs merge block

**Location**: `bb3: %v712 = phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]`
**Impact**: Positive. The phi node correctly merges the original parameter (when `n >= 0`, skip negation) with the negated value (when `n < 0`). This avoids an unnecessary `alloca`/`store`/`load` pattern that a naive codegen might produce.
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Branchless select for @my_max

**Location**: `%sel = select i1 %gt, i64 %0, i64 %1` in `@_ori_my_max`
**Impact**: Positive. The compiler lowered `if a > b then a else b` to a single `select` instruction, which compiles to `cmovg` on x86_64. This eliminates branch misprediction entirely. Only 3 IR instructions, 11 bytes native.
**Found in**: Branching: Lowering Strategy Selection (Category 8)

### NOTE-5: Hybrid strategy in @my_sign

**Location**: `@_ori_my_sign` uses branch for outer if, select for inner if
**Impact**: Positive. The compiler correctly uses a branch for the outer condition (allowing the inner comparison to be entirely skipped when `n > 0`) and a branchless select for the inner condition (choosing between -1 and 0). This is the optimal hybrid strategy.
**Found in**: Branching: Lowering Strategy Selection (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 10/10 | 100.0% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 10.0 / 10**

## Verdict

Journey 2's branching codegen achieves a perfect score. The compiler demonstrates three distinct and intelligent lowering strategies for if/then/else: branch+phi for side-effecting branches (`@my_abs`), branchless select for simple value selection (`@my_max`), and a hybrid branch+select+phi for nested conditionals (`@my_sign`). All four user functions match the hand-written ideal IR instruction-for-instruction. Negation is correctly implemented as overflow-checked subtraction from zero. All attributes are correctly applied across the board.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED -- extends to negation via `ssub.with.overflow` |
| `fastcc` on internal functions | J1 | J2 | CONFIRMED -- all 3 helpers use `fastcc` |
| `memory(none)` on pure functions | J1 | J2 | CONFIRMED -- extends to 3 more functions |
| `nounwind` via fixed-point analysis | J1 | J2 | CONFIRMED -- 4 functions analyzed in 2 passes |
| `noundef` on params and returns | J1 | J2 | CONFIRMED |
| `cold noreturn` on panic | J1 | J2 | CONFIRMED |
| Let binding elimination to SSA | J1 | J2 | CONFIRMED -- 3 let bindings in @main eliminated |
| Instruction-optimal codegen | J1 | J2 | CONFIRMED -- 0 unjustified instructions across all functions |

Journey 2 extends Journey 1's patterns to conditional logic and reveals that the compiler's codegen quality is not limited to straight-line arithmetic -- it produces optimal IR for branching patterns as well. The `select` lowering for `@my_max` is a standout: it produces the most compact user function seen so far at 11 bytes native.
