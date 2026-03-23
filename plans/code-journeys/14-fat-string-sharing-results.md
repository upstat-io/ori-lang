---
journey: 14
slug: fat-string-sharing
theme: "I am a fat pointer"
date: 2026-03-20
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
  - "See how aggregate loads replaced per-field GEP materialization for fat structs"
  - "Distinguish SSO strings (no heap, no RC) from heap strings (RC-managed data pointer)"

features:
  - strings
  - arc
  - function_calls
  - multiple_functions
feature_description: "Fat pointer string representation, SSO vs heap discrimination, borrow elision on read-only parameters, aggregate load optimization"

score: 10.0
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 10
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
  attr_applicable: 21
  attr_correct: 21
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
@shared_len: +0 rc_inc, +0 rc_dec (BORROW ELISION -- read-only param, no ownership transfer)
  - Parameter passed by ptr readonly -- no RC ops needed
@main: +0 rc_inc, +1 rc_dec (normal path only -- no EH landing pad needed)
  - Creates "long" string via ori_str_from_raw
  - Passes to shared_len via direct call (nounwind -- no EH overhead)
  - rc_dec in add.ok6 after all computation
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

**RC ops inserted**: 4 | **Elided**: 2 | **Net ops**: 2

<details>
<summary>ARC annotations</summary>

```text
@sso_len: +0 rc_inc, +1 rc_dec (SSO-guarded -- skipped at runtime for "hello")
@heap_len: +0 rc_inc, +1 rc_dec (SSO-guarded -- fires at runtime for 30-char heap string)
@shared_len: +0 rc_inc, +0 rc_dec (BORROW ELISION -- ptr readonly, no ownership)
@main: +0 rc_inc, +1 rc_dec (normal path only, no EH landing pad)
  Total: 3 rc_dec syntactic, all nounwind -- no EH overhead anywhere
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
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %sret.tmp, ptr @str, i64 5)
  %sret.load = load { i64, i64, ptr }, ptr %sret.tmp, align 8
  store { i64, i64, ptr } %sret.load, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  %0 = extractvalue { i64, i64, ptr } %sret.load, 2
  %1 = ptrtoint ptr %0 to i64
  %2 = and i64 %1, -9223372036854775808
  %3 = icmp ne i64 %2, 0
  %4 = icmp eq i64 %1, 0
  %5 = or i1 %3, %4
  br i1 %5, label %rc_dec.sso_skip, label %rc_dec.heap

rc_dec.heap:                                      ; preds = %bb0
  call void @ori_rc_dec(ptr %0, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip

rc_dec.sso_skip:                                  ; preds = %rc_dec.heap, %bb0
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @heap_len ---
define fastcc noundef i64 @_ori_heap_len() #0 {
bb0:
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %sret.tmp, ptr @str.1, i64 30)
  %sret.load = load { i64, i64, ptr }, ptr %sret.tmp, align 8
  store { i64, i64, ptr } %sret.load, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  %0 = extractvalue { i64, i64, ptr } %sret.load, 2
  %1 = ptrtoint ptr %0 to i64
  %2 = and i64 %1, -9223372036854775808
  %3 = icmp ne i64 %2, 0
  %4 = icmp eq i64 %1, 0
  %5 = or i1 %3, %4
  br i1 %5, label %rc_dec.sso_skip, label %rc_dec.heap

rc_dec.heap:                                      ; preds = %bb0
  call void @ori_rc_dec(ptr %0, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip

rc_dec.sso_skip:                                  ; preds = %rc_dec.heap, %bb0
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @shared_len ---
define fastcc noundef i64 @_ori_shared_len(ptr noundef nonnull readonly dereferenceable(24) %0) #0 {
bb0:
  %str.len = call i64 @ori_str_len(ptr %0)
  ret i64 %str.len
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  %call = call fastcc i64 @_ori_sso_len()
  %call1 = call fastcc i64 @_ori_heap_len()
  call void @ori_str_from_raw(ptr %sret.tmp, ptr @str.1, i64 30)
  %sret.load = load { i64, i64, ptr }, ptr %sret.tmp, align 8
  store { i64, i64, ptr } %sret.load, ptr %ref_arg, align 8
  %call2 = call fastcc i64 @_ori_shared_len(ptr %ref_arg)
  %0 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %1 = extractvalue { i64, i1 } %0, 0
  %2 = extractvalue { i64, i1 } %0, 1
  br i1 %2, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %1, i64 %call2)
  %add.val4 = extractvalue { i64, i1 } %add3, 0
  %add.ovf5 = extractvalue { i64, i1 } %add3, 1
  br i1 %add.ovf5, label %add.ovf_panic7, label %add.ok6

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok6:                                          ; preds = %add.ok
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %sret.load, 2
  %rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808
  %rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
  %rc_dec.is_null = icmp eq i64 %rc_dec.p2i, 0
  %rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.is_null
  br i1 %rc_dec.skip_rc, label %rc_dec.sso_skip, label %rc_dec.heap

add.ovf_panic7:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

rc_dec.heap:                                      ; preds = %add.ok6
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; RC-- str
  br label %rc_dec.sso_skip

rc_dec.sso_skip:                                  ; preds = %rc_dec.heap, %add.ok6
  ret i64 %add.val4
}

; Function Attrs: nounwind
declare void @ori_str_from_raw(ptr noalias sret({ i64, i64, ptr }), ptr, i64) #1

; Function Attrs: nounwind
declare i64 @ori_str_len(ptr) #1

; Function Attrs: cold nounwind uwtable
; --- drop str ---
define void @"_ori_drop$3"(ptr noundef %0) #2 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}

; Function Attrs: nounwind
declare void @ori_rc_free(ptr, i64, i64) #1

; Function Attrs: nounwind memory(inaccessiblemem: readwrite)
declare void @ori_rc_dec(ptr, ptr) #3

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #4

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #5

; Function Attrs: nounwind uwtable
define noundef i32 @main() #0 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}

; Function Attrs: nounwind
declare i32 @ori_check_leaks() #1

attributes #0 = { nounwind uwtable }
attributes #1 = { nounwind }
attributes #2 = { cold nounwind uwtable }
attributes #3 = { nounwind memory(inaccessiblemem: readwrite) }
attributes #4 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #5 = { cold noreturn }
```

#### Disassembly

```asm
_ori_sso_len:                        ; 144 bytes
  sub    $0x48,%rsp
  lea    @str(%rip),%rsi             ; "hello\0"
  lea    0x18(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw
  ; aggregate load from sret, store to str_len alloca
  lea    0x30(%rsp),%rdi
  call   ori_str_len
  ; SSO guard: check bit 63 of data ptr
  movabs $0x8000000000000000,%rdx
  ; ... setne/sete/or/test/jne pattern ...
  ; conditional ori_rc_dec
  ret

_ori_heap_len:                       ; 144 bytes
  ; [identical structure to @sso_len, with @str.1 (30 chars)]
  sub    $0x48,%rsp
  lea    @str.1(%rip),%rsi
  ; ... same SSO guard pattern ...
  ret

_ori_shared_len:                     ; 8 bytes
  push   %rax
  call   ori_str_len
  pop    %rcx
  ret

_ori_main:                           ; 241 bytes
  sub    $0x68,%rsp
  call   _ori_sso_len                ; a = 5
  ; save result
  call   _ori_heap_len               ; b = 30
  ; create "long" string via ori_str_from_raw
  ; aggregate load, store to ref_arg
  call   _ori_shared_len             ; c = 30 (direct call, no invoke)
  ; overflow-checked a + b
  add    %rcx,%rax
  jo     .overflow_panic
  ; overflow-checked (a+b) + c
  add    %rcx,%rax
  jo     .overflow_panic
  ; SSO-guarded rc_dec for "long" string (single path, no EH)
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @sso_len | 16 | 16 | 1.00x | OPTIMAL |
| 2 | @heap_len | 16 | 16 | 1.00x | OPTIMAL |
| 3 | @shared_len | 2 | 2 | 1.00x | OPTIMAL |
| 4 | @main | 30 | 30 | 1.00x | OPTIMAL |

Every function achieves OPTIMAL instruction count. Key features:

- **Aggregate load** (`load { i64, i64, ptr }`) replaces 9-instruction GEP+load+insertvalue chains
- **No EH infrastructure** anywhere (no `personality`, `invoke`, `landingpad`, `resume`)
- **Single ptrtoint** in SSO guard (no duplicate) saves 1 instruction per guard site
- **No redundant branches** (bb0/bb1 merged in sso_len/heap_len)
- **@shared_len reduced to 2 instructions** -- dead `%param.load` eliminated (was 3 in previous run)
- **@_ori_main now `nounwind`** -- nounwind analysis correctly propagates through all callees

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @sso_len | 0 | 1 | YES | N/A | N/A |
| @heap_len | 0 | 1 | YES | N/A | N/A |
| @shared_len | 0 | 0 | YES | 1 elided pair | 0 moves |
| @main | 0 | 1 | YES | 0 elided | 0 moves |

**Verdict**: All functions balanced. Zero violations. `ori_str_from_raw` implicitly creates the reference (rc=1 for heap strings), and each function decrements exactly once via the SSO-guarded `ori_rc_dec` path.

**Borrow elision on @shared_len** is excellent: the parameter is passed as `ptr noundef nonnull readonly dereferenceable(24)` -- no rc_inc on entry, no rc_dec on exit. The caller retains ownership; the callee borrows without touching the reference count. [NOTE-3]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @sso_len | YES | YES | N/A | N/A | NO | |
| @heap_len | YES | YES | N/A | N/A | NO | |
| @shared_len | YES | YES | N/A | YES (param) | NO | `noundef nonnull readonly deref(24)` on param |
| @main | NO (C) | YES | N/A | N/A | NO | C calling convention for entry point [NOTE-5] |
| @_ori_drop$3 | N/A | YES | N/A | N/A | YES | Correct cold annotation |
| @ori_str_from_raw | N/A | YES | YES (sret) | N/A | N/A | |
| @ori_rc_dec | N/A | YES | N/A | N/A | N/A | `memory(inaccessiblemem: readwrite)` |
| @ori_panic_cstr | N/A | N/A | N/A | N/A | YES | `cold noreturn` |

**Verdict**: 100% attribute compliance (21/21 checks pass). All user functions now have `nounwind uwtable` -- including @_ori_main, which previously lacked `nounwind`. The nounwind analysis correctly determines that `ori_panic_cstr` is `noreturn` (never unwinds) so all callers can be marked `nounwind`. @shared_len parameter has full attribute set including `readonly` and `dereferenceable(24)`. The C `main` wrapper also correctly has `nounwind`.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @sso_len | 3 | 0 | 0 | 0 | |
| @heap_len | 3 | 0 | 0 | 0 | |
| @shared_len | 1 | 0 | 0 | 0 | |
| @main | 7 | 0 | 0 | 0 | |

**@sso_len / @heap_len**: Clean 3-block diamond: bb0 -> rc_dec.heap/rc_dec.sso_skip. No redundant branches.

**@shared_len**: Single basic block. OPTIMAL.

**@main**: 7 blocks, all justified: bb0 (setup + calls), add.ok (second add), add.ovf_panic (first panic), add.ok6 (SSO guard entry), add.ovf_panic7 (second panic), rc_dec.heap (conditional dec), rc_dec.sso_skip (return). No unnecessary blocks or branches.

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
| .text section | 885.4 KiB |
| .rodata section | 133.8 KiB |
| User code | 537 bytes (4 user functions + drop + C wrapper) |
| Runtime | 99.9% of binary |

#### Disassembly: @sso_len

```asm
_ori_sso_len:                        ; 144 bytes
  sub    $0x48,%rsp
  lea    @str(%rip),%rsi             ; "hello\0"
  lea    0x18(%rsp),%rdi
  mov    $0x5,%edx
  call   ori_str_from_raw
  mov    0x28(%rsp),%rdx             ; load data ptr (field 2)
  mov    %rdx,0x8(%rsp)             ; save for SSO check
  mov    0x18(%rsp),%rax             ; aggregate fields -> str_len alloca
  mov    0x20(%rsp),%rcx
  mov    %rdx,0x40(%rsp)
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
```

#### Disassembly: @shared_len

```asm
_ori_shared_len:                     ; 8 bytes
  push   %rax                       ; align stack
  call   ori_str_len                ; rdi already points to caller's str
  pop    %rcx                       ; restore stack
  ret                               ; NO rc_inc, NO rc_dec -- borrow elision
```

#### Disassembly: @main

```asm
_ori_main:                           ; 241 bytes
  sub    $0x68,%rsp
  call   _ori_sso_len               ; a = 5
  mov    %rax,0x20(%rsp)
  call   _ori_heap_len              ; b = 30
  mov    %rax,0x18(%rsp)
  ; create "long" string
  lea    @str.1(%rip),%rsi
  lea    0x38(%rsp),%rdi
  mov    $0x1e,%edx
  call   ori_str_from_raw
  ; load fat pointer fields, store to ref_arg
  call   _ori_shared_len            ; c = 30 (direct call, no invoke)
  ; overflow-checked a + b
  add    %rcx,%rax
  jo     .overflow_panic
  ; overflow-checked (a+b) + c
  add    %rcx,%rax
  jo     .overflow_panic
  ; SSO-guarded rc_dec for "long" string (single path, no EH)
  ; ... bit 63 check pattern ...
  ret
```

### 7. Optimal IR Comparison

#### @sso_len: Ideal vs Actual

```llvm
; IDEAL (16 instructions) = ACTUAL (16 instructions)
define fastcc noundef i64 @_ori_sso_len() nounwind {
  %self = alloca { i64, i64, ptr }, align 8
  %sret = alloca { i64, i64, ptr }, align 8
  call void @ori_str_from_raw(ptr %sret, ptr @str, i64 5)
  %val = load { i64, i64, ptr }, ptr %sret, align 8
  store { i64, i64, ptr } %val, ptr %self, align 8
  %len = call i64 @ori_str_len(ptr %self)
  %data = extractvalue { i64, i64, ptr } %val, 2
  %p2i = ptrtoint ptr %data to i64
  %sso = and i64 %p2i, -9223372036854775808
  %is_sso = icmp ne i64 %sso, 0
  %is_null = icmp eq i64 %p2i, 0
  %skip = or i1 %is_sso, %is_null
  br i1 %skip, label %done, label %heap
heap:
  call void @ori_rc_dec(ptr %data, ptr @"_ori_drop$3")
  br label %done
done:
  ret i64 %len
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @shared_len: Ideal vs Actual

```llvm
; IDEAL (2 instructions) = ACTUAL (2 instructions)
define fastcc noundef i64 @_ori_shared_len(ptr noundef nonnull readonly dereferenceable(24) %0) nounwind {
  %len = call i64 @ori_str_len(ptr %0)
  ret i64 %len
}
```

**Delta**: +0 instructions. OPTIMAL. The dead `%param.load` (present in the previous run) has been eliminated. Zero RC ops, clean borrow semantics. Only 2 instructions -- the theoretical minimum for a function that delegates to a runtime call.

#### @main: Ideal vs Actual

```llvm
; IDEAL (30 instructions) = ACTUAL (30 instructions)
define noundef i64 @_ori_main() nounwind {
  %ref_arg = alloca { i64, i64, ptr }, align 8
  %sret = alloca { i64, i64, ptr }, align 8
  %call = call fastcc i64 @_ori_sso_len()
  %call1 = call fastcc i64 @_ori_heap_len()
  call void @ori_str_from_raw(ptr %sret, ptr @str.1, i64 30)
  %val = load { i64, i64, ptr }, ptr %sret, align 8
  store { i64, i64, ptr } %val, ptr %ref_arg, align 8
  %call2 = call fastcc i64 @_ori_shared_len(ptr %ref_arg)
  ; overflow-checked add (a + b): 4 instructions
  ; overflow-checked add ((a+b) + c): 4 instructions
  ; overflow panic x2: 4 instructions
  ; SSO guard (extractvalue, ptrtoint, and, icmp, icmp, or, br): 7 instructions
  ; rc_dec + br: 2 instructions
  ; ret: 1 instruction
}
```

**Delta**: +0 instructions. OPTIMAL. @_ori_main now correctly carries `nounwind` (previously missing).

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @sso_len | 16 | 16 | +0 | N/A | OPTIMAL |
| @heap_len | 16 | 16 | +0 | N/A | OPTIMAL |
| @shared_len | 2 | 2 | +0 | N/A | OPTIMAL |
| @main | 30 | 30 | +0 | N/A | OPTIMAL |

### 8. Fat Pointers: SSO vs Heap Discrimination

The SSO guard pattern correctly discriminates between inline (SSO) and heap-allocated strings:

**Guard sequence** (7 instructions per site):
1. `extractvalue` -- extract `data` pointer from fat struct field 2
2. `ptrtoint` -- convert to integer for bit inspection (single conversion, reused for both checks)
3. `and i64 %p2i, 0x8000000000000000` -- isolate bit 63 (SSO flag)
4. `icmp ne` -- check if SSO flag is set
5. `icmp eq i64 %p2i, 0` -- check for null pointer (reuses `%p2i` from step 2)
6. `or i1` -- skip RC if SSO OR null
7. `br i1` -- conditional branch

**SSO semantics**: For "hello" (5 chars, under the 23-byte SSO threshold), `ori_str_from_raw` stores the data inline in the `{len, cap, data}` struct with bit 63 set in the `data` field. The guard detects this and skips `ori_rc_dec`. For "abcdefghijklmnopqrstuvwxyz1234" (30 chars, above SSO threshold), the data is heap-allocated and the guard falls through to `ori_rc_dec`.

### 9. Fat Pointers: Borrow Elision and Dead Code Elimination

**@shared_len** demonstrates two important optimizations working together:

1. **Borrow elision**: The parameter is annotated `ptr noundef nonnull readonly dereferenceable(24)`. The callee never takes ownership, so zero RC operations are emitted. The caller retains ownership and is responsible for cleanup.

2. **Dead code elimination**: The previous run included a dead `%param.load = load { i64, i64, ptr }, ptr %0, align 8` -- the fat struct was loaded but never used (only the pointer was passed to `ori_str_len`). This has now been eliminated, reducing @shared_len from 3 instructions to 2. This is the theoretical minimum: one call to the runtime function, one return.

The native code reflects this perfectly: `push %rax` (stack alignment), `call ori_str_len`, `pop %rcx`, `ret` -- 4 native instructions, 8 bytes total.

### 10. Fat Pointers: Complete Nounwind Propagation

The nounwind fixed-point analysis now correctly marks ALL user functions as `nounwind`:

| Function | Previous | Current | Reason |
|----------|----------|---------|--------|
| @sso_len | nounwind | nounwind | No unwinding callees |
| @heap_len | nounwind | nounwind | No unwinding callees |
| @shared_len | nounwind | nounwind | Only calls `ori_str_len` (nounwind) |
| @_ori_main | missing | **nounwind** | `ori_panic_cstr` is `noreturn` -- never unwinds |
| @main (C wrapper) | missing | **nounwind** | Only calls @_ori_main (nounwind) + @ori_check_leaks (nounwind) |

The key insight: `ori_panic_cstr` is declared `cold noreturn`. A function that never returns also never unwinds, so callers that only "throw" via `noreturn` paths can be correctly marked `nounwind`. The analysis log confirms: `nounwind_count=4` with 2 passes to reach fixed-point.

This eliminates all EH overhead: no `personality` declaration, no `invoke`/`landingpad`/`resume`, no duplicate SSO guards on exception paths.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE | ARC | Excellent borrow elision on @shared_len | CONFIRMED | J14 |
| 2 | NOTE | Fat Pointers | SSO guard correctly discriminates inline vs heap strings | CONFIRMED | J14 |
| 3 | NOTE | IR Quality | Aggregate load replaces per-field GEP chain (9:1 reduction) | CONFIRMED | J14 |
| 4 | NOTE | Control Flow | EH infrastructure eliminated via nounwind analysis | CONFIRMED | J14 |
| 5 | NOTE | Attributes | @_ori_main and C main now correctly marked nounwind | NEW | J14 |
| 6 | NOTE | IR Quality | Dead %param.load in @shared_len eliminated (3->2 instructions) | NEW | J14 |
| 7 | NOTE | IR Quality | Duplicate ptrtoint in SSO guard eliminated | FIXED | J14 |
| 8 | NOTE | Control Flow | Redundant unconditional br in sso_len/heap_len eliminated | FIXED | J14 |

### NOTE-1: Excellent borrow elision on @shared_len

**Location**: @shared_len parameter signature
**Impact**: Positive -- saves 2 RC operations (rc_inc + rc_dec) that would otherwise bracket the call. Parameter annotated with `readonly dereferenceable(24)` gives LLVM maximum optimization freedom. The native code compiles to just 4 instructions (push, call, pop, ret).
**Found in**: ARC Purity (Category 2)

### NOTE-2: SSO guard correctly discriminates inline vs heap strings

**Location**: All SSO guard sites (3 total: sso_len, heap_len, main)
**Impact**: Positive -- runtime avoids entering `ori_rc_dec` for SSO strings entirely. The bit 63 check is a single AND+CMP, adding minimal overhead to the fast path. Uses a single `ptrtoint` reused for both checks.
**Found in**: Fat Pointers: SSO vs Heap Discrimination (Category 8)

### NOTE-3: Aggregate load replaces per-field GEP materialization

**Location**: All fat struct load sites (@sso_len, @heap_len, @main)
**Impact**: Positive -- 9:1 instruction reduction per materialization site. A single `load { i64, i64, ptr }` replaces the 3-field GEP+load+insertvalue chain.
**Found in**: Fat Pointers: SSO vs Heap Discrimination (Category 8)

### NOTE-4: EH infrastructure eliminated via nounwind analysis

**Location**: All user functions
**Impact**: Positive -- eliminates all exception handling machinery. No `personality`, `invoke`, `landingpad`, `resume`, or duplicate SSO guards on exception paths.
**Found in**: Fat Pointers: Complete Nounwind Propagation (Category 10)

### NOTE-5: @_ori_main and C main now correctly marked nounwind

**Location**: @_ori_main and @main function declarations
**Impact**: Positive -- previously @_ori_main used `{ uwtable }` (missing `nounwind`). Now correctly uses `{ nounwind uwtable }`. The nounwind analysis recognizes that `ori_panic_cstr` is `noreturn` and therefore cannot unwind. This allows LLVM to eliminate unwind tables for @_ori_main, reducing binary overhead.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-6: Dead %param.load in @shared_len eliminated

**Location**: @shared_len function body
**Impact**: Positive -- the previous run included `%param.load = load { i64, i64, ptr }, ptr %0, align 8` which was dead code (loaded but never used). Now eliminated, reducing @shared_len from 3 instructions to 2 -- the theoretical minimum for a delegating call.
**Found in**: Fat Pointers: Borrow Elision and Dead Code Elimination (Category 9)

### NOTE-7: Duplicate ptrtoint in SSO guard eliminated (FIXED)

**Location**: Previously in all SSO guard sites
**Impact**: Previously LOW -- 1 unnecessary instruction per guard site. Now FIXED: the SSO guard reuses the single `%p2i` result for both the bit-63 check and the null check.
**Found in**: Optimal IR Comparison (Category 7)

### NOTE-8: Redundant unconditional br in sso_len/heap_len eliminated (FIXED)

**Location**: Previously in @sso_len bb0->bb1 and @heap_len bb0->bb1
**Impact**: Previously LOW -- 1 unnecessary instruction per function. Now FIXED: bb0 and bb1 merged.
**Found in**: Control Flow & Block Layout (Category 4)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 10/10 | 100.0% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 10.0 / 10**

## Verdict

Journey 14's fat pointer codegen maintains its perfect 10.0 score with two further improvements since the last run: (1) @_ori_main and the C main wrapper now correctly carry `nounwind`, completing the nounwind propagation across the entire call graph; (2) the dead `%param.load` in @shared_len has been eliminated, reducing it to the theoretical minimum of 2 instructions. All functions achieve OPTIMAL instruction ratios (1.00x), ARC is perfectly balanced with excellent borrow elision, and the SSO guard pattern correctly discriminates inline from heap strings with zero unnecessary overhead.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| SSO guard pattern | J9 | J14 | CONFIRMED |
| Borrow elision | J4 (structs) | J14 (strings) | CONFIRMED |
| Overflow checking | J1 | J14 | CONFIRMED |
| fastcc on user functions | J1 | J14 | CONFIRMED |
| Aggregate load optimization | J9 | J14 | CONFIRMED |
| Nounwind fixed-point analysis | J14 | J14 | CONFIRMED |
| @_ori_main nounwind | J14 | J14 | NEW (previously missing) |
| Dead param.load elimination | J14 | J14 | NEW (previously 3 instructions) |
| Redundant unconditional br | J14 (prev) | J14 | FIXED |
| Duplicate ptrtoint in SSO guard | J14 (prev) | J14 | FIXED |

The nounwind propagation is now complete: every user function in this journey carries `nounwind`, including @_ori_main which previously lacked it. The dead code elimination in @shared_len demonstrates the compiler's improving ability to avoid generating unnecessary instructions. Both issues found in earlier Journey 14 runs (redundant `br label %bb1` and duplicate `ptrtoint`) remain FIXED.
