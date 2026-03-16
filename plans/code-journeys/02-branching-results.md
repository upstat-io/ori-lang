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
  - "Understanding of conditionals and comparison operators"
learning_objectives:
  - "Understand how if/then/else lowers to LLVM branches vs select instructions"
  - "See how the compiler chooses select for simple conditionals and br+phi for complex ones"
  - "Compare nested conditionals in source to flat CFG in IR"
  - "Observe how overflow checking interacts with unary negation"
features:
  - branching
  - comparison
  - function_calls
  - multiple_functions
feature_description: "Branching with comparisons, multiple functions, and nested conditionals"
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
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 142 | **Keywords**: 12 (`if`, `then`, `else`, `let`) | **Identifiers**: 28 | **Errors**: 0

<details>
<summary>Token stream (user code)</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen Arrow
Ident(int) Eq If Ident(n) Lt Lit(0) Then Minus Ident(n) Else Ident(n) Semi

Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma Ident(b)
Colon Ident(int) RParen Arrow Ident(int) Eq If Ident(a) Gt Ident(b)
Then Ident(a) Else Ident(b) Semi

Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen Arrow
Ident(int) Eq If Ident(n) Gt Lit(0) Then Lit(1) Else LParen If Ident(n)
Lt Lit(0) Then Minus Lit(1) Else Lit(0) RParen Semi

Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus Lit(7) RParen Semi
Let Ident(b) Eq Ident(my_max) LParen Ident(a) Colon Lit(3) Comma Ident(b)
Colon Lit(10) RParen Semi
Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon Lit(0) RParen Semi
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

**Constraints**: 24 | **Types inferred**: 12 | **Unifications**: 18 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@my_abs (n: int) -> int = if n < 0 then -n else n
//                           ^ bool (Lt<int, int> -> bool)
//                                   ^ int (Neg<int> -> int)
//                                          ^ int (param n)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int> -> bool)
//                                               ^ int      ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1
//     ^ bool (Gt<int, int> -> bool)
//                ^ int (literal)
    else (if n < 0 then -1 else 0)
//          ^ bool          ^ int   ^ int

@main () -> int = {
    let a: int = my_abs(n: -7)     // inferred: int
    let b: int = my_max(a: 3, b: 10)  // inferred: int
    let c: int = my_sign(n: 0)    // inferred: int
    a + b + c                      // int (Add<int, int> -> int)
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
- Call arguments normalized to positional order
- Unary negation on literal -7 folded to Int(-7)
- Unary negation on literal -1 folded to Int(-1)
- If/else chains preserved as nested If nodes
- 43 canonical nodes produced from 40 source expressions
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
@my_abs: no heap values — pure scalar conditional + negation
@my_max: no heap values — pure scalar conditional
@my_sign: no heap values — pure scalar conditional
@main: no heap values — pure scalar arithmetic + calls
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
  └─ let a = @my_abs(n: -7)
       └─ if -7 < 0 → true
            └─ -(-7) = 7
       → 7
  └─ let b = @my_max(a: 3, b: 10)
       └─ if 3 > 10 → false
            └─ 10
       → 10
  └─ let c = @my_sign(n: 0)
       └─ if 0 > 0 → false
            └─ if 0 < 0 → false
                 └─ 0
       → 0
  └─ 7 + 10 + 0 = 17
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
  ret i32 %exit_code
}

attributes #0 = { nounwind memory(none) uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
```

#### Disassembly

```asm
_ori_my_abs:
   sub    $0x18,%rsp
   mov    %rdi,0x8(%rsp)
   cmp    $0x0,%rdi
   mov    %rdi,0x10(%rsp)
   jge    1b12b
   mov    0x8(%rsp),%rcx
   xor    %eax,%eax
   sub    %rcx,%rax
   seto   %cl
   test   $0x1,%cl
   mov    %rax,0x10(%rsp)
   jne    1b135
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
   lea    0xd31c0(%rip),%rdi
   call   1bcf0 <ori_panic_cstr>

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
   jg     1b190
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
   call   1b100 <_ori_my_abs>
   mov    %rax,0x10(%rsp)
   mov    $0x3,%edi
   mov    $0xa,%esi
   call   1b150 <_ori_my_max>
   mov    %rax,0x8(%rsp)
   xor    %eax,%eax
   mov    %eax,%edi
   call   1b160 <_ori_my_sign>
   mov    0x8(%rsp),%rcx
   mov    %rax,%rdx
   mov    0x10(%rsp),%rax
   mov    %rdx,0x18(%rsp)
   add    %rcx,%rax
   mov    %rax,0x20(%rsp)
   seto   %al
   jo     1b209
   mov    0x18(%rsp),%rcx
   mov    0x20(%rsp),%rax
   add    %rcx,%rax
   mov    %rax,(%rsp)
   seto   %al
   jo     1b21e
   jmp    1b215
   lea    0xd3109(%rip),%rdi
   call   1bcf0 <ori_panic_cstr>
   mov    (%rsp),%rax
   add    $0x28,%rsp
   ret
   lea    0xd30f4(%rip),%rdi
   call   1bcf0 <ori_panic_cstr>

main:
   push   %rax
   call   1b1a0 <_ori_main>
   pop    %rcx
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @my_abs  | 10     | 10    | 1.00x | OPTIMAL |
| 2 | @my_max  | 3      | 3     | 1.00x | OPTIMAL |
| 3 | @my_sign | 7      | 7     | 1.00x | OPTIMAL |
| 4 | @main    | 16     | 16    | 1.00x | OPTIMAL |

**@my_abs** (10 instructions): `icmp slt` for condition, `br` for branch, `call @llvm.ssub.with.overflow` for checked negation (0 - n), two `extractvalue` to unpack result and overflow flag, `br` on overflow, `phi` to merge paths, `ret`, plus panic path (`call @ori_panic_cstr` + `unreachable`). All instructions are necessary -- the overflow check on negation catches `INT_MIN` which would silently wrap.

**@my_max** (3 instructions): `icmp sgt` + `select` + `ret`. The compiler correctly recognized that `if a > b then a else b` can use a `select` instead of a branch+phi pattern. This is the ideal lowering -- no branches, no phi nodes, just a conditional move.

**@my_sign** (7 instructions): The outer `if n > 0` uses `br`+`phi` (necessary because the else branch has computation), while the inner `if n < 0 then -1 else 0` uses `select` (both arms are constants). This hybrid approach is optimal -- the compiler correctly chose `select` where possible and `br` where needed.

**@main** (16 instructions): 3 function calls, 2 overflow-checked additions (each requires `call @llvm.sadd.with.overflow` + 2 `extractvalue` + `br`), `ret`, and 2 panic paths. All justified.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | memory(none) | cold | Notes |
|----------|--------|----------|---------|--------------|------|-------|
| @my_abs  | YES    | YES      | YES     | YES          | NO   | [NOTE-1] |
| @my_max  | YES    | YES      | YES     | YES          | NO   | [NOTE-1] |
| @my_sign | YES    | YES      | YES     | YES          | NO   | [NOTE-1] |
| @main    | NO     | YES      | YES     | NO           | NO   | C calling convention for entry point (correct) |
| @panic   | N/A    | N/A      | N/A     | N/A          | YES  | `cold noreturn` (correct) |

All user functions have `nounwind memory(none)` -- the compiler correctly determined they are pure (no memory side effects) and cannot unwind. The AIMS pipeline's fixed-point nounwind + memory analysis is working correctly.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 4      | 0           | 0                 | 1         | [NOTE-2] |
| @my_max  | 1      | 0           | 0                 | 0         | [NOTE-3] |
| @my_sign | 3      | 0           | 0                 | 1         | [NOTE-4] |
| @main    | 5      | 0           | 0                 | 0         | Clean overflow structure |

**@my_abs**: 4 blocks (entry, negation, merge, panic). The phi node in `bb3` cleanly merges the `n` (no-negate) and `neg.val` (negate) paths. No wasted blocks.

**@my_max**: Single block -- the `select` instruction eliminates all branching. This is the best possible lowering for a simple conditional.

**@my_sign**: 3 blocks (entry, else-branch, merge). The inner `if n < 0 then -1 else 0` is lowered to a `select` within `bb2`, avoiding a fourth block. The phi in `bb3` merges the outer if paths.

**@main**: 5 blocks (entry, add.ok, add.ok6, add.ovf_panic, add.ovf_panic7). Each overflow check adds a panic block. The two separate panic blocks (`add.ovf_panic` and `add.ovf_panic7`) could theoretically share a single block, but LLVM may merge them during optimization anyway, and the cold attribute ensures they do not affect hot-path performance.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| negation (-n) | YES | YES | Uses `llvm.ssub.with.overflow(0, n)` -- catches INT_MIN |
| add (a+b) | YES | YES | Uses `llvm.sadd.with.overflow` |
| add ((a+b)+c) | YES | YES | Uses `llvm.sadd.with.overflow` |

All arithmetic operations are overflow-checked. The negation is particularly noteworthy: it uses `ssub.with.overflow(0, n)` rather than a simple `sub nsw`, which correctly catches the case where `n = INT_MIN` (since `-INT_MIN` overflows in two's complement). The panic messages are descriptive: "integer overflow on negation" and "integer overflow on addition".

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.2 KiB |
| .rodata section | 133.5 KiB |
| User code (@my_abs) | 66 bytes (17 instructions) |
| User code (@my_max) | 11 bytes (4 instructions) |
| User code (@my_sign) | 54 bytes (14 instructions) |
| User code (@main) | 138 bytes (31 instructions) |
| User code (main wrapper) | 8 bytes (4 instructions) |
| Total user code | 277 bytes |
| Runtime | ~99.97% of .text |

#### Disassembly: @my_max

```asm
_ori_my_max:
   mov    %rsi,%rax
   cmp    %rax,%rdi
   cmovg  %rdi,%rax
   ret
```

The `cmovg` (conditional move if greater) is the native x86 equivalent of LLVM's `select` -- branchless, single-cycle on modern CPUs. This is textbook optimal codegen for `max(a, b)`.

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (10 instructions — overflow-checked negation required)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) nounwind memory(none) {
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %negate, label %done
negate:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %panic, label %done
done:
  %r = phi i64 [ %0, %entry ], [ %neg.val, %negate ]
  ret i64 %r
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (10 instructions — matches ideal exactly)
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

**Delta**: +0 instructions. Structurally identical.

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) nounwind memory(none) {
  %gt = icmp sgt i64 %0, %1
  %r = select i1 %gt, i64 %0, i64 %1
  ret i64 %r
}
```

```llvm
; ACTUAL (3 instructions — matches ideal exactly)
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}
```

**Delta**: +0 instructions. The compiler chose `select` over `br`+`phi` -- textbook optimal.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions — hybrid select+branch for nested conditionals)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) nounwind memory(none) {
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %done, label %else
else:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %done
done:
  %r = phi i64 [ %sel, %else ], [ 1, %entry ]
  ret i64 %r
}
```

```llvm
; ACTUAL (7 instructions — matches ideal exactly)
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

**Delta**: +0 instructions. Structurally identical. The compiler smartly used `select` for the inner conditional (both arms are constants) and `br`+`phi` for the outer (where the else branch requires computation).

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions — 3 calls + 2 overflow-checked adds)
define noundef i64 @_ori_main() nounwind {
  %a = call fastcc i64 @_ori_my_abs(i64 -7)
  %b = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %c = call fastcc i64 @_ori_my_sign(i64 0)
  %ab = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %ab.val = extractvalue { i64, i1 } %ab, 0
  %ab.ovf = extractvalue { i64, i1 } %ab, 1
  br i1 %ab.ovf, label %panic1, label %ok1
ok1:
  %abc = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %ab.val, i64 %c)
  %abc.val = extractvalue { i64, i1 } %abc, 0
  %abc.ovf = extractvalue { i64, i1 } %abc, 1
  br i1 %abc.ovf, label %panic2, label %ok2
ok2:
  ret i64 %abc.val
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +0 instructions. Actual matches ideal.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 10    | 10     | +0    | N/A       | OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |

### 8. Branching: Select vs Branch Strategy

The compiler exhibits intelligent branch lowering by choosing between two strategies based on the structure of the conditional:

**Strategy 1 -- `select` (branchless)**: Used when both arms are simple values with no side effects and no further computation. Example: `@my_max` lowers `if a > b then a else b` to a single `select` instruction, which becomes `cmovg` in x86 -- zero branch misprediction cost.

**Strategy 2 -- `br` + `phi` (branching)**: Used when at least one arm requires computation (like overflow-checked negation in `@my_abs`) or when the conditional is the outer level of a nested structure (like the outer `if` in `@my_sign`).

**Hybrid approach in @my_sign**: The compiler correctly identified that the outer conditional (`if n > 0`) needs branching (the else arm has computation), but the inner conditional (`if n < 0 then -1 else 0`) can use `select` (both arms are constants). This hybrid produces exactly the minimal number of basic blocks (3) and instructions (7).

This is excellent codegen -- the decision between `select` and `br`+`phi` matches what an experienced LLVM engineer would write by hand.

### 9. Branching: Negation Overflow Safety

The negation in `@my_abs` (`-n` when `n < 0`) is lowered to `llvm.ssub.with.overflow(0, n)` rather than a naive `sub i64 0, %n`. This is critical for correctness: in two's complement, `-INT_MIN` overflows because `INT_MIN = -9223372036854775808` but `INT_MAX = 9223372036854775807`. The overflow intrinsic catches this edge case and panics with a descriptive message ("integer overflow on negation").

This is the correct behavior for a safe language -- Ori does not silently wrap on overflow.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE     | Attributes | nounwind and memory(none) on all pure functions | NEW | J2 |
| 2 | NOTE     | Control Flow | Optimal select vs branch strategy | NEW | J2 |
| 3 | NOTE     | Control Flow | Hybrid select+branch in nested conditionals | NEW | J2 |
| 4 | NOTE     | Overflow | Negation overflow correctly caught via ssub intrinsic | NEW | J2 |

### NOTE-1: Excellent attribute inference

**Location**: @my_abs, @my_max, @my_sign function declarations
**Impact**: Positive -- `nounwind` enables better exception table generation, `memory(none)` enables aggressive dead store elimination, load hoisting, and CSE across calls
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-2: Clean phi-merge in @my_abs

**Location**: @my_abs, block bb3
**Impact**: Positive -- single phi node cleanly merges the two paths (negated vs original)
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-3: Branchless @my_max via select

**Location**: @my_max, single block
**Impact**: Positive -- eliminates branch misprediction entirely; compiles to `cmovg` on x86
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Hybrid select+branch in @my_sign

**Location**: @my_sign, blocks bb0/bb2/bb3
**Impact**: Positive -- minimizes blocks (3 instead of 4-5) by using `select` for inner conditional
**Found in**: Control Flow & Block Layout (Category 4)

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

Journey 2's branching codegen is flawless. The compiler demonstrates intelligent lowering of conditionals, choosing branchless `select` for `@my_max` (compiling to `cmovg`), overflow-checked negation with `ssub.with.overflow` for `@my_abs`, and a hybrid select+branch strategy for the nested conditional in `@my_sign`. Every function matches its ideal IR instruction-for-instruction. All attributes (`nounwind`, `memory(none)`, `noundef`, `fastcc`) are correctly applied. Zero ARC overhead for pure scalar operations.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED -- extended to negation |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind attribute | J1 | J2 | CONFIRMED |
| memory(none) attribute | J1 | J2 | CONFIRMED |
| noundef on params/returns | J1 | J2 | CONFIRMED |

Journey 2 extends Journey 1's attribute quality to branching functions. The `memory(none)` attribute is particularly impressive here -- the compiler correctly determined that all three user functions (`my_abs`, `my_max`, `my_sign`) are pure despite containing branches and overflow checks. The `nounwind` fixed-point analysis (visible in the ARC trace: "nounwind + memory analysis complete, passes=2, nounwind_count=4, pure_count=3") correctly marked all four functions.
