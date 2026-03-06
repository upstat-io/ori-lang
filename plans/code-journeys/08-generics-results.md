---
journey: 8
slug: generics
theme: "I am generic"
date: 2026-03-06
status: PASS
expected: 57
eval_result: 57
aot_result: 57
difficulty: moderate
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of parameterized types"
  - "Understanding of structs and function calls"
learning_objectives:
  - "See how generic functions are monomorphized to concrete LLVM IR"
  - "Understand how generic structs are lowered to LLVM struct types"
  - "Compare ideal vs actual codegen for identity/projection functions"
  - "Observe that monomorphization eliminates all type-parameter overhead"
features:
  - generics
  - monomorphization
  - generic_structs
  - type_inference
feature_description: "Generic functions, generic structs, monomorphization, and type inference"
score: 9.8
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 10
  attributes_safety: 8
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
  attr_applicable: 23
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
  - journey: 1
    relationship: "Same overflow checking pattern on arithmetic"
  - journey: 4
    relationship: "Both test struct construction and field access"
---

# Journey 8: "I am generic"

## Source

```ori
// Journey 8: "I am generic"
// Slug: generics
// Difficulty: moderate
// Features: generics, monomorphization, generic_structs, type_inference
// Expected: identity(42) + first(10, 20) + get_value(Box { value: 5 }) = 42 + 10 + 5 = 57

type Box<T> = { value: T }

@identity<T> (x: T) -> T = x;

@first<A, B> (a: A, b: B) -> A = a;

@get_value<T> (b: Box<T>) -> T = b.value;

@main () -> int = {
    let a = identity(x: 42);                   // = 42
    let b = first(a: 10, b: 20);               // = 10
    let c = get_value(b: Box { value: 5 });     // = 5
    a + b + c                                   // = 57
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 57        | 57       | (none) | (none) | PASS   |
| AOT     | 57        | 57       | (none) | (none) | PASS   |

## Compiler Pipeline

### 1. Lexer

> The lexer (tokenizer) breaks raw source text into a stream of tokens -- the smallest
> meaningful units like keywords, identifiers, operators, and literals.

**Tokens**: 141 | **Keywords**: 8 | **Identifiers**: 25 | **Errors**: 0

<details>
<summary>Token stream (user module)</summary>

```text
Type Ident(Box) Lt Ident(T) Gt Eq LBrace Ident(value) Colon Ident(T) RBrace
Fn(@) Ident(identity) Lt Ident(T) Gt LParen Ident(x) Colon Ident(T) RParen
  Arrow Ident(T) Eq Ident(x) Semi
Fn(@) Ident(first) Lt Ident(A) Comma Ident(B) Gt LParen Ident(a) Colon Ident(A)
  Comma Ident(b) Colon Ident(B) RParen Arrow Ident(A) Eq Ident(a) Semi
Fn(@) Ident(get_value) Lt Ident(T) Gt LParen Ident(b) Colon Ident(Box) Lt Ident(T)
  Gt RParen Arrow Ident(T) Eq Ident(b) Dot Ident(value) Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
  Let Ident(a) Eq Ident(identity) LParen Ident(x) Colon Lit(42) RParen Semi
  Let Ident(b) Eq Ident(first) LParen Ident(a) Colon Lit(10) Comma Ident(b) Colon Lit(20) RParen Semi
  Let Ident(c) Eq Ident(get_value) LParen Ident(b) Colon Ident(Box) LBrace Ident(value) Colon Lit(5) RBrace RParen Semi
  Ident(a) Plus Ident(b) Plus Ident(c)
RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 22 | **Max depth**: 4 | **Functions**: 4 | **Types**: 1 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
+-- TypeDecl Box<T>
|   +-- Field: value: T
+-- FnDecl @identity<T>
|   +-- Params: (x: T)
|   +-- Return: T
|   +-- Body: Ident(x)
+-- FnDecl @first<A, B>
|   +-- Params: (a: A, b: B)
|   +-- Return: A
|   +-- Body: Ident(a)
+-- FnDecl @get_value<T>
|   +-- Params: (b: Box<T>)
|   +-- Return: T
|   +-- Body: Field(b, value)
+-- FnDecl @main
    +-- Return: int
    +-- Body: Block
        +-- Let a = Call(@identity, x: 42)
        +-- Let b = Call(@first, a: 10, b: 20)
        +-- Let c = Call(@get_value, b: Box { value: 5 })
        +-- BinOp(+)
            +-- BinOp(+)
            |   +-- Ident(a)
            |   +-- Ident(b)
            +-- Ident(c)
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: 16 | **Types inferred**: 8 | **Unifications**: 12 | **Mono instances**: 3 | **Errors**: 0

<details>
<summary>Inferred types and monomorphization</summary>

```ori
// Type parameters resolved via unification:
//   identity<T> at call site: T = int
//   first<A, B> at call site: A = int, B = int
//   get_value<T> at call site: T = int
//   Box<T> instantiated as Box<int> = { value: int }

@identity<T> (x: T) -> T = x
//  monomorphized to: identity$m$int (x: int) -> int

@first<A, B> (a: A, b: B) -> A = a
//  monomorphized to: first$m$int_int (a: int, b: int) -> int

@get_value<T> (b: Box<T>) -> T = b.value
//  monomorphized to: get_value$m$int (b: Box<int>) -> int

@main () -> int = {
    let a: int = identity(x: 42)       // -> int (from identity<int>)
    let b: int = first(a: 10, b: 20)   // -> int (from first<int, int>)
    let c: int = get_value(b: Box { value: 5 })  // -> int (from get_value<int>)
    a + b + c                           // -> int (Add<int, int> -> int)
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
- Generic function calls replaced with monomorphized call targets
- Box<int> struct construction lowered to canonical struct literal
- Field access b.value lowered to canonical field projection
- Function bodies lowered to canonical expression form
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
@identity$m$int: no heap values -- pure scalar passthrough
@first$m$int_int: no heap values -- pure scalar passthrough
@get_value$m$int: no heap values -- Box<int> is a value type (single i64 field)
@main: no heap values -- all values are scalars (int)
```

</details>

### Backend: Interpreter

> The interpreter (eval path) executes the canonical IR directly, without
> compilation. It serves as the reference implementation for correctness testing.

**Result**: 57 | **Status**: PASS

<details>
<summary>Evaluation trace</summary>

```text
@main()
  +-- let a = @identity(x: 42)
  |         +-- return x = 42
  +-- let b = @first(a: 10, b: 20)
  |         +-- return a = 10
  +-- let c = @get_value(b: Box { value: 5 })
  |         +-- b.value = 5
  +-- a + b + c
       +-- 42 + 10 = 52
       +-- 52 + 5 = 57
-> 57
```

</details>

### Backend: LLVM Codegen

> The LLVM backend compiles the canonical IR to LLVM IR, which is then compiled
> to native machine code via LLVM's optimization and code generation pipeline.
> This path produces ahead-of-time compiled binaries.

#### ARC Pipeline

**RC ops inserted**: 0 | **Elided**: 0 | **Net ops**: 0

<details>
<summary>ARC annotations</summary>

```text
@identity$m$int: +0 rc_inc, +0 rc_dec (no heap values)
@first$m$int_int: +0 rc_inc, +0 rc_dec (no heap values)
@get_value$m$int: +0 rc_inc, +0 rc_dec (no heap values)
@main: +0 rc_inc, +0 rc_dec (no heap values)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '08-generics'
source_filename = "08-generics"

%ori.Box = type { i64 }

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @"_ori_identity$24m$24int"(i64 42)
  %call1 = call fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)
  %call2 = call fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box { i64 5 })
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

; Function Attrs: nounwind uwtable
; --- first$m$int_int ---
define fastcc noundef i64 @"_ori_first$24m$24int_int"(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  ret i64 %0
}

; Function Attrs: nounwind uwtable
; --- get_value$m$int ---
define fastcc noundef i64 @"_ori_get_value$24m$24int"(%ori.Box %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Box %0, 0
  ret i64 %proj.0
}

; Function Attrs: nounwind uwtable
; --- identity$m$int ---
define fastcc noundef i64 @"_ori_identity$24m$24int"(i64 noundef %0) #0 {
bb0:
  ret i64 %0
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #1

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #2

; Function Attrs: nounwind
define i32 @main() #3 {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}

attributes #0 = { nounwind uwtable }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { cold noreturn }
attributes #3 = { nounwind }
```

#### Disassembly

```asm
_ori_main:
  sub    $0x28,%rsp
  mov    $0x2a,%edi
  call   _ori_identity$m$int
  mov    %rax,0x10(%rsp)
  mov    $0xa,%edi
  mov    $0x14,%esi
  call   _ori_first$m$int_int
  mov    %rax,0x8(%rsp)
  mov    $0x5,%edi
  call   _ori_get_value$m$int
  mov    0x8(%rsp),%rcx
  mov    %rax,%rdx
  mov    0x10(%rsp),%rax
  mov    %rdx,0x18(%rsp)
  add    %rcx,%rax
  mov    %rax,0x20(%rsp)
  seto   %al
  jo     .ovf_panic1
  mov    0x18(%rsp),%rcx
  mov    0x20(%rsp),%rax
  add    %rcx,%rax
  mov    %rax,(%rsp)
  seto   %al
  jo     .ovf_panic2
  jmp    .ret
.ovf_panic1:
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr
.ret:
  mov    (%rsp),%rax
  add    $0x28,%rsp
  ret
.ovf_panic2:
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr

_ori_first$m$int_int:
  mov    %rdi,%rax
  ret

_ori_get_value$m$int:
  mov    %rdi,%rax
  ret

_ori_identity$m$int:
  mov    %rdi,%rax
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
| 1 | @identity$m$int | 1 | 1 | 1.00x | OPTIMAL |
| 2 | @first$m$int_int | 1 | 1 | 1.00x | OPTIMAL |
| 3 | @get_value$m$int | 2 | 2 | 1.00x | OPTIMAL |
| 4 | @main | 16 | 16 | 1.00x | OPTIMAL |

All four monomorphized functions produce the minimum possible instruction count.

**@identity$m$int**: Single `ret i64 %0` -- the identity function compiles to a single return. Zero overhead from generics.

**@first$m$int_int**: Single `ret i64 %0` -- ignores second parameter, returns first. Optimal projection.

**@get_value$m$int**: `extractvalue %ori.Box %0, 0` + `ret i64 %proj.0` -- extracts the single field from the Box struct and returns it. Minimal two-instruction sequence.

**@main**: 3 calls + 2 overflow-checked additions (each requiring: call to `@llvm.sadd.with.overflow.i64`, 2 extractvalue, 1 branch, 1 panic call, 1 unreachable) + 1 return = 16 instructions. Every instruction is justified by either computation or safety checking.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @identity$m$int | 0 | 0 | YES | N/A | N/A |
| @first$m$int_int | 0 | 0 | YES | N/A | N/A |
| @get_value$m$int | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 0 | YES | N/A | N/A |

**Verdict**: No heap values in any function. All operations are on scalars (i64) or value-type structs (Box<int> = single i64). Zero RC operations. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | noundef(ret) | noundef(params) | Notes |
|----------|--------|----------|---------|--------------|-----------------|-------|
| @identity$m$int | YES | YES | YES | YES | YES (1/1) | |
| @first$m$int_int | YES | YES | YES | YES | YES (2/2) | |
| @get_value$m$int | YES | YES | YES | YES | NO (0/1) | [LOW-1] |
| @main (entry) | N/A | YES | YES | YES | N/A | |
| @main (wrapper) | N/A | YES | N/A | NO | N/A | [LOW-2] |
| ori_panic_cstr | N/A | N/A | N/A | N/A | cold=YES, noreturn=YES | |

**Compliance**: 21/23 applicable attributes correct (91.3%).

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @identity$m$int | 1 | 0 | 0 | 0 | |
| @first$m$int_int | 1 | 0 | 0 | 0 | |
| @get_value$m$int | 1 | 0 | 0 | 0 | |
| @main | 5 | 0 | 0 | 0 | |

**Verdict**: Clean control flow. The 5 blocks in @main are all necessary: entry (bb0), two add.ok blocks (one per overflow check), and two panic blocks. No empty blocks, no redundant branches, no trivial phi nodes.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (a + b) | YES | YES | Uses llvm.sadd.with.overflow.i64, panics on overflow |
| add ((a+b) + c) | YES | YES | Uses llvm.sadd.with.overflow.i64, panics on overflow |

Both integer additions in `@main` are checked with `@llvm.sadd.with.overflow.i64`. Panic paths call `ori_panic_cstr` with an overflow message. The monomorphized functions perform no arithmetic and thus need no overflow checking.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 868.6 KiB |
| .rodata section | 133.5 KiB |
| User code | 137 bytes |
| Runtime | >99.9% of .text |

#### Disassembly: @identity$m$int

```asm
_ori_identity$m$int:
  mov    %rdi,%rax
  ret
```

2 instructions, 4 bytes. The `mov` is the x86_64 calling convention return value transfer (argument in %rdi, return in %rax). Cannot be elided at this optimization level.

#### Disassembly: @first$m$int_int

```asm
_ori_first$m$int_int:
  mov    %rdi,%rax
  ret
```

2 instructions, 4 bytes. Identical to identity -- first parameter in %rdi is moved to %rax for return.

#### Disassembly: @get_value$m$int

```asm
_ori_get_value$m$int:
  mov    %rdi,%rax
  ret
```

2 instructions, 4 bytes. Box<int> is a single-field struct containing i64 -- passed in %rdi by the calling convention. Field extraction is a no-op at the machine level.

#### Disassembly: @main

```asm
_ori_main:
  sub    $0x28,%rsp
  mov    $0x2a,%edi           ; identity(42)
  call   _ori_identity$m$int
  mov    %rax,0x10(%rsp)      ; save a
  mov    $0xa,%edi            ; first(10, 20)
  mov    $0x14,%esi
  call   _ori_first$m$int_int
  mov    %rax,0x8(%rsp)       ; save b
  mov    $0x5,%edi            ; get_value(Box{5})
  call   _ori_get_value$m$int
  ; ... overflow-checked additions ...
  ret
```

31 instructions, 125 bytes (debug build). All spills to stack are expected at -O0.

### 7. Optimal IR Comparison

#### @identity$m$int: Ideal vs Actual

```llvm
; IDEAL (1 instruction)
define fastcc noundef i64 @"_ori_identity$24m$24int"(i64 noundef %0) nounwind {
  ret i64 %0
}
```

```llvm
; ACTUAL (1 instruction)
define fastcc noundef i64 @"_ori_identity$24m$24int"(i64 noundef %0) #0 {
bb0:
  ret i64 %0
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @first$m$int_int: Ideal vs Actual

```llvm
; IDEAL (1 instruction)
define fastcc noundef i64 @"_ori_first$24m$24int_int"(i64 noundef %0, i64 noundef %1) nounwind {
  ret i64 %0
}
```

```llvm
; ACTUAL (1 instruction)
define fastcc noundef i64 @"_ori_first$24m$24int_int"(i64 noundef %0, i64 noundef %1) #0 {
bb0:
  ret i64 %0
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @get_value$m$int: Ideal vs Actual

```llvm
; IDEAL (2 instructions)
define fastcc noundef i64 @"_ori_get_value$24m$24int"(%ori.Box %0) nounwind {
  %v = extractvalue %ori.Box %0, 0
  ret i64 %v
}
```

```llvm
; ACTUAL (2 instructions)
define fastcc noundef i64 @"_ori_get_value$24m$24int"(%ori.Box %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Box %0, 0
  ret i64 %proj.0
}
```

**Delta**: +0 instructions. OPTIMAL.

#### @main: Ideal vs Actual

```llvm
; IDEAL (16 instructions — with overflow checking)
define noundef i64 @_ori_main() nounwind {
  %a = call fastcc i64 @"_ori_identity$24m$24int"(i64 42)
  %b = call fastcc i64 @"_ori_first$24m$24int_int"(i64 10, i64 20)
  %c = call fastcc i64 @"_ori_get_value$24m$24int"(%ori.Box { i64 5 })
  %r1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %v1 = extractvalue { i64, i1 } %r1, 0
  %o1 = extractvalue { i64, i1 } %r1, 1
  br i1 %o1, label %panic1, label %ok1
ok1:
  %r2 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %v1, i64 %c)
  %v2 = extractvalue { i64, i1 } %r2, 0
  %o2 = extractvalue { i64, i1 } %r2, 1
  br i1 %o2, label %panic2, label %ok2
ok2:
  ret i64 %v2
panic1:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
panic2:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (16 instructions)
; [identical structure to ideal — see Generated LLVM IR section above]
```

**Delta**: +0 instructions. OPTIMAL.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @identity$m$int | 1 | 1 | +0 | N/A | OPTIMAL |
| @first$m$int_int | 1 | 1 | +0 | N/A | OPTIMAL |
| @get_value$m$int | 2 | 2 | +0 | N/A | OPTIMAL |
| @main | 16 | 16 | +0 | N/A | OPTIMAL |

### 8. Generics: Monomorphization

This journey tests the compiler's monomorphization pipeline -- how generic functions are specialized to concrete types at compile time.

**Monomorphization strategy**: The Ori compiler performs whole-program monomorphization during the type checking phase. Each generic function call with concrete type arguments generates a specialized instance. The LLVM backend receives fully monomorphized IR with no remaining type parameters.

**Naming convention**: Monomorphized instances use the `$m$` separator followed by type names. For example:
- `identity<int>` becomes `identity$m$int`
- `first<int, int>` becomes `first$m$int_int`
- `get_value<int>` becomes `get_value$m$int`

In the LLVM IR, `$` is encoded as `$24` in quoted names (e.g., `@"_ori_identity$24m$24int"`).

**Key observations**:

1. **Zero abstraction cost**: Every monomorphized function compiles to exactly the same IR as a hand-written non-generic equivalent would. `identity<int>` produces the same single `ret i64 %0` as a non-generic `@identity_int (x: int) -> int = x` would.

2. **Struct monomorphization**: `Box<T>` instantiated as `Box<int>` becomes `%ori.Box = type { i64 }`. The generic struct is fully specialized to its concrete layout. The `extractvalue` instruction for field access is a compile-time offset, not a runtime operation.

3. **Type erasure is complete**: No type tags, no vtables, no runtime type information. The monomorphized functions are indistinguishable from hand-written specialized functions. This is the expected behavior for a monomorphization-based generics system (like Rust, C++, or Zig).

4. **Calling convention correctness**: All monomorphized functions use `fastcc` (internal calling convention), while `@_ori_main` correctly uses the C calling convention since it is the entry point.

5. **Single-field struct optimization**: `Box<int>` is a single-field struct containing `i64`. The compiler passes it directly in a register (via `%ori.Box { i64 5 }` as a literal argument). At the machine level, field extraction is a no-op -- all three helper functions compile to identical `mov %rdi, %rax; ret`.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Attributes | Missing noundef on %ori.Box parameter of @get_value$m$int | NEW | J8 |
| 2 | LOW | Attributes | Missing noundef on @main wrapper return value | CONFIRMED | J1 |
| 3 | NOTE | Monomorphization | Zero-cost abstraction: all generic functions compile to optimal IR | NEW | J8 |
| 4 | NOTE | Instruction Purity | All 4 user functions at exactly 1.00x ratio | NEW | J8 |

### LOW-1: Missing noundef on %ori.Box parameter of @get_value$m$int

**Location**: `@"_ori_get_value$24m$24int"(%ori.Box %0)` -- parameter %0 lacks `noundef`
**Impact**: LLVM cannot assume the struct value is well-defined, preventing certain optimizations
**Fix**: Mark struct parameters with `noundef` when all fields are defined types
**First seen**: Journey 8
**Found in**: Attributes & Calling Convention (Category 3)

### LOW-2: Missing noundef on @main wrapper return value

**Location**: `define i32 @main() #3` -- return type lacks `noundef`
**Impact**: Minor -- LLVM may not optimize the return path as aggressively
**Fix**: Add `noundef` to the i32 return type of the C main wrapper
**First seen**: Journey 1
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Zero-cost abstraction achieved

**Location**: All monomorphized functions
**Impact**: Positive -- generics add zero runtime overhead
**Found in**: Generics: Monomorphization (Category 8)

### NOTE-4: Perfect instruction efficiency across all functions

**Location**: All 4 user functions
**Impact**: Positive -- every function achieves the theoretical minimum instruction count
**Found in**: Instruction Purity (Category 1)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 8/10 | 91.3% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.8 / 10**

## Verdict

Journey 8's generics codegen is near-perfect. Monomorphization produces fully specialized functions that are indistinguishable from hand-written equivalents -- all four user functions achieve the theoretical minimum instruction count (1.00x ratio). The generic struct `Box<T>` is lowered to an efficient single-field LLVM struct type passed directly in registers. The only imperfections are two missing `noundef` attributes: one on a struct parameter and one on the C main wrapper return. ARC is irrelevant for this journey since all values are scalars or value-type structs with zero heap allocation.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J8 | CONFIRMED |
| fastcc usage | J1 | J8 | CONFIRMED |
| nounwind on all user functions | J1 | J8 | CONFIRMED |
| Missing noundef on @main wrapper | J1 | J8 | CONFIRMED |
| Struct field access via extractvalue | J4 | J8 | CONFIRMED |

The monomorphization pipeline is the highlight of this journey. Unlike previous journeys that tested runtime features, this journey validates a compile-time transformation. The compiler correctly specializes all three generic patterns (identity function, projection function, and struct field accessor) to their optimal concrete forms. The `Box<int>` struct type demonstrates that generic structs are also fully specialized, with the single `i64` field accessed via a zero-cost `extractvalue` instruction.
