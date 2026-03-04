---
journey: 1
slug: arithmetic
theme: "I am arithmetic"
date: 2026-03-03
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
  - "Learn what function attributes (nounwind, fastcc) mean and why they matter"
features:
  - arithmetic
  - function_calls
  - let_bindings
  - int_literals
  - multiple_functions
feature_description: "Basic arithmetic with function calls, let bindings, and integer operations"
score: 8.7
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 7
  control_flow: 8
  ir_quality: 9
  binary_quality: 8
overflow_check: PASS
bugs_found: []
related_journeys: []
---

# Journey 1: "I am arithmetic"

## Source

```ori
// Journey 1: "I am arithmetic"
// Slug: arithmetic
// Difficulty: beginner
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

> The lexer (tokenizer) breaks raw source text into a stream of tokens — the smallest
> meaningful units like keywords, identifiers, operators, and literals. This is the first
> stage of every compiler.

**Tokens**: 75 | **Keywords**: 4 | **Identifiers**: 11 | **Errors**: 0

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
> (AST) — a tree structure that represents the grammatical structure of the program.
> Operator precedence is resolved here: `*` binds tighter than `-`.

**Nodes**: 16 | **Max depth**: 4 | **Functions**: 2 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @add
│  ├─ Params: (a: int, b: int)
│  ├─ Return: int
│  └─ Body: BinOp(+)
│       ├─ Ident(a)
│       └─ Ident(b)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let x = Lit(3)
        ├─ Let y = Lit(4)
        ├─ Let sum = Call(@add)
        │    ├─ a: Ident(x)
        │    └─ b: Ident(y)
        ├─ Let result = BinOp(-)
        │    ├─ BinOp(*)
        │    │    ├─ Ident(sum)
        │    │    └─ Lit(5)
        │    └─ Lit(2)
        └─ Ident(result)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit annotations everywhere.

**Constraints**: 12 | **Types inferred**: 6 | **Unifications**: 10 | **Errors**: 0

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

> The canonicalizer transforms the typed AST into a simplified canonical form — a flat
> sequence of operations suitable for backend consumption. It desugars syntactic sugar,
> lowers complex expressions, and resolves named arguments to positional order.

**Transforms**: 2 | **Desugared**: 0 | **Errors**: 0

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
> reference counting operations. It performs borrow inference to minimize RC overhead —
> parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@add: no heap values — pure scalar arithmetic (int params, int return)
@main: no heap values — all let bindings hold int scalars
Total RC ops: 0 (optimal for scalar-only program)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without compilation.
> It serves as the reference implementation for correctness testing — if eval and AOT
> disagree, the bug is in LLVM codegen, not the interpreter.

**Result**: 33 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  ├─ let x = 3
  ├─ let y = 4
  ├─ let sum = @add(a: 3, b: 4)
  │    └─ 3 + 4 = 7
  ├─ let result = 7 * 5 - 2
  │    ├─ 7 * 5 = 35
  │    └─ 35 - 2 = 33
  └─ result = 33
→ 33
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
@_ori_add: +0 rc_inc, +0 rc_dec (pure scalar — no heap values)
@_ori_main: +0 rc_inc, +0 rc_dec (pure scalar — no heap values)
Nounwind analysis: 2 passes, both functions marked nounwind
```

</details>

#### Generated LLVM IR

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.2 = private unnamed_addr constant [32 x i8] c"integer overflow on subtraction\00", align 1

define fastcc i64 @_ori_add(i64 %0, i64 %1) #0 {
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

define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  br label %bb1

bb1:
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

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

#### Disassembly

```asm
_ori_add:
  push   %rax
  add    %rsi,%rdi
  mov    %rdi,(%rsp)
  seto   %al
  jo     panic
  mov    (%rsp),%rax
  pop    %rcx
  ret

_ori_main:
  sub    $0x18,%rsp
  mov    $0x3,%edi
  mov    $0x4,%esi
  call   _ori_add
  imul   $0x5,%rax,%rcx
  seto   %al
  jo     mul_panic
  sub    $0x2,%rcx
  seto   %al
  jo     sub_panic
  mov    %rcx,%rax
  add    $0x18,%rsp
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @add     | 7      | 7     | 1.00x | OPTIMAL |
| 2 | @main    | 16     | 15    | 1.07x | NEAR-OPTIMAL [MEDIUM-1] |
| 3 | main (wrapper) | 3 | 3  | 1.00x | OPTIMAL |

**@add (7 instructions)**: Every instruction is necessary — overflow-checked addition requires the intrinsic call, two extractvalues, a conditional branch, the return, the panic call, and unreachable. **OPTIMAL.**

**@main (16 instructions)**: 15 are necessary (call to add, two overflow-checked operations with full intrinsic sequences, return). The 1 extra is `br label %bb1` — a redundant unconditional branch between `bb0` and `bb1` that could be eliminated by merging the blocks.

**Let binding elimination**: All four `let` bindings (`x`, `y`, `sum`, `result`) are correctly eliminated — no `alloca`/`store`/`load` chains. Constants `3` and `4` are passed directly as arguments. The call result feeds directly into the multiply. **Excellent codegen for an unoptimized build.**

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @add     | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Correct — this program uses only `int` scalars (i64), which are value types requiring no reference counting. No `ori_rc_inc`, `ori_rc_dec`, or any RC-related calls present. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @add     | YES    | YES      | N/A     | N/A      | N/A  |       |
| @main    | NO (C) | YES      | N/A     | N/A      | N/A  | C conv for entry point — correct |
| main wrapper | NO (C) | NO  | N/A     | N/A      | N/A  | [LOW-2] |
| ori_panic_cstr | N/A | N/A | N/A     | N/A      | YES  | Missing noreturn [MEDIUM-2] |

**@_ori_add uses `fastcc`**: Correct. Internal function benefits from fast calling convention.

**@_ori_main uses C convention**: Correct. Called from the C `main()` wrapper, must use C ABI.

**ori_panic_cstr missing `noreturn`**: This function never returns (calls `longjmp` or `abort`). Missing `noreturn` prevents LLVM from eliminating dead code after panic calls.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @add     | 3      | 0           | 0                 | 0         | Optimal layout |
| @main    | 6      | 0           | 1                 | 0         | [MEDIUM-1] |
| main wrapper | 1  | 0           | 0                 | 0         | Optimal |

**@add**: 3 blocks — entry, happy path (`add.ok`), panic path (`add.ovf_panic`). Happy path is fallthrough. Panic block at end. **Optimal layout.**

**@main**: 6 blocks with 1 redundant unconditional branch (`bb0 → bb1`). These blocks should be merged — the branch exists because the codegen creates a new basic block at let-binding boundaries. Panic blocks are correctly placed after the happy path.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (`+`) | YES     | YES     | `llvm.sadd.with.overflow.i64` |
| mul (`*`) | YES     | YES     | `llvm.smul.with.overflow.i64` |
| sub (`-`) | YES     | YES     | `llvm.ssub.with.overflow.i64` |

All three arithmetic operations use LLVM overflow intrinsics with dedicated panic message strings. Each has a conditional branch to a `cold` panic path followed by `unreachable`. Correct per the Ori spec: "overflow panics."

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.26 MiB (debug) |
| .text section | 868 KiB |
| .rodata section | 133 KiB |
| .debug_info | 1.57 MiB |
| User code | ~134 bytes (add: 31, main: 101, wrapper: 8) |
| Runtime | ~99.99% of binary |

The binary is large due to static linking of `ori_rt` (the Ori runtime, which includes Rust stdlib for panic handling, I/O, etc.) and debug symbols. The user's actual code is 134 bytes — everything else is runtime infrastructure.

#### Disassembly: @add

```asm
_ori_add:                        ; 31 bytes, 8 instructions
  push   %rax                   ; frame setup
  add    %rsi,%rdi              ; a + b (sets overflow flag)
  mov    %rdi,(%rsp)            ; save result to stack
  seto   %al                    ; set AL if overflow
  jo     panic                  ; jump to panic if overflow
  mov    (%rsp),%rax            ; load result from stack
  pop    %rcx                   ; frame teardown
  ret
```

Note: the `mov %rdi,(%rsp)` / `mov (%rsp),%rax` is an unnecessary stack round-trip. An optimal sequence: `add %rsi,%rdi; jo panic; mov %rdi,%rax; ret` (4 instructions vs 8). This is expected for unoptimized (`-O0`) builds — LLVM's register allocator doesn't elide stack spills at `-O0`.

#### Disassembly: @main

```asm
_ori_main:                       ; 101 bytes, ~25 instructions
  sub    $0x18,%rsp              ; stack frame
  mov    $0x3,%edi               ; arg a = 3
  mov    $0x4,%esi               ; arg b = 4
  call   _ori_add               ; sum = add(3, 4)
  imul   $0x5,%rax,%rcx         ; sum * 5
  seto   %al                    ; overflow check
  jo     mul_panic
  sub    $0x2,%rcx              ; - 2
  seto   %al                    ; overflow check
  jo     sub_panic
  mov    %rcx,%rax              ; return value
  add    $0x18,%rsp
  ret
```

### 7. Optimal IR Comparison

#### @add: Ideal vs Actual

```llvm
; IDEAL (7 instructions — overflow checking is mandatory)
define fastcc i64 @_ori_add(i64 %a, i64 %b) nounwind {
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
define fastcc i64 @_ori_add(i64 %0, i64 %1) #0 {
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

**Delta**: 0 instructions. Matches ideal exactly. **OPTIMAL.**

#### @main: Ideal vs Actual

```llvm
; IDEAL (15 instructions)
define i64 @_ori_main() nounwind {
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
; ACTUAL (16 instructions — 1 extra)
define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  br label %bb1                ; ← REDUNDANT: unconditional branch to next block
bb1:
  ; ... (same as ideal from here)
}
```

**Delta**: +1 instruction — `br label %bb1` (redundant unconditional branch between let-binding blocks). LLVM's optimizer eliminates this at `-O1`+, but it shouldn't be emitted in the first place.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @add     | 7     | 7      | +0    | N/A       | OPTIMAL |
| @main    | 15    | 16     | +1    | NO        | NEAR-OPTIMAL |
| main wrapper | 3 | 3      | +0    | N/A       | OPTIMAL |

### 8. Arithmetic: Let Binding Elimination

All four `let` bindings are compiled away entirely — no `alloca`, no `store`, no `load`. Values flow directly as SSA registers:
- `let x = 3` → constant `i64 3` passed directly to `@_ori_add`
- `let y = 4` → constant `i64 4` passed directly to `@_ori_add`
- `let sum = add(...)` → `%call` result feeds directly into `smul.with.overflow`
- `let result = ...` → `%sub.val` is the return value directly

This is excellent codegen — many compilers at `-O0` would emit alloca+store+load for each binding. Ori's codegen emits direct SSA, which is closer to `-O1` quality even in debug builds.

### 9. Arithmetic: Constant Propagation

| Expression | Foldable? | Folded? | Notes |
|------------|-----------|---------|-------|
| `add(a: 3, b: 4)` | YES (interprocedural) | NO | Call emitted — acceptable, requires inlining |
| `7 * 5` | Depends on fold of add | NO | Would require folding add first |
| `35 - 2` | Depends on fold of mul | NO | Would require folding multiply first |

The codegen does not perform interprocedural constant folding — `@add` is a separate function that might have side effects from overflow checking. LLVM's `-O1`+ passes inline `@add` and fold the entire main to `ret i64 33`. Current `-O0` behavior is correct.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM   | Control Flow | Redundant unconditional branch in @main | NEW | J1 |
| 2 | MEDIUM   | Attributes | Missing `noreturn` on `ori_panic_cstr` | NEW | J1 |
| 3 | LOW      | Attributes | Missing `nounwind` on main wrapper | NEW | J1 |
| 4 | LOW      | Attributes | Missing `noundef` on function parameters | NEW | J1 |
| 5 | NOTE     | Instruction Purity | Let bindings eliminated to direct SSA — O1-quality at O0 | NEW | J1 |

### MEDIUM-1: Redundant unconditional branch in @main

**Location**: `_ori_main`, `bb0 → bb1`
**Impact**: 1 unnecessary instruction per call. Indicates codegen creates new basic blocks at let-binding boundaries even when no control flow diverges.
**Fix**: Merge sequential blocks when no control flow divergence occurs. The block splitter should not create a new block after a simple call expression.
**First seen**: Journey 1
**Found in**: Control Flow & Block Layout (Category 4)

### MEDIUM-2: Missing `noreturn` on `ori_panic_cstr` declaration

**Location**: `declare void @ori_panic_cstr(ptr)` — has `cold` but missing `noreturn`
**Impact**: LLVM may not fully optimize code paths after panic calls. Without `noreturn`, dead code after panic calls is not eliminated, and branch prediction hints are suboptimal.
**Fix**: Add `noreturn` to the `ori_panic_cstr` declaration attributes in `compiler/ori_llvm/src/codegen/runtime_decl/mod.rs`.
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-3: Missing `nounwind` on main wrapper

**Location**: `define i32 @main()`
**Impact**: Minimal — affects exception table generation. The wrapper only calls `_ori_main` (which is nounwind), so the wrapper is transitively nounwind.
**Fix**: Add `nounwind` to the `main` wrapper function.
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-4: Missing `noundef` on function parameters

**Location**: `_ori_add` parameters (`i64 %0`, `i64 %1`)
**Impact**: Minimal — LLVM can usually infer this. Adding `noundef` explicitly enables LLVM to assume defined values, improving poison propagation analysis.
**Fix**: Add `noundef` to all `i64` parameters in generated functions.
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-5: Let bindings eliminated to direct SSA

**Location**: `_ori_main` — all four `let` bindings compiled away
**Impact**: Positive — no `alloca`/`store`/`load` chains. Values flow directly as SSA registers. This is `-O1` quality codegen in a debug build. Many compilers emit stack operations for unoptimized let bindings.
**Found in**: Arithmetic: Let Binding Elimination (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 20% | 9/10 | 1 redundant branch in @main, @add is OPTIMAL |
| ARC Correctness | 20% | 10/10 | Zero RC ops — correct for scalar-only program |
| Attributes & Safety | 15% | 7/10 | Missing noreturn on panic, noundef on params, nounwind on wrapper |
| Control Flow | 15% | 8/10 | 1 redundant block boundary, panic blocks placed correctly |
| IR Quality | 20% | 9/10 | @add matches ideal exactly, @main has 1 extra instruction |
| Binary Quality | 10% | 8/10 | User code is 134 bytes, runtime dominates binary size |

**Overall: 8.7 / 10**

## Verdict

Journey 1's arithmetic codegen is strong for a debug build. The `@add` function matches hand-written ideal IR exactly — OPTIMAL. The only waste in `@main` is a single redundant unconditional branch at a let-binding boundary. All let bindings are eliminated to direct SSA (no alloca/store/load), which is `-O1` quality at `-O0`. ARC is irrelevant for pure scalar arithmetic — zero RC operations. The main improvement opportunities are missing function attributes (`noreturn` on panic, `noundef` on parameters).
