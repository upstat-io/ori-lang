---
journey: 11
slug: derived-traits
theme: "I am a derived trait"
date: 2026-03-06
status: PASS
expected: 33
eval_result: 33
aot_result: 33
difficulty: complex
prerequisites:
  - "Understanding of struct and sum type definitions"
  - "Familiarity with equality comparison concepts"
  - "Knowledge of trait derivation / automatic method generation"
learning_objectives:
  - "See how #derive(Eq) generates per-type comparison functions in LLVM IR"
  - "Understand tag-based dispatch for sum type equality"
  - "Compare struct field-by-field vs sum type variant-aware equality codegen"
  - "Evaluate the short-circuit pattern in derived equality methods"
features:
  - derived_traits
  - trait_methods
  - struct_construction
  - sum_types
  - pattern_matching
feature_description: "Derived Eq trait on structs and sum types with field and tag-based equality"
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
  attr_applicable: 19
  attr_correct: 18
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
  - journey: 2
    relationship: "Both test branching codegen via if/then/else"
  - journey: 4
    relationship: "J4 tests struct construction; J11 tests derived Eq on structs"
---

# Journey 11: "I am a derived trait"

## Source

```ori
// Journey 11: "I am a derived trait"
// Slug: derived-traits
// Difficulty: complex
// Features: derived_traits, trait_methods, struct_construction, sum_types, pattern_matching
// Expected: check_struct_eq() + check_sum_eq() + check_nested() = 7 + 11 + 15 = 33

#[derive(Eq)]
type Point = { x: int, y: int }

#[derive(Eq)]
type Color = Red | Green | Blue;

#[derive(Eq)]
type Shape = Circle(radius: int) | Rect(w: int, h: int);

@check_struct_eq () -> int = {
    let p1 = Point { x: 10, y: 20 };
    let p2 = Point { x: 10, y: 20 };
    let p3 = Point { x: 10, y: 30 };
    let same = if p1 == p2 then 3 else 0;
    let diff = if p1 != p3 then 4 else 0;
    same + diff
    // = 3 + 4 = 7
}

@check_sum_eq () -> int = {
    let c1 = Red;
    let c2 = Red;
    let c3 = Blue;
    let unit_same = if c1 == c2 then 5 else 0;
    let unit_diff = if c1 != c3 then 6 else 0;
    unit_same + unit_diff
    // = 5 + 6 = 11
}

@check_nested () -> int = {
    let s1 = Circle(radius: 10);
    let s2 = Circle(radius: 10);
    let s3 = Rect(w: 5, h: 8);
    let payload_same = if s1 == s2 then 7 else 0;
    let payload_diff = if s1 != s3 then 8 else 0;
    payload_same + payload_diff
    // = 7 + 8 = 15
}

@main () -> int = {
    let a = check_struct_eq();   // = 7
    let b = check_sum_eq();      // = 11
    let c = check_nested();      // = 15
    a + b + c                    // = 33
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

**Tokens**: 346 | **Keywords**: ~40 | **Identifiers**: ~80 | **Errors**: 0

<details>
<summary>Token stream (excerpt)</summary>

```text
Hash LBracket Ident(derive) LParen Ident(Eq) RParen RBracket
Ident(type) Ident(Point) Eq LBrace Ident(x) Colon Ident(int)
Comma Ident(y) Colon Ident(int) RBrace
Hash LBracket Ident(derive) LParen Ident(Eq) RParen RBracket
Ident(type) Ident(Color) Eq Ident(Red) Pipe Ident(Green) Pipe Ident(Blue) Semi
Hash LBracket Ident(derive) LParen Ident(Eq) RParen RBracket
Ident(type) Ident(Shape) Eq Ident(Circle) LParen Ident(radius) Colon Ident(int) RParen
  Pipe Ident(Rect) LParen Ident(w) Colon Ident(int) Comma Ident(h) Colon Ident(int) RParen Semi
Fn(@) Ident(check_struct_eq) LParen RParen Arrow Ident(int) Eq LBrace ...
```

</details>

### 2. Parser

> The parser transforms the flat token stream into a hierarchical Abstract Syntax Tree
> (AST) -- a tree structure that represents the grammatical structure of the program.

**Nodes**: 85 | **Max depth**: 5 | **Functions**: 4 | **Errors**: 0

<details>
<summary>AST (simplified)</summary>

```text
Module
├─ TypeDecl Point #derive(Eq)
│  └─ Struct { x: int, y: int }
├─ TypeDecl Color #derive(Eq)
│  └─ Sum: Red | Green | Blue
├─ TypeDecl Shape #derive(Eq)
│  └─ Sum: Circle(radius: int) | Rect(w: int, h: int)
├─ FnDecl @check_struct_eq
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let p1 = Point { x: 10, y: 20 }
│       ├─ Let p2 = Point { x: 10, y: 20 }
│       ├─ Let p3 = Point { x: 10, y: 30 }
│       ├─ Let same = If(p1 == p2, 3, 0)
│       ├─ Let diff = If(p1 != p3, 4, 0)
│       └─ BinOp(+): same + diff
├─ FnDecl @check_sum_eq
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let c1 = Red
│       ├─ Let c2 = Red
│       ├─ Let c3 = Blue
│       ├─ Let unit_same = If(c1 == c2, 5, 0)
│       ├─ Let unit_diff = If(c1 != c3, 6, 0)
│       └─ BinOp(+): unit_same + unit_diff
├─ FnDecl @check_nested
│  ├─ Return: int
│  └─ Body: Block
│       ├─ Let s1 = Circle(radius: 10)
│       ├─ Let s2 = Circle(radius: 10)
│       ├─ Let s3 = Rect(w: 5, h: 8)
│       ├─ Let payload_same = If(s1 == s2, 7, 0)
│       ├─ Let payload_diff = If(s1 != s3, 8, 0)
│       └─ BinOp(+): payload_same + payload_diff
└─ FnDecl @main
   ├─ Return: int
   └─ Body: Block
        ├─ Let a = Call(@check_struct_eq)
        ├─ Let b = Call(@check_sum_eq)
        ├─ Let c = Call(@check_nested)
        └─ BinOp(+): BinOp(+): a + b + c
```

</details>

### 3. Type Checker

> The type checker verifies that all expressions have compatible types using
> Hindley-Milner type inference. It resolves type variables, checks constraints,
> and ensures type safety without requiring explicit type annotations everywhere.

**Constraints**: ~24 | **Types inferred**: 13 | **Unifications**: ~18 | **Errors**: 0

<details>
<summary>Inferred types</summary>

```ori
// All types resolved:
// Point: { x: int, y: int } with derived Eq
// Color: Red | Green | Blue with derived Eq
// Shape: Circle(radius: int) | Rect(w: int, h: int) with derived Eq

@check_struct_eq () -> int = {
    let p1: Point = Point { x: 10, y: 20 }
    let p2: Point = Point { x: 10, y: 20 }
    let p3: Point = Point { x: 10, y: 30 }
    let same: int = if p1 == p2 then 3 else 0
    //                 ^ Point.equals(self: Point, other: Point) -> bool
    let diff: int = if p1 != p3 then 4 else 0
    //                 ^ !(Point.equals(self: Point, other: Point)) -> bool
    same + diff  // -> int
}

@check_sum_eq () -> int = {
    let c1: Color = Red    // Color, tag = 0
    let c2: Color = Red    // Color, tag = 0
    let c3: Color = Blue   // Color, tag = 2
    let unit_same: int = if c1 == c2 then 5 else 0
    //                      ^ Color.equals(self: Color, other: Color) -> bool
    let unit_diff: int = if c1 != c3 then 6 else 0
    unit_same + unit_diff  // -> int
}

@check_nested () -> int = {
    let s1: Shape = Circle(radius: 10)  // tag=0, payload=[10, 0]
    let s2: Shape = Circle(radius: 10)  // tag=0, payload=[10, 0]
    let s3: Shape = Rect(w: 5, h: 8)   // tag=1, payload=[5, 8]
    let payload_same: int = if s1 == s2 then 7 else 0
    //                         ^ Shape.equals(self: Shape, other: Shape) -> bool
    let payload_diff: int = if s1 != s3 then 8 else 0
    payload_same + payload_diff  // -> int
}

@main () -> int = {
    let a: int = check_struct_eq()   // -> int
    let b: int = check_sum_eq()      // -> int
    let c: int = check_nested()      // -> int
    a + b + c  // -> int (Add<int, int> -> int)
}
```

</details>

### 4. Canonicalization

> The canonicalizer transforms the typed AST into a simplified canonical form.
> It desugars syntactic sugar, lowers complex expressions, and prepares the IR
> for backend consumption.

**Transforms**: 6 | **Desugared**: 6 | **Errors**: 0

<details>
<summary>Key transformations</summary>

```text
- `==` desugared to call to derived $eq method
- `!=` desugared to call to derived $eq method + boolean negation
- Struct literals lowered to field-ordered construction
- Sum type variant construction lowered to tag + payload
- 6 constant int values propagated
- Function bodies normalized to canonical expression form
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
@check_struct_eq: no heap values -- Point is 2x i64, passed by value
@check_sum_eq: no heap values -- Color is 1x i64 tag, passed by value
@check_nested: no heap values -- Shape is i64 tag + [2 x i64] payload, passed by ptr (stack)
@main: no heap values -- pure scalar results
Point$eq: no RC -- compares two Points by value
Color$eq: no RC -- compares two Colors by tag
Shape$eq: no RC -- compares two Shapes by ptr, all-int payload
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
  let a = @check_struct_eq()
    let p1 = Point { x: 10, y: 20 }
    let p2 = Point { x: 10, y: 20 }
    let p3 = Point { x: 10, y: 30 }
    p1 == p2 -> Point.equals -> true -> same = 3
    p1 != p3 -> Point.equals -> false -> !false -> true -> diff = 4
    3 + 4 = 7
  -> a = 7
  let b = @check_sum_eq()
    let c1 = Red (tag=0)
    let c2 = Red (tag=0)
    let c3 = Blue (tag=2)
    c1 == c2 -> Color.equals -> true -> unit_same = 5
    c1 != c3 -> Color.equals -> false -> !false -> true -> unit_diff = 6
    5 + 6 = 11
  -> b = 11
  let c = @check_nested()
    let s1 = Circle(radius: 10)
    let s2 = Circle(radius: 10)
    let s3 = Rect(w: 5, h: 8)
    s1 == s2 -> Shape.equals -> tags match (0==0), Circle: radius 10==10 -> true -> payload_same = 7
    s1 != s3 -> Shape.equals -> tags differ (0!=1) -> false -> !false -> true -> payload_diff = 8
    7 + 8 = 15
  -> c = 15
  7 + 11 + 15 = 33
-> 33
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
@check_struct_eq: +0 rc_inc, +0 rc_dec (scalar structs, no heap)
@check_sum_eq: +0 rc_inc, +0 rc_dec (unit-variant enum, no heap)
@check_nested: +0 rc_inc, +0 rc_dec (payload enum on stack, no heap)
@main: +0 rc_inc, +0 rc_dec (scalar returns)
Point$eq: +0 rc_inc, +0 rc_dec (by-value comparison)
Color$eq: +0 rc_inc, +0 rc_dec (tag comparison)
Shape$eq: +0 rc_inc, +0 rc_dec (by-ptr comparison)
```

</details>

#### Generated LLVM IR

```llvm
; ModuleID = '11-derived-traits'
source_filename = "11-derived-traits"

%ori.Point = type { i64, i64 }
%ori.Color = type { i64 }
%ori.Shape = type { i64, [2 x i64] }

@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1

; Function Attrs: nounwind uwtable
; --- @check_struct_eq ---
define fastcc noundef i64 @_ori_check_struct_eq() #0 {
bb0:
  %eq_trait = call fastcc i1 @"_ori_Point$eq"(%ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 20 })
  %sel = select i1 %eq_trait, i64 3, i64 0
  %eq_trait1 = call fastcc i1 @"_ori_Point$eq"(%ori.Point { i64 10, i64 20 }, %ori.Point { i64 10, i64 30 })
  %neq = xor i1 %eq_trait1, true
  %sel2 = select i1 %neq, i64 4, i64 0
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %sel, i64 %sel2)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @check_sum_eq ---
define fastcc noundef i64 @_ori_check_sum_eq() #0 {
bb0:
  %eq_trait = call fastcc i1 @"_ori_Color$eq"(%ori.Color zeroinitializer, %ori.Color zeroinitializer)
  %sel = select i1 %eq_trait, i64 5, i64 0
  %eq_trait1 = call fastcc i1 @"_ori_Color$eq"(%ori.Color zeroinitializer, %ori.Color { i64 2 })
  %neq = xor i1 %eq_trait1, true
  %sel2 = select i1 %neq, i64 6, i64 0
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %sel, i64 %sel2)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @check_nested ---
define fastcc noundef i64 @_ori_check_nested() #0 {
bb0:
  %ref_arg3 = alloca %ori.Shape, align 8
  %ref_arg2 = alloca %ori.Shape, align 8
  %ref_arg1 = alloca %ori.Shape, align 8
  %ref_arg = alloca %ori.Shape, align 8
  store %ori.Shape { i64 0, [2 x i64] [i64 10, i64 0] }, ptr %ref_arg, align 8
  store %ori.Shape { i64 0, [2 x i64] [i64 10, i64 0] }, ptr %ref_arg1, align 8
  %eq_trait = call fastcc i1 @"_ori_Shape$eq"(ptr %ref_arg, ptr %ref_arg1)
  %sel = select i1 %eq_trait, i64 7, i64 0
  store %ori.Shape { i64 0, [2 x i64] [i64 10, i64 0] }, ptr %ref_arg2, align 8
  store %ori.Shape { i64 1, [2 x i64] [i64 5, i64 8] }, ptr %ref_arg3, align 8
  %eq_trait4 = call fastcc i1 @"_ori_Shape$eq"(ptr %ref_arg2, ptr %ref_arg3)
  %neq = xor i1 %eq_trait4, true
  %sel5 = select i1 %neq, i64 8, i64 0
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %sel, i64 %sel5)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_check_struct_eq()
  %call1 = call fastcc i64 @_ori_check_sum_eq()
  %call2 = call fastcc i64 @_ori_check_nested()
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
; --- Point.@eq ---
define fastcc noundef i1 @"_ori_Point$eq"(%ori.Point %0, %ori.Point %1) #0 {
entry:
  %eq.self.x = extractvalue %ori.Point %0, 0
  %eq.other.x = extractvalue %ori.Point %1, 0
  %eq.x = icmp eq i64 %eq.self.x, %eq.other.x
  br i1 %eq.x, label %eq.field.1, label %eq.false

eq.true:
  ret i1 true

eq.false:
  ret i1 false

eq.field.1:
  %eq.self.y = extractvalue %ori.Point %0, 1
  %eq.other.y = extractvalue %ori.Point %1, 1
  %eq.y = icmp eq i64 %eq.self.y, %eq.other.y
  br i1 %eq.y, label %eq.true, label %eq.false
}

; Function Attrs: nounwind uwtable
; --- Color.@eq ---
define fastcc noundef i1 @"_ori_Color$eq"(%ori.Color %0, %ori.Color %1) #0 {
entry:
  %eq.tag.self = extractvalue %ori.Color %0, 0
  %eq.tag.other = extractvalue %ori.Color %1, 0
  %eq.tags = icmp eq i64 %eq.tag.self, %eq.tag.other
  br i1 %eq.tags, label %eq.true, label %eq.false

eq.true:
  ret i1 true

eq.false:
  ret i1 false
}

; Function Attrs: nounwind uwtable
; --- Shape.@eq ---
define fastcc noundef i1 @"_ori_Shape$eq"(ptr %0, ptr %1) #0 {
entry:
  %param.0.f0.ptr = getelementptr inbounds nuw %ori.Shape, ptr %0, i32 0, i32 0
  %param.0.f0 = load i64, ptr %param.0.f0.ptr, align 8
  %param.0.s0 = insertvalue %ori.Shape zeroinitializer, i64 %param.0.f0, 0
  %param.0.f1.ptr = getelementptr inbounds nuw %ori.Shape, ptr %0, i32 0, i32 1
  %param.0.f1 = load [2 x i64], ptr %param.0.f1.ptr, align 8
  %param.0.s1 = insertvalue %ori.Shape %param.0.s0, [2 x i64] %param.0.f1, 1
  %param.1.f0.ptr = getelementptr inbounds nuw %ori.Shape, ptr %1, i32 0, i32 0
  %param.1.f0 = load i64, ptr %param.1.f0.ptr, align 8
  %param.1.s0 = insertvalue %ori.Shape zeroinitializer, i64 %param.1.f0, 0
  %param.1.f1.ptr = getelementptr inbounds nuw %ori.Shape, ptr %1, i32 0, i32 1
  %param.1.f1 = load [2 x i64], ptr %param.1.f1.ptr, align 8
  %param.1.s1 = insertvalue %ori.Shape %param.1.s0, [2 x i64] %param.1.f1, 1
  %eq.tag.self = extractvalue %ori.Shape %param.0.s1, 0
  %eq.tag.other = extractvalue %ori.Shape %param.1.s1, 0
  %eq.tags = icmp eq i64 %eq.tag.self, %eq.tag.other
  br i1 %eq.tags, label %eq.tags.match, label %eq.false

eq.true:
  ret i1 true

eq.false:
  ret i1 false

eq.tags.match:
  %eq.self.payload = extractvalue %ori.Shape %param.0.s1, 1
  %eq.other.payload = extractvalue %ori.Shape %param.1.s1, 1
  switch i64 %eq.tag.self, label %eq.false [
    i64 0, label %eq.v.Circle
    i64 1, label %eq.v.Rect
  ]

eq.v.Circle:
  %eq.v0.self.f0 = extractvalue [2 x i64] %eq.self.payload, 0
  %eq.v0.other.f0 = extractvalue [2 x i64] %eq.other.payload, 0
  %eq.v0.f0 = icmp eq i64 %eq.v0.self.f0, %eq.v0.other.f0
  br i1 %eq.v0.f0, label %eq.true, label %eq.false

eq.v.Rect:
  %eq.v1.self.f0 = extractvalue [2 x i64] %eq.self.payload, 0
  %eq.v1.other.f0 = extractvalue [2 x i64] %eq.other.payload, 0
  %eq.v1.f0 = icmp eq i64 %eq.v1.self.f0, %eq.v1.other.f0
  br i1 %eq.v1.f0, label %eq.v1.f1, label %eq.false

eq.v1.f1:
  %eq.v1.self.f1 = extractvalue [2 x i64] %eq.self.payload, 1
  %eq.v1.other.f1 = extractvalue [2 x i64] %eq.other.payload, 1
  %eq.v1.f11 = icmp eq i64 %eq.v1.self.f1, %eq.v1.other.f1
  br i1 %eq.v1.f11, label %eq.true, label %eq.false
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
_ori_check_struct_eq:
  sub    $0x18,%rsp
  mov    $0xa,%edx
  mov    $0x14,%ecx
  mov    %rdx,%rdi
  mov    %rcx,%rsi
  call   _ori_Point$eq
  mov    %al,%dl
  xor    %eax,%eax
  mov    $0x3,%ecx
  test   $0x1,%dl
  cmovne %rcx,%rax
  mov    %rax,0x8(%rsp)
  mov    $0x14,%esi
  mov    $0xa,%edx
  mov    $0x1e,%ecx
  mov    %rdx,%rdi
  call   _ori_Point$eq
  mov    %al,%sil
  mov    0x8(%rsp),%rax
  xor    $0xff,%sil
  xor    %ecx,%ecx
  mov    $0x4,%edx
  test   $0x1,%sil
  cmovne %rdx,%rcx
  add    %rcx,%rax
  mov    %rax,0x10(%rsp)
  seto   %al
  jo     .overflow
  mov    0x10(%rsp),%rax
  add    $0x18,%rsp
  ret
.overflow:
  lea    ovf.msg(%rip),%rdi
  call   ori_panic_cstr

_ori_Point$eq:
  mov    %rcx,-0x10(%rsp)
  mov    %rsi,-0x8(%rsp)
  cmp    %rdx,%rdi           ; compare x fields
  je     .check_y
  jmp    .false
.true:
  mov    $0x1,%al
  ret
.false:
  xor    %eax,%eax
  ret
.check_y:
  mov    -0x8(%rsp),%rax
  mov    -0x10(%rsp),%rcx
  cmp    %rcx,%rax           ; compare y fields
  je     .true
  jmp    .false

_ori_Color$eq:
  cmp    %rsi,%rdi           ; compare tags directly
  jne    .false
  mov    $0x1,%al
  ret
.false:
  xor    %eax,%eax
  ret

_ori_Shape$eq:
  mov    0x10(%rdi),%rax     ; load self payload[1]
  mov    (%rdi),%rax         ; load self tag
  mov    0x8(%rdi),%rcx      ; load self payload[0]
  mov    0x10(%rsi),%rcx     ; load other payload[1]
  mov    (%rsi),%rcx         ; load other tag
  mov    0x8(%rsi),%rdx      ; load other payload[0]
  cmp    %rcx,%rax           ; compare tags
  je     .tags_match
  jmp    .false
  ; ... switch on tag, per-variant field comparison ...
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @check_struct_eq | 12 | 12 | 1.00x | OPTIMAL |
| 2 | @check_sum_eq | 12 | 12 | 1.00x | OPTIMAL |
| 3 | @check_nested | 20 | 20 | 1.00x | OPTIMAL |
| 4 | @main | 16 | 16 | 1.00x | OPTIMAL |
| 5 | Point$eq | 10 | 10 | 1.00x | OPTIMAL |
| 6 | Color$eq | 6 | 6 | 1.00x | OPTIMAL |
| 7 | Shape$eq | 33 | 33 | 1.00x | OPTIMAL |

All user functions and derived methods are OPTIMAL. The `check_*` functions efficiently use `select` for conditional values and `call` for derived equality, with overflow-checked addition for the final sum. The derived `$eq` methods use short-circuit field comparison (struct) and tag-then-switch dispatch (sum types) with no wasted instructions.

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @check_struct_eq | 0 | 0 | YES | N/A | N/A |
| @check_sum_eq | 0 | 0 | YES | N/A | N/A |
| @check_nested | 0 | 0 | YES | N/A | N/A |
| @main | 0 | 0 | YES | N/A | N/A |
| Point$eq | 0 | 0 | YES | N/A | N/A |
| Color$eq | 0 | 0 | YES | N/A | N/A |
| Shape$eq | 0 | 0 | YES | N/A | N/A |

**Verdict**: All types in this journey contain only `int` fields (scalar data). No heap allocations, no reference counting needed. Zero RC operations across the entire module. OPTIMAL.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noundef | uwtable | cold | Notes |
|----------|--------|----------|---------|---------|------|-------|
| @check_struct_eq | YES | YES | YES | YES | NO | |
| @check_sum_eq | YES | YES | YES | YES | NO | |
| @check_nested | YES | YES | YES | YES | NO | |
| @main | NO (C) | YES | YES | YES | NO | Correct: C entry point |
| Point$eq | YES | YES | YES | YES | NO | |
| Color$eq | YES | YES | YES | YES | NO | |
| Shape$eq | YES | YES | YES | YES | NO | |
| ori_panic_cstr | N/A | N/A | N/A | N/A | YES | `cold noreturn` -- correct |
| main (wrapper) | NO (C) | YES | N/A | NO | NO | [LOW-1] |

The `main()` C wrapper lacks `uwtable`. This is a minor gap since the wrapper is trivial (call + trunc + ret). All user functions and derived methods have the full attribute set. The `@_ori_main` correctly uses C calling convention rather than `fastcc` since it's the module entry point called from the C wrapper.

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|-------------|-------------------|-----------|-------|
| @check_struct_eq | 3 | 0 | 0 | 0 | |
| @check_sum_eq | 3 | 0 | 0 | 0 | |
| @check_nested | 3 | 0 | 0 | 0 | |
| @main | 5 | 0 | 0 | 0 | |
| Point$eq | 4 | 0 | 0 | 0 | Short-circuit: 2 field checks |
| Color$eq | 3 | 0 | 0 | 0 | Tag-only: 1 branch |
| Shape$eq | 8 | 0 | 0 | 0 | Tag dispatch + per-variant checks |

Control flow is clean across all functions. The `@main` has 5 blocks due to two overflow-checked additions (a+b, then +c), each needing ok/panic paths. The derived `$eq` methods use an efficient branching structure: `Point$eq` short-circuits after the first field mismatch, `Color$eq` is a simple tag comparison, and `Shape$eq` uses a `switch` instruction for tag dispatch into per-variant comparison blocks.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| add (check_struct_eq) | YES | YES | `llvm.sadd.with.overflow.i64` for same + diff |
| add (check_sum_eq) | YES | YES | `llvm.sadd.with.overflow.i64` for unit_same + unit_diff |
| add (check_nested) | YES | YES | `llvm.sadd.with.overflow.i64` for payload_same + payload_diff |
| add (main, 1st) | YES | YES | `llvm.sadd.with.overflow.i64` for a + b |
| add (main, 2nd) | YES | YES | `llvm.sadd.with.overflow.i64` for (a+b) + c |

All 5 addition operations are overflow-checked using LLVM intrinsics with proper panic on overflow. No arithmetic operations are missed.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 6.25 MiB (debug) |
| .text section | 869 KiB |
| .rodata section | 133 KiB |
| User code | 819 bytes (all 7 functions + main wrapper) |
| Runtime | >99% of binary |

#### Disassembly: Point$eq

```asm
_ori_Point$eq:
  mov    %rcx,-0x10(%rsp)     ; spill y fields
  mov    %rsi,-0x8(%rsp)
  cmp    %rdx,%rdi             ; compare x fields (rdi=self.x, rdx=other.x)
  je     .check_y              ; x equal -> check y
  jmp    .false                ; x differ -> false
.true:
  mov    $0x1,%al
  ret
.false:
  xor    %eax,%eax
  ret
.check_y:
  mov    -0x8(%rsp),%rax       ; reload self.y
  mov    -0x10(%rsp),%rcx      ; reload other.y
  cmp    %rcx,%rax             ; compare y fields
  je     .true
  jmp    .false
```

Point is passed by value in 4 registers (rdi=self.x, rsi=self.y, rdx=other.x, rcx=other.y). The short-circuit comparison is clean: check x first, skip y if x differs.

#### Disassembly: Color$eq

```asm
_ori_Color$eq:
  cmp    %rsi,%rdi             ; compare tags directly
  jne    .false
  mov    $0x1,%al              ; true
  ret
.false:
  xor    %eax,%eax             ; false
  ret
```

Color compiles to 5 instructions. Pure tag comparison with no payload -- the ideal case for unit-variant sum types.

### 7. Optimal IR Comparison

#### @check_struct_eq: Ideal vs Actual

```llvm
; IDEAL (12 instructions)
define fastcc i64 @_ori_check_struct_eq() nounwind {
bb0:
  %eq = call fastcc i1 @"_ori_Point$eq"(%ori.Point {i64 10, i64 20}, %ori.Point {i64 10, i64 20})
  %sel = select i1 %eq, i64 3, i64 0
  %eq1 = call fastcc i1 @"_ori_Point$eq"(%ori.Point {i64 10, i64 20}, %ori.Point {i64 10, i64 30})
  %neq = xor i1 %eq1, true
  %sel2 = select i1 %neq, i64 4, i64 0
  %r = call {i64,i1} @llvm.sadd.with.overflow.i64(i64 %sel, i64 %sel2)
  %v = extractvalue {i64,i1} %r, 0
  %o = extractvalue {i64,i1} %r, 1
  br i1 %o, label %panic, label %ok
ok:
  ret i64 %v
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```

**Delta**: 0 instructions. Actual matches ideal exactly.

#### Point$eq: Ideal vs Actual

```llvm
; IDEAL (10 instructions -- short-circuit field comparison)
define fastcc i1 @"_ori_Point$eq"(%ori.Point %0, %ori.Point %1) nounwind {
entry:
  %sx = extractvalue %ori.Point %0, 0
  %ox = extractvalue %ori.Point %1, 0
  %cx = icmp eq i64 %sx, %ox
  br i1 %cx, label %f1, label %false
false:
  ret i1 false
true:
  ret i1 true
f1:
  %sy = extractvalue %ori.Point %0, 1
  %oy = extractvalue %ori.Point %1, 1
  %cy = icmp eq i64 %sy, %oy
  br i1 %cy, label %true, label %false
}
```

**Delta**: 0 instructions. The short-circuit pattern is identical.

#### Color$eq: Ideal vs Actual

```llvm
; IDEAL (6 instructions -- tag-only comparison)
define fastcc i1 @"_ori_Color$eq"(%ori.Color %0, %ori.Color %1) nounwind {
entry:
  %t0 = extractvalue %ori.Color %0, 0
  %t1 = extractvalue %ori.Color %1, 0
  %eq = icmp eq i64 %t0, %t1
  br i1 %eq, label %true, label %false
true:
  ret i1 true
false:
  ret i1 false
}
```

**Delta**: 0 instructions. Unit-variant enum equality is a pure tag comparison.

#### Shape$eq: Ideal vs Actual

```llvm
; IDEAL (33 instructions)
; Needs ptr-to-value reconstruction (12 GEP+load+insertvalue for 2 params)
; Then tag compare, switch dispatch, per-variant field comparison
; The 12-instruction ptr reconstruction is necessary because Shape > 16B
; (24 bytes = i64 tag + [2 x i64] payload), so it's passed by pointer.
```

**Delta**: 0 instructions. The ptr-to-value reconstruction adds 12 instructions but is required by the calling convention for >16B aggregates.

#### Module Summary

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @check_struct_eq | 12 | 12 | +0 | N/A | OPTIMAL |
| @check_sum_eq | 12 | 12 | +0 | N/A | OPTIMAL |
| @check_nested | 20 | 20 | +0 | N/A | OPTIMAL |
| @main | 16 | 16 | +0 | N/A | OPTIMAL |
| Point$eq | 10 | 10 | +0 | N/A | OPTIMAL |
| Color$eq | 6 | 6 | +0 | N/A | OPTIMAL |
| Shape$eq | 33 | 33 | +0 | N/A | OPTIMAL |

### 8. Derived Traits: Eq Generation

The compiler generates three distinct Eq patterns based on type structure:

**Struct (Point)**: Lexicographic short-circuit field comparison. Fields are compared in declaration order (x, then y). If x differs, y comparison is skipped. The struct is passed by value (16 bytes = 2 registers). This is the textbook derived Eq implementation.

**Unit-variant sum (Color)**: Pure tag comparison. Since Red, Green, Blue have no payload, equality reduces to `tag_self == tag_other`. The `%ori.Color = type { i64 }` representation uses a single i64 for the discriminant. The generated code is minimal -- 6 LLVM IR instructions, 5 x86 instructions.

**Payload sum (Shape)**: Tag-then-switch-then-per-variant field comparison. The algorithm is:
1. Compare tags. If different, return false immediately.
2. If tags match, `switch` on the tag value to dispatch to variant-specific comparison.
3. Circle (1 field): compare `radius` fields.
4. Rect (2 fields): short-circuit compare `w`, then `h`.

Shape uses the `{ i64, [2 x i64] }` tagged union layout with the max payload size across variants. Circle only uses `payload[0]` (radius); Rect uses `payload[0]` (w) and `payload[1]` (h). The `switch` default falls through to `eq.false`, which correctly handles any hypothetical tag corruption.

**Calling convention**: Shape (24 bytes) exceeds the 16-byte by-value threshold and is passed by pointer. The `Shape$eq` function uses GEP+load+insertvalue to reconstruct the value from pointer arguments before comparison. This is correct and necessary.

### 9. Sum Types: Tag Dispatch

The compiler uses a consistent tagged-union representation:

| Type | Layout | Size | Tag Values |
|------|--------|------|------------|
| Color | `{ i64 }` | 8B | Red=0, Green=1, Blue=2 |
| Shape | `{ i64, [2 x i64] }` | 24B | Circle=0, Rect=1 |

**Tag assignment**: Variants are assigned consecutive integer tags starting from 0 in declaration order. This is stable and predictable.

**Payload layout**: The payload array is sized to the maximum variant. Circle has 1 field (radius), Rect has 2 fields (w, h). The union uses `[2 x i64]` to accommodate the largest variant. Circle's unused `payload[1]` is zeroed (`[i64 10, i64 0]`).

**Switch dispatch**: The `switch` instruction in `Shape$eq` dispatches to per-variant comparison blocks with a default that falls to `eq.false`. This is correct and handles potential tag corruption gracefully.

**Equality semantics**: `!=` is correctly implemented as `!(==)` using `xor i1 %eq_result, true`. This ensures consistent semantics between `==` and `!=` derived from the same underlying `$eq` method.

**Constant folding**: The compiler passes struct/enum constants directly in the IR (e.g., `%ori.Point { i64 10, i64 20 }`) rather than constructing them instruction-by-instruction. For `Color`, it uses `zeroinitializer` for `Red` (tag=0) and `{ i64 2 }` for `Blue`. This is clean and efficient.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | LOW | Attributes | Missing `uwtable` on C main wrapper | NEW | J11 |
| 2 | NOTE | Derived Traits | Excellent derived Eq generation -- all three patterns (struct, unit-sum, payload-sum) are OPTIMAL | NEW | J11 |
| 3 | NOTE | Control Flow | Short-circuit field comparison avoids unnecessary work | NEW | J11 |
| 4 | NOTE | Sum Types | Clean tagged-union layout with switch dispatch | NEW | J11 |

### LOW-1: Missing uwtable on C main wrapper

**Location**: `define i32 @main() #3` where `#3 = { nounwind }` (no `uwtable`)
**Impact**: The C entry wrapper lacks `uwtable`, meaning stack unwinding tables may not be generated for it. Impact is minimal since the wrapper is trivial (call + trunc + ret) and never unwinds.
**Fix**: Add `uwtable` to the main wrapper's attribute group.
**First seen**: Journey 11
**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-2: Excellent derived Eq generation

**Location**: `Point$eq`, `Color$eq`, `Shape$eq`
**Impact**: Positive -- all three derived Eq implementations achieve OPTIMAL instruction counts. The compiler correctly generates three distinct patterns: lexicographic short-circuit for structs, tag-only for unit enums, and tag-switch-then-field for payload enums.
**Found in**: Derived Traits: Eq Generation (Category 8)

### NOTE-3: Short-circuit field comparison

**Location**: `Point$eq` entry block branches to `eq.field.1` or `eq.false`
**Impact**: Positive -- if the first field differs, the second field comparison is entirely skipped. This is the correct optimization for derived Eq.
**Found in**: Control Flow & Block Layout (Category 4)

### NOTE-4: Clean tagged-union layout

**Location**: `%ori.Shape = type { i64, [2 x i64] }`
**Impact**: Positive -- the max-payload union layout is standard and efficient. The `switch` dispatch with default-to-false is robust. Unused payload bytes are zeroed.
**Found in**: Sum Types: Tag Dispatch (Category 9)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x -- OPTIMAL |
| ARC Correctness | 20% | 10/10 | 0 violations |
| Attributes & Safety | 10% | 8/10 | 94.7% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions |
| Binary Quality | 10% | 10/10 | 0 defects |
| Other Findings | 15% | 10/10 | No uncategorized findings |

**Overall: 9.8 / 10**

## Verdict

Journey 11's derived trait codegen is outstanding. The compiler generates three distinct and correct Eq implementations: short-circuit field comparison for structs, tag-only comparison for unit enums, and tag-switch-then-per-variant comparison for payload enums. All seven functions achieve OPTIMAL instruction counts with zero unjustified overhead. ARC is irrelevant (all-scalar types, zero RC ops). The only minor gap is a missing `uwtable` on the trivial C main wrapper. The tagged-union representation and switch dispatch for sum types are textbook quality.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|-------------|--------------|--------|
| Overflow checking | J1 | J11 | CONFIRMED |
| fastcc usage | J1 | J11 | CONFIRMED |
| nounwind on user functions | J1 | J11 | FIXED (all have nounwind now) |
| select for if/then/else | J2 | J11 | CONFIRMED |
| Struct by-value passing | J4 | J11 | CONFIRMED |

The `nounwind` attribute that was missing in J1 is now present on all user functions and derived methods. The `select` instruction pattern for branchless if/then/else (first seen in J2) is reused effectively here for conditional value selection based on equality results. Struct by-value passing (J4) is confirmed to work correctly for Point (16B, 2 registers) while Shape (24B) correctly switches to by-pointer passing.
