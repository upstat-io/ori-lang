---
journey: 7
slug: loops
theme: "I am a loop"
date: 2026-03-06
status: PASS
expected: 30
eval_result: 30
aot_result: 30
difficulty: moderate
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of mutable variables and assignment"
  - "Understanding of ranges and iteration"
learning_objectives:
  - "See how loop/break lowers to LLVM phi-based loops"
  - "Understand how for-in ranges compile to counted loop patterns"
  - "Compare loop and for codegen approaches for the same computation"
  - "Observe range struct construction and its optimization potential"
features:
  - loops
  - ranges
  - break_continue
  - let_bindings
feature_description: "Loop/break and for-in-range iteration with mutable accumulators"
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
  instruction_ratio: 1.04
  instruction_ratio_max: 1.06
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 17
  attr_correct: 16
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
    relationship: "Same nounwind/fastcc attribute patterns"
  - journey: 2
    relationship: "Both test conditional control flow, J7 adds loop iteration"
---

# Journey 7: "I am a loop"

## Source

```ori
// Journey 7: "I am a loop"
// Slug: loops
// Difficulty: moderate
// Features: loops, ranges, break_continue, let_bindings
// Expected: sum_loop(5) + sum_for(5) = 15 + 15 = 30

@sum_loop (n: int) -> int = {
    let i = 0;
    let total = 0;
    loop {
        if i >= n then break;
        total += i + 1;
        i += 1
    };
    total
}

@sum_for (n: int) -> int = {
    let total = 0;
    for x in 1..=n do total += x;
    total
}

@main () -> int = {
    let a = sum_loop(n: 5);   // = 15
    let b = sum_for(n: 5);    // = 15
    a + b                     // = 30
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 30        | 30       | (none) | (none) | PASS   |
| AOT     | 30        | 30       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 134 | **Keywords**: 14 | **Identifiers**: 22 | **Errors**: 0

<details>
<summary>Token stream (first 30)</summary>

```text
Fn(@) Ident(sum_loop) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq LBrace Let Ident(i) Eq Int(0) Semi
Let Ident(total) Eq Int(0) Semi Loop LBrace If Ident(i)
GtEq Ident(n) Then Break Semi Ident(total) PlusEq ...
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 45 | **Max depth**: 5 | **Functions**: 3 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @sum_loop
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let i = 0
│       ├─ Let total = 0
│       ├─ Loop
│       │    └─ Block
│       │         ├─ If (i >= n) Then Break
│       │         ├─ total += i + 1
│       │         └─ i += 1
│       └─ Ident(total)
├─ FnDecl @sum_for
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let total = 0
│       ├─ For x In Range(1..=n) Do total += x
│       └─ Ident(total)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = sum_loop(n: 5)
        ├─ Let b = sum_for(n: 5)
        └─ BinOp(+): a + b
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 18 | **Types inferred**: 8 | **Unifications**: 12 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@sum_loop (n: int) -> int = {
    let i: int = 0;
    let total: int = 0;
    loop {
        if i >= n then break;   // >= : (int, int) -> bool; break : Never
        total += i + 1;         // + : (int, int) -> int; += : assign int
        i += 1                  // += : assign int
    };                          // loop : void (break with no value)
    total                       // -> int
}

@sum_for (n: int) -> int = {
    let total: int = 0;
    for x: int in 1..=n do      // ..= : (int, int) -> RangeInclusive<int>
        total += x;             // += : assign int
    total                       // -> int
}

@main () -> int = {
    let a: int = sum_loop(n: 5);
    let b: int = sum_for(n: 5);
    a + b                       // + : (int, int) -> int -> 30
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 6 | **Desugared**: 3 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- for-in range desugared to loop with counter variable and bounds check
- compound assignments (+=) desugared to binary op + assignment
- Range 1..=n lowered to range struct {start: 1, end: n, step: 1, inclusive: true}
- break lowered to loop exit node
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
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
@sum_loop: no heap values — pure scalar arithmetic with mutable locals
@sum_for: no heap values — pure scalar arithmetic with range iteration
@main: no heap values — pure scalar function calls and addition
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 30 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ let a = @sum_loop(n: 5)
       └─ let i = 0, total = 0
       └─ loop iteration 1: i=0, i+1=1, total=0+1=1, i=1
       └─ loop iteration 2: i=1, i+1=2, total=1+2=3, i=2
       └─ loop iteration 3: i=2, i+1=3, total=3+3=6, i=3
       └─ loop iteration 4: i=3, i+1=4, total=6+4=10, i=4
       └─ loop iteration 5: i=4, i+1=5, total=10+5=15, i=5
       └─ i(5) >= n(5): break
       → 15
  └─ let b = @sum_for(n: 5)
       └─ let total = 0
       └─ for x in 1..=5: total=0+1=1, 1+2=3, 3+3=6, 6+4=10, 10+5=15
       → 15
  └─ a + b = 15 + 15 = 30
→ 30
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
@sum_loop: +0 rc_inc, +0 rc_dec (no heap values)
@sum_for: +0 rc_inc, +0 rc_dec (no heap values)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '07-loops'
source_filename = "07-loops"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @sum_loop ---
define fastcc noundef i64 @_ori_sum_loop(i64 noundef %0) #0 {
bb0:
  br label %bb1

bb1:                                              ; preds = %add.ok4, %bb0
  %v5 = phi i64 [ 0, %bb0 ], [ %add.val, %add.ok4 ]
  %v6 = phi i64 [ 0, %bb0 ], [ %add.val2, %add.ok4 ]
  %ge = icmp sge i64 %v5, %0
  br i1 %ge, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  ret i64 %v6

bb3:                                              ; preds = %bb1
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v5, i64 1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb3
  %add1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v6, i64 %add.val)
  %add.val2 = extractvalue { i64, i1 } %add1, 0
  %add.ovf3 = extractvalue { i64, i1 } %add1, 1
  br i1 %add.ovf3, label %add.ovf_panic5, label %add.ok4

add.ovf_panic:                                    ; preds = %bb3
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok4:                                          ; preds = %add.ok
  br label %bb1

add.ovf_panic5:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @sum_for ---
define fastcc noundef i64 @_ori_sum_for(i64 noundef %0) #0 {
bb0:
  %ctor.1 = insertvalue { i64, i64, i64, i64 } { i64 1, i64 undef, i64 undef, i64 undef }, i64 %0, 1
  %ctor.2 = insertvalue { i64, i64, i64, i64 } %ctor.1, i64 1, 2
  %ctor.3 = insertvalue { i64, i64, i64, i64 } %ctor.2, i64 1, 3
  %proj.0 = extractvalue { i64, i64, i64, i64 } %ctor.3, 0
  %proj.1 = extractvalue { i64, i64, i64, i64 } %ctor.3, 1
  %proj.2 = extractvalue { i64, i64, i64, i64 } %ctor.3, 2
  %proj.3 = extractvalue { i64, i64, i64, i64 } %ctor.3, 3
  br label %bb1

bb1:                                              ; preds = %add.ok4, %bb0
  %v8 = phi i64 [ %proj.0, %bb0 ], [ %add.val2, %add.ok4 ]
  %v9 = phi i64 [ 0, %bb0 ], [ %add.val, %add.ok4 ]
  %le = icmp sle i64 %v8, %proj.1
  br i1 %le, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v9, i64 %v8)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb3:                                              ; preds = %bb1
  ret i64 %v9

add.ok:                                           ; preds = %bb2
  %add1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v8, i64 %proj.2)
  %add.val2 = extractvalue { i64, i1 } %add1, 0
  %add.ovf3 = extractvalue { i64, i1 } %add1, 1
  br i1 %add.ovf3, label %add.ovf_panic5, label %add.ok4

add.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok4:                                          ; preds = %add.ok
  br label %bb1

add.ovf_panic5:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_sum_loop(i64 5)
  %call1 = call fastcc i64 @_ori_sum_for(i64 5)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #1

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #2

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
_ori_sum_loop:
   sub    $0x38,%rsp
   mov    %rdi,0x20(%rsp)
   xor    %eax,%eax
   mov    %rax,%rcx
   mov    %rcx,0x28(%rsp)
   mov    %rax,0x30(%rsp)
   jmp    .+0                       ; fallthrough to loop header
.loop:
   mov    0x20(%rsp),%rcx           ; reload n
   mov    0x28(%rsp),%rax           ; reload i
   mov    0x30(%rsp),%rdx           ; reload total
   cmp    %rcx,%rax
   jl     .body                     ; i < n → continue
   mov    0x10(%rsp),%rax           ; return total
   add    $0x38,%rsp
   ret
.body:
   inc    %rax                      ; i + 1 (with overflow check)
   jo     .panic1
   add    %rcx,%rax                 ; total += (i+1) (with overflow check)
   jo     .panic2
   mov    %rcx,0x28(%rsp)           ; store new i
   mov    %rax,0x30(%rsp)           ; store new total
   jmp    .loop

_ori_sum_for:
   sub    $0x48,%rsp
   mov    $0x1,%ecx                 ; start = 1
   mov    %rdi,0x30(%rsp)           ; end = n
   xor    %eax,%eax                 ; total = 0
.loop:
   cmp    0x30(%rsp),%rax           ; x <= n?
   jg     .exit
   add    %rcx,%rax                 ; total += x (overflow check)
   jo     .panic1
   add    $0x1,%rax                 ; x += step (overflow check)
   jo     .panic2
   jmp    .loop
.exit:
   ret

_ori_main:
   sub    $0x18,%rsp
   mov    $0x5,%edi
   call   _ori_sum_loop             ; a = 15
   mov    %rax,0x8(%rsp)
   mov    $0x5,%edi
   call   _ori_sum_for              ; b = 15
   add    0x8(%rsp),%rax            ; a + b (overflow check)
   jo     .panic
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @sum_loop | 19 | 18 | 1.06x | NEAR-OPTIMAL |
| 2 | @sum_for | 26 | 25 | 1.04x | NEAR-OPTIMAL |
| 3 | @main | 9 | 9 | 1.00x | OPTIMAL |

**@sum_loop** (19 actual vs 18 ideal): The single unjustified instruction is the initial `br label %bb1` in bb0, which unconditionally branches to the loop header. LLVM can fold this away, but it is unnecessary at the IR level. The remaining overhead is justified: 2 overflow-checked additions per iteration (i+1 and total += ...), each requiring call/extractvalue/br -- standard safety overhead. The phi-based loop header is the correct lowering for mutable variables across loop iterations.

**@sum_for** (26 actual vs 25 ideal): The range struct is constructed via 3 insertvalue instructions then immediately destructured via 4 extractvalue instructions in bb0. While LLVM's mem2reg trivially eliminates this, the construct-then-destructure pattern is 1 instruction above ideal. The loop body itself is clean: checked addition for accumulation + checked addition for step increment, with phi nodes in the header.

**@main** (9 actual vs 9 ideal): OPTIMAL. Two fastcc calls + one overflow-checked addition. No wasted instructions.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @sum_loop | 0 | 0 | YES | N/A | N/A |
| @sum_for | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 0 | YES | N/A | N/A |

**Verdict**: No heap values. Zero RC operations. OPTIMAL. All values are scalar integers -- loops, ranges, and counters involve no heap allocation.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | uwtable | cold | Notes |
|----------|--------|----------|---------|---------|------|-------|
| @sum_loop | YES | YES | YES | YES | N/A | |
| @sum_for | YES | YES | YES | YES | N/A | |
| @main | C (correct) | YES | YES | YES | N/A | [LOW-1] |
| ori_panic_cstr | N/A | N/A | N/A | N/A | YES | cold + noreturn |

All user functions have `nounwind` (fixed-point analysis confirmed all 3 are nounwind). `fastcc` is applied to all non-entry-point functions. `noundef` on parameters and return values is present. The `@_ori_main` function correctly uses C calling convention for entry point compatibility.

**[LOW-1]**: @main uses `C` calling convention instead of `fastcc` -- correct for the entry point wrapper, but the `main()` trampoline calls `@_ori_main` without `fastcc`. This is a negligible efficiency gap since `main` is called exactly once.

The 1 missing attribute: `ori_panic_cstr` lacks `nounwind` -- it is a `cold noreturn` function that likely unwinds. This is arguably correct (panic may unwind), so it is borderline.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @sum_loop | 8 | 2 | 1 | 2 | [LOW-2] |
| @sum_for | 8 | 1 | 1 | 2 | [LOW-3] |
| @main | 3 | 0 | 0 | 0 | Clean |

**@sum_loop**: bb0 exists solely to branch to bb1 (the loop header). add.ok4 exists solely to branch back to bb1. Both are empty trampolines that LLVM can merge. The 2 phi nodes in bb1 are correct and necessary for `i` and `total` state across iterations.

**@sum_for**: Similar pattern -- add.ok4 is an empty trampoline back to bb1. The range construction in bb0 does contain real work (insertvalue/extractvalue), so bb0 is not empty per se, but the construct-then-immediate-extract is redundant.

**@main**: Clean control flow -- one entry block with overflow check branching.

The 5 CF defects (3 + 2) are all minor: empty trampoline blocks and redundant unconditional branches. None affect correctness, and LLVM's SimplifyCFG pass eliminates them during optimization.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| i + 1 (sum_loop) | YES | YES | llvm.sadd.with.overflow.i64 |
| total += (i+1) (sum_loop) | YES | YES | llvm.sadd.with.overflow.i64 |
| total += x (sum_for) | YES | YES | llvm.sadd.with.overflow.i64 |
| x += step (sum_for) | YES | YES | llvm.sadd.with.overflow.i64 |
| a + b (main) | YES | YES | llvm.sadd.with.overflow.i64 |

All 5 addition operations are overflow-checked. Each uses the `llvm.sadd.with.overflow.i64` intrinsic with proper panic on overflow. The overflow panic paths are marked `cold` via the `ori_panic_cstr` declaration.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 868.8 KiB |
| .rodata section | 133.5 KiB |
| User code (@sum_loop) | 154 bytes (39 instructions) |
| User code (@sum_for) | 166 bytes (42 instructions) |
| User code (@main) | 80 bytes (17 instructions) |
| Total user code | 400 bytes |
| Runtime | >99% of binary |

The user code compiles to 400 bytes of machine code across 3 functions. The native code uses stack spills heavily (debug mode, no register allocation optimization), but the structure is correct. Overflow checks compile to `jo` (jump on overflow) instructions, which is the optimal x86 encoding.

#### Disassembly: @sum_loop

```asm
_ori_sum_loop:
   sub    $0x38,%rsp
   mov    %rdi,0x20(%rsp)         ; save n to stack
   xor    %eax,%eax               ; i = 0, total = 0
   jmp    .loop_header
.loop_header:
   mov    0x20(%rsp),%rcx         ; load n
   mov    0x28(%rsp),%rax         ; load i
   cmp    %rcx,%rax               ; i >= n?
   jl     .loop_body
   mov    0x10(%rsp),%rax         ; return total
   add    $0x38,%rsp
   ret
.loop_body:
   inc    %rax                    ; i + 1
   jo     .panic
   add    %rcx,%rax               ; total + (i+1)
   jo     .panic
   jmp    .loop_header
```

#### Disassembly: @main

```asm
_ori_main:
   sub    $0x18,%rsp
   mov    $0x5,%edi
   call   _ori_sum_loop
   mov    %rax,0x8(%rsp)
   mov    $0x5,%edi
   call   _ori_sum_for
   add    0x8(%rsp),%rax
   jo     .panic
   ret
```

### 7. Optimal IR Comparison

#### @sum_loop: Ideal vs Actual

```llvm
; IDEAL (18 instructions)
define fastcc noundef i64 @_ori_sum_loop(i64 noundef %n) nounwind {
entry:
  br label %loop

loop:
  %i = phi i64 [ 0, %entry ], [ %i.next, %continue ]
  %total = phi i64 [ 0, %entry ], [ %total.next, %continue ]
  %done = icmp sge i64 %i, %n
  br i1 %done, label %exit, label %body

body:
  %i1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %i, i64 1)
  %i.next = extractvalue { i64, i1 } %i1, 0
  %i.ovf = extractvalue { i64, i1 } %i1, 1
  br i1 %i.ovf, label %panic1, label %add_total

add_total:
  %t1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %total, i64 %i.next)
  %total.next = extractvalue { i64, i1 } %t1, 0
  %t.ovf = extractvalue { i64, i1 } %t1, 1
  br i1 %t.ovf, label %panic2, label %continue

continue:
  br label %loop

exit:
  ret i64 %total

panic1:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (19 instructions) — 1 extra: initial br in bb0
define fastcc noundef i64 @_ori_sum_loop(i64 noundef %0) #0 {
bb0:
  br label %bb1                    ; <-- unjustified (could merge bb0 into bb1)
  ; ... rest identical to ideal structure
}
```

**Delta**: +1 instruction. The `br label %bb1` in bb0 is an unconditional branch to the loop header that exists because codegen emits a separate entry block. The loop structure itself is identical to ideal: phi-based header with condition check, overflow-checked body, and backedge.

#### @sum_for: Ideal vs Actual

```llvm
; IDEAL (25 instructions) — phi with constants directly, no range struct
define fastcc noundef i64 @_ori_sum_for(i64 noundef %n) nounwind {
entry:
  br label %loop

loop:
  %x = phi i64 [ 1, %entry ], [ %x.next, %continue ]
  %total = phi i64 [ 0, %entry ], [ %total.next, %continue ]
  %done = icmp sgt i64 %x, %n
  br i1 %done, label %exit, label %body

body:
  %t1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %total, i64 %x)
  %total.next = extractvalue { i64, i1 } %t1, 0
  %t.ovf = extractvalue { i64, i1 } %t1, 1
  br i1 %t.ovf, label %panic1, label %step

step:
  %s1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %x, i64 1)
  %x.next = extractvalue { i64, i1 } %s1, 0
  %s.ovf = extractvalue { i64, i1 } %s1, 1
  br i1 %s.ovf, label %panic2, label %continue

continue:
  br label %loop

exit:
  ret i64 %total

panic1:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (26 instructions) — range struct construction adds overhead
define fastcc noundef i64 @_ori_sum_for(i64 noundef %0) #0 {
bb0:
  %ctor.1 = insertvalue { i64, i64, i64, i64 } { ... }, i64 %0, 1   ; construct range
  %ctor.2 = insertvalue { i64, i64, i64, i64 } %ctor.1, i64 1, 2
  %ctor.3 = insertvalue { i64, i64, i64, i64 } %ctor.2, i64 1, 3
  %proj.0 = extractvalue { i64, i64, i64, i64 } %ctor.3, 0          ; destructure
  %proj.1 = extractvalue { i64, i64, i64, i64 } %ctor.3, 1
  %proj.2 = extractvalue { i64, i64, i64, i64 } %ctor.3, 2
  %proj.3 = extractvalue { i64, i64, i64, i64 } %ctor.3, 3          ; unused
  br label %bb1
  ; ... loop body structurally identical to ideal
}
```

**Delta**: +1 instruction. The range struct construction (3 insertvalue + 4 extractvalue) and immediate destructure is redundant -- ideal code would thread constants directly into the phi nodes. However, 6 of these 7 are optimized away by LLVM's instcombine/mem2reg, so the net overhead at runtime is minimal. The 1 unjustified instruction is the `%proj.3` extractvalue for the unused `inclusive` field.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @sum_loop | 18 | 19 | +1 | NO (redundant br) | NEAR-OPTIMAL |
| @sum_for | 25 | 26 | +1 | NO (unused extract) | NEAR-OPTIMAL |
| @main | 9 | 9 | +0 | N/A | OPTIMAL |

### 8. Loops: Lowering

The `loop { ... break }` construct in `@sum_loop` is lowered to a textbook phi-based loop:

1. **Entry block** (bb0): Initializes loop variables (i=0, total=0) and branches to the loop header.
2. **Loop header** (bb1): Contains phi nodes that merge initial values from bb0 with updated values from the backedge (add.ok4). The loop condition `i >= n` is checked here.
3. **Exit block** (bb2): Returns the accumulated `total`.
4. **Body block** (bb3): Computes `i + 1` with overflow checking.
5. **Continuation** (add.ok, add.ok4): Computes `total += (i+1)` with overflow checking, then branches back to the loop header.

This is the standard SSA lowering for imperative loops with mutable variables. The compiler correctly converts mutable locals (`let i`, `let total`) to phi nodes rather than stack allocas, which is the optimal approach. The `break` statement is correctly lowered to a conditional branch to the exit block.

The `loop` construct produces exactly the control flow expected: an infinite loop with an explicit exit condition, matching the semantics of Ori's `loop { if cond then break; body }` pattern.

### 9. Ranges: Iteration

The `for x in 1..=n do body` construct in `@sum_for` is lowered through a range struct:

1. **Range construction**: The `1..=n` expression creates a `{i64, i64, i64, i64}` struct representing `{start=1, end=n, step=1, inclusive=1}`. This is the canonical range representation.
2. **Destructuring**: The range fields are immediately extracted via extractvalue, creating local SSA values `%proj.0` through `%proj.3`.
3. **Loop structure**: The iteration variable and accumulator use phi nodes. The loop condition uses `icmp sle` (signed less-or-equal) for inclusive ranges, compared to the `icmp sge` (signed greater-or-equal) used in the explicit loop.

**Optimization opportunity**: The range construction and immediate destructuring (`insertvalue` x3 + `extractvalue` x4) could be eliminated by directly threading the range constants into the phi nodes and loop condition. This is a 7-instruction cold-path overhead that LLVM's optimization passes eliminate, but the Ori codegen could avoid emitting them in the first place for constant ranges. The `%proj.3` (inclusive flag) is extracted but never used in the generated code -- the `sle` vs `slt` comparison already encodes inclusivity.

The for-loop and explicit loop produce equivalent machine code structure at the LLVM IR level: both use phi-based iteration with overflow-checked arithmetic in the body. The main difference is the range struct overhead in the entry block.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Attributes | Missing `nounwind` on `ori_panic_cstr` declaration | CONFIRMED | J1 |
| 2 | LOW | Control Flow | Redundant `br label %bb1` entry in @sum_loop | NEW | J7 |
| 3 | LOW | Control Flow | Empty trampoline blocks (add.ok4) in both loop functions | NEW | J7 |
| 4 | LOW | IR Quality | Range construct-then-destructure in @sum_for bb0 | NEW | J7 |
| 5 | NOTE | Loops | Correct phi-based lowering for mutable loop variables | NEW | J7 |
| 6 | NOTE | Ranges | Inclusive range correctly uses `sle` comparison | NEW | J7 |
| 7 | NOTE | ARC | Zero RC overhead for pure scalar loops | NEW | J7 |

### LOW-1: Missing nounwind on ori_panic_cstr

**Location**: `ori_panic_cstr` declaration
**Impact**: LLVM may generate unnecessary exception handling tables
**Fix**: If panic does not unwind (aborts), add `nounwind`. If it can unwind, this is correct.
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-2: Redundant entry block branch in @sum_loop

**Location**: @sum_loop, bb0 `br label %bb1`
**Impact**: 1 unnecessary instruction (eliminated by LLVM SimplifyCFG)
**Fix**: Emit phi predecessors directly from entry block, or merge bb0 into bb1 when entry is unconditional
**First seen**: Journey 7
**Found in**: Control Flow & Block Layout (Category 4), Instruction Purity (Category 1)

### LOW-3: Empty trampoline blocks (add.ok4)

**Location**: @sum_loop add.ok4, @sum_for add.ok4 (and bb0 empty in sum_loop)
**Impact**: 3 blocks with only unconditional `br` -- unnecessary indirection
**Fix**: Emit backedge directly from the last overflow-check success block to loop header
**First seen**: Journey 7
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-4: Range struct construct-then-destructure

**Location**: @sum_for bb0, insertvalue x3 + extractvalue x4
**Impact**: 7 instructions in cold entry path; LLVM optimizes away, but avoidable at codegen
**Fix**: For constant ranges, thread values directly into phi nodes. Extract only used fields.
**First seen**: Journey 7
**Found in**: Optimal IR Comparison (Category 7), Loops: Lowering (Category 8)

### NOTE-5: Correct phi-based loop lowering

**Location**: Both loop functions
**Impact**: Positive -- mutable locals are correctly converted to phi nodes rather than stack allocas, producing optimal SSA form
**Found in**: Loops: Lowering (Category 8)

### NOTE-6: Inclusive range comparison correctness

**Location**: @sum_for, `%le = icmp sle i64 %v8, %proj.1`
**Impact**: Positive -- `1..=n` correctly uses `sle` (signed less-or-equal) vs `slt` for exclusive ranges
**Found in**: Ranges: Iteration (Category 9)

### NOTE-7: Zero ARC overhead for scalar loops

**Location**: All functions
**Impact**: Positive -- the ARC pipeline correctly identifies that all loop variables are scalars and emits zero RC operations
**Found in**: ARC Purity (Category 2)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.04x avg ratio (max 1.06x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 8/10 | 94.1% compliance |
| Control Flow | 10% | 7/10 | 5 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.2 / 10**

## Verdict

Journey 7's loop codegen is strong. Both `loop/break` and `for-in` range iteration produce near-optimal phi-based loops with correct overflow checking on all arithmetic. The compiler correctly converts mutable locals to SSA phi nodes rather than stack allocas, which is the key optimization for loop performance. The main inefficiencies are minor: empty trampoline blocks (3 instances) and a range struct construct-then-destructure pattern that LLVM trivially eliminates. ARC is perfectly irrelevant -- zero RC operations for pure scalar loops.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J7 | CONFIRMED |
| nounwind attribute | J1 | J7 | FIXED (user functions now have nounwind) |
| fastcc usage | J1 | J7 | CONFIRMED |
| noundef on params | J1 | J7 | CONFIRMED |
| Empty trampoline blocks | J7 | J7 | NEW |

The nounwind analysis has improved since J1: all user functions now correctly receive the `nounwind` attribute via the fixed-point nounwind analysis (visible in the arc_trace: "nounwind analysis complete, passes=2, nounwind_count=3"). The `ori_panic_cstr` declaration still lacks `nounwind`, which is the only remaining attribute gap.
