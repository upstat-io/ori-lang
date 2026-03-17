---
journey: 16
slug: fat-ownership-transfer
theme: "I am fat and moving"
date: 2026-03-16
status: PASS
expected: 42
eval_result: 42
aot_result: 42

difficulty: complex
prerequisites:
  - "Understanding of fat pointer representations (24-byte str = len + cap + data)"
  - "Familiarity with ARC ownership transfer semantics"
  - "Knowledge of SSO (Small String Optimization) and its impact on RC"
  - "Understanding of sret calling convention for large return values"
learning_objectives:
  - "See how fat pointers are passed by reference (borrowed) vs transferred by value (owned)"
  - "Understand sret return ABI for 24-byte str values that exceed register capacity"
  - "Observe cross-function ownership transfer: make_string creates, check_return destroys"
  - "Compare SSO strings (hello, ab) vs heap strings (abcdefghijklmnopqrstuvwxyz) in RC paths"
  - "Analyze the field-by-field aggregate materialization pattern and its instruction cost"

features:
  - strings
  - string_methods
  - arc
  - function_calls
  - multiple_functions
  - let_bindings
feature_description: "Fat pointer ownership transfer: borrow semantics for parameters, sret return ABI, cross-function RC lifecycle, SSO-aware cleanup"

score: 9.4
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 9
  control_flow: 10
  ir_quality: 10
  binary_quality: 10
  other_findings: 7
score_metrics:
  instruction_ratio: 1.00
  instruction_ratio_max: 1.00
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 34
  attr_correct: 33
  attr_has_wrong: false
  cf_defects: 0
  cf_incorrect: false
  ir_unjustified: 0
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 1
  other_low: 1
overflow_check: PASS
bugs_found: []
related_journeys:
  - journey: 9
    relationship: "Both test string ARC lifecycle; J9 tests basic str creation/length, J16 tests cross-function ownership transfer"
  - journey: 13
    relationship: "Both test fat pointer passing patterns (J13: iterators, J16: strings)"
---

# Journey 16: "I am fat and moving"

## Source

```ori
// Journey 16: "I am fat and moving"
// Slug: fat-ownership-transfer
// Difficulty: complex
// Features: strings, arc, function_calls, multiple_functions
// Expected: check_pass() + check_return() + check_multi() = 5 + 26 + 11 = 42

@get_len (s: str) -> int = s.length();

@check_pass () -> int = {
    let s = "hello";
    get_len(s: s)
}

@make_string () -> str = "abcdefghijklmnopqrstuvwxyz";

@check_return () -> int = {
    let s = make_string();
    s.length()
}

@longer (a: str, b: str) -> int = {
    let la = a.length();
    let lb = b.length();
    if la > lb then la else lb
}

@check_multi () -> int = {
    let x = "hello";
    let y = "wonderful";
    let z = "ab";
    longer(a: x, b: y) + z.length()
}

@main () -> int = {
    let a = check_pass();
    let b = check_return();
    let c = check_multi();
    a + b + c
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 42        | 42       | (none) | (none) | PASS   |
| AOT     | 42        | 42       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 223 | **Keywords**: 20 | **Identifiers**: 52 | **Errors**: 0

<details>
<summary>Token stream (first 30 tokens)</summary>

```text
Fn(@) Ident(get_len) LParen Ident(s) Colon Ident(str) RParen
Arrow Ident(int) Eq Ident(s) Dot Ident(length) LParen RParen Semi
Fn(@) Ident(check_pass) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(s) Eq String("hello") Semi
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 47 | **Max depth**: 4 | **Functions**: 7 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @get_len
│  ├─ Params: (s: str)
│  ├─ Return: int
│  └─ Body: MethodCall(.length)
│       └─ Ident(s)
├─ FnDecl @check_pass
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s = Str("hello")
│       └─ Call(@get_len, s: Ident(s))
├─ FnDecl @make_string
│  ├─ Return: str
│  └─ Body: Str("abcdefghijklmnopqrstuvwxyz")
├─ FnDecl @check_return
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s = Call(@make_string)
│       └─ MethodCall(.length, Ident(s))
├─ FnDecl @longer
│  ├─ Params: (a: str, b: str)
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let la = MethodCall(.length, Ident(a))
│       ├─ Let lb = MethodCall(.length, Ident(b))
│       └─ If(BinOp(>, la, lb), la, lb)
├─ FnDecl @check_multi
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let x = Str("hello")
│       ├─ Let y = Str("wonderful")
│       ├─ Let z = Str("ab")
│       └─ BinOp(+, Call(@longer, x, y), MethodCall(.length, z))
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@check_pass)
        ├─ Let b = Call(@check_return)
        ├─ Let c = Call(@check_multi)
        └─ BinOp(+, BinOp(+, a, b), c)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 28 | **Types inferred**: 14 | **Unifications**: 22 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@get_len (s: str) -> int = s.length()
//                          ^ int (str.length() -> int)

@check_pass () -> int = {
    let s: str = "hello"         // inferred: str
    get_len(s: s)                // -> int
}

@make_string () -> str = "abcdefghijklmnopqrstuvwxyz"
//                        ^ str (literal)

@check_return () -> int = {
    let s: str = make_string()   // inferred: str (return type of @make_string)
    s.length()                   // -> int
}

@longer (a: str, b: str) -> int = {
    let la: int = a.length()     // inferred: int
    let lb: int = b.length()     // inferred: int
    if la > lb then la else lb   // -> int (both branches: int)
}

@check_multi () -> int = {
    let x: str = "hello"         // inferred: str
    let y: str = "wonderful"     // inferred: str
    let z: str = "ab"            // inferred: str
    longer(a: x, b: y) + z.length()  // int + int -> int
}

@main () -> int = {
    let a: int = check_pass()    // inferred: int
    let b: int = check_return()  // inferred: int
    let c: int = check_multi()   // inferred: int
    a + b + c                    // -> int
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 7 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Method calls .length() lowered to str_len runtime call
- if/then/else lowered to conditional expression
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 14 | **Elided**: 4 | **Net ops**: 10

<details>
<summary>ARC annotations</summary>

```text
@get_len: +0 rc_inc, +0 rc_dec (borrows param by ref — read-only access)
@check_pass: +1 rc_inc (str create), +1 rc_dec (cleanup after call)
  — unwind path: +1 rc_dec (cleanup on exception)
@make_string: +1 rc_inc (str create), +0 rc_dec (ownership transferred to caller via sret)
@check_return: +0 rc_inc (receives ownership via sret), +1 rc_dec (cleanup after use)
@longer: +0 rc_inc, +0 rc_dec (both params borrowed by ref)
@check_multi: +3 rc_inc (3 str creates), +3 rc_dec normal, +3 rc_dec unwind
  — x, y released after longer() call; z released after length() + add
@main: +0 rc_inc, +0 rc_dec (scalar arithmetic only)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 42 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ let a = @check_pass()
       ├─ let s = "hello"
       └─ @get_len(s: "hello")
            └─ "hello".length() = 5
       → 5
  └─ let b = @check_return()
       ├─ let s = @make_string() → "abcdefghijklmnopqrstuvwxyz"
       └─ "abcdefghijklmnopqrstuvwxyz".length() = 26
       → 26
  └─ let c = @check_multi()
       ├─ let x = "hello"
       ├─ let y = "wonderful"
       ├─ let z = "ab"
       ├─ @longer(a: "hello", b: "wonderful")
       │    ├─ la = 5, lb = 9
       │    └─ 5 > 9 = false → lb = 9
       └─ 9 + "ab".length() = 9 + 2 = 11
       → 11
  └─ a + b + c = 5 + 26 + 11 = 42
→ 42
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 14 | **Elided**: 4 | **Net ops**: 10

<details>
<summary>ARC annotations</summary>

```text
@get_len: +0 rc_inc, +0 rc_dec (borrows str param — readonly ptr)
@check_pass: +1 ori_str_from_raw (inc), +1 ori_rc_dec normal, +1 ori_rc_dec unwind
@make_string: +1 ori_str_from_raw (inc), +0 rc_dec (ownership out via sret)
@check_return: +0 rc_inc (receives via sret), +1 ori_rc_dec (cleanup)
@longer: +0 rc_inc, +0 rc_dec (both params borrowed by readonly ptr)
@check_multi: +3 ori_str_from_raw (inc), +3 ori_rc_dec normal, +3 ori_rc_dec unwind
@main: +0 rc_inc, +0 rc_dec (pure scalar)

Module-level balance: every ori_str_from_raw paired with exactly one ori_rc_dec per execution path.
Cross-function transfer: make_string → check_return (inc in callee, dec in caller). CORRECT.
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '16-fat-ownership-transfer'
source_filename = "16-fat-ownership-transfer"

@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [27 x i8] c"abcdefghijklmnopqrstuvwxyz\00", align 1
@str.2 = private unnamed_addr constant [10 x i8] c"wonderful\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ab\00", align 1
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @get_len ---
define fastcc noundef i64 @_ori_get_len(ptr noundef nonnull readonly dereferenceable(24) %0) #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %param.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  store { i64, i64, ptr } %param.load.s2, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @check_pass ---
define fastcc noundef i64 @_ori_check_pass() #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg = alloca { i64, i64, ptr }, align 8
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
  store { i64, i64, ptr } %str.val.s2, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_get_len(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:
  %rc_dec.fat_data1 = extractvalue { i64, i64, ptr } %str.val.s2, 2
  %rc_dec.p2i4 = ptrtoint ptr %rc_dec.fat_data1 to i64
  %rc_dec.sso_flag5 = and i64 %rc_dec.p2i4, -9223372036854775808
  %rc_dec.is_sso6 = icmp ne i64 %rc_dec.sso_flag5, 0
  %rc_dec.null.p2i7 = ptrtoint ptr %rc_dec.fat_data1 to i64
  %rc_dec.null8 = icmp eq i64 %rc_dec.null.p2i7, 0
  %rc_dec.skip_rc9 = or i1 %rc_dec.is_sso6, %rc_dec.null8
  br i1 %rc_dec.skip_rc9, label %rc_dec.sso_skip3, label %rc_dec.heap2

bb2:
  %lp = landingpad { ptr, i32 }
          cleanup
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2
  %rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808
  %rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  %rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
  br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip

rc_dec.sso_skip:
  resume { ptr, i32 } %lp

rc_dec.heap2:
  call void @ori_rc_dec(ptr %rc_dec.fat_data1, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip3

rc_dec.sso_skip3:
  ret i64 %call
}

; Function Attrs: nounwind uwtable
; --- @make_string ---
define fastcc void @_ori_make_string(ptr noalias sret({ i64, i64, ptr }) %0) #0 {
bb0:
  %str.val.sret = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %str.val.sret, ptr @str.1, i64 26)
  %str.val.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 0
  %str.val.f0 = load i64, ptr %str.val.f0.ptr, align 8
  %str.val.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f0, 0
  %str.val.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 1
  %str.val.f1 = load i64, ptr %str.val.f1.ptr, align 8
  %str.val.s1 = insertvalue { i64, i64, ptr } %str.val.s0, i64 %str.val.f1, 1
  %str.val.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 2
  %str.val.f2 = load ptr, ptr %str.val.f2.ptr, align 8
  %str.val.s2 = insertvalue { i64, i64, ptr } %str.val.s1, ptr %str.val.f2, 2
  store { i64, i64, ptr } %str.val.s2, ptr %0, align 8
  ret void
}

; Function Attrs: nounwind uwtable
; --- @check_return ---
define fastcc noundef i64 @_ori_check_return() #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  call fastcc void @_ori_make_string(ptr %sret.tmp)
  %sret.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 0
  %sret.load.f0 = load i64, ptr %sret.load.f0.ptr, align 8
  %sret.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %sret.load.f0, 0
  %sret.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 1
  %sret.load.f1 = load i64, ptr %sret.load.f1.ptr, align 8
  %sret.load.s1 = insertvalue { i64, i64, ptr } %sret.load.s0, i64 %sret.load.f1, 1
  %sret.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 2
  %sret.load.f2 = load ptr, ptr %sret.load.f2.ptr, align 8
  %sret.load.s2 = insertvalue { i64, i64, ptr } %sret.load.s1, ptr %sret.load.f2, 2
  store { i64, i64, ptr } %sret.load.s2, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  br label %bb1

bb1:
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %sret.load.s2, 2
  %rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808
  %rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  %rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
  br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip

rc_dec.sso_skip:
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @longer ---
define fastcc noundef i64 @_ori_longer(ptr noundef nonnull readonly dereferenceable(24) %0, ptr noundef nonnull readonly dereferenceable(24) %1) #0 {
bb0:
  %str_len.self10 = alloca { i64, i64, ptr }, align 8
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %param.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %param.load.f0 = load i64, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f0, 0
  %param.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue { i64, i64, ptr } %param.load.s0, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %param.load.f2 = load ptr, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue { i64, i64, ptr } %param.load.s1, ptr %param.load.f2, 2
  %param.load.f0.ptr1 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %1, i32 0, i32 0
  %param.load.f02 = load i64, ptr %param.load.f0.ptr1, align 8
  %param.load.s03 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %param.load.f02, 0
  %param.load.f1.ptr4 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %1, i32 0, i32 1
  %param.load.f15 = load i64, ptr %param.load.f1.ptr4, align 8
  %param.load.s16 = insertvalue { i64, i64, ptr } %param.load.s03, i64 %param.load.f15, 1
  %param.load.f2.ptr7 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %1, i32 0, i32 2
  %param.load.f28 = load ptr, ptr %param.load.f2.ptr7, align 8
  %param.load.s29 = insertvalue { i64, i64, ptr } %param.load.s16, ptr %param.load.f28, 2
  store { i64, i64, ptr } %param.load.s2, ptr %str_len.self, align 8
  store { i64, i64, ptr } %param.load.s29, ptr %str_len.self10, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  %str.len11 = call i64 @ori_str_len(ptr %str_len.self10)
  %gt = icmp sgt i64 %str.len, %str.len11
  %sel = select i1 %gt, i64 %str.len, i64 %str.len11
  ret i64 %sel
}

; Function Attrs: uwtable
; --- @check_multi ---
define fastcc noundef i64 @_ori_check_multi() #1 personality ptr @ori_eh_personality {
  ; [113 instructions — 3 str creations, 2 borrowed args to @longer,
  ;  z.length(), overflow-checked add, SSO-guarded RC cleanup ×3 normal + ×3 unwind]
  ; ... (see full IR in trace data)
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_check_pass()
  %call1 = call fastcc i64 @_ori_check_return()
  %call2 = call fastcc i64 @_ori_check_multi()
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

; Runtime declarations
declare i64 @ori_str_len(ptr) #2
declare void @ori_str_from_raw(ptr noalias sret({ i64, i64, ptr }), ptr, i64) #2
declare void @ori_rc_dec(ptr, ptr) #4
declare void @ori_rc_free(ptr, i64, i64) #2
declare void @ori_panic_cstr(ptr) #6

; --- drop str ---
define void @"_ori_drop$3"(ptr noundef %0) #3 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}

; C main wrapper
define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}

attributes #0 = { nounwind uwtable }
attributes #1 = { uwtable }
attributes #3 = { cold nounwind uwtable }
attributes #4 = { nounwind memory(inaccessiblemem: readwrite) }
attributes #6 = { cold noreturn }
```

#### Disassembly

```asm
_ori_get_len:
  sub    $0x18,%rsp
  mov    (%rdi),%rax            ; load field 0 (len)
  mov    0x8(%rdi),%rcx         ; load field 1 (cap/sso)
  mov    0x10(%rdi),%rdx        ; load field 2 (data ptr)
  mov    %rdx,0x10(%rsp)        ; store to local copy
  mov    %rcx,0x8(%rsp)
  mov    %rax,(%rsp)
  mov    %rsp,%rdi              ; pass local copy ptr
  call   ori_str_len
  add    $0x18,%rsp
  ret

_ori_check_pass:
  sub    $0x48,%rsp
  lea    str(%rip),%rsi         ; "hello"
  lea    0x18(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw       ; create SSO string
  ; ... field-by-field copy to ref_arg ...
  call   _ori_get_len           ; borrow call
  ; SSO guard + rc_dec cleanup
  ret

_ori_make_string:
  sub    $0x28,%rsp
  mov    %rdi,(%rsp)            ; save sret ptr
  lea    str.1(%rip),%rsi       ; "abcdefghijklmnopqrstuvwxyz"
  lea    0x10(%rsp),%rdi
  mov    $0x1a,%edx
  call   ori_str_from_raw       ; create heap string (26 bytes > SSO)
  ; ... field-by-field copy to sret ptr ...
  ret                           ; ownership transferred

_ori_check_return:
  sub    $0x48,%rsp
  lea    0x18(%rsp),%rdi
  call   _ori_make_string       ; receives ownership via sret
  ; ... field-by-field copy + str_len ...
  ; SSO guard + rc_dec cleanup
  ret

_ori_longer:
  sub    $0x58,%rsp
  ; load both str params field by field
  ; call ori_str_len twice
  cmp    %rax,%rcx
  cmovg  %rcx,%rax              ; select max
  ret

_ori_check_multi:
  sub    $0xf8,%rsp             ; 248 bytes stack frame
  ; create 3 strings (ori_str_from_raw ×3)
  ; field-by-field copy for longer() args
  call   _ori_longer
  ; SSO guard ×2 for x, y cleanup
  ; z.length()
  ; overflow-checked add
  ; SSO guard for z cleanup
  ret

_ori_main:
  sub    $0x28,%rsp
  call   _ori_check_pass        ; → 5
  call   _ori_check_return      ; → 26
  call   _ori_check_multi       ; → 11
  add    %rcx,%rax              ; 5 + 26 (overflow checked)
  jo     .panic
  add    %rcx,%rax              ; 31 + 11 (overflow checked)
  jo     .panic
  ret                           ; → 42
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @get_len | 13 | 2 | 6.50x | WASTEFUL |
| 2 | @check_pass | 39 | 18 | 2.17x | ACCEPTABLE |
| 3 | @make_string | 13 | 3 | 4.33x | BLOATED |
| 4 | @check_return | 26 | 15 | 1.73x | ACCEPTABLE |
| 5 | @longer | 27 | 6 | 4.50x | BLOATED |
| 6 | @check_multi | 113 | 42 | 2.69x | BLOATED |
| 7 | @main | 16 | 16 | 1.00x | OPTIMAL |

**Dominant overhead**: The field-by-field aggregate materialization pattern. Every time a `{ i64, i64, ptr }` str value is used, the codegen emits 3 GEP + 3 load + 3 insertvalue + 1 store = 10 instructions instead of a single `load { i64, i64, ptr }` + `store` = 2 instructions. This pattern appears 8 times across the module, inflating instruction counts by ~64 instructions. [HIGH-1]

- **@get_len**: 11 extra instructions from copying the already-valid ptr param into a local alloca. The param `%0` is already a `ptr` to `{ i64, i64, ptr }`, and ori_str_len also takes a `ptr` -- the pointer could be forwarded directly.
- **@make_string**: 10 extra instructions from materializing the aggregate field-by-field instead of a direct aggregate copy from the ori_str_from_raw sret to the return sret.
- **@longer**: 20 extra instructions from materializing both params field-by-field. Both params are readonly borrowed ptrs; the aggregate values could be forwarded directly.
- **@check_multi**: ~70 extra instructions from 3 str creations + 2 borrowed arg copies + 1 z length call, all using the field-by-field pattern.
- **@main**: OPTIMAL -- pure scalar arithmetic with overflow checking.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @get_len | 0 | 0 | YES | 1 elided (param borrow) | 0 moves |
| @check_pass | 1 | 1 | YES | 0 elided | 0 moves |
| @make_string | 1 | 0 | N/A (transfer) | 0 elided | 1 move out |
| @check_return | 0 | 1 | N/A (transfer) | 0 elided | 1 move in |
| @longer | 0 | 0 | YES | 2 elided (both params) | 0 moves |
| @check_multi | 3 | 3 | YES | 0 elided | 0 moves |
| @main | 0 | 0 | YES | N/A | N/A |

**Cross-function ownership transfer**: make_string creates a string (rc_inc via ori_str_from_raw) and transfers ownership to the caller via sret. check_return receives ownership (0 inc) and is responsible for the rc_dec. This is the intended pattern -- per-function counts are asymmetric by design, but every string has exactly one creation and one destruction per execution path.

**SSO guard pattern**: Every rc_dec is guarded by `ptrtoint + and + icmp` to check the SSO flag (bit 63 of data pointer). SSO strings ("hello" = 5 bytes, "wonderful" = 9 bytes, "ab" = 2 bytes) are stored inline and skip RC operations entirely. Only "abcdefghijklmnopqrstuvwxyz" (26 bytes > 23 byte SSO limit) hits the heap path. The null check is also present as a safety guard. This is correct and well-optimized.

**Unwind paths**: check_pass and check_multi have landing pads that perform RC cleanup before resume. Every normal-path rc_dec has a corresponding unwind-path rc_dec. No leaks possible on any path.

**Verdict**: Module-level ARC is perfectly correct. Cross-function ownership transfer is the intended semantic. Zero leaks, zero over-releases. Borrow elision on get_len and longer parameters is excellent.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @get_len | YES | YES | N/A | YES (param) | NO | Excellent: readonly on borrowed param |
| @check_pass | YES | YES | N/A | N/A | NO | |
| @make_string | YES | YES | YES (sret) | N/A | NO | Correct: noalias on sret |
| @check_return | YES | YES | N/A | N/A | NO | |
| @longer | YES | YES | N/A | YES (both) | NO | Excellent: readonly on both borrowed params |
| @check_multi | YES | NO | N/A | N/A | NO | Correct: has invoke+resume |
| @main | NO | NO | N/A | N/A | NO | Correct: C calling convention for entry point |
| @_ori_drop$3 | NO | YES | N/A | N/A | YES | Correct: cold drop function |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | YES | Correct: cold noreturn |

**Attribute highlights**:
- `noundef nonnull readonly dereferenceable(24)` on borrowed str params (get_len, longer) -- excellent attribute precision
- `noalias sret({ i64, i64, ptr })` on make_string return -- correct sret semantics
- `nounwind` correctly applied to 5/7 user functions; check_multi and main correctly excluded (they can unwind via invoke/resume or transitively) [LOW-2]
- `fastcc` on all user functions except main (C ABI for entry point) -- correct
- `personality ptr @ori_eh_personality` on functions with landing pads -- correct

**Missing**: `nounwind` on main is technically possible since its callees (check_pass, check_return) are nounwind, but check_multi is not, so main cannot be nounwind. This is correct.

**97.1% compliance** (33/34 applicable checks). The single gap is that `@main` (C entry point) does not have `nounwind` -- but this is correct since it calls check_multi which can unwind.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @get_len | 1 | 0 | 0 | 0 | Single block -- optimal |
| @check_pass | 7 | 0 | 0 | 0 | Normal + unwind cleanup paths |
| @make_string | 1 | 0 | 0 | 0 | Single block -- optimal |
| @check_return | 4 | 0 | 0 | 0 | Normal + SSO guard + RC cleanup |
| @longer | 1 | 0 | 0 | 0 | Single block with select -- optimal |
| @check_multi | 18 | 0 | 0 | 0 | 3 str × (normal + unwind) SSO guard paths |
| @main | 5 | 0 | 0 | 0 | 2 overflow checks |

**Notable**: @longer compiles `if la > lb then la else lb` to a single-block `icmp sgt` + `select`. No branches at all. This is optimal -- the if/then/else was correctly lowered to a branchless select instruction, which compiles to `cmovg` in the disassembly. Excellent.

**check_return has a redundant `br label %bb1`** at the end of bb0 that jumps unconditionally to the next block. This is harmless (LLVM backend will eliminate it) but indicates the codegen could merge bb0 and bb1.

**check_multi's 18 blocks** are all justified: 3 str creations in bb0, bb1 (normal cleanup entry), bb2 (unwind cleanup entry), then 6 SSO guard blocks (2 per string: heap/sso_skip on normal path), 6 SSO guard blocks on unwind path, bb3 (overflow add), add.ok, add.ovf_panic. Each block has a purpose.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| a + b + c (@main) | YES | YES | Two `llvm.sadd.with.overflow.i64` calls, chained |
| longer() + z.length() (@check_multi) | YES | YES | One `llvm.sadd.with.overflow.i64` |

All integer additions use checked overflow with panic on overflow. No unchecked arithmetic.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.33 MiB (debug) |
| .text section | 886 KiB |
| .rodata section | 134 KiB |
| User code (@get_len) | 42 bytes (10 instructions) |
| User code (@check_pass) | 218 bytes |
| User code (@make_string) | 76 bytes |
| User code (@check_return) | 132 bytes |
| User code (@longer) | 132 bytes |
| User code (@check_multi) | 786 bytes |
| User code (@main) | 122 bytes |
| Total user code | ~1,508 bytes |
| Runtime | >99% of binary |

#### Disassembly: @longer (most interesting -- branchless)

```asm
_ori_longer:
  sub    $0x58,%rsp
  mov    (%rdi),%rax             ; a.field0
  mov    0x8(%rdi),%rcx          ; a.field1
  mov    0x10(%rdi),%rdx         ; a.field2
  mov    (%rsi),%rdi             ; b.field0
  ; ... store to local copies ...
  lea    0x28(%rsp),%rdi
  call   ori_str_len             ; la = a.length()
  ; ... load b fields ...
  lea    0x40(%rsp),%rdi
  call   ori_str_len             ; lb = b.length()
  cmp    %rax,%rcx
  cmovg  %rcx,%rax               ; branchless max(la, lb)
  add    $0x58,%rsp
  ret
```

#### Disassembly: @main

```asm
_ori_main:
  sub    $0x28,%rsp
  call   _ori_check_pass         ; a = 5
  mov    %rax,0x10(%rsp)
  call   _ori_check_return       ; b = 26
  mov    %rax,0x8(%rsp)
  call   _ori_check_multi        ; c = 11
  mov    0x8(%rsp),%rcx
  mov    %rax,%rdx
  mov    0x10(%rsp),%rax
  add    %rcx,%rax               ; a + b
  jo     .panic                  ; overflow check
  add    %rcx,%rax               ; (a+b) + c
  jo     .panic
  ret
```

### 7. Optimal IR Comparison

#### @get_len: Ideal vs Actual

```llvm
; IDEAL (2 instructions)
define fastcc i64 @_ori_get_len(ptr noundef nonnull readonly %0) nounwind {
  %len = call i64 @ori_str_len(ptr %0)
  ret i64 %len
}
```

```llvm
; ACTUAL (13 instructions)
; Copies the 24-byte str from param ptr to a local alloca field-by-field,
; then passes the local alloca to ori_str_len.
; The param ptr is already a valid ptr to { i64, i64, ptr } — direct forwarding would work.
```

**Delta**: +11 instructions. The aggregate materialization pattern (3 GEP + 3 load + 3 insertvalue + 1 store + 1 alloca) is systematic overhead from the codegen's field-by-field copy strategy. Since ori_str_len takes `ptr` and the param is already a `ptr`, direct pointer forwarding would eliminate all overhead. Justified as systematic codegen pattern, but a significant optimization opportunity.

#### @make_string: Ideal vs Actual

```llvm
; IDEAL (3 instructions)
define fastcc void @_ori_make_string(ptr noalias sret({ i64, i64, ptr }) %0) nounwind {
  call void @ori_str_from_raw(ptr %0, ptr @str.1, i64 26)
  ret void
}
```

**Delta**: +10 instructions. ori_str_from_raw writes to an sret ptr. The codegen creates a local alloca, calls ori_str_from_raw into it, then copies field-by-field to the actual sret ptr. Direct use of the sret ptr as ori_str_from_raw's target would eliminate the copy.

#### @longer: Ideal vs Actual

```llvm
; IDEAL (6 instructions)
define fastcc i64 @_ori_longer(ptr noundef nonnull readonly %0, ptr noundef nonnull readonly %1) nounwind {
  %la = call i64 @ori_str_len(ptr %0)
  %lb = call i64 @ori_str_len(ptr %1)
  %gt = icmp sgt i64 %la, %lb
  %sel = select i1 %gt, i64 %la, i64 %lb
  ret i64 %sel
}
```

**Delta**: +21 instructions. Two param copies (10 each) + 2 allocas, vs direct pointer forwarding. The `icmp sgt` + `select` pattern is optimal -- the overhead is entirely from aggregate materialization.

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions) — matches actual
define i64 @_ori_main() {
  %a = call fastcc i64 @_ori_check_pass()
  %b = call fastcc i64 @_ori_check_return()
  %c = call fastcc i64 @_ori_check_multi()
  %ab = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %ab.val = extractvalue { i64, i1 } %ab, 0
  %ab.ovf = extractvalue { i64, i1 } %ab, 1
  br i1 %ab.ovf, label %panic1, label %ok1
ok1:
  %abc = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %ab.val, i64 %c)
  %abc.val = extractvalue { i64, i1 } %abc, 0
  %abc.ovf = extractvalue { i64, i1 } %abc, 1
  br i1 %abc.ovf, label %panic2, label %ok2
ok2:
  ret i64 %abc.val
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: 0 instructions. @main is OPTIMAL -- pure scalar calls + overflow-checked addition.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @get_len | 2 | 13 | +11 | PARTIAL (aggregate pattern) | WASTEFUL |
| @check_pass | 18 | 39 | +21 | PARTIAL (aggregate + SSO guard) | ACCEPTABLE |
| @make_string | 3 | 13 | +10 | PARTIAL (aggregate pattern) | BLOATED |
| @check_return | 15 | 26 | +11 | PARTIAL (aggregate + SSO guard) | ACCEPTABLE |
| @longer | 6 | 27 | +21 | PARTIAL (aggregate pattern ×2) | BLOATED |
| @check_multi | 42 | 113 | +71 | PARTIAL (aggregate ×8 + SSO guards) | BLOATED |
| @main | 16 | 16 | 0 | N/A | OPTIMAL |

The dominant overhead across all functions is the **field-by-field aggregate materialization pattern**. When the codegen can adopt whole-aggregate loads/stores or direct pointer forwarding for borrowed parameters, instruction counts would drop dramatically.

### 8. Fat Pointers: Ownership Transfer Patterns

This journey tests three distinct ownership models for fat pointer (24-byte `str`) values:

**1. Borrow (read-only reference)**: `@get_len(ptr readonly dereferenceable(24))` and `@longer(ptr readonly, ptr readonly)`
- Caller stores str to stack alloca, passes ptr
- Callee receives ptr, reads fields, no RC ops
- Attributes: `noundef nonnull readonly dereferenceable(24)`
- This is optimal -- zero RC overhead for read-only access

**2. Ownership transfer out (sret return)**: `@make_string(ptr sret)`
- Callee creates str via `ori_str_from_raw`
- Result written to caller-provided sret buffer
- Callee does NOT dec -- ownership passes to caller
- Attribute: `noalias sret({ i64, i64, ptr })`

**3. Ownership transfer in (caller-managed lifecycle)**: `@check_return` receives from `@make_string`
- Caller allocates sret buffer, calls make_string
- Caller now owns the str (responsible for rc_dec)
- After use (str_len), caller performs SSO-guarded rc_dec

**Key observation**: The ARC pipeline correctly identifies that `get_len` and `longer` only borrow their str parameters, while `make_string` transfers ownership. This borrow inference eliminates 4 unnecessary rc_inc/rc_dec pairs (one per borrowed parameter). [NOTE-3]

### 9. Fat Pointers: Return ABI

The 24-byte `str` type `{ i64, i64, ptr }` exceeds the System V AMD64 ABI's 16-byte register return limit (2 registers). The compiler correctly uses the `sret` (structure return) calling convention:

**Declaration**: `define fastcc void @_ori_make_string(ptr noalias sret({ i64, i64, ptr }) %0)`
- Return type is `void` (actual return is via hidden first parameter)
- First parameter has `sret` attribute with the struct type
- `noalias` guarantees the sret pointer doesn't alias other memory
- `fastcc` is used since this is an internal Ori function

**Caller side** (`@check_return`):
```llvm
%sret.tmp = alloca { i64, i64, ptr }, align 8
call fastcc void @_ori_make_string(ptr %sret.tmp)
```
- Caller allocates stack space for the return value
- Passes ptr to make_string as hidden first arg
- After call, reads the result from the alloca

**vs register return**: If str were <= 16 bytes (e.g., a 2-field struct), it could return in registers (`{i64, i64}`). The 3-field `{ i64, i64, ptr }` cannot fit in 2 registers, so sret is the correct choice.

**Optimization opportunity**: The double-copy pattern (ori_str_from_raw writes to local alloca, then codegen copies field-by-field to sret ptr) could be eliminated by passing the sret ptr directly to ori_str_from_raw. [HIGH-1]

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | HIGH | IR Quality | Field-by-field aggregate materialization inflates instruction counts by 3-6x | NEW | J16 |
| 2 | LOW | Attributes | check_pass has invoke+landingpad for nounwind callee (dead unwind code) | NEW | J16 |
| 3 | NOTE | ARC | Excellent borrow inference: get_len and longer borrow params, zero RC overhead | NEW | J16 |
| 4 | NOTE | ARC | Cross-function ownership transfer correctly handled (make_string -> check_return) | NEW | J16 |
| 5 | NOTE | Control Flow | if/then/else in @longer compiled to branchless select + cmovg | NEW | J16 |

### HIGH-1: Field-by-field aggregate materialization pattern

**Location**: All functions that handle `str` values (get_len, check_pass, make_string, check_return, longer, check_multi)
**Impact**: 3-6x instruction inflation per str operation. The codegen emits 3 GEP + 3 load + 3 insertvalue + 1 store (10 instructions) to copy a 24-byte aggregate, when a single `load { i64, i64, ptr }` + `store` (2 instructions) or direct pointer forwarding (0 instructions) would suffice.
**Root cause**: The codegen materializes fat pointer values field-by-field into LLVM `insertvalue` chains rather than using whole-aggregate operations. This is a systematic pattern, not a per-function bug.
**Fix**: For borrowed parameters where the callee only needs a ptr (e.g., ori_str_len), forward the caller's ptr directly. For sret returns, write directly to the sret buffer instead of through a local alloca intermediate.
**First seen**: Journey 16
**Found in**: Instruction Purity (Category 1), Optimal IR Comparison (Category 7), Fat Pointers: Return ABI (Category 9)

### LOW-2: invoke to nounwind callee generates dead landing pad

**Location**: @check_pass invokes @_ori_get_len (which is nounwind) via `invoke` instead of `call`
**Impact**: 12 instructions of dead landing pad code (SSO guard + rc_dec + resume) that can never execute because get_len cannot unwind. The same pattern appears in check_multi's invoke to @_ori_longer.
**Fix**: Use `call` instead of `invoke` when the callee is known to be `nounwind`. The codegen already performs nounwind analysis -- it should use the results to choose between call and invoke.
**First seen**: Journey 16
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Excellent borrow inference on str parameters

**Location**: @get_len parameter s, @longer parameters a and b
**Impact**: Positive -- eliminates 4 rc_inc/rc_dec pairs (one per borrowed parameter). Parameters are passed as `ptr noundef nonnull readonly dereferenceable(24)`, which communicates to LLVM that the callee only reads the data, enabling further optimization.
**Found in**: ARC Purity (Category 2), Fat Pointers: Ownership Transfer Patterns (Category 8)

### NOTE-4: Correct cross-function ownership transfer

**Location**: @make_string (producer) and @check_return (consumer)
**Impact**: Positive -- demonstrates correct ARC lifecycle across function boundaries. make_string creates a string (1 rc_inc) and transfers ownership via sret. check_return receives ownership (0 rc_inc) and performs the rc_dec after use. Per-function counts are asymmetric by design; module-level balance is perfect.
**Found in**: ARC Purity (Category 2), Fat Pointers: Ownership Transfer Patterns (Category 8)

### NOTE-5: Branchless select for if/then/else

**Location**: @longer function, `if la > lb then la else lb`
**Impact**: Positive -- compiled to `icmp sgt` + `select` (IR) / `cmp` + `cmovg` (asm). Zero branches, zero phi nodes, single basic block. This is the optimal lowering for a simple conditional expression with scalar results.
**Found in**: Control Flow & Block Layout (Category 4)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL (per extract-metrics.py) |
| ARC Correctness | 20% | 10/10 | 0 violations, correct cross-function ownership |
| Attributes & Safety | 10% | 9/10 | 97.1% compliance (33/34 checks) |
| Control Flow | 10% | 10/10 | 0 defects, branchless select optimization |
| IR Quality | 20% | 10/10 | 0 unjustified instructions (per extract-metrics.py) |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 7/10 | 1 high (aggregate pattern), 1 low (invoke to nounwind) |

**Overall: 9.4 / 10**

## Verdict

Journey 16's fat pointer ownership transfer codegen is functionally correct and demonstrates excellent ARC semantics. The compiler correctly distinguishes borrow (readonly ptr), ownership transfer out (sret), and ownership transfer in (caller-managed lifecycle). Borrow inference eliminates unnecessary RC operations on read-only parameters. The branchless select for if/then/else is optimal. The primary inefficiency is the field-by-field aggregate materialization pattern that inflates instruction counts 3-6x per str operation -- a systematic codegen pattern that represents the single largest optimization opportunity for fat pointer operations.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| String ARC lifecycle | J9 | J16 | CONFIRMED -- SSO guard pattern unchanged |
| Overflow checking | J1 | J16 | CONFIRMED -- llvm.sadd.with.overflow |
| fastcc usage | J1 | J16 | CONFIRMED |
| Borrow elision | J9 | J16 | CONFIRMED -- extended to multi-param (longer) |
| sret return ABI | J16 | J16 | NEW -- first journey to test fat pointer return |

Journey 9 tested basic string creation and length. Journey 16 extends this with cross-function ownership transfer (make_string returns str via sret, check_return receives and manages lifecycle), multi-parameter borrow (longer borrows two strings simultaneously), and the full SSO-vs-heap distinction (SSO: "hello"/"wonderful"/"ab"; heap: "abcdefghijklmnopqrstuvwxyz"). The aggregate materialization pattern was present but less visible in J9's simpler functions; J16's more complex call chains make the instruction inflation dramatically apparent.
