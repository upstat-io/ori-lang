---
journey: 1
slug: arithmetic
theme: "I am arithmetic"
date: 2026-03-19
status: PASS
expected: 33
eval_result: 33
aot_result: 33
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of functions and variables"
learning_objectives:
  - "Understand how arithmetic expressions are lowered to LLVM IR"
  - "See how overflow checking adds safety instructions to every operation"
  - "Compare ideal vs actual codegen for simple functions"
  - "Learn what function attributes (nounwind, fastcc, noundef, memory) mean and why they matter"
features:
  - arithmetic
  - function_calls
  - let_bindings
  - int_literals
  - multiple_functions
feature_description: "Basic arithmetic with function calls, let bindings, and integer operations"
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
  attr_applicable: 10
  attr_correct: 10
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
related_journeys: []
---

# Journey 1: "I am arithmetic"

## Source

```ori
// Journey 1: "I am arithmetic"
// Slug: arithmetic
// Difficulty: simple
// Features: arithmetic, function_calls, let_bindings, int_literals, multiple_functions
// Expected: (3 + 4) * 5 - 2 = 33

@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let x = 3;
    let y = 4;
    let sum = add(a: x, b: y);   // = 7
    let result = sum * 5 - 2;  // = 35 - 2 = 33
    result
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 33        | 33       | (none) | (none) | PASS   |
| AOT     | 33        | 33       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals. This is the first
> stage of every compiler.

**Tokens**: 77 | **Keywords**: 4 | **Identifiers**: 11 | **Errors**: 0

The source file is 387 bytes. The lexer produces 77 tokens with zero errors. Keywords include `let` (x4) and `@` function markers (x2). Identifiers cover function names, parameter names, type names, and local bindings.

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(add) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(int) Eq
Ident(a) Plus Ident(b) Semi

Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(x) Eq Int(3) Semi
Let Ident(y) Eq Int(4) Semi
Let Ident(sum) Eq Ident(add) LParen Ident(a) Colon Ident(x)
  Comma Ident(b) Colon Ident(y) RParen Semi
Let Ident(result) Eq Ident(sum) Star Int(5) Minus Int(2) Semi
Ident(result) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.
> Operator precedence is resolved here: `*` binds tighter than `-`.

**Nodes**: 16 | **Max depth**: 4 | **Functions**: 2 | **Errors**: 0

The parser produces 16 expression nodes across 2 function declarations. The `sum * 5 - 2` expression is correctly parsed with `*` binding tighter than `-`, producing `(sum * 5) - 2` not `sum * (5 - 2)`. Named arguments `a:` and `b:` are preserved in the AST for later resolution to positional order.

<details>
<summary>AST (simplified)</summary>

```text
Module
+-- FnDecl @add
|  +-- Params: (a: int, b: int)
|  +-- Return: int
|  +-- Body: BinOp(+)
|       +-- Ident(a)
|       +-- Ident(b)
+-- FnDecl @main
   +-- Return: int
   +-- Body: Block
        +-- Let x = Lit(3)
        +-- Let y = Lit(4)
        +-- Let sum = Call(@add)
        |    +-- a: Ident(x)
        |    +-- b: Ident(y)
        +-- Let result = BinOp(-)
        |    +-- BinOp(*)
        |    |    +-- Ident(sum)
        |    |    +-- Lit(5)
        |    +-- Lit(2)
        +-- Ident(result)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit annotations everywhere.

**Constraints**: 12 | **Types inferred**: 6 | **Unifications**: 10 | **Errors**: 0

All types resolve to `int`. The 6 inferred bindings are: `x`, `y`, `sum`, `result`, plus the two operator results (`sum * 5` and `(sum * 5) - 2`). The type checker confirms that `Add<int, int> -> int`, `Mul<int, int> -> int`, and `Sub<int, int> -> int` are all valid. The return type of `@main` matches the final expression `result: int`.

<details>
<summary>Inferred types</summary>

```ori
@add (a: int, b: int) -> int = a + b
//                               ^ int (Add<int, int> -> int)

@main () -> int = {
    let x: int = 3           // inferred from literal
    let y: int = 4           // inferred from literal
    let sum: int = add(a: x, b: y)  // inferred from @add return type
    let result: int = sum * 5 - 2   // int (Mul then Sub)
    result  // -> int (matches return type)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form -- a flat
> sequence of operations suitable for backend consumption. It desugars syntactic sugar,
> lowers complex expressions, and resolves named arguments to positional order.

**Transforms**: 2 | **Desugared**: 0 | **Errors**: 0

The canonicalizer produces 20 canon nodes from 16 AST nodes (let bindings create extra pattern nodes). Named arguments `a: x, b: y` are resolved to positional order. No desugaring is needed -- there is no syntactic sugar in this program.

<details>
<summary>Key transformations</summary>

```text
- 20 canon nodes from 16 AST nodes (let bindings create extra pattern nodes)
- 2 roots: @add, @main
- 6 constants: int literals 3, 4, 5, 2 plus function-level metadata
- 0 decision trees (no pattern matching)
- Named arguments (a: x, b: y) resolved to positional order
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
@add: no heap values -- pure scalar arithmetic (int params, int return)
@main: no heap values -- all let bindings hold int scalars
Total RC ops: 0 (optimal for scalar-only program)
AIMS lattice: all values classified as scalar -- no RC analysis needed
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without compilation.
> It serves as the reference implementation for correctness testing -- if eval and AOT
> disagree, the bug is in LLVM codegen, not the interpreter.

**Result**: 33 | **Status**: PASS

The eval trace shows the canonical execution order: `@main` evaluates the block, binds `x=3`, `y=4`, calls `@add(3,4)=7`, computes `7*5=35`, then `35-2=33`. The final expression `result` evaluates to `Int(33)`.

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-- let x = 3
  +-- let y = 4
  +-- let sum = @add(a: 3, b: 4)
  |    +-- 3 + 4 = 7
  +-- let result = 7 * 5 - 2
  |    +-- 7 * 5 = 35
  |    +-- 35 - 2 = 33
  +-- result = 33
-> 33
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
@_ori_add: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
@_ori_main: +0 rc_inc, +0 rc_dec (pure scalar -- no heap values)
Nounwind analysis: 2 passes (fixed-point), both functions marked nounwind
Memory analysis: @_ori_add marked memory(none) -- pure function, no observable side effects
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '01-arithmetic'
source_filename = "01-arithmetic"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.2 = private unnamed_addr constant [32 x i8] c"integer overflow on subtraction\00", align 1

; Function Attrs: nounwind memory(none) uwtable
; --- @add ---
define fastcc noundef i64 @_ori_add(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %0, i64 %1)
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
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %call, i64 5)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok

mul.ok:                                           ; preds = %bb0
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %mul.val, i64 2)
  %sub.val = extractvalue { i64, i1 } %sub, 0
  %sub.ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.ovf, label %sub.ovf_panic, label %sub.ok

mul.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable

sub.ok:                                           ; preds = %mul.ok
  ret i64 %sub.val

sub.ovf_panic:                                    ; preds = %mul.ok
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64) #2

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
000000000001b100 <_ori_add>:                    ; 31 bytes
   1b100:  push   %rax                   ; save scratch register / align stack
   1b101:  add    %rsi,%rdi              ; a + b (sets overflow flag)
   1b104:  mov    %rdi,(%rsp)            ; spill result to stack (O0 regalloc)
   1b108:  seto   %al                    ; set AL=1 if overflow occurred
   1b10b:  jo     panic                  ; branch to panic if overflow
   1b10d:  mov    (%rsp),%rax            ; reload result from stack (O0 regalloc)
   1b111:  pop    %rcx                   ; restore stack
   1b112:  ret                           ; return result in %rax
   ; --- overflow path ---
   1b113:  lea    ovf.msg(%rip),%rdi     ; load panic message address
   1b11a:  call   ori_panic_cstr         ; panic (does not return)

000000000001b120 <_ori_main>:                   ; 93 bytes
   1b120:  sub    $0x18,%rsp              ; stack frame (24 bytes)
   1b124:  mov    $0x3,%edi               ; arg a = 3
   1b129:  mov    $0x4,%esi               ; arg b = 4
   1b12e:  call   _ori_add               ; sum = add(3, 4) -> 7
   1b133:  mov    $0x5,%ecx              ; load constant 5
   1b138:  imul   %rcx,%rax              ; sum * 5 = 35
   1b13c:  mov    %rax,0x10(%rsp)        ; spill mul result (O0)
   1b141:  seto   %al                    ; overflow check (mul)
   1b144:  jo     mul_panic              ; branch if overflow
   1b146:  mov    0x10(%rsp),%rax        ; reload mul result (O0)
   1b14b:  sub    $0x2,%rax              ; 35 - 2 = 33
   1b14f:  mov    %rax,0x8(%rsp)         ; spill sub result (O0)
   1b154:  seto   %al                    ; overflow check (sub)
   1b157:  jo     sub_panic              ; branch if overflow
   1b159:  jmp    epilogue               ; jump over panic blocks
   ; --- mul overflow path ---
   1b15b:  lea    ovf.msg.1(%rip),%rdi
   1b162:  call   ori_panic_cstr
   ; --- epilogue ---
   1b167:  mov    0x8(%rsp),%rax         ; reload final result (O0)
   1b16c:  add    $0x18,%rsp             ; restore stack
   1b170:  ret                           ; return 33
   ; --- sub overflow path ---
   1b171:  lea    ovf.msg.2(%rip),%rdi
   1b178:  call   ori_panic_cstr

000000000001b180 <main>:                        ; 29 bytes
   1b180:  push   %rax                    ; align stack
   1b181:  call   _ori_main               ; call Ori main
   1b186:  mov    %eax,0x4(%rsp)          ; save exit code
   1b18a:  call   ori_check_leaks         ; RC leak detection
   1b18f:  mov    %eax,%ecx               ; leak result -> ecx
   1b191:  mov    0x4(%rsp),%eax          ; reload exit code
   1b195:  cmp    $0x0,%ecx               ; check if leaks detected
   1b198:  cmovne %ecx,%eax              ; use leak code if nonzero
   1b19b:  pop    %rcx                    ; restore stack
   1b19c:  ret                            ; return final exit code
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual (IR) | Ideal (IR) | Ratio | Verdict |
|---|----------|-------------|------------|-------|---------|
| 1 | @add     | 7           | 7          | 1.00x | OPTIMAL |
| 2 | @main    | 14          | 14         | 1.00x | OPTIMAL |

**@add (7 instructions)**: Every instruction is justified. The overflow-checked addition requires the intrinsic call (1), two extractvalues to split the result and overflow flag (2), a conditional branch (1), a return (1), and a panic path with call + unreachable (2). No wasted instructions. **OPTIMAL.**

**@main (14 instructions)**: Every instruction is justified. The function calls `@_ori_add` (1), performs overflow-checked multiplication (call + 2 extractvalue + branch = 4), overflow-checked subtraction (call + 2 extractvalue + branch = 4), returns (1), and has two panic paths (2 x (call + unreachable) = 4). The call result flows directly into the multiply in the same `bb0` block -- no redundant bridge blocks. **OPTIMAL.**

**Let binding elimination**: All four `let` bindings (`x`, `y`, `sum`, `result`) are correctly eliminated -- no `alloca`/`store`/`load` chains. Constants `3` and `4` are inlined directly as call arguments. The call result feeds directly into the multiply intrinsic. This is `-O1` quality codegen in a debug build.

**Native instruction overhead**: The native disassembly shows additional overhead from LLVM's `-O0` register allocation -- stack spills (`mov %rax,0x10(%rsp)` / `mov 0x10(%rsp),%rax`) that would be eliminated at `-O1`. This is expected for unoptimized builds and not charged against IR quality.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @add     | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Correct -- this program uses only `int` scalars (i64), which are value types requiring no reference counting. No `ori_rc_inc`, `ori_rc_dec`, or any RC-related calls present in the IR or disassembly. The AIMS unified lattice correctly classifies all values as scalars. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | memory | noundef | noreturn | cold | Notes |
|----------|--------|----------|---------|--------|---------|----------|------|-------|
| @add     | YES    | YES      | YES     | none   | YES (params + ret) | N/A | N/A | [NOTE-1] |
| @main    | NO (C) | YES      | YES     | N/A    | YES (ret) | N/A  | N/A  | C conv for entry point -- correct |
| main wrapper | NO (C) | YES | YES     | N/A    | YES (ret) | N/A  | N/A  | All attributes present |
| ori_panic_cstr | N/A | N/A | N/A     | N/A    | N/A     | YES      | YES  | Both noreturn and cold present [NOTE-2] |
| ori_check_leaks | N/A | YES | N/A    | N/A    | N/A     | N/A      | N/A  | Leak detection runtime function |

**@_ori_add uses `fastcc` with `memory(none)`**: Correct. Internal function benefits from fast calling convention. The `nounwind` attribute is present (fixed-point analysis confirms both user functions do not unwind). `memory(none)` tells LLVM that `@_ori_add` has no observable memory effects -- it only reads its arguments and produces a return value. This is correct for a pure arithmetic function. `uwtable` is present for stack unwinding. `noundef` is present on both parameters and the return value.

**@_ori_main uses C convention**: Correct. Called from the C `main()` wrapper, must use C ABI for compatibility. Also marked `nounwind`, `uwtable`, and `noundef` on the return. Does not have `memory(none)` because it calls `@_ori_add` -- the memory attribute on non-leaf functions requires interprocedural analysis, and the compiler conservatively omits it since `@_ori_main` calls another function.

**main wrapper has full attribute coverage**: The C entry point wrapper has `nounwind`, `uwtable`, and `noundef` on its `i32` return. It now integrates `ori_check_leaks()` for RC leak detection at program exit.

**`ori_panic_cstr` has both `cold` and `noreturn`**: Correct. Enables dead code elimination and branch prediction heuristics.

**Attribute compliance**: 10 applicable attributes checked (per extract-metrics.py). 10 of 10 correct. 100% compliance.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @add     | 3      | 0           | 0                 | 0         | Optimal layout |
| @main    | 5      | 0           | 0                 | 0         | Optimal layout |
| main wrapper | 1  | 0           | 0                 | 0         | Optimal |

**@add block layout**: 3 blocks -- `bb0` (entry with overflow check), `add.ok` (happy-path return), `add.ovf_panic` (cold panic). Happy path is fallthrough from the conditional branch. Panic block placed at the end. **Optimal layout.**

**@main block layout**: 5 blocks with zero redundant branches. The call to `@_ori_add` and the subsequent overflow-checked multiply reside in the same `bb0` block -- no redundant bridge blocks:
- `bb0`: call to `@_ori_add` + overflow-checked multiply + conditional branch
- `mul.ok`: overflow-checked subtract + conditional branch
- `mul.ovf_panic`: cold panic for multiply overflow
- `sub.ok`: return
- `sub.ovf_panic`: cold panic for subtract overflow

Panic blocks are correctly placed after the happy path, and the `cold` attribute on `ori_panic_cstr` helps LLVM's branch prediction heuristics. **Optimal layout.**

### 5. Overflow Checking

**Status**: PASS

| Operation | Intrinsic | Checked | Correct | Panic Message |
|-----------|-----------|---------|---------|---------------|
| `a + b`   | `llvm.sadd.with.overflow.i64` | YES | YES | "integer overflow on addition" |
| `sum * 5` | `llvm.smul.with.overflow.i64` | YES | YES | "integer overflow on multiplication" |
| `sum * 5 - 2` | `llvm.ssub.with.overflow.i64` | YES | YES | "integer overflow on subtraction" |

All three arithmetic operations use the correct LLVM signed overflow intrinsics. Each operation has a dedicated human-readable panic message stored as a global constant string. The overflow flag is checked with a conditional branch (`br i1 %ovf`) routing to a `cold noreturn` panic function followed by `unreachable`. This is correct per the Ori spec: "overflow panics."

The panic messages are operation-specific (not generic), which is good for debugging. The message strings are `private unnamed_addr constant` with NUL termination, which is correct for C string interop with `ori_panic_cstr`.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (6,559,000 bytes, debug) |
| .text section | 869 KiB (890,217 bytes) |
| .rodata section | 134 KiB (136,737 bytes) |
| .debug_info | 1.56 MiB (1,640,996 bytes) |
| .debug_str | 1.72 MiB (1,804,511 bytes) |
| .eh_frame | 109 KiB (112,080 bytes) |
| User code (@add) | 31 bytes (0x1b100-0x1b11f) |
| User code (@main) | 93 bytes (0x1b120-0x1b17d) |
| User code (main wrapper) | 29 bytes (0x1b180-0x1b19c) |
| User code total | 153 bytes |
| User code % of .text | 0.017% |
| Runtime % of binary | ~99.98% |

The binary is large due to static linking of `ori_rt` (the Ori runtime, which includes Rust's standard library for panic handling, I/O, memory allocation) and full debug symbols (3.28 MiB of .debug_* sections). The user's actual code is 153 bytes -- everything else is runtime infrastructure. This is expected for a debug build of a statically-linked binary. The main wrapper now includes RC leak detection logic (`ori_check_leaks`), adding 21 bytes over a minimal wrapper.

#### Disassembly: @add

```asm
_ori_add:                        ; 31 bytes, 10 instructions (+ 1 nop)
  push   %rax                   ; save scratch register / align stack
  add    %rsi,%rdi              ; a + b (sets overflow flag)
  mov    %rdi,(%rsp)            ; spill result to stack (O0 regalloc)
  seto   %al                    ; set AL=1 if overflow occurred
  jo     panic                  ; branch to panic if overflow
  mov    (%rsp),%rax            ; reload result from stack (O0 regalloc)
  pop    %rcx                   ; restore stack
  ret                           ; return result in %rax
  ; --- overflow path ---
  lea    ovf.msg(%rip),%rdi     ; load panic message address
  call   ori_panic_cstr         ; panic (does not return)
```

#### Disassembly: @main

```asm
_ori_main:                       ; 93 bytes, 22 instructions
  sub    $0x18,%rsp              ; stack frame (24 bytes)
  mov    $0x3,%edi               ; arg a = 3
  mov    $0x4,%esi               ; arg b = 4
  call   _ori_add               ; sum = add(3, 4) -> 7
  mov    $0x5,%ecx              ; load constant 5
  imul   %rcx,%rax              ; sum * 5 = 35
  mov    %rax,0x10(%rsp)        ; spill mul result (O0)
  seto   %al                    ; overflow check (mul)
  jo     mul_panic              ; branch if overflow
  mov    0x10(%rsp),%rax        ; reload mul result (O0)
  sub    $0x2,%rax              ; 35 - 2 = 33
  mov    %rax,0x8(%rsp)         ; spill sub result (O0)
  seto   %al                    ; overflow check (sub)
  jo     sub_panic              ; branch if overflow
  jmp    epilogue               ; jump over panic blocks
  ; --- mul overflow path ---
  lea    ovf.msg.1(%rip),%rdi
  call   ori_panic_cstr
  ; --- epilogue ---
  mov    0x8(%rsp),%rax         ; reload final result (O0)
  add    $0x18,%rsp             ; restore stack
  ret                           ; return 33
  ; --- sub overflow path ---
  lea    ovf.msg.2(%rip),%rdi
  call   ori_panic_cstr
```

### 7. Optimal IR Comparison

#### @add: Ideal vs Actual

```llvm
; IDEAL (7 instructions -- overflow checking is mandatory)
define fastcc noundef i64 @_ori_add(i64 noundef %a, i64 noundef %b) #0 {
entry:
  %r = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue { i64, i1 } %r, 0
  %ovf = extractvalue { i64, i1 } %r, 1
  br i1 %ovf, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (7 instructions)
define fastcc noundef i64 @_ori_add(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %0, i64 %1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok
add.ok:
  ret i64 %add.val
add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: 0 instructions. The actual IR matches the ideal IR exactly in structure and instruction count. Only differences are naming conventions (`bb0`/`add.ok`/`add.ovf_panic` vs `entry`/`ok`/`panic`) and unnamed parameters (`%0`, `%1` vs `%a`, `%b`). **OPTIMAL.**

#### @main: Ideal vs Actual

```llvm
; IDEAL (14 instructions)
define noundef i64 @_ori_main() #1 {
entry:
  %sum = call fastcc i64 @_ori_add(i64 3, i64 4)
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %sum, i64 5)
  %mul.v = extractvalue { i64, i1 } %mul, 0
  %mul.o = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.o, label %mul_panic, label %mul_ok
mul_ok:
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %mul.v, i64 2)
  %sub.v = extractvalue { i64, i1 } %sub, 0
  %sub.o = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.o, label %sub_panic, label %sub_ok
sub_ok:
  ret i64 %sub.v
mul_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
sub_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}
```

```llvm
; ACTUAL (14 instructions)
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %call, i64 5)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok
mul.ok:
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %mul.val, i64 2)
  %sub.val = extractvalue { i64, i1 } %sub, 0
  %sub.ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.ovf, label %sub.ovf_panic, label %sub.ok
mul.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
sub.ok:
  ret i64 %sub.val
sub.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}
```

**Delta**: 0 instructions. The actual IR matches the ideal IR exactly. The call to `@_ori_add` and the overflow-checked multiply reside in the same `bb0` block, exactly as in the ideal IR. **OPTIMAL.**

#### main wrapper: Ideal vs Actual

```llvm
; IDEAL (6 instructions -- leak detection is mandatory for AIMS)
define noundef i32 @main() #1 {
entry:
  %r = call i64 @_ori_main()
  %c = trunc i64 %r to i32
  %lk = call i32 @ori_check_leaks()
  %has = icmp ne i32 %lk, 0
  %fin = select i1 %has, i32 %lk, i32 %c
  ret i32 %fin
}
```

```llvm
; ACTUAL (6 instructions)
define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}
```

**Delta**: 0 instructions. **OPTIMAL.** The `ori_check_leaks` integration is mandatory for AIMS RC leak detection and adds zero unjustified overhead.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @add     | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 14    | 14     | +0    | N/A       | OPTIMAL |
| main wrapper | 6 | 6      | +0    | N/A       | OPTIMAL |
| **Total** | **27** | **27** | **+0** | | |

### 8. Arithmetic: Let Binding Elimination

All four `let` bindings are compiled away entirely -- no `alloca`, no `store`, no `load`. Values flow directly as SSA registers:

- `let x = 3` -- constant `i64 3` passed directly to `@_ori_add` as first argument
- `let y = 4` -- constant `i64 4` passed directly to `@_ori_add` as second argument
- `let sum = add(...)` -- `%call` result feeds directly into `smul.with.overflow`
- `let result = sum * 5 - 2` -- `%sub.val` is the return value directly

This is excellent codegen -- many compilers at `-O0` would emit alloca+store+load for each binding. Ori's codegen emits direct SSA, which is closer to `-O1` quality even in debug builds. The IR treats let bindings as SSA value bindings rather than stack slots, which is the correct semantic for immutable scalar let bindings.

### 9. Arithmetic: Constant Propagation Opportunities

| Expression | Foldable? | Folded? | Notes |
|------------|-----------|---------|-------|
| `add(a: 3, b: 4)` | YES (interprocedural) | NO | Call emitted -- acceptable, requires inlining |
| `7 * 5` | Depends on fold of add | NO | Would require folding add first |
| `35 - 2` | Depends on fold of mul | NO | Would require folding multiply first |

The codegen does not perform interprocedural constant folding -- `@add` is a separate function that might have side effects from overflow checking. LLVM's `-O1`+ passes would inline `@add` and fold the entire main body to `ret i64 33`. Current `-O0` behavior is correct -- constant folding across function boundaries is an optimization, not a correctness requirement. At `-O0`, maintaining separate functions is better for debugging (breakpoints, stack traces).

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE     | Attributes | `memory(none)` on `@_ori_add` -- pure function optimization | NEW | J1 |
| 2 | NOTE     | Attributes | `noreturn` + `cold` present on `ori_panic_cstr` | NEW | J1 |
| 3 | NOTE     | Attributes | `noundef` on user function params and returns | NEW | J1 |
| 4 | NOTE     | Instruction Purity | Let bindings eliminated to direct SSA -- O1-quality at O0 | NEW | J1 |
| 5 | NOTE     | Instruction Purity | All user functions match ideal IR exactly -- OPTIMAL | NEW | J1 |

### NOTE-1: `memory(none)` on `@_ori_add`

**Location**: `define fastcc noundef i64 @_ori_add(i64 noundef %0, i64 noundef %1) #0` where `#0 = { nounwind memory(none) uwtable }`
**Impact**: Positive. The `memory(none)` attribute tells LLVM that `@_ori_add` has no observable memory effects -- it neither reads from nor writes to memory. This is correct for a pure arithmetic function that only operates on its scalar arguments. This enables LLVM to perform more aggressive optimizations: dead call elimination, call reordering, and common subexpression elimination.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-2: `noreturn` + `cold` present on `ori_panic_cstr`

**Location**: `declare void @ori_panic_cstr(ptr) #3` where `#3 = { cold noreturn }`
**Impact**: Positive. Both attributes are present. `noreturn` allows LLVM to eliminate dead code after panic calls. `cold` helps branch prediction heuristics place panic paths at the end of the function.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: `noundef` on user function parameters and returns

**Location**: `@_ori_add` params (`i64 noundef %0, i64 noundef %1`) and return (`noundef i64`); `@_ori_main` return (`noundef i64`); `@main` return (`noundef i32`)
**Impact**: Positive. The `noundef` attribute tells LLVM that these values are always well-defined. This enables additional optimizations -- particularly for signed overflow checking, where undefined behavior semantics are critical.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-4: Let bindings eliminated to direct SSA

**Location**: `_ori_main` -- all four `let` bindings compiled to SSA registers
**Impact**: Positive. No `alloca`/`store`/`load` chains for scalar let bindings. Values flow directly from definition to use as SSA values. This is `-O1` quality codegen in a debug build.
**Found in**: Arithmetic: Let Binding Elimination (Category 8)

### NOTE-5: All user functions match ideal IR exactly

**Location**: `_ori_add` and `_ori_main`
**Impact**: Positive. Both user functions produce instruction counts exactly matching the hand-written ideal IR. @add: 7/7. @main: 14/14. The entire module has zero unjustified overhead.
**Found in**: Optimal IR Comparison (Category 7)

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

Journey 1's arithmetic codegen achieves a perfect score. Both `@add` and `@main` match the hand-written ideal IR instruction-for-instruction with zero overhead beyond mandatory overflow checking. All attributes are correctly applied -- `memory(none)` on the pure `@_ori_add`, `nounwind` on all user functions via fixed-point analysis, `noundef` on all parameters and returns, and `cold noreturn` on panic paths. ARC is correctly absent for pure scalar arithmetic -- zero RC operations. The main wrapper now integrates RC leak detection via `ori_check_leaks`, which is optimal for the AIMS pipeline.
