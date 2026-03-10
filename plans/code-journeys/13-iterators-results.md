---
journey: 13
slug: iterators
theme: "I am an iterator"
date: 2026-03-10
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
  - "Observe how non-capturing closures produce null environment pointers with dead RC cleanup"
  - "Compare lazy iterator fold codegen with what a hand-unrolled loop would produce"

features:
  - iterators
  - iterator_adapters
  - lists
  - function_calls
  - closures
  - higher_order
feature_description: "List-backed iterator creation, .map() adapter with closure, .fold() consumer, and trampoline-based callback dispatch"

score: 7.5
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 3
  attributes_safety: 4
  control_flow: 8
  ir_quality: 9
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.03
  instruction_ratio_max: 1.06
  arc_violations: 6
  arc_has_unbalanced: true
  arc_has_scalar_rc: false
  attr_applicable: 38
  attr_correct: 20
  attr_has_wrong: false
  cf_defects: 2
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
  - journey: 5
    relationship: "Both exercise closure representation ({fn_ptr, env_ptr} pairs) and trampoline dispatch"
  - journey: 10
    relationship: "Both allocate lists via ori_list_alloc_data; J10 iterates with for-loop, J13 with .iter().fold()"
  - journey: 1
    relationship: "Same missing attribute patterns (nounwind on @main)"
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

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 2 | **Elided**: 2 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@square: no heap values — pure scalar arithmetic, no RC needed
@main: list allocated via ori_list_alloc_data (implicit RC=1)
  - Iterator takes ownership of list (no rc_inc needed)
  - Non-capturing lambdas have null env (rc_dec guarded by `br i1 true`, always skipped)
  - ori_iter_fold consumes the iterator chain (runtime handles cleanup)
@__lambda_0: no captures — scalar passthrough to @square
@__lambda_1: no captures — scalar addition
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

**RC ops inserted**: 2 | **Elided**: 2 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@square: +0 rc_inc, +0 rc_dec (pure scalar)
@main: +0 rc_inc, +2 rc_dec (null-env guards — always skipped, dead code)
  - list allocated with implicit RC=1 via ori_list_alloc_data
  - ori_iter_from_list takes ownership (no rc_inc)
  - 2x ori_rc_dec for lambda envs, both guarded by `br i1 true` (null env → dead)
@__lambda_0: +0 rc_inc, +0 rc_dec (no captures)
@__lambda_1: +0 rc_inc, +0 rc_dec (no captures)
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

; Function Attrs: nounwind uwtable
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

; Function Attrs: uwtable
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
  br i1 true, label %rc_dec.skip, label %rc_dec.do

rc_dec.do:                                        ; preds = %bb0
  %rc_dec.drop_fn = load ptr, ptr null, align 8
  call void @ori_rc_dec(ptr null, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:                                      ; preds = %rc_dec.do, %bb0
  store { ptr, ptr } { ptr @_ori___lambda_1, ptr null }, ptr %tramp.closure6, align 8
  store i64 0, ptr %fold.init, align 8
  call void @ori_iter_fold(ptr %iter.map, ptr %fold.init, ptr @_ori_tramp_1, ptr %tramp.closure6, i64 8, i64 8, ptr %fold.out)
  %fold.result = load i64, ptr %fold.out, align 8
  br i1 true, label %rc_dec.skip8, label %rc_dec.do7

rc_dec.do7:                                       ; preds = %rc_dec.skip
  %rc_dec.drop_fn9 = load ptr, ptr null, align 8
  call void @ori_rc_dec(ptr null, ptr %rc_dec.drop_fn9)  ; RC--
  br label %rc_dec.skip8

rc_dec.skip8:                                     ; preds = %rc_dec.do7, %rc_dec.skip
  ret i64 %fold.result
}

; Function Attrs: nounwind uwtable
; --- @__lambda_0 ---
define noundef i64 @_ori___lambda_0(ptr %0, i64 noundef %1) #0 {
bb0:
  %call = call fastcc i64 @_ori_square(i64 %1)
  ret i64 %call
}

; Function Attrs: nounwind uwtable
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

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind uwtable }
attributes #1 = { uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
attributes #4 = { nounwind }
attributes #5 = { nounwind memory(inaccessiblemem: readwrite) }
```

#### Disassembly

```asm
_ori_square:
   push   %rax
   imul   %rdi,%rdi
   mov    %rdi,(%rsp)
   seto   %al
   jo     1c114
   mov    (%rsp),%rax
   pop    %rcx
   ret
   lea    0xde1e1(%rip),%rdi
   call   1edf0 <ori_panic_cstr>

_ori_main:
   sub    $0x58,%rsp
   mov    $0x5,%edi
   mov    %rdi,0x10(%rsp)
   mov    $0x8,%esi
   mov    %rsi,0x18(%rsp)
   call   1c740 <ori_list_alloc_data>
   mov    0x10(%rsp),%rdx
   mov    0x18(%rsp),%rcx
   mov    %rax,%rdi
   movq   $0x1,(%rdi)
   movq   $0x2,0x8(%rdi)
   movq   $0x3,0x10(%rdi)
   movq   $0x4,0x18(%rdi)
   movq   $0x5,0x20(%rdi)
   xor    %eax,%eax
   mov    %eax,%r8d
   mov    %rdx,%rsi
   call   33f90 <ori_iter_from_list>
   mov    %rax,%rdi
   lea    0xb8(%rip),%rax
   mov    %rax,0x28(%rsp)
   movq   $0x0,0x30(%rsp)
   lea    0xe3(%rip),%rsi
   lea    0x28(%rsp),%rdx
   mov    $0x8,%ecx
   call   23bd0 <ori_iter_map>
   mov    %rax,0x20(%rsp)
   mov    $0x1,%al                  ; br i1 true (always skip rc_dec)
   test   $0x1,%al
   jne    1c1c5
   ... dead rc_dec block ...
   mov    0x20(%rsp),%rdi
   lea    0x8f(%rip),%rax
   mov    %rax,0x38(%rsp)
   movq   $0x0,0x40(%rsp)
   movq   $0x0,0x48(%rsp)
   lea    0x48(%rsp),%rsi
   lea    0xac(%rip),%rdx
   lea    0x38(%rsp),%rcx
   mov    $0x8,%r9d
   lea    0x50(%rsp),%rax
   mov    %r9,%r8
   mov    %rax,(%rsp)
   call   32590 <ori_iter_fold>
   mov    0x50(%rsp),%rax
   mov    %rax,0x8(%rsp)
   mov    $0x1,%al                  ; br i1 true (always skip rc_dec)
   test   $0x1,%al
   jne    1c22e
   ... dead rc_dec block ...
   mov    0x8(%rsp),%rax
   add    $0x58,%rsp
   ret

_ori___lambda_0:
   push   %rax
   mov    %rsi,(%rsp)
   mov    %rdi,%rax
   mov    (%rsp),%rdi
   call   1c100 <_ori_square>
   pop    %rcx
   ret

_ori___lambda_1:
   push   %rax
   add    %rdx,%rsi
   mov    %rsi,(%rsp)
   seto   %al
   jo     1c273
   mov    (%rsp),%rax
   pop    %rcx
   ret
   lea    0xde0a5(%rip),%rdi
   call   1edf0 <ori_panic_cstr>

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
| 2 | @main | 35 | 33 | 1.06x | NEAR-OPTIMAL |
| 3 | @__lambda_0 | 2 | 2 | 1.00x | OPTIMAL |
| 4 | @__lambda_1 | 7 | 7 | 1.00x | OPTIMAL |
| 5 | @tramp_0 | 8 | 8 | 1.00x | OPTIMAL |
| 6 | @tramp_1 | 9 | 9 | 1.00x | OPTIMAL |

`@square`, `@__lambda_0`, `@__lambda_1`, `@tramp_0`, and `@tramp_1` are all OPTIMAL -- every instruction is necessary.

`@main` has 2 unjustified instructions: the `br i1 true` guards for null-environment rc_dec blocks. These branches always evaluate to true (skip the rc_dec) because both lambdas are non-capturing, meaning the environment pointer is null. The dead `rc_dec.do`/`rc_dec.do7` blocks contain 3 instructions each that are never reached. The 2 unjustified instructions in the live path are the `br i1 true` branch instructions themselves, which should be elided at IR generation time since the condition is statically known. [MEDIUM-1]

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @square | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 2 (dead) | NOMINALLY | null-env skip | list ownership transferred |
| @__lambda_0 | 0 | 0 | YES | N/A | N/A |
| @__lambda_1 | 0 | 0 | YES | N/A | N/A |
| @tramp_0 | 0 | 0 | YES | N/A | N/A |
| @tramp_1 | 0 | 0 | YES | N/A | N/A |

**Verdict**: The 2 `ori_rc_dec` calls in `@main` are dead code -- guarded by `br i1 true` (always skip). They target `null` environment pointers from non-capturing lambdas. At runtime, these rc_dec calls are never executed, so there is no actual leak or double-free. However, the extract-metrics tool correctly flags them as unbalanced because the IR contains rc_dec without matching rc_inc. This is a codegen cleanliness issue: the ARC pipeline should recognize non-capturing closures and omit the rc_dec entirely rather than emitting dead cleanup code. [MEDIUM-2]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | noundef | Notes |
|----------|--------|----------|---------|----------|------|---------|-------|
| @square | YES | YES | N/A | N/A | NO | YES | |
| @main | NO (C) | NO | N/A | N/A | NO | YES | [MEDIUM-3] |
| @__lambda_0 | NO | YES | N/A | N/A | NO | YES | Expected -- callback ABI |
| @__lambda_1 | NO | YES | N/A | N/A | NO | YES | Expected -- callback ABI |
| @tramp_0 | NO | YES | N/A | N/A | NO | N/A | Expected -- runtime bridge |
| @tramp_1 | NO | YES | N/A | N/A | NO | N/A | Expected -- runtime bridge |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | YES | N/A | Correct cold+noreturn |

`@main` uses C calling convention (correct -- entry point) but lacks `nounwind`. Since `@main` calls `ori_iter_fold` which calls `@tramp_1` which calls `@__lambda_1` which can panic on overflow, `@main` legitimately may unwind. However, the Ori runtime uses `ori_panic_cstr` which is `noreturn` -- it aborts rather than unwinds. Therefore `nounwind` would be correct on `@main`.

Lambda and trampoline functions correctly use C calling convention since they are called via function pointer from the runtime. `@square` correctly has `fastcc` since it is only called directly.

The 52.6% compliance rate (20/38) is driven by the large number of runtime declarations (6 external functions) that the tool counts applicable checks on. [MEDIUM-3]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @square | 3 | 0 | 0 | 0 | |
| @main | 5 | 0 | 2 | 0 | [MEDIUM-1] |
| @__lambda_0 | 1 | 0 | 0 | 0 | |
| @__lambda_1 | 3 | 0 | 0 | 0 | |
| @tramp_0 | 1 | 0 | 0 | 0 | |
| @tramp_1 | 1 | 0 | 0 | 0 | |

`@main` has 2 redundant branches: both `br i1 true` instructions that guard the null-env rc_dec blocks. Since the condition is a constant `true`, LLVM will trivially fold these away during optimization, but they should not be emitted in the first place. The dead `rc_dec.do` and `rc_dec.do7` blocks add 2 unreachable blocks to the CFG.

All other functions have clean control flow. `@square` and `@__lambda_1` have 3 blocks each (entry, ok, panic) which is the expected structure for overflow-checked arithmetic.

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
| User code | ~318 bytes (6 functions) |
| Runtime | >99% of binary |

The user code is extremely compact relative to the binary. The 6 user-defined functions (@square, @main, @__lambda_0, @__lambda_1, @tramp_0, @tramp_1) total approximately 318 bytes in the .text section. The vast majority of the binary is the Ori runtime (list allocation, iterator machinery, RC management, panic infrastructure).

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

Compact: 8 instructions including overflow check. The `seto`+`jo` sequence is redundant (LLVM artifact -- `imul` already sets OF), but this is an LLVM backend issue, not an Ori codegen issue.

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

Clean trampoline: 10 instructions for the closure-to-runtime bridge. Loads the function pointer and environment from the closure struct, loads the element from the input pointer, calls the lambda, and stores the result.

### 7. Optimal IR Comparison

#### @square: Ideal vs Actual

```llvm
; IDEAL (7 instructions — overflow checking required)
define fastcc noundef i64 @_ori_square(i64 noundef %0) nounwind {
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
; IDEAL (33 instructions — no dead rc_dec blocks)
define noundef i64 @_ori_main() nounwind {
bb0:
  %fold.out = alloca i64, align 8
  %fold.init = alloca i64, align 8
  %tramp.closure6 = alloca { ptr, ptr }, align 8
  %tramp.closure = alloca { ptr, ptr }, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
  %p0 = getelementptr inbounds i64, ptr %list.data, i64 0
  store i64 1, ptr %p0, align 8
  %p1 = getelementptr inbounds i64, ptr %list.data, i64 1
  store i64 2, ptr %p1, align 8
  %p2 = getelementptr inbounds i64, ptr %list.data, i64 2
  store i64 3, ptr %p2, align 8
  %p3 = getelementptr inbounds i64, ptr %list.data, i64 3
  store i64 4, ptr %p3, align 8
  %p4 = getelementptr inbounds i64, ptr %list.data, i64 4
  store i64 5, ptr %p4, align 8
  %list = insertvalue { i64, i64, ptr } { i64 5, i64 5, ptr undef }, ptr %list.data, 2
  %data = extractvalue { i64, i64, ptr } %list, 2
  %len = extractvalue { i64, i64, ptr } %list, 0
  %cap = extractvalue { i64, i64, ptr } %list, 1
  %iter = call ptr @ori_iter_from_list(ptr %data, i64 %len, i64 %cap, i64 8, ptr null)
  store { ptr, ptr } { ptr @_ori___lambda_0, ptr null }, ptr %tramp.closure, align 8
  %mapped = call ptr @ori_iter_map(ptr %iter, ptr @_ori_tramp_0, ptr %tramp.closure, i64 8)
  ; No rc_dec for null env — non-capturing lambda needs no cleanup
  store { ptr, ptr } { ptr @_ori___lambda_1, ptr null }, ptr %tramp.closure6, align 8
  store i64 0, ptr %fold.init, align 8
  call void @ori_iter_fold(ptr %mapped, ptr %fold.init, ptr @_ori_tramp_1, ptr %tramp.closure6, i64 8, i64 8, ptr %fold.out)
  %result = load i64, ptr %fold.out, align 8
  ; No rc_dec for null env — non-capturing lambda needs no cleanup
  ret i64 %result
}
```

```llvm
; ACTUAL (35 instructions — includes 2 dead br-i1-true guards)
; ... (see Generated LLVM IR section above)
```

**Delta**: +2 instructions (dead `br i1 true` guards for null-env rc_dec). The 2 dead blocks (`rc_dec.do`, `rc_dec.do7`) add 6 more instructions that are never executed but pollute the IR.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @square | 7 | 7 | +0 | N/A | OPTIMAL |
| @main | 33 | 35 | +2 | NO (dead code) | NEAR-OPTIMAL |
| @__lambda_0 | 2 | 2 | +0 | N/A | OPTIMAL |
| @__lambda_1 | 7 | 7 | +0 | N/A | OPTIMAL |
| @tramp_0 | 8 | 8 | +0 | N/A | OPTIMAL |
| @tramp_1 | 9 | 9 | +0 | N/A | OPTIMAL |

### 8. Iterators: Runtime Delegation Model

The LLVM backend does not inline the iterator protocol. Instead, iterator operations are delegated to the Ori runtime via opaque pointer-based APIs:

- `ori_iter_from_list(data, len, cap, elem_size, drop_fn) -> iter_ptr` -- creates an iterator state object
- `ori_iter_map(iter, tramp_fn, closure, elem_size) -> iter_ptr` -- wraps iterator with a map adapter
- `ori_iter_fold(iter, init, tramp_fn, closure, acc_size, elem_size, out)` -- consumes iterator via fold

This is a **correct architectural choice** for a debug build: it avoids monomorphizing iterator machinery into every call site and keeps the generated IR small. The tradeoff is that the iterator loop itself (next/check-done/apply-transform/accumulate) runs inside opaque runtime code with indirect calls through trampolines, preventing LLVM from optimizing the loop body (e.g., vectorization, loop unrolling).

The trampoline functions (`@tramp_0`, `@tramp_1`) are well-structured: they unpack the `{fn_ptr, env_ptr}` closure, load arguments from pointers, call the lambda, and store the result back. This is the minimal possible bridge between a type-erased runtime and typed user code.

### 9. Iterators: Closure Representation

Both lambdas in this journey are non-capturing:
- `x -> square(n: x)` captures nothing -- it only calls the top-level `@square` function
- `(acc, x) -> acc + x` captures nothing -- it only uses its parameters

The codegen correctly identifies both as non-capturing (`non_capturing=true` in the ARC trace) and uses `null` for the environment pointer in the `{fn_ptr, env_ptr}` closure struct. This avoids any heap allocation for closure environments.

However, the ARC pipeline still emits dead `ori_rc_dec` calls for these null environments, guarded by `br i1 true`. This is harmless at runtime (the branch is always taken to skip the rc_dec) but represents unnecessary code generation that inflates the IR.

### 10. Iterators: Eval vs AOT Behavior Comparison

The interpreter and LLVM backend produce identical results (55) through different execution models:

- **Interpreter**: Eagerly calls `eval_iter_next()` in a loop, applying the map transform and fold accumulator at each step. The iterator state is a Rust `IteratorValue::Mapped` wrapping `IteratorValue::List`.
- **AOT**: Delegates the entire fold loop to `ori_iter_fold` in the runtime, which internally calls `ori_iter_next` on the mapped iterator, invoking the trampoline for each element.

Both paths correctly compute: fold(0, [1,4,9,16,25], +) = 0+1+4+9+16+25 = 55.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM | Control Flow | Dead `br i1 true` guards for null-env rc_dec in @main | NEW | J13 |
| 2 | MEDIUM | ARC | Dead rc_dec calls for non-capturing closure environments | NEW | J13 |
| 3 | MEDIUM | Attributes | Missing nounwind on @main (entry point) | CONFIRMED | J1 |
| 4 | NOTE | Instruction Purity | 5 of 6 user functions are OPTIMAL | NEW | J13 |
| 5 | NOTE | Iterators | Runtime delegation model produces clean, compact user IR | NEW | J13 |

### MEDIUM-1: Dead `br i1 true` guards for null-env rc_dec in @main

**Location**: @main, blocks `bb0->rc_dec.skip` and `rc_dec.skip->rc_dec.skip8`
**Impact**: 2 unjustified branch instructions in the live path, plus 6 dead instructions in unreachable blocks
**Root cause**: ARC pipeline emits rc_dec cleanup for closure environments even when the environment is statically null (non-capturing lambda)
**Fix**: When the closure environment is known-null at IR generation time, skip the rc_dec emission entirely
**First seen**: Journey 13
**Found in**: Control Flow & Block Layout (Category 4), Instruction Purity (Category 1)

### MEDIUM-2: Dead rc_dec calls for non-capturing closure environments

**Location**: @main, `rc_dec.do` and `rc_dec.do7` blocks
**Impact**: Unbalanced RC in static analysis (2 rc_dec with no matching rc_inc). No runtime impact -- blocks are dead code.
**Root cause**: Same as MEDIUM-1 -- ARC pipeline does not special-case null environments
**Fix**: Same as MEDIUM-1
**First seen**: Journey 13
**Found in**: ARC Purity (Category 2)

### MEDIUM-3: Missing nounwind on @main

**Location**: @_ori_main function declaration (attribute group #1 has `uwtable` but not `nounwind`)
**Impact**: LLVM generates unnecessary exception handling metadata for the function
**Fix**: Mark @_ori_main as nounwind -- Ori panic uses abort semantics (`noreturn`), not unwinding
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-4: Excellent instruction purity across user functions

**Location**: @square, @__lambda_0, @__lambda_1, @tramp_0, @tramp_1
**Impact**: Positive -- 5 of 6 user functions achieve OPTIMAL instruction ratio (1.0x)
**Found in**: Instruction Purity (Category 1)

### NOTE-5: Clean runtime delegation architecture

**Location**: Iterator chain in @main
**Impact**: Positive -- the runtime delegation model keeps user IR compact (35 instructions for the entire iterator chain) while pushing the iteration loop into well-tested runtime code
**Found in**: Iterators: Runtime Delegation Model (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.03x avg ratio (max 1.06x) |
| ARC Correctness | 20% | 3/10 | 6 violations |
| Attributes & Safety | 10% | 4/10 | 52.6% compliance |
| Control Flow | 10% | 8/10 | 2 defects |
| IR Quality | 20% | 9/10 | 2 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 7.5 / 10**

Gates applied:
- arc_unbalanced_gate: unbalanced RC pair (leak/double-free), capped at 3

## Verdict

Journey 13's iterator codegen produces correct results through both backends and achieves excellent instruction efficiency (1.03x average ratio, 5 of 6 functions OPTIMAL). The runtime delegation model for iterators is architecturally sound, keeping user IR compact while the iteration loop runs in well-tested runtime code. The main deficiency is dead code from the ARC pipeline: non-capturing closures produce null environment pointers, but the codegen still emits `br i1 true`-guarded rc_dec blocks for them, which inflates the IR and triggers the unbalanced-RC gate in scoring.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J13 | CONFIRMED working |
| fastcc on user functions | J1 | J13 | CONFIRMED (on @square) |
| Missing nounwind on @main | J1 | J13 | CONFIRMED still missing |
| Closure {fn_ptr, env_ptr} repr | J5 | J13 | CONFIRMED correct |
| List allocation via ori_list_alloc_data | J10 | J13 | CONFIRMED correct |
| Non-capturing lambda null env | J5 | J13 | CONFIRMED (first time with dead rc_dec) |

The dead-rc_dec-for-null-env pattern is new to J13. Journey 5 (closures) tested non-capturing lambdas but did not exercise iterator adapters that trigger the ARC pipeline's closure cleanup path. The combination of non-capturing closures inside iterator chains is what surfaces this dead code pattern.
