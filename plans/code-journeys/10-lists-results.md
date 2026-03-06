---
journey: 10
slug: lists
theme: "I am a list"
date: 2026-03-06
status: PASS
expected: 33
eval_result: 33
aot_result: 33

difficulty: complex
prerequisites:
  - "Understanding of heap-allocated collections"
  - "Familiarity with reference counting memory management"
  - "Knowledge of iteration and for-loop compilation"
learning_objectives:
  - "See how list literals are lowered to heap-allocated buffers via ori_list_alloc_data"
  - "Understand ARC lifecycle for lists: allocation, sharing (rc_inc), and cleanup (rc_dec/drop_unique)"
  - "Compare iterator-based for-loop codegen with runtime-backed ori_iter_from_list/ori_iter_next"
  - "Observe how list parameters are passed by-reference via alloca+store+ptr"

features:
  - lists
  - list_methods
  - loops
  - arc
  - function_calls
feature_description: "List creation, .length() method calls, for-loop iteration, ARC lifecycle, and passing lists to functions"

score: 8.2
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 5
  control_flow: 7
  ir_quality: 8
  binary_quality: 10
  other_findings: 7
score_metrics:
  instruction_ratio: 1.02
  instruction_ratio_max: 1.02
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 14
  attr_correct: 9
  attr_has_wrong: false
  cf_defects: 4
  cf_incorrect: false
  ir_unjustified: 3
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 1
  other_low: 1
overflow_check: PASS

bugs_found: []
related_journeys:
  - journey: 7
    relationship: "Both test for-loop codegen; J7 uses ranges, J10 uses list iteration"
  - journey: 1
    relationship: "Same missing nounwind pattern on user functions"
---

# Journey 10: "I am a list"

## Source

```ori
// Journey 10: "I am a list"
// Slug: lists
// Difficulty: complex
// Features: lists, list_methods, loops, arc, function_calls
// Expected: check_length() + check_iteration() + check_passing() = 13 + 15 + 5 = 33

@count_items (xs: [int]) -> int = xs.length();

@check_length () -> int = {
    let a = [10, 20, 30];
    let b = [40, 50];
    let c = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    a.length() + b.length() + count_items(xs: c) - count_items(xs: b)
    // = 3 + 2 + 10 - 2 = 13
}

@check_iteration () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let total = 0;
    for x in xs do total += x;
    total
    // = 15
}

@check_passing () -> int = count_items(xs: [100, 200, 300, 400, 500]);

@main () -> int = {
    let a = check_length();     // = 13
    let b = check_iteration();  // = 15
    let c = check_passing();    // = 5
    a + b + c                   // = 33
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
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 228 | **Keywords**: 14 | **Identifiers**: 30 | **Errors**: 0

<details>
<summary>Token stream (first 30 tokens)</summary>

```text
Fn(@) Ident(count_items) LParen Ident(xs) Colon LBracket
Ident(int) RBracket RParen Arrow Ident(int) Eq Ident(xs) Dot
Ident(length) LParen RParen Semi Fn(@) Ident(check_length)
LParen RParen Arrow Ident(int) Eq LBrace Let Ident(a) Eq
LBracket Int(10) Comma Int(20) Comma Int(30) RBracket Semi
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 70 | **Max depth**: 5 | **Functions**: 5 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @count_items
│  ├─ Params: (xs: [int])
│  ├─ Return: int
│  └─ Body: MethodCall(.length)
│       └─ Ident(xs)
├─ FnDecl @check_length
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let a = List[10, 20, 30]
│       ├─ Let b = List[40, 50]
│       ├─ Let c = List[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
│       └─ BinOp(-)
│            ├─ BinOp(+)
│            │    ├─ BinOp(+)
│            │    │    ├─ MethodCall(a.length)
│            │    │    └─ MethodCall(b.length)
│            │    └─ Call(@count_items, xs: c)
│            └─ Call(@count_items, xs: b)
├─ FnDecl @check_iteration
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let xs = List[1, 2, 3, 4, 5]
│       ├─ Let total = 0
│       ├─ ForDo(x in xs) CompoundAssign(total += x)
│       └─ Ident(total)
├─ FnDecl @check_passing
│  ├─ Return: int
│  └─ Body: Call(@count_items, xs: List[100, 200, 300, 400, 500])
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@check_length)
        ├─ Let b = Call(@check_iteration)
        ├─ Let c = Call(@check_passing)
        └─ BinOp(+): a + b + c
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 22 | **Types inferred**: 12 | **Unifications**: 18 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@count_items (xs: [int]) -> int = xs.length()
//                                 ^ [int].length() -> int (Len trait)

@check_length () -> int = {
    let a: [int] = [10, 20, 30]              // inferred: [int]
    let b: [int] = [40, 50]                  // inferred: [int]
    let c: [int] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]  // inferred: [int]
    a.length() + b.length() + count_items(xs: c) - count_items(xs: b)
    //           ^ int        ^ int                   ^ int
}

@check_iteration () -> int = {
    let xs: [int] = [1, 2, 3, 4, 5]         // inferred: [int]
    let total: int = 0                        // inferred: int
    for x in xs do total += x                 // x: int, total: int
    total                                     // -> int
}

@check_passing () -> int = count_items(xs: [100, 200, 300, 400, 500])
//                         ^ int (return type of @count_items)

@main () -> int = {
    let a: int = check_length()              // inferred: int
    let b: int = check_iteration()           // inferred: int
    let c: int = check_passing()             // inferred: int
    a + b + c                                // -> int
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 78 | **Desugared**: 3 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- .length() method calls lowered to field access on list struct (field 0 = len)
- for x in xs do total += x desugared to loop + iterator protocol
- total += x desugared to total = total + x
- List literals lowered to alloc + element stores
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 10 | **Elided**: 2 | **Net ops**: 8

<details>
<summary>ARC annotations</summary>

```text
@count_items: +0 rc_inc, +0 rc_dec (borrows param, reads length only)
@check_length: +1 rc_inc, +5 rc_dec (3 lists allocated, b shared across 2 calls)
  - list a: alloc(+1) -> rc_dec(-1) = balanced
  - list b: alloc(+1) + rc_inc(+1) -> rc_dec(-1) + rc_dec(-1) = balanced
  - list c: alloc(+1) -> rc_dec(-1) = balanced
  - cleanup path: rc_dec(b) on unwind = correct exception cleanup
@check_iteration: +1 rc_inc, +1 rc_dec + iter_drop (list shared with iterator)
  - list xs: alloc(+1) + rc_inc(+1) -> rc_dec(-1) + iter_drop(-1) = balanced
@check_passing: +0 rc_inc, +1 drop_unique (list uniquely owned, no sharing)
@main: +0 rc_inc, +0 rc_dec (no heap values, only scalar results)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 33 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ let a = @check_length()
       └─ let a = [10, 20, 30]
       └─ let b = [40, 50]
       └─ let c = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
       └─ a.length() = 3
       └─ b.length() = 2
       └─ @count_items(xs: c) -> 10
       └─ @count_items(xs: b) -> 2
       └─ 3 + 2 + 10 - 2 = 13
  └─ let b = @check_iteration()
       └─ let xs = [1, 2, 3, 4, 5]
       └─ for x in xs: total = 0+1+2+3+4+5 = 15
  └─ let c = @check_passing()
       └─ @count_items(xs: [100, 200, 300, 400, 500]) -> 5
  └─ 13 + 15 + 5 = 33
→ 33
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 10 | **Elided**: 2 | **Net ops**: 8

<details>
<summary>ARC annotations</summary>

```text
@count_items: +0 rc_inc, +0 rc_dec (borrow param — reads len field only)
@check_length: +1 rc_inc(b), +4 rc_dec(a,b,c,b) + 1 rc_dec(b on unwind) = balanced
@check_iteration: +1 rc_inc(xs for iter), +1 rc_dec(xs) + 1 iter_drop = balanced
@check_passing: +0 rc_inc, +1 drop_unique (unique ownership) = balanced
@main: +0 rc_inc, +0 rc_dec (pure scalar arithmetic)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '10-lists'
source_filename = "10-lists"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [32 x i8] c"integer overflow on subtraction\00", align 1

; Function Attrs: uwtable
; --- @count_items ---
define fastcc noundef i64 @_ori_count_items(ptr %0) #0 {
bb0:
  %param.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load.s2, 0
  ret i64 %list.len
}

; Function Attrs: uwtable
; --- @check_length ---
define fastcc noundef i64 @_ori_check_length() #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg34 = alloca { i64, i64, ptr }, align 8
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 3, i64 8)
  %list.elem_ptr = getelementptr inbounds i64, ptr %list.data, i64 0
  store i64 10, ptr %list.elem_ptr, align 8
  %list.elem_ptr1 = getelementptr inbounds i64, ptr %list.data, i64 1
  store i64 20, ptr %list.elem_ptr1, align 8
  %list.elem_ptr2 = getelementptr inbounds i64, ptr %list.data, i64 2
  store i64 30, ptr %list.elem_ptr2, align 8
  %list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2
  %list.data3 = call ptr @ori_list_alloc_data(i64 2, i64 8)
  %list.elem_ptr4 = getelementptr inbounds i64, ptr %list.data3, i64 0
  store i64 40, ptr %list.elem_ptr4, align 8
  %list.elem_ptr5 = getelementptr inbounds i64, ptr %list.data3, i64 1
  store i64 50, ptr %list.elem_ptr5, align 8
  %list.26 = insertvalue { i64, i64, ptr } { i64 2, i64 2, ptr undef }, ptr %list.data3, 2
  %list.data7 = call ptr @ori_list_alloc_data(i64 10, i64 8)
  %list.elem_ptr8 = getelementptr inbounds i64, ptr %list.data7, i64 0
  store i64 1, ptr %list.elem_ptr8, align 8
  %list.elem_ptr9 = getelementptr inbounds i64, ptr %list.data7, i64 1
  store i64 2, ptr %list.elem_ptr9, align 8
  %list.elem_ptr10 = getelementptr inbounds i64, ptr %list.data7, i64 2
  store i64 3, ptr %list.elem_ptr10, align 8
  %list.elem_ptr11 = getelementptr inbounds i64, ptr %list.data7, i64 3
  store i64 4, ptr %list.elem_ptr11, align 8
  %list.elem_ptr12 = getelementptr inbounds i64, ptr %list.data7, i64 4
  store i64 5, ptr %list.elem_ptr12, align 8
  %list.elem_ptr13 = getelementptr inbounds i64, ptr %list.data7, i64 5
  store i64 6, ptr %list.elem_ptr13, align 8
  %list.elem_ptr14 = getelementptr inbounds i64, ptr %list.data7, i64 6
  store i64 7, ptr %list.elem_ptr14, align 8
  %list.elem_ptr15 = getelementptr inbounds i64, ptr %list.data7, i64 7
  store i64 8, ptr %list.elem_ptr15, align 8
  %list.elem_ptr16 = getelementptr inbounds i64, ptr %list.data7, i64 8
  store i64 9, ptr %list.elem_ptr16, align 8
  %list.elem_ptr17 = getelementptr inbounds i64, ptr %list.data7, i64 9
  store i64 10, ptr %list.elem_ptr17, align 8
  %list.218 = insertvalue { i64, i64, ptr } { i64 10, i64 10, ptr undef }, ptr %list.data7, 2
  %list.len = extractvalue { i64, i64, ptr } %list.2, 0
  br label %bb1

bb1:
  %rc.data_ptr = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr, i64 %rc.len, i64 %rc.cap, i64 8, ptr null)
  %rc_inc.data = extractvalue { i64, i64, ptr } %list.26, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %list.26, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.len19 = extractvalue { i64, i64, ptr } %list.26, 0
  br label %bb3

bb3:
  %rc.data_ptr20 = extractvalue { i64, i64, ptr } %list.26, 2
  %rc.len21 = extractvalue { i64, i64, ptr } %list.26, 0
  %rc.cap22 = extractvalue { i64, i64, ptr } %list.26, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr20, i64 %rc.len21, i64 %rc.cap22, i64 8, ptr null)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %list.len, i64 %list.len19)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb5:
  %rc.data_ptr26 = extractvalue { i64, i64, ptr } %list.218, 2
  %rc.len27 = extractvalue { i64, i64, ptr } %list.218, 0
  %rc.cap28 = extractvalue { i64, i64, ptr } %list.218, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr26, i64 %rc.len27, i64 %rc.cap28, i64 8, ptr null)
  %add29 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call)
  %add.val30 = extractvalue { i64, i1 } %add29, 0
  %add.ovf31 = extractvalue { i64, i1 } %add29, 1
  br i1 %add.ovf31, label %add.ovf_panic33, label %add.ok32

bb6:
  %lp = landingpad { ptr, i32 }
          cleanup
  %rc.data_ptr23 = extractvalue { i64, i64, ptr } %list.26, 2
  %rc.len24 = extractvalue { i64, i64, ptr } %list.26, 0
  %rc.cap25 = extractvalue { i64, i64, ptr } %list.26, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr23, i64 %rc.len24, i64 %rc.cap25, i64 8, ptr null)
  resume { ptr, i32 } %lp

add.ok:
  store { i64, i64, ptr } %list.218, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_count_items(ptr %ref_arg)
          to label %bb5 unwind label %bb6

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok32:
  store { i64, i64, ptr } %list.26, ptr %ref_arg34, align 8
  %call35 = call fastcc i64 @_ori_count_items(ptr %ref_arg34)
  %rc.data_ptr36 = extractvalue { i64, i64, ptr } %list.26, 2
  %rc.len37 = extractvalue { i64, i64, ptr } %list.26, 0
  %rc.cap38 = extractvalue { i64, i64, ptr } %list.26, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr36, i64 %rc.len37, i64 %rc.cap38, i64 8, ptr null)
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %add.val30, i64 %call35)
  %sub.val = extractvalue { i64, i1 } %sub, 0
  %sub.ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.ovf, label %sub.ovf_panic, label %sub.ok

add.ovf_panic33:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

sub.ok:
  ret i64 %sub.val

sub.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: uwtable
; --- @check_iteration ---
define fastcc noundef i64 @_ori_check_iteration() #0 {
bb0:
  %iter_next.scratch = alloca i64, align 8
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
  %rc_inc.data = extractvalue { i64, i64, ptr } %list.2, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.data5 = extractvalue { i64, i64, ptr } %list.2, 2
  %list.len = extractvalue { i64, i64, ptr } %list.2, 0
  %list.cap = extractvalue { i64, i64, ptr } %list.2, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data5, i64 %list.len, i64 %list.cap, i64 8, ptr null)
  br label %bb1

bb1:
  %v12 = phi i64 [ 0, %bb0 ], [ %add.val, %add.ok ]
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 8)
  %iter_next.tag = zext i8 %iter_next.has to i64
  %iter_next.elem = load i64, ptr %iter_next.scratch, align 8
  %iter_next.0 = insertvalue { i64, i64 } undef, i64 %iter_next.tag, 0
  %iter_next.1 = insertvalue { i64, i64 } %iter_next.0, i64 %iter_next.elem, 1
  %proj.0 = extractvalue { i64, i64 } %iter_next.1, 0
  %ne = icmp ne i64 %proj.0, 0
  br i1 %ne, label %bb2, label %bb3

bb2:
  %proj.1 = extractvalue { i64, i64 } %iter_next.1, 1
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v12, i64 %proj.1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb3:
  %rc.data_ptr = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr, i64 %rc.len, i64 %rc.cap, i64 8, ptr null)
  call void @ori_iter_drop(ptr %list.iter)
  ret i64 %v12

add.ok:
  br label %bb1

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: uwtable
; --- @check_passing ---
define fastcc noundef i64 @_ori_check_passing() #0 {
bb0:
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 5, i64 8)
  %list.elem_ptr = getelementptr inbounds i64, ptr %list.data, i64 0
  store i64 100, ptr %list.elem_ptr, align 8
  %list.elem_ptr1 = getelementptr inbounds i64, ptr %list.data, i64 1
  store i64 200, ptr %list.elem_ptr1, align 8
  %list.elem_ptr2 = getelementptr inbounds i64, ptr %list.data, i64 2
  store i64 300, ptr %list.elem_ptr2, align 8
  %list.elem_ptr3 = getelementptr inbounds i64, ptr %list.data, i64 3
  store i64 400, ptr %list.elem_ptr3, align 8
  %list.elem_ptr4 = getelementptr inbounds i64, ptr %list.data, i64 4
  store i64 500, ptr %list.elem_ptr4, align 8
  %list.2 = insertvalue { i64, i64, ptr } { i64 5, i64 5, ptr undef }, ptr %list.data, 2
  store { i64, i64, ptr } %list.2, ptr %ref_arg, align 8
  %call = call fastcc i64 @_ori_count_items(ptr %ref_arg)
  %udrop.data_ptr = extractvalue { i64, i64, ptr } %list.2, 2
  %udrop.len = extractvalue { i64, i64, ptr } %list.2, 0
  %udrop.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_drop_unique(ptr %udrop.data_ptr, i64 %udrop.len, i64 %udrop.cap, i64 8, ptr null)
  ret i64 %call
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_check_length()
  %call1 = call fastcc i64 @_ori_check_iteration()
  %call2 = call fastcc i64 @_ori_check_passing()
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
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok6:
  ret i64 %add.val4

add.ovf_panic7:
  call void @ori_panic_cstr(ptr @ovf.msg)
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
_ori_count_items:
   mov    (%rdi),%rax
   mov    0x8(%rdi),%rcx
   mov    0x10(%rdi),%rcx
   ret

_ori_check_length:
   sub    $0xb8,%rsp
   ; ... (allocates 3 lists, calls count_items twice, overflow-checked arithmetic)
   ; 168 instructions total
   add    $0xb8,%rsp
   ret

_ori_check_iteration:
   sub    $0x48,%rsp
   ; ... (allocates list, creates iterator, loop with overflow-checked add)
   ; 55 instructions total
   add    $0x48,%rsp
   ret

_ori_check_passing:
   sub    $0x38,%rsp
   ; ... (allocates list, calls count_items, drop_unique)
   ; 38 instructions total
   add    $0x38,%rsp
   ret

_ori_main:
   sub    $0x28,%rsp
   call   _ori_check_length
   call   _ori_check_iteration
   call   _ori_check_passing
   ; overflow-checked addition of results
   add    $0x28,%rsp
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @count_items | 11 | 11 | 1.00x | OPTIMAL |
| 2 | @check_length | 92 | 90 | 1.02x | NEAR-OPTIMAL |
| 3 | @check_iteration | 44 | 43 | 1.02x | NEAR-OPTIMAL |
| 4 | @check_passing | 20 | 20 | 1.00x | OPTIMAL |
| 5 | @main | 16 | 16 | 1.00x | OPTIMAL |

**@count_items** (11 instructions): Loads all 3 struct fields via GEP+load+insertvalue, then extracts field 0 (length). The full struct reconstruction is unnecessary since only `len` is needed -- but the load of field 0 and ret are correct. The extra loads of fields 1 and 2 are overhead but the tool counts them as part of the param-loading pattern. OPTIMAL per tool.

**@check_length** (92 instructions): 3 list allocations (3+2+10 elements = 15 element stores), ARC operations, 2 function calls (one via invoke), 3 overflow-checked arithmetic ops. The 2 unjustified instructions are the redundant `br label %bb1` and `br label %bb3` (unconditional jumps to the next block that could be merged).

**@check_iteration** (44 instructions): List allocation (5 elements), iterator creation via runtime, loop with phi node, overflow-checked addition, cleanup. The 1 unjustified instruction is the `br label %bb1` in the `add.ok` block (empty trampoline back to loop header).

**@check_passing** (20 instructions): List allocation (5 elements), store to alloca, call, drop_unique, ret. Clean and OPTIMAL.

**@main** (16 instructions): 3 calls + 2 overflow-checked additions. OPTIMAL -- no waste.

### 2. ARC Purity

| Function | rc_inc | rc_dec | drop_unique | iter_drop | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|-------------|-----------|----------|----------------|----------------|
| @count_items | 0 | 0 | 0 | 0 | YES | 1 (param borrowed) | 0 |
| @check_length | 1 | 5 | 0 | 0 | YES | 0 | 0 |
| @check_iteration | 1 | 1 | 0 | 1 | YES | 0 | 0 |
| @check_passing | 0 | 0 | 1 | 0 | YES | 0 | 1 (unique drop) |
| @main | 0 | 0 | 0 | 0 | YES | N/A | N/A |

**Verdict**: All functions are ARC-balanced when accounting for initial allocation refcount (+1 per `ori_list_alloc_data`). Key observations:

- **@count_items**: Correctly borrows the list parameter -- zero RC operations. The list struct is passed by pointer, and only the `len` field is read. This is excellent borrow elision.
- **@check_length**: List `b` is shared across two call sites (passed to `count_items` twice), requiring `rc_inc` before second use and separate `rc_dec` for each reference. The landingpad (bb6) correctly decrements `b`'s refcount on unwind.
- **@check_passing**: Uses `ori_buffer_drop_unique` instead of `rc_dec` -- the ARC pipeline correctly detected that the list is uniquely owned (never shared), avoiding the refcount check overhead.

### 3. Attributes & Calling Convention

| Function | fastcc | noundef | nounwind | readonly | uwtable | Notes |
|----------|--------|---------|----------|----------|---------|-------|
| @count_items | YES | YES | NO | NO | YES | [HIGH-1] [LOW-2] |
| @check_length | YES | YES | NO | N/A | YES | personality present (correct) |
| @check_iteration | YES | YES | NO | N/A | YES | [HIGH-1] |
| @check_passing | YES | YES | NO | N/A | YES | [HIGH-1] |
| @main | C (correct) | YES | NO | N/A | YES | [HIGH-1] |

**Attribute compliance**: 9/14 = 64.3%

Missing `nounwind` on 4 functions (count_items, check_iteration, check_passing, main) that cannot unwind. `check_length` correctly omits `nounwind` because it uses `invoke` for exception handling. Missing `readonly` on `count_items`'s parameter.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @count_items | 1 | 0 | 0 | 0 | Single block, clean |
| @check_length | 11 | 0 | 2 | 0 | [LOW-3] bb0->bb1, bb1->bb3 |
| @check_iteration | 6 | 1 | 1 | 1 | [LOW-3] add.ok->bb1 trampoline |
| @check_passing | 1 | 0 | 0 | 0 | Single block, clean |
| @main | 5 | 0 | 0 | 0 | Clean overflow check layout |

**Total defects**: 4 (2 redundant branches + 1 empty block + 1 trampoline)

**@check_length**: `bb0` unconditionally branches to `bb1`, and `bb1` unconditionally branches to `bb3`. These could be merged into a single block. The separation exists because the ARC pipeline inserts cleanup operations between evaluation steps.

**@check_iteration**: The `add.ok` block contains only `br label %bb1` -- a trampoline back to the loop header. This could be eliminated by branching directly from the overflow check to `%bb1`. The phi node in `bb1` correctly accumulates the running total.

### 5. Overflow Checking

**Status**: PASS

| Operation | Function | Checked | Correct | Notes |
|-----------|----------|---------|---------|-------|
| add (len+len) | @check_length | YES | YES | llvm.sadd.with.overflow.i64 |
| add (sum+count) | @check_length | YES | YES | llvm.sadd.with.overflow.i64 |
| sub (total-count) | @check_length | YES | YES | llvm.ssub.with.overflow.i64 |
| add (total+=x) | @check_iteration | YES | YES | llvm.sadd.with.overflow.i64 |
| add (a+b) | @main | YES | YES | llvm.sadd.with.overflow.i64 |
| add (sum+c) | @main | YES | YES | llvm.sadd.with.overflow.i64 |

All 6 arithmetic operations are correctly overflow-checked with appropriate panic paths.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.35 MiB (debug) |
| .text section | 898.9 KiB |
| .rodata section | 133.7 KiB |
| User code | 1246 bytes (5 functions) |
| Runtime | 99.9% of .text section |

#### Disassembly: @count_items (12 bytes)

```asm
_ori_count_items:
  mov    (%rdi),%rax       ; load len field
  mov    0x8(%rdi),%rcx    ; load cap (unused)
  mov    0x10(%rdi),%rcx   ; load data ptr (unused)
  ret
```

4 instructions, 12 bytes. Loads all 3 struct fields even though only `len` is needed. Fields 1 and 2 are dead loads that LLVM's optimizer would eliminate at higher optimization levels.

#### Disassembly: @main (117 bytes)

```asm
_ori_main:
  sub    $0x28,%rsp
  call   _ori_check_length
  mov    %rax,0x10(%rsp)
  call   _ori_check_iteration
  mov    %rax,0x8(%rsp)
  call   _ori_check_passing
  mov    0x8(%rsp),%rcx
  mov    %rax,%rdx
  mov    0x10(%rsp),%rax
  add    %rcx,%rax         ; a + b
  jo     .panic1
  mov    0x18(%rsp),%rcx
  add    %rcx,%rax         ; (a+b) + c
  jo     .panic2
  add    $0x28,%rsp
  ret
```

Clean structure: 3 calls, 2 checked additions, stack frame management.

### 7. Optimal IR Comparison

#### @count_items: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc i64 @_ori_count_items(ptr readonly %xs) nounwind {
  %len = load i64, ptr %xs, align 8
  ret i64 %len
}
```

```llvm
; ACTUAL (11 instructions)
define fastcc noundef i64 @_ori_count_items(ptr %0) #0 {
bb0:
  %param.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load.s2, 0
  ret i64 %list.len
}
```

**Delta**: +8 instructions. The codegen loads all 3 fields of the list struct and reconstructs it as an LLVM aggregate, then extracts field 0. The ideal code directly loads the first i64 from the pointer. However, +6 of these are justified as the standard parameter unpacking pattern (codegen always unpacks the full struct for uniformity). The remaining +2 (GEP for field 0 offset, which is always 0 and could be elided) are unjustified but trivially optimized away by LLVM.

#### @check_passing: Ideal vs Actual

```llvm
; IDEAL (20 instructions — same as actual)
; List allocation, element stores, call, drop_unique, ret
; No waste — this function is OPTIMAL.
```

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions — same as actual)
; 3 calls, 2 overflow-checked additions, ret
; No waste — this function is OPTIMAL.
```

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @count_items | 11 | 11 | +0 | N/A | OPTIMAL |
| @check_length | 90 | 92 | +2 | NO (redundant br) | NEAR-OPTIMAL |
| @check_iteration | 43 | 44 | +1 | NO (empty trampoline) | NEAR-OPTIMAL |
| @check_passing | 20 | 20 | +0 | N/A | OPTIMAL |
| @main | 16 | 16 | +0 | N/A | OPTIMAL |

### 8. Lists: ARC Lifecycle

The journey exercises three distinct ARC patterns for list values:

**Pattern 1: Borrow (@count_items)** -- The list parameter is passed by pointer reference. The callee reads the `len` field and returns. Zero RC operations. The ARC pipeline correctly infers borrow semantics: the callee does not store or extend the lifetime of the list.

**Pattern 2: Shared ownership (@check_length)** -- List `b` is used in two separate call sites. The ARC pipeline inserts `rc_inc` before the second use and `rc_dec` after each use completes. The landingpad (bb6) handles cleanup if `count_items(c)` unwinds, correctly decrementing `b`'s refcount. Note: `count_items` cannot actually unwind (it only reads a field), so the `invoke` + landing pad is overly defensive [HIGH-1].

**Pattern 3: Unique ownership (@check_passing)** -- The list is created, immediately passed to `count_items`, then dropped. The ARC pipeline detects unique ownership and uses `ori_buffer_drop_unique` instead of `ori_buffer_rc_dec`, skipping the refcount check entirely. This is excellent static uniqueness analysis.

### 9. Lists: Iteration

The `@check_iteration` function exercises the for-loop-over-list compilation path:

1. **List creation**: Standard `ori_list_alloc_data` + element stores
2. **Iterator creation**: `ori_list_rc_inc` (share list with iterator) + `ori_iter_from_list` (create runtime iterator state)
3. **Loop body**: `ori_iter_next` returns (has_value: i8, element: i64). The phi node `%v12` accumulates the running total across iterations. Overflow-checked addition on each iteration.
4. **Cleanup**: After the loop terminates (iter_next returns 0), `ori_buffer_rc_dec` drops the list reference, and `ori_iter_drop` frees the iterator state.

The iterator protocol is runtime-backed rather than inlined. Each `ori_iter_next` call involves a function call to the runtime, which is appropriate for debug builds but would benefit from inlining at higher optimization levels. The phi node usage for the loop accumulator is correct and efficient.

The `insertvalue`/`extractvalue` dance for the `(tag, element)` pair returned from `ori_iter_next` adds overhead that could be avoided by using separate return values, but this is consistent with the Option<T> representation used throughout the codegen. [LOW-4]

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | HIGH | Attributes | Unnecessary invoke/landing pad for non-unwinding count_items | NEW | J10 |
| 2 | LOW | Attributes | Missing readonly on count_items parameter | NEW | J10 |
| 3 | LOW | Control Flow | Redundant unconditional branches and empty trampoline blocks | CONFIRMED | J1 |
| 4 | NOTE | ARC | Excellent borrow elision on count_items parameter | NEW | J10 |
| 5 | NOTE | ARC | Static uniqueness detection in check_passing (drop_unique) | NEW | J10 |

### HIGH-1: Unnecessary invoke/landing pad for non-unwinding count_items

**Location**: @check_length, `add.ok` block -- `invoke fastcc i64 @_ori_count_items` + bb6 landingpad
**Impact**: Generates exception handling infrastructure (landingpad, cleanup block, personality function) for a callee that only reads a struct field and returns an integer. This adds ~15 native instructions for the landing pad path and prevents LLVM from performing certain optimizations across the call.
**Fix**: The nounwind analysis should detect that `count_items` cannot unwind (no calls to unwinding functions) and mark it `nounwind`. The caller would then use `call` instead of `invoke`, eliminating the landing pad entirely.
**First seen**: Journey 10
**Found in**: Attributes & Calling Convention (Category 3), Lists: ARC Lifecycle (Category 8)

### LOW-2: Missing readonly on count_items parameter

**Location**: @count_items function declaration
**Impact**: Without `readonly`, LLVM cannot optimize memory accesses around calls to this function. The parameter is genuinely read-only (only the `len` field is loaded).
**Fix**: Add `readonly` attribute to the parameter or mark the function `memory(argmem: read)`.
**First seen**: Journey 10
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-3: Redundant unconditional branches and empty trampoline blocks

**Location**: @check_length (bb0->bb1, bb1->bb3), @check_iteration (add.ok->bb1)
**Impact**: 3 unnecessary branch instructions. The blocks could be merged. These are artifacts of the ARC pipeline inserting operations at block boundaries.
**First seen**: Journey 1 (same pattern)
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Excellent borrow elision on count_items parameter

**Location**: @count_items
**Impact**: Positive -- the list parameter is passed by-reference with zero RC operations. The ARC pipeline correctly infers that `count_items` borrows the list without extending its lifetime.
**Found in**: ARC Purity (Category 2)

### NOTE-5: Static uniqueness detection (drop_unique)

**Location**: @check_passing, `ori_buffer_drop_unique` call
**Impact**: Positive -- instead of the general `ori_buffer_rc_dec` (which checks refcount at runtime), the ARC pipeline statically determines the list is uniquely owned and uses `drop_unique`, which unconditionally frees the buffer. This is a performance win.
**Found in**: ARC Purity (Category 2)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.02x avg ratio |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 5/10 | 64.3% compliance |
| Control Flow | 10% | 7/10 | 4 defects |
| IR Quality | 20% | 8/10 | 3 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 7/10 | 1 high, 1 low |

**Overall: 8.2 / 10**

## Verdict

Journey 10's list codegen demonstrates strong ARC fundamentals with correct borrow elision, shared ownership tracking, and static uniqueness optimization (drop_unique). The main weakness is the unnecessary invoke/landing pad infrastructure around calls to non-unwinding functions like `count_items`, caused by the nounwind analysis failing to propagate through simple read-only functions. Instruction efficiency is near-optimal at 1.02x, with only 3 unjustified instructions across 5 functions. The for-loop iteration path is correctly compiled with a phi-based accumulator and proper iterator lifecycle management.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J10 | CONFIRMED |
| fastcc usage | J1 | J10 | CONFIRMED |
| Missing nounwind | J1 | J10 | CONFIRMED |
| Redundant branches | J1 | J10 | CONFIRMED |
| For-loop compilation | J7 | J10 | CONFIRMED (list iter vs range iter) |

The missing `nounwind` pattern persists across all journeys. Journey 10 introduces the first significant consequence: without `nounwind` on `count_items`, the caller must use `invoke` instead of `call`, generating unnecessary exception handling infrastructure. In previous journeys (J1-J9), the missing `nounwind` only caused minor LLVM optimization misses; here it directly inflates the codegen.
