---
journey: 4
slug: structs
theme: "I am a struct"
date: 2026-03-06
status: PASS
expected: 57
eval_result: 57
aot_result: 57
difficulty: simple
prerequisites:
  - "Basic programming knowledge"
  - "Understanding of structs/records"
learning_objectives:
  - "See how structs are lowered to LLVM struct types"
  - "Understand field access via GEP instructions"
  - "Observe struct-by-reference calling convention for aggregate types"
  - "Compare ideal vs actual codegen for struct operations"
features:
  - struct_construction
  - field_access
  - nested_structs
  - function_calls
feature_description: "Struct construction, field access, nested struct passing, and function calls"
score: 8.5
score_breakdown:
  instruction_efficiency: 7
  arc_correctness: 10
  attributes_safety: 7
  control_flow: 10
  ir_quality: 6
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.48
  instruction_ratio_max: 1.60
  arc_violations: 0
  arc_has_unbalanced: false
  arc_has_scalar_rc: false
  attr_applicable: 12
  attr_correct: 10
  attr_has_wrong: false
  cf_defects: 0
  cf_incorrect: false
  ir_unjustified: 10
  ir_incorrect: false
  bin_defects: 0
  bin_hard_fail: false
  other_critical: 0
  other_high: 0
  other_low: 0
overflow_check: PASS
bugs_found: []
related_journeys: []
---

# Journey 4: "I am a struct"

## Source

```ori
// Journey 4: "I am a struct"
// Slug: structs
// Difficulty: simple
// Features: struct_construction, field_access, nested_structs, function_calls
// Expected: p.x + p.y + area(r) = 3 + 4 + 10 * 5 = 57

type Point = { x: int, y: int }
type Rect = { origin: Point, width: int, height: int }

@area (r: Rect) -> int = r.width * r.height;

@main () -> int = {
    let p = Point { x: 3, y: 4 };
    let r = Rect { origin: p, width: 10, height: 5 };
    p.x + p.y + area(r: r)    // = 3 + 4 + 50 = 57
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

**Tokens**: 118 | **Keywords**: 4 (`type` x2, `let` x2) | **Identifiers**: 22 | **Errors**: 0

<details>
<summary>Token stream (user code)</summary>

```text
Type Ident(Point) Eq LBrace Ident(x) Colon Ident(int)
Comma Ident(y) Colon Ident(int) RBrace
Type Ident(Rect) Eq LBrace Ident(origin) Colon Ident(Point)
Comma Ident(width) Colon Ident(int) Comma Ident(height) Colon Ident(int) RBrace
Fn(@) Ident(area) LParen Ident(r) Colon Ident(Rect) RParen Arrow Ident(int)
Eq Ident(r) Dot Ident(width) Star Ident(r) Dot Ident(height) Semi
Fn(@) Ident(main) LParen RParen Arrow Ident(int) Eq LBrace
Let Ident(p) Eq Ident(Point) LBrace Ident(x) Colon Lit(3) Comma Ident(y) Colon Lit(4) RBrace Semi
Let Ident(r) Eq Ident(Rect) LBrace Ident(origin) Colon Ident(p) Comma
Ident(width) Colon Lit(10) Comma Ident(height) Colon Lit(5) RBrace Semi
Ident(p) Dot Ident(x) Plus Ident(p) Dot Ident(y) Plus Ident(area) LParen
Ident(r) Colon Ident(r) RParen RBrace
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 24 | **Max depth**: 4 | **Functions**: 2 | **Types**: 2 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
+-- TypeDecl Point
|   +-- Field x: int
|   +-- Field y: int
+-- TypeDecl Rect
|   +-- Field origin: Point
|   +-- Field width: int
|   +-- Field height: int
+-- FnDecl @area
|   +-- Params: (r: Rect)
|   +-- Return: int
|   +-- Body: BinOp(*)
|        +-- Field(r, width)
|        +-- Field(r, height)
+-- FnDecl @main
    +-- Return: int
    +-- Body: Block
         +-- Let p = StructLit(Point)
         |        +-- x: Lit(3)
         |        +-- y: Lit(4)
         +-- Let r = StructLit(Rect)
         |        +-- origin: Ident(p)
         |        +-- width: Lit(10)
         |        +-- height: Lit(5)
         +-- BinOp(+)
              +-- BinOp(+)
              |    +-- Field(p, x)
              |    +-- Field(p, y)
              +-- Call(@area)
                   +-- r: Ident(r)
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
type Point = { x: int, y: int }
type Rect = { origin: Point, width: int, height: int }

@area (r: Rect) -> int = r.width * r.height
//                        ^ int (field of Rect)   ^ int (Mul<int, int> -> int)

@main () -> int = {
    let p: Point = Point { x: 3, y: 4 }     // inferred: Point
    let r: Rect = Rect { origin: p, width: 10, height: 5 }  // inferred: Rect
    p.x + p.y + area(r: r)
//  ^ int  ^ int  ^ int (return of @area)
//  result: int (Add<int, int> -> int, twice)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 2 | **Desugared**: 0 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- Canon nodes: 24 (user module), roots: 2
- Constants: 6 (literals 3, 4, 10, 5 + struct type info)
- Struct construction lowered to CanId Struct nodes with field ranges
- Field access lowered to CanId Field nodes
- Function call normalized to positional arg ordering
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
@area: no heap values -- structs are value types with scalar fields only
@main: no heap values -- all struct fields are int (scalars)
Point and Rect contain only int fields, so no reference counting is needed.
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
  +-- let p = Point { x: 3, y: 4 }
  +-- let r = Rect { origin: p, width: 10, height: 5 }
  +-- p.x -> 3
  +-- p.y -> 4
  +-- 3 + 4 = 7
  +-- @area(r: Rect { origin: Point { x: 3, y: 4 }, width: 10, height: 5 })
  |    +-- r.width -> 10
  |    +-- r.height -> 5
  |    +-- 10 * 5 = 50
  +-- 7 + 50 = 57
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
@area: +0 rc_inc, +0 rc_dec (struct fields are all scalars)
@main: +0 rc_inc, +0 rc_dec (struct fields are all scalars)
No heap allocations -- Point and Rect are stack-allocated value types.
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '04-structs'
source_filename = "04-structs"

%ori.Rect = type { %ori.Point, i64, i64 }
%ori.Point = type { i64, i64 }

@ovf.msg = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @area ---
define fastcc noundef i64 @_ori_area(ptr %0) #0 {
bb0:
  %param.load.f1.ptr = getelementptr inbounds nuw %ori.Rect, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue %ori.Rect zeroinitializer, i64 %param.load.f1, 1
  %param.load.f2.ptr = getelementptr inbounds nuw %ori.Rect, ptr %0, i32 0, i32 2
  %param.load.f2 = load i64, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue %ori.Rect %param.load.s1, i64 %param.load.f2, 2
  %proj.1 = extractvalue %ori.Rect %param.load.s2, 1
  %proj.2 = extractvalue %ori.Rect %param.load.s2, 2
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %proj.1, i64 %proj.2)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok

mul.ok:                                           ; preds = %bb0
  ret i64 %mul.val

mul.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %ref_arg = alloca %ori.Rect, align 8
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 3, i64 4)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:                                           ; preds = %bb0
  store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }, ptr %ref_arg, align 8
  %call = call fastcc i64 @_ori_area(ptr %ref_arg)
  %add1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call)
  %add.val2 = extractvalue { i64, i1 } %add1, 0
  %add.ovf3 = extractvalue { i64, i1 } %add1, 1
  br i1 %add.ovf3, label %add.ovf_panic5, label %add.ok4

add.ovf_panic:                                    ; preds = %bb0
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable

add.ok4:                                          ; preds = %add.ok
  ret i64 %add.val2

add.ovf_panic5:                                   ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64) #1

; Function Attrs: cold noreturn
declare void @ori_panic_cstr(ptr) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #1

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
_ori_area:
  push   %rax
  mov    0x10(%rdi),%rax          ; load width (field index 1)
  mov    0x18(%rdi),%rcx          ; load height (field index 2)
  xor    %edx,%edx
  imul   %rcx,%rax                ; width * height
  mov    %rax,(%rsp)
  seto   %al
  jo     1b11e                    ; overflow -> panic
  mov    (%rsp),%rax              ; reload result
  pop    %rcx
  ret

_ori_main:
  sub    $0x38,%rsp
  mov    $0x3,%eax
  add    $0x4,%rax                ; 3 + 4 (not constant-folded)
  mov    %rax,0x10(%rsp)
  seto   %al
  jo     1b18c                    ; overflow check on 3+4 (unnecessary)
  movq   $0x5,0x30(%rsp)         ; store Rect on stack
  movq   $0xa,0x28(%rsp)         ;   width: 10
  movq   $0x4,0x20(%rsp)         ;   Point.y: 4
  movq   $0x3,0x18(%rsp)         ;   Point.x: 3
  lea    0x18(%rsp),%rdi          ; ptr to Rect
  call   _ori_area                ; area(r)
  mov    %rax,%rcx
  mov    0x10(%rsp),%rax          ; reload 3+4 result
  add    %rcx,%rax                ; 7 + area(r)
  mov    %rax,0x8(%rsp)
  seto   %al
  jo     1b1a2                    ; overflow check
  jmp    1b198
  ; ...panic paths...
  mov    0x8(%rsp),%rax
  add    $0x38,%rsp
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @area    | 15     | 11    | 1.36x | NEAR-OPTIMAL |
| 2 | @main    | 16     | 10    | 1.60x | ACCEPTABLE |

**@area (15 actual, 11 ideal, +4 unjustified):**
The function loads `width` and `height` via GEP+load (correct), but then performs a pointless round-trip: `insertvalue` into a zeroinitializer `%ori.Rect`, then immediately `extractvalue` the same indices back out. These 4 instructions (2x `insertvalue`, 2x `extractvalue`) are pure overhead -- the loaded i64 values could feed directly into the overflow-checked multiply.

**@main (16 actual, 10 ideal, +6 unjustified):**
The expression `p.x + p.y` computes `3 + 4` using an overflow-checked `llvm.sadd.with.overflow.i64(i64 3, i64 4)`. Since both operands are compile-time constants, the compiler should constant-fold this to `7`. The 6 extra instructions are: `call sadd` + 2x `extractvalue` + `br` + `call panic` + `unreachable` (the entire first overflow check path). The duplicate panic block `add.ovf_panic` also adds dead weight.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @area    | 0      | 0      | YES      | N/A            | N/A            |
| @main    | 0      | 0      | YES      | N/A            | N/A            |

**Verdict**: Zero RC operations. Point and Rect contain only `int` fields (scalars), so no heap allocation or reference counting is needed. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | uwtable | noundef | readonly | memory | Notes |
|----------|--------|----------|---------|---------|----------|--------|-------|
| @area    | YES    | YES      | YES     | YES     | NO       | NO     | [MEDIUM-1] |
| @main    | NO (C) | YES      | YES     | YES     | N/A      | N/A    |       |

`@_ori_main` correctly uses C calling convention since it is the program entry point called from the C `main()` wrapper.

`@_ori_area` takes a `ptr` parameter that it only reads from (never writes through). It should have `memory(argmem: read)` or `readonly` to enable LLVM alias analysis optimizations. The `ptr` parameter could also carry `readonly` and `noalias` attributes since it points to a caller-owned stack slot. [MEDIUM-1]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @area    | 3      | 0           | 0                 | 0         |       |
| @main    | 5      | 0           | 0                 | 0         | [LOW-1] |

`@_ori_area` has clean 3-block structure: entry, ok (ret), panic. No issues.

`@_ori_main` has 5 blocks: entry `bb0`, `add.ok`, `add.ovf_panic`, `add.ok4`, `add.ovf_panic5`. The first overflow panic block (`add.ovf_panic`) is unreachable dead code since `3 + 4` can never overflow. The duplicate panic blocks (`add.ovf_panic` and `add.ovf_panic5`) both call the same `ori_panic_cstr` with the same message -- they could be merged into one. [LOW-1]

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| mul (width * height) | YES | YES | `llvm.smul.with.overflow.i64` -- justified (runtime values) |
| add (p.x + p.y)      | YES | YES | `llvm.sadd.with.overflow.i64(3, 4)` -- correct but not folded |
| add (7 + area(r))    | YES | YES | `llvm.sadd.with.overflow.i64` -- justified (runtime value) |

All arithmetic operations are overflow-checked. The safety invariant is maintained. The `3 + 4` check is *correct* but wasteful -- constant folding would eliminate it without losing safety.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 868 KiB |
| .rodata section | 134 KiB |
| User code (@area) | 43 bytes (0x1b100-0x1b12a) |
| User code (@main) | 127 bytes (0x1b130-0x1b1ae) |
| User code total | 170 bytes |
| Runtime | >99% of binary |

#### Disassembly: @area

```asm
_ori_area:
  push   %rax                    ; save for stack alignment
  mov    0x10(%rdi),%rax         ; load Rect.width (offset 16)
  mov    0x18(%rdi),%rcx         ; load Rect.height (offset 24)
  xor    %edx,%edx
  imul   %rcx,%rax               ; width * height
  mov    %rax,(%rsp)             ; spill result
  seto   %al                     ; check overflow flag
  jo     panic                   ; jump if overflow
  mov    (%rsp),%rax             ; reload result
  pop    %rcx
  ret
```

11 instructions, 43 bytes. The spill/reload of the multiply result via the stack is suboptimal (LLVM could keep it in a register), but this is a debug-mode artifact.

#### Disassembly: @main

```asm
_ori_main:
  sub    $0x38,%rsp              ; allocate stack frame (56 bytes)
  mov    $0x3,%eax               ; load constant 3
  add    $0x4,%rax               ; 3 + 4 = 7 (not folded at IR level)
  mov    %rax,0x10(%rsp)         ; spill sum
  seto   %al                     ; overflow check (unnecessary for constants)
  jo     panic1
  movq   $0x5,0x30(%rsp)        ; store Rect.height = 5
  movq   $0xa,0x28(%rsp)        ; store Rect.width = 10
  movq   $0x4,0x20(%rsp)        ; store Point.y = 4
  movq   $0x3,0x18(%rsp)        ; store Point.x = 3
  lea    0x18(%rsp),%rdi        ; ptr to Rect
  call   _ori_area               ; area(r)
  mov    %rax,%rcx
  mov    0x10(%rsp),%rax        ; reload 7
  add    %rcx,%rax              ; 7 + 50
  mov    %rax,0x8(%rsp)
  seto   %al
  jo     panic2
  jmp    ret_block              ; unconditional jump to ret
  ...
  mov    0x8(%rsp),%rax         ; reload result
  add    $0x38,%rsp
  ret
```

27 instructions, 127 bytes. The struct is correctly laid out on the stack with fields at the expected offsets. The native code demonstrates correct GEP-to-offset lowering: `Point.x` at +0, `Point.y` at +8, `width` at +16, `height` at +24.

### 7. Optimal IR Comparison

#### @area: Ideal vs Actual

```llvm
; IDEAL (11 instructions)
define fastcc noundef i64 @_ori_area(ptr readonly %0) nounwind {
bb0:
  %width.ptr = getelementptr inbounds %ori.Rect, ptr %0, i32 0, i32 1
  %width = load i64, ptr %width.ptr, align 8
  %height.ptr = getelementptr inbounds %ori.Rect, ptr %0, i32 0, i32 2
  %height = load i64, ptr %height.ptr, align 8
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %width, i64 %height)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %panic, label %ok
ok:
  ret i64 %mul.val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

```llvm
; ACTUAL (15 instructions) -- delta: +4
define fastcc noundef i64 @_ori_area(ptr %0) #0 {
bb0:
  %param.load.f1.ptr = getelementptr inbounds nuw %ori.Rect, ptr %0, i32 0, i32 1
  %param.load.f1 = load i64, ptr %param.load.f1.ptr, align 8
  %param.load.s1 = insertvalue %ori.Rect zeroinitializer, i64 %param.load.f1, 1   ; UNJUSTIFIED
  %param.load.f2.ptr = getelementptr inbounds nuw %ori.Rect, ptr %0, i32 0, i32 2
  %param.load.f2 = load i64, ptr %param.load.f2.ptr, align 8
  %param.load.s2 = insertvalue %ori.Rect %param.load.s1, i64 %param.load.f2, 2    ; UNJUSTIFIED
  %proj.1 = extractvalue %ori.Rect %param.load.s2, 1                              ; UNJUSTIFIED
  %proj.2 = extractvalue %ori.Rect %param.load.s2, 2                              ; UNJUSTIFIED
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %proj.1, i64 %proj.2)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok
mul.ok:
  ret i64 %mul.val
mul.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: +4 instructions. The codegen loads individual fields via GEP then reconstructs a partial struct value via `insertvalue`, only to immediately destructure it with `extractvalue`. This insert-extract round-trip is a no-op that LLVM will optimize away, but it should not be emitted in the first place. The root cause is the codegen pattern for struct parameter access: it always reconstructs the struct in SSA form rather than using loaded field values directly.

#### @main: Ideal vs Actual

```llvm
; IDEAL (10 instructions)
define noundef i64 @_ori_main() nounwind {
bb0:
  %ref_arg = alloca %ori.Rect, align 8
  store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }, ptr %ref_arg, align 8
  %call = call fastcc i64 @_ori_area(ptr %ref_arg)
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 7, i64 %call)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %panic, label %ok
ok:
  ret i64 %add.val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
}
```

**Delta**: +6 instructions. The compiler emits an overflow-checked `sadd(3, 4)` instead of constant-folding to `7`. This produces an entire extra overflow check path (call, 2x extractvalue, br) plus a dead panic block (call + unreachable). The constant-fold optimization is missing in the LLVM codegen for constant-only arithmetic expressions.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @area    | 11    | 15     | +4    | NO        | NEAR-OPTIMAL |
| @main    | 10    | 16     | +6    | NO        | ACCEPTABLE |

### 8. Structs: Field Access

Struct field access is lowered to `getelementptr` + `load` sequences, which is the standard LLVM pattern for accessing aggregate member fields.

**Type layout:**
- `%ori.Point = type { i64, i64 }` -- 16 bytes, 2 fields
- `%ori.Rect = type { %ori.Point, i64, i64 }` -- 32 bytes, nested Point + 2 scalars

**Field offset mapping (verified in disassembly):**
| Struct | Field | GEP Index | Byte Offset | Correct |
|--------|-------|-----------|-------------|---------|
| Point  | x     | 0, 0      | +0          | YES     |
| Point  | y     | 0, 1      | +8          | YES     |
| Rect   | origin | 0, 0     | +0          | YES     |
| Rect   | width  | 0, 1     | +16         | YES     |
| Rect   | height | 0, 2     | +24         | YES     |

The disassembly confirms correct offset arithmetic: `mov 0x10(%rdi),%rax` (width at +16) and `mov 0x18(%rdi),%rcx` (height at +24).

**Calling convention for structs:**
`@area` receives `Rect` by pointer (`ptr %0`), which is correct for a 32-byte aggregate. The caller allocates the struct on the stack via `alloca` and passes a pointer. This avoids expensive value-type register passing for large structs.

### 9. Structs: Layout

**Nested struct flattening:**
The `Rect` type embeds `Point` directly (not by pointer). In LLVM IR, `%ori.Rect = type { %ori.Point, i64, i64 }` nests the Point struct inline. The total size is 32 bytes (4 x i64) with natural alignment.

**Stack layout in @main:**
The disassembly shows the Rect is stored at `%rsp+0x18` with fields written in reverse order:
- `%rsp+0x30`: height (5)
- `%rsp+0x28`: width (10)
- `%rsp+0x20`: Point.y (4)
- `%rsp+0x18`: Point.x (3)

This matches the expected memory layout of `{ { i64, i64 }, i64, i64 }`.

**Struct construction:**
The IR uses a single `store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }` constant aggregate store. This is efficient -- a single store of a constant struct rather than field-by-field stores. The native code breaks it into 4 `movq` instructions (one per i64 field), which is expected since x86_64 cannot store a 32-byte immediate in one instruction.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | MEDIUM   | IR Quality | Redundant insertvalue/extractvalue round-trip in @area | NEW | J4 |
| 2 | MEDIUM   | IR Quality | Missing constant folding for `3 + 4` in @main | NEW | J4 |
| 3 | MEDIUM   | Attributes | Missing `readonly`/`memory(read)` on @area ptr param | NEW | J4 |
| 4 | LOW      | Control Flow | Duplicate and dead overflow panic blocks in @main | NEW | J4 |
| 5 | NOTE     | ARC | Zero RC operations -- all-scalar structs need no heap management | NEW | J4 |
| 6 | NOTE     | Structs | Correct nested struct layout and GEP offset computation | NEW | J4 |
| 7 | NOTE     | Structs | Efficient constant aggregate store for struct construction | NEW | J4 |

### MEDIUM-1: Missing readonly/memory attributes on @area

**Location**: `@_ori_area` function declaration and `ptr %0` parameter
**Impact**: LLVM cannot infer that `@_ori_area` only reads from its pointer argument, preventing potential load-store optimizations and alias analysis at call sites.
**Fix**: Add `memory(argmem: read)` to `@_ori_area` and/or `readonly` + `noalias` to the `ptr` parameter. The nounwind analysis pass already exists; a similar read-only analysis could propagate `memory` attributes.
**First seen**: Journey 4
**Found in**: Attributes & Calling Convention (Category 3)

### MEDIUM-2: Redundant insertvalue/extractvalue round-trip in @area

**Location**: `@_ori_area`, instructions `%param.load.s1` through `%proj.2`
**Impact**: 4 unnecessary IR instructions per function call. While LLVM's `instcombine` pass will eliminate these in optimized builds, they bloat unoptimized IR and slow debug-mode execution.
**Fix**: When accessing struct fields from a pointer parameter, use the loaded i64 values directly instead of reconstructing a partial struct via `insertvalue` then re-extracting with `extractvalue`.
**First seen**: Journey 4
**Found in**: Optimal IR Comparison (Category 7)

### MEDIUM-3: Missing constant folding for compile-time arithmetic

**Location**: `@_ori_main`, `%add = call ... @llvm.sadd.with.overflow.i64(i64 3, i64 4)`
**Impact**: 6 unnecessary instructions (overflow check + dead panic block) for an expression whose result is statically known to be 7. Wastes code size and execution time in debug builds.
**Fix**: Implement constant folding in the LLVM codegen: when both operands of an arithmetic operation are compile-time constants, emit the result directly (e.g., `i64 7`) and skip the overflow check. The canonicalizer or a pre-codegen pass could handle this.
**First seen**: Journey 4
**Found in**: Optimal IR Comparison (Category 7)

### LOW-1: Duplicate overflow panic blocks

**Location**: `@_ori_main`, blocks `add.ovf_panic` and `add.ovf_panic5`
**Impact**: Both blocks call `ori_panic_cstr` with the same message pointer `@ovf.msg.1`. They could share a single panic block, saving code size.
**Fix**: When multiple overflow checks use the same panic message, merge their panic blocks into one shared target.
**First seen**: Journey 4
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-1: Zero RC operations on all-scalar structs

**Location**: All functions
**Impact**: Positive -- Point and Rect contain only `int` fields, so no heap allocation or reference counting is needed. The ARC pipeline correctly identifies this and emits zero RC operations.
**Found in**: ARC Purity (Category 2)

### NOTE-2: Correct nested struct layout

**Location**: `%ori.Rect = type { %ori.Point, i64, i64 }`
**Impact**: Positive -- nested struct is inlined (not heap-indirected), field offsets are correctly computed via GEP indices, and the disassembly confirms correct byte offsets (+0/+8/+16/+24).
**Found in**: Structs: Field Access (Category 8)

### NOTE-3: Efficient constant aggregate store

**Location**: `@_ori_main`, `store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }`
**Impact**: Positive -- struct construction uses a single constant aggregate store rather than field-by-field stores. This is a good codegen pattern.
**Found in**: Structs: Layout (Category 9)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 7/10 | 1.48x avg ratio (max 1.60x) |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 7/10 | 83.3% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 6/10 | 10 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 8.5 / 10**

## Verdict

Journey 4's struct codegen is solid but has room for improvement. Struct types are correctly lowered to LLVM aggregate types with proper nested layout, and field access compiles to efficient GEP+load sequences verified at the disassembly level. ARC is perfectly clean with zero RC operations on all-scalar structs. The two main codegen weaknesses are: (1) a redundant insertvalue/extractvalue round-trip when accessing struct fields from pointer parameters, and (2) missing constant folding for compile-time arithmetic (`3 + 4` emits a full overflow check instead of folding to `7`). Both are correctness-preserving inefficiencies that LLVM's optimizer would clean up, but they should ideally not be emitted in the first place.
