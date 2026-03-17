---
journey: 15
slug: fat-nested-collections
theme: "I am nested fat"
date: 2026-03-16
status: PASS
expected: 18
eval_result: 18
aot_result: 18

difficulty: complex
prerequisites:
  - "Understanding of list creation and heap allocation"
  - "Familiarity with string fat pointers (len, cap, data)"
  - "ARC lifecycle for nested collections (list of strings)"
  - "Iterator protocol and element-level RC cleanup"
learning_objectives:
  - "See how list-of-strings creates nested ARC — the list buffer holds string fat pointers, each string has its own RC"
  - "Understand element-level cleanup via _ori_elem_dec callback when the list buffer is dropped"
  - "Observe the iterator protocol over [str] — fat pointer values copied out of the list buffer"
  - "Identify double-free bugs in nested ARC cleanup when both iterator and list destructor touch the same elements"

features:
  - lists
  - strings
  - arc
  - function_calls
  - loops
feature_description: "Nested fat pointer collections: list of strings with element-level ARC, for-loop iteration, and multi-use list passing"

score: 6.2
score_breakdown:
  instruction_efficiency: 7
  arc_correctness: 3
  attributes_safety: 7
  control_flow: 10
  ir_quality: 7
  binary_quality: 9
  other_findings: 4
score_metrics:
  instruction_ratio: 1.30
  instruction_ratio_max: 3.67
  arc_violations: 2
  arc_has_unbalanced: true
  arc_has_scalar_rc: false
  attr_applicable: 21
  attr_correct: 18
  attr_has_wrong: false
  cf_defects: 0
  cf_incorrect: false
  ir_unjustified: 8
  ir_incorrect: false
  bin_defects: 1
  bin_hard_fail: false
  other_critical: 1
  other_high: 1
  other_low: 1
overflow_check: PASS

bugs_found:
  - id: C15-1
    severity: CRITICAL
    description: "Double-free on string elements: iterator consumption + list buffer drop both decrement string RCs"
    status: OPEN
    found_in: journey15
  - id: C15-2
    severity: CRITICAL
    description: "Double ori_buffer_rc_dec in unwind path (bb2) of @main — two decrements on same list buffer"
    status: OPEN
    found_in: journey15

related_journeys:
  - journey: 9
    relationship: "Both test string ARC with SSO guards; J15 nests strings inside a list"
  - journey: 10
    relationship: "Both allocate lists and iterate with for-loop; J10 uses [int], J15 uses [str] (fat pointer elements)"
  - journey: 13
    relationship: "Both exercise iterator protocol with list-backed iterators; J15 adds element-level destructor complexity"
---

# Journey 15: "I am nested fat"

## Source

```ori
// Journey 15: "I am nested fat"
// Slug: fat-nested-collections
// Difficulty: complex
// Features: lists, strings, arc, function_calls, loops
// Expected: count_chars(words) + total_items(words) = 15 + 3 = 18

@count_chars (words: [str]) -> int = {
    let total = 0;
    for w in words do total += w.length();
    total
}

@total_items (xs: [str]) -> int = xs.length();

@main () -> int = {
    let words = ["hello", "world", "12345"];
    let a = count_chars(words: words);
    let b = total_items(xs: words);
    a + b
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 18        | 18       | (none) | (none) | PASS   |
| AOT     | 18        | 18       | (none) | double-free FATAL | PASS (result correct, RC bug on cleanup) |

The AOT backend produces the correct exit code (18) but emits a FATAL error on stderr: `ori_rc_dec called on already-freed allocation`. This is a double-free bug in the nested ARC cleanup — the string elements inside the list are freed twice (once by the iterator, once by the list buffer destructor).

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens — the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 115 | **Keywords**: 8 | **Identifiers**: 18 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(count_chars) LParen Ident(words) Colon LBracket
Ident(str) RBracket RParen Arrow Ident(int) Eq LBrace
Let Ident(total) Eq Int(0) Semi
For Ident(w) In Ident(words) Do Ident(total) PlusEq
Ident(w) Dot Ident(length) LParen RParen Semi
Ident(total) RBrace
Fn(@) Ident(total_items) LParen Ident(xs) Colon LBracket
Ident(str) RBracket RParen Arrow Ident(int) Eq Ident(xs)
Dot Ident(length) LParen RParen Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(words) Eq LBracket Str("hello") Comma Str("world")
Comma Str("12345") RBracket Semi
Let Ident(a) Eq Ident(count_chars) LParen Ident(words)
Colon Ident(words) RParen Semi
Let Ident(b) Eq Ident(total_items) LParen Ident(xs)
Colon Ident(words) RParen Semi
Ident(a) Plus Ident(b) RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) — a tree structure that represents the grammatical structure of the program.

**Nodes**: 27 | **Max depth**: 4 | **Functions**: 3 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @count_chars
│  ├─ Params: (words: [str])
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let total = 0
│       ├─ For w in words do total += w.length()
│       └─ Ident(total)
├─ FnDecl @total_items
│  ├─ Params: (xs: [str])
│  ├─ Return: int
│  └─ Body: MethodCall(xs, length, [])
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let words = ["hello", "world", "12345"]
        ├─ Let a = count_chars(words: words)
        ├─ Let b = total_items(xs: words)
        └─ BinOp(+, a, b)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 14 | **Types inferred**: 8 | **Unifications**: 10 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@count_chars (words: [str]) -> int = {
    let total: int = 0;
    for w: str in words do total += w.length();
    //                                ^ int (from str.length() -> int)
    total  // -> int
}

@total_items (xs: [str]) -> int = xs.length();
//                                 ^ int (from [str].length() -> int)

@main () -> int = {
    let words: [str] = ["hello", "world", "12345"];
    let a: int = count_chars(words: words);
    let b: int = total_items(xs: words);
    a + b  // -> int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 5 | **Desugared**: 2 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- total += w.length() desugared to total = total + w.length()
- For-loop lowered to iterator protocol (iter/next/pattern-match)
- List literal lowered to element construction sequence
- Function call arguments normalized to positional order
- xs.length() lowered to method dispatch on [str]
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead — parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 8 | **Elided**: 0 | **Net ops**: 8

<details>
<summary>ARC annotations</summary>

```text
@count_chars: +0 rc_inc, +1 rc_dec (iterator cleanup via ori_iter_drop)
  - words: borrowed (passed by-ref, not consumed)
  - Iterator created from list data; iter_drop handles iterator state cleanup
  - Element destructor (_ori_elem_dec$3) registered for string element RC
@total_items: +0 rc_inc, +0 rc_dec (pure read, readonly attribute applied)
  - xs: borrowed (passed by-ref, only length read)
@main: +1 rc_inc (ori_list_rc_inc), +4 rc_dec (ori_buffer_rc_dec)
  - List allocated, rc_inc for multi-use, rc_dec after each use + unwind paths
  - String elements: 3x ori_str_from_raw (each creates RC=1 string)
  - BUG: bb2 unwind path has 2x ori_buffer_rc_dec on same list (double-free)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 18 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  ├─ let words = ["hello", "world", "12345"]
  ├─ let a = @count_chars(words: ["hello", "world", "12345"])
  │    ├─ let total = 0
  │    ├─ for w in words:
  │    │    ├─ w = "hello", total = 0 + 5 = 5
  │    │    ├─ w = "world", total = 5 + 5 = 10
  │    │    └─ w = "12345", total = 10 + 5 = 15
  │    └─ total = 15
  ├─ let b = @total_items(xs: ["hello", "world", "12345"])
  │    └─ xs.length() = 3
  └─ a + b = 15 + 3 = 18
→ 18
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 8 | **Elided**: 0 | **Net ops**: 8

<details>
<summary>ARC annotations</summary>

```text
@count_chars: +0 rc_inc, +1 rc_dec (ori_iter_drop cleans up iterator state)
  - List parameter borrowed by-ref (ptr %0)
  - ori_iter_from_list creates iterator with _ori_elem_dec$3 callback
  - ori_iter_drop called at loop exit — drops iterator, potentially RC-decs remaining elements
@total_items: +0 rc_inc, +0 rc_dec (nounwind, readonly — pure read)
@main: +1 rc_inc (ori_list_rc_inc), +4 rc_dec (ori_buffer_rc_dec across paths)
  - Normal path: 1x rc_inc + 2x rc_dec (after count_chars + after add) = balanced for list
  - Unwind bb2: 2x rc_dec on same list = DOUBLE-FREE
  - Unwind bb4: 1x rc_dec = correct for that path
  - BUG: String elements double-freed at runtime (iterator + list destructor both clean up)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '15-fat-nested-collections'
source_filename = "15-fat-nested-collections"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [6 x i8] c"world\00", align 1
@str.2 = private unnamed_addr constant [6 x i8] c"12345\00", align 1

; Function Attrs: uwtable
; --- @count_chars ---
define fastcc noundef i64 @_ori_count_chars(ptr noundef nonnull dereferenceable(24) %0) #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %iter_next.scratch = alloca { i64, i64, ptr }, align 8
  %param.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  %list.data = extractvalue { i64, i64, ptr } %param.load.s2, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load.s2, 0
  %list.cap = extractvalue { i64, i64, ptr } %param.load.s2, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data, i64 %list.len, i64 %list.cap, i64 24, ptr @"_ori_elem_dec$3")
  br label %bb1

bb1:                                              ; preds = %bb2, %bb0
  %v51 = phi i64 [ 0, %bb0 ], [ %add.val, %bb2 ]
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 24)
  %iter_next.tag = zext i8 %iter_next.has to i64
  %iter_next.elem.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %iter_next.scratch, i32 0, i32 0
  %iter_next.elem.f0 = load i64, ptr %iter_next.elem.f0.ptr, align 8
  %iter_next.elem.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %iter_next.elem.f0, 0
  %iter_next.elem.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %iter_next.scratch, i32 0, i32 1
  %iter_next.elem.f1 = load i64, ptr %iter_next.elem.f1.ptr, align 8
  %iter_next.elem.s1 = insertvalue { i64, i64, ptr } %iter_next.elem.s0, i64 %iter_next.elem.f1, 1
  %iter_next.elem.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %iter_next.scratch, i32 0, i32 2
  %iter_next.elem.f2 = load ptr, ptr %iter_next.elem.f2.ptr, align 8
  %iter_next.elem.s2 = insertvalue { i64, i64, ptr } %iter_next.elem.s1, ptr %iter_next.elem.f2, 2
  %iter_next.0 = insertvalue { i64, { i64, i64, ptr } } undef, i64 %iter_next.tag, 0
  %iter_next.1 = insertvalue { i64, { i64, i64, ptr } } %iter_next.0, { i64, i64, ptr } %iter_next.elem.s2, 1
  %proj.0 = extractvalue { i64, { i64, i64, ptr } } %iter_next.1, 0
  %ne = icmp ne i64 %proj.0, 0
  br i1 %ne, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  %proj.1 = extractvalue { i64, { i64, i64, ptr } } %iter_next.1, 1
  store { i64, i64, ptr } %proj.1, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v51, i64 %str.len)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %bb1

bb3:                                              ; preds = %bb1
  call void @ori_iter_drop(ptr %list.iter)
  ret i64 %v51

add.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @total_items ---
define fastcc noundef i64 @_ori_total_items(ptr noundef nonnull readonly dereferenceable(24) %0) #1 {
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
; --- @main ---
define noundef i64 @_ori_main() #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg29 = alloca { i64, i64, ptr }, align 8
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %str.val.sret11 = alloca { i64, i64, ptr }, align 8
  %str.val.sret1 = alloca { i64, i64, ptr }, align 8
  %str.val.sret = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %str.val.sret, ptr @str, i64 5)
  %str.val.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 0
  %str.val.f0 = load i64, ptr %str.val.f0.ptr, align 8
  %str.val.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f0, 0
  %str.val.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 1
  %str.val.f1 = load i64, ptr %str.val.f1.ptr, align 8
  %str.val.s1 = insertvalue { i64, i64, ptr } %str.val.s0, i64 %str.val.f1, 1
  %str.val.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 2
  %str.val.f2 = load ptr, ptr %str.val.f2.ptr, align 8
  %str.val.s2 = insertvalue { i64, i64, ptr } %str.val.s1, ptr %str.val.f2, 2
  call void @ori_str_from_raw(ptr %str.val.sret1, ptr @str.1, i64 5)
  %str.val.f0.ptr2 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 0
  %str.val.f03 = load i64, ptr %str.val.f0.ptr2, align 8
  %str.val.s04 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f03, 0
  %str.val.f1.ptr5 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 1
  %str.val.f16 = load i64, ptr %str.val.f1.ptr5, align 8
  %str.val.s17 = insertvalue { i64, i64, ptr } %str.val.s04, i64 %str.val.f16, 1
  %str.val.f2.ptr8 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 2
  %str.val.f29 = load ptr, ptr %str.val.f2.ptr8, align 8
  %str.val.s210 = insertvalue { i64, i64, ptr } %str.val.s17, ptr %str.val.f29, 2
  call void @ori_str_from_raw(ptr %str.val.sret11, ptr @str.2, i64 5)
  %str.val.f0.ptr12 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret11, i32 0, i32 0
  %str.val.f013 = load i64, ptr %str.val.f0.ptr12, align 8
  %str.val.s014 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f013, 0
  %str.val.f1.ptr15 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret11, i32 0, i32 1
  %str.val.f116 = load i64, ptr %str.val.f1.ptr15, align 8
  %str.val.s117 = insertvalue { i64, i64, ptr } %str.val.s014, i64 %str.val.f116, 1
  %str.val.f2.ptr18 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret11, i32 0, i32 2
  %str.val.f219 = load ptr, ptr %str.val.f2.ptr18, align 8
  %str.val.s220 = insertvalue { i64, i64, ptr } %str.val.s117, ptr %str.val.f219, 2
  %list.data = call ptr @ori_list_alloc_data(i64 3, i64 24)
  %list.elem_ptr = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 0
  store { i64, i64, ptr } %str.val.s2, ptr %list.elem_ptr, align 8
  %list.elem_ptr21 = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 1
  store { i64, i64, ptr } %str.val.s210, ptr %list.elem_ptr21, align 8
  %list.elem_ptr22 = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 2
  store { i64, i64, ptr } %str.val.s220, ptr %list.elem_ptr22, align 8
  %list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2
  %rc_inc.data = extractvalue { i64, i64, ptr } %list.2, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  store { i64, i64, ptr } %list.2, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_count_chars(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  %rc.data_ptr26 = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len27 = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap28 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr26, i64 %rc.len27, i64 %rc.cap28, i64 24, ptr @"_ori_elem_dec$3")
  store { i64, i64, ptr } %list.2, ptr %ref_arg29, align 8
  %call30 = invoke fastcc i64 @_ori_total_items(ptr %ref_arg29)
          to label %bb3 unwind label %bb4

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  %rc.data_ptr = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr, i64 %rc.len, i64 %rc.cap, i64 24, ptr @"_ori_elem_dec$3")
  %rc.data_ptr23 = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len24 = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap25 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr23, i64 %rc.len24, i64 %rc.cap25, i64 24, ptr @"_ori_elem_dec$3")
  resume { ptr, i32 } %lp

bb3:                                              ; preds = %bb1
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call30)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb4:                                              ; preds = %bb1
  %lp31 = landingpad { ptr, i32 }
          cleanup
  %rc.data_ptr32 = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len33 = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap34 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr32, i64 %rc.len33, i64 %rc.cap34, i64 24, ptr @"_ori_elem_dec$3")
  resume { ptr, i32 } %lp31

add.ok:                                           ; preds = %bb3
  %rc.data_ptr35 = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len36 = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap37 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr35, i64 %rc.len36, i64 %rc.cap37, i64 24, ptr @"_ori_elem_dec$3")
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %bb3
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: cold nounwind
; --- elem_dec.@3 ---
define void @"_ori_elem_dec$3"(ptr %0) #3 {
entry:
  %elem.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %elem.f0 = load i64, ptr %elem.f0.ptr, align 8
  %elem.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %elem.f0, 0
  %elem.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %elem.f1 = load i64, ptr %elem.f1.ptr, align 8
  %elem.s1 = insertvalue { i64, i64, ptr } %elem.s0, i64 %elem.f1, 1
  %elem.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %elem.f2 = load ptr, ptr %elem.f2.ptr, align 8
  %elem.s2 = insertvalue { i64, i64, ptr } %elem.s1, ptr %elem.f2, 2
  %rc_dec.data = extractvalue { i64, i64, ptr } %elem.s2, 2
  %rc_dec.str.p2i = ptrtoint ptr %rc_dec.data to i64
  %rc_dec.str.sso_flag = and i64 %rc_dec.str.p2i, -9223372036854775808
  %rc_dec.str.is_sso = icmp ne i64 %rc_dec.str.sso_flag, 0
  %rc_dec.str.null.p2i = ptrtoint ptr %rc_dec.data to i64
  %rc_dec.str.null = icmp eq i64 %rc_dec.str.null.p2i, 0
  %rc_dec.str.skip_rc = or i1 %rc_dec.str.is_sso, %rc_dec.str.null
  br i1 %rc_dec.str.skip_rc, label %rc_dec.str_skip, label %rc_dec.str_heap

rc_dec.str_heap:                                  ; preds = %entry
  call void @ori_rc_dec(ptr %rc_dec.data, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.str_skip

rc_dec.str_skip:                                  ; preds = %rc_dec.str_heap, %entry
  ret void
}

; Function Attrs: cold nounwind uwtable
; --- drop str ---
define void @"_ori_drop$3"(ptr noundef %0) #4 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}

; Runtime declarations (abbreviated)
declare ptr @ori_iter_from_list(ptr, i64, i64, i64, ptr) #2
declare i8 @ori_iter_next(ptr, ptr, i64) #2
declare void @ori_iter_drop(ptr) #2
declare i64 @ori_str_len(ptr) #2
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #6
declare void @ori_panic_cstr(ptr) #7
declare void @ori_str_from_raw(ptr noalias sret({ i64, i64, ptr }), ptr, i64) #2
declare ptr @ori_list_alloc_data(i64, i64) #2
declare void @ori_list_rc_inc(ptr, i64) #5
declare void @ori_buffer_rc_dec(ptr, i64, i64, i64, ptr) #5
declare void @ori_rc_dec(ptr, ptr) #5
declare void @ori_rc_free(ptr, i64, i64) #2
declare i32 @ori_check_leaks() #2

; Entry point wrapper
define noundef i32 @main() #0 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}

attributes #0 = { uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nounwind }
attributes #3 = { cold nounwind }
attributes #4 = { cold nounwind uwtable }
attributes #5 = { nounwind memory(inaccessiblemem: readwrite) }
attributes #6 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #7 = { cold noreturn }
```

#### Disassembly

```asm
_ori_count_chars:
   sub    $0x68,%rsp
   mov    (%rdi),%rsi               ; load list.len
   mov    0x8(%rdi),%rdx            ; load list.cap
   mov    0x10(%rdi),%rdi           ; load list.data
   mov    $0x18,%ecx                ; elem_size = 24
   lea    _ori_elem_dec$3(%rip),%r8 ; element destructor
   call   ori_iter_from_list        ; create list iterator
   mov    %rax,0x28(%rsp)           ; save iter ptr
   xor    %eax,%eax                 ; total = 0
.loop:
   mov    0x28(%rsp),%rdi           ; iter ptr
   lea    0x38(%rsp),%rsi           ; scratch buffer for next elem
   mov    $0x18,%edx                ; elem_size
   call   ori_iter_next             ; get next element
   movzbl %al,%eax                  ; has_next flag
   ; ... load string fat pointer from scratch ...
   cmp    $0x0,%rax
   je     .done                     ; if no more elements, exit
   ; ... store str fat pointer, call ori_str_len ...
   add    %rcx,%rax                 ; total += str.len
   seto   %cl                       ; overflow check
   jne    .overflow_panic
   jmp    .loop
.done:
   mov    0x28(%rsp),%rdi
   call   ori_iter_drop             ; cleanup iterator
   mov    0x8(%rsp),%rax            ; return total
   add    $0x68,%rsp
   ret
.overflow_panic:
   lea    ovf.msg(%rip),%rdi
   call   ori_panic_cstr
```

```asm
_ori_total_items:
   mov    (%rdi),%rax               ; load list.len (field 0)
   mov    0x8(%rdi),%rcx            ; DEAD: load list.cap (unused)
   mov    0x10(%rdi),%rcx           ; DEAD: load list.data (unused)
   ret
```

```asm
_ori_main:
   push   %r14
   push   %rbx
   sub    $0x108,%rsp
   ; --- create 3 strings via ori_str_from_raw ---
   lea    "hello"(%rip),%rsi
   lea    0x90(%rsp),%rdi
   mov    $0x5,%edx
   call   ori_str_from_raw          ; str[0] = "hello"
   ; ... load str fat pointer fields ...
   lea    "world"(%rip),%rsi
   call   ori_str_from_raw          ; str[1] = "world"
   ; ... load str fat pointer fields ...
   lea    "12345"(%rip),%rsi
   call   ori_str_from_raw          ; str[2] = "12345"
   ; ... load str fat pointer fields ...
   ; --- allocate list buffer ---
   mov    $0x3,%edi
   mov    $0x18,%esi
   call   ori_list_alloc_data       ; alloc 3x24 byte buffer
   ; ... store 3 string fat pointers into list buffer ...
   ; --- rc_inc for multi-use ---
   call   ori_list_rc_inc           ; RC 1->2
   ; --- call count_chars ---
   lea    0xd8(%rsp),%rdi
   call   _ori_count_chars          ; a = count_chars(words)
   ; --- rc_dec after count_chars ---
   call   ori_buffer_rc_dec         ; RC 2->1
   ; --- call total_items ---
   lea    0xf0(%rsp),%rdi
   call   _ori_total_items          ; b = total_items(words)
   ; --- a + b with overflow check ---
   add    %rcx,%rax
   seto   %al
   jo     .overflow_panic
   ; --- rc_dec final cleanup ---
   call   ori_buffer_rc_dec         ; RC 1->0 (drop + element cleanup)
   ; ... return result ...
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @count_chars | 44 | 24 | 1.83x | ACCEPTABLE |
| 2 | @total_items | 11 | 3 | 3.67x | BLOATED |
| 3 | @main | 85 | 55 | 1.55x | ACCEPTABLE |
| 4 | @_ori_elem_dec$3 | 20 | 10 | 2.00x | ACCEPTABLE |

**@count_chars** (44 actual vs 24 ideal): The iterator element loading reconstructs the full `{i64, i64, ptr}` struct via 3x (GEP+load+insertvalue) = 9 instructions, then wraps into `{i64, {i64, i64, ptr}}` option struct with 3 more insertvalue+extractvalue. This is the standard pattern for the iterator protocol but could be streamlined to direct memory access. The overflow checking adds 4 justified instructions. The phi node, loop branch, and iterator API calls are all necessary.

**@total_items** (11 actual vs 3 ideal): Loads all 3 list fields (len, cap, data) via 3x (GEP+load+insertvalue) but only uses field 0 (length). Fields 1 and 2 are dead loads. Ideal: 1 GEP + 1 load + 1 ret = 3 instructions. [MEDIUM-3]

**@main** (85 actual vs 55 ideal): The string creation pattern (3x ori_str_from_raw + 3x field reload into SSA) adds ~27 instructions. The list construction and RC management is reasonable. Landing pads add necessary overhead for exception safety. The double `ori_buffer_rc_dec` in bb2 is a bug, not just overhead. [CRITICAL-1, CRITICAL-2]

**@_ori_elem_dec$3** (20 actual vs 10 ideal): Loads all 3 fields of the string fat pointer but only needs the data pointer (field 2) for the SSO check and RC decrement. Fields 0 and 1 are dead loads, adding 6 unnecessary instructions.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @count_chars | 0 | 0+iter | YES (local) | 1 (param borrowed) | 0 moves |
| @total_items | 0 | 0 | YES | 1 (param borrowed, readonly) | 0 moves |
| @main | 1 | 2 (normal) / 2 (bb2) / 1 (bb4) | NO (bb2 double-dec) | 0 | 0 |
| @_ori_elem_dec$3 | 0 | 1 | YES (callback) | 0 | 0 |

**Verdict**: CRITICAL ARC violation. The `@main` function has a double-free bug manifesting in two ways:

1. **Runtime double-free (CRITICAL-1)**: The AOT execution reports `ori_rc_dec called on already-freed allocation`. The string elements inside the list buffer are freed both by the iterator cleanup (when `ori_iter_drop` is called after the for-loop in `count_chars`) and by the list buffer destructor (when `ori_buffer_rc_dec` drops the buffer to RC=0 and calls `_ori_elem_dec$3` on each element). This double-frees each string element.

2. **Unwind path double-dec (CRITICAL-2)**: In `@main` bb2 (the landingpad for count_chars unwind), two `ori_buffer_rc_dec` calls are emitted on the same list value. With RC starting at 2 after `ori_list_rc_inc`, the first dec takes it to 1 and the second to 0, triggering a drop+element-cleanup. This is a double-free waiting to happen on the unwind path. The correct behavior would be a single `ori_buffer_rc_dec` (one for the rc_inc copy, one for the original).

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @count_chars | YES | NO | N/A | N/A | NO | Correct: may panic on overflow |
| @total_items | YES | YES | N/A | YES | NO | Excellent: readonly correctly applied |
| @main | C (correct) | NO | N/A | N/A | NO | Correct: uses invoke |
| @_ori_elem_dec$3 | C | YES | N/A | N/A | YES | Correct: cold callback |
| @_ori_drop$3 | C | YES | N/A | N/A | YES | Correct: cold destructor |
| @main (entry) | C | NO | N/A | N/A | NO | [LOW-4] missing nounwind |

**Attribute compliance**: 18/21 = 85.7%. The 3 missing attributes are on the entry `main` wrapper and related to minor optimization hints. No wrong attributes applied.

The `readonly` on `@total_items` is a notable positive finding — the compiler correctly identifies that the function only reads the list parameter. [NOTE-5]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @count_chars | 5 | 0 | 0 | 1 (loop) | Clean loop structure |
| @total_items | 1 | 0 | 0 | 0 | Single block, optimal |
| @main | 7 | 0 | 0 | 0 | Landing pads well-structured |
| @_ori_elem_dec$3 | 3 | 0 | 0 | 0 | Clean SSO branch |

**Verdict**: Control flow is clean. The `@count_chars` loop uses a proper phi node for the accumulator. The `@main` landing pad structure is correct in shape (bb2 for count_chars unwind, bb4 for total_items unwind), even though bb2 has the double-dec bug. No empty blocks or redundant branches.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| total += w.length() | YES | YES | llvm.sadd.with.overflow in @count_chars bb2 |
| a + b | YES | YES | llvm.sadd.with.overflow in @main bb3 |

Both integer additions use `llvm.sadd.with.overflow.i64` with proper panic on overflow.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.39 MiB (debug) |
| .text section | 930 KiB |
| .rodata section | 134 KiB |
| User code | ~680 bytes (count_chars: 214, total_items: 12, main: ~450) |
| Runtime | >99% of binary |

#### Disassembly: @total_items

```asm
_ori_total_items:
   mov    (%rdi),%rax       ; load list.len -> return value
   mov    0x8(%rdi),%rcx    ; DEAD: load list.cap (unused)
   mov    0x10(%rdi),%rcx   ; DEAD: load list.data (unused)
   ret
```

The dead loads of cap and data fields in `@total_items` are a minor binary defect — 2 unnecessary `mov` instructions. [MEDIUM-3]

#### Disassembly: @_ori_elem_dec$3

```asm
_ori_elem_dec$3:
   push   %rax
   mov    (%rdi),%rax       ; DEAD: load str.len
   mov    0x8(%rdi),%rax    ; DEAD: load str.cap
   mov    0x10(%rdi),%rcx   ; load str.data (needed for SSO check)
   ; ... SSO flag check, null check, conditional ori_rc_dec ...
   ret
```

The elem_dec callback correctly implements the SSO guard (skip RC for small strings and null pointers). The two dead loads of len and cap are redundant but harmless.

### 7. Optimal IR Comparison

#### @total_items: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc noundef i64 @_ori_total_items(ptr noundef nonnull readonly dereferenceable(24) %0) nounwind {
  %len = load i64, ptr %0, align 8
  ret i64 %len
}
```

```llvm
; ACTUAL (11 instructions)
define fastcc noundef i64 @_ori_total_items(ptr noundef nonnull readonly dereferenceable(24) %0) #1 {
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

**Delta**: +8 instructions. The codegen loads all 3 struct fields into an SSA aggregate and then extracts field 0. The ideal approach loads only field 0 directly via the pointer. The overhead is unjustified — only `len` is used, so `cap` and `data` loads are dead code.

#### @count_chars: Ideal vs Actual

```llvm
; IDEAL (24 instructions — iterator protocol requires runtime calls)
define fastcc noundef i64 @_ori_count_chars(ptr noundef nonnull dereferenceable(24) %0) {
bb0:
  %scratch = alloca { i64, i64, ptr }, align 8
  %data = load ptr, ptr (gep %0, 0, 2), align 8
  %len = load i64, ptr %0, align 8
  %cap = load i64, ptr (gep %0, 0, 1), align 8
  %iter = call ptr @ori_iter_from_list(ptr %data, i64 %len, i64 %cap, i64 24, ptr @elem_dec)
  br label %loop
loop:
  %total = phi i64 [0, %bb0], [%new_total, %body]
  %has = call i8 @ori_iter_next(ptr %iter, ptr %scratch, i64 24)
  %cond = icmp ne i8 %has, 0
  br i1 %cond, label %body, label %exit
body:
  %slen = call i64 @ori_str_len(ptr %scratch)
  %r = call {i64,i1} @llvm.sadd.with.overflow.i64(i64 %total, i64 %slen)
  %new_total = extractvalue {i64,i1} %r, 0
  %ovf = extractvalue {i64,i1} %r, 1
  br i1 %ovf, label %panic, label %loop
exit:
  call void @ori_iter_drop(ptr %iter)
  ret i64 %total
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +20 instructions. The overhead comes from: (a) full struct reconstruction of the parameter via 3x GEP+load+insertvalue instead of direct field loads (+6), (b) iterator element reconstruction into `{i64, {i64, i64, ptr}}` option wrapper with insertvalue+extractvalue chain instead of using the scratch buffer directly (+8), (c) second alloca for `str_len.self` and store+load round-trip (+6).

#### @main: Ideal vs Actual

Ideal @main would have ~55 instructions: 3x ori_str_from_raw (3 calls + 3 direct stores = 6), list alloc + 3 stores (4), rc_inc (3), count_chars call (4), rc_dec (5), total_items call (4), rc_dec (5), add with overflow (6), plus landing pads (~20). Actual is 85 instructions (+30 overhead from SSA field reconstruction of string fat pointers).

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @count_chars | 24 | 44 | +20 | PARTIAL (struct reconstruction overhead) | ACCEPTABLE |
| @total_items | 3 | 11 | +8 | NO (dead field loads) | BLOATED |
| @main | 55 | 85 | +30 | PARTIAL (SSA reconstruction pattern) | ACCEPTABLE |
| @_ori_elem_dec$3 | 10 | 20 | +10 | PARTIAL (dead len/cap loads, SSO justified) | ACCEPTABLE |

### 8. Nested ARC: List-of-Strings Cleanup

The core test of this journey: when a `[str]` list is dropped, the list buffer destructor calls `_ori_elem_dec$3` on each string element, which checks the SSO flag and conditionally calls `ori_rc_dec` on the string's data pointer. This creates a two-level ARC cleanup chain:

1. **List buffer**: RC managed by `ori_list_rc_inc` / `ori_buffer_rc_dec`
2. **String elements**: RC managed by `ori_rc_dec` via the `_ori_elem_dec$3` callback

The `_ori_elem_dec$3` callback correctly implements the SSO guard pattern:
- Check if the data pointer has the SSO flag set (bit 63) — if so, skip RC (inline string)
- Check if the data pointer is null — if so, skip RC
- Otherwise, call `ori_rc_dec` on the heap-allocated string data

**BUG**: The interaction between the iterator (which also receives `_ori_elem_dec$3`) and the list buffer destructor creates a double-free. When `count_chars` iterates over all elements and then the iterator is dropped, the iterator runtime may decrement string element RCs. When `@main` later decrements the list buffer to RC=0, the buffer destructor calls `_ori_elem_dec$3` again on the same string elements — but they have already been freed. [CRITICAL-1]

### 9. Fat Pointers: Element Iteration

The `for w in words` loop compiles to the iterator protocol:
1. `ori_iter_from_list(data, len, cap, elem_size=24, elem_dec)` — creates an opaque iterator pointer
2. `ori_iter_next(iter, scratch, elem_size=24)` — copies the next element (24-byte string fat pointer) into a scratch buffer
3. The loop body loads the string fat pointer from the scratch buffer, stores it to another alloca for `ori_str_len`, and accumulates the result

The element iteration correctly handles fat pointer values — each string element is a 24-byte `{len, cap, data}` struct that gets copied out of the list buffer into the scratch space. The `ori_str_len` function receives a pointer to the string fat pointer and returns the character count.

**Observation**: The codegen creates two separate allocas (`iter_next.scratch` and `str_len.self`) when a single alloca would suffice — `ori_str_len` could read directly from the iterator scratch buffer. This adds an unnecessary store+load round-trip per iteration. [HIGH-6]

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | CRITICAL | ARC | Double-free on string elements: iterator and list destructor both call _ori_elem_dec$3 | NEW | J15 |
| 2 | CRITICAL | ARC | Double ori_buffer_rc_dec in @main bb2 unwind path | NEW | J15 |
| 3 | MEDIUM | IR Quality | @total_items loads all 3 struct fields but only uses len | NEW | J15 |
| 4 | LOW | Attributes | Missing nounwind on entry main wrapper | CONFIRMED | J1 |
| 5 | NOTE | Attributes | readonly correctly applied to @total_items | NEW | J15 |
| 6 | HIGH | IR Quality | Redundant alloca + store/load round-trip for str_len.self in @count_chars loop | NEW | J15 |
| 7 | LOW | Codegen | ERROR log during codegen: extract_value on non-struct value (index 2) | NEW | J15 |

### CRITICAL-1: Double-free on string elements in nested ARC cleanup

**Location**: Interaction between `ori_iter_drop` (in @count_chars) and `ori_buffer_rc_dec` (in @main)
**Impact**: Runtime FATAL error — `ori_rc_dec called on already-freed allocation`. Program produces correct result but has undefined behavior due to use-after-free on string data pointers.
**Root cause**: Both the iterator runtime and the list buffer destructor call `_ori_elem_dec$3` on the same string elements. The iterator consumes elements during iteration and may decrement their RCs on drop. When the list buffer is later dropped (RC reaches 0), it tries to decrement the same string elements again.
**Fix**: Either (a) the iterator should NOT call the element destructor on elements it has already yielded (they are borrowed, not consumed), or (b) the list buffer drop should not call the element destructor on elements that were consumed by an iterator, or (c) the codegen should rc_inc each string element when yielding from the iterator.
**First seen**: Journey 15
**Found in**: ARC Purity (Category 2), Nested ARC (Category 8)

### CRITICAL-2: Double ori_buffer_rc_dec in @main bb2 unwind path

**Location**: @main, bb2 (landingpad for count_chars unwind)
**Impact**: If count_chars unwinds (e.g., overflow panic), the list buffer is RC-decremented twice in the same cleanup block. With RC=2, first dec takes it to 1, second to 0 (triggering drop), but these are two separate dec calls on the same value — the second call operates on already-freed memory.
**Fix**: The unwind cleanup should emit exactly one `ori_buffer_rc_dec` for the rc_inc copy and one for the original — but since the original hasn't been consumed yet at bb2, only one total dec is needed to match the one rc_inc.
**First seen**: Journey 15
**Found in**: ARC Purity (Category 2)

### MEDIUM-3: Dead field loads in @total_items

**Location**: @total_items, fields 1 (cap) and 2 (data)
**Impact**: 8 unnecessary instructions. LLVM's dead code elimination may remove these at -O1+, but at -O0 they remain as wasted work.
**Fix**: The codegen should detect when only a subset of struct fields are used and generate targeted loads rather than loading the entire aggregate.
**First seen**: Journey 15
**Found in**: Instruction Purity (Category 1), Optimal IR Comparison (Category 7)

### LOW-4: Missing nounwind on entry main wrapper

**Location**: `define noundef i32 @main()` — missing `nounwind`
**Impact**: LLVM generates unnecessary exception handling tables for the entry wrapper
**Fix**: Add `nounwind` attribute to the C `main()` wrapper since it never unwinds
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-5: readonly correctly applied to @total_items

**Location**: @total_items function declaration — `readonly` attribute present
**Impact**: Positive — enables LLVM to optimize callers knowing the function has no side effects
**Found in**: Attributes & Calling Convention (Category 3)

### HIGH-6: Redundant alloca and store/load round-trip in @count_chars loop

**Location**: @count_chars bb2 — `%str_len.self` alloca used only to pass the string to `ori_str_len`
**Impact**: One extra alloca, one store, and one load per loop iteration. The `iter_next.scratch` buffer already contains the string fat pointer in the correct layout — `ori_str_len` could read directly from it.
**Fix**: Pass `%iter_next.scratch` directly to `ori_str_len` instead of copying to a separate alloca
**First seen**: Journey 15
**Found in**: Fat Pointers: Element Iteration (Category 9)

### LOW-7: ERROR log during codegen about extract_value on non-struct value

**Location**: Codegen phase — `ori_llvm::codegen::ir_builder::aggregates`
**Impact**: An ERROR-level log is emitted: `extract_value on non-struct value (index 2) -- type resolution produced wrong layout`. This suggests a type resolution mismatch during code generation. The program still compiles correctly but the error indicates a code path that falls back to a default value.
**Fix**: Investigate the type resolution path for the `[str]` list type in the LLVM codegen phase
**First seen**: Journey 15
**Found in**: Fat Pointers: Element Iteration (Category 9)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 7/10 | 1.30x avg ratio (max 3.67x) |
| ARC Correctness | 20% | 3/10 | 2 violations |
| Attributes & Safety | 10% | 7/10 | 85.7% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 7/10 | 8 unjustified instructions |
| Binary Quality | 10% | 9/10 | 1 defect |
| Other Findings | 15% | 4/10 | 1 critical, 1 high, 1 low |

**Overall: 6.2 / 10**

Gates applied:
- arc_unbalanced_gate: unbalanced RC pair (leak/double-free), capped at 3

## Verdict

Journey 15 exposes a critical double-free bug in nested ARC cleanup for `[str]` lists. When a list of strings is iterated and then dropped, both the iterator and the list buffer destructor attempt to free the same string elements, causing a `FATAL` runtime error. The codegen produces the correct result (18) but has undefined behavior. Additionally, the unwind path in `@main` emits duplicate `ori_buffer_rc_dec` calls. On the positive side, `readonly` is correctly applied to `@total_items`, control flow is clean, and overflow checking works correctly.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| String ARC (SSO guard) | J9 | J15 | CONFIRMED (SSO guard correct in _ori_elem_dec$3) |
| List allocation + iteration | J10 | J15 | REGRESSED (J10 [int] worked; J15 [str] double-frees) |
| Iterator protocol | J13 | J15 | CONFIRMED (iter_from_list/iter_next/iter_drop pattern) |
| Overflow checking | J1 | J15 | CONFIRMED |
| readonly attribute | J15 | J15 | NEW (first seen on a collection function) |

The regression from J10 to J15 is significant: J10 tested `[int]` lists where elements are scalars (no element-level RC needed). J15 introduces `[str]` where each element has its own RC lifecycle. The element destructor callback (`_ori_elem_dec$3`) is correctly generated, but the interaction between iterator element consumption and list buffer destruction creates a double-free. This is the first journey to expose nested ARC cleanup bugs.
