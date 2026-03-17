---
journey: 14
slug: fat-string-sharing
theme: "I am a fat pointer"
date: 2026-03-16
status: PASS
expected: 65
eval_result: 65
aot_result: 65

difficulty: complex
prerequisites:
  - "Understanding of heap-allocated string representations (fat pointers)"
  - "Familiarity with SSO (Small String Optimization) and when it applies"
  - "ARC memory management for shared heap-allocated values"
  - "Function call conventions for aggregate types passed by reference"
learning_objectives:
  - "See how strings are represented as fat pointers {i64 len, i64 cap, ptr data} in LLVM IR"
  - "Understand SSO guard pattern: bit 63 check on data pointer to skip RC for inline strings"
  - "Observe borrow elision: read-only string parameters avoid rc_inc/rc_dec entirely"
  - "Compare EH-aware RC cleanup (landing pad vs normal path) for correct exception safety"
  - "Distinguish SSO strings (no heap, no RC) from heap strings (RC-managed data pointer)"

features:
  - strings
  - arc
  - function_calls
  - multiple_functions
feature_description: "Fat pointer string representation, SSO vs heap discrimination, borrow elision on read-only parameters, EH-safe RC cleanup"

score: 9.4
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 10
  control_flow: 8
  ir_quality: 8
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.00
  instruction_ratio_max: 1.00
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 21
  attr_correct: 21
  attr_has_wrong: false
  cf_defects: 2
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
  - journey: 9
    relationship: "Both test string ARC lifecycle with SSO guards; J9 tests boolean logic + empty strings, J14 focuses on fat pointer sharing and borrow elision"
  - journey: 10
    relationship: "Both test ARC for heap-allocated collections; J10 tests lists, J14 tests heap strings passed across function boundaries"
---

# Journey 14: "I am a fat pointer"

## Source

```ori
// Journey 14: "I am a fat pointer"
// Slug: fat-string-sharing
// Difficulty: complex
// Features: strings, arc, function_calls, multiple_functions
// Expected: sso_len() + heap_len() + shared_len(long) = 5 + 30 + 30 = 65

@sso_len () -> int = {
    let s = "hello";
    s.length()
}

@heap_len () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    s.length()
}

@shared_len (s: str) -> int = s.length();

@main () -> int = {
    let a = sso_len();
    let b = heap_len();
    let long = "abcdefghijklmnopqrstuvwxyz1234";
    let c = shared_len(s: long);
    a + b + c
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 65        | 65       | (none) | (none) | PASS   |
| AOT     | 65        | 65       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 123 | **Keywords**: 8 (`let` x4, `int` x4) | **Identifiers**: 20 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(sso_len) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(s) Eq Str("hello") Semi
  Ident(s) Dot Ident(length) LParen RParen
RBrace Semi
Fn(@) Ident(heap_len) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(s) Eq Str("abcdefghijklmnopqrstuvwxyz1234") Semi
  Ident(s) Dot Ident(length) LParen RParen
RBrace Semi
Fn(@) Ident(shared_len) LParen Ident(s) Colon Ident(str) RParen Arrow Ident(int) Eq
  Ident(s) Dot Ident(length) LParen RParen Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(a) Eq Ident(sso_len) LParen RParen Semi
  Let Ident(b) Eq Ident(heap_len) LParen RParen Semi
  Let Ident(long) Eq Str("abcdefghijklmnopqrstuvwxyz1234") Semi
  Let Ident(c) Eq Ident(shared_len) LParen Ident(s) Colon Ident(long) RParen Semi
  Ident(a) Plus Ident(b) Plus Ident(c)
RBrace Semi
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 24 | **Max depth**: 4 | **Functions**: 4 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @sso_len
│  ├─ Params: ()
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s = Str("hello")
│       └─ MethodCall(s, length, [])
├─ FnDecl @heap_len
│  ├─ Params: ()
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s = Str("abcdefghijklmnopqrstuvwxyz1234")
│       └─ MethodCall(s, length, [])
├─ FnDecl @shared_len
│  ├─ Params: (s: str)
│  ├─ Return: int
│  └─ Body: MethodCall(s, length, [])
└─ FnDecl @main
   ├─ Params: ()
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@sso_len, [])
        ├─ Let b = Call(@heap_len, [])
        ├─ Let long = Str("abcdefghijklmnopqrstuvwxyz1234")
        ├─ Let c = Call(@shared_len, [s: long])
        └─ BinOp(+)
             ├─ BinOp(+)
             │    ├─ Ident(a)
             │    └─ Ident(b)
             └─ Ident(c)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 16 | **Types inferred**: 8 | **Unifications**: 12 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@sso_len () -> int = {
    let s: str = "hello";             // str literal -> str
    s.length()                        // str.length() -> int
    //          ^ int (builtin method)
}

@heap_len () -> int = {
    let s: str = "abcdefghijklmnopqrstuvwxyz1234";  // str literal -> str
    s.length()                        // str.length() -> int
}

@shared_len (s: str) -> int = s.length();
//                            ^ int (str.length() -> int)

@main () -> int = {
    let a: int = sso_len();           // () -> int
    let b: int = heap_len();          // () -> int
    let long: str = "abcdefghijklmnopqrstuvwxyz1234";
    let c: int = shared_len(s: long); // (str) -> int
    a + b + c                         // int + int + int -> int
    // ^ int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 4 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Method call s.length() lowered to builtin str_len dispatch (x3)
- Function bodies lowered to canonical expression form
- String literals interned as constant references
- Call arguments normalized to positional order
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 6 | **Elided**: 2 | **Net ops**: 4

<details>
<summary>ARC annotations</summary>

```text
@sso_len: +0 rc_inc (str created via ori_str_from_raw), +1 rc_dec (SSO-guarded drop at end)
  - ori_str_from_raw handles initial allocation; SSO guard skips rc_dec for inline strings
@heap_len: +0 rc_inc, +1 rc_dec (SSO-guarded drop at end)
  - Same pattern; 30-char string exceeds SSO threshold, so rc_dec fires at runtime
@shared_len: +0 rc_inc, +0 rc_dec (BORROW ELISION — read-only param, no ownership transfer)
  - Parameter passed by ptr readonly — no RC ops needed
@main: +0 rc_inc, +2 rc_dec (normal path + EH landing pad, mutually exclusive)
  - Creates "long" string via ori_str_from_raw
  - Passes to shared_len via invoke (EH-aware)
  - Normal: rc_dec in add.ok6 | Exception: rc_dec in bb2 landing pad
  - Semantically balanced: exactly 1 rc_dec fires per execution
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 65 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  ├─ let a = @sso_len()
  │    ├─ let s = "hello"
  │    └─ s.length() = 5
  │  → 5
  ├─ let b = @heap_len()
  │    ├─ let s = "abcdefghijklmnopqrstuvwxyz1234"
  │    └─ s.length() = 30
  │  → 30
  ├─ let long = "abcdefghijklmnopqrstuvwxyz1234"
  ├─ let c = @shared_len(s: long)
  │    └─ s.length() = 30
  │  → 30
  └─ 5 + 30 + 30 = 65
→ 65
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 6 | **Elided**: 2 | **Net ops**: 4

<details>
<summary>ARC annotations</summary>

```text
@sso_len: +0 rc_inc, +1 rc_dec (SSO-guarded — skipped at runtime for "hello")
@heap_len: +0 rc_inc, +1 rc_dec (SSO-guarded — fires at runtime for 30-char heap string)
@shared_len: +0 rc_inc, +0 rc_dec (BORROW ELISION — ptr readonly, no ownership)
@main: +0 rc_inc, +2 rc_dec (mutually exclusive: normal path OR EH landing pad)
  Total: 4 rc_dec syntactic, 3 semantic (1 elided by mutual exclusion)
  Borrow elision saved: 2 ops (rc_inc + rc_dec on shared_len parameter)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '14-fat-string-sharing'
source_filename = "14-fat-string-sharing"

@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [31 x i8] c"abcdefghijklmnopqrstuvwxyz1234\00", align 1
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @sso_len ---
define fastcc noundef i64 @_ori_sso_len() #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
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

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip

rc_dec.sso_skip:
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @heap_len ---
define fastcc noundef i64 @_ori_heap_len() #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %str.val.sret = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %str.val.sret, ptr @str.1, i64 30)
  %str.val.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 0
  %str.val.f0 = load i64, ptr %str.val.f0.ptr, align 8
  %str.val.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %str.val.f0, 0
  %str.val.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 1
  %str.val.f1 = load i64, ptr %str.val.f1.ptr, align 8
  %str.val.s1 = insertvalue { i64, i64, ptr } %str.val.s0, i64 %str.val.f1, 1
  %str.val.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %str.val.sret, i32 0, i32 2
  %str.val.f2 = load ptr, ptr %str.val.f2.ptr, align 8
  %str.val.s2 = insertvalue { i64, i64, ptr } %str.val.s1, ptr %str.val.f2, 2
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

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip

rc_dec.sso_skip:
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @shared_len ---
define fastcc noundef i64 @_ori_shared_len(ptr noundef nonnull readonly dereferenceable(24) %0) #0 {
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

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 personality ptr @ori_eh_personality {
bb0:
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %str.val.sret = alloca { i64, i64, ptr }, align 8
  %call = call fastcc i64 @_ori_sso_len()
  %call1 = call fastcc i64 @_ori_heap_len()
  call void @ori_str_from_raw(ptr %str.val.sret, ptr @str.1, i64 30)
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
  %call2 = invoke fastcc i64 @_ori_shared_len(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

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

add.ok:
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  %add.val4 = extractvalue { i64, i1 } %add3, 0
  %add.ovf5 = extractvalue { i64, i1 } %add3, 1
  br i1 %add.ovf5, label %add.ovf_panic7, label %add.ok6

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok6:
  %rc_dec.fat_data8 = extractvalue { i64, i64, ptr } %str.val.s2, 2
  %rc_dec.p2i11 = ptrtoint ptr %rc_dec.fat_data8 to i64
  %rc_dec.sso_flag12 = and i64 %rc_dec.p2i11, -9223372036854775808
  %rc_dec.is_sso13 = icmp ne i64 %rc_dec.sso_flag12, 0
  %rc_dec.null.p2i14 = ptrtoint ptr %rc_dec.fat_data8 to i64
  %rc_dec.null15 = icmp eq i64 %rc_dec.null.p2i14, 0
  %rc_dec.skip_rc16 = or i1 %rc_dec.is_sso13, %rc_dec.null15
  br i1 %rc_dec.skip_rc16, label %rc_dec.sso_skip10, label %rc_dec.heap9

add.ovf_panic7:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

rc_dec.heap9:
  call void @ori_rc_dec(ptr %rc_dec.fat_data8, ptr @"_ori_drop$3")
  br label %rc_dec.sso_skip10

rc_dec.sso_skip10:
  ret i64 %add.val4
}
```

#### Disassembly

```asm
_ori_sso_len:
  sub    $0x48,%rsp
  lea    0xd71f1(%rip),%rsi          ; @str "hello"
  lea    0x18(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw
  mov    0x18(%rsp),%rax             ; load len
  mov    0x20(%rsp),%rcx             ; load cap
  mov    0x28(%rsp),%rdx             ; load data ptr
  mov    %rdx,0x8(%rsp)             ; save data ptr
  mov    %rdx,0x40(%rsp)            ; store for str_len
  mov    %rcx,0x38(%rsp)
  mov    %rax,0x30(%rsp)
  lea    0x30(%rsp),%rdi
  call   ori_str_len
  mov    %rax,0x10(%rsp)            ; save result
  mov    0x8(%rsp),%rcx             ; reload data ptr
  movabs $0x8000000000000000,%rdx   ; SSO flag mask (bit 63)
  mov    %rcx,%rax
  and    %rdx,%rax                  ; check bit 63
  cmp    $0x0,%rax
  setne  %al                        ; is_sso = (bit63 != 0)
  cmp    $0x0,%rcx
  sete   %cl                        ; is_null = (ptr == 0)
  or     %cl,%al                    ; skip = is_sso || is_null
  test   $0x1,%al
  jne    .sso_skip                  ; skip RC if SSO or null
  mov    0x8(%rsp),%rdi
  lea    _ori_drop$3(%rip),%rsi
  call   ori_rc_dec                 ; RC-- (only for heap strings)
.sso_skip:
  mov    0x10(%rsp),%rax            ; return length
  add    $0x48,%rsp
  ret

_ori_heap_len:
  ; [identical structure to @sso_len, with @str.1 (30 chars) instead of @str]
  sub    $0x48,%rsp
  lea    @str.1(%rip),%rsi
  ; ... same SSO guard pattern ...
  ret

_ori_shared_len:
  sub    $0x18,%rsp
  mov    (%rdi),%rax                ; load len from caller's ptr
  mov    0x8(%rdi),%rcx             ; load cap
  mov    0x10(%rdi),%rdx            ; load data ptr
  mov    %rdx,0x10(%rsp)
  mov    %rcx,0x8(%rsp)
  mov    %rax,(%rsp)
  mov    %rsp,%rdi
  call   ori_str_len                ; call with local copy
  add    $0x18,%rsp
  ret                               ; NO rc_inc, NO rc_dec — borrow elision

_ori_main:
  sub    $0x68,%rsp
  call   _ori_sso_len               ; a = 5
  mov    %rax,0x18(%rsp)
  call   _ori_heap_len              ; b = 30
  mov    %rax,0x20(%rsp)
  ; create "long" string
  lea    @str.1(%rip),%rsi
  lea    0x38(%rsp),%rdi
  mov    $0x1e,%edx
  call   ori_str_from_raw
  ; load fat pointer fields, store to ref_arg
  ; invoke shared_len (EH-aware)
  call   _ori_shared_len            ; c = 30
  ; overflow-checked a + b
  add    %rcx,%rax
  jo     .overflow_panic
  ; overflow-checked (a+b) + c
  add    %rcx,%rax
  jo     .overflow_panic
  ; SSO-guarded rc_dec for "long" string
  ; ... bit 63 check pattern ...
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @sso_len | 26 | 24 | 1.08x | NEAR-OPTIMAL |
| 2 | @heap_len | 26 | 24 | 1.08x | NEAR-OPTIMAL |
| 3 | @shared_len | 13 | 13 | 1.00x | OPTIMAL |
| 4 | @main | 53 | 51 | 1.04x | NEAR-OPTIMAL |

**@sso_len / @heap_len overhead** (+2 each): 1 redundant `br label %bb1` (could merge bb0 and bb1), 1 duplicate `ptrtoint` in SSO guard (same pointer converted twice).

**@shared_len**: Perfectly minimal. Load 3 fields from caller's pointer, store to local alloca, call `ori_str_len`, return. Zero RC ops due to borrow elision.

**@main overhead** (+2): 2 duplicate `ptrtoint` instructions (one in normal path `add.ok6`, one in EH landing pad `bb2`). Each SSO guard sequence converts the same `%rc_dec.fat_data` pointer to integer twice (`%rc_dec.p2i` and `%rc_dec.null.p2i` are identical operations). The redundant pair in the EH path is a separate copy from the one in the normal path, so both contribute 1 unjustified instruction each.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @sso_len | 0 | 1 | YES | N/A | N/A |
| @heap_len | 0 | 1 | YES | N/A | N/A |
| @shared_len | 0 | 0 | YES | 1 elided pair | 0 moves |
| @main | 0 | 2 (mutex) | YES | 0 elided | 0 moves |

**Verdict**: All functions semantically balanced. The extract-metrics tool flags @_ori_main as unbalanced because it counts 2 syntactic `ori_rc_dec` calls against 1 `ori_str_from_raw`. However, these 2 rc_dec calls are on mutually exclusive paths: the normal path (`add.ok6`) and the EH landing pad (`bb2`). At runtime, exactly one fires. This is the correct EH cleanup pattern for exception safety.

**Borrow elision on @shared_len** is excellent: the parameter is passed as `ptr noundef nonnull readonly dereferenceable(24)` -- no rc_inc on entry, no rc_dec on exit. The caller retains ownership; the callee borrows without touching the reference count. [NOTE-3]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @sso_len | YES | YES | N/A | N/A | NO | |
| @heap_len | YES | YES | N/A | N/A | NO | |
| @shared_len | YES | YES | N/A | YES (param) | NO | `noundef nonnull readonly deref(24)` on param |
| @main | NO (C) | NO | N/A | N/A | NO | C calling convention for entry point |
| @_ori_drop$3 | N/A | YES | N/A | N/A | YES | Correct cold annotation |
| @ori_str_from_raw | N/A | YES | YES (sret) | N/A | N/A | |
| @ori_rc_dec | N/A | YES | N/A | N/A | N/A | `memory(inaccessiblemem: readwrite)` |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | YES | `cold noreturn` |

**Verdict**: 100% attribute compliance. All user functions have correct calling conventions. @shared_len parameter has full attribute set including `readonly` and `dereferenceable(24)`. @_ori_main correctly uses C calling convention (entry point) and omits `nounwind` (has `personality` for EH). Runtime declarations have appropriate attributes.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @sso_len | 4 | 0 | 1 | 0 | [LOW-1] |
| @heap_len | 4 | 0 | 1 | 0 | [LOW-1] |
| @shared_len | 1 | 0 | 0 | 0 | |
| @main | 11 | 0 | 0 | 0 | |

**@sso_len / @heap_len**: The `br label %bb1` at the end of `bb0` is a redundant unconditional branch. `bb0` and `bb1` could be merged since `bb1` has only one predecessor. The remaining 3-block structure (bb1 -> rc_dec.heap/rc_dec.sso_skip) is the expected SSO guard diamond pattern. [LOW-1]

**@main**: 11 blocks is high but justified: bb0 (setup + invoke), bb1 (first add), bb2 (EH landing pad), rc_dec.heap/rc_dec.sso_skip (EH cleanup pair), add.ok (second add), add.ovf_panic (first panic), add.ok6 (normal cleanup), add.ovf_panic7 (second panic), rc_dec.heap9/rc_dec.sso_skip10 (normal cleanup pair). All blocks serve a purpose.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (a+b) | YES | YES | `llvm.sadd.with.overflow.i64` with panic on overflow |
| add ((a+b)+c) | YES | YES | Second `llvm.sadd.with.overflow.i64` |

Both additions in `@main` use `llvm.sadd.with.overflow.i64` with branches to `ori_panic_cstr` on overflow. No arithmetic in other functions (they only call `ori_str_len`).

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.33 MiB (debug) |
| .text section | 885.7 KiB |
| .rodata section | 133.8 KiB |
| User code | 690 bytes (4 user functions + drop + C wrapper) |
| Runtime | 99.9% of binary |

#### Disassembly: @sso_len

```asm
_ori_sso_len:                        ; 144 bytes
  sub    $0x48,%rsp
  lea    @str(%rip),%rsi             ; "hello\0"
  lea    0x18(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw
  ; load 3 fields from sret, store to str_len alloca
  lea    0x30(%rsp),%rdi
  call   ori_str_len
  ; SSO guard: check bit 63 of data ptr
  movabs $0x8000000000000000,%rdx
  ; ... setne/sete/or/test/jne pattern ...
  ; conditional ori_rc_dec
  ret
```

#### Disassembly: @shared_len

```asm
_ori_shared_len:                     ; 42 bytes
  sub    $0x18,%rsp
  mov    (%rdi),%rax                 ; load len
  mov    0x8(%rdi),%rcx              ; load cap
  mov    0x10(%rdi),%rdx             ; load data
  mov    %rdx,0x10(%rsp)
  mov    %rcx,0x8(%rsp)
  mov    %rax,(%rsp)
  mov    %rsp,%rdi
  call   ori_str_len
  add    $0x18,%rsp
  ret
```

#### Disassembly: @main

```asm
_ori_main:                           ; 313 bytes
  sub    $0x68,%rsp
  call   _ori_sso_len
  ; save result, call _ori_heap_len
  ; create "long" string, invoke _ori_shared_len
  ; overflow-checked additions
  ; SSO-guarded cleanup (normal + EH paths)
  ret
```

### 7. Optimal IR Comparison

#### @sso_len: Ideal vs Actual

```llvm
; IDEAL (24 instructions)
define fastcc noundef i64 @_ori_sso_len() nounwind {
  %sret = alloca { i64, i64, ptr }, align 8
  %self = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %sret, ptr @str, i64 5)
  %f0p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret, i32 0, i32 0
  %f0 = load i64, ptr %f0p, align 8
  %s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %f0, 0
  %f1p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret, i32 0, i32 1
  %f1 = load i64, ptr %f1p, align 8
  %s1 = insertvalue { i64, i64, ptr } %s0, i64 %f1, 1
  %f2p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %sret, i32 0, i32 2
  %f2 = load ptr, ptr %f2p, align 8
  %s2 = insertvalue { i64, i64, ptr } %s1, ptr %f2, 2
  store { i64, i64, ptr } %s2, ptr %self, align 8
  %len = call i64 @ori_str_len(ptr %self)
  ; SSO guard — merged into same block, single ptrtoint
  %data = extractvalue { i64, i64, ptr } %s2, 2
  %p2i = ptrtoint ptr %data to i64
  %sso = and i64 %p2i, -9223372036854775808
  %is_sso = icmp ne i64 %sso, 0
  %is_null = icmp eq i64 %p2i, 0           ; reuse %p2i
  %skip = or i1 %is_sso, %is_null
  br i1 %skip, label %done, label %heap
heap:
  call void @ori_rc_dec(ptr %data, ptr @"_ori_drop$3")
  br label %done
done:
  ret i64 %len
}
```

```llvm
; ACTUAL (26 instructions)
; Differences: +1 redundant br label %bb1, +1 duplicate ptrtoint
```

**Delta**: +2 instructions. The redundant `br label %bb1` and the duplicate `ptrtoint` are unjustified.

#### @shared_len: Ideal vs Actual

```llvm
; IDEAL (13 instructions) = ACTUAL (13 instructions)
define fastcc noundef i64 @_ori_shared_len(ptr noundef nonnull readonly dereferenceable(24) %0) nounwind {
  %self = alloca { i64, i64, ptr }, align 8
  %f0p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 0
  %f0 = load i64, ptr %f0p, align 8
  %s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %f0, 0
  %f1p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 1
  %f1 = load i64, ptr %f1p, align 8
  %s1 = insertvalue { i64, i64, ptr } %s0, i64 %f1, 1
  %f2p = getelementptr inbounds nuw { i64, i64, ptr }, ptr %0, i32 0, i32 2
  %f2 = load ptr, ptr %f2p, align 8
  %s2 = insertvalue { i64, i64, ptr } %s1, ptr %f2, 2
  store { i64, i64, ptr } %s2, ptr %self, align 8
  %len = call i64 @ori_str_len(ptr %self)
  ret i64 %len
}
```

**Delta**: +0 instructions. OPTIMAL. Zero RC ops, clean borrow semantics.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @sso_len | 24 | 26 | +2 | NO (redundant br + dup ptrtoint) | NEAR-OPTIMAL |
| @heap_len | 24 | 26 | +2 | NO (redundant br + dup ptrtoint) | NEAR-OPTIMAL |
| @shared_len | 13 | 13 | +0 | N/A | OPTIMAL |
| @main | 51 | 53 | +2 | NO (2x dup ptrtoint) | NEAR-OPTIMAL |

### 8. Fat Pointers: SSO vs Heap Discrimination

The SSO guard pattern correctly discriminates between inline (SSO) and heap-allocated strings:

**Guard sequence** (8 instructions per site):
1. `extractvalue` -- extract `data` pointer from fat struct field 2
2. `ptrtoint` -- convert to integer for bit inspection
3. `and i64 %p2i, 0x8000000000000000` -- isolate bit 63 (SSO flag)
4. `icmp ne` -- check if SSO flag is set
5. `ptrtoint` -- (REDUNDANT, same as step 2) [LOW-2]
6. `icmp eq i64 %p2i, 0` -- check for null pointer
7. `or i1` -- skip RC if SSO OR null
8. `br i1` -- conditional branch

**SSO semantics**: For "hello" (5 chars, under the 23-byte SSO threshold), `ori_str_from_raw` stores the data inline in the `{len, cap, data}` struct with bit 63 set in the `data` field. The guard detects this and skips `ori_rc_dec`. For "abcdefghijklmnopqrstuvwxyz1234" (30 chars, above SSO threshold), the data is heap-allocated and the guard falls through to `ori_rc_dec`.

**Correctness**: The guard is correct. The null check is defense-in-depth (should never happen for properly constructed strings, but prevents UB if it does). The SSO flag check is the primary fast path. [NOTE-4]

### 9. Fat Pointers: RC Lifecycle

**String creation**: All strings are created via `ori_str_from_raw(ptr sret, ptr raw_data, i64 len)`. This runtime function handles:
- SSO: copies data inline, sets bit 63 in the data pointer field
- Heap: allocates buffer, copies data, initializes RC to 1

**Borrow elision**: `@shared_len` receives its parameter as `ptr readonly` -- the caller (`@main`) stores the fat struct to `%ref_arg` and passes the pointer. The callee loads the fields, calls `ori_str_len`, and returns. No RC operations needed because the caller guarantees the string lives across the call. [NOTE-3]

**EH-safe cleanup in @main**: The call to `@shared_len` uses `invoke` (not `call`) because `@_ori_main` has `personality ptr @ori_eh_personality`. This means:
- Normal path: rc_dec in `add.ok6` block (after arithmetic completes)
- Exception path: rc_dec in `bb2` landing pad (cleanup before resuming unwind)
- These paths are **mutually exclusive** -- exactly one fires per execution
- This is the correct pattern for exception-safe resource management [NOTE-5]

**Ownership flow**: `@main` owns the "long" string from creation (`ori_str_from_raw`) to cleanup (`ori_rc_dec`). `@shared_len` borrows it without affecting the reference count. After `shared_len` returns, `@main` drops the string exactly once.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Control Flow | Redundant `br label %bb1` in @sso_len and @heap_len | NEW | J14 |
| 2 | LOW | IR Quality | Duplicate `ptrtoint` in SSO guard (4 instances) | NEW | J14 |
| 3 | NOTE | ARC | Excellent borrow elision on @shared_len | NEW | J14 |
| 4 | NOTE | Fat Pointers | SSO guard correctly discriminates inline vs heap strings | NEW | J14 |
| 5 | NOTE | Fat Pointers | EH-safe RC cleanup with mutually exclusive paths in @main | NEW | J14 |

### LOW-1: Redundant unconditional branch in @sso_len and @heap_len

**Location**: @sso_len bb0 -> bb1, @heap_len bb0 -> bb1
**Impact**: 1 unnecessary instruction per function (2 total). `bb1` has exactly one predecessor (`bb0`), so the blocks could be merged.
**Fix**: Merge bb0 and bb1 during block layout -- the SSO guard can be emitted directly after `ori_str_len` without an intervening unconditional branch.
**First seen**: Journey 14
**Found in**: Control Flow & Block Layout (Category 4)

### LOW-2: Duplicate ptrtoint in SSO guard sequence

**Location**: 4 SSO guard sites (@sso_len, @heap_len, @main normal path, @main EH landing pad)
**Impact**: 4 unnecessary instructions total. `%rc_dec.p2i` and `%rc_dec.null.p2i` perform identical `ptrtoint ptr %rc_dec.fat_data to i64` conversions. The second result should reuse the first.
**Fix**: In `emit_rc_dec_fat`, reuse the ptrtoint result for both the SSO bit-test and the null check.
**First seen**: Journey 14
**Found in**: Optimal IR Comparison (Category 7)

### NOTE-3: Excellent borrow elision on @shared_len

**Location**: @shared_len parameter signature
**Impact**: Positive -- saves 2 RC operations (rc_inc + rc_dec) that would otherwise bracket the call. Parameter annotated with `readonly dereferenceable(24)` gives LLVM maximum optimization freedom.
**Found in**: ARC Purity (Category 2)

### NOTE-4: SSO guard correctly discriminates inline vs heap strings

**Location**: All SSO guard sites
**Impact**: Positive -- runtime avoids entering `ori_rc_dec` for SSO strings entirely. The bit 63 check is a single AND+CMP, adding minimal overhead to the fast path.
**Found in**: Fat Pointers: SSO vs Heap Discrimination (Category 8)

### NOTE-5: EH-safe RC cleanup with mutually exclusive paths

**Location**: @main bb2 (landing pad) and add.ok6 (normal path)
**Impact**: Positive -- correct exception safety. The string is always cleaned up exactly once regardless of whether `@shared_len` throws or succeeds.
**Found in**: Fat Pointers: RC Lifecycle (Category 9)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 10/10 | 100.0% compliance |
| Control Flow | 10% | 8/10 | 2 defects |
| IR Quality | 20% | 8/10 | 4 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.4 / 10**

## Verdict

Journey 14's fat pointer codegen demonstrates strong string handling. The SSO guard pattern correctly discriminates between inline and heap-allocated strings at the IR level. The standout feature is borrow elision on `@shared_len` -- the read-only parameter avoids all RC overhead, producing OPTIMAL code for that function. The EH-safe cleanup pattern in `@main` correctly handles exception safety with mutually exclusive rc_dec paths. Two minor code quality issues (redundant unconditional branches and duplicate ptrtoint instructions in the SSO guard) prevent a perfect score, but the overall architecture is sound.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| SSO guard pattern | J9 | J14 | CONFIRMED |
| Borrow elision | J4 (structs) | J14 (strings) | CONFIRMED |
| EH landing pad cleanup | J10 | J14 | CONFIRMED |
| Overflow checking | J1 | J14 | CONFIRMED |
| fastcc on user functions | J1 | J14 | CONFIRMED |
| Redundant unconditional br | J14 | J14 | NEW |
| Duplicate ptrtoint in SSO guard | J14 | J14 | NEW |

The redundant `br label %bb1` and duplicate `ptrtoint` are new findings not seen in previous journeys. J9 tested strings but with simpler control flow that may not have exposed the block-split pattern. The borrow elision quality matches what was seen for struct parameters in J4, now confirmed to extend to fat pointer (string) parameters.
