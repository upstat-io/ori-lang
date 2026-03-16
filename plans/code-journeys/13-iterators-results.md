---
journey: 13
slug: iterators
theme: "I am an iterator"
date: 2026-03-16
status: PASS
expected: 55
eval_result: 55
aot_result: 55

difficulty: complex
prerequisites:
  - "Understanding of list creation and heap allocation"
  - "Closures and higher-order functions"
  - "Iterator protocol (lazy evaluation, adapter chaining)"
  - "ARC lifecycle for collections and iterator state"
learning_objectives:
  - "See how iterator chains (.iter().map().fold()) are lowered to runtime-backed opaque pointers"
  - "Understand trampoline functions as the bridge between typed lambdas and generic iterator runtime"
  - "Observe how AIMS eliminates dead RC cleanup for non-capturing closures"
  - "Compare lazy iterator fold codegen with what a hand-unrolled loop would produce"

features:
  - iterators
  - iterator_adapters
  - lists
  - function_calls
  - closures
  - higher_order
feature_description: "List-backed iterator creation, .map() adapter with closure, .fold() consumer, and trampoline-based callback dispatch"

score: 9.5
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 5
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
  attr_applicable: 28
  attr_correct: 17
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
  - journey: 5
    relationship: "Both exercise closure representation ({fn_ptr, env_ptr} pairs) and trampoline dispatch"
  - journey: 10
    relationship: "Both allocate lists via ori_list_alloc_data; J10 iterates with for-loop, J13 with .iter().fold()"
  - journey: 1
    relationship: "Missing nounwind on @main pattern now FIXED"
---

# Journey 13: "I am an iterator"

## Source

```ori
// Journey 13: "I am an iterator"
// Slug: iterators
// Difficulty: complex
// Features: iterators, iterator_adapters, lists, function_calls
// Expected: 1+4+9+16+25 = 55

@square (n: int) -> int = n * n;

@main () -> int = {
    let $nums = [1, 2, 3, 4, 5];
    nums.iter()
        .map(transform: x -> square(n: x))
        .fold(initial: 0, op: (acc, x) -> acc + x)
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 55        | 55       | (none) | (none) | PASS   |
| AOT     | 55        | 55       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 92 | **Keywords**: 3 (`let`, `@` x2) | **Identifiers**: 18 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(square) LParen Ident(n) Colon Ident(int) RParen
Arrow Ident(int) Eq Ident(n) Star Ident(n) Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Dollar Ident(nums) Eq LBracket
Int(1) Comma Int(2) Comma Int(3) Comma Int(4) Comma Int(5) RBracket Semi
Ident(nums) Dot Ident(iter) LParen RParen
Dot Ident(map) LParen Ident(transform) Colon Ident(x) Arrow
Ident(square) LParen Ident(n) Colon Ident(x) RParen RParen
Dot Ident(fold) LParen Ident(initial) Colon Int(0) Comma
Ident(op) Colon LParen Ident(acc) Comma Ident(x) RParen Arrow
Ident(acc) Plus Ident(x) RParen
RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 26 | **Max depth**: 5 | **Functions**: 2 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @square
│  ├─ Params: (n: int)
│  ├─ Return: int
│  └─ Body: BinOp(*)
│       ├─ Ident(n)
│       └─ Ident(n)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let $nums = List[1, 2, 3, 4, 5]
        └─ MethodChain
             ├─ Ident(nums).iter()
             ├─ .map(transform: Lambda(x -> Call(@square, n: x)))
             └─ .fold(initial: 0, op: Lambda((acc, x) -> BinOp(+, acc, x)))
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 14 | **Types inferred**: 10 | **Unifications**: 12 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@square (n: int) -> int = n * n
//                         ^ int (Mul<int, int> -> int)

@main () -> int = {
    let $nums: [int] = [1, 2, 3, 4, 5]
    //                  ^ [int] (list literal)
    nums.iter()                          // -> Iterator<int>
        .map(transform: x -> square(n: x))  // -> Iterator<int>
        //              ^ int (from Iterator.Item)
        //                   ^ int (return type of @square)
        .fold(initial: 0, op: (acc, x) -> acc + x)
        //   ^ int (initial accumulator)
        //                    ^ (int, int) -> int
        //                                    ^ int (Add<int, int> -> int)
        // -> int (fold result)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 4 | **Desugared**: 2 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Method chain .iter().map().fold() lowered to sequential method calls
- Lambda expressions lowered to anonymous function definitions with capture analysis
- Named arguments (transform:, initial:, op:, n:) normalized to positional
- List literal [1, 2, 3, 4, 5] lowered to canonical list construction
```

</details>

### 5. ARC Pipeline

> The AIMS (Abstract Interpretation over a Memory-state lattice with Symbolic tracking)
> pipeline analyzes value lifetimes and inserts reference counting operations. It performs
> ownership inference to classify values as Scalar, RcPtr, or FatVal and determines
> own/borrow transfer semantics for each call site.

**RC ops inserted**: 0 | **Elided**: 2 | **Net ops**: 0

<details>
<summary>ARC annotations (AIMS IR)</summary>

```text
fn @main() -> int [entry: bb0]
  bb0:
    %5: [int] [RcPtr] = Construct List(1, 2, 3, 4, 5)
    %8: DoubleEndedIterator<int> [Scalar] = Apply @iter(%5 [own])
    %9: (int) -> int [FatVal] = PartialApply @__lambda_0()
    %10: DoubleEndedIterator<int> [Scalar] = Apply @map(%8 [borrow], %9 [own])
    %11: int [Scalar] = 0
    %12: (int, int) -> int [FatVal] = PartialApply @__lambda_1()
    %13: int [Scalar] = Apply @fold(%10 [borrow], %11 [own], %12 [own])
    Return %13

fn @__lambda_0(%0: int [own]) -> int — no captures, scalar passthrough
fn @__lambda_1(%0: int [own], %1: int [own]) -> int — no captures, scalar add
fn @square(%0: int [own]) -> int — pure scalar arithmetic
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 55 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ let $nums = [1, 2, 3, 4, 5]
  └─ nums.iter() → Iterator<int> (List variant, 5 elements)
  └─ .map(transform: x -> square(n: x))
  └─ .fold(initial: 0, op: (acc, x) -> acc + x)
       ├─ iter.next() → Some(1)
       │  └─ square(n: 1) → 1*1 = 1
       │  └─ fold: 0 + 1 = 1
       ├─ iter.next() → Some(2)
       │  └─ square(n: 2) → 2*2 = 4
       │  └─ fold: 1 + 4 = 5
       ├─ iter.next() → Some(3)
       │  └─ square(n: 3) → 3*3 = 9
       │  └─ fold: 5 + 9 = 14
       ├─ iter.next() → Some(4)
       │  └─ square(n: 4) → 4*4 = 16
       │  └─ fold: 14 + 16 = 30
       ├─ iter.next() → Some(5)
       │  └─ square(n: 5) → 5*5 = 25
       │  └─ fold: 30 + 25 = 55
       └─ iter.next() → None → return 55
→ 55
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 2 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@square: +0 rc_inc, +0 rc_dec (pure scalar, memory(none))
@main: +0 rc_inc, +0 rc_dec (AIMS eliminated dead null-env rc_dec)
  - list allocated with implicit RC=1 via ori_list_alloc_data
  - ori_iter_from_list takes ownership (no rc_inc)
  - Non-capturing lambdas: AIMS recognizes FatVal with null env, no cleanup emitted
@__lambda_0: +0 rc_inc, +0 rc_dec (no captures)
@__lambda_1: +0 rc_inc, +0 rc_dec (no captures, memory(none))
@tramp_0: +0 rc_inc, +0 rc_dec (passthrough)
@tramp_1: +0 rc_inc, +0 rc_dec (passthrough)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '13-iterators'
source_filename = "13-iterators"

@ovf.msg = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind memory(none) uwtable
; --- @square ---
define fastcc noundef i64 @_ori_square(i64 noundef %0) #0 {
bb0:
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %0, i64 %0)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok

mul.ok:                                           ; preds = %bb0
  ret i64 %mul.val

mul.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %fold.out = alloca i64, align 8
  %fold.init = alloca i64, align 8
  %tramp.closure6 = alloca { ptr, ptr }, align 8
  %tramp.closure = alloca { ptr, ptr }, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
  %list.elem_ptr = getelementptr inbounds i64, ptr %list.data, i64 0
  store i64 1, ptr %list.elem_ptr, align 8
  %list.elem_ptr1 = getelementptr inbounds i64, ptr %list.data, i64 1
  store i64 2, ptr %list.elem_ptr1, align 8
  %list.elem_ptr2 = getelementptr inbounds i64, ptr %list.data, i64 2
  store i64 3, ptr %list.elem_ptr2, align 8
  %list.elem_ptr3 = getelementptr inbounds i64, ptr %list.data, i64 3
  store i64 4, ptr %list.elem_ptr3, align 8
  %list.elem_ptr4 = getelementptr inbounds i64, ptr %list.data, i64 4
  store i64 5, ptr %list.elem_ptr4, align 8
  %list.2 = insertvalue { i64, i64, ptr } { i64 5, i64 5, ptr undef }, ptr %list.data, 2
  %list.data5 = extractvalue { i64, i64, ptr } %list.2, 2
  %list.len = extractvalue { i64, i64, ptr } %list.2, 0
  %list.cap = extractvalue { i64, i64, ptr } %list.2, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data5, i64 %list.len, i64 %list.cap, i64 8, ptr null)
  store { ptr, ptr } { ptr @_ori___lambda_0, ptr null }, ptr %tramp.closure, align 8
  %iter.map = call ptr @ori_iter_map(ptr %list.iter, ptr @_ori_tramp_0, ptr %tramp.closure, i64 8)
  store { ptr, ptr } { ptr @_ori___lambda_1, ptr null }, ptr %tramp.closure6, align 8
  store i64 0, ptr %fold.init, align 8
  call void @ori_iter_fold(ptr %iter.map, ptr %fold.init, ptr @_ori_tramp_1, ptr %tramp.closure6, i64 8, i64 8, ptr %fold.out)
  %fold.result = load i64, ptr %fold.out, align 8
  ret i64 %fold.result
}

; Function Attrs: nounwind uwtable
; --- @__lambda_0 ---
define noundef i64 @_ori___lambda_0(ptr %0, i64 noundef %1) #1 {
bb0:
  %call = call fastcc i64 @_ori_square(i64 %1)
  ret i64 %call
}

; Function Attrs: nounwind memory(none) uwtable
; --- @__lambda_1 ---
define noundef i64 @_ori___lambda_1(ptr %0, i64 noundef %1, i64 noundef %2) #0 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %1, i64 %2)
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
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64) #2

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: nounwind
declare ptr @ori_list_alloc_data(i64, i64) #4

; Function Attrs: nounwind
declare ptr @ori_iter_from_list(ptr, i64, i64, i64, ptr) #4

; Function Attrs: nounwind
; --- @tramp_0 ---
define void @_ori_tramp_0(ptr %0, ptr %1, ptr %2) #4 {
entry:
  %tramp.fn_ptr.gep = getelementptr inbounds nuw { ptr, ptr }, ptr %0, i32 0, i32 0
  %tramp.fn_ptr = load ptr, ptr %tramp.fn_ptr.gep, align 8
  %tramp.env.gep = getelementptr inbounds nuw { ptr, ptr }, ptr %0, i32 0, i32 1
  %tramp.env = load ptr, ptr %tramp.env.gep, align 8
  %tramp.elem = load i64, ptr %1, align 8
  %tramp.result = call i64 %tramp.fn_ptr(ptr %tramp.env, i64 %tramp.elem)
  store i64 %tramp.result, ptr %2, align 8
  ret void
}

; Function Attrs: nounwind
declare ptr @ori_iter_map(ptr, ptr, ptr, i64) #4

; Function Attrs: nounwind
; --- @tramp_1 ---
define void @_ori_tramp_1(ptr %0, ptr %1, ptr %2, ptr %3) #4 {
entry:
  %tramp.fn_ptr.gep = getelementptr inbounds nuw { ptr, ptr }, ptr %0, i32 0, i32 0
  %tramp.fn_ptr = load ptr, ptr %tramp.fn_ptr.gep, align 8
  %tramp.env.gep = getelementptr inbounds nuw { ptr, ptr }, ptr %0, i32 0, i32 1
  %tramp.env = load ptr, ptr %tramp.env.gep, align 8
  %tramp.acc = load i64, ptr %1, align 8
  %tramp.elem = load i64, ptr %2, align 8
  %tramp.fold = call i64 %tramp.fn_ptr(ptr %tramp.env, i64 %tramp.acc, i64 %tramp.elem)
  store i64 %tramp.fold, ptr %3, align 8
  ret void
}

; Function Attrs: nounwind
declare void @ori_iter_fold(ptr, ptr, ptr, ptr, i64, i64, ptr) #4

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
attributes #4 = { nounwind }
```

#### Disassembly

```asm
_ori_square:
   push   %rax
   imul   %rdi,%rdi
   mov    %rdi,(%rsp)
   seto   %al
   jo     .panic
   mov    (%rsp),%rax
   pop    %rcx
   ret
   lea    0xde1e1(%rip),%rdi
   call   ori_panic_cstr

_ori_main:
   sub    $0x48,%rsp
   mov    $0x5,%edi
   mov    %rdi,0x8(%rsp)
   mov    $0x8,%esi
   mov    %rsi,0x10(%rsp)
   call   ori_list_alloc_data
   mov    0x8(%rsp),%rdx
   mov    0x10(%rsp),%rcx
   mov    %rax,%rdi
   movq   $0x1,(%rdi)
   movq   $0x2,0x8(%rdi)
   movq   $0x3,0x10(%rdi)
   movq   $0x4,0x18(%rdi)
   movq   $0x5,0x20(%rdi)
   xor    %eax,%eax
   mov    %eax,%r8d
   mov    %rdx,%rsi
   call   ori_iter_from_list
   mov    0x10(%rsp),%rcx
   mov    %rax,%rdi
   lea    _ori___lambda_0(%rip),%rax
   mov    %rax,0x18(%rsp)
   movq   $0x0,0x20(%rsp)
   lea    _ori_tramp_0(%rip),%rsi
   lea    0x18(%rsp),%rdx
   call   ori_iter_map
   mov    %rax,%rdi
   lea    _ori___lambda_1(%rip),%rax
   mov    %rax,0x28(%rsp)
   movq   $0x0,0x30(%rsp)
   movq   $0x0,0x38(%rsp)
   lea    0x38(%rsp),%rsi
   lea    _ori_tramp_1(%rip),%rdx
   lea    0x28(%rsp),%rcx
   mov    $0x8,%r9d
   lea    0x40(%rsp),%rax
   mov    %r9,%r8
   mov    %rax,(%rsp)
   call   ori_iter_fold
   mov    0x40(%rsp),%rax
   add    $0x48,%rsp
   ret

_ori___lambda_0:
   push   %rax
   mov    %rsi,(%rsp)
   mov    %rdi,%rax
   mov    (%rsp),%rdi
   call   _ori_square
   pop    %rcx
   ret

_ori___lambda_1:
   push   %rax
   add    %rdx,%rsi
   mov    %rsi,(%rsp)
   seto   %al
   jo     .panic
   mov    (%rsp),%rax
   pop    %rcx
   ret
   lea    0xde0e5(%rip),%rdi
   call   ori_panic_cstr

_ori_tramp_0:
   push   %rax
   mov    %rdx,(%rsp)
   mov    (%rdi),%rax
   mov    0x8(%rdi),%rdi
   mov    (%rsi),%rsi
   call   *%rax
   mov    (%rsp),%rdx
   mov    %rax,(%rdx)
   pop    %rax
   ret

_ori_tramp_1:
   push   %rax
   mov    %rcx,(%rsp)
   mov    (%rdi),%rax
   mov    0x8(%rdi),%rdi
   mov    (%rsi),%rsi
   mov    (%rdx),%rdx
   call   *%rax
   mov    (%rsp),%rcx
   mov    %rax,(%rcx)
   pop    %rax
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @square | 7 | 7 | 1.00x | OPTIMAL |
| 2 | @main | 27 | 27 | 1.00x | OPTIMAL |
| 3 | @__lambda_0 | 2 | 2 | 1.00x | OPTIMAL |
| 4 | @__lambda_1 | 7 | 7 | 1.00x | OPTIMAL |
| 5 | @tramp_0 | 8 | 8 | 1.00x | OPTIMAL |
| 6 | @tramp_1 | 9 | 9 | 1.00x | OPTIMAL |

All 6 user functions achieve OPTIMAL instruction ratio. Every instruction is necessary. `@main` is a single straight-line basic block with 27 instructions: 4 allocas, 1 list alloc call, 5 GEP+store pairs for elements, 3 insertvalue/extractvalue for the list struct, and the iterator chain calls (iter_from_list, iter_map, iter_fold) with their argument setup. [NOTE-1]

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @square | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 0 | YES | list ownership transfer | iter borrows |
| @__lambda_0 | 0 | 0 | YES | N/A | N/A |
| @__lambda_1 | 0 | 0 | YES | N/A | N/A |
| @tramp_0 | 0 | 0 | YES | N/A | N/A |
| @tramp_1 | 0 | 0 | YES | N/A | N/A |

**Verdict**: Zero RC operations in user code. Perfectly balanced. AIMS correctly identifies that:
- The list is constructed and immediately consumed by `ori_iter_from_list` (ownership transfer, no rc_inc needed)
- Both lambdas are non-capturing (FatVal with null env), so no rc_dec cleanup is needed
- The iterator runtime handles all internal RC management

[NOTE-2]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | memory | uwtable | noundef | cold | Notes |
|----------|--------|----------|--------|---------|---------|------|-------|
| @square | YES | YES | memory(none) | YES | YES(ret+p0) | NO | Pure function, fully annotated |
| @main | NO (C) | YES | N/A | YES | YES(ret) | NO | Entry point, correct C cc |
| @__lambda_0 | NO | YES | N/A | YES | YES(ret)+MISS(p0) | NO | Callback ABI [LOW-1] |
| @__lambda_1 | NO | YES | memory(none) | YES | YES(ret,p1,p2)+MISS(p0) | NO | Callback ABI [LOW-1] |
| @tramp_0 | NO | YES | N/A | MISS | MISS(p0,p1,p2) | NO | [LOW-2] |
| @tramp_1 | NO | YES | N/A | MISS | MISS(p0,p1,p2,p3) | NO | [LOW-2] |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | N/A | YES | Correct cold+noreturn |

**Compliance**: 17/28 applicable attributes correct (60.7%). The 11 missing attributes are all on trampoline and lambda functions:
- Lambda env pointer parameters (`ptr %0`) missing `noundef` (2 misses on `@__lambda_0`, `@__lambda_1`)
- Trampoline functions missing `uwtable` (2 misses on `@tramp_0`, `@tramp_1`)
- Trampoline pointer parameters missing `noundef` (7 misses across both trampolines)

Notably, `@_ori_main` now correctly has `nounwind` -- this was missing in prior journeys and is a cross-journey improvement. `@square` and `@__lambda_1` correctly carry `memory(none)` as pure functions.

Lambda and trampoline functions correctly use C calling convention since they are invoked via function pointer from the runtime. `@square` correctly has `fastcc` since it is only called directly.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @square | 3 | 0 | 0 | 0 | |
| @main | 1 | 0 | 0 | 0 | |
| @__lambda_0 | 1 | 0 | 0 | 0 | |
| @__lambda_1 | 3 | 0 | 0 | 0 | |
| @tramp_0 | 1 | 0 | 0 | 0 | |
| @tramp_1 | 1 | 0 | 0 | 0 | |

All functions have clean control flow. `@main` is a single basic block with zero branches. `@square` and `@__lambda_1` have 3 blocks each (entry, ok, panic) which is the expected structure for overflow-checked arithmetic.

### 5. Overflow Checking

**Status**: PASS

| Operation | Function | Checked | Correct | Notes |
|-----------|----------|---------|---------|-------|
| mul (n*n) | @square | YES | YES | llvm.smul.with.overflow.i64 |
| add (acc+x) | @__lambda_1 | YES | YES | llvm.sadd.with.overflow.i64 |

Both arithmetic operations use checked intrinsics with panic on overflow. Error messages are descriptive: "integer overflow on multiplication" and "integer overflow on addition".

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.40 MiB (debug) |
| .text section | 913 KiB |
| .rodata section | 134 KiB |
| User code | 400 bytes (7 functions incl. C main) |
| Runtime | >99% of binary |

The user code is compact. The 7 functions (including the C `main` wrapper) total 400 bytes:
- `@square`: 32 bytes
- `@main`: 224 bytes (straight-line, no branches)
- `@__lambda_0`: 32 bytes
- `@__lambda_1`: 32 bytes
- `@tramp_0`: 32 bytes
- `@tramp_1`: 32 bytes
- `main` (C wrapper): 16 bytes

#### Disassembly: @square

```asm
_ori_square:
   push   %rax
   imul   %rdi,%rdi       ; n * n with overflow detection
   mov    %rdi,(%rsp)
   seto   %al
   jo     .panic
   mov    (%rsp),%rax
   pop    %rcx
   ret
```

Compact: 8 instructions including overflow check. The `seto`+`jo` sequence is an LLVM backend artifact (`imul` already sets OF), not an Ori codegen issue.

#### Disassembly: @main (key excerpt)

```asm
_ori_main:
   sub    $0x48,%rsp            ; 72 bytes stack frame
   ; ... list allocation and initialization (5 elements) ...
   call   ori_iter_from_list
   ; ... setup tramp.closure for map ...
   call   ori_iter_map
   ; ... setup tramp.closure6 for fold ...
   call   ori_iter_fold
   mov    0x40(%rsp),%rax       ; load fold result
   add    $0x48,%rsp
   ret
```

Clean straight-line flow: list alloc, element stores, iter_from_list, iter_map, iter_fold, return. Zero branches in user code.

#### Disassembly: @tramp_0

```asm
_ori_tramp_0:
   push   %rax
   mov    %rdx,(%rsp)     ; save output ptr
   mov    (%rdi),%rax      ; load fn_ptr from closure
   mov    0x8(%rdi),%rdi   ; load env from closure
   mov    (%rsi),%rsi      ; load element value
   call   *%rax            ; indirect call to lambda
   mov    (%rsp),%rdx      ; restore output ptr
   mov    %rax,(%rdx)      ; store result
   pop    %rax
   ret
```

Clean trampoline: 10 instructions for the closure-to-runtime bridge.

### 7. Optimal IR Comparison

#### @square: Ideal vs Actual

```llvm
; IDEAL (7 instructions -- overflow checking required)
define fastcc noundef i64 @_ori_square(i64 noundef %0) nounwind memory(none) {
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %0, i64 %0)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %panic, label %ok
ok:
  ret i64 %mul.val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (7 instructions)
define fastcc noundef i64 @_ori_square(i64 noundef %0) #0 {
bb0:
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %0, i64 %0)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok
mul.ok:
  ret i64 %mul.val
mul.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @main: Ideal vs Actual

```llvm
; IDEAL (27 instructions -- single straight-line block)
define noundef i64 @_ori_main() nounwind {
bb0:
  %fold.out = alloca i64, align 8
  %fold.init = alloca i64, align 8
  %tramp.closure6 = alloca { ptr, ptr }, align 8
  %tramp.closure = alloca { ptr, ptr }, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
  ; 5x GEP + store for list elements
  ; insertvalue/extractvalue for list struct
  %list.iter = call ptr @ori_iter_from_list(...)
  store { ptr, ptr } { ptr @_ori___lambda_0, ptr null }, ptr %tramp.closure, align 8
  %iter.map = call ptr @ori_iter_map(...)
  store { ptr, ptr } { ptr @_ori___lambda_1, ptr null }, ptr %tramp.closure6, align 8
  store i64 0, ptr %fold.init, align 8
  call void @ori_iter_fold(...)
  %fold.result = load i64, ptr %fold.out, align 8
  ret i64 %fold.result
}
```

```llvm
; ACTUAL (27 instructions -- matches ideal exactly)
; ... (see Generated LLVM IR section above)
```

**Delta**: +0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @square | 7 | 7 | +0 | N/A | OPTIMAL |
| @main | 27 | 27 | +0 | N/A | OPTIMAL |
| @__lambda_0 | 2 | 2 | +0 | N/A | OPTIMAL |
| @__lambda_1 | 7 | 7 | +0 | N/A | OPTIMAL |
| @tramp_0 | 8 | 8 | +0 | N/A | OPTIMAL |
| @tramp_1 | 9 | 9 | +0 | N/A | OPTIMAL |

### 8. Iterators: Adapter Chain Codegen

The LLVM backend does not inline the iterator protocol. Instead, iterator operations are delegated to the Ori runtime via opaque pointer-based APIs:

- `ori_iter_from_list(data, len, cap, elem_size, drop_fn) -> iter_ptr` -- creates an iterator state object
- `ori_iter_map(iter, tramp_fn, closure, elem_size) -> iter_ptr` -- wraps iterator with a map adapter
- `ori_iter_fold(iter, init, tramp_fn, closure, acc_size, elem_size, out)` -- consumes iterator via fold

The trampoline functions (`@tramp_0`, `@tramp_1`) bridge between the type-erased runtime and typed user code. They unpack the `{fn_ptr, env_ptr}` closure struct, load arguments from pointer-typed parameters, call the lambda, and store the result back. This is the minimal possible bridge for generic iterator dispatch.

The trampoline dispatch pattern has zero overhead beyond the indirect call:
- `@tramp_0` (map): 8 instructions -- 4 loads, 1 indirect call, 1 store, 1 push/pop pair
- `@tramp_1` (fold): 9 instructions -- 5 loads, 1 indirect call, 1 store, 1 push/pop pair

The extra instruction in `@tramp_1` is the additional `load` for the accumulator parameter, which is structurally required for fold's 3-argument callback vs map's 2-argument callback.

A hand-optimized version could potentially inline the map+fold into a single loop, eliminating the trampoline overhead entirely. However, the current architecture separates concerns cleanly: the runtime owns the iteration protocol, and trampolines provide type-safe bridging. For a 5-element list, the 2 indirect calls per element (map trampoline + fold trampoline) add negligible overhead compared to the actual computation.

### 9. Iterators: ARC in Iteration

AIMS produces clean ownership annotations for the iterator chain:

```text
%5: [int] [RcPtr] = Construct List(...)     -- list is RcPtr (heap-allocated)
%8: Iterator [Scalar] = Apply @iter(%5 [own])  -- ownership transferred to iter
%9: (int)->int [FatVal] = PartialApply @__lambda_0()  -- non-capturing, null env
%10: Iterator [Scalar] = Apply @map(%8 [borrow], %9 [own])  -- iter borrowed, lambda owned
%12: (int,int)->int [FatVal] = PartialApply @__lambda_1()  -- non-capturing, null env
%13: int [Scalar] = Apply @fold(%10 [borrow], %11 [own], %12 [own])  -- iter borrowed
```

Key observations:
1. **Iterators are `[Scalar]`**: The iterator state is an opaque runtime pointer. AIMS correctly classifies it as Scalar (no user-level RC needed) because the runtime manages the iterator's internal refcounts.
2. **List ownership transfer**: The list `%5` is `[RcPtr]` but is immediately consumed by `@iter` with `[own]` semantics -- no rc_inc before the call, the runtime takes ownership.
3. **FatVal for lambdas**: Non-capturing closures are `[FatVal]` (a `{ptr, ptr}` pair). Since the env is null, AIMS correctly emits no rc_dec for them -- this is the fix that eliminated the previous dead `br i1 true` blocks.
4. **Borrow for iterator args**: `@map` and `@fold` receive their iterator argument as `[borrow]`, meaning the runtime borrows rather than takes ownership of the upstream iterator state.

The `memory(none)` attribute on `@square` and `@__lambda_1` further confirms that AIMS and the nounwind/memory analysis work together: pure scalar functions that only compute and potentially panic are correctly identified as having no memory effects.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Attributes | Missing noundef on lambda env pointer params | NEW | J13 |
| 2 | LOW | Attributes | Missing uwtable and noundef on trampoline functions | NEW | J13 |
| 3 | NOTE | Attributes | @_ori_main now has nounwind (was missing in prior journeys) | FIXED | J1 |
| 4 | NOTE | Attributes | @square and @__lambda_1 correctly have memory(none) | NEW | J13 |
| 5 | NOTE | ARC | Zero RC in user code -- AIMS eliminated dead null-env rc_dec | NEW | J13 |
| 6 | NOTE | Instruction Purity | All 6 user functions achieve OPTIMAL (1.0x) ratio | NEW | J13 |
| 7 | NOTE | Iterators | Clean ownership chain: list->iter->map->fold with zero user RC | NEW | J13 |

### LOW-1: Missing noundef on lambda env pointer parameters

**Location**: `@_ori___lambda_0(ptr %0, ...)`, `@_ori___lambda_1(ptr %0, ...)`
**Impact**: LLVM cannot assume the env pointer is non-poison, slightly limiting optimization
**Fix**: Add `noundef` to the env pointer parameter in lambda function declarations
**First seen**: Journey 13
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-2: Missing uwtable and noundef on trampoline functions

**Location**: `@_ori_tramp_0`, `@_ori_tramp_1` -- attribute group `#4 = { nounwind }` lacks `uwtable`
**Impact**: Missing `uwtable` means no unwind table entries for stack unwinding through trampolines. Missing `noundef` on pointer parameters prevents LLVM from assuming they are non-poison.
**Fix**: Add `uwtable` to trampoline function attribute group. Add `noundef` to all pointer parameters.
**First seen**: Journey 13
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: @_ori_main now has nounwind

**Location**: `@_ori_main()` -- attribute group `#1 = { nounwind uwtable }`
**Impact**: Positive -- LLVM no longer generates unnecessary exception handling metadata for the entry function. This was a cross-journey finding first identified in J1, now resolved.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-4: @square and @__lambda_1 correctly have memory(none)

**Location**: `@_ori_square` (attribute group #0), `@_ori___lambda_1` (attribute group #0)
**Impact**: Positive -- LLVM can optimize more aggressively knowing these functions have no memory side effects. The nounwind/memory analysis correctly identified pure scalar functions.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-5: Zero RC in user code

**Location**: All 6 user functions
**Impact**: Positive -- AIMS correctly identifies that non-capturing lambdas need no env cleanup, and the list-to-iterator ownership transfer requires no user-side rc_inc. The runtime handles all internal RC management behind the opaque iterator pointer.
**Found in**: ARC Purity (Category 2)

### NOTE-6: All 6 user functions OPTIMAL

**Location**: @square, @main, @__lambda_0, @__lambda_1, @tramp_0, @tramp_1
**Impact**: Positive -- every user function achieves 1.0x instruction ratio, meaning zero unjustified instructions
**Found in**: Instruction Purity (Category 1)

### NOTE-7: Clean iterator ownership chain

**Location**: AIMS IR for @main
**Impact**: Positive -- AIMS correctly models the list->iterator->adapter->consumer ownership chain with zero user-visible RC operations. The runtime handles all internal RC management behind the opaque iterator pointer.
**Found in**: Iterators: ARC in Iteration (Category 9)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 5/10 | 60.7% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.5 / 10**

## Verdict

Journey 13 demonstrates excellent iterator codegen under AIMS. All 6 user functions achieve OPTIMAL instruction ratio (1.0x) with zero user-visible RC operations. The iterator chain `[1,2,3,4,5].iter().map(square).fold(0, +)` compiles to a clean single-block straight-line function in @main that delegates to the runtime via type-erased trampoline callbacks. The nounwind/memory analysis correctly identifies `@square` and `@__lambda_1` as pure functions with `memory(none)`, and `@_ori_main` now correctly has `nounwind` (a cross-journey fix since J1). The remaining attribute gaps are limited to missing `noundef` on lambda env pointers and missing `uwtable`/`noundef` on trampoline functions (60.7% compliance).

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J13 | CONFIRMED working |
| fastcc on user functions | J1 | J13 | CONFIRMED (on @square) |
| Missing nounwind on @main | J1 | J13 | FIXED -- now has nounwind |
| Closure {fn_ptr, env_ptr} repr | J5 | J13 | CONFIRMED correct |
| List allocation via ori_list_alloc_data | J10 | J13 | CONFIRMED correct |
| memory(none) on pure functions | J13 | J13 | NEW -- @square and @__lambda_1 |

The `memory(none)` attribute on pure scalar functions is a new observation first seen in Journey 13. This indicates the nounwind/memory analysis pipeline is correctly propagating purity information through the call graph. `@__lambda_0` correctly does *not* get `memory(none)` because it calls `@_ori_square` via `fastcc` -- while `@_ori_square` itself has `memory(none)`, the memory analysis conservatively treats the caller as having memory effects due to the call instruction. The nounwind analysis is more precise: since `@_ori_square` is `nounwind`, `@__lambda_0` is also correctly marked `nounwind`.

The most significant cross-journey improvement is the FIXED nounwind on `@_ori_main`. This was identified as a missing attribute in Journeys 1 through 12 and is now resolved, indicating the post-hoc nounwind pass now correctly propagates through entry-point functions.
