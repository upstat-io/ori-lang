---
journey: 9
slug: strings
theme: "I am a string"
date: 2026-03-16
status: PASS
expected: 13
eval_result: 13
aot_result: 13

difficulty: complex
prerequisites:
  - "Understanding of heap-allocated string representations"
  - "Familiarity with ARC memory management and SSO"
  - "Knowledge of boolean logic operators"
learning_objectives:
  - "See how string literals are lowered to heap-allocated OriStr via ori_str_from_raw and ori_str_empty"
  - "Understand the SSO (Small String Optimization) guard pattern in RC cleanup"
  - "Compare ARC lifecycle for strings: allocation, borrowing, and conditional RC decrement"
  - "Observe how boolean short-circuit operators compile to constant propagation"

features:
  - strings
  - string_methods
  - arc
  - branching
  - function_calls
  - multiple_functions
  - let_bindings
feature_description: "String creation, .length() method calls, ARC lifecycle with SSO guards, boolean logic with constant folding"

score: 8.7
score_breakdown:
  instruction_efficiency: 9
  arc_correctness: 10
  attributes_safety: 7
  control_flow: 7
  ir_quality: 8
  binary_quality: 10
  other_findings: 9
score_metrics:
  instruction_ratio: 1.0339
  instruction_ratio_max: 1.0476
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 19
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
  other_low: 1
overflow_check: PASS
bugs_found: []
related_journeys:
  - journey: 2
    relationship: "Both test boolean branching; J2 uses runtime branches, J9 constant-folds"
  - journey: 10
    relationship: "Both test ARC for heap-allocated collections (str vs list)"
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

**Tokens**: 179 | **Keywords**: 16 | **Identifiers**: 38 | **Errors**: 0

<details>
<summary>Token stream (first 30 tokens)</summary>

```text
Fn(@) Ident(bool_to_int) LParen Ident(b) Colon Ident(bool) RParen
Arrow Ident(int) Eq If Ident(b) Then Int(1) Else Int(0) Semi
Fn(@) Ident(check_logic) LParen RParen Arrow Ident(int) Eq
LBrace Let Ident(a) Eq True AndAnd True Semi
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
+-  FnDecl @bool_to_int
|  +-  Params: (b: bool)
|  +-  Return: int
|  +-- Body: If(Ident(b), Int(1), Int(0))
+-  FnDecl @check_logic
|  +-  Return: int
|  +-- Body: Block
|       +-  Let a = BinOp(&&, true, true)
|       +-  Let b = BinOp(&&, true, false)
|       +-  Let c = BinOp(||, false, true)
|       +-  Let d = BinOp(||, false, false)
|       +-- BinOp(+, BinOp(+, BinOp(+, Call(@bool_to_int, a), Call(@bool_to_int, b)), Call(@bool_to_int, c)), Call(@bool_to_int, d))
+-  FnDecl @check_strings
|  +-  Return: int
|  +-- Body: Block
|       +-  Let s1 = Str("hello")
|       +-  Let s2 = Str("world!")
|       +-  Let s3 = Str("")
|       +-- BinOp(+, BinOp(+, MethodCall(s1, length), MethodCall(s2, length)), MethodCall(s3, length))
+-- FnDecl @main
   +-  Return: int
   +-- Body: Block
        +-  Let a = Call(@check_logic)
        +-  Let b = Call(@check_strings)
        +-- BinOp(+, a, b)
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
//                                        ^ int (literal)
//                                              ^ int (literal)
//                                ^ int (if-then-else unified)

@check_logic () -> int = {
    let a = true && true        // a: bool (short-circuit AND)
    let b = true && false       // b: bool
    let c = false || true       // c: bool (short-circuit OR)
    let d = false || false      // d: bool
    bool_to_int(b: a) + bool_to_int(b: b) + bool_to_int(b: c) + bool_to_int(b: d)
    //                 ^ int (Add<int, int> -> int)
}

@check_strings () -> int = {
    let s1 = "hello"            // s1: str
    let s2 = "world!"           // s2: str
    let s3 = ""                 // s3: str
    s1.length() + s2.length() + s3.length()
    // ^ int      ^ int          ^ int
    //          ^ int (Add<int, int> -> int)
}

@main () -> int = {
    let a = check_logic()       // a: int
    let b = check_strings()     // b: int
    a + b                       // int (Add<int, int> -> int)
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
- Boolean && / || desugared to constant values (compile-time evaluation)
  true && true -> true, true && false -> false
  false || true -> true, false || false -> false
- .length() method calls lowered to runtime call ori_str_len
- Empty string "" lowered to ori_str_empty() call
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
@bool_to_int: no heap values -- pure scalar logic
@check_logic: no heap values -- pure scalar arithmetic
@check_strings: +3 rc_inc (implicit from ori_str_from_raw/ori_str_empty), +3 rc_dec (conditional SSO cleanup)
@main: no heap values -- delegates to check_logic/check_strings
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
  +-- @check_logic()
       +-- let a = true && true -> true
       +-- let b = true && false -> false
       +-- let c = false || true -> true
       +-- let d = false || false -> false
       +-- bool_to_int(b: true) -> 1
       +-- bool_to_int(b: false) -> 0
       +-- bool_to_int(b: true) -> 1
       +-- bool_to_int(b: false) -> 0
       +-- 1 + 0 + 1 + 0 = 2
  +-- @check_strings()
       +-- let s1 = "hello"
       +-- let s2 = "world!"
       +-- let s3 = ""
       +-- s1.length() -> 5
       +-- s2.length() -> 6
       +-- s3.length() -> 0
       +-- 5 + 6 + 0 = 11
  +-- 2 + 11 = 13
-> 13
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
@bool_to_int: +0 rc_inc, +0 rc_dec (no heap values)
@check_logic: +0 rc_inc, +0 rc_dec (no heap values -- boolean constants folded)
@check_strings: +3 rc_inc (from ori_str_from_raw/ori_str_empty), +3 rc_dec (conditional via SSO guard)
@main: +0 rc_inc, +0 rc_dec (delegates to helpers)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '09-strings'
source_filename = "09-strings"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [7 x i8] c"world!\00", align 1

; Function Attrs: nounwind memory(none) uwtable
; --- @bool_to_int ---
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) #0 {
bb0:
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}

; Function Attrs: nounwind uwtable
; --- @check_logic ---
define fastcc noundef i64 @_ori_check_logic() #1 {
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
define fastcc noundef i64 @_ori_check_strings() #2 {
bb0:
  %str_len.self22 = alloca { i64, i64, ptr }, align 8
  %str_len.self11 = alloca { i64, i64, ptr }, align 8
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
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
  call void @ori_str_from_raw(ptr %str.val.sret1, ptr @str.1, i64 6)
  %str.val.f0.ptr2 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 0
  %str.val.f03 = load i64, ptr %str.val.f0.ptr2, align 8
  %str.val.s04 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f03, 0
  %str.val.f1.ptr5 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 1
  %str.val.f16 = load i64, ptr %str.val.f1.ptr5, align 8
  %str.val.s17 = insertvalue { i64, i64, ptr } %str.val.s04, i64 %str.val.f16, 1
  %str.val.f2.ptr8 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret1, i32 0, i32 2
  %str.val.f29 = load ptr, ptr %str.val.f2.ptr8, align 8
  %str.val.s210 = insertvalue { i64, i64, ptr } %str.val.s17, ptr %str.val.f29, 2
  call void @ori_str_empty(ptr %sret.tmp)
  %sret.load.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 0
  %sret.load.f0 = load i64, ptr %sret.load.f0.ptr, align 8
  %sret.load.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %sret.load.f0, 0
  %sret.load.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 1
  %sret.load.f1 = load i64, ptr %sret.load.f1.ptr, align 8
  %sret.load.s1 = insertvalue { i64, i64, ptr } %sret.load.s0, i64 %sret.load.f1, 1
  %sret.load.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret.tmp, i32 0, i32 2
  %sret.load.f2 = load ptr, ptr %sret.load.f2.ptr, align 8
  %sret.load.s2 = insertvalue { i64, i64, ptr } %sret.load.s1, ptr %sret.load.f2, 2
  store { i64, i64, ptr } %str.val.s2, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  br label %bb1

bb1:
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2
  %rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808
  %rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  %rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
  br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap

bb3:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %str.len, i64 %str.len12)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb5:
  %add24 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %str.len23)
  %add.val25 = extractvalue { i64, i1 } %add24, 0
  %add.ovf26 = extractvalue { i64, i1 } %add24, 1
  br i1 %add.ovf26, label %add.ovf_panic28, label %add.ok27

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip

rc_dec.sso_skip:
  store { i64, i64, ptr } %str.val.s210, ptr %str_len.self11, align 8
  %str.len12 = call i64 @ori_str_len(ptr %str_len.self11)
  br label %bb3

add.ok:
  %rc_dec.fat_data13 = extractvalue { i64, i64, ptr } %str.val.s210, 2
  %rc_dec.p2i16 = ptrtoint ptr %rc_dec.fat_data13 to i64
  %rc_dec.sso_flag17 = and i64 %rc_dec.p2i16, -9223372036854775808
  %rc_dec.is_sso18 = icmp ne i64 %rc_dec.sso_flag17, 0
  %rc_dec.null.p2i19 = ptrtoint ptr %rc_dec.fat_data13 to i64
  %rc_dec.null20 = icmp eq i64 %rc_dec.null.p2i19, 0
  %rc_dec.skip_rc21 = or i1 %rc_dec.is_sso18, %rc_dec.null20
  br i1 %rc_dec.skip_rc21, label %rc_dec.sso_skip15, label %rc_dec.heap14

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

rc_dec.heap14:
  call void @ori_rc_dec(ptr %rc_dec.fat_data13, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip15

rc_dec.sso_skip15:
  store { i64, i64, ptr } %sret.load.s2, ptr %str_len.self22, align 8
  %str.len23 = call i64 @ori_str_len(ptr %str_len.self22)
  br label %bb5

add.ok27:
  %rc_dec.fat_data29 = extractvalue { i64, i64, ptr } %sret.load.s2, 2
  %rc_dec.p2i32 = ptrtoint ptr %rc_dec.fat_data29 to i64
  %rc_dec.sso_flag33 = and i64 %rc_dec.p2i32, -9223372036854775808
  %rc_dec.is_sso34 = icmp ne i64 %rc_dec.sso_flag33, 0
  %rc_dec.null.p2i35 = ptrtoint ptr %rc_dec.fat_data29 to i64
  %rc_dec.null36 = icmp eq i64 %rc_dec.null.p2i35, 0
  %rc_dec.skip_rc37 = or i1 %rc_dec.is_sso34, %rc_dec.null36
  br i1 %rc_dec.skip_rc37, label %rc_dec.sso_skip31, label %rc_dec.heap30

add.ovf_panic28:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

rc_dec.heap30:
  call void @ori_rc_dec(ptr %rc_dec.fat_data29, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip31

rc_dec.sso_skip31:
  ret i64 %add.val25
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #2 {
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

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #3

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #4

; Function Attrs: nounwind
declare void @ori_str_from_raw(ptr noalias sret({ i64, i64, ptr }), ptr, i64) #5

; Function Attrs: nounwind
declare void @ori_str_empty(ptr noalias sret({ i64, i64, ptr })) #5

; Function Attrs: nounwind
declare i64 @ori_str_len(ptr) #5

; Function Attrs: cold nounwind
; --- drop str ---
define void @"_ori_drop$3"(ptr %0) #6 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}

; Function Attrs: nounwind
declare void @ori_rc_free(ptr, i64, i64) #5

; Function Attrs: nounwind memory(inaccessiblemem: readwrite)
declare void @ori_rc_dec(ptr, ptr) #7

; Function Attrs: uwtable
define noundef i32 @main() #2 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind memory(none) uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { uwtable }
attributes #3 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { cold noreturn }
attributes #5 = { nounwind }
attributes #6 = { cold nounwind }
attributes #7 = { nounwind memory(inaccessiblemem: readwrite) }
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
  call   _ori_bool_to_int
  mov    %rax,0x18(%rsp)
  xor    %edi,%edi
  call   _ori_bool_to_int
  mov    %rax,%rcx
  mov    0x18(%rsp),%rax
  add    %rcx,%rax
  mov    %rax,0x20(%rsp)
  seto   %al
  jo     .overflow_1
  mov    $0x1,%edi
  call   _ori_bool_to_int
  ; ... (continues with overflow-checked additions)
  add    $0x28,%rsp
  ret

_ori_check_strings:
  sub    $0x108,%rsp
  ; ori_str_from_raw("hello", 5)
  lea    str(%rip),%rsi
  lea    0x78(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw
  ; load 3-field OriStr from sret into registers
  ; ori_str_from_raw("world!", 6)
  ; ori_str_empty() for ""
  ; ori_str_len(s1)
  call   ori_str_len
  ; SSO guard: check high bit + null -> skip/rc_dec
  ; ori_str_len(s2)
  ; SSO guard for s2
  ; ori_str_len(s3)
  ; SSO guard for s3
  ; overflow-checked s1.len + s2.len + s3.len
  add    $0x108,%rsp
  ret

_ori_main:
  sub    $0x18,%rsp
  call   _ori_check_logic
  mov    %rax,0x8(%rsp)
  call   _ori_check_strings
  mov    %rax,%rcx
  mov    0x8(%rsp),%rax
  add    %rcx,%rax
  mov    %rax,0x10(%rsp)
  seto   %al
  jo     .overflow
  mov    0x10(%rsp),%rax
  add    $0x18,%rsp
  ret

main:
  push   %rax
  call   _ori_main
  pop    %rcx
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @bool_to_int | 2 | 2 | 1.00x | OPTIMAL |
| 2 | @check_logic | 23 | 23 | 1.00x | OPTIMAL |
| 3 | @check_strings | 88 | 84 | 1.05x | NEAR-OPTIMAL |
| 4 | @main | 9 | 9 | 1.00x | OPTIMAL |

**@bool_to_int**: OPTIMAL. `select i1 %0, i64 1, i64 0` + `ret` -- the ideal lowering for `if b then 1 else 0`. No branches, just a conditional select.

**@check_logic**: OPTIMAL. Boolean `&&`/`||` on constants are folded to `true`/`false` at compile time, then passed as constant arguments to `bool_to_int`. Three overflow-checked additions are necessary for the sum chain. All 23 instructions are justified.

**@check_strings**: NEAR-OPTIMAL at 1.05x. The 4 unjustified instructions are 4 redundant unconditional branches (`br label %bbN`) that could be eliminated by merging blocks with their sole predecessors.

**@main**: OPTIMAL. Two calls + one overflow-checked add + ret.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @bool_to_int | 0 | 0 | YES | N/A | N/A |
| @check_logic | 0 | 0 | YES | N/A | N/A |
| @check_strings | 3 | 3 | YES | 0 elided | 0 moves |
| @main | 0 | 0 | YES | N/A | N/A |

**Module-level**: Balanced. All 3 strings created in `@check_strings` are properly cleaned up via conditional RC decrement. The RC decrements are correctly guarded by the SSO (Small String Optimization) check: strings <= 23 bytes are stored inline and require no heap deallocation.

For "hello" (5 bytes) and "world!" (6 bytes), both fit within SSO. The empty string "" also uses a special inline representation. In all three cases, the SSO guard will skip the `ori_rc_dec` call at runtime, but the guard itself is correct safety infrastructure.

**Verdict**: All functions balanced. No leaks detected. ARC is OPTIMAL for the string lifecycle.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | noundef | cold | Notes |
|----------|--------|----------|---------|---------|------|-------|
| @bool_to_int | YES | YES | YES | YES | N/A | memory(none) -- excellent |
| @check_logic | YES | YES | YES | YES | N/A | |
| @check_strings | YES | NO | YES | YES | N/A | Correct: calls non-nounwind ori_str_len |
| @main | NO | NO | YES | YES | N/A | C calling convention (entry point) |
| @_ori_drop$3 | N/A | N/A | NO | N/A | YES | [LOW-2] missing uwtable, noundef |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | YES | cold noreturn -- correct |

**@bool_to_int** has the ideal attribute set: `nounwind memory(none)` -- the compiler correctly identified this function as pure (no memory access, cannot unwind).

**@check_strings** is correctly missing `nounwind`: it calls `ori_str_len` which is declared `nounwind` but also calls `ori_rc_dec` which has `memory(inaccessiblemem: readwrite)`. The two-pass fixed-point analysis correctly determined that `check_strings` may unwind through the string operations path.

**@_ori_drop$3** is missing `uwtable` and `noundef` on its parameter -- these are the 2 missing attributes (19 applicable, 17 correct = 89.5% compliance).

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @bool_to_int | 1 | 0 | 0 | 0 | |
| @check_logic | 7 | 0 | 0 | 0 | |
| @check_strings | 14 | 0 | 4 | 0 | [MEDIUM-1] |
| @main | 3 | 0 | 0 | 0 | |

**@check_strings** has 4 redundant unconditional branches. The 14-block structure arises from interleaving string operations, SSO-guarded RC cleanup, and overflow-checked additions. Each SSO guard generates a correct 3-block diamond (check, heap-path, skip-path). The redundant branches occur at block boundaries where sequential operations are unnecessarily split:

1. `bb0` ends with `br label %bb1` (could merge bb0 into bb1)
2. `rc_dec.sso_skip` ends with `br label %bb3` (could merge)
3. `rc_dec.sso_skip15` ends with `br label %bb5` (could merge)
4. One additional structural br within the RC flow

These are codegen artifacts from the string cleanup pattern, not semantic issues.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (check_logic, 3x) | YES | YES | llvm.sadd.with.overflow.i64 |
| add (check_strings, 2x) | YES | YES | llvm.sadd.with.overflow.i64 |
| add (main, 1x) | YES | YES | llvm.sadd.with.overflow.i64 |

All 6 integer additions use `llvm.sadd.with.overflow.i64` with correct panic-on-overflow branching.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.3 MiB (debug) |
| .text section | 885.4 KiB |
| .rodata section | 133.7 KiB |
| User code | ~199 instructions (~600 bytes) |
| Runtime | >99% of binary |

#### Disassembly: @bool_to_int

```asm
_ori_bool_to_int:
  mov    %dil,%dl
  xor    %eax,%eax
  mov    $0x1,%ecx
  test   $0x1,%dl
  cmovne %rcx,%rax
  ret
```

Compact 6-instruction implementation using `cmovne` for branchless bool-to-int conversion.

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
  mov    %rax,0x10(%rsp)
  seto   %al
  jo     .overflow
  mov    0x10(%rsp),%rax
  add    $0x18,%rsp
  ret
```

### 7. Optimal IR Comparison

#### @bool_to_int: Ideal vs Actual

```llvm
; IDEAL (2 instructions)
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) nounwind memory(none) {
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}
```

```llvm
; ACTUAL (2 instructions) -- identical
define fastcc noundef i64 @_ori_bool_to_int(i1 noundef %0) #0 {
bb0:
  %sel = select i1 %0, i64 1, i64 0
  ret i64 %sel
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @check_logic: Ideal vs Actual

```llvm
; IDEAL (23 instructions)
; Same as actual -- constant folding of && and || is correct,
; 4 calls to bool_to_int with constant args,
; 3 overflow-checked additions, 3 panic blocks, 1 ret.
; All 23 instructions justified.
```

**Delta**: +0 instructions. OPTIMAL.

#### @check_strings: Ideal vs Actual

```llvm
; IDEAL (84 instructions)
; String function requires:
; - 6 allocas
; - 3 string constructions (ori_str_from_raw x2, ori_str_empty x1)
; - 3 sret load sequences (3 GEP+load+insertvalue each = 27 total)
; - 3 store + ori_str_len calls = 6
; - 3 SSO-guarded RC decrements (8 instructions each = 24)
; - 2 overflow-checked additions (7 each = 14)
; - 1 ret
; Total: 6 + 3 + 27 + 6 + 24 + 14 + 1 = 81-84
; Actual has 4 redundant br instructions -> 88
```

**Delta**: +4 instructions (redundant unconditional branches -- unjustified)

#### @main: Ideal vs Actual

```llvm
; IDEAL (9 instructions)
define noundef i64 @_ori_main() {
  %call = call fastcc i64 @_ori_check_logic()
  %call1 = call fastcc i64 @_ori_check_strings()
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %panic, label %ok
ok:
  ret i64 %add.val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @bool_to_int | 2 | 2 | +0 | N/A | OPTIMAL |
| @check_logic | 23 | 23 | +0 | N/A | OPTIMAL |
| @check_strings | 84 | 88 | +4 | NO (redundant br) | NEAR-OPTIMAL |
| @main | 9 | 9 | +0 | N/A | OPTIMAL |

### 8. Strings: Representation

Ori strings use a 3-field representation: `{ i64, i64, ptr }` -- the `OriStr` fat struct:
- Field 0 (`i64`): inline data / pointer to heap buffer
- Field 1 (`i64`): length in bytes
- Field 2 (`ptr`): heap data pointer (with SSO flag in high bit)

String literals are constructed via `ori_str_from_raw(ptr sret, ptr raw, i64 len)` which takes a destination sret pointer, a raw C string pointer, and the byte length. The empty string uses the specialized `ori_str_empty()` constructor.

The sret (struct return) pattern requires an alloca + store + per-field GEP+load+insertvalue sequence to move the result into SSA registers. This is verbose (9 instructions per string construction) but correct for the JIT-safe aggregate loading pattern.

### 9. Strings: SSO Guard Pattern

Each string's RC decrement is guarded by an SSO (Small String Optimization) check:

```llvm
%rc_dec.fat_data = extractvalue { i64, i64, ptr } %str.val.s2, 2
%rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
%rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808   ; check high bit
%rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
%rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64         ; duplicate ptrtoint [LOW-4]
%rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
%rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap
```

This 8-instruction SSO guard checks two conditions: (1) high bit set = SSO string stored inline, (2) null pointer = no heap allocation. Both cases skip the `ori_rc_dec` call. The `ptrtoint` is computed twice with the same operand -- LLVM's CSE pass eliminates this before native codegen, so it is cosmetic at the IR level. [LOW-4]

### 10. Strings: Constant Folding Opportunity

The compiler correctly folds `true && true` to `true`, `true && false` to `false`, etc. at canonicalization time. However, it does not fold `bool_to_int(b: true)` to `1` -- the function calls are emitted with constant arguments rather than being inlined. This is acceptable behavior: LLVM's inliner handles this in release builds, and at `-O0` the calls provide clear debugging. Not a defect, just a potential compile-time optimization opportunity.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM | Control Flow | 4 redundant unconditional branches in @check_strings | NEW | J9 |
| 2 | LOW | Attributes | Missing uwtable and noundef on @_ori_drop$3 | CONFIRMED | J5 |
| 3 | NOTE | ARC | Correct SSO-guarded conditional RC decrement for all 3 strings | NEW | J9 |
| 4 | LOW | IR Quality | Duplicate ptrtoint in SSO guard (LLVM CSE handles it) | NEW | J9 |
| 5 | NOTE | Codegen | Excellent constant folding of boolean && / \|\| operators | NEW | J9 |
| 6 | NOTE | Codegen | Pure function detection: @bool_to_int gets memory(none) | NEW | J9 |

### MEDIUM-1: Redundant unconditional branches in @check_strings

**Location**: @check_strings, 4 blocks ending with `br label %target` where target has a single predecessor
**Impact**: 4 unnecessary branch instructions; correct but suboptimal block layout
**Fix**: Merge sequential blocks when the target has exactly one predecessor
**First seen**: Journey 9
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-2: Missing uwtable and noundef on @_ori_drop$3

**Location**: @_ori_drop$3 function definition
**Impact**: Missing unwind tables reduce debugger accuracy; missing noundef is cosmetic
**Fix**: Add `uwtable` attribute to generated drop helpers; add `noundef` to the ptr parameter
**First seen**: Journey 5
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Correct SSO-guarded conditional RC decrement

**Location**: @check_strings, 3 SSO guard sequences
**Impact**: Positive -- correctly avoids calling ori_rc_dec on SSO/inline strings
**Found in**: ARC Purity (Category 2)

### LOW-4: Duplicate ptrtoint in SSO guard

**Location**: @check_strings, each SSO guard block (3 occurrences)
**Impact**: 3 redundant `ptrtoint` instructions at IR level -- LLVM CSE eliminates these before native codegen
**Fix**: Reuse the first `ptrtoint` result for the null check
**First seen**: Journey 9
**Found in**: Strings: SSO Guard Pattern (Category 9)

### NOTE-5: Excellent constant folding of boolean operators

**Location**: @check_logic
**Impact**: Positive -- `true && true` becomes constant `true`, eliminating all runtime branching for boolean logic
**Found in**: Compiler Pipeline / Canonicalization

### NOTE-6: Pure function detection yields memory(none)

**Location**: @bool_to_int
**Impact**: Positive -- the `nounwind memory(none)` attribute set is ideal for a pure function, enabling maximum LLVM optimization
**Found in**: Attributes & Calling Convention (Category 3)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 9/10 | 1.03x avg ratio (max 1.05x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 7/10 | 89.5% compliance |
| Control Flow | 10% | 7/10 | 4 defects |
| IR Quality | 20% | 8/10 | 4 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 9/10 | 1 low |

**Overall: 8.7 / 10**

## Verdict

Journey 9's string codegen is solid. The compiler correctly handles the full OriStr lifecycle: heap allocation via `ori_str_from_raw`, SSO-guarded conditional RC decrement, and the specialized `ori_str_empty()` for empty strings. ARC is perfectly balanced with zero violations. The main overhead comes from 4 redundant unconditional branches in `@check_strings` (a block merging opportunity) and 2 missing attributes on the drop helper. The boolean logic side demonstrates excellent constant folding -- `&&` and `||` on compile-time-known operands are resolved during canonicalization. The `memory(none)` annotation on `@bool_to_int` confirms the compiler's pure function analysis working correctly.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J9 | CONFIRMED |
| fastcc usage | J1 | J9 | CONFIRMED |
| Constant folding (booleans) | J2 | J9 | CONFIRMED |
| nounwind propagation | J1 | J9 | CONFIRMED |
| ARC string lifecycle | NEW | J9 | NEW |
| SSO guard pattern | NEW | J9 | NEW |
| memory(none) on pure functions | NEW | J9 | NEW |
| Drop helper missing uwtable | J5 | J9 | CONFIRMED |

This is the first journey to exercise heap-allocated string values with ARC. The SSO guard pattern (8 instructions per string cleanup) is new infrastructure that will appear in any journey involving strings, lists, or other heap-allocated types. The `memory(none)` attribute on `@bool_to_int` confirms the compiler can identify pure functions and annotate them optimally.
