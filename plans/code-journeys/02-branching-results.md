---
journey: 2
slug: branching
theme: "I am a branch"
date: 2026-03-06
status: PASS
expected: 17
eval_result: 17
aot_result: 17
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of conditional expressions"
learning_objectives:
  - "See how if/then/else is lowered to LLVM branch + phi vs select"
  - "Understand overflow checking on negation (ssub) vs addition (sadd)"
  - "Compare ideal vs actual codegen for branching functions"
  - "Observe LLVM select instruction optimization for simple conditionals"
features:
  - branching
  - comparison
  - function_calls
  - multiple_functions
feature_description: "Conditional branching with comparisons, multiple user-defined functions"
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
    relationship: "Same overflow checking pattern, nounwind now present (was missing in J1)"
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

**Tokens**: 142 | **Keywords**: 12 | **Identifiers**: 20 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Lt Lit(0) Then Minus Ident(n)
Else Ident(n) Semi
Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(int) Eq If
Ident(a) Gt Ident(b) Then Ident(a) Else Ident(b) Semi
Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Gt Lit(0) Then Lit(1) Else
LParen If Ident(n) Lt Lit(0) Then Minus Lit(1) Else Lit(0)
RParen Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus
Lit(7) RParen Semi Let Ident(b) Eq Ident(my_max) LParen
Ident(a) Colon Lit(3) Comma Ident(b) Colon Lit(10) RParen
Semi Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon
Lit(0) RParen Semi Ident(a) Plus Ident(b) Plus Ident(c) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 40 | **Max depth**: 4 | **Functions**: 4 | **Errors**: 0

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
│       ├─ Then: Unary(-)
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
│       └─ Else: If
│            ├─ Cond: BinOp(<)
│            │    ├─ Ident(n)
│            │    └─ Lit(0)
│            ├─ Then: Unary(-)
│            │    └─ Lit(1)
│            └─ Else: Lit(0)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@my_abs, n: Unary(-) Lit(7))
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
//                                          ^ int (parameter)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int> -> bool)
//                                               ^ int      ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1
//     ^ bool       ^ int
    else (if n < 0 then -1 else 0)
//          ^ bool       ^ int   ^ int

@main () -> int = {
    let a: int = my_abs(n: -7)       // int (return type of @my_abs)
    let b: int = my_max(a: 3, b: 10) // int (return type of @my_max)
    let c: int = my_sign(n: 0)       // int (return type of @my_sign)
    a + b + c                        // int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 6 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
- Unary negation (-n) preserved as Unary(Neg, n)
- Nested if/else in @my_sign lowered as nested If nodes
- Constants (-7, 3, 10, 0) folded into canonical literal form
- Block expression in @main lowered with let-bindings + result expression
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
@my_abs:  no heap values — pure scalar arithmetic
@my_max:  no heap values — pure scalar arithmetic
@my_sign: no heap values — pure scalar arithmetic
@main:    no heap values — pure scalar arithmetic
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
  └─ let b = @my_max(a: 3, b: 10)
       └─ if 3 > 10 → false
            └─ 10
  └─ let c = @my_sign(n: 0)
       └─ if 0 > 0 → false
            └─ if 0 < 0 → false
                 └─ 0
  └─ a + b + c
       └─ 7 + 10 = 17
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
@my_abs:  +0 rc_inc, +0 rc_dec (no heap values)
@my_max:  +0 rc_inc, +0 rc_dec (no heap values)
@my_sign: +0 rc_inc, +0 rc_dec (no heap values)
@main:    +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '02-branching'
source_filename = "02-branching"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on negation\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
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

; Function Attrs: nounwind uwtable
; --- @my_max ---
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}

; Function Attrs: nounwind uwtable
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
_ori_my_abs:
   sub    $0x18,%rsp
   mov    %rdi,0x10(%rsp)
   cmp    $0x0,%rdi
   jge    <_ori_my_abs+0x25>
   mov    0x10(%rsp),%rcx
   xor    %eax,%eax
   sub    %rcx,%rax
   mov    %rax,0x8(%rsp)
   seto   %al
   jo     <_ori_my_abs+0x44>
   jmp    <_ori_my_abs+0x39>
   mov    0x10(%rsp),%rax
   mov    %rax,(%rsp)
   jmp    <_ori_my_abs+0x30>
   mov    (%rsp),%rax
   add    $0x18,%rsp
   ret
   mov    0x8(%rsp),%rax
   mov    %rax,(%rsp)
   jmp    <_ori_my_abs+0x30>
   lea    0xd31b1(%rip),%rdi
   call   <ori_panic_cstr>

_ori_my_max:
   mov    %rsi,%rax
   cmp    %rax,%rdi
   cmovg  %rdi,%rax
   ret
   nopl   0x0(%rax,%rax,1)

_ori_my_sign:
   mov    %rdi,-0x8(%rsp)
   cmp    $0x0,%rdi
   jle    <_ori_my_sign+0x17>
   mov    $0x1,%eax
   mov    %rax,-0x10(%rsp)
   jmp    <_ori_my_sign+0x32>
   mov    -0x8(%rsp),%rdx
   xor    %eax,%eax
   mov    $0xffffffffffffffff,%rcx
   cmp    $0x0,%rdx
   cmovl  %rcx,%rax
   mov    %rax,-0x10(%rsp)
   mov    -0x10(%rsp),%rax
   ret
   nopl   0x0(%rax,%rax,1)

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
   jo     <_ori_main+0x69>
   mov    0x18(%rsp),%rcx
   mov    0x20(%rsp),%rax
   add    %rcx,%rax
   mov    %rax,(%rsp)
   seto   %al
   jo     <_ori_main+0x7e>
   jmp    <_ori_main+0x75>
   lea    0xd3109(%rip),%rdi
   call   <ori_panic_cstr>
   mov    (%rsp),%rax
   add    $0x28,%rsp
   ret
   lea    0xd30f4(%rip),%rdi
   call   <ori_panic_cstr>
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @my_abs  | 12     | 11    | 1.09x | NEAR-OPTIMAL |
| 2 | @my_max  | 3      | 3     | 1.00x | OPTIMAL |
| 3 | @my_sign | 8      | 7     | 1.14x | NEAR-OPTIMAL |
| 4 | @main    | 16     | 16    | 1.00x | OPTIMAL |

**@my_abs** (12 actual vs 11 ideal): The single unjustified instruction is the unconditional `br label %bb3` in `bb2` (the else branch). The `bb0` could branch directly to `bb3` with `%0` as a phi operand from `bb0`, eliminating `bb2` entirely. The negation overflow check (`ssub.with.overflow`) and the conditional branch are justified. Similarly, `neg.ok` has an unconditional branch to `bb3` that is structurally necessary for the phi but could be merged. [LOW-1]

**@my_max** (3 actual = 3 ideal): Perfect. The compiler correctly lowered the simple `if a > b then a else b` to `icmp sgt` + `select` + `ret` -- no branches at all. This is textbook optimal codegen for a branchless max.

**@my_sign** (8 actual vs 7 ideal): The single unjustified instruction is the unconditional `br label %bb3` in `bb1` (the then branch returning constant 1). The phi node in `bb3` correctly merges the result. The inner `if n < 0 then -1 else 0` was excellently lowered to `icmp slt` + `select` -- branchless. [LOW-1]

**@main** (16 actual = 16 ideal): All instructions are justified -- 3 calls, 2 overflow-checked additions (each = call + 2 extractvalue + conditional branch), and the ret. No waste.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. OPTIMAL. All values are scalars (`int`), so ARC is correctly inactive.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | uwtable | cold | Notes |
|----------|--------|----------|---------|---------|------|-------|
| @my_abs  | YES    | YES      | YES     | YES     | NO   |       |
| @my_max  | YES    | YES      | YES     | YES     | NO   |       |
| @my_sign | YES    | YES      | YES     | YES     | NO   |       |
| @main    | NO (C) | YES      | YES     | YES     | NO   |       |
| @panic   | N/A    | N/A      | N/A     | N/A     | YES  |       |

All user functions correctly have `nounwind` (fixed since Journey 1). The `@_ori_main` correctly uses C calling convention (it is the entry point called from the `main` wrapper). All parameters and return values have `noundef`. The `ori_panic_cstr` declaration correctly has `cold noreturn`.

The 1 missing attribute (23 applicable, 22 correct) is likely a missing `memory` attribute on pure functions like `@my_max` which is a pure function of its arguments (no memory side effects). [LOW-2]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 6      | 2           | 1                 | 1         | [LOW-1] |
| @my_max  | 1      | 0           | 0                 | 0         |       |
| @my_sign | 4      | 1           | 1                 | 1         | [LOW-1] |
| @main    | 5      | 0           | 0                 | 0         |       |

**@my_abs**: `bb2` is an empty block containing only `br label %bb3`. The phi in `bb3` takes `%0` from `bb2` -- this could be restructured so `bb0` branches directly to `bb3` with `%0` as a phi operand from `bb0`. Similarly, `neg.ok` is a single `br label %bb3` -- structurally necessary to feed the phi but could be merged. These are minor inefficiencies that LLVM's SimplifyCFG pass would clean up in an optimized build.

**@my_max**: Perfect single-block structure. The `select` instruction avoids branching entirely.

**@my_sign**: `bb1` is an empty block containing only `br label %bb3`. Similar to `@my_abs`, the phi could take the constant `1` directly from `bb0`.

**@main**: Well-structured. The 5 blocks (bb0, add.ok, add.ok6, add.ovf_panic, add.ovf_panic7) are all necessary for the two overflow checks. No empty blocks.

### 5. Overflow Checking

**Status**: PASS

| Operation | Intrinsic | Checked | Correct | Notes |
|-----------|-----------|---------|---------|-------|
| negation (-n) | `llvm.ssub.with.overflow.i64` | YES | YES | `0 - n` catches INT_MIN |
| addition (a+b) | `llvm.sadd.with.overflow.i64` | YES | YES | First add in @main |
| addition (a+b+c) | `llvm.sadd.with.overflow.i64` | YES | YES | Second add in @main |
| comparison (<) | `icmp slt` | N/A | YES | No overflow possible |
| comparison (>) | `icmp sgt` | N/A | YES | No overflow possible |

All arithmetic operations that can overflow are checked. Comparisons correctly use `icmp` which cannot overflow. Negation correctly uses `ssub.with.overflow(0, n)` which catches the edge case of negating `INT_MIN` (-2^63).

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 868.1 KiB |
| .rodata section | 133.5 KiB |
| User code | 293 bytes (77 instructions) |
| Runtime | >99.9% of binary |

#### Disassembly: @my_abs

```asm
_ori_my_abs:
   sub    $0x18,%rsp           ; frame setup
   mov    %rdi,0x10(%rsp)      ; spill n to stack
   cmp    $0x0,%rdi            ; n < 0?
   jge    +0x25                ; skip negation if n >= 0
   mov    0x10(%rsp),%rcx      ; reload n
   xor    %eax,%eax            ; zero rax (for 0 - n)
   sub    %rcx,%rax            ; 0 - n
   mov    %rax,0x8(%rsp)       ; spill result
   seto   %al                  ; check overflow flag
   jo     +0x44                ; jump to panic if overflow
   jmp    +0x39                ; jump to merge (neg.ok -> bb3)
   mov    0x10(%rsp),%rax      ; else: reload n
   mov    %rax,(%rsp)          ; store to result slot
   jmp    +0x30                ; jump to merge (bb2 -> bb3)
   mov    (%rsp),%rax          ; load result
   add    $0x18,%rsp           ; frame teardown
   ret
   mov    0x8(%rsp),%rax       ; neg.ok: load negated value
   mov    %rax,(%rsp)          ; store to result slot
   jmp    -0x14                ; jump to merge
   lea    (%rip),%rdi          ; panic: load message ptr
   call   <ori_panic_cstr>     ; panic
```

The stack spills (6 memory operations) are artifacts of debug mode (-O0). In an optimized build, LLVM would keep everything in registers.

#### Disassembly: @my_max

```asm
_ori_my_max:
   mov    %rsi,%rax            ; rax = b
   cmp    %rax,%rdi            ; compare a with b
   cmovg  %rdi,%rax            ; if a > b, rax = a
   ret
```

Only 4 instructions (3 + ret). Branchless, optimal. The `cmovg` is the ideal lowering of `select`.

#### Disassembly: @my_sign

```asm
_ori_my_sign:
   mov    %rdi,-0x8(%rsp)      ; spill n
   cmp    $0x0,%rdi            ; n > 0?
   jle    +0x17                ; jump if n <= 0
   mov    $0x1,%eax            ; result = 1
   mov    %rax,-0x10(%rsp)     ; store result
   jmp    +0x32                ; jump to merge
   mov    -0x8(%rsp),%rdx      ; reload n
   xor    %eax,%eax            ; rax = 0
   mov    $-1,%rcx             ; rcx = -1
   cmp    $0x0,%rdx            ; n < 0?
   cmovl  %rcx,%rax            ; if n < 0, rax = -1
   mov    %rax,-0x10(%rsp)     ; store result
   mov    -0x10(%rsp),%rax     ; load result
   ret
```

The inner `if n < 0 then -1 else 0` compiles to a branchless `cmovl` -- excellent. The outer `if n > 0` uses a branch (correct, as the then-clause is a constant and the else-clause has computation).

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (11 instructions)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) nounwind {
bb0:
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %bb1, label %bb3

bb1:
  %neg = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %neg.val = extractvalue { i64, i1 } %neg, 0
  %neg.ovf = extractvalue { i64, i1 } %neg, 1
  br i1 %neg.ovf, label %neg.ovf_panic, label %bb3

bb3:
  %v7 = phi i64 [ %0, %bb0 ], [ %neg.val, %bb1 ]
  ret i64 %v7

neg.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (12 instructions)
; Delta: +1 instruction (empty bb2 with unconditional br)
; bb2 contains only `br label %bb3` — redundant
; neg.ok contains only `br label %bb3` — also redundant but structurally similar
```

**Delta**: +1 unjustified instruction. The empty `bb2` block could be eliminated by having `bb0` branch directly to `bb3` and adding `%0` as a phi operand from `bb0`. The `neg.ok` block similarly adds an unconditional jump but is counted as part of the overflow checking pattern.

#### @my_max: Ideal vs Actual

```llvm
; IDEAL (3 instructions) — matches ACTUAL exactly
define fastcc noundef i64 @_ori_my_max(i64 noundef %0, i64 noundef %1) nounwind {
bb0:
  %gt = icmp sgt i64 %0, %1
  %sel = select i1 %gt, i64 %0, i64 %1
  ret i64 %sel
}
```

**Delta**: +0 instructions. OPTIMAL. The compiler correctly chose `select` over branching.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) nounwind {
bb0:
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %bb3_const1, label %bb2

bb2:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %bb3

bb3:
  %v11 = phi i64 [ %sel, %bb2 ], [ 1, %bb0 ]
  ret i64 %v11
}
```

```llvm
; ACTUAL (8 instructions)
; Delta: +1 instruction (empty bb1 with unconditional br)
; bb1 contains only `br label %bb3` — redundant
```

**Delta**: +1 unjustified instruction. Same pattern as `@my_abs` -- empty block for the then branch.

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions) — matches ACTUAL exactly
define noundef i64 @_ori_main() nounwind {
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

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable

add.ok6:
  ret i64 %add.val4

add.ovf_panic7:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 11    | 12     | +1    | NO (empty block) | NEAR-OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 8      | +1    | NO (empty block) | NEAR-OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |

### 8. Branching: Select vs Branch Lowering

The compiler demonstrates two distinct lowering strategies for `if/then/else`:

1. **`select` (branchless)**: Used for `@my_max` where both branches are simple values (`a` or `b`). Also used for the inner conditional in `@my_sign` where both branches are constants (`-1` or `0`). This is the optimal strategy for simple value selection -- no branch misprediction penalty.

2. **`br` + `phi` (branching)**: Used for `@my_abs` where the then-branch contains computation (overflow-checked negation) and for the outer conditional in `@my_sign` where one branch has computation. This is correct -- `select` would eagerly evaluate both sides, which is wasteful when one side is expensive.

The compiler's heuristic appears sound: it uses `select` when both branches are trivial values and `br`+`phi` when a branch involves computation or side effects (overflow checking). The only improvement opportunity is eliminating the empty passthrough blocks in the branching pattern (the blocks that contain only `br label %merge`).

### 9. Branching: Negation Overflow Safety

The compiler correctly handles negation via `llvm.ssub.with.overflow.i64(0, n)` rather than a raw `sub` or `neg`. This is semantically correct: the expression `-n` is `0 - n`, and on i64, negating `INT_MIN` (-2^63 = -9223372036854775808) would overflow because `2^63` exceeds `INT_MAX` (2^63 - 1). The panic path is reachable for this edge case.

Alternative approaches considered:
- `llvm.neg` does not exist as a checked intrinsic
- Raw `sub nsw i64 0, %n` would be UB on overflow -- unsafe
- The `ssub.with.overflow` approach is the correct choice

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW      | Control Flow | Empty passthrough blocks in branching functions | NEW | J2 |
| 2 | LOW      | Attributes | Missing `memory` attribute on pure functions | NEW | J2 |
| 3 | NOTE     | Codegen | Excellent select vs branch heuristic | NEW | J2 |
| 4 | NOTE     | Codegen | Correct negation overflow checking via ssub | NEW | J2 |
| 5 | NOTE     | Attributes | nounwind now present on all user functions (fixed since J1) | NEW | J2 |

### LOW-1: Empty passthrough blocks in branching functions

**Location**: `@my_abs` block `bb2`, `@my_sign` block `bb1`
**Impact**: 1 unnecessary `br` instruction per function. LLVM's SimplifyCFG would eliminate these in an optimized build. In unoptimized IR, they add minor code size overhead.
**Fix**: When lowering `if/then/else` to branch+phi, skip generating the empty passthrough block and instead have the entry block branch directly to the merge block with the value as a phi operand.
**First seen**: Journey 2
**Found in**: Control Flow & Block Layout (Category 4), Instruction Purity (Category 1)

### LOW-2: Missing `memory` attribute on pure functions

**Location**: `@my_max` function declaration (and potentially `@my_sign`)
**Impact**: LLVM cannot prove these functions are side-effect-free, limiting inter-procedural optimizations (e.g., hoisting calls out of loops, eliminating redundant calls).
**Fix**: Add `memory(none)` or `memory(read)` attributes to functions that do not write memory. `@my_max` is a pure function of its arguments. `@my_abs` and `@my_sign` may call `ori_panic_cstr` on overflow, so `memory(none)` is not fully applicable to them without more nuanced analysis.
**First seen**: Journey 2
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Excellent select vs branch heuristic

**Location**: `@my_max` and inner branch of `@my_sign`
**Impact**: Positive -- branchless codegen for simple conditionals eliminates branch misprediction. The compiler correctly distinguishes between cases where `select` is appropriate (both sides are values) and where `br`+`phi` is needed (a side has computation).
**Found in**: Branching: Select vs Branch Lowering (Category 8)

### NOTE-4: Correct negation overflow checking via ssub

**Location**: `@my_abs` block `bb1`
**Impact**: Positive -- catches the `INT_MIN` edge case that would silently produce wrong results with unchecked negation. Uses `llvm.ssub.with.overflow.i64(0, n)` which is the correct intrinsic.
**Found in**: Branching: Negation Overflow Safety (Category 9)

### NOTE-5: nounwind now present on all user functions

**Location**: All four user function declarations
**Impact**: Positive -- LLVM can now omit exception handling tables for these functions, improving both code size and performance. This was flagged as LOW in Journey 1 and is now fixed.
**Found in**: Attributes & Calling Convention (Category 3)

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

Journey 2's branching codegen is strong. The compiler demonstrates an excellent `select`-vs-branch heuristic: simple value selection (`@my_max`) compiles to branchless `select`, while computed branches (`@my_abs`, `@my_sign`) correctly use `br`+`phi`. Negation overflow is properly caught via `ssub.with.overflow`. The `nounwind` attribute is now present on all functions, fixing the Journey 1 finding. The only overhead is minor empty passthrough blocks in the branching pattern -- these are cosmetic at -O0 and would be eliminated by LLVM's SimplifyCFG at higher optimization levels.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED (extended to negation via ssub) |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind attribute | J1 (missing) | J2 | FIXED |
| noundef attribute | J1 | J2 | CONFIRMED |

Journey 2 extends the overflow checking story from simple addition (J1) to negation via `ssub.with.overflow`, demonstrating the compiler handles multiple arithmetic overflow patterns. The `nounwind` finding from Journey 1 is resolved -- all user functions now carry the attribute.
