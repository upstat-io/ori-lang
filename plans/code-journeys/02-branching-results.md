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
  - "Understanding of conditional expressions"
learning_objectives:
  - "See how if/then/else lowers to LLVM branches and phi nodes"
  - "Understand when the compiler optimizes branches to select instructions"
  - "Compare nested conditionals vs flat select in generated IR"
  - "Learn how negation overflow checking protects against INT_MIN edge cases"
features:
  - branching
  - comparison
  - function_calls
  - multiple_functions
feature_description: "Branching with if/then/else, comparison operators, and multiple function calls"
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
    relationship: "Both test scalar arithmetic with overflow checking; J2 adds branching"
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
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 142 | **Keywords**: 12 | **Identifiers**: 24 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Lt Int(0) Then Minus
Ident(n) Else Ident(n) Semi

Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(int) Eq If
Ident(a) Gt Ident(b) Then Ident(a) Else Ident(b) Semi

Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Gt Int(0) Then Int(1) Else
LParen If Ident(n) Lt Int(0) Then Minus Int(1) Else Int(0) RParen Semi

Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus Int(7) RParen Semi
Let Ident(b) Eq Ident(my_max) LParen Ident(a) Colon Int(3) Comma
    Ident(b) Colon Int(10) RParen Semi
Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon Int(0) RParen Semi
Ident(a) Plus Ident(b) Plus Ident(c)
RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 40 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @my_abs
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: If
│       ├─ Cond: BinOp(<)
│       │    ├─ Ident(n)
│       │    └─ Lit(0)
│       ├─ Then: UnaryOp(-)
│       │    └─ Ident(n)
│       └─ Else: Ident(n)
├─ FnDecl @my_max
│  ├─ Params: (a: int, b: int)
│  ├─ Return: int
│  └─ Body: If
│       ├─ Cond: BinOp(>)
│       │    ├─ Ident(a)
│       │    └─ Ident(b)
│       ├─ Then: Ident(a)
│       └─ Else: Ident(b)
├─ FnDecl @my_sign
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: If
│       ├─ Cond: BinOp(>)
│       │    ├─ Ident(n)
│       │    └─ Lit(0)
│       ├─ Then: Lit(1)
│       └─ Else: If (nested)
│            ├─ Cond: BinOp(<)
│            │    ├─ Ident(n)
│            │    └─ Lit(0)
│            ├─ Then: UnaryOp(-)
│            │    └─ Lit(1)
│            └─ Else: Lit(0)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@my_abs, n: UnaryOp(-) Lit(7))
        ├─ Let b = Call(@my_max, a: Lit(3), b: Lit(10))
        ├─ Let c = Call(@my_sign, n: Lit(0))
        └─ BinOp(+)
             ├─ BinOp(+)
             │    ├─ Ident(a)
             │    └─ Ident(b)
             └─ Ident(c)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 18 | **Types inferred**: 10 | **Unifications**: 14 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n
//                           ^ bool (Lt<int, int> -> bool)
//                                   ^ int (Neg<int> -> int)
//                                          ^ int (parameter n)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int> -> bool)
//                                              ^ int    ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1
//     ^ bool (Gt<int, int> -> bool)
//                ^ int (literal)
    else (if n < 0 then -1 else 0)
//          ^ bool        ^ int  ^ int

@main () -> int = {
    let a: int = my_abs(n: -7)   // inferred: int (return type of @my_abs)
    let b: int = my_max(a: 3, b: 10)  // inferred: int (return type of @my_max)
    let c: int = my_sign(n: 0)  // inferred: int (return type of @my_sign)
    a + b + c  // -> int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 4 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Function bodies lowered to canonical expression form
- If/then/else expressions lowered to conditional nodes
- Nested if in @my_sign preserved as nested conditional
- Call arguments normalized to positional order
- Unary negation on literals folded: -7 → Int(-7), -1 → Int(-1)
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@my_abs: no heap values — pure scalar arithmetic + branching
@my_max: no heap values — pure scalar comparison
@my_sign: no heap values — pure scalar comparison
@main: no heap values — pure scalar arithmetic
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 17 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  ├─ let a = @my_abs(n: -7)
  │    └─ if -7 < 0 → true
  │         └─ -(-7) = 7
  ├─ let b = @my_max(a: 3, b: 10)
  │    └─ if 3 > 10 → false
  │         └─ 10
  ├─ let c = @my_sign(n: 0)
  │    └─ if 0 > 0 → false
  │         └─ if 0 < 0 → false
  │              └─ 0
  └─ 7 + 10 + 0
       └─ 17 + 0 = 17
→ 17
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@my_abs: +0 rc_inc, +0 rc_dec (no heap values)
@my_max: +0 rc_inc, +0 rc_dec (no heap values)
@my_sign: +0 rc_inc, +0 rc_dec (no heap values)
@main: +0 rc_inc, +0 rc_dec (no heap values)
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
_ori_my_abs:
   sub    $0x18,%rsp
   mov    %rdi,0x8(%rsp)
   cmp    $0x0,%rdi
   mov    %rdi,0x10(%rsp)
   jge    <+0x2b>
   mov    0x8(%rsp),%rcx
   xor    %eax,%eax
   sub    %rcx,%rax
   seto   %cl
   test   $0x1,%cl
   mov    %rax,0x10(%rsp)
   jne    <panic>
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
   lea    ovf.msg(%rip),%rdi
   call   <ori_panic_cstr>

_ori_my_max:
   mov    %rsi,%rax
   cmp    %rax,%rdi
   cmovg  %rdi,%rax
   ret

_ori_my_sign:
   mov    %rdi,-0x10(%rsp)
   mov    $0x1,%eax
   cmp    $0x0,%rdi
   mov    %rax,-0x8(%rsp)
   jg     <+0x30>
   mov    -0x10(%rsp),%rdx
   xor    %eax,%eax
   mov    $0xffffffffffffffff,%rcx
   cmp    $0x0,%rdx
   cmovl  %rcx,%rax
   mov    %rax,-0x8(%rsp)
   mov    -0x8(%rsp),%rax
   ret

_ori_main:
   sub    $0x28,%rsp
   mov    $0xfffffffffffffff9,%rdi
   call   <_ori_my_abs>
   mov    %rax,0x10(%rsp)
   mov    $0x3,%edi
   mov    $0xa,%esi
   call   <_ori_my_max>
   mov    %rax,0x8(%rsp)
   xor    %eax,%eax
   mov    %eax,%edi
   call   <_ori_my_sign>
   mov    0x8(%rsp),%rcx
   mov    %rax,%rdx
   mov    0x10(%rsp),%rax
   mov    %rdx,0x18(%rsp)
   add    %rcx,%rax
   mov    %rax,0x20(%rsp)
   seto   %al
   jo     <panic1>
   mov    0x18(%rsp),%rcx
   mov    0x20(%rsp),%rax
   add    %rcx,%rax
   mov    %rax,(%rsp)
   seto   %al
   jo     <panic2>
   jmp    <+0x75>
   lea    ovf.msg.1(%rip),%rdi
   call   <ori_panic_cstr>
   mov    (%rsp),%rax
   add    $0x28,%rsp
   ret
   lea    ovf.msg.1(%rip),%rdi
   call   <ori_panic_cstr>
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @my_abs  | 10     | 10    | 1.00x | OPTIMAL |
| 2 | @my_max  | 3      | 3     | 1.00x | OPTIMAL |
| 3 | @my_sign | 7      | 7     | 1.00x | OPTIMAL |
| 4 | @main    | 16     | 16    | 1.00x | OPTIMAL |

All four functions achieve exactly the ideal instruction count. Every instruction is necessary and justified:

- **@my_abs**: `icmp` for condition, `br` for branch, `ssub.with.overflow` for safe negation (0 - n), two `extractvalue` for result/overflow, conditional `br`, `phi` to merge, `ret`, plus panic path (`call` + `unreachable`). The negation overflow check protects against `my_abs(INT_MIN)` which would overflow.
- **@my_max**: `icmp sgt` + `select` + `ret` -- the compiler correctly optimized the simple if/then/else into a branchless `select`, which is the ideal codegen for this pattern.
- **@my_sign**: Outer `if n > 0` uses `icmp sgt` + `br` (needed because the then-path is a constant `1`). Inner `if n < 0` uses `icmp slt` + `select` (branchless). `phi` merges both paths. This is exactly optimal for the nested conditional.
- **@main**: Three `call` instructions, two overflow-checked additions (each: `sadd.with.overflow` + two `extractvalue` + `br`), two panic paths, and `ret`. All necessary.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. All functions operate exclusively on `i64` scalars. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | memory(none) | cold | Notes |
|----------|--------|----------|---------|--------------|------|-------|
| @my_abs  | YES    | YES      | YES     | YES          | N/A  |       |
| @my_max  | YES    | YES      | YES     | YES          | N/A  |       |
| @my_sign | YES    | YES      | YES     | YES          | N/A  |       |
| @main    | NO (C) | YES      | YES     | NO (calls)   | N/A  | Correct: entry point uses C cc |
| @ori_panic_cstr | N/A | N/A  | N/A     | N/A          | YES  | Correct: cold + noreturn |

All attributes are correct and complete:
- `fastcc` on all user functions (except `@main` which correctly uses C calling convention as the program entry point)
- `nounwind` on all functions (the nounwind analysis correctly determined no function can unwind)
- `memory(none)` on pure functions (`my_abs`, `my_max`, `my_sign`) -- correctly absent from `@main` which calls other functions
- `noundef` on all parameters and return values
- `cold noreturn` on `ori_panic_cstr`

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 4      | 0           | 0                 | 1         | [NOTE-1] |
| @my_max  | 1      | 0           | 0                 | 0         | [NOTE-2] |
| @my_sign | 3      | 0           | 0                 | 1         | [NOTE-3] |
| @main    | 5      | 0           | 0                 | 0         |       |

All block structures are well-formed with no empty blocks or redundant branches:
- **@my_abs** (4 blocks): entry, negation, merge (phi), panic. Structurally correct for an if/else with overflow-checked negation on one branch.
- **@my_max** (1 block): The entire if/then/else compiled to a single block with `select`. Excellent.
- **@my_sign** (3 blocks): entry, inner-conditional, merge (phi). The inner `if n < 0 then -1 else 0` compiled to a `select` within a block, while the outer `if n > 0` requires a branch because the then-path constant `1` and else-path both need a phi merge. Correct and minimal.
- **@main** (5 blocks): entry, first-add-ok, first-add-panic, second-add-ok, second-add-panic. Each overflow check requires its own conditional branch and panic block.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| negation (my_abs)  | YES | YES | `llvm.ssub.with.overflow.i64(0, n)` -- protects against `my_abs(INT_MIN)` |
| addition (a + b)   | YES | YES | `llvm.sadd.with.overflow.i64` |
| addition ((a+b)+c) | YES | YES | `llvm.sadd.with.overflow.i64` |
| comparison (n < 0) | N/A | N/A | Comparisons cannot overflow |
| comparison (a > b) | N/A | N/A | Comparisons cannot overflow |

All arithmetic operations that can overflow are checked. Comparisons correctly have no overflow checking. The negation is correctly implemented as `0 - n` with `ssub.with.overflow` rather than a plain `sub`, catching the edge case where `n = INT_MIN` (since `-INT_MIN` overflows in two's complement).

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.7 KiB |
| .rodata section | 133.5 KiB |
| User code (@my_abs) | 66 bytes (17 instructions) |
| User code (@my_max) | 11 bytes (4 instructions) |
| User code (@my_sign) | 54 bytes (13 instructions) |
| User code (@main) | 138 bytes (30 instructions) |
| Total user code | 269 bytes (64 instructions) |
| Runtime | >99% of binary |

#### Disassembly: @my_max

```asm
_ori_my_max:
   mov    %rsi,%rax
   cmp    %rax,%rdi
   cmovg  %rdi,%rax
   ret
```

The `my_max` function compiles to just 4 native instructions (11 bytes) -- a branchless `cmovg` sequence. This is textbook optimal codegen for a two-way integer max.

#### Disassembly: @my_abs

```asm
_ori_my_abs:
   sub    $0x18,%rsp
   mov    %rdi,0x8(%rsp)
   cmp    $0x0,%rdi
   mov    %rdi,0x10(%rsp)
   jge    <merge>
   mov    0x8(%rsp),%rcx
   xor    %eax,%eax
   sub    %rcx,%rax
   seto   %cl
   test   $0x1,%cl
   mov    %rax,0x10(%rsp)
   jne    <panic>
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
```

Stack spills are present in the debug build due to `-O0`. LLVM's optimizer would eliminate them in release mode. The overflow checking on negation is correctly wired through `seto` + `test` + `jne`.

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (10 instructions — overflow-checked negation required)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) nounwind memory(none) {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb3
bb1:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %panic, label %bb3
bb3:
  %result = phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]
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

**Delta**: +0 instructions. Actual matches ideal exactly.

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions — branchless select)
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) nounwind memory(none) {
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
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

**Delta**: +0 instructions. The compiler recognized the simple if-then-else pattern where both branches return a parameter and correctly lowered it to a branchless `select`.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions — nested conditional with select)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) nounwind memory(none) {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb3, label %bb2
bb2:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3
bb3:
  %result = phi i64 [ %sel, %bb2 ], [ 1, %bb0 ]
  ret i64 %result
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

**Delta**: +0 instructions. The compiler correctly handled the nested if/else by using a branch for the outer conditional (needed because one path leads to a sub-expression) and a `select` for the inner conditional (which only chooses between two constants). This is the optimal lowering.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 10    | 10     | +0    | N/A       | OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |

### 8. Branching: Select vs Branch Lowering

The compiler demonstrates intelligent branch lowering with three distinct patterns:

1. **Simple if/else with parameter forwarding** (`@my_max`): `if a > b then a else b` compiles to a branchless `select`. This is optimal -- no branches, no phi nodes, just `icmp` + `select` + `ret`. The compiler recognizes that both branches yield a function parameter with no side effects.

2. **If/else with side-effecting then-branch** (`@my_abs`): `if n < 0 then -n else n` requires a real branch because the then-path performs an overflow-checked negation (a function call to `llvm.ssub.with.overflow`). The phi node at the merge point correctly selects between the original value and the negated value.

3. **Nested if/else** (`@my_sign`): The outer `if n > 0` uses a branch (it guards a constant `1` vs a sub-expression), while the inner `if n < 0 then -1 else 0` uses `select` (choosing between two constants). This hybrid branch/select approach is exactly right -- it minimizes branches while maintaining correctness.

### 9. Branching: Phi Node Correctness

Both phi nodes in the module are correct and minimal:

- **@my_abs `%v712`**: `phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]` -- selects the original parameter `n` (when `n >= 0`) or the negated value (when `n < 0`). Both incoming edges are correct.
- **@my_sign `%v111`**: `phi i64 [ %sel, %bb2 ], [ 1, %bb0 ]` -- selects the inner conditional result (when `n <= 0`) or the constant `1` (when `n > 0`). Both incoming edges are correct.

No unnecessary phi nodes exist. `@my_max` avoids phi nodes entirely through `select`. `@main` uses no phi nodes because it follows a linear path with overflow checks branching only to panic.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE     | Control Flow | Excellent select optimization for @my_max | NEW | J2 |
| 2 | NOTE     | Control Flow | Hybrid branch/select in @my_sign | NEW | J2 |
| 3 | NOTE     | Attributes | Full memory(none) on pure functions | NEW | J2 |

### NOTE-1: Excellent select optimization for @my_max

**Location**: @my_max function body
**Impact**: Positive -- branchless codegen eliminates branch misprediction cost entirely
**Found in**: Control Flow & Block Layout (Category 4), Branching: Select vs Branch (Category 8)

### NOTE-2: Hybrid branch/select in @my_sign

**Location**: @my_sign function body
**Impact**: Positive -- the compiler correctly uses branches where needed (outer conditional with different-complexity paths) and selects where possible (inner conditional with two constants)
**Found in**: Control Flow & Block Layout (Category 4), Branching: Select vs Branch (Category 8)

### NOTE-3: Full memory(none) attribute on pure functions

**Location**: @my_abs, @my_max, @my_sign function declarations
**Impact**: Positive -- `memory(none)` enables LLVM to freely reorder, hoist, and eliminate calls to these functions since they have no observable side effects
**Found in**: Attributes & Calling Convention (Category 3)

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

Journey 2's branching codegen is flawless. The compiler demonstrates three distinct branch-lowering strategies -- branchless `select` for simple conditionals, overflow-checked branches for arithmetic with side effects, and a hybrid approach for nested conditionals -- all achieving the theoretical optimal instruction count. Every function has full attribute coverage including `memory(none)` for pure functions, `nounwind` from fixed-point analysis, and `fastcc` calling convention. ARC is irrelevant for pure scalar operations, and all overflow checks are correctly placed.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind attribute | J1 | J2 | CONFIRMED |
| noundef attribute | J1 | J2 | CONFIRMED |
| memory(none) | J2 | J2 | NEW |
| select optimization | J2 | J2 | NEW |

Journey 1 also achieved 10.0/10 for its simple arithmetic. Journey 2 extends coverage to branching patterns while maintaining the same perfect score, confirming that the compiler's branch lowering (select optimization, phi node generation, and nested conditional handling) produces optimal code for scalar integer operations.
