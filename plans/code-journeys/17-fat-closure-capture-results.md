---
journey: 17
slug: fat-closure-capture
theme: "I am a captured fat pointer"
date: 2026-03-16
status: FAIL_AOT
expected: 10
eval_result: 10
aot_result: 1

difficulty: complex
prerequisites:
  - "Understanding of closures and variable capture"
  - "Familiarity with fat pointer representations (str = {len, cap, ptr})"
  - "Knowledge of ARC memory management for heap-allocated values"
  - "Understanding of monomorphization and type variable resolution"
learning_objectives:
  - "See how closure capture of fat-pointer types (str) triggers unresolved type variables at codegen"
  - "Understand the difference between scalar capture (int) and fat-pointer capture (str) in closure environments"
  - "Observe how monomorphization fails to resolve method dispatch on captured values"
  - "Identify the root cause chain: type variable leak -> missing mono instance -> LLVM IR verification failure"

features:
  - strings
  - arc
  - closures
  - capture
  - higher_order
feature_description: "Closure capturing a str (fat pointer) value, calling .length() on captured and parameter strings"

score: 3.0
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 9
  control_flow: 10
  ir_quality: 10
  binary_quality: 0
  other_findings: 7
score_metrics:
  instruction_ratio: 1.00
  instruction_ratio_max: 1.00
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 24
  attr_correct: 23
  attr_has_wrong: false
  cf_defects: 0
  cf_incorrect: false
  ir_unjustified: 0
  ir_incorrect: false
  bin_defects: 4
  bin_hard_fail: true
  other_critical: 1
  other_high: 0
  other_low: 0
overflow_check: PASS

bugs_found:
  - id: C17
    severity: CRITICAL
    description: "Closure capturing str (fat pointer) produces unresolved type variable Idx(202) at LLVM codegen, causing IR verification failure"
    status: OPEN
    found_in: journey17

related_journeys:
  - journey: 5
    relationship: "Both test closures with capture; J5 captures int (scalar), J17 captures str (fat pointer)"
  - journey: 9
    relationship: "Both test str operations and .length(); J9 passes without closures, J17 fails when str is captured"
---

# Journey 17: "I am a captured fat pointer"

## Source

```ori
// Journey 17: "I am a captured fat pointer"
// Slug: fat-closure-capture
// Difficulty: complex
// Features: strings, arc, closures, capture, higher_order
// Expected: check_capture() = 10
// NOTE: This journey exposes a compiler bug — closure capturing str
//       triggers unresolved type variable at codegen (Idx leak)

@check_capture () -> int = {
    let prefix = "hello";
    let f = s -> prefix.length() + s.length();
    f("world")
}

@main () -> int = check_capture();
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 10        | 10       | (none) | (none) | PASS   |
| AOT     | 1         | 10       | (none) | unresolved type variable Idx(202); LLVM IR verification failed | FAIL   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 62 | **Keywords**: 4 | **Identifiers**: 14 | **Errors**: 0

<details>
<summary>Token stream</summary>

```text
Fn(@) Ident(check_capture) LParen RParen Arrow Ident(int) Eq
LBrace Let Ident(prefix) Eq Str("hello") Semi
Let Ident(f) Eq Ident(s) Arrow Ident(prefix) Dot Ident(length)
LParen RParen Plus Ident(s) Dot Ident(length) LParen RParen Semi
Ident(f) LParen Str("world") RParen
RBrace Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq
Ident(check_capture) LParen RParen Semi
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 14 | **Max depth**: 4 | **Functions**: 2 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @check_capture
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let prefix = Str("hello")
│       ├─ Let f = Lambda(s)
│       │       └─ BinOp(+)
│       │            ├─ MethodCall(prefix.length())
│       │            └─ MethodCall(s.length())
│       └─ Call(f, Str("world"))
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Call(@check_capture)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 12 | **Types inferred**: 8 | **Unifications**: 10 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@check_capture () -> int = {
    let prefix: str = "hello"              // inferred: str
    let f: (str) -> int = s -> prefix.length() + s.length()
    //  ^ inferred: (str) -> int           // s: str, .length(): int
    f("world")                             // -> int
}

@main () -> int = check_capture()          // -> int
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 3 | **Desugared**: 1 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Lambda `s -> prefix.length() + s.length()` lowered to closure with capture set {prefix: str}
- Method calls `.length()` resolved to str.length built-in
- Block expression lowered: let bindings → sequential statements, final expression = result
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 5 | **Elided**: 0 | **Net ops**: 5

<details>
<summary>ARC annotations</summary>

```text
@check_capture: +3 rc_inc (str alloc, closure env alloc, str alloc), +1 rc_dec (closure env)
  - prefix: str allocated via ori_str_from_raw → rc_inc implicit
  - closure env: ori_rc_alloc for capture environment
  - "world": str allocated via ori_str_from_raw → rc_inc implicit
  - closure env: rc_dec after call (ownership transfer to callee)
@__lambda_0: +0 rc_inc, +2 rc_dec (captured str, parameter str)
  - consumes captured prefix (rc_dec on str data pointer)
  - consumes parameter s (rc_dec via drop$202 — BUG: uses i64 instead of ptr)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 10 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  └─ @check_capture()
       ├─ let prefix = "hello"           // str
       ├─ let f = s -> prefix.length() + s.length()  // closure captures prefix
       └─ f("world")
            ├─ prefix.length() = 5       // captured str
            ├─ s.length() = 5            // parameter str
            └─ 5 + 5 = 10
→ 10
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> **This journey fails at LLVM IR verification due to an unresolved type variable.**

#### ARC Pipeline

**RC ops inserted**: 5 | **Elided**: 0 | **Net ops**: 5

<details>
<summary>ARC annotations</summary>

```text
@check_capture: +3 rc_inc, +1 rc_dec (closure env deallocated after indirect call)
@__lambda_0: +0 rc_inc, +2 rc_dec (BUG: second rc_dec uses i64 instead of ptr — type mismatch)
@partial_0_drop: +0 rc_inc, +1 rc_dec (captured str freed when closure is dropped)
@partial_1: +0 rc_inc, +0 rc_dec (forwarding thunk)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '17-fat-closure-capture'
source_filename = "17-fat-closure-capture"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@str = private unnamed_addr constant [6 x i8] c"hello\00", align 1
@str.1 = private unnamed_addr constant [6 x i8] c"world\00", align 1

; Function Attrs: nounwind uwtable
; --- @check_capture ---
define fastcc noundef i64 @_ori_check_capture() #0 {
bb0:
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
  %env.data = call ptr @ori_rc_alloc(i64 32, i64 8)
  %env.drop_fn = getelementptr inbounds nuw { ptr, { i64, i64, ptr } }, ptr %env.data, i32 0, i32 0
  store ptr @_ori_partial_0_drop, ptr %env.drop_fn, align 8
  %env.cap.0 = getelementptr inbounds nuw { ptr, { i64, i64, ptr } }, ptr %env.data, i32 0, i32 1
  store { i64, i64, ptr } %str.val.s2, ptr %env.cap.0, align 8
  %partial_apply.1 = insertvalue { ptr, ptr } { ptr @_ori_partial_1, ptr undef }, ptr %env.data, 1
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
  %closure.fn_ptr = extractvalue { ptr, ptr } %partial_apply.1, 0
  %closure.env_ptr = extractvalue { ptr, ptr } %partial_apply.1, 1
  %icall.arg.tmp = alloca { i64, i64, ptr }, align 8
  store { i64, i64, ptr } %str.val.s210, ptr %icall.arg.tmp, align 8
  %icall = call i64 %closure.fn_ptr(ptr %closure.env_ptr, ptr %icall.arg.tmp)
  %rc_dec.env = extractvalue { ptr, ptr } %partial_apply.1, 1
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.env to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  br i1 %rc_dec.null, label %rc_dec.skip, label %rc_dec.do

rc_dec.do:
  %rc_dec.drop_fn = load ptr, ptr %rc_dec.env, align 8
  call void @ori_rc_dec(ptr %rc_dec.env, ptr %rc_dec.drop_fn)  ; RC--
  br label %rc_dec.skip

rc_dec.skip:
  ret i64 %icall
}

; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_check_capture()
  ret i64 %call
}

; --- @__lambda_0 ---
define fastcc noundef i64 @_ori___lambda_0(ptr noundef nonnull dereferenceable(24) %0, i64 noundef %1) #1 {
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
  br label %bb1

bb1:
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %param.load.s2, 2
  %rc_dec.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.sso_flag = and i64 %rc_dec.p2i, -9223372036854775808
  %rc_dec.is_sso = icmp ne i64 %rc_dec.sso_flag, 0
  %rc_dec.null.p2i = ptrtoint ptr %rc_dec.fat_data to i64
  %rc_dec.null = icmp eq i64 %rc_dec.null.p2i, 0
  %rc_dec.skip_rc = or i1 %rc_dec.is_sso, %rc_dec.null
  br i1 %rc_dec.skip_rc, label %bb3, label %rc_dec.heap

bb3:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %str.len, i64 0)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

rc_dec.heap:
  call void @ori_rc_dec(ptr %rc_dec.fat_data, ptr @"_ori_drop$3")  ; RC-- str
  br label %bb3

add.ok:
  call void @ori_rc_dec(i64 %1, ptr @"_ori_drop$202")  ; RC-- forall t13   <-- BUG: i64 instead of ptr
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; --- drop str ---
define void @"_ori_drop$3"(ptr noundef %0) #3 {
entry:
  call void @ori_rc_free(ptr %0, i64 24, i64 8)
  ret void
}

; --- drop forall t13 ---
define void @"_ori_drop$202"(ptr noundef %0) #3 {
entry:
  call void @ori_rc_free(ptr %0, i64 8, i64 8)
  ret void
}

; --- @partial_0_drop ---
define void @_ori_partial_0_drop(ptr noundef %0) #3 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, { i64, i64, ptr } }, ptr %0, i32 0, i32 1
  %cap.0.f0.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %cap.0.ptr, i32 0, i32 0
  %cap.0.f0 = load i64, ptr %cap.0.f0.ptr, align 8
  %cap.0.s0 = insertvalue { i64, i64, ptr } zeroinitializer, i64 %cap.0.f0, 0
  %cap.0.f1.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %cap.0.ptr, i32 0, i32 1
  %cap.0.f1 = load i64, ptr %cap.0.f1.ptr, align 8
  %cap.0.s1 = insertvalue { i64, i64, ptr } %cap.0.s0, i64 %cap.0.f1, 1
  %cap.0.f2.ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %cap.0.ptr, i32 0, i32 2
  %cap.0.f2 = load ptr, ptr %cap.0.f2.ptr, align 8
  %cap.0.s2 = insertvalue { i64, i64, ptr } %cap.0.s1, ptr %cap.0.f2, 2
  %rc.data_ptr = extractvalue { i64, i64, ptr } %cap.0.s2, 2
  %rc_str.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.sso_flag = and i64 %rc_str.p2i, -9223372036854775808
  %rc_str.is_sso = icmp ne i64 %rc_str.sso_flag, 0
  %rc_str.null.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.null = icmp eq i64 %rc_str.null.p2i, 0
  %rc_str.skip_rc = or i1 %rc_str.is_sso, %rc_str.null
  %rc.str_safe_ptr = select i1 %rc_str.skip_rc, ptr null, ptr %rc.data_ptr
  call void @ori_rc_dec(ptr %rc.str_safe_ptr, ptr @"_ori_drop$3")  ; RC-- str
  call void @ori_rc_free(ptr %0, i64 32, i64 8)
  ret void
}

; --- @partial_1 ---
define noundef i64 @_ori_partial_1(ptr noundef %0, i64 noundef %1) #1 {
entry:
  %cap.0.ptr = getelementptr inbounds nuw { ptr, { i64, i64, ptr } }, ptr %0, i32 0, i32 1
  %result = call fastcc i64 @_ori___lambda_0(ptr %cap.0.ptr, i64 %1)
  ret i64 %result
}

; --- C main() entry point ---
define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}
```

#### Disassembly

Binary compilation failed (LLVM IR verification error) -- no disassembly available.

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @check_capture | 41 | 41 | 1.00x | OPTIMAL |
| 2 | @main | 2 | 2 | 1.00x | OPTIMAL |
| 3 | @__lambda_0 | 31 | 31 | 1.00x | OPTIMAL |
| 4 | @partial_0_drop | 21 | 21 | 1.00x | OPTIMAL |
| 5 | @partial_1 | 3 | 3 | 1.00x | OPTIMAL |

Note: Instruction counts reflect the generated IR as-is. However, the IR is **semantically incorrect** -- `@__lambda_0` passes `i64 %1` to `ori_rc_dec(ptr, ptr)` where a `ptr` is expected. The instruction count analysis is moot because the module fails verification.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @check_capture | 3 | 1 | NO | N/A | ownership transfer to lambda |
| @main | 0 | 0 | YES | N/A | N/A |
| @__lambda_0 | 0 | 2 | NO | N/A | consumes captured + param (BUG on param) |
| @partial_0_drop | 0 | 1 | NO | N/A | drop function for closure env |
| @partial_1 | 0 | 0 | YES | N/A | forwarding thunk |

**Verdict**: Module-level balance appears correct (3 inc from str/env allocations, 4 dec from lambda+drop). However, the second `rc_dec` in `@__lambda_0` is **semantically broken**: it calls `ori_rc_dec(i64 %1, ptr @"_ori_drop$202")` where `%1` is `i64` but `ori_rc_dec` expects `ptr`. This is a type error that would cause undefined behavior if the module were accepted. [CRITICAL-1]

Additionally, the `add` instruction computes `%str.len + 0` -- the second operand should be the result of `s.length()` but the `length` method on the parameter `s` was never called. The codegen emitted `i64 0` as a fallback for the unresolved method. [CRITICAL-2]

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @check_capture | YES | YES | N/A | N/A | NO | |
| @main | NO (C) | NO | N/A | N/A | NO | entry point -- C calling convention correct |
| @__lambda_0 | YES | NO | N/A | N/A | NO | [LOW-1] |
| @partial_0_drop | NO | YES | N/A | N/A | YES | drop function -- cold correct |
| @partial_1 | NO | NO | N/A | N/A | NO | closure thunk -- C ABI for indirect calls |

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @check_capture | 3 | 0 | 0 | 0 | clean: bb0 -> rc_dec.do/rc_dec.skip |
| @main | 1 | 0 | 0 | 0 | |
| @__lambda_0 | 6 | 0 | 0 | 0 | SSO guard + overflow check blocks |
| @partial_0_drop | 1 | 0 | 0 | 0 | |
| @partial_1 | 1 | 0 | 0 | 0 | |

Control flow structure is clean. No redundant branches or empty blocks.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (prefix.length() + s.length()) | YES | PARTIAL | Uses llvm.sadd.with.overflow but second operand is 0 (bug) |

The overflow check infrastructure is present, but the second operand to the addition is `i64 0` instead of the result of `s.length()`. This is because the `length` method call on the parameter `s` (whose type is the unresolved `forall t13`) was never emitted.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | N/A (compilation failed) |
| .text section | N/A |
| .rodata section | N/A |
| User code | N/A |
| Runtime | N/A |

**Hard fail**: LLVM IR verification rejected the module before native code generation. Error: `"Call parameter type does not match function signature! i64 %1 / ptr -- call void @ori_rc_dec(i64 %1, ptr @"_ori_drop$202")"`.

No disassembly is available.

### 7. Optimal IR Comparison

#### @check_capture: Ideal vs Actual

The actual IR for `@check_capture` is structurally sound -- it correctly:
- Allocates the "hello" string via `ori_str_from_raw`
- Creates a closure environment via `ori_rc_alloc` with the captured str
- Stores the drop function pointer and captured value into the environment
- Allocates the "world" string
- Performs an indirect call through the closure
- Decrements the closure environment RC after the call

```llvm
; IDEAL (simplified, ~35 instructions)
define fastcc i64 @_ori_check_capture() nounwind {
  ; allocate "hello" str
  ; create closure env {drop_fn, captured_str}
  ; allocate "world" str
  ; indirect call through closure(env, "world")
  ; rc_dec closure env
  ; ret result
}
```

The actual code matches this structure well. No unjustified overhead.

#### @__lambda_0: Ideal vs Actual (THE BUG)

```llvm
; IDEAL (should be ~25 instructions)
define fastcc i64 @_ori___lambda_0(ptr %captured_prefix, ptr %param_s) nounwind {
  ; load captured prefix str from %captured_prefix
  ; call ori_str_len(prefix) -> %len1
  ; rc_dec prefix str
  ; load param s str from %param_s (or receive as {i64, i64, ptr})
  ; call ori_str_len(s) -> %len2
  ; rc_dec s str
  ; add %len1, %len2 with overflow check
  ; ret result
}
```

```llvm
; ACTUAL (31 instructions -- semantically broken)
define fastcc i64 @_ori___lambda_0(ptr %0, i64 %1) {
  ; loads captured prefix from %0 -- CORRECT
  ; calls ori_str_len(prefix) -- CORRECT
  ; rc_dec on prefix str data ptr -- CORRECT
  ; MISSING: ori_str_len call on parameter s
  ; adds %str.len + 0 -- WRONG (should be %str.len + %s_len)
  ; rc_dec(i64 %1, ...) -- TYPE ERROR: %1 is i64, needs ptr
  ; ret
}
```

**Root cause chain**:
1. The lambda parameter `s` has type `str` (a fat pointer: `{i64, i64, ptr}`), but the codegen receives it as `i64 %1` -- an unresolved type variable `Idx(202)` was not resolved to `str` during monomorphization
2. Because the type is unresolved, the method call `s.length()` cannot be dispatched -- codegen emits `i64 0` as fallback
3. The ARC cleanup for parameter `s` calls `ori_rc_dec(i64 %1, ptr @"_ori_drop$202")` where `%1` is `i64` but `ori_rc_dec` expects `(ptr, ptr)` -- LLVM IR verification catches this type mismatch
4. The drop function `_ori_drop$202` uses `ori_rc_free(ptr, i64 8, i64 8)` which is sized for `i64` (8 bytes), not for `str` (24 bytes) -- further evidence the type was never resolved

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @check_capture | 41 | 41 | +0 | N/A | OPTIMAL |
| @main | 2 | 2 | +0 | N/A | OPTIMAL |
| @__lambda_0 | ~25 | 31 | N/A | N/A | BROKEN (unresolved type) |
| @partial_0_drop | 21 | 21 | +0 | N/A | OPTIMAL |
| @partial_1 | 3 | 3 | +0 | N/A | OPTIMAL |

### 8. Fat Pointers: Closure Capture Bug

The core defect is a **type variable leak from the type checker into LLVM codegen**. The lambda parameter `s` is inferred as `str` by the type checker (confirmed by the eval path producing the correct result of 10), but the monomorphization pass fails to propagate this resolution into the closure body for the LLVM backend.

**Evidence from traces**:

1. **Type checker succeeds**: `body checking complete` with 0 errors, 2 functions, all types inferred correctly
2. **Codegen ERROR**: `unresolved type variable at codegen -- type inference bug idx=Idx(202)` -- this `Idx(202)` is the type index for the lambda parameter `s` that should have been resolved to `str` (Idx 3 in the type table)
3. **Method WARN**: `unresolved function 'length' in invoke -- missing mono instance?` -- because the type is unresolved, method dispatch for `.length()` on `s` fails
4. **LLVM verification failure**: The `ori_rc_dec` call receives `i64` instead of `ptr`

**Hypothesis**: When a closure captures a fat-pointer type (str), the monomorphization or type lowering pass creates a fresh type variable for the closure's parameter but fails to unify it with the concrete type. The captured variable `prefix` is correctly typed (its `rc_dec` uses `ptr` and `_ori_drop$3`), but the lambda parameter `s` retains the unresolved variable. This suggests the bug is specifically in how captured closures propagate type information to their parameters -- non-capturing closures (Journey 5) and direct str operations (Journey 9) work correctly.

**Key difference from J5**: Journey 5 captures an `int` (scalar, 8 bytes). Journey 17 captures a `str` (fat pointer, 24 bytes: `{i64, i64, ptr}`). The fat-pointer representation requires different ABI handling (pass-by-pointer vs pass-by-value), and this is where the type resolution breaks down.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | CRITICAL | Fat Pointers: Closure Capture | Unresolved type variable Idx(202) at codegen: lambda parameter typed as i64 instead of str | NEW | J17 |
| 2 | CRITICAL | Fat Pointers: Closure Capture | Missing s.length() call: addition uses 0 as second operand | NEW | J17 |
| 3 | CRITICAL | Binary Analysis | AOT compilation fails with LLVM IR verification error: i64 passed to ptr parameter | NEW | J17 |
| 4 | LOW | Attributes | Missing nounwind on @__lambda_0 | CONFIRMED | J1 |

### CRITICAL-1: Unresolved type variable Idx(202) leaks to LLVM codegen

**Location**: `@_ori___lambda_0`, parameter `%1`
**Impact**: Lambda parameter `s: str` is lowered as `i64` instead of `{i64, i64, ptr}`. This causes cascading failures: method dispatch fails, RC cleanup uses wrong type, LLVM IR verification rejects the module.
**Root cause**: Monomorphization does not resolve the type variable for closure parameters when the closure captures a fat-pointer type. The type checker correctly infers `s: str`, but this resolution is lost before codegen.
**Fix**: The monomorphization pass (or type lowering) must propagate concrete types for all closure parameters, not just the captured variables. The closure's function type `(str) -> int` must be fully resolved before codegen begins.
**First seen**: Journey 17
**Found in**: Fat Pointers: Closure Capture Bug (Category 8)

### CRITICAL-2: Missing s.length() method call

**Location**: `@_ori___lambda_0`, `%add = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %str.len, i64 0)`
**Impact**: The second operand to the addition is `0` instead of the result of `s.length()`. Even if the type mismatch were somehow bypassed, the computed result would be wrong (5 + 0 = 5 instead of 5 + 5 = 10).
**Root cause**: Because the type of `s` is unresolved (`forall t13`), the method `length` cannot be dispatched. The codegen emits a `WARN` and falls back to `i64 0`.
**First seen**: Journey 17
**Found in**: Fat Pointers: Closure Capture Bug (Category 8)

### CRITICAL-3: LLVM IR verification failure

**Location**: `@_ori___lambda_0`, line `call void @ori_rc_dec(i64 %1, ptr @"_ori_drop$202")`
**Impact**: The LLVM module is rejected. No binary is produced. AOT compilation fails completely.
**Root cause**: `ori_rc_dec` is declared as `(ptr, ptr) -> void` but called with `(i64, ptr)`. The `i64` comes from the unresolved type variable -- the parameter should be a `ptr` (the data pointer of the str fat pointer).
**First seen**: Journey 17
**Found in**: Binary Analysis (Category 6)

### LOW-1: Missing nounwind on @__lambda_0

**Location**: `@_ori___lambda_0` function declaration
**Impact**: LLVM generates unnecessary exception handling tables
**Fix**: Add `nounwind` attribute to all non-unwinding functions
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 9/10 | 95.8% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 0/10 | 4 defects |
| Other Findings | 15% | 7/10 | 1 critical |

**Overall: 3.0 / 10**

Gates applied:
- bin_hard_fail_gate: wrong output, crash, or eval/AOT mismatch, score forced to 0
- global_gate: binary_quality == 0, overall capped at 3.0

## Verdict

Journey 17 exposes a critical compiler bug: closure capture of fat-pointer types (str) causes unresolved type variables to leak into LLVM codegen. The interpreter handles this correctly (exit code 10), but the AOT path fails with an LLVM IR verification error. The root cause is in monomorphization -- the lambda parameter's type is not resolved from its type variable to the concrete `str` type before code generation. This is distinct from Journey 5 (which captures int, a scalar) and Journey 9 (which uses str without closures). The bug is specific to the intersection of closures and fat-pointer capture.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Closures (scalar capture) | J5 | J17 | CONFIRMED (works) |
| String operations (.length()) | J9 | J17 | REGRESSED (fails when str is captured in closure) |
| Overflow checking | J1 | J17 | CONFIRMED (infrastructure present) |
| fastcc usage | J1 | J17 | CONFIRMED |
| ARC for strings (SSO guard) | J9 | J17 | CONFIRMED (works for captured prefix, broken for param) |

The key regression pattern: str works in direct use (J9) and closures work with scalar capture (J5), but the combination of str + closure capture fails. This is a classic cross-feature interaction bug -- each feature works individually but their intersection exposes a monomorphization gap.
