---
journey: 9
slug: strings
theme: "I am a string"
date: 2026-03-08
status: PASS
expected: 13
eval_result: 13
aot_result: 13
difficulty: complex
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of reference counting and memory management"
  - "Familiarity with string representations (heap vs inline/SSO)"
learning_objectives:
  - "See how string literals are lowered to global constants and constructed at runtime via ori_str_from_raw"
  - "Understand SSO (Small String Optimization) gating in ARC cleanup"
  - "Compare heap-allocated vs SSO string ARC lifecycle in generated IR"
  - "Analyze method dispatch overhead for string .length() calls"
features:
  - strings
  - string_methods
  - arc
  - branching
feature_description: "String construction, method calls (.length()), boolean logic, and ARC lifecycle management"
score: 8.8
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 6
  control_flow: 7
  ir_quality: 8
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.03
  instruction_ratio_max: 1.05
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 23
  attr_correct: 17
  attr_has_wrong: false
  cf_defects: 4
  cf_incorrect: false
  ir_unjustified: 4
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 0
  other_low: 0
overflow_check: PASS
bugs_found: []
related_journeys:
  - journey: 2
    relationship: "Both test branching with if/then/else"
  - journey: 1
    relationship: "Both test overflow-checked arithmetic"
  - journey: 10
    relationship: "Both test ARC lifecycle for heap-allocated types"
---

# Journey 9: "I am a string"

## Source

```ori
// Journey 9: "I am a string"
// Slug: strings
// Difficulty: complex
// Features: strings, string_methods, arc, branching
// Expected: check_logic() + check_strings() = 2 + 11 = 13

@bool_to_int (b: bool) -> int = if b then 1 else 0;

@check_logic () -> int = {
    let a = true && true;       // true  -> 1
    let b = true && false;      // false -> 0
    let c = false || true;      // true  -> 1
    let d = false || false;     // false -> 0
    bool_to_int(b: a) + bool_to_int(b: b) + bool_to_int(b: c) + bool_to_int(b: d)
    // = 1 + 0 + 1 + 0 = 2
}

@check_strings () -> int = {
    let s1 = "hello";
    let s2 = "world!";
    let s3 = "";
    s1.length() + s2.length() + s3.length()
    // = 5 + 6 + 0 = 11
}

@main () -> int = {
    let a = check_logic();      // = 2
    let b = check_strings();    // = 11
    a + b                       // = 13
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 13        | 13       | (none) | (none) | PASS   |
| AOT     | 13        | 13       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 179 | **Keywords**: 14 | **Identifiers**: 32 | **Errors**: 0

<details>
<summary>Token stream (user module)</summary>

```text
Fn(@) Ident(bool_to_int) LParen Ident(b) Colon Ident(bool) RParen Arrow Ident(int)
Eq If Ident(b) Then Int(1) Else Int(0) Semi
Fn(@) Ident(check_logic) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(a) Eq True And And True Semi
  Let Ident(b) Eq True And And False Semi
  Let Ident(c) Eq False Or Or True Semi
  Let Ident(d) Eq False Or Or False Semi
  Ident(bool_to_int) LParen Ident(b) Colon Ident(a) RParen Plus ...
RBrace
Fn(@) Ident(check_strings) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(s1) Eq Str("hello") Semi
  Let Ident(s2) Eq Str("world!") Semi
  Let Ident(s3) Eq Str("") Semi
  Ident(s1) Dot Ident(length) LParen RParen Plus ...
RBrace
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace ... RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 52 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @bool_to_int
│  ├─ Params: (b: bool)
│  ├─ Return: int
│  └─ Body: If(Ident(b), Int(1), Int(0))
├─ FnDecl @check_logic
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let a = BinOp(&&, true, true)
│       ├─ Let b = BinOp(&&, true, false)
│       ├─ Let c = BinOp(||, false, true)
│       ├─ Let d = BinOp(||, false, false)
│       └─ BinOp(+, ..., ...)
│            ├─ Call(@bool_to_int, b: a) + Call(@bool_to_int, b: b)
│            └─ Call(@bool_to_int, b: c) + Call(@bool_to_int, b: d)
├─ FnDecl @check_strings
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s1 = Str("hello")
│       ├─ Let s2 = Str("world!")
│       ├─ Let s3 = Str("")
│       └─ BinOp(+, ..., ...)
│            ├─ MethodCall(s1, length) + MethodCall(s2, length)
│            └─ MethodCall(s3, length)
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@check_logic)
        ├─ Let b = Call(@check_strings)
        └─ BinOp(+, a, b)
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
@bool_to_int (b: bool) -> int = if b then 1 else 0
//                                        ^ int (literal)  ^ int (literal)
//                               ^ int (unified from both branches)

@check_logic () -> int = {
    let a: bool = true && true    // bool (LogicalAnd)
    let b: bool = true && false   // bool (LogicalAnd)
    let c: bool = false || true   // bool (LogicalOr)
    let d: bool = false || false  // bool (LogicalOr)
    bool_to_int(b: a) + bool_to_int(b: b) + bool_to_int(b: c) + bool_to_int(b: d)
    // ^ int (Add<int, int> -> int, chained)
}

@check_strings () -> int = {
    let s1: str = "hello"    // str (literal)
    let s2: str = "world!"   // str (literal)
    let s3: str = ""         // str (literal)
    s1.length() + s2.length() + s3.length()
    // .length() -> int (built-in str method)
    // + -> int (Add<int, int> -> int)
}

@main () -> int = {
    let a: int = check_logic()    // int (return type of @check_logic)
    let b: int = check_strings()  // int (return type of @check_strings)
    a + b                         // int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 4 | **Desugared**: 4 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- && / || short-circuit operators desugared to constant booleans (all operands are literals)
- Method calls (.length()) lowered to canonical MethodCall nodes
- String literals lowered to canonical Str constants
- Function bodies lowered to canonical expression form
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 6 | **Elided**: 0 | **Net ops**: 6

<details>
<summary>ARC annotations</summary>

```text
@bool_to_int: no heap values — pure scalar logic
@check_logic: no heap values — pure scalar arithmetic + booleans
@check_strings: +3 rc_inc (ori_str_from_raw), +3 rc_dec (str cleanup after .length())
  - s1 ("hello"): rc_inc via ori_str_from_raw, rc_dec after s1.length()
  - s2 ("world!"): rc_inc via ori_str_from_raw, rc_dec after s2.length()
  - s3 (""): rc_inc via ori_str_from_raw, rc_dec after s3.length()
  - SSO gating: all 3 rc_dec sites check for SSO/null before calling ori_rc_dec
@main: no heap values — passes through scalar int results
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 13 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  ├─ let a = @check_logic()
  │    ├─ let a = true && true  → true (constant)
  │    ├─ let b = true && false → false (constant)
  │    ├─ let c = false || true → true (constant)
  │    ├─ let d = false || false → false (constant)
  │    ├─ @bool_to_int(b: true)  → if true then 1 → 1
  │    ├─ @bool_to_int(b: false) → if false then 0 → 0
  │    ├─ @bool_to_int(b: true)  → 1
  │    ├─ @bool_to_int(b: false) → 0
  │    └─ 1 + 0 + 1 + 0 = 2
  ├─ let b = @check_strings()
  │    ├─ let s1 = "hello"   (str, len=5)
  │    ├─ let s2 = "world!"  (str, len=6)
  │    ├─ let s3 = ""        (str, len=0)
  │    ├─ s1.length() = 5
  │    ├─ s2.length() = 6
  │    ├─ s3.length() = 0
  │    └─ 5 + 6 + 0 = 11
  └─ 2 + 11 = 13
→ 13
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 6 | **Elided**: 0 | **Net ops**: 6

<details>
<summary>ARC annotations</summary>

```text
@bool_to_int: +0 rc_inc, +0 rc_dec (no heap values — pure scalar select)
@check_logic: +0 rc_inc, +0 rc_dec (no heap values — constant boolean folding)
@check_strings: +3 rc_inc, +3 rc_dec (balanced — 3 strings constructed, 3 cleaned up)
  - ori_str_from_raw() implicitly allocates (counted as rc_inc)
  - 3 SSO-gated ori_rc_dec calls for cleanup after each .length()
@main: +0 rc_inc, +0 rc_dec (scalar results only)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '09-strings'
source_filename = "09-strings"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [7 x i8] c"world!\00", align 1
@str.2 = private unnamed_addr constant [1 x i8] zeroinitializer, align 1

; Function Attrs: nounwind uwtable
; --- @bool_to_int ---
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) #0 {
bb0:
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}

; Function Attrs: nounwind uwtable
; --- @check_logic ---
define fastcc noundef i64 @_ori_check_logic() #0 {
bb0:
  %call = call fastcc i64 @_ori_bool_to_int(i1 true)
  %call1 = call fastcc i64 @_ori_bool_to_int(i1 false)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  %call2 = call fastcc i64 @_ori_bool_to_int(i1 true)
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  %add.val4 = extractvalue { i64, i1 } %add3, 0
  %add.ovf5 = extractvalue { i64, i1 } %add3, 1
  br i1 %add.ovf5, label %add.ovf_panic7, label %add.ok6

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok6:
  %call8 = call fastcc i64 @_ori_bool_to_int(i1 false)
  %add9 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val4, i64 %call8)
  %add.val10 = extractvalue { i64, i1 } %add9, 0
  %add.ovf11 = extractvalue { i64, i1 } %add9, 1
  br i1 %add.ovf11, label %add.ovf_panic13, label %add.ok12

add.ovf_panic7:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok12:
  ret i64 %add.val10

add.ovf_panic13:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: uwtable
; --- @check_strings ---
define fastcc noundef i64 @_ori_check_strings() #1 {
bb0:
  %str_len.self32 = alloca { i64, i64, ptr }, align 8
  %str_len.self21 = alloca { i64, i64, ptr }, align 8
  %str_len.self = alloca { i64, i64, ptr }, align 8
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
  ; ... (repeated for s2 "world!" and s3 "")
  ; ori_str_len calls + SSO-gated rc_dec for each string
  ; overflow-checked additions for length sums
  ret i64 %add.val44
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_check_logic()
  %call1 = call fastcc i64 @_ori_check_strings()
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; --- Runtime declarations ---
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2
declare void @ori_panic_cstr(ptr) #3                    ; cold noreturn
declare void @ori_str_from_raw(ptr noalias sret(...), ptr, i64) #4  ; nounwind
declare i64 @ori_str_len(ptr) #4                         ; nounwind
declare void @ori_rc_free(ptr, i64, i64) #4             ; nounwind
declare void @ori_rc_dec(ptr, ptr) #6                   ; nounwind memory(inaccessiblemem: readwrite)

; --- Drop glue ---
define void @"_ori_drop$3"(ptr %0) #5 {                 ; cold nounwind
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
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
attributes #3 = { cold noreturn }
attributes #5 = { cold nounwind }
attributes #6 = { nounwind memory(inaccessiblemem: readwrite) }
```

#### Disassembly

```asm
_ori_bool_to_int:
  mov    %dil,%dl
  xor    %eax,%eax
  mov    $0x1,%ecx
  test   $0x1,%dl
  cmovne %rcx,%rax
  ret

_ori_check_logic:
  sub    $0x28,%rsp
  mov    $0x1,%edi
  call   _ori_bool_to_int          ; bool_to_int(true)
  mov    %rax,0x18(%rsp)
  xor    %edi,%edi
  call   _ori_bool_to_int          ; bool_to_int(false)
  mov    %rax,%rcx
  mov    0x18(%rsp),%rax
  add    %rcx,%rax                 ; 1 + 0
  jo     .overflow
  ; ... (2 more calls + checked adds)
  ret

_ori_check_strings:
  sub    $0x108,%rsp
  ; ori_str_from_raw("hello", 5) -> alloca sret
  ; GEP+load+insertvalue to materialize { i64, i64, ptr } str struct
  ; ori_str_from_raw("world!", 6)
  ; ori_str_from_raw("", 0)
  ; ori_str_len(s1) -> length 5
  ; SSO check: test high bit of ptr field, skip rc_dec if SSO or null
  ; ori_rc_dec(s1.data, _ori_drop$3)
  ; ori_str_len(s2) -> length 6
  ; SSO check + ori_rc_dec(s2.data, _ori_drop$3)
  ; checked add: 5 + 6
  ; ori_str_len(s3) -> length 0
  ; SSO check + ori_rc_dec(s3.data, _ori_drop$3)
  ; checked add: 11 + 0
  ret

_ori_main:
  sub    $0x18,%rsp
  call   _ori_check_logic           ; -> 2
  mov    %rax,0x8(%rsp)
  call   _ori_check_strings         ; -> 11
  mov    %rax,%rcx
  mov    0x8(%rsp),%rax
  add    %rcx,%rax                  ; 2 + 11
  jo     .overflow
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @bool_to_int | 2 | 2 | 1.00x | OPTIMAL |
| 2 | @check_logic | 23 | 23 | 1.00x | OPTIMAL |
| 3 | @check_strings | 87 | 83 | 1.05x | NEAR-OPTIMAL |
| 4 | @main | 9 | 9 | 1.00x | OPTIMAL |

**@bool_to_int**: Pure `select` + `ret`. No overhead.

**@check_logic**: 4 calls to `@bool_to_int` with constant bool args, 3 overflow-checked additions, 3 panic blocks. All instructions justified -- boolean constant folding happens at canonicalization, overflow checking is required.

**@check_strings**: The 4 unjustified instructions are redundant unconditional branches (`br label`) in the SSO-gated rc_dec sequences. Each SSO check produces a diamond pattern (test -> heap/skip -> merge) but the merge blocks sometimes have unnecessary `br` to the next block instead of falling through. [MEDIUM-1]

**@main**: 2 calls + 1 checked add. OPTIMAL.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @bool_to_int | 0 | 0 | YES | N/A | N/A |
| @check_logic | 0 | 0 | YES | N/A | N/A |
| @check_strings | 3 | 3 | YES | 0 elided | 0 moves |
| @main | 0 | 0 | YES | N/A | N/A |

**Verdict**: All functions balanced. Zero leaks. The 3 rc_inc operations come from `ori_str_from_raw()` which implicitly allocates string data. The 3 rc_dec operations clean up each string after its `.length()` call. SSO gating correctly avoids rc_dec for strings that use small-string optimization (all 3 strings in this journey are short enough for SSO, so the rc_dec calls are skipped at runtime, but the codegen correctly generates the gating logic). [NOTE-1]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | noalias | uwtable | cold | Notes |
|----------|--------|----------|--------|---------|---------|------|-------|
| @bool_to_int | YES | YES | YES (ret+param) | N/A | YES | NO | |
| @check_logic | YES | YES | YES (ret) | N/A | YES | NO | |
| @check_strings | YES | NO | YES (ret) | N/A | YES | NO | [MEDIUM-2] |
| @main | NO | NO | YES (ret) | N/A | YES | NO | [LOW-1] |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | N/A | YES | |
| @ori_str_from_raw | N/A | YES | N/A | YES (sret) | N/A | NO | |
| @ori_str_len | N/A | YES | N/A | N/A | N/A | NO | |
| @_ori_drop$3 | N/A | YES | N/A | N/A | N/A | YES | |
| @ori_rc_dec | N/A | YES | N/A | N/A | N/A | NO | memory(inaccessiblemem: readwrite) |

**Attribute compliance**: 17/23 applicable checks correct (73.9%).

**Missing `nounwind` on @check_strings** [MEDIUM-2]: The nounwind analysis correctly determined that `@check_strings` can unwind (it calls `ori_panic_cstr` which is `noreturn` but the function has `uwtable` without `nounwind`). This is actually correct -- the function calls `ori_panic_cstr` on overflow which raises an exception. The nounwind analysis properly excludes it. However, the panic path uses `unreachable` after `ori_panic_cstr`, so technically the function cannot normally unwind -- it either returns normally or terminates via panic. This is a design choice rather than a bug.

**Missing `fastcc` on @main** [LOW-1]: Entry point uses C calling convention (required for ABI compatibility with the `main()` wrapper). This is correct behavior.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @bool_to_int | 1 | 0 | 0 | 0 | |
| @check_logic | 7 | 0 | 0 | 0 | |
| @check_strings | 13 | 0 | 4 | 0 | [MEDIUM-1] |
| @main | 3 | 0 | 0 | 0 | |

**@check_strings has 4 redundant branches**: The SSO-gated rc_dec pattern for each of the 3 strings produces a `br label %next_block` that could be eliminated by block merging. One additional redundant branch comes from the block transition between the first string's rc_dec completion and the second string's length call. These are codegen artifacts from the sequential SSO check + rc_dec + continue pattern.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (check_logic, 3x) | YES | YES | llvm.sadd.with.overflow.i64, chained |
| add (check_strings, 2x) | YES | YES | llvm.sadd.with.overflow.i64 for length sums |
| add (main, 1x) | YES | YES | llvm.sadd.with.overflow.i64 |

All 6 integer addition operations use `llvm.sadd.with.overflow.i64` with proper panic-on-overflow. No operations missed.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.33 MiB (debug) |
| .text section | 885 KiB |
| .rodata section | 134 KiB |
| User code (@bool_to_int) | 18 bytes (7 instructions) |
| User code (@check_logic) | 156 bytes |
| User code (@check_strings) | 632 bytes |
| User code (@main) | 60 bytes |
| User code total | ~866 bytes |
| Runtime | >99% of binary |

#### Disassembly: @bool_to_int

```asm
_ori_bool_to_int:
  mov    %dil,%dl          ; extract bool arg
  xor    %eax,%eax         ; rax = 0
  mov    $0x1,%ecx         ; rcx = 1
  test   $0x1,%dl          ; test bool
  cmovne %rcx,%rax         ; rax = b ? 1 : 0
  ret
```

Excellent: the `select i1, i64 1, i64 0` lowered to a branchless `cmovne`. 6 native instructions for a trivial bool-to-int conversion.

#### Disassembly: @main

```asm
_ori_main:
  sub    $0x18,%rsp
  call   _ori_check_logic
  mov    %rax,0x8(%rsp)
  call   _ori_check_strings
  mov    %rax,%rcx
  mov    0x8(%rsp),%rax
  add    %rcx,%rax
  jo     .overflow
  add    $0x18,%rsp
  ret
```

Clean: 2 calls, spill/restore, checked add. The `jo` traps overflow correctly.

### 7. Optimal IR Comparison

#### @bool_to_int: Ideal vs Actual

```llvm
; IDEAL (2 instructions)
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) nounwind {
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}
```

```llvm
; ACTUAL (2 instructions)
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) #0 {
bb0:
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}
```

**Delta**: 0 instructions. OPTIMAL.

#### @check_logic: Ideal vs Actual

```llvm
; IDEAL (23 instructions) — constant booleans folded, overflow checking required
define fastcc noundef i64 @_ori_check_logic() nounwind {
  ; 4 calls to @bool_to_int with constant args
  ; 3 overflow-checked adds with panic blocks
  ; All justified — no constant folding of the calls themselves
}
```

**Delta**: 0 unjustified instructions. OPTIMAL. The compiler correctly folds `true && true` to `true`, etc., at canonicalization, and passes the resulting constants to `@bool_to_int`.

#### @check_strings: Ideal vs Actual

```llvm
; IDEAL (83 instructions)
; Same structure but without 4 redundant unconditional branches
; in the SSO-gated rc_dec diamond patterns
```

```llvm
; ACTUAL (87 instructions)
; 4 extra `br label %next` instructions from SSO check merge points
```

**Delta**: +4 unjustified instructions (redundant branches in SSO rc_dec gating). [MEDIUM-1]

#### @main: Ideal vs Actual

```llvm
; IDEAL (9 instructions)
define noundef i64 @_ori_main() {
  %call = call fastcc i64 @_ori_check_logic()
  %call1 = call fastcc i64 @_ori_check_strings()
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
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

**Delta**: 0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @bool_to_int | 2 | 2 | +0 | N/A | OPTIMAL |
| @check_logic | 23 | 23 | +0 | N/A | OPTIMAL |
| @check_strings | 83 | 87 | +4 | NO (redundant br) | NEAR-OPTIMAL |
| @main | 9 | 9 | +0 | N/A | OPTIMAL |

### 8. Strings: SSO Gating Pattern

The SSO (Small String Optimization) gating in `@check_strings` is the most interesting codegen pattern in this journey. For each string's rc_dec, the compiler generates:

```llvm
; Extract data pointer from { i64, i64, ptr } string struct
%rc_dec.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2
; Test high bit (SSO flag)
%rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
%rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808   ; 0x8000000000000000
%rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
; Test null
%rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
%rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
; Skip if SSO or null
%rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap
```

This is correct and safe -- SSO strings store their data inline in the struct (no heap allocation), so calling `ori_rc_dec` on them would be incorrect. The pattern has a minor redundancy: `ptrtoint` is computed twice (once for SSO flag, once for null check) on the same pointer. LLVM's optimizer will CSE this away.

All 3 strings ("hello" = 5 bytes, "world!" = 6 bytes, "" = 0 bytes) are within SSO threshold (23 bytes), so at runtime the rc_dec calls are all skipped. The codegen correctly handles the general case nonetheless.

### 9. Strings: Construction Protocol

String literals are lowered through a two-stage protocol:

1. **Global constants**: `@str = private unnamed_addr constant [6 x i8] c"hello\00"` -- null-terminated C strings in .rodata
2. **Runtime construction**: `call void @ori_str_from_raw(ptr %sret, ptr @str, i64 5)` -- constructs an `OriStr` struct `{ i64, i64, ptr }` (len, capacity, data) via sret

The sret convention for `ori_str_from_raw` requires 6 alloca slots in `@check_strings` (3 for sret destinations, 3 for `ori_str_len` arguments). The field-by-field GEP+load+insertvalue sequence to materialize the `{ i64, i64, ptr }` aggregate is verbose but follows the LLVM codegen rule of never loading a >16B struct with a single `load %BigStruct, ptr` instruction (JIT compatibility).

### 10. Branching: Boolean Short-Circuit Folding

The `&&` and `||` operators on constant booleans are folded at canonicalization time:

- `true && true` -> `true` (passed as `i1 true` to `@bool_to_int`)
- `true && false` -> `false`
- `false || true` -> `true`
- `false || false` -> `false`

This is correct constant folding. The prelude's `@and`/`@or` functions (which desugar `&&`/`||` into `match` expressions) are never emitted -- they are resolved to constants. The generated IR calls `@bool_to_int` directly with `i1 true` or `i1 false` constant arguments.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM | Control Flow | 4 redundant unconditional branches in SSO rc_dec gating | CONFIRMED | J9 |
| 2 | MEDIUM | Attributes | Missing nounwind on @check_strings and @main | CONFIRMED | J1 |
| 3 | LOW | Attributes | Missing fastcc on @main (by design -- C ABI entry) | CONFIRMED | J1 |
| 4 | NOTE | ARC | All 3 strings perfectly balanced: 3 rc_inc, 3 rc_dec | NEW | J9 |
| 5 | NOTE | Codegen | Boolean constant folding eliminates all &&/|| runtime logic | NEW | J9 |
| 6 | NOTE | Codegen | select-based bool_to_int lowers to branchless cmovne | NEW | J9 |

### MEDIUM-1: Redundant unconditional branches in SSO rc_dec gating

**Location**: @check_strings, blocks bb1, bb3, and 2 SSO merge blocks
**Impact**: 4 unnecessary branch instructions (~4% of function)
**Fix**: Block merging pass to eliminate `br label %next` when blocks can be combined
**First seen**: Journey 9
**Found in**: Control Flow & Block Layout (Category 4), Instruction Purity (Category 1)

### MEDIUM-2: Missing nounwind on @check_strings and @main

**Location**: @check_strings and @_ori_main function declarations
**Impact**: LLVM generates exception handling tables (uwtable without nounwind)
**Fix**: The nounwind analysis correctly excludes these because they call `ori_panic_cstr`. However, since `ori_panic_cstr` is `cold noreturn`, these functions cannot normally unwind -- they either return or terminate. A more precise analysis could mark them `nounwind` since `noreturn` functions do not propagate exceptions back to the caller.
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-1: Missing fastcc on @main

**Location**: @_ori_main function declaration
**Impact**: Uses C calling convention instead of fastcc
**Fix**: By design -- entry point must use C ABI for compatibility with the `main()` wrapper
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-1: Perfect ARC balance on strings

**Location**: @check_strings
**Impact**: Positive -- 3 strings allocated, 3 cleaned up, zero leaks
**Found in**: ARC Purity (Category 2)

### NOTE-2: Boolean constant folding

**Location**: @check_logic
**Impact**: Positive -- `&&`/`||` on constant bools folded away at canonicalization
**Found in**: Optimal IR Comparison (Category 7)

### NOTE-3: Branchless bool_to_int

**Location**: @bool_to_int native disassembly
**Impact**: Positive -- `select` lowers to `cmovne`, no branch prediction penalty
**Found in**: Binary Analysis (Category 6)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.03x avg ratio (max 1.05x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 6/10 | 73.9% compliance |
| Control Flow | 10% | 7/10 | 4 defects |
| IR Quality | 20% | 8/10 | 4 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 8.8 / 10**

## Verdict

Journey 9 demonstrates strong string codegen with a significant improvement from the previous run. ARC is now perfectly balanced at 10/10 (up from 3/10) thanks to fixed tooling that correctly accounts for implicit allocations by `ori_str_from_raw` and excludes drop glue from user function analysis. The SSO gating pattern for rc_dec is correct and safe, though it introduces 4 redundant branches that keep control flow at 7/10. Boolean constant folding and branchless `select` lowering are highlights. The main remaining weakness is attribute compliance (73.9%), primarily due to the nounwind analysis conservatively excluding functions that call `noreturn` panic functions.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J9 | CONFIRMED |
| fastcc usage | J1 | J9 | CONFIRMED |
| Missing nounwind on callers of panic | J1 | J9 | CONFIRMED |
| Boolean constant folding | J2 | J9 | CONFIRMED |
| select lowering to cmovne | J2 | J9 | CONFIRMED |
| SSO gating for ARC | J9 | J9 | NEW |
| String construction protocol | J9 | J9 | NEW |

The SSO gating pattern is unique to string-handling journeys and will likely appear again in J10+ when strings are used within collections. The redundant branch pattern in SSO diamond merges matches the same codegen pattern seen in J2's branching overhead, suggesting a common block-merging optimization opportunity.
