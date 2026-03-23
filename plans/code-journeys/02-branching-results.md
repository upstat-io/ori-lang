---
journey: 2
slug: branching
theme: "I am a branch"
date: 2026-03-20
status: PASS
expected: 17
eval_result: 17
aot_result: 17
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of conditional expressions"
learning_objectives:
  - "See how if/then/else lowers to LLVM branches, phi nodes, and select"
  - "Understand overflow-checked negation via llvm.ssub.with.overflow"
  - "Compare ideal vs actual codegen for branching functions"
  - "Learn how the compiler optimizes simple conditionals to branchless select"
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
    relationship: "Same overflow checking pattern, first journey with nounwind/memory(none) confirmed"
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
    let a = my_abs(n: -7);
    let b = my_max(a: 3, b: 10);
    let c = my_sign(n: 0);
    a + b + c
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

**Tokens**: 142 | **Keywords**: 16 | **Identifiers**: 28 | **Errors**: 0

<details>
<summary>Token stream (user code)</summary>

```text
Fn(@) Ident(my_abs) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Lt Int(0) Then Minus
Ident(n) Else Ident(n) Semi
Fn(@) Ident(my_max) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(int) Eq If
Ident(a) Gt Ident(b) Then Ident(a) Else Ident(b) Semi
Fn(@) Ident(my_sign) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq If Ident(n) Gt Int(0) Then Int(1)
Else LParen If Ident(n) Lt Int(0) Then Minus Int(1)
Else Int(0) RParen Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(a) Eq Ident(my_abs) LParen Ident(n) Colon Minus
Int(7) RParen Semi Let Ident(b) Eq Ident(my_max) LParen
Ident(a) Colon Int(3) Comma Ident(b) Colon Int(10) RParen Semi
Let Ident(c) Eq Ident(my_sign) LParen Ident(n) Colon Int(0)
RParen Semi Ident(a) Plus Ident(b) Plus Ident(c) RBrace
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
//                                        ^ int (parameter n)

@my_max (a: int, b: int) -> int = if a > b then a else b
//                                   ^ bool (Gt<int, int> -> bool)
//                                          ^ int  ^ int

@my_sign (n: int) -> int =
    if n > 0 then 1              // ^ bool, then: int literal
    else (if n < 0 then -1 else 0)  // nested if: bool, int, int

@main () -> int = {
    let a: int = my_abs(n: -7)     // inferred from return type
    let b: int = my_max(a: 3, b: 10)
    let c: int = my_sign(n: 0)
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
- 4 function bodies lowered to canonical expression form
- Call arguments normalized to positional order
- Nested if/else in @my_sign preserved as single expression tree
- Canon nodes: 43 (user module), 46 (prelude)
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
@my_abs: no heap values — pure scalar arithmetic + comparison
@my_max: no heap values — pure scalar comparison
@my_sign: no heap values — pure scalar comparison
@main: no heap values — pure scalar arithmetic + function calls
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
  │    └─ if -7 < 0 → true → -(-7) = 7
  ├─ let b = @my_max(a: 3, b: 10)
  │    └─ if 3 > 10 → false → 10
  ├─ let c = @my_sign(n: 0)
  │    └─ if 0 > 0 → false → if 0 < 0 → false → 0
  └─ a + b + c = 7 + 10 + 0 = 17
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

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

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
   jge    1b12b                    ; skip negation if n >= 0
   mov    0x8(%rsp),%rcx
   xor    %eax,%eax
   sub    %rcx,%rax                ; 0 - n
   seto   %cl                      ; overflow flag
   test   $0x1,%cl
   mov    %rax,0x10(%rsp)
   jne    1b135                    ; panic on overflow
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
   lea    0xd31c0(%rip),%rdi       ; "integer overflow on negation"
   call   ori_panic_cstr

_ori_my_max:
   mov    %rsi,%rax                ; rax = b
   cmp    %rax,%rdi                ; compare a, b
   cmovg  %rdi,%rax                ; if a > b, rax = a
   ret                             ; 4 instructions — branchless!

_ori_my_sign:
   mov    %rdi,-0x10(%rsp)
   mov    $0x1,%eax                ; default = 1
   cmp    $0x0,%rdi
   mov    %rax,-0x8(%rsp)
   jg     1b190                    ; if n > 0, return 1
   mov    -0x10(%rsp),%rdx
   xor    %eax,%eax                ; 0
   mov    $0xffffffffffffffff,%rcx ; -1
   cmp    $0x0,%rdx
   cmovl  %rcx,%rax                ; if n < 0, rax = -1
   mov    %rax,-0x8(%rsp)
   mov    -0x8(%rsp),%rax
   ret

_ori_main:
   sub    $0x28,%rsp
   mov    $0xfffffffffffffff9,%rdi ; -7
   call   _ori_my_abs
   mov    %rax,0x10(%rsp)
   mov    $0x3,%edi                ; 3
   mov    $0xa,%esi                ; 10
   call   _ori_my_max
   mov    %rax,0x8(%rsp)
   xor    %eax,%eax
   mov    %eax,%edi                ; 0
   call   _ori_my_sign
   mov    0x8(%rsp),%rcx
   mov    %rax,%rdx
   mov    0x10(%rsp),%rax
   mov    %rdx,0x18(%rsp)
   add    %rcx,%rax                ; a + b
   mov    %rax,0x20(%rsp)
   seto   %al
   jo     1b209                    ; overflow check
   mov    0x18(%rsp),%rcx
   mov    0x20(%rsp),%rax
   add    %rcx,%rax                ; (a + b) + c
   mov    %rax,(%rsp)
   seto   %al
   jo     1b21e                    ; overflow check
   jmp    1b215
   lea    (%rip),%rdi              ; "integer overflow on addition"
   call   ori_panic_cstr
   mov    (%rsp),%rax
   add    $0x28,%rsp
   ret
   lea    (%rip),%rdi
   call   ori_panic_cstr
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @my_abs  | 10     | 10    | 1.00x | OPTIMAL |
| 2 | @my_max  | 3      | 3     | 1.00x | OPTIMAL |
| 3 | @my_sign | 7      | 7     | 1.00x | OPTIMAL |
| 4 | @main    | 16     | 16    | 1.00x | OPTIMAL |

Every instruction is justified:
- **@my_abs**: `icmp` + `br` (condition), `ssub.with.overflow` + 2x `extractvalue` + `br` (overflow-checked negation), `phi` + `ret` (merge), `call` + `unreachable` (panic path). All necessary for safe negation.
- **@my_max**: `icmp` + `select` + `ret`. Branchless -- the compiler recognized that both arms are trivial values and emitted `select` instead of `br`/`phi`. This is the ideal lowering.
- **@my_sign**: `icmp` + `br` (outer), `icmp` + `select` + `br` (inner), `phi` + `ret`. The nested if/else compiles to a hybrid: branch for the outer, branchless select for the inner. Optimal because the outer branch skips the second comparison entirely.
- **@main**: 3 calls + 2 overflow-checked additions (each: `sadd.with.overflow` + 2x `extractvalue` + `br` + panic). All justified.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @my_abs  | 0      | 0      | YES      | N/A            | N/A            |
| @my_max  | 0      | 0      | YES      | N/A            | N/A            |
| @my_sign | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: No heap values. Zero RC operations. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | memory(none) | noundef | cold | Notes |
|----------|--------|----------|-------------|---------|------|-------|
| @my_abs  | YES    | YES      | YES         | YES     | NO   | Pure function -- correct |
| @my_max  | YES    | YES      | YES         | YES     | NO   | Pure function -- correct |
| @my_sign | YES    | YES      | YES         | YES     | NO   | Pure function -- correct |
| @main    | NO     | YES      | NO          | YES     | NO   | C calling convention (correct for entry) |
| @ori_panic_cstr | N/A | N/A | N/A        | N/A     | YES  | cold + noreturn -- correct |

All 20 applicable attribute checks pass. The three pure functions correctly receive `memory(none)` (they perform no memory operations -- only register-level arithmetic and comparisons). `@_ori_main` correctly omits `memory(none)` because it calls other functions. The `fastcc` vs C convention split between user functions and the entry point wrapper is correct.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @my_abs  | 4      | 0           | 0                 | 1         | phi merges normal/negated paths |
| @my_max  | 1      | 0           | 0                 | 0         | Branchless select [NOTE-1] |
| @my_sign | 3      | 0           | 0                 | 1         | Hybrid: branch + select |
| @main    | 5      | 0           | 0                 | 0         | Linear with overflow checks |

Zero defects. Block layout is clean with no empty blocks or redundant branches. The control flow graph is well-structured.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| negation  | YES     | YES     | Uses llvm.ssub.with.overflow (0 - n), panics on INT_MIN |
| add (1st) | YES     | YES     | Uses llvm.sadd.with.overflow for a + b |
| add (2nd) | YES     | YES     | Uses llvm.sadd.with.overflow for (a+b) + c |

All arithmetic operations are overflow-checked. The negation correctly uses subtraction-from-zero with overflow detection, which catches the INT_MIN edge case (-(-9223372036854775808) would overflow).

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.26 MiB (debug) |
| .text section | 869.6 KiB |
| .rodata section | 133.5 KiB |
| User code | 302 bytes (5 functions) |
| Runtime | ~99.97% of .text |

#### Disassembly: @my_abs

```asm
_ori_my_abs:                       ; 66 bytes
   sub    $0x18,%rsp               ; stack frame
   cmp    $0x0,%rdi                ; n < 0?
   jge    .ret                     ; skip negation
   xor    %eax,%eax
   sub    %rcx,%rax                ; 0 - n
   seto   %cl                      ; overflow?
   test   $0x1,%cl
   jne    .panic                   ; panic on overflow
.ret:
   mov    0x10(%rsp),%rax
   add    $0x18,%rsp
   ret
```

#### Disassembly: @my_max

```asm
_ori_my_max:                       ; 11 bytes
   mov    %rsi,%rax                ; rax = b
   cmp    %rax,%rdi                ; a > b?
   cmovg  %rdi,%rax                ; if so, rax = a
   ret                             ; 4 instructions — branchless!
```

#### Disassembly: @my_sign

```asm
_ori_my_sign:                      ; 54 bytes
   mov    $0x1,%eax                ; default = 1
   cmp    $0x0,%rdi                ; n > 0?
   jg     .ret                     ; return 1
   xor    %eax,%eax                ; default = 0
   mov    $0xffffffffffffffff,%rcx ; -1
   cmp    $0x0,%rdx                ; n < 0?
   cmovl  %rcx,%rax                ; if so, rax = -1
.ret:
   ret
```

### 7. Optimal IR Comparison

#### @my_abs: Ideal vs Actual

```llvm
; IDEAL (10 instructions — overflow-checked negation)
define fastcc noundef i64 @_ori_my_abs(i64 noundef %0) nounwind memory(none) {
  %lt = icmp slt i64 %0, 0
  br i1 %lt, label %neg, label %ret
neg:
  %sub = call {i64, i1} @llvm.ssub.with.overflow.i64(i64 0, i64 %0)
  %val = extractvalue {i64, i1} %sub, 0
  %ovf = extractvalue {i64, i1} %sub, 1
  br i1 %ovf, label %panic, label %ret
ret:
  %r = phi i64 [%0, %entry], [%val, %neg]
  ret i64 %r
panic:
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

**Delta**: +0 instructions. The compiler correctly emits `select` instead of a branch/phi pattern for this simple conditional. This is the best possible lowering.

#### @my_sign: Ideal vs Actual

```llvm
; IDEAL (7 instructions — branch + select hybrid)
define fastcc noundef i64 @_ori_my_sign(i64 noundef %0) nounwind memory(none) {
  %gt = icmp sgt i64 %0, 0
  br i1 %gt, label %ret, label %inner
inner:
  %lt = icmp slt i64 %0, 0
  %sel = select i1 %lt, i64 -1, i64 0
  br label %ret
ret:
  %r = phi i64 [1, %entry], [%sel, %inner]
  ret i64 %r
}
```

**Delta**: +0 instructions. The compiler uses a branch for the outer if (allowing the inner comparison to be skipped when n > 0) and a select for the inner if (since both branches are constants). This hybrid strategy is optimal.

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions — 3 calls + 2 overflow-checked adds)
define noundef i64 @_ori_main() nounwind {
  %a = call fastcc i64 @_ori_my_abs(i64 -7)
  %b = call fastcc i64 @_ori_my_max(i64 3, i64 10)
  %c = call fastcc i64 @_ori_my_sign(i64 0)
  %add1 = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %v1 = extractvalue {i64, i1} %add1, 0
  %o1 = extractvalue {i64, i1} %add1, 1
  br i1 %o1, label %panic1, label %ok1
ok1:
  %add2 = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %v1, i64 %c)
  %v2 = extractvalue {i64, i1} %add2, 0
  %o2 = extractvalue {i64, i1} %add2, 1
  br i1 %o2, label %panic2, label %ok2
ok2:
  ret i64 %v2
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +0 instructions. Actual matches ideal exactly.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @my_abs  | 10    | 10     | +0    | N/A       | OPTIMAL |
| @my_max  | 3     | 3      | +0    | N/A       | OPTIMAL |
| @my_sign | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 16    | 16     | +0    | N/A       | OPTIMAL |

### 8. Branching: Lowering Strategy

The compiler demonstrates three distinct lowering strategies for `if/then/else`:

1. **Branch + phi** (@my_abs): Used when at least one arm performs computation (negation). The branch allows skipping the expensive arm, and the phi merges the results. This is the classic conditional pattern.

2. **Branchless select** (@my_max): Used when both arms are trivial values (just forwarding parameters). The `select` instruction is a conditional move that avoids a branch entirely. This eliminates branch misprediction overhead and is the optimal lowering for simple value selection.

3. **Hybrid branch + select** (@my_sign): The outer conditional uses a branch (to skip the inner comparison when n > 0), while the inner conditional uses a select (since both values are constants). This hybrid strategy is architecturally sound -- it combines the early-exit benefit of branching with the branch-prediction-friendly nature of select.

The compiler's strategy selection is correct in all three cases. The `select` optimization for @my_max is particularly noteworthy -- many compilers would emit a branch/phi pair, but the Ori compiler recognizes that both arms are trivial and uses the more efficient `select`.

### 9. Branching: Nested Conditionals

The nested `if/else` in `@my_sign` compiles cleanly:

```ori
if n > 0 then 1
else (if n < 0 then -1 else 0)
```

This produces 3 basic blocks (not 4), because the inner if/else uses `select` instead of branching, eliminating one block. The nesting does not introduce any redundant branches, empty blocks, or unnecessary phi nodes. The parenthesization in the source `(if n < 0 then -1 else 0)` is purely syntactic and has no codegen impact.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE     | Control Flow | @my_max compiles to branchless select | NEW | J2 |
| 2 | NOTE     | Control Flow | @my_sign uses optimal hybrid branch+select for nested if | NEW | J2 |
| 3 | NOTE     | Attributes | All pure functions correctly marked memory(none) | NEW | J2 |
| 4 | NOTE     | Attributes | nounwind on all user functions confirmed | CONFIRMED | J1 |

### NOTE-1: Branchless select for @my_max

**Location**: @_ori_my_max function body
**Impact**: Positive -- eliminates branch misprediction, reduces basic blocks from 3 to 1
**Details**: The compiler correctly identifies that `if a > b then a else b` has trivial arms (just forwarding parameters) and emits `select` instead of `br`/`phi`. This produces a single basic block with 3 instructions, which is the optimal lowering.
**First seen**: Journey 2
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-2: Hybrid branch+select for nested conditionals

**Location**: @_ori_my_sign function body
**Impact**: Positive -- combines early-exit branching with branchless inner conditional
**Details**: The outer `if n > 0` uses a branch (allowing the second comparison to be skipped), while the inner `if n < 0 then -1 else 0` uses `select` (since both arms are constants). This produces 3 blocks instead of 4 and is the optimal strategy.
**First seen**: Journey 2
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-3: memory(none) on all pure functions

**Location**: @_ori_my_abs, @_ori_my_max, @_ori_my_sign function declarations
**Impact**: Positive -- allows LLVM to perform aggressive optimizations (CSE, dead call elimination, reordering)
**Details**: All three user functions that perform only scalar arithmetic and comparisons are correctly annotated with `memory(none)`, indicating they have no memory side effects. The `@_ori_main` function correctly omits this attribute since it calls other functions.
**First seen**: Journey 2 (first journey with `memory(none)` analysis)
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-4: nounwind confirmed on all user functions

**Location**: All function declarations
**Impact**: Positive -- LLVM can omit unwind tables, reducing binary size and improving performance
**Details**: All user functions are marked `nounwind`, confirmed working. The `ori_panic_cstr` runtime function is correctly marked `cold noreturn` (not `nounwind`, since it unwinds).
**First seen**: Journey 1
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

Journey 2's branching codegen is flawless. The compiler demonstrates sophisticated lowering strategy selection: branchless `select` for trivial conditionals (@my_max), branch+phi for computed arms (@my_abs), and a hybrid for nested conditionals (@my_sign). All functions receive correct attributes (`nounwind`, `memory(none)`, `noundef`, `fastcc`), overflow checking is present on all arithmetic (negation and addition), and ARC is correctly absent for pure scalar computation. Every instruction in the generated IR is justified -- zero waste.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J2 | CONFIRMED (now includes negation) |
| fastcc usage | J1 | J2 | CONFIRMED |
| nounwind | J1 | J2 | CONFIRMED |
| memory(none) | N/A | J2 | NEW (first journey with pure function analysis) |
| Branchless select | N/A | J2 | NEW (first journey with branching) |

Journey 2 introduces branching as a new feature domain and confirms that the codegen improvements observed since Journey 1 (nounwind attributes) are stable. The `memory(none)` attribute is tested for the first time here, and the branchless `select` optimization demonstrates that the compiler goes beyond basic correctness to produce genuinely efficient code.
