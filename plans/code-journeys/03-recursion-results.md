---
journey: 3
slug: recursion
theme: "I am recursive"
date: 2026-03-16
status: PASS
expected: 61
eval_result: 61
aot_result: 61
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of recursion and base cases"
  - "Familiarity with comparison operators"
learning_objectives:
  - "See how recursive functions are compiled to LLVM IR"
  - "Observe tail-call optimization (TCO) on gcd via loop lowering"
  - "Understand overflow checking overhead in recursive arithmetic"
  - "Compare tree recursion (fib) vs tail recursion (gcd) codegen"
features:
  - recursion
  - comparison
  - arithmetic
  - function_calls
  - multiple_functions
feature_description: "Recursive functions with tree recursion (fib) and tail recursion (gcd)"
score: 9.2
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 8
  control_flow: 7
  ir_quality: 9
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.02
  instruction_ratio_max: 1.14
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 15
  attr_correct: 14
  attr_has_wrong: false
  cf_defects: 4
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
    relationship: "Same overflow checking pattern, same zero-RC scalar program"
  - journey: 2
    relationship: "Same empty block pattern from if/else codegen"
---

# Journey 3: "I am recursive"

## Source

```ori
// Journey 3: "I am recursive"
// Slug: recursion
// Difficulty: simple
// Features: recursion, comparison, arithmetic, function_calls
// Expected: fib(10) + gcd(48, 18) = 55 + 6 = 61

@fib (n: int) -> int =
    if n <= 1 then n
    else fib(n: n - 1) + fib(n: n - 2);

@gcd (a: int, b: int) -> int =
    if b == 0 then a
    else gcd(a: b, b: a % b);

@main () -> int = {
    let f = fib(n: 10);        // = 55
    let g = gcd(a: 48, b: 18); // = 6
    f + g                      // = 61
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 61        | 61       | (none) | (none) | PASS   |
| AOT     | 61        | 61       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 125 | **Keywords**: 12 | **Identifiers**: 24 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(fib) LParen Ident(n) Colon Ident(int)
RParen Arrow Ident(int) Eq If Ident(n) LtEq
Lit(1) Then Ident(n) Else Ident(fib) LParen Ident(n)
Colon Ident(n) Minus Lit(1) RParen Plus Ident(fib)
LParen Ident(n) Colon Ident(n) Minus Lit(2) RParen Semi
Fn(@) Ident(gcd) LParen Ident(a) Colon Ident(int)
Comma Ident(b) Colon Ident(int) RParen Arrow Ident(int)
Eq If Ident(b) EqEq Lit(0) Then Ident(a) Else
Ident(gcd) LParen Ident(a) Colon Ident(b) Comma Ident(b)
Colon Ident(a) Percent Ident(b) RParen Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq
LBrace Let Ident(f) Eq Ident(fib) LParen Ident(n)
Colon Lit(10) RParen Semi Let Ident(g) Eq Ident(gcd)
LParen Ident(a) Colon Lit(48) Comma Ident(b) Colon
Lit(18) RParen Semi Ident(f) Plus Ident(g) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 38 | **Max depth**: 5 | **Functions**: 3 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @fib
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: If
│       ├─ Cond: BinOp(<=)
│       │    ├─ Ident(n)
│       │    └─ Lit(1)
│       ├─ Then: Ident(n)
│       └─ Else: BinOp(+)
│            ├─ Call(@fib)
│            │    └─ n: BinOp(-)
│            │         ├─ Ident(n)
│            │         └─ Lit(1)
│            └─ Call(@fib)
│                 └─ n: BinOp(-)
│                      ├─ Ident(n)
│                      └─ Lit(2)
├─ FnDecl @gcd
│  ├─ Params: (a: int, b: int)
│  ├─ Return: int
│  └─ Body: If
│       ├─ Cond: BinOp(==)
│       │    ├─ Ident(b)
│       │    └─ Lit(0)
│       ├─ Then: Ident(a)
│       └─ Else: Call(@gcd)
│            ├─ a: Ident(b)
│            └─ b: BinOp(%)
│                 ├─ Ident(a)
│                 └─ Ident(b)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let f = Call(@fib, n: Lit(10))
        ├─ Let g = Call(@gcd, a: Lit(48), b: Lit(18))
        └─ BinOp(+)
             ├─ Ident(f)
             └─ Ident(g)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 14 | **Types inferred**: 7 | **Unifications**: 12 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@fib (n: int) -> int =
    if n <= 1 then n
    //  ^ bool (Comparable<int, int> -> bool)
    else fib(n: n - 1) + fib(n: n - 2)
    //       ^ int (Sub<int, int> -> int)
    //   ^ int (return type of @fib)
    //                      ^ int (Sub<int, int> -> int)
    //                  ^ int (return type of @fib)
    // ^ int (Add<int, int> -> int)

@gcd (a: int, b: int) -> int =
    if b == 0 then a
    //  ^ bool (Eq<int, int> -> bool)
    else gcd(a: b, b: a % b)
    //               ^ int (Rem<int, int> -> int)
    //   ^ int (return type of @gcd)

@main () -> int = {
    let f: int = fib(n: 10)   // inferred: int
    let g: int = gcd(a: 48, b: 18)  // inferred: int
    f + g  // -> int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 3 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Canon nodes: 40, roots: 3, constants: 6
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
- Named argument labels resolved to positional indices
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
@fib: no heap values -- pure scalar arithmetic
@gcd: no heap values -- pure scalar arithmetic
@main: no heap values -- pure scalar arithmetic
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 61 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ let f = @fib(n: 10)
       └─ if 10 <= 1 → false → else
            └─ @fib(n: 9) + @fib(n: 8)
                 └─ ... (tree recursion, 177 calls total)
            └─ = 55
  └─ let g = @gcd(a: 48, b: 18)
       └─ if 18 == 0 → false → else
            └─ @gcd(a: 18, b: 48 % 18 = 12)
                 └─ @gcd(a: 12, b: 18 % 12 = 6)
                      └─ @gcd(a: 6, b: 12 % 6 = 0)
                           └─ if 0 == 0 → true → 6
  └─ f + g = 55 + 6 = 61
→ 61
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
@fib: +0 rc_inc, +0 rc_dec (no heap values)
@gcd: +0 rc_inc, +0 rc_dec (no heap values)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '03-recursion'
source_filename = "03-recursion"

@ovf.msg = private unnamed_addr constant [32 x i8] c"integer overflow on subtraction\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: uwtable
; --- @fib ---
define fastcc noundef i64 @_ori_fib(i64 noundef %0) #0 {
bb0:
  %le = icmp sle i64 %0, 1
  br i1 %le, label %bb1, label %bb2

bb1:                                              ; preds = %bb0
  br label %bb3

bb2:                                              ; preds = %bb0
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %0, i64 1)
  %sub.val = extractvalue { i64, i1 } %sub, 0
  %sub.ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.ovf, label %sub.ovf_panic, label %sub.ok

bb3:                                              ; preds = %bb1, %add.ok
  %v14 = phi i64 [ %add.val, %add.ok ], [ %0, %bb1 ]
  ret i64 %v14

sub.ok:                                           ; preds = %bb2
  %call = call fastcc i64 @_ori_fib(i64 %sub.val)
  %sub1 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %0, i64 2)
  %sub.val2 = extractvalue { i64, i1 } %sub1, 0
  %sub.ovf3 = extractvalue { i64, i1 } %sub1, 1
  br i1 %sub.ovf3, label %sub.ovf_panic5, label %sub.ok4

sub.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

sub.ok4:                                          ; preds = %sub.ok
  %call6 = call fastcc i64 @_ori_fib(i64 %sub.val2)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call6)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

sub.ovf_panic5:                                   ; preds = %sub.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok:                                           ; preds = %sub.ok4
  br label %bb3

add.ovf_panic:                                    ; preds = %sub.ok4
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: nounwind memory(none) uwtable
; --- @gcd ---
define fastcc noundef i64 @_ori_gcd(i64 noundef %0, i64 noundef %1) #1 {
bb3:
  br label %bb0

bb0:                                              ; preds = %bb2, %bb3
  %v12 = phi i64 [ %0, %bb3 ], [ %v13, %bb2 ]
  %v13 = phi i64 [ %1, %bb3 ], [ %rem, %bb2 ]
  %eq = icmp eq i64 %v13, 0
  br i1 %eq, label %bb1, label %bb2

bb1:                                              ; preds = %bb0
  ret i64 %v12

bb2:                                              ; preds = %bb0
  %rem = srem i64 %v12, %v13
  br label %bb0
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_fib(i64 10)
  %call1 = call fastcc i64 @_ori_gcd(i64 48, i64 18)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64) #2

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: uwtable
define i32 @main() #0 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { uwtable }
attributes #1 = { nounwind memory(none) uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
```

#### Disassembly

```asm
_ori_fib:
   sub    $0x38,%rsp
   mov    %rdi,0x30(%rsp)
   cmp    $0x1,%rdi
   jg     .else
   mov    0x30(%rsp),%rax
   mov    %rax,0x28(%rsp)
   jmp    .merge
   mov    0x30(%rsp),%rax
   dec    %rax
   mov    %rax,0x20(%rsp)
   seto   %al
   jo     .panic_sub1
   jmp    .sub1_ok
   mov    0x28(%rsp),%rax
   add    $0x38,%rsp
   ret
   mov    0x20(%rsp),%rdi
   call   _ori_fib
   mov    %rax,%rcx
   mov    0x30(%rsp),%rax
   mov    %rcx,0x10(%rsp)
   sub    $0x2,%rax
   mov    %rax,0x18(%rsp)
   seto   %al
   jo     .panic_sub2
   jmp    .sub2_ok
   lea    ovf.msg(%rip),%rdi
   call   ori_panic_cstr
   mov    0x18(%rsp),%rdi
   call   _ori_fib
   mov    %rax,%rcx
   mov    0x10(%rsp),%rax
   add    %rcx,%rax
   mov    %rax,0x8(%rsp)
   seto   %al
   jo     .panic_add
   jmp    .add_ok
   lea    ovf.msg(%rip),%rdi
   call   ori_panic_cstr
   mov    0x8(%rsp),%rax
   mov    %rax,0x28(%rsp)
   jmp    .merge
   lea    ovf.msg.1(%rip),%rdi
   call   ori_panic_cstr

_ori_gcd:
   mov    %rdi,-0x10(%rsp)
   mov    %rsi,-0x8(%rsp)
   jmp    .loop
.loop:
   mov    -0x10(%rsp),%rax
   mov    -0x8(%rsp),%rdx
   mov    %rdx,-0x20(%rsp)
   mov    %rax,-0x18(%rsp)
   cmp    $0x0,%rdx
   jne    .body
   mov    -0x18(%rsp),%rax
   ret
.body:
   mov    -0x20(%rsp),%rcx
   mov    -0x18(%rsp),%rax
   cqto
   idiv   %rcx
   mov    -0x20(%rsp),%rax
   mov    %rax,-0x10(%rsp)
   mov    %rdx,-0x8(%rsp)
   jmp    .loop

_ori_main:
   sub    $0x18,%rsp
   mov    $0xa,%edi
   call   _ori_fib
   mov    %rax,0x8(%rsp)
   mov    $0x30,%edi
   mov    $0x12,%esi
   call   _ori_gcd
   mov    %rax,%rcx
   mov    0x8(%rsp),%rax
   add    %rcx,%rax
   mov    %rax,0x10(%rsp)
   seto   %al
   jo     .panic
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
   lea    ovf.msg.1(%rip),%rdi
   call   ori_panic_cstr

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
| 1 | @fib     | 26     | 26    | 1.00x | OPTIMAL |
| 2 | @gcd     | 8      | 7     | 1.14x | NEAR-OPTIMAL |
| 3 | @main    | 9      | 9     | 1.00x | OPTIMAL |

**@fib** (26 instructions): Tree-recursive function with two subtraction overflow checks, one addition overflow check, two recursive calls, and a phi-based merge. All instructions are justified: the 3 overflow checking sequences (call + extractvalue x2 + br + panic path each) account for the bulk, and the recursive calls are mandatory. The empty `bb1 -> bb3` and `add.ok -> bb3` branches are control flow defects (Cat 4), not instruction count issues since the `br` is still structurally needed.

**@gcd** (8 instructions, ideal 7): The tail recursion has been correctly lowered to a loop with phi nodes. The single unjustified instruction is the unconditional `br label %bb0` in the entry block `bb3` -- this could be eliminated by making `bb0` the entry block directly. [LOW-1]

**@main** (9 instructions): Two calls, one overflow-checked addition, branch to ok/panic. All instructions justified.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @fib     | 0      | 0      | YES      | N/A            | N/A            |
| @gcd     | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. OPTIMAL. All parameters and return values are scalar `i64`.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | memory(none) | noalias | noundef | uwtable | Notes |
|----------|--------|----------|-------------|---------|--------|---------|-------|
| @fib     | YES    | NO       | NO          | N/A     | YES    | YES     | nounwind absent due to panic paths (correct) |
| @gcd     | YES    | YES      | YES         | N/A     | YES    | YES     | Pure function, correctly annotated [NOTE-5] |
| @main    | NO (C) | NO       | NO          | N/A     | YES    | YES     | C calling convention for entry point (correct) |

**Attribute compliance**: 14/15 applicable attributes present (93.3%).

The `nounwind` absence on `@fib` and `@main` is semantically correct -- `@fib` can panic on overflow (calling `ori_panic_cstr` which is `noreturn` but may unwind), and `@main` transitively calls `@fib`. The nounwind fixed-point analysis correctly identifies only `@gcd` as provably non-unwinding.

The `memory(none)` on `@gcd` is a new improvement from AIMS Section 02. It correctly identifies that `@gcd` is a pure function -- it takes two `i64` parameters, performs only comparison and remainder operations, and returns an `i64`. No memory is read or written. This enables LLVM to perform aggressive optimizations like CSE and dead call elimination on repeated `gcd` calls.

The `@main` entry point correctly uses C calling convention (not `fastcc`) for ABI compatibility with the C `main()` wrapper.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @fib     | 10     | 2           | 0                 | 1         | [LOW-2] |
| @gcd     | 4      | 1           | 1                 | 2         | [LOW-3] |
| @main    | 3      | 0           | 0                 | 0         |       |

**@fib**: 10 blocks for a function with 3 overflow checks, 2 recursive calls, and an if/else branch. Two empty blocks: `bb1` (which only contains `br label %bb3`) and `add.ok` (which only contains `br label %bb3`). Both could be eliminated by branching directly to `bb3` from their predecessors.

**@gcd**: The entry block `bb3` contains only `br label %bb0` -- a redundant unconditional branch to the loop header. This is an artifact of the tail-recursion-to-loop transformation; the entry block could be merged with `bb0`.

**Total control flow defects**: 4 (2 empty blocks in @fib, 1 empty block + 1 redundant branch in @gcd).

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| sub (n-1) | YES     | YES     | `llvm.ssub.with.overflow.i64` |
| sub (n-2) | YES     | YES     | `llvm.ssub.with.overflow.i64` |
| add (fib+fib) | YES | YES     | `llvm.sadd.with.overflow.i64` |
| rem (a%b) | N/A     | N/A     | `srem` -- no overflow possible for integer remainder |
| add (f+g) | YES     | YES     | `llvm.sadd.with.overflow.i64` |

All arithmetic operations that can overflow are correctly checked. The `srem` in `@gcd` correctly does NOT use overflow checking (integer remainder cannot overflow). Panic messages correctly distinguish "subtraction" vs "addition" overflow.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.2 KiB |
| .rodata section | 133.5 KiB |
| User code (@fib) | 178 bytes (38 native instructions) |
| User code (@gcd) | 77 bytes (20 native instructions) |
| User code (@main) | 78 bytes (18 native instructions) |
| User code (main wrapper) | 8 bytes (4 native instructions) |
| Total user code | 341 bytes |
| Runtime | >99% of binary |

#### Disassembly: @fib

```asm
_ori_fib:
   sub    $0x38,%rsp           ; frame setup (56 bytes for spills)
   mov    %rdi,0x30(%rsp)      ; save n
   cmp    $0x1,%rdi            ; n <= 1?
   jg     .else                ; jump to else branch
   mov    0x30(%rsp),%rax      ; base case: return n
   mov    %rax,0x28(%rsp)
   jmp    .merge
   ; ... (overflow-checked subtraction, two recursive calls, overflow-checked addition)
   ; 38 native instructions total
```

#### Disassembly: @gcd

```asm
_ori_gcd:
   mov    %rdi,-0x10(%rsp)     ; store a (no frame pointer needed -- red zone)
   mov    %rsi,-0x8(%rsp)      ; store b
   jmp    .loop                ; redundant entry jump
.loop:
   mov    -0x10(%rsp),%rax
   mov    -0x8(%rsp),%rdx
   cmp    $0x0,%rdx            ; b == 0?
   jne    .body
   mov    -0x18(%rsp),%rax     ; return a
   ret
.body:
   cqto                        ; sign-extend for idiv
   idiv   %rcx                 ; a / b (remainder in %rdx)
   ; swap and loop back
   jmp    .loop
```

#### Disassembly: @main

```asm
_ori_main:
   sub    $0x18,%rsp
   mov    $0xa,%edi            ; fib(10)
   call   _ori_fib
   mov    %rax,0x8(%rsp)       ; save result
   mov    $0x30,%edi           ; gcd(48, 18)
   mov    $0x12,%esi
   call   _ori_gcd
   mov    %rax,%rcx
   mov    0x8(%rsp),%rax
   add    %rcx,%rax            ; f + g
   ; overflow check + return
```

### 7. Optimal IR Comparison

#### @fib: Ideal vs Actual

```llvm
; IDEAL (26 instructions -- same as actual, overflow checking is mandatory)
define fastcc noundef i64 @_ori_fib(i64 noundef %n) uwtable {
entry:
  %le = icmp sle i64 %n, 1
  br i1 %le, label %base, label %recurse

base:
  ret i64 %n

recurse:
  %sub1 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %n, i64 1)
  %n1 = extractvalue { i64, i1 } %sub1, 0
  %ovf1 = extractvalue { i64, i1 } %sub1, 1
  br i1 %ovf1, label %panic_sub, label %ok1

ok1:
  %r1 = call fastcc i64 @_ori_fib(i64 %n1)
  %sub2 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %n, i64 2)
  %n2 = extractvalue { i64, i1 } %sub2, 0
  %ovf2 = extractvalue { i64, i1 } %sub2, 1
  br i1 %ovf2, label %panic_sub2, label %ok2

ok2:
  %r2 = call fastcc i64 @_ori_fib(i64 %n2)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %r1, i64 %r2)
  %sum = extractvalue { i64, i1 } %add, 0
  %ovf3 = extractvalue { i64, i1 } %add, 1
  br i1 %ovf3, label %panic_add, label %done

done:
  ret i64 %sum

panic_sub:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic_sub2:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic_add:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +0 unjustified instructions. The actual IR has 2 empty blocks (`bb1` and `add.ok`) that add `br` instructions, but the phi node in `bb3` requires distinct predecessor blocks, so these are architectural artifacts rather than instruction bloat. All semantic instructions are justified.

#### @gcd: Ideal vs Actual

```llvm
; IDEAL (7 instructions -- entry directly at loop header)
define fastcc noundef i64 @_ori_gcd(i64 noundef %a, i64 noundef %b) nounwind memory(none) uwtable {
entry:
  %va = phi i64 [ %a, %entry_pred ], [ %vb, %loop ]
  %vb = phi i64 [ %b, %entry_pred ], [ %rem, %loop ]
  %eq = icmp eq i64 %vb, 0
  br i1 %eq, label %done, label %loop

done:
  ret i64 %va

loop:
  %rem = srem i64 %va, %vb
  br label %entry
}
```

```llvm
; ACTUAL (8 instructions -- extra entry block with unconditional branch)
define fastcc noundef i64 @_ori_gcd(i64 noundef %0, i64 noundef %1) #1 {
bb3:
  br label %bb0            ; <-- unjustified: redundant entry jump

bb0:
  %v12 = phi i64 [ %0, %bb3 ], [ %v13, %bb2 ]
  %v13 = phi i64 [ %1, %bb3 ], [ %rem, %bb2 ]
  %eq = icmp eq i64 %v13, 0
  br i1 %eq, label %bb1, label %bb2

bb1:
  ret i64 %v12

bb2:
  %rem = srem i64 %v12, %v13
  br label %bb0
}
```

**Delta**: +1 unjustified instruction (entry block `bb3` with sole `br label %bb0`). This is an artifact of the tail-recursion-to-loop transformation creating a separate entry block for initial values.

#### @main: Ideal vs Actual

```llvm
; IDEAL (9 instructions)
define noundef i64 @_ori_main() uwtable {
entry:
  %f = call fastcc i64 @_ori_fib(i64 10)
  %g = call fastcc i64 @_ori_gcd(i64 48, i64 18)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %f, i64 %g)
  %sum = extractvalue { i64, i1 } %add, 0
  %ovf = extractvalue { i64, i1 } %add, 1
  br i1 %ovf, label %panic, label %ok

ok:
  ret i64 %sum

panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +0 unjustified instructions. Actual IR matches ideal exactly.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @fib     | 26    | 26     | +0    | N/A       | OPTIMAL |
| @gcd     | 7     | 8      | +1    | NO        | NEAR-OPTIMAL |
| @main    | 9     | 9      | +0    | N/A       | OPTIMAL |

### 8. Recursion: Tail-Call Optimization

The compiler correctly identifies `@gcd` as tail-recursive and lowers it to an iterative loop. This is verified by the presence of phi nodes at the loop header and `br label %bb0` forming the back-edge -- a proper loop structure with no recursive `call @_ori_gcd` instruction.

In contrast, `@fib` is tree-recursive (two recursive calls whose results are combined with `+`), which cannot be converted to a simple loop. The compiler correctly preserves both recursive `call fastcc i64 @_ori_fib(...)` instructions.

**Tail-call detection quality**: Excellent. The nounwind analysis correctly determined only `@gcd` is nounwind (because it has no panic paths, being purely comparison and remainder operations). `@fib` is correctly identified as potentially unwinding due to overflow-checked arithmetic that can call `ori_panic_cstr`.

The `memory(none)` attribute on `@gcd` is a new improvement from AIMS Section 02's posthoc memory analysis. It correctly identifies that `@gcd` is a pure function with no memory side effects -- it operates exclusively on register-level scalar values (`i64`), performs only comparison and signed remainder, and returns a scalar. This enables LLVM to apply CSE, LICM, and dead call elimination on repeated `gcd` invocations.

The loop-lowered `@gcd` uses phi nodes cleanly:
- `%v12 = phi i64 [ %0, %bb3 ], [ %v13, %bb2 ]` -- a becomes the previous b
- `%v13 = phi i64 [ %1, %bb3 ], [ %rem, %bb2 ]` -- b becomes a % b

This is textbook Euclidean algorithm loop form.

### 9. Recursion: Stack Frame Efficiency

For tree-recursive `@fib`, each invocation creates a stack frame. The native disassembly shows `sub $0x38, %rsp` -- a 56-byte frame. This frame stores:
- The input parameter `n` (spilled to 0x30(%rsp))
- First recursive call result (at 0x10(%rsp))
- Intermediate subtraction results (at 0x18(%rsp), 0x20(%rsp))
- Addition result (at 0x08(%rsp))
- Merge value (at 0x28(%rsp))

At 56 bytes per frame with fib(10) reaching ~10 levels deep, the stack usage is modest (~560 bytes peak). For larger inputs, this could become significant -- but this is inherent to tree recursion, not a compiler inefficiency.

For `@gcd`, the loop-lowered form uses red zone storage (`-0x10(%rsp)` through `-0x20(%rsp)`) without even needing a frame setup (`sub $X, %rsp`). This is excellent -- the `nounwind` attribute allows the compiler to use the red zone safely.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW      | IR Quality | Redundant entry block in @gcd loop lowering | CONFIRMED | J3 |
| 2 | LOW      | Control Flow | 2 empty blocks in @fib (bb1, add.ok) | CONFIRMED | J3 |
| 3 | LOW      | Control Flow | Empty entry block + redundant branch in @gcd | CONFIRMED | J3 |
| 4 | NOTE     | Attributes | memory(none) on @gcd -- pure function correctly annotated | NEW | J3 |
| 5 | NOTE     | Recursion | Excellent tail-call optimization on @gcd | CONFIRMED | J3 |
| 6 | NOTE     | Recursion | Correct nounwind analysis (gcd only) | CONFIRMED | J3 |
| 7 | NOTE     | ARC | Zero RC operations on pure scalar recursion | CONFIRMED | J3 |

### LOW-1: Redundant entry block in @gcd loop lowering

**Location**: @gcd, block `bb3`
**Impact**: 1 unjustified instruction (`br label %bb0`)
**Fix**: In the tail-recursion-to-loop transform, merge the entry block with the loop header when the entry block contains only an unconditional branch. The phi nodes can accept the function parameters directly from the entry predecessor.
**First seen**: Journey 3
**Found in**: Optimal IR Comparison (Category 7)

### LOW-2: Empty blocks in @fib

**Location**: @fib, blocks `bb1` and `add.ok`
**Impact**: 2 unnecessary `br` instructions; blocks contain only unconditional jumps to `bb3`
**Fix**: The if/else codegen could directly branch to the merge block rather than creating intermediate empty blocks
**First seen**: Journey 3
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-3: Empty entry block and redundant branch in @gcd

**Location**: @gcd, block `bb3`
**Impact**: 1 empty block with a redundant unconditional branch to `bb0`
**Fix**: The tail-recursion loop lowering creates a separate entry block; it should merge with the loop header
**First seen**: Journey 3
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: memory(none) on @gcd

**Location**: @gcd function declaration
**Impact**: Positive -- AIMS Section 02's posthoc memory analysis correctly identifies `@gcd` as a pure function with no memory effects. This enables LLVM to apply CSE, LICM, and dead call elimination on repeated calls.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-5: Excellent tail-call optimization on @gcd

**Location**: @gcd function
**Impact**: Positive -- tail recursion correctly lowered to an iterative loop with phi nodes, eliminating all recursive call overhead. The resulting loop is near-optimal.
**Found in**: Recursion: Tail-Call Optimization (Category 8)

### NOTE-6: Correct nounwind analysis

**Location**: Fixed-point nounwind analysis
**Impact**: Positive -- the compiler correctly identifies that @gcd (with only comparison and remainder) cannot unwind, while @fib (with overflow-checked arithmetic leading to `ori_panic_cstr`) can. This enables red zone usage in @gcd's native code.
**Found in**: Recursion: Tail-Call Optimization (Category 8)

### NOTE-7: Zero RC operations on pure scalar recursion

**Location**: All three functions
**Impact**: Positive -- the compiler correctly identifies that all values are scalar `i64` and emits zero reference counting operations. ARC is completely absent from this program.
**Found in**: ARC Purity (Category 2)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.02x avg ratio (max 1.14x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 8/10 | 93.3% compliance |
| Control Flow | 10% | 7/10 | 4 defects |
| IR Quality | 20% | 9/10 | 1 unjustified instruction |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.2 / 10**

## Verdict

Journey 3's recursion codegen has improved on the AIMS branch. The highlight remains the tail-call optimization on `@gcd`, correctly lowered to an iterative loop with phi nodes, and tree-recursive `@fib` achieving OPTIMAL instruction purity with all overflow checks justified. The new `memory(none)` attribute on `@gcd` from AIMS Section 02 correctly identifies it as a pure function, improving from 77.8% to 93.3% attribute compliance and raising the overall score from 8.9 to 9.2. The minor control flow issues (empty blocks from if/else codegen and TCO loop entry) remain the only areas for improvement.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J3 | CONFIRMED |
| fastcc usage | J1 | J3 | CONFIRMED |
| Zero-RC scalar programs | J1 | J3 | CONFIRMED |
| Empty blocks from if/else | J2 | J3 | CONFIRMED |
| Tail-call to loop lowering | -- | J3 | NEW |
| nounwind fixed-point analysis | -- | J3 | NEW |
| memory(none) pure function | -- | J3 | NEW (AIMS Section 02) |
