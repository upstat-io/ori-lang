---
journey: 12
slug: options
theme: "I am an option"
date: 2026-03-19
status: PASS
expected: 33
eval_result: 33
aot_result: 33
difficulty: complex
prerequisites:
  - "Understanding of sum types (Option/Some/None)"
  - "Pattern matching fundamentals"
  - "Function calls and composition"
  - "Error propagation with the ? operator"
learning_objectives:
  - "See how Option<int> is lowered to a tagged union {i64, i64} in LLVM IR"
  - "Understand how the ? operator compiles to branch-on-discriminant"
  - "Compare ideal vs actual codegen for match expressions on Option"
  - "Observe zero ARC overhead for scalar Option values"
features:
  - option_type
  - pattern_matching
  - error_propagation
  - function_calls
feature_description: "Option type construction, pattern matching, ? operator propagation, and function composition"
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
  attr_applicable: 38
  attr_correct: 38
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
  - journey: 6
    relationship: "Both test pattern matching; J6 covers sum types, J12 covers Option specifically"
  - journey: 2
    relationship: "Both test branching; J12 uses if/then/else for safe_div"
---

# Journey 12: "I am an option"

## Source

```ori
// Journey 12: "I am an option"
// Slug: options
// Difficulty: complex
// Features: option_type, pattern_matching, error_propagation, function_calls
// Expected: check_some() + check_none() + check_chain() + check_prop() = 20 + 5 + 8 + 0 = 33

@safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b);

@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }

@check_some () -> int = {
    let a = safe_div(a: 100, b: 5);
    unwrap_or(opt: a, default: 0)
    // = 20
}

@check_none () -> int = {
    let b = safe_div(a: 100, b: 0);
    unwrap_or(opt: b, default: 5)
    // = 5
}

@check_chain () -> int = {
    let x = unwrap_or(opt: safe_div(a: 80, b: 10), default: 0);
    let y = unwrap_or(opt: safe_div(a: 50, b: 0), default: 0);
    x + y
    // = 8 + 0 = 8
}

@try_div (a: int, b: int, c: int) -> Option<int> = {
    let x = safe_div(a: a, b: b)?;
    safe_div(a: x, b: c)
}

@check_prop () -> int = {
    let ok = unwrap_or(opt: try_div(a: 1000, b: 10, c: 5), default: -1);
    let fail_first = unwrap_or(opt: try_div(a: 1000, b: 0, c: 5), default: -10);
    let fail_second = unwrap_or(opt: try_div(a: 1000, b: 10, c: 0), default: -10);
    ok + fail_first + fail_second
    // = 20 + (-10) + (-10) = 0
}

@main () -> int = {
    let a = check_some();     // = 20
    let b = check_none();     // = 5
    let c = check_chain();    // = 8
    let d = check_prop();     // = 0
    a + b + c + d             // = 33
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

**Tokens**: 428 | **Keywords**: ~40 | **Identifiers**: ~100 | **Errors**: 0

<details>
<summary>Token stream (first 30 tokens)</summary>

```text
Fn(@) Ident(safe_div) LParen Ident(a) Colon Ident(int) Comma
Ident(b) Colon Ident(int) RParen Arrow Ident(Option) Lt
Ident(int) Gt Eq If Ident(b) EqEq Int(0) Then None Else
Some LParen Ident(a) Div Ident(b) RParen Semi
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 105 | **Max depth**: 5 | **Functions**: 8 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ FnDecl @safe_div
│  ├─ Params: (a: int, b: int)
│  ├─ Return: Option<int>
│  └─ Body: If(b == 0, None, Some(a / b))
├─ FnDecl @unwrap_or
│  ├─ Params: (opt: Option<int>, default: int)
│  ├─ Return: int
│  └─ Body: Match(opt)
│       ├─ Some(v) -> v
│       └─ None -> default
├─ FnDecl @check_some
│  └─ Body: Block [let a = safe_div(...); unwrap_or(a, 0)]
├─ FnDecl @check_none
│  └─ Body: Block [let b = safe_div(...); unwrap_or(b, 5)]
├─ FnDecl @check_chain
│  └─ Body: Block [let x = ...; let y = ...; x + y]
├─ FnDecl @try_div
│  ├─ Params: (a: int, b: int, c: int)
│  ├─ Return: Option<int>
│  └─ Body: Block [let x = safe_div(a, b)?; safe_div(x, c)]
├─ FnDecl @check_prop
│  └─ Body: Block [let ok = ...; let fail_first = ...; let fail_second = ...; ok + ...]
└─ FnDecl @main
   └─ Body: Block [let a = ...; let b = ...; let c = ...; let d = ...; a+b+c+d]
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: ~40 | **Types inferred**: ~20 | **Unifications**: ~30 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
@safe_div (a: int, b: int) -> Option<int> =
    if b == 0 then None else Some(a / b)
//  ^ Option<int> (both branches unified)
//  b == 0: bool (Eq<int, int>)
//  a / b: int (Div<int, int>)

@unwrap_or (opt: Option<int>, default: int) -> int =
    match opt { Some(v) -> v, None -> default }
//              ^ v: int (destructured from Some)
//              ^ default: int (matches return)

@try_div (a: int, b: int, c: int) -> Option<int> = {
    let x = safe_div(a: a, b: b)?;
//  ^ x: int (unwrapped from Option<int> via ?)
//  ^ ?: on None, returns None from @try_div
    safe_div(a: x, b: c)
//  ^ Option<int> (matches return type)
}

@main () -> int = {
    let a = check_some();  // int
    let b = check_none();  // int
    let c = check_chain(); // int
    let d = check_prop();  // int
    a + b + c + d          // int (Add<int, int>)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 6 | **Desugared**: 2 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Match expressions lowered to decision trees (1 decision tree for @unwrap_or)
- ? operator in @try_div desugared to: match safe_div(a, b) { Some(x) -> ..., None -> None }
- Function bodies lowered to canonical expression form
- Call arguments normalized to positional order
- 6 constants interned
- 117 canonical nodes (user module), 46 nodes (prelude)
```

</details>

### 5. ARC Pipeline

> The ARC (Automatic Reference Counting) pipeline analyzes value lifetimes and
> inserts reference counting operations. It performs borrow inference to minimize
> RC overhead -- parameters that are only read can be borrowed rather than owned.

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@safe_div:    no heap values — Option<int> is scalar {i64, i64}
@unwrap_or:   no heap values — scalar pattern match
@check_some:  no heap values — pure scalar pipeline
@check_none:  no heap values — pure scalar pipeline
@check_chain: no heap values — pure scalar pipeline
@try_div:     no heap values — ? on scalar Option
@check_prop:  no heap values — pure scalar pipeline
@main:        no heap values — pure scalar pipeline
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
  ├─ @check_some()
  │    ├─ @safe_div(a: 100, b: 5) → b != 0 → Some(100 / 5) = Some(20)
  │    └─ @unwrap_or(Some(20), 0) → match Some(v) → 20
  │    → 20
  ├─ @check_none()
  │    ├─ @safe_div(a: 100, b: 0) → b == 0 → None
  │    └─ @unwrap_or(None, 5) → match None → 5
  │    → 5
  ├─ @check_chain()
  │    ├─ @safe_div(80, 10) → Some(8), unwrap_or → 8
  │    ├─ @safe_div(50, 0) → None, unwrap_or → 0
  │    └─ 8 + 0 = 8
  │    → 8
  ├─ @check_prop()
  │    ├─ @try_div(1000, 10, 5): safe_div(1000,10)=Some(100), ?→100, safe_div(100,5)=Some(20)
  │    │   unwrap_or(Some(20), -1) → 20
  │    ├─ @try_div(1000, 0, 5): safe_div(1000,0)=None, ?→None
  │    │   unwrap_or(None, -10) → -10
  │    ├─ @try_div(1000, 10, 0): safe_div(1000,10)=Some(100), ?→100, safe_div(100,0)=None
  │    │   unwrap_or(None, -10) → -10
  │    └─ 20 + (-10) + (-10) = 0
  │    → 0
  └─ 20 + 5 + 8 + 0 = 33
→ 33
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

Nounwind analysis: 2 passes (fixed-point), all 8 functions marked nounwind. Purity analysis: `@safe_div` marked `memory(none)` (pure function -- no memory side effects). RC leak detection integrated into main wrapper via `ori_check_leaks`.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@safe_div:    +0 rc_inc, +0 rc_dec (Option<int> is scalar — no heap)
@unwrap_or:   +0 rc_inc, +0 rc_dec (scalar pattern match)
@check_some:  +0 rc_inc, +0 rc_dec (scalar pipeline)
@check_none:  +0 rc_inc, +0 rc_dec (scalar pipeline)
@check_chain: +0 rc_inc, +0 rc_dec (scalar pipeline)
@try_div:     +0 rc_inc, +0 rc_dec (scalar ? propagation)
@check_prop:  +0 rc_inc, +0 rc_dec (scalar pipeline)
@main:        +0 rc_inc, +0 rc_dec (scalar pipeline)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '12-options'
source_filename = "12-options"

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind memory(none) uwtable
; --- @safe_div ---
define fastcc noundef { i64, i64 } @_ori_safe_div(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %bb3, label %bb2

bb2:                                              ; preds = %bb0
  %div = sdiv i64 %0, %1
  %variant.f0 = insertvalue { i64, i64 } zeroinitializer, i64 %div, 1
  br label %bb3

bb3:                                              ; preds = %bb0, %bb2
  %v101 = phi { i64, i64 } [ %variant.f0, %bb2 ], [ { i64 1, i64 0 }, %bb0 ]
  ret { i64, i64 } %v101
}

; Function Attrs: nounwind uwtable
; --- @unwrap_or ---
define fastcc noundef i64 @_ori_unwrap_or({ i64, i64 } noundef %0, i64 noundef %1) #1 {
bb0:
  %proj.0 = extractvalue { i64, i64 } %0, 0
  switch i64 %proj.0, label %bb4 [
    i64 0, label %bb2
    i64 1, label %bb1
  ]

bb1:                                              ; preds = %bb0, %bb2
  %v31 = phi i64 [ %1, %bb0 ], [ %proj.1, %bb2 ]
  ret i64 %v31

bb2:                                              ; preds = %bb0
  %proj.1 = extractvalue { i64, i64 } %0, 1
  br label %bb1

bb4:                                              ; preds = %bb0
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @check_some ---
define fastcc noundef i64 @_ori_check_some() #1 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 100, i64 5)
  %call1 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call, i64 0)
  ret i64 %call1
}

; Function Attrs: nounwind uwtable
; --- @check_none ---
define fastcc noundef i64 @_ori_check_none() #1 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 100, i64 0)
  %call1 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call, i64 5)
  ret i64 %call1
}

; Function Attrs: nounwind uwtable
; --- @check_chain ---
define fastcc noundef i64 @_ori_check_chain() #1 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 80, i64 10)
  %call1 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call, i64 0)
  %call2 = call fastcc { i64, i64 } @_ori_safe_div(i64 50, i64 0)
  %call3 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call2, i64 0)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call1, i64 %call3)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  ret i64 %add.val

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @try_div ---
define fastcc noundef { i64, i64 } @_ori_try_div(i64 noundef %0, i64 noundef %1, i64 noundef %2) #1 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1)
  %proj.0 = extractvalue { i64, i64 } %call, 0
  %eq = icmp eq i64 %proj.0, 0
  br i1 %eq, label %bb1, label %bb2

bb1:                                              ; preds = %bb0
  %proj.1 = extractvalue { i64, i64 } %call, 1
  %call1 = call fastcc { i64, i64 } @_ori_safe_div(i64 %proj.1, i64 %2)
  ret { i64, i64 } %call1

bb2:                                              ; preds = %bb0
  ret { i64, i64 } { i64 1, i64 0 }
}

; Function Attrs: nounwind uwtable
; --- @check_prop ---
define fastcc noundef i64 @_ori_check_prop() #1 {
bb0:
  %call = call fastcc { i64, i64 } @_ori_try_div(i64 1000, i64 10, i64 5)
  %call1 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call, i64 -1)
  %call2 = call fastcc { i64, i64 } @_ori_try_div(i64 1000, i64 0, i64 5)
  %call3 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call2, i64 -10)
  %call4 = call fastcc { i64, i64 } @_ori_try_div(i64 1000, i64 10, i64 0)
  %call5 = call fastcc i64 @_ori_unwrap_or({ i64, i64 } %call4, i64 -10)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call1, i64 %call3)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %add6 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call5)
  %add.val7 = extractvalue { i64, i1 } %add6, 0
  %add.ovf8 = extractvalue { i64, i1 } %add6, 1
  br i1 %add.ovf8, label %add.ovf_panic10, label %add.ok9

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok9:                                          ; preds = %add.ok
  ret i64 %add.val7

add.ovf_panic10:                                  ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #1 {
bb0:
  %call = call fastcc i64 @_ori_check_some()
  %call1 = call fastcc i64 @_ori_check_none()
  %call2 = call fastcc i64 @_ori_check_chain()
  %call3 = call fastcc i64 @_ori_check_prop()
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  %add4 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  %add.val5 = extractvalue { i64, i1 } %add4, 0
  %add.ovf6 = extractvalue { i64, i1 } %add4, 1
  br i1 %add.ovf6, label %add.ovf_panic8, label %add.ok7

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok7:                                          ; preds = %add.ok
  %add9 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val5, i64 %call3)
  %add.val10 = extractvalue { i64, i1 } %add9, 0
  %add.ovf11 = extractvalue { i64, i1 } %add9, 1
  br i1 %add.ovf11, label %add.ovf_panic13, label %add.ok12

add.ovf_panic8:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok12:                                         ; preds = %add.ok7
  ret i64 %add.val10

add.ovf_panic13:                                  ; preds = %add.ok7
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #3

; Function Attrs: nounwind uwtable
define noundef i32 @main() #1 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  %leak_check = call i32 @ori_check_leaks()
  %has_leak = icmp ne i32 %leak_check, 0
  %final_exit = select i1 %has_leak, i32 %leak_check, i32 %exit_code
  ret i32 %final_exit
}

; Function Attrs: nounwind
declare i32 @ori_check_leaks() #4

attributes #0 = { nounwind memory(none) uwtable }
attributes #1 = { nounwind uwtable }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { cold noreturn }
attributes #4 = { nounwind }
```

#### Disassembly

```asm
_ori_safe_div:
   1b100: mov    %rsi,-0x20(%rsp)
   1b105: mov    %rdi,-0x18(%rsp)
   1b10a: mov    $0x1,%ecx
   1b10f: xor    %eax,%eax
   1b111: test   %rsi,%rsi
   1b114: mov    %rcx,-0x10(%rsp)
   1b119: mov    %rax,-0x8(%rsp)
   1b11e: je     1b13f
   1b120: jmp    1b122
   1b122: mov    -0x20(%rsp),%rcx
   1b127: mov    -0x18(%rsp),%rax
   1b12c: cqto
   1b12e: idiv   %rcx
   1b131: xor    %ecx,%ecx
   1b133: mov    %rcx,-0x10(%rsp)
   1b138: mov    %rax,-0x8(%rsp)
   1b13d: jmp    1b13f
   1b13f: mov    -0x10(%rsp),%rax
   1b144: mov    -0x8(%rsp),%rdx
   1b149: ret

_ori_unwrap_or:
   1b150: mov    %rdx,-0x10(%rsp)
   1b155: mov    %rsi,-0x8(%rsp)
   1b15a: test   %rdi,%rdi
   1b15d: je     1b171
   1b15f: jmp    1b161
   1b161: mov    -0x10(%rsp),%rax
   1b166: mov    %rax,-0x18(%rsp)
   1b16b: mov    -0x18(%rsp),%rax
   1b170: ret
   1b171: mov    -0x8(%rsp),%rax
   1b176: mov    %rax,-0x18(%rsp)
   1b17b: jmp    1b16b

_ori_check_some:
   1b180: push   %rax
   1b181: mov    $0x64,%edi
   1b186: mov    $0x5,%esi
   1b18b: call   1b100 <_ori_safe_div>
   1b190: mov    %rax,%rdi
   1b193: mov    %rdx,%rsi
   1b196: xor    %eax,%eax
   1b198: mov    %eax,%edx
   1b19a: call   1b150 <_ori_unwrap_or>
   1b19f: pop    %rcx
   1b1a0: ret

_ori_check_none:
   1b1b0: push   %rax
   1b1b1: mov    $0x64,%edi
   1b1b6: xor    %eax,%eax
   1b1b8: mov    %eax,%esi
   1b1ba: call   1b100 <_ori_safe_div>
   1b1bf: mov    %rax,%rdi
   1b1c2: mov    %rdx,%rsi
   1b1c5: mov    $0x5,%edx
   1b1ca: call   1b150 <_ori_unwrap_or>
   1b1cf: pop    %rcx
   1b1d0: ret

_ori_check_chain:
   1b1e0: sub    $0x18,%rsp
   1b1e4: mov    $0x50,%edi
   1b1e9: mov    $0xa,%esi
   1b1ee: call   1b100 <_ori_safe_div>
   1b1f3: mov    %rax,%rdi
   1b1f6: mov    %rdx,%rsi
   1b1f9: xor    %eax,%eax
   1b1fb: mov    %eax,%edx
   1b1fd: call   1b150 <_ori_unwrap_or>
   1b202: mov    %rax,0x8(%rsp)
   1b207: mov    $0x32,%edi
   1b20c: xor    %eax,%eax
   1b20e: mov    %eax,%esi
   1b210: call   1b100 <_ori_safe_div>
   1b215: mov    %rax,%rdi
   1b218: mov    %rdx,%rsi
   1b21b: xor    %eax,%eax
   1b21d: mov    %eax,%edx
   1b21f: call   1b150 <_ori_unwrap_or>
   1b224: mov    %rax,%rcx
   1b227: mov    0x8(%rsp),%rax
   1b22c: add    %rcx,%rax
   1b22f: mov    %rax,0x10(%rsp)
   1b234: seto   %al
   1b237: jo     1b243
   1b239: mov    0x10(%rsp),%rax
   1b23e: add    $0x18,%rsp
   1b242: ret
   1b243: lea    0xd30b2(%rip),%rdi
   1b24a: call   1bef0 <ori_panic_cstr>

_ori_try_div:
   1b250: sub    $0x18,%rsp
   1b254: mov    %rdx,0x8(%rsp)
   1b259: call   1b100 <_ori_safe_div>
   1b25e: mov    %rdx,0x10(%rsp)
   1b263: cmp    $0x0,%rax
   1b267: jne    1b27d
   1b269: mov    0x8(%rsp),%rsi
   1b26e: mov    0x10(%rsp),%rdi
   1b273: call   1b100 <_ori_safe_div>
   1b278: add    $0x18,%rsp
   1b27c: ret
   1b27d: xor    %eax,%eax
   1b27f: mov    %eax,%edx
   1b281: mov    $0x1,%eax
   1b286: add    $0x18,%rsp
   1b28a: ret

_ori_check_prop:
   1b290: sub    $0x28,%rsp
   1b294: mov    $0x3e8,%edi
   1b299: mov    $0xa,%esi
   1b29e: mov    $0x5,%edx
   1b2a3: call   1b250 <_ori_try_div>
   1b2a8: mov    %rax,%rdi
   1b2ab: mov    %rdx,%rsi
   1b2ae: mov    $0xffffffffffffffff,%rdx
   1b2b5: call   1b150 <_ori_unwrap_or>
   1b2ba: mov    %rax,0x10(%rsp)
   1b2bf: mov    $0x3e8,%edi
   1b2c4: xor    %eax,%eax
   1b2c6: mov    %eax,%esi
   1b2c8: mov    $0x5,%edx
   1b2cd: call   1b250 <_ori_try_div>
   1b2d2: mov    %rax,%rdi
   1b2d5: mov    %rdx,%rsi
   1b2d8: mov    $0xfffffffffffffff6,%rdx
   1b2df: call   1b150 <_ori_unwrap_or>
   1b2e4: mov    %rax,0x8(%rsp)
   1b2e9: mov    $0x3e8,%edi
   1b2ee: mov    $0xa,%esi
   1b2f3: xor    %eax,%eax
   1b2f5: mov    %eax,%edx
   1b2f7: call   1b250 <_ori_try_div>
   1b2fc: mov    %rax,%rdi
   1b2ff: mov    %rdx,%rsi
   1b302: mov    $0xfffffffffffffff6,%rdx
   1b309: call   1b150 <_ori_unwrap_or>
   1b30e: mov    0x8(%rsp),%rcx
   1b313: mov    %rax,%rdx
   1b316: mov    0x10(%rsp),%rax
   1b31b: mov    %rdx,0x18(%rsp)
   1b320: add    %rcx,%rax
   1b323: mov    %rax,0x20(%rsp)
   1b328: seto   %al
   1b32b: jo     1b345
   1b32d: mov    0x18(%rsp),%rcx
   1b332: mov    0x20(%rsp),%rax
   1b337: add    %rcx,%rax
   1b33a: mov    %rax,(%rsp)
   1b33e: seto   %al
   1b341: jo     1b35a
   1b343: jmp    1b351
   1b345: lea    0xd2fb0(%rip),%rdi
   1b34c: call   1bef0 <ori_panic_cstr>
   1b351: mov    (%rsp),%rax
   1b355: add    $0x28,%rsp
   1b359: ret
   1b35a: lea    0xd2f9b(%rip),%rdi
   1b361: call   1bef0 <ori_panic_cstr>

_ori_main:
   1b370: sub    $0x38,%rsp
   1b374: call   1b180 <_ori_check_some>
   1b379: mov    %rax,0x20(%rsp)
   1b37e: call   1b1b0 <_ori_check_none>
   1b383: mov    %rax,0x18(%rsp)
   1b388: call   1b1e0 <_ori_check_chain>
   1b38d: mov    %rax,0x10(%rsp)
   1b392: call   1b290 <_ori_check_prop>
   1b397: mov    0x18(%rsp),%rcx
   1b39c: mov    %rax,%rdx
   1b39f: mov    0x20(%rsp),%rax
   1b3a4: mov    %rdx,0x28(%rsp)
   1b3a9: add    %rcx,%rax
   1b3ac: mov    %rax,0x30(%rsp)
   1b3b1: seto   %al
   1b3b4: jo     1b3cf
   1b3b6: mov    0x10(%rsp),%rcx
   1b3bb: mov    0x30(%rsp),%rax
   1b3c0: add    %rcx,%rax
   1b3c3: mov    %rax,0x8(%rsp)
   1b3c8: seto   %al
   1b3cb: jo     1b3f3
   1b3cd: jmp    1b3db
   1b3cf: lea    0xd2f26(%rip),%rdi
   1b3d6: call   1bef0 <ori_panic_cstr>
   1b3db: mov    0x28(%rsp),%rcx
   1b3e0: mov    0x8(%rsp),%rax
   1b3e5: add    %rcx,%rax
   1b3e8: mov    %rax,(%rsp)
   1b3ec: seto   %al
   1b3ef: jo     1b408
   1b3f1: jmp    1b3ff
   1b3f3: lea    0xd2f02(%rip),%rdi
   1b3fa: call   1bef0 <ori_panic_cstr>
   1b3ff: mov    (%rsp),%rax
   1b403: add    $0x38,%rsp
   1b407: ret
   1b408: lea    0xd2eed(%rip),%rdi
   1b40f: call   1bef0 <ori_panic_cstr>

main:
   1b420: push   %rax
   1b421: call   1b370 <_ori_main>
   1b426: mov    %eax,0x4(%rsp)
   1b42a: call   209e0 <ori_check_leaks>
   1b42f: mov    %eax,%ecx
   1b431: mov    0x4(%rsp),%eax
   1b435: cmp    $0x0,%ecx
   1b438: cmovne %ecx,%eax
   1b43b: pop    %rcx
   1b43c: ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @safe_div | 7 | 7 | 1.00x | OPTIMAL |
| 2 | @unwrap_or | 7 | 7 | 1.00x | OPTIMAL |
| 3 | @check_some | 3 | 3 | 1.00x | OPTIMAL |
| 4 | @check_none | 3 | 3 | 1.00x | OPTIMAL |
| 5 | @check_chain | 11 | 11 | 1.00x | OPTIMAL |
| 6 | @try_div | 8 | 8 | 1.00x | OPTIMAL |
| 7 | @check_prop | 19 | 19 | 1.00x | OPTIMAL |
| 8 | @main | 23 | 23 | 1.00x | OPTIMAL |

**Weighted average ratio**: 1.00x | **Max ratio**: 1.00x

Every function matches its ideal instruction count. The CFG simplification pass eliminated the previously-observed empty intermediate blocks in `@safe_div` and `@unwrap_or`, bringing both to OPTIMAL. All 8 functions are at 1.00x ratio with zero unjustified instructions.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @safe_div | 0 | 0 | YES | N/A | N/A |
| @unwrap_or | 0 | 0 | YES | N/A | N/A |
| @check_some | 0 | 0 | YES | N/A | N/A |
| @check_none | 0 | 0 | YES | N/A | N/A |
| @check_chain | 0 | 0 | YES | N/A | N/A |
| @try_div | 0 | 0 | YES | N/A | N/A |
| @check_prop | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 0 | YES | N/A | N/A |

**Verdict**: Zero RC operations across all 8 functions. `Option<int>` is represented as `{i64, i64}` -- a flat scalar pair returned in registers (rax, rdx). No heap allocation, no ARC involvement. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | memory | noundef | uwtable | cold | Notes |
|----------|--------|----------|--------|---------|---------|------|-------|
| @safe_div | YES | YES | memory(none) | params+return | YES | NO | [NOTE-1] |
| @unwrap_or | YES | YES | -- | return+param | YES | NO | |
| @check_some | YES | YES | -- | return | YES | NO | |
| @check_none | YES | YES | -- | return | YES | NO | |
| @check_chain | YES | YES | -- | return | YES | NO | |
| @try_div | YES | YES | -- | params | YES | NO | |
| @check_prop | YES | YES | -- | return | YES | NO | |
| @main (ori) | C cc | YES | -- | return | YES | NO | |
| main (wrapper) | C cc | YES | -- | return | YES | NO | |
| ori_panic_cstr | N/A | N/A | -- | N/A | N/A | YES | cold noreturn |
| ori_check_leaks | N/A | YES | -- | N/A | N/A | NO | |

All user functions have `nounwind` (fixed-point analysis, 2 passes). All internal calls use `fastcc`. `@_ori_main` correctly uses C calling convention as an entry point. The `noundef` attribute is applied appropriately on scalar returns and parameters. `@safe_div` has `memory(none)` -- the purity analysis correctly identified it as having no memory side effects (pure arithmetic + branch). [NOTE-1]

**Compliance**: 38/38 applicable attributes correct (100.0%).

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @safe_div | 3 | 0 | 0 | 1 | [NOTE-2] |
| @unwrap_or | 4 | 0 | 0 | 1 | [NOTE-3] |
| @check_some | 1 | 0 | 0 | 0 | |
| @check_none | 1 | 0 | 0 | 0 | |
| @check_chain | 3 | 0 | 0 | 0 | |
| @try_div | 3 | 0 | 0 | 0 | |
| @check_prop | 5 | 0 | 0 | 0 | |
| @main | 7 | 0 | 0 | 0 | |

**@safe_div**: The CFG simplification pass eliminated the empty bb1 (None branch) that previously contained only `br label %bb3`. bb0 now branches directly to bb3 (the phi/ret block) for the None case, with `{ i64 1, i64 0 }` sourced from bb0. Down from 4 blocks to 3. [NOTE-2]

**@unwrap_or**: The CFG simplification pass eliminated the empty bb3 (None case) that previously contained only `br label %bb1`. The switch now dispatches None (i64 1) directly to bb1 where the phi merges `%1` (default) as the value from bb0. Down from 5 blocks to 4. The remaining bb4 (`unreachable`) is the switch default -- a correct safety guard for exhaustive matching. [NOTE-3]

Zero defects across all functions.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (check_chain) | YES | YES | `llvm.sadd.with.overflow.i64` |
| add (check_prop, x2) | YES | YES | Two chained additions, both checked |
| add (main, x3) | YES | YES | Three chained additions, all checked |
| div (safe_div) | N/A | N/A | Division by zero handled at source level (if b == 0) |

All arithmetic additions use `llvm.sadd.with.overflow.i64` with proper panic on overflow. Division does not use overflow intrinsics because `safe_div` explicitly guards against division by zero at the application level -- this is correct since `sdiv` with a non-zero divisor cannot overflow except for `INT_MIN / -1`, which is a separate concern. The compiler correctly does not add redundant overflow checking on top of the user's own safety logic.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869.8 KiB |
| .rodata section | 133.5 KiB |
| User code | 733 bytes (8 functions) |
| Runtime | 99.9% of binary |

Per-function native code sizes:

| Function | Bytes | Instructions (approx) |
|----------|-------|-----------------------|
| @safe_div | 74 | 21 |
| @unwrap_or | 45 | 13 |
| @check_some | 33 | 9 |
| @check_none | 33 | 9 |
| @check_chain | 111 | 28 |
| @try_div | 59 | 15 |
| @check_prop | 214 | 48 |
| @main | 164 | 38 |

The user code is compact at 733 bytes. The `{i64, i64}` Option representation maps efficiently to the (rax, rdx) return convention, avoiding any heap allocation or indirection. The `main` wrapper (not counted in user code) adds 29 bytes for the `ori_check_leaks` integration.

### 7. Optimal IR Comparison

#### @safe_div: Ideal vs Actual

```llvm
; IDEAL (7 instructions)
define fastcc noundef { i64, i64 } @_ori_safe_div(i64 noundef %0, i64 noundef %1) nounwind memory(none) {
entry:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %exit, label %some
some:
  %div = sdiv i64 %0, %1
  %result = insertvalue { i64, i64 } zeroinitializer, i64 %div, 1
  br label %exit
exit:
  %ret = phi { i64, i64 } [ %result, %some ], [ { i64 1, i64 0 }, %entry ]
  ret { i64, i64 } %ret
}
```

```llvm
; ACTUAL (7 instructions -- matches ideal)
define fastcc noundef { i64, i64 } @_ori_safe_div(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  %eq = icmp eq i64 %1, 0
  br i1 %eq, label %bb3, label %bb2
bb2:
  %div = sdiv i64 %0, %1
  %variant.f0 = insertvalue { i64, i64 } zeroinitializer, i64 %div, 1
  br label %bb3
bb3:
  %v101 = phi { i64, i64 } [ %variant.f0, %bb2 ], [ { i64 1, i64 0 }, %bb0 ]
  ret { i64, i64 } %v101
}
```

**Delta**: +0 instructions. The previously-observed empty bb1 has been eliminated by the CFG simplification pass. bb0 now branches directly to bb3 for the None case.

#### @unwrap_or: Ideal vs Actual

```llvm
; IDEAL (7 instructions)
define fastcc noundef i64 @_ori_unwrap_or({ i64, i64 } noundef %0, i64 noundef %1) nounwind {
entry:
  %disc = extractvalue { i64, i64 } %0, 0
  switch i64 %disc, label %unreachable [
    i64 0, label %some
    i64 1, label %exit
  ]
some:
  %val = extractvalue { i64, i64 } %0, 1
  br label %exit
exit:
  %result = phi i64 [ %1, %entry ], [ %val, %some ]
  ret i64 %result
unreachable:
  unreachable
}
```

```llvm
; ACTUAL (7 instructions -- matches ideal)
define fastcc noundef i64 @_ori_unwrap_or({ i64, i64 } noundef %0, i64 noundef %1) #1 {
bb0:
  %proj.0 = extractvalue { i64, i64 } %0, 0
  switch i64 %proj.0, label %bb4 [
    i64 0, label %bb2
    i64 1, label %bb1
  ]
bb1:
  %v31 = phi i64 [ %1, %bb0 ], [ %proj.1, %bb2 ]
  ret i64 %v31
bb2:
  %proj.1 = extractvalue { i64, i64 } %0, 1
  br label %bb1
bb4:
  unreachable
}
```

**Delta**: +0 instructions. The previously-observed empty bb3 (None case) has been eliminated. The None case (i64 1) now dispatches directly to bb1 where the phi merges the default value from bb0.

#### @try_div: Ideal vs Actual

```llvm
; IDEAL (8 instructions -- matches actual)
define fastcc noundef { i64, i64 } @_ori_try_div(i64 noundef %0, i64 noundef %1, i64 noundef %2) nounwind {
entry:
  %call = call fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1)
  %disc = extractvalue { i64, i64 } %call, 0
  %is_some = icmp eq i64 %disc, 0
  br i1 %is_some, label %some, label %none
some:
  %val = extractvalue { i64, i64 } %call, 1
  %call2 = call fastcc { i64, i64 } @_ori_safe_div(i64 %val, i64 %2)
  ret { i64, i64 } %call2
none:
  ret { i64, i64 } { i64 1, i64 0 }
}
```

The `?` operator lowering is OPTIMAL: check discriminant, branch on Some/None, extract payload only in the Some path, return None directly in the None path. No intermediate blocks, no unnecessary copies.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @safe_div | 7 | 7 | +0 | N/A | OPTIMAL |
| @unwrap_or | 7 | 7 | +0 | N/A | OPTIMAL |
| @check_some | 3 | 3 | +0 | N/A | OPTIMAL |
| @check_none | 3 | 3 | +0 | N/A | OPTIMAL |
| @check_chain | 11 | 11 | +0 | N/A | OPTIMAL |
| @try_div | 8 | 8 | +0 | N/A | OPTIMAL |
| @check_prop | 19 | 19 | +0 | N/A | OPTIMAL |
| @main | 23 | 23 | +0 | N/A | OPTIMAL |

### 8. Options: ? Operator Codegen

The `?` operator on `Option<int>` in `@try_div` is lowered cleanly:

```llvm
%call = call fastcc { i64, i64 } @_ori_safe_div(i64 %0, i64 %1)
%proj.0 = extractvalue { i64, i64 } %call, 0    ; get discriminant
%eq = icmp eq i64 %proj.0, 0                     ; 0 = Some
br i1 %eq, label %bb1, label %bb2                ; Some -> continue, None -> early return
```

In the Some path (bb1): extract payload, continue with second `safe_div` call.
In the None path (bb2): return `{ i64 1, i64 0 }` (None) immediately.

This is textbook `?` lowering -- branch on discriminant, extract-and-continue on success, propagate-error on failure. No intermediate allocation, no runtime function call, no exception mechanism. The lowering is equivalent to:

```text
match safe_div(a, b) {
    Some(x) -> safe_div(x, c),
    None -> None,
}
```

The codegen for this is OPTIMAL at 8 instructions. The `?` operator adds zero overhead compared to manual pattern matching. In native code, the discriminant check compiles to `cmp $0x0,%rax; jne` -- a single comparison and conditional jump. The early return path produces the None constant inline (`xor %eax,%eax; mov %eax,%edx; mov $0x1,%eax`) without any function calls.

### 9. Options: Scalar Optimization

`Option<int>` is lowered to `{ i64, i64 }` -- a tagged union where field 0 is the discriminant (0=Some, 1=None) and field 1 is the payload. This is a flat, register-friendly representation:

- **Construction**: `Some(v)` becomes `insertvalue { i64, i64 } zeroinitializer, i64 %v, 1` (discriminant 0 is implicit from zeroinitializer). `None` becomes `{ i64 1, i64 0 }` (discriminant 1, payload zeroed).
- **Destruction**: `extractvalue { i64, i64 } %opt, 0` gets discriminant, `extractvalue { i64, i64 } %opt, 1` gets payload.
- **Return convention**: The `{i64, i64}` struct is returned in the (rax, rdx) register pair on x86_64 with `fastcc`. No heap allocation, no pointer indirection.
- **Match lowering**: `unwrap_or` uses `switch i64 %disc, label %default [i64 0, label %some; i64 1, label %none]` -- a comparison chain. The `default` block contains `unreachable` as a safety guard for exhaustive matching.
- **Discriminant convention**: 0=Some, 1=None. This means `test %rdi,%rdi; je some_path` -- the common (Some) case is the conditional jump target when zero, which is branch-prediction friendly since `test; je` on zero is the "expected" case in many ABIs.
- **Purity**: `@safe_div` is marked `memory(none)` -- the purity analysis correctly identifies it as having no memory side effects (pure arithmetic + branching). This enables LLVM to perform additional optimizations like CSE and dead store elimination on callers.

This representation is equivalent to what Rust uses for `Option<i64>` (discriminant + payload with niche optimization not applicable here since the payload spans the full i64 range). The compiler correctly avoids boxing or heap allocation for scalar Options. The zero ARC overhead across all 8 functions confirms that the scalar optimization is working end-to-end.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | NOTE | Attributes | memory(none) on @safe_div -- purity analysis working | NEW | J12 |
| 2 | NOTE | Control Flow | Empty bb1 in @safe_div eliminated by CFG simplification | FIXED | J12 |
| 3 | NOTE | Control Flow | Empty bb3 in @unwrap_or eliminated by CFG simplification | FIXED | J12 |
| 4 | NOTE | ARC | Zero RC operations -- Option<int> is entirely scalar | CONFIRMED | J12 |
| 5 | NOTE | Codegen | ? operator lowering is OPTIMAL -- zero overhead vs manual match | CONFIRMED | J12 |

### NOTE-1: memory(none) on @safe_div

**Location**: @safe_div function declaration
**Impact**: Positive -- the purity analysis correctly marks `@safe_div` as having no memory side effects. This is a `{i64, i64}` returning function with only arithmetic and branching. The attribute enables LLVM to perform CSE, LICM, and dead store elimination on callers.
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-2: Empty bb1 in @safe_div eliminated

**Location**: @safe_div CFG
**Impact**: Positive -- the previously-observed empty bb1 (None branch with only `br label %bb3`) has been eliminated by the CFG simplification pass. bb0 now branches directly to bb3 with `{ i64 1, i64 0 }` as a phi predecessor. This removes 1 block and 1 instruction, bringing @safe_div from 8 to 7 instructions (OPTIMAL).
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-3: Empty bb3 in @unwrap_or eliminated

**Location**: @unwrap_or CFG
**Impact**: Positive -- the previously-observed empty bb3 (None case with only `br label %bb1`) has been eliminated by the CFG simplification pass. The None case (i64 1) now dispatches directly to bb1 where the phi merges the default value from bb0. This removes 1 block and 1 instruction.
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Zero RC operations for Option<int>

**Location**: All 8 functions
**Impact**: Positive -- Option<int> as `{i64, i64}` is entirely scalar, requiring no heap allocation or reference counting. This is the ideal outcome for small sum types.
**Found in**: ARC Purity (Category 2)

### NOTE-5: Optimal ? operator lowering

**Location**: @try_div
**Impact**: Positive -- the `?` operator compiles to a simple discriminant check + branch, identical in cost to a manual `match`. No runtime overhead, no exception mechanism, no intermediate allocations.
**Found in**: Options: ? Operator Codegen (Category 8)

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

Journey 12 achieves a perfect score. All 8 functions match their ideal instruction counts at 1.00x ratio. The `Option<int>` representation as a flat `{i64, i64}` tagged pair is register-friendly and heap-free, resulting in zero ARC overhead. The `?` operator in `@try_div` compiles to an optimal 8-instruction discriminant-branch-and-propagate sequence. The purity analysis marks `@safe_div` with `memory(none)`, enabling additional LLVM optimizations. Attribute compliance is 100% (38/38). The `main` wrapper now includes `ori_check_leaks` integration for RC leak detection at program exit. This is textbook Option codegen.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J12 | CONFIRMED |
| fastcc usage | J1 | J12 | CONFIRMED |
| nounwind analysis | J1 | J12 | CONFIRMED |
| noundef on returns/params | J1 | J12 | CONFIRMED |
| CFG simplification (empty blocks) | J2 | J12 | FIXED (bb1 in @safe_div, bb3 in @unwrap_or) |
| Pattern matching lowering | J6 | J12 | CONFIRMED (switch + phi) |
| memory(none) purity annotation | J12 | J12 | NEW |
| ori_check_leaks in main wrapper | J12 | J12 | NEW |

The CFG simplification pass has resolved the empty-block pattern that previously affected if/then/else and match codegen. Both `@safe_div` (if/then/else with Option construction) and `@unwrap_or` (match on Option) now have clean block layouts with no redundant branches. The purity analysis correctly marks `@safe_div` as `memory(none)` -- the first journey-specific function to receive this annotation. The main wrapper now integrates `ori_check_leaks` for automatic RC leak detection at program exit, replacing the simpler trunc+ret pattern. All nounwind and fastcc patterns continue working correctly across 8 functions.
