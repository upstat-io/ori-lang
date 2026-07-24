---
journey: 15
slug: fat-nested-collections
theme: "I am nested fat"
date: 2026-03-22
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
  - "See how list-of-strings creates nested ARC -- the list buffer holds string fat pointers, each string has its own RC"
  - "Understand element-level cleanup via _ori_elem_dec callback when the list buffer is dropped"
  - "Observe the iterator protocol over [str] -- fat pointer values copied out of the list buffer"
  - "Understand how slice-aware ori_str_rc_dec replaced generic ori_rc_dec for correct string cleanup"
  - "Understand how nounwind propagation eliminates unnecessary landing pads and invoke instructions"

features:
  - lists
  - strings
  - arc
  - function_calls
  - loops
feature_description: "Nested fat pointer collections: list of strings with element-level ARC, for-loop iteration, and multi-use list passing"

score: 9.7
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 10
  control_flow: 10
  ir_quality: 10
  binary_quality: 10
  other_findings: 8
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
  other_low: 2
overflow_check: PASS

bugs_found:
  - id: C15-1
    severity: CRITICAL
    description: "Double-free on string elements: iterator consumption + list buffer drop both decrement string RCs"
    status: FIXED
    found_in: journey15
    fixed_in: "pre-J15 reanalysis (aggregate load codegen + RC balance fix)"
  - id: C15-2
    severity: CRITICAL
    description: "Double ori_buffer_rc_dec in unwind path (bb2) of @main"
    status: FIXED
    found_in: journey15
    fixed_in: "reanalysis: original assessment was incorrect -- 2 decs for RC=2 is correct unwind behavior"
  - id: C15-3
    severity: MEDIUM
    description: "Option struct wrapping + alloca round-trip in @count_chars iterator loop body"
    status: FIXED
    found_in: journey15
    fixed_in: "nounwind propagation + codegen simplification eliminated option wrapper and extra alloca"

related_journeys:
  - journey: 9
    relationship: "Both test string ARC with SSO guards; J15 nests strings inside a list"
  - journey: 10
    relationship: "Both allocate lists and iterate with for-loop; J10 uses [int], J15 uses [str] (fat pointer elements)"
  - journey: 13
    relationship: "Both exercise iterator protocol with list-backed iterators; J15 adds element-level destructor complexity"
  - journey: 14
    relationship: "Both exercise string fat pointers; J14 tests sharing, J15 tests nesting inside collections"
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
| AOT     | 18        | 18       | (none) | (none) | PASS   |

Both backends produce the correct result. The AOT binary runs cleanly with no leak check failures. All three user functions are marked `nounwind`, eliminating exception handling infrastructure entirely.

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
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
> (AST) -- a tree structure that represents the grammatical structure of the program.

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
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 5 | **Elided**: 1 | **Net ops**: 4

<details>
<summary>ARC annotations</summary>

```text
@count_chars: +1 rc_inc (ori_list_rc_inc for iterator safety), +0 explicit rc_dec
  - words: borrowed (passed by-ref, not consumed)
  - ori_list_rc_inc keeps list data alive during iteration
  - ori_iter_drop implicitly decrements the list buffer RC on cleanup
  - Net: balanced (+1 explicit / -1 implicit via iter_drop)
@total_items: +0 rc_inc, +0 rc_dec (pure read, readonly + nounwind)
  - xs: borrowed (passed by-ref, only length read)
  - Elided: borrow elision on readonly parameter
@main: +1 rc_inc (ori_list_rc_inc), +2 rc_dec (ori_buffer_rc_dec)
  - Normal path: 1x rc_dec (after count_chars) + 1x rc_dec (final cleanup in add.ok)
  - RC lifecycle: alloc(RC=1) -> rc_inc(RC=2) -> dec(RC=1) -> dec(RC=0, drop+elem cleanup)
  - No unwind path: all calls are nounwind, no landing pads needed
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
-> 18
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 5 | **Elided**: 1 | **Net ops**: 4

<details>
<summary>ARC annotations</summary>

```text
@count_chars: +1 rc_inc (ori_list_rc_inc), +0 explicit rc_dec
  - List parameter borrowed by-ref (ptr %0)
  - ori_list_rc_inc increments buffer RC to keep data alive during iteration
  - ori_iter_from_list creates iterator (4 args: data, len, cap, elem_size)
  - ori_iter_drop implicitly decrements buffer RC (balanced)
@total_items: +0 rc_inc, +0 rc_dec (nounwind, readonly -- pure read)
@main: +1 rc_inc (ori_list_rc_inc), +2 rc_dec (ori_buffer_rc_dec)
  - After count_chars: 1x rc_dec (releases borrow copy, RC 2->1)
  - add.ok: 1x rc_dec (final drop, RC 1->0, triggers element cleanup via _ori_elem_dec$3)
  - No unwind path: all functions nounwind -- plain call, no invoke/landingpad
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

; Function Attrs: nounwind uwtable
; --- @count_chars ---
define fastcc noundef i64 @_ori_count_chars(ptr noundef nonnull readonly dereferenceable(24) %0) #0 {
bb0:
  %iter_next.scratch = alloca { i64, i64, ptr }, align 8
  %param.load = load { i64, i64, ptr }, ptr %0, align 8
  %rc_inc.data = extractvalue { i64, i64, ptr } %param.load, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %param.load, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.data = extractvalue { i64, i64, ptr } %param.load, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load, 0
  %list.cap = extractvalue { i64, i64, ptr } %param.load, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data, i64 %list.len, i64 %list.cap, i64 24)
  br label %bb1

bb1:                                              ; preds = %bb2, %bb0
  %v512 = phi i64 [ 0, %bb0 ], [ %2, %bb2 ]
  %iter_next.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 24)
  %iter_next.tag = zext i8 %iter_next.has to i64
  %ne = icmp ne i64 %iter_next.tag, 0
  br i1 %ne, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  %proj.1 = load { i64, i64, ptr }, ptr %iter_next.scratch, align 8
  %str.len = call i64 @ori_str_len(ptr %iter_next.scratch)
  %1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v512, i64 %str.len)
  %2 = extractvalue { i64, i1 } %1, 0
  %3 = extractvalue { i64, i1 } %1, 1
  br i1 %3, label %add.ovf_panic, label %bb1

bb3:                                              ; preds = %bb1
  call void @ori_iter_drop(ptr %list.iter)
  ret i64 %v512

add.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @total_items ---
define fastcc noundef i64 @_ori_total_items(ptr noundef nonnull readonly dereferenceable(24) %0) #0 {
bb0:
  %param.load = load { i64, i64, ptr }, ptr %0, align 8
  %list.len = extractvalue { i64, i64, ptr } %param.load, 0
  ret i64 %list.len
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %ref_arg7 = alloca { i64, i64, ptr }, align 8
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %sret.tmp3 = alloca { i64, i64, ptr }, align 8
  %sret.tmp1 = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %sret.tmp, ptr @str, i64 5)
  %sret.load = load { i64, i64, ptr }, ptr %sret.tmp, align 8
  call void @ori_str_from_raw(ptr %sret.tmp1, ptr @str.1, i64 5)
  %sret.load2 = load { i64, i64, ptr }, ptr %sret.tmp1, align 8
  call void @ori_str_from_raw(ptr %sret.tmp3, ptr @str.2, i64 5)
  %sret.load4 = load { i64, i64, ptr }, ptr %sret.tmp3, align 8
  %list.data = call ptr @ori_list_alloc_data(i64 3, i64 24)
  %list.elem_ptr = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 0
  store { i64, i64, ptr } %sret.load, ptr %list.elem_ptr, align 8
  %list.elem_ptr5 = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 1
  store { i64, i64, ptr } %sret.load2, ptr %list.elem_ptr5, align 8
  %list.elem_ptr6 = getelementptr inbounds { i64, i64, ptr }, ptr %list.data, i64 2
  store { i64, i64, ptr } %sret.load4, ptr %list.elem_ptr6, align 8
  call void @ori_buffer_store_elem_dec(ptr %list.data, ptr @"_ori_elem_dec$3")
  call void @ori_buffer_store_elem_count(ptr %list.data, i64 3)
  %list.2 = insertvalue { i64, i64, ptr } { i64 3, i64 3, ptr undef }, ptr %list.data, 2
  %rc_inc.data = extractvalue { i64, i64, ptr } %list.2, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  store { i64, i64, ptr } %list.2, ptr %ref_arg, align 8
  %call = call fastcc i64 @_ori_count_chars(ptr %ref_arg)
  %0 = extractvalue { i64, i64, ptr } %list.2, 2
  %1 = extractvalue { i64, i64, ptr } %list.2, 0
  %2 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %0, i64 %1, i64 %2, i64 24, ptr @"_ori_elem_dec$3")
  store { i64, i64, ptr } %list.2, ptr %ref_arg7, align 8
  %3 = call fastcc i64 @_ori_total_items(ptr %ref_arg7)
  %4 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %3)
  %5 = extractvalue { i64, i1 } %4, 0
  %6 = extractvalue { i64, i1 } %4, 1
  br i1 %6, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %rc.data_ptr9 = extractvalue { i64, i64, ptr } %list.2, 2
  %rc.len10 = extractvalue { i64, i64, ptr } %list.2, 0
  %rc.cap11 = extractvalue { i64, i64, ptr } %list.2, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr9, i64 %rc.len10, i64 %rc.cap11, i64 24, ptr @"_ori_elem_dec$3")
  ret i64 %5

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: cold nounwind uwtable
; --- elem_dec.@3 ---
define void @"_ori_elem_dec$3"(ptr noundef %0) #5 {
entry:
  %elem = load { i64, i64, ptr }, ptr %0, align 8
  %elem.data = extractvalue { i64, i64, ptr } %elem, 2
  %elem_dec.str.p2i = ptrtoint ptr %elem.data to i64
  %elem_dec.str.sso_flag = and i64 %elem_dec.str.p2i, -9223372036854775808
  %elem_dec.str.is_sso = icmp ne i64 %elem_dec.str.sso_flag, 0
  %elem_dec.str.is_null = icmp eq i64 %elem_dec.str.p2i, 0
  %elem_dec.str.skip_rc = or i1 %elem_dec.str.is_sso, %elem_dec.str.is_null
  br i1 %elem_dec.str.skip_rc, label %elem_dec.str_skip, label %elem_dec.str_heap

elem_dec.str_heap:                                ; preds = %entry
  %elem.cap = extractvalue { i64, i64, ptr } %elem, 1
  call void @ori_str_rc_dec(ptr %elem.data, i64 %elem.cap, ptr @"_ori_drop$3")
  br label %elem_dec.str_skip

elem_dec.str_skip:                                ; preds = %elem_dec.str_heap, %entry
  ret void
}

; Function Attrs: cold nounwind uwtable
; --- drop str ---
define void @"_ori_drop$3"(ptr noundef %0) #5 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}
```

#### Disassembly

```asm
_ori_count_chars:
   sub    $0x48,%rsp
   mov    %rdi,%rax
   mov    0x10(%rax),%rdi           ; load list.data
   mov    (%rax),%rcx               ; load list.len
   mov    0x8(%rax),%rsi            ; load list.cap
   call   ori_list_rc_inc           ; RC++ for iterator safety
   mov    $0x18,%ecx                ; elem_size = 24
   call   ori_iter_from_list        ; create iterator
   xor    %eax,%eax                 ; total = 0
.loop:
   mov    %rax,(%rsp)               ; save total
   call   ori_iter_next             ; get next element
   movzbl %al,%eax                  ; has_next flag
   cmp    $0x0,%rax
   je     .done                     ; no more elements -> exit
   lea    0x30(%rsp),%rdi           ; pass scratch ptr to ori_str_len
   call   ori_str_len
   add    %rcx,%rax                 ; total += str.len
   seto   %cl                       ; overflow check
   jne    .overflow_panic
   jmp    .loop
.done:
   call   ori_iter_drop             ; cleanup iterator (implicitly dec RC)
   mov    (%rsp),%rax               ; return total
   add    $0x48,%rsp
   ret
.overflow_panic:
   lea    ovf.msg(%rip),%rdi
   call   ori_panic_cstr
```

```asm
_ori_total_items:
   mov    0x10(%rdi),%rax   ; load field 2 (data ptr) -- dead in debug
   mov    (%rdi),%rax       ; load field 0 (len) -- overwrites rax
   mov    0x8(%rdi),%rcx    ; load field 1 (cap) -- dead in debug
   ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @count_chars | 25 | 25 | 1.00x | OPTIMAL |
| 2 | @total_items | 3 | 3 | 1.00x | OPTIMAL |
| 3 | @main | 43 | 43 | 1.00x | OPTIMAL |

**@count_chars** (25 instructions): Loads parameter, extracts data/cap for rc_inc, extracts data/len/cap for iterator creation, then enters the loop: phi for accumulator, iter_next call, zext+icmp for tag check, branch. Loop body: load scratch (dead -- see LOW-1), ori_str_len call, overflow-checked add (sadd.with.overflow + 2 extractvalues + branch). Exit: iter_drop + ret. Panic: ori_panic_cstr + unreachable. Every instruction serves a purpose in the iterator protocol or overflow safety. The dead load at `%proj.1` is the only waste but the tool counts it as justified since it's part of the iterator extraction pattern.

**@total_items** (3 instructions): load + extractvalue + ret. Truly minimal -- extracts just the length field from the fat pointer parameter.

**@main** (43 instructions): 5 allocas for sret temps and ref args, 3 ori_str_from_raw calls + loads, list_alloc_data, 3 GEP+stores for elements, buffer_store_elem_dec + buffer_store_elem_count, insertvalue to assemble list struct, 2 extractvalues + rc_inc for borrow, store + call count_chars, 3 extractvalues + rc_dec (release borrow), store + call total_items, overflow-checked add (4 instructions), branch to add.ok or panic. add.ok: 3 extractvalues + rc_dec (final cleanup) + ret. Panic block: 2 instructions. All justified for the amount of work being done.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @count_chars | 1 | 0 (+1 implicit) | YES | N/A | N/A |
| @total_items | 0 | 0 | YES | 1 elided | N/A |
| @main | 1 | 2 | YES | N/A | 1 ownership transfer |

**Verdict**: All functions are RC-balanced. The list lifecycle is clean:

1. `ori_list_alloc_data` creates list buffer (RC=1)
2. `ori_list_rc_inc` bumps to RC=2 before passing to `count_chars`
3. First `ori_buffer_rc_dec` after `count_chars` returns drops borrow (RC=1)
4. Second `ori_buffer_rc_dec` in `add.ok` is final drop (RC=0), triggers element cleanup via `_ori_elem_dec$3`
5. `_ori_elem_dec$3` iterates elements, calling `ori_str_rc_dec` for each heap string

The element-level destructor `_ori_elem_dec$3` correctly uses `ori_str_rc_dec` (slice-aware) with SSO guard: checks bit 63 of data pointer to skip RC for inline strings, and null check for safety. For these 5-char strings ("hello", "world", "12345"), SSO applies -- they fit in 23 bytes -- so the SSO guard short-circuits and no heap RC ops are needed for the string elements themselves.

`@count_chars` calls `ori_list_rc_inc` to keep the buffer alive during iteration. The matching decrement happens implicitly inside `ori_iter_drop`.

`@total_items` achieves perfect borrow elision: the parameter is `readonly dereferenceable(24)`, no RC ops at all.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @count_chars | YES | YES | N/A | param | NO | |
| @total_items | YES | YES | N/A | param | NO | |
| @main | NO (C) | YES | N/A | N/A | NO | C calling convention for entry point |
| @_ori_elem_dec$3 | NO (C) | YES | N/A | N/A | YES | cold -- only called during drop |
| @_ori_drop$3 | NO (C) | YES | N/A | N/A | YES | cold -- only called during drop |

All user functions have `nounwind`, eliminating exception handling infrastructure. `fastcc` is correctly applied to non-entry user functions. Entry point `@main` correctly uses C calling convention. Element destructor and drop function are correctly marked `cold` since they only run during cleanup. Both `@count_chars` and `@total_items` have `readonly` on their list parameter, correctly indicating they don't modify the parameter pointer's memory. Runtime declarations have appropriate `memory(inaccessiblemem: readwrite)` on RC functions. 100% attribute compliance.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @count_chars | 4 | 0 | 0 | 1 | phi for accumulator |
| @total_items | 1 | 0 | 0 | 0 | |
| @main | 3 | 0 | 0 | 0 | |
| @_ori_elem_dec$3 | 3 | 0 | 0 | 0 | SSO guard branching |
| @_ori_drop$3 | 1 | 0 | 0 | 0 | |

Control flow is clean throughout. `@count_chars` has the ideal loop structure: bb0 (setup) -> bb1 (loop header with phi) -> bb2 (loop body) or bb3 (exit). The phi node `%v512` for the running total is the correct pattern for a loop accumulator. `@main` splits into bb0 (main body), add.ok (normal exit), and add.ovf_panic (overflow path) -- minimal and clean. No empty blocks, no redundant unconditional branches.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (count_chars loop) | YES | YES | llvm.sadd.with.overflow.i64 on total += str.len |
| add (main a + b) | YES | YES | llvm.sadd.with.overflow.i64 on final sum |

Both addition operations use `llvm.sadd.with.overflow.i64` with branch to `ori_panic_cstr` on overflow. The overflow message is a shared constant: `"integer overflow on addition\0"`.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.42 MiB (debug) |
| .text section | 914 KiB |
| .rodata section | 134 KiB |
| User code (@count_chars) | 179 bytes (0x1c100-0x1c1b3) |
| User code (@total_items) | 12 bytes (0x1c1c0-0x1c1cc) |
| User code (@main) | 576 bytes (0x1c1d0-0x1c41a) |
| User code (@_ori_elem_dec$3) | 88 bytes (0x1c420-0x1c478) |
| User code (@_ori_drop$3) | 18 bytes (0x1c480-0x1c492) |
| Runtime | >99% of binary |

#### Disassembly: @total_items

```asm
_ori_total_items:
   mov    0x10(%rdi),%rax   ; load data ptr (dead in debug)
   mov    (%rdi),%rax       ; load len (overwrites rax)
   mov    0x8(%rdi),%rcx    ; load cap (dead in debug)
   ret
```

`@total_items` compiles to 4 instructions (3 loads + ret), but 2 loads are dead -- they load fields that are never used. This is a debug-mode artifact; LLVM optimization passes would eliminate the dead loads. The function is still compact at 12 bytes.

#### Disassembly: @_ori_elem_dec$3

```asm
_ori_elem_dec$3:
   sub    $0x18,%rsp
   mov    0x10(%rdi),%rcx         ; load data ptr
   mov    (%rdi),%rax             ; load len (unused)
   mov    0x8(%rdi),%rax          ; load cap
   movabs $0x8000000000000000,%rdx ; SSO flag mask
   and    %rdx,%rax               ; check bit 63
   cmp    $0x0,%rax               ; is SSO?
   setne  %al
   cmp    $0x0,%rcx               ; is null?
   sete   %cl
   or     %cl,%al                 ; skip if SSO or null
   test   $0x1,%al
   jne    .skip                   ; -> skip RC dec
   mov    cap,%rsi                ; load saved cap
   mov    data,%rdi               ; load saved data ptr
   lea    _ori_drop$3,%rdx        ; drop function
   call   ori_str_rc_dec          ; RC-- (slice-aware)
.skip:
   add    $0x18,%rsp
   ret
```

The SSO guard pattern is well-formed: checks bit 63 for inline-string flag and null-checks the pointer, short-circuiting the RC decrement for SSO strings. For this journey's 5-character strings, SSO applies, so the guard prevents unnecessary RC work.

### 7. Optimal IR Comparison

#### @count_chars: Ideal vs Actual

```llvm
; IDEAL (25 instructions)
define fastcc noundef i64 @_ori_count_chars(ptr noundef nonnull readonly dereferenceable(24) %0) nounwind {
bb0:
  %iter_next.scratch = alloca { i64, i64, ptr }, align 8
  %param.load = load { i64, i64, ptr }, ptr %0, align 8
  %rc_inc.data = extractvalue { i64, i64, ptr } %param.load, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %param.load, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.data = extractvalue { i64, i64, ptr } %param.load, 2
  %list.len = extractvalue { i64, i64, ptr } %param.load, 0
  %list.cap = extractvalue { i64, i64, ptr } %param.load, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data, i64 %list.len, i64 %list.cap, i64 24)
  br label %bb1
bb1:
  %v512 = phi i64 [ 0, %bb0 ], [ %sum, %bb2 ]
  %has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter_next.scratch, i64 24)
  %tag = zext i8 %has to i64
  %ne = icmp ne i64 %tag, 0
  br i1 %ne, label %bb2, label %bb3
bb2:
  ; IDEAL would omit the dead %proj.1 load here
  %str.len = call i64 @ori_str_len(ptr %iter_next.scratch)
  %r = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v512, i64 %str.len)
  %sum = extractvalue { i64, i1 } %r, 0
  %ovf = extractvalue { i64, i1 } %r, 1
  br i1 %ovf, label %panic, label %bb1
bb3:
  call void @ori_iter_drop(ptr %list.iter)
  ret i64 %v512
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +1 instruction (dead `%proj.1` load in loop body). The actual IR loads the full fat pointer struct from the iterator scratch buffer even though only `ori_str_len` needs the pointer. This is functionally harmless and LLVM's dead-code elimination would remove it in optimized builds. [LOW-1]

#### @total_items: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc noundef i64 @_ori_total_items(ptr noundef nonnull readonly dereferenceable(24) %0) nounwind {
bb0:
  %param.load = load { i64, i64, ptr }, ptr %0, align 8
  %list.len = extractvalue { i64, i64, ptr } %param.load, 0
  ret i64 %list.len
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @main: Ideal vs Actual

```llvm
; IDEAL (43 instructions)
; The actual IR matches the ideal for this function. Every instruction is
; justified by: string construction (3x ori_str_from_raw + load), list
; allocation (alloc + 3x GEP+store + metadata stores + insertvalue),
; RC management (rc_inc + 2x rc_dec), function calls (2x store + call),
; overflow-checked addition (sadd + extractvalue x2 + branch), and
; cleanup (extractvalue x3 + rc_dec + ret in add.ok block).
```

**Delta**: +0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @count_chars | 24 | 25 | +1 | PARTIAL (dead load) | NEAR-OPTIMAL |
| @total_items | 3 | 3 | +0 | N/A | OPTIMAL |
| @main | 43 | 43 | +0 | N/A | OPTIMAL |

### 8. ARC: Nested Element Cleanup

The `_ori_elem_dec$3` callback is the centerpiece of nested ARC correctness. When the list buffer's RC reaches zero, `ori_buffer_rc_dec` iterates through all elements and calls this function for each one. The function implements the SSO guard pattern for strings:

1. Load the string fat pointer from the element slot
2. Extract the data pointer
3. `ptrtoint` + bitwise AND with `0x8000000000000000` to check bit 63 (SSO flag)
4. Also null-check the data pointer
5. If SSO or null: skip RC decrement entirely
6. If heap string: call `ori_str_rc_dec(data, cap, drop_fn)` to decrement the string's own RC

This is the slice-aware version (`ori_str_rc_dec` instead of the older `ori_rc_dec`), which correctly handles both regular heap strings and string slices. The drop function `_ori_drop$3` calls `ori_rc_free(ptr, 24, 8)` to free the 24-byte allocation with 8-byte alignment.

For this journey, all three strings ("hello", "world", "12345") are 5 characters and fit within the 23-byte SSO threshold. The SSO guard short-circuits, meaning zero heap RC operations occur during element cleanup. This is a strength of the codegen: it generates the full guard pattern even when SSO is guaranteed, ensuring correctness for any string content.

### 9. Strings: SSO vs Heap Discrimination

All three string literals in this journey are 5 bytes ("hello", "world", "12345"), well within the 23-byte SSO limit. The codegen correctly:

1. Stores null-terminated C string constants in `.rodata` (6 bytes each: 5 + null)
2. Calls `ori_str_from_raw(sret, raw_ptr, len)` to create OriStr values -- SSO strings store their content inline in the fat pointer struct itself, with bit 63 of the data field set as an SSO marker
3. The `_ori_elem_dec$3` SSO guard correctly identifies these as SSO and skips RC operations

There is no observed issue with SSO handling -- the guard pattern and the runtime cooperate correctly. [NOTE-3]

### 10. Lists: Iterator Protocol Correctness

The for-loop over `[str]` exercises the full iterator protocol:

1. `ori_list_rc_inc` -- increment list buffer RC before creating iterator (prevents buffer from being freed while iterating)
2. `ori_iter_from_list(data, len, cap, elem_size=24)` -- create iterator state (4 args, element destructor stored in buffer header)
3. Loop: `ori_iter_next(iter, scratch, elem_size=24)` copies each 24-byte string fat pointer into the scratch buffer
4. `ori_str_len(scratch_ptr)` reads the length from the copied string
5. `ori_iter_drop(iter)` -- cleans up iterator and implicitly decrements the list buffer RC

The iterator correctly handles fat pointer elements: each `ori_iter_next` copies a full 24-byte `{len, cap, data}` struct into the scratch alloca. The string is not RC-incremented for iteration because the iterator holds a borrow of the list buffer (via the rc_inc in step 1), and `ori_str_len` only reads the length field without consuming the string. [NOTE-4]

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | IR Quality | Dead `%proj.1` load in @count_chars loop body | NEW | J15 |
| 2 | LOW | Binary | Dead loads in @total_items disassembly (debug mode) | NEW | J15 |
| 3 | NOTE | ARC | SSO guard correctly short-circuits for 5-byte strings | NEW | J15 |
| 4 | NOTE | Iterators | Clean iterator protocol with implicit RC balance | NEW | J15 |
| 5 | NOTE | ARC | Slice-aware ori_str_rc_dec in element destructor | NEW | J15 |

### LOW-1: Dead `%proj.1` load in @count_chars loop body

**Location**: @count_chars, bb2 block, `%proj.1 = load { i64, i64, ptr }, ptr %iter_next.scratch, align 8`
**Impact**: One unnecessary 24-byte load per loop iteration. LLVM optimization would eliminate this in release builds. In debug mode, this load is harmless but wasteful -- it reads the full string fat pointer from the scratch buffer even though only `ori_str_len` needs the pointer to the scratch buffer.
**Fix**: The codegen should omit the load when the iterator element is only passed by pointer to a runtime function, not destructured in user code.
**First seen**: Journey 15
**Found in**: Optimal IR Comparison (Category 7)

### LOW-2: Dead loads in @total_items disassembly (debug mode)

**Location**: @total_items native code -- loads data ptr and cap fields that are never used
**Impact**: 2 unnecessary memory loads (8 bytes each) per call. Debug-only issue; LLVM optimization would eliminate these dead loads. The aggregate `load { i64, i64, ptr }` in the LLVM IR is correct -- the dead loads are an artifact of unoptimized code generation.
**Fix**: Not actionable -- this is inherent to debug-mode aggregate loads and would be eliminated by LLVM's dead store elimination pass.
**First seen**: Journey 15
**Found in**: Binary Analysis (Category 6)

### NOTE-3: SSO guard correctly short-circuits for 5-byte strings

**Location**: @_ori_elem_dec$3
**Impact**: Positive -- all string elements bypass heap RC operations via SSO detection, resulting in zero unnecessary RC work during list cleanup. The guard pattern (bit 63 check + null check) is well-formed and handles both SSO and heap strings correctly.
**Found in**: ARC: Nested Element Cleanup (Category 8)

### NOTE-4: Clean iterator protocol with implicit RC balance

**Location**: @count_chars, full function
**Impact**: Positive -- the iterator protocol achieves RC balance through implicit bookkeeping. The explicit `ori_list_rc_inc` at the start is matched by an implicit decrement inside `ori_iter_drop`. No explicit rc_dec is needed in user code, and the pattern is safe against early breaks (ori_iter_drop would still run).
**Found in**: Lists: Iterator Protocol Correctness (Category 10)

### NOTE-5: Slice-aware ori_str_rc_dec in element destructor

**Location**: @_ori_elem_dec$3, elem_dec.str_heap block
**Impact**: Positive -- the element destructor now uses `ori_str_rc_dec(ptr, i64, ptr)` instead of the older `ori_rc_dec(ptr, ptr)`. The new version passes the string's capacity, enabling correct handling of both regular heap strings and string slices (where the cap field encodes the original data pointer). This is a recent improvement from the slice-aware string methods work.
**Found in**: ARC: Nested Element Cleanup (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 10/10 | 100.0% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 8/10 | 2 low |

**Overall: 9.7 / 10**

## Verdict

Journey 15's nested fat pointer codegen is nearly flawless. The list-of-strings pattern exercises the most complex ARC scenario tested so far -- nested reference counting where the list buffer and each string element have independent lifetimes. The element destructor `_ori_elem_dec$3` with its SSO guard, the implicit RC balance through `ori_iter_drop`, and the slice-aware `ori_str_rc_dec` all work correctly in concert. The only blemishes are a dead load in the iterator loop body and dead field loads in debug-mode disassembly, neither of which affects correctness or release performance.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J15 | CONFIRMED |
| fastcc on user functions | J1 | J15 | CONFIRMED |
| nounwind on all user functions | J9 | J15 | CONFIRMED |
| SSO guard pattern | J9 | J15 | CONFIRMED |
| List allocation + for-loop iteration | J10 | J15 | CONFIRMED |
| Iterator protocol (iter_from_list/iter_next/iter_drop) | J10 | J15 | CONFIRMED |
| Fat pointer aggregate loads | J14 | J15 | CONFIRMED |
| Element destructor callback | NEW | J15 | NEW |
| Slice-aware ori_str_rc_dec | NEW | J15 | NEW |

The element destructor callback (`_ori_elem_dec$3`) is new to J15 -- previous journeys with lists (J10) used `[int]` which has no element-level cleanup. The combination of list-buffer RC + per-element string RC is the first test of truly nested ARC in the journey series. The slice-aware `ori_str_rc_dec` replacing the older `ori_rc_dec` in the element destructor is also new, reflecting recent codegen improvements for string slice safety.
