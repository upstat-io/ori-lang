---
journey: 19
slug: rc-lifecycle
theme: "I am a lifecycle"
date: 2026-07-19
status: FAIL_AOT
expected: 51
eval_result: 51
aot_result: 51
difficulty: complex
prerequisites:
  - "Understanding of list and string reference counting"
  - "Understanding of aggregate field ownership"
  - "Familiarity with borrowed calls and iterator lifetimes"
learning_objectives:
  - "Trace ownership of a list field through aggregate construction and borrowing"
  - "Distinguish an iterator's retained list reference from its caller's original reference"
  - "Recognize an unsound partial aggregate cleanup from final ARC IR"
  - "Correlate ARC instructions with runtime leak-counter evidence"
features:
  - struct_construction
  - field_access
  - nested_structs
  - loops
  - ranges
  - lists
  - strings
  - arc
  - function_calls
  - multiple_functions
  - let_bindings
feature_description: "RC lifecycle through aggregate construction, borrowed list iteration, ownership transfer, field projection, and nested cleanup"
score: 8.5
score_breakdown:
  instruction_efficiency: 10
  arc_correctness: 3
  attributes_safety: 9
  control_flow: 10
  ir_quality: 10
  binary_quality: 10
  other_findings: 10
score_metrics:
  instruction_ratio: 1.00
  instruction_ratio_max: 1.00
  arc_violations: 12
  arc_has_unbalanced: true
  arc_has_scalar_rc: false
  attr_applicable: 35
  attr_correct: 34
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
bugs_found:
  - id: J19-RC-1
    severity: CRITICAL
    description: "Caller-side RcDecPartial skips live Container.items after a borrowed read, leaking four list buffers"
    status: OPEN
    found_in: journey19
related_journeys:
  - journey: 10
    relationship: "Both exercise list creation and iterator-backed traversal; J19 exposes a caller-cleanup leak"
  - journey: 15
    relationship: "Both test iterator/list ownership; J15 previously exposed iterator-versus-container cleanup defects"
  - journey: 16
    relationship: "Both test ownership transfer across calls; J19 adds a borrowed aggregate read"
  - journey: 18
    relationship: "Both combine list iteration with heap-backed values and unwind-aware cleanup"
---

# Journey 19: "I am a lifecycle"

## Source

```ori
// Journey 19: "I am a lifecycle"
// Slug: rc-lifecycle
// Difficulty: complex
// Features: structs, heap_fields, rc, ownership_transfer, nested_structs, field_projection
// Expected: make_and_use(5) + extract_and_use(make_container(3)) + pass_through_sum(4) + nested_containers() = 15 + 6 + 10 + 20 = 51

type Container = { items: [int], name: str }

type Nested = { inner: Container, label: str }

@make_container (n: int) -> Container = {
    let items = for i in 0..n yield i + 1;
    Container { items: items, name: "container" }
}

@extract_and_use (c: Container) -> int = {
    // Project fields from struct with heap fields.
    // After extraction, let the container go out of scope.
    let total = 0;
    for item in c.items do total = total + item;
    total
}

@pass_through (c: Container) -> Container = {
    // Identity — tests ownership transfer through function call.
    c
}

@pass_through_sum (n: int) -> int = {
    let c = make_container(n:);
    let c2 = pass_through(c:);
    extract_and_use(c: c2)
}

@make_and_use (n: int) -> int = {
    let c = make_container(n:);
    extract_and_use(c:)
}

@nested_containers () -> int = {
    // Struct containing another struct with RC fields.
    // Exercises recursive aggregate drop.
    let inner = make_container(n: 3);
    let nested = Nested { inner: inner, label: "outer" };
    // Access through nested struct
    let inner_sum = extract_and_use(c: nested.inner);
    // nested.label is a str (heap) — accessing it exercises str projection
    let label_len = nested.label.length();
    inner_sum + label_len + nested.inner.name.length()
}

@main () -> int = {
    let a = make_and_use(n: 5);
    let b = extract_and_use(c: make_container(n: 3));
    let c = pass_through_sum(n: 4);
    let d = nested_containers();
    a + b + c + d
}
```

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval | 51 | 51 | (none) | (none) | PASS |
| AOT, ordinary execution | 51 | 51 | (none) | compile status only | PASS |
| AOT, canonical leak guard | leak failure | zero live allocations | (none) | four live 64-byte RC allocations | FAIL |

The canonical run `1784485982125582088-df1efd21` reports the failure in `journey_guard::test_journey_19_rc_lifecycle` under `ORI_CHECK_LEAKS=1`. Ordinary exit-code parity therefore does not establish AOT correctness. VM evidence was not captured by the journey harness, so cross-executor parity is incomplete independently of the AOT leak.

## Compiler Pipeline

### 1. Lexer

> The lexer turns source bytes into tokens before parsing.

**Tokens**: 354 | **Keywords**: not emitted | **Identifiers**: not emitted | **Errors**: 0

<details>
<summary>Lexer trace</summary>

```text
source_len=1813 with_metadata=true
lexing complete tokens=354 errors=0
```

</details>

### 2. Parser

> The parser builds the source expression graph and declarations.

**Nodes**: 86 expressions | **Max depth**: not emitted | **Functions**: 7 | **Errors**: 0

<details>
<summary>Parser summary</summary>

```text
parse_module complete functions=7 tests=0 types=2 traits=0 impls=0
imports=0 expressions=86 errors=0 warnings=0
```

</details>

### 3. Type Checker

> The type checker resolves declarations, call signatures, field projections, and method calls.

**Constraints**: not emitted | **Types inferred**: not emitted | **Unifications**: not emitted | **Errors**: 0

<details>
<summary>Type-checker summary</summary>

```text
registration passes complete functions=7 tests=0 impls=0
signature collection complete functions=7 tests=0 impls=0
body checking complete functions=7 tests=0 impls=0
```

</details>

### 4. Canonicalization

> Canonicalization lowers the typed source into the backend-neutral expression form consumed by evaluator and ownership lowering.

**Canon nodes**: 97 | **Roots**: 7 | **Constants**: 6 | **Decision trees**: 0 | **Errors**: 0

<details>
<summary>Canonicalizer summary</summary>

```text
canon lower_module started functions=7 tests=0 impls=0 source_exprs=86
canon lower_module complete canon_nodes=97 roots=7 method_roots=0 constants=6 decision_trees=0
```

</details>

### 5. AIMS / Ownership Projection

> AIMS freezes ownership obligations before the LLVM lane realizes them as physical reference-count operations. A partial decrement's skip set names fields that have transferred out and must not be released with the container.

**Leaked allocations**: 4 | **Bytes per allocation**: 64 | **Leaked bytes**: 256 | **Failing skip set**: `[0]`

<details>
<summary>Ownership evidence</summary>

```text
@extract_and_use:
  %4: [int] = Project %3.0
  RcInc %4 [HeapPtr]
  %5 = Apply @iter(%4 [own])
  ...
  Apply @ori_iter_drop(%5 [own])

@make_and_use normal and unwind cleanup:
  rc_dec_partial %4 skip=[0]

@pass_through_sum normal and unwind cleanup:
  rc_dec_partial %7 skip=[0]

@main normal and unwind cleanup:
  rc_dec_partial %4 skip=[0]

@nested_containers cleanup sites:
  rc_dec_partial %8 skip=[0]
  rc_dec_partial %19 skip=[0]
  rc_dec_partial %1 skip=[0]
```

</details>

The total is reproducible with `python3 .claude/skills/calc/calc.py '4*64' -d 12`. The callee's `RcInc` is consumed by the owned iterator and balanced by `ori_iter_drop`; it does not transfer the caller's original field reference. Each caller still owes a field-0 decrement.

### Backend: Interpreter

> The interpreter executes canonical IR directly and validates value semantics without exercising LLVM's physical RC projection.

**Result**: 51 | **Status**: PASS

<details>
<summary>Evaluation trace summary</summary>

```text
make_and_use(5) = 15
extract_and_use(make_container(3)) = 6
pass_through_sum(4) = 10
nested_containers() = 20
main = 51
```

</details>

### Backend: LLVM Codegen

> LLVM codegen realizes final ARC instructions. It preserves the upstream `skip=[0]` decision exactly, so the physical emitter is the messenger rather than the origin of this leak.

#### ARC Pipeline

**Runtime leak result**: 4 allocations / 256 bytes | **Module balance**: FAIL | **Scalar RC**: none

<details>
<summary>Final ARC IR around the defect</summary>

```text
fn @make_and_use(%0: int [own]) -> int
  %2: Container = Apply @make_container(%1 [own])
  %4: Container = %2
  %5: int = Invoke @extract_and_use(%4 [borrow]) normal bb1 unwind bb2
bb1:
  rc_dec_partial %4 skip=[0]
  Return %5
bb2:
  rc_dec_partial %4 skip=[0]
  Resume
```

</details>

#### Generated LLVM IR

The exact generated definitions are shown below. All seven user functions and the generated `Error` constructor are included; runtime declarations are omitted as permitted by the schema.

```llvm
; ModuleID = '19-rc-lifecycle'
source_filename = "19-rc-lifecycle"

%ori.Container = type { { i64, i64, ptr }, { i64, i64, ptr } }
%ori.Nested = type { %ori.Container, { i64, i64, ptr } }
%ori.Error = type { { i64, i64, ptr }, { i64, i64, ptr } }

@str = private unnamed_addr constant [10 x i8] c"container\00", align 1
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow in addition\00", align 1
@str.1 = private unnamed_addr constant [6 x i8] c"outer\00", align 1

; Function Attrs: uwtable
; --- @make_container ---
define fastcc void @_ori_make_container(ptr noalias sret(%ori.Container) %0, i64 noundef %1) #0 {
bb0:
  %elem_arg = alloca i64, align 8
  %sret.tmp = alloca { i64, i64, ptr }, align 8
  %manual.sret.out = alloca { i64, i64, ptr }, align 8
  %iter.step.scratch = alloca i64, align 8
  %ctor = insertvalue { i64, i64, i64, i64 } { i64 0, i64 undef, i64 undef, i64 undef }, i64 %1, 1
  %ctor1 = insertvalue { i64, i64, i64, i64 } %ctor, i64 1, 2
  %ctor2 = insertvalue { i64, i64, i64, i64 } %ctor1, i64 0, 3
  %range.start = extractvalue { i64, i64, i64, i64 } %ctor2, 0
  %range.end = extractvalue { i64, i64, i64, i64 } %ctor2, 1
  %range.step = extractvalue { i64, i64, i64, i64 } %ctor2, 2
  %range.incl.raw = extractvalue { i64, i64, i64, i64 } %ctor2, 3
  %range.inclusive = trunc i64 %range.incl.raw to i1
  %range.iter = call ptr @ori_iter_from_range(i64 %range.start, i64 %range.end, i64 %range.step, i1 %range.inclusive)
  %call = call ptr @ori_list_new(i64 8, i64 8)
  br label %bb1

bb1:                                              ; preds = %add.ok, %bb0
  %iter.step.has = call i8 @ori_iter_next(ptr %range.iter, ptr %iter.step.scratch, i64 8)
  %iter.step.tag = zext i8 %iter.step.has to i64
  %ne = icmp ne i64 %iter.step.tag, 0
  br i1 %ne, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  %proj.1 = load i64, ptr %iter.step.scratch, align 8
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %proj.1, i64 1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb3:                                              ; preds = %bb1
  call void @ori_iter_drop(ptr %range.iter)
  call void @ori_list_take(ptr %call, ptr %manual.sret.out)
  %manual.sret.val = load { i64, i64, ptr }, ptr %manual.sret.out, align 8
  %list_take.data = extractvalue { i64, i64, ptr } %manual.sret.val, 2
  %list_take.len = extractvalue { i64, i64, ptr } %manual.sret.val, 0
  call void @ori_buffer_store_elem_dec(ptr %list_take.data, ptr null)
  call void @ori_buffer_store_elem_count(ptr %list_take.data, i64 %list_take.len)
  call void @ori_str_from_raw(ptr %sret.tmp, ptr @str, i64 9)
  %sret.load = load { i64, i64, ptr }, ptr %sret.tmp, align 8
  %ctor3 = insertvalue %ori.Container undef, { i64, i64, ptr } %manual.sret.val, 0
  %ctor4 = insertvalue %ori.Container %ctor3, { i64, i64, ptr } %sret.load, 1
  store %ori.Container %ctor4, ptr %0, align 8
  ret void

add.ok:                                           ; preds = %bb2
  store i64 %add.val, ptr %elem_arg, align 8
  call void @ori_list_push(ptr %call, ptr %elem_arg, i64 8)
  %list_builder.len_ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %call, i32 0, i32 0
  %list_builder.data_ptr = getelementptr inbounds nuw { i64, i64, ptr }, ptr %call, i32 0, i32 2
  %list_builder.len = load i64, ptr %list_builder.len_ptr, align 8
  %list_builder.data = load ptr, ptr %list_builder.data_ptr, align 8
  call void @ori_buffer_store_elem_dec(ptr %list_builder.data, ptr null)
  call void @ori_buffer_store_elem_count(ptr %list_builder.data, i64 %list_builder.len)
  br label %bb1

add.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: uwtable
; --- @extract_and_use ---
define fastcc noundef i64 @_ori_extract_and_use(ptr noundef nonnull readonly dereferenceable(48) %0) #0 {
bb0:
  %iter.step.scratch = alloca i64, align 8
  %param.load.f0.ptr = getelementptr inbounds nuw %ori.Container, ptr %0, i32 0, i32 0
  %param.load.f0 = load { i64, i64, ptr }, ptr %param.load.f0.ptr, align 8
  %param.load.s0 = insertvalue %ori.Container zeroinitializer, { i64, i64, ptr } %param.load.f0, 0
  %proj.0 = extractvalue %ori.Container %param.load.s0, 0
  %rc_inc.data = extractvalue { i64, i64, ptr } %proj.0, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %proj.0, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %list.data = extractvalue { i64, i64, ptr } %proj.0, 2
  %list.len = extractvalue { i64, i64, ptr } %proj.0, 0
  %list.cap = extractvalue { i64, i64, ptr } %proj.0, 1
  %list.iter = call ptr @ori_iter_from_list(ptr %list.data, i64 %list.len, i64 %list.cap, i64 8, i1 true)
  br label %bb1

bb1:                                              ; preds = %bb2, %bb0
  %phi1 = phi i64 [ 0, %bb0 ], [ %add.val, %bb2 ]
  %iter.step.has = call i8 @ori_iter_next(ptr %list.iter, ptr %iter.step.scratch, i64 8)
  %iter.step.tag = zext i8 %iter.step.has to i64
  %ne = icmp ne i64 %iter.step.tag, 0
  br i1 %ne, label %bb2, label %bb3

bb2:                                              ; preds = %bb1
  %proj.1 = load i64, ptr %iter.step.scratch, align 8
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %phi1, i64 %proj.1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %bb1

bb3:                                              ; preds = %bb1
  call void @ori_iter_drop(ptr %list.iter)
  ret i64 %phi1

add.ovf_panic:                                    ; preds = %bb2
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @pass_through ---
define fastcc void @_ori_pass_through(ptr noalias sret(%ori.Container) %0, ptr noundef nonnull dereferenceable(48) %1) #1 {
bb0:
  %param.load = load %ori.Container, ptr %1, align 8
  store %ori.Container %param.load, ptr %0, align 8
  ret void
}

; Function Attrs: uwtable
; --- @pass_through_sum ---
define fastcc noundef i64 @_ori_pass_through_sum(i64 noundef %0) #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg3 = alloca %ori.Container, align 8
  %sret.tmp1 = alloca %ori.Container, align 8
  %ref_arg = alloca %ori.Container, align 8
  %sret.tmp = alloca %ori.Container, align 8
  call fastcc void @_ori_make_container(ptr %sret.tmp, i64 %0)
  %sret.load = load %ori.Container, ptr %sret.tmp, align 8
  store %ori.Container %sret.load, ptr %ref_arg, align 8
  call fastcc void @_ori_pass_through(ptr %sret.tmp1, ptr %ref_arg)
  %sret.load2 = load %ori.Container, ptr %sret.tmp1, align 8
  store %ori.Container %sret.load2, ptr %ref_arg3, align 8
  %call = invoke fastcc i64 @_ori_extract_and_use(ptr %ref_arg3)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  %burden.spill4 = alloca %ori.Container, align 8
  store %ori.Container %sret.load2, ptr %burden.spill4, align 8
  %burden_dec_partial.1.ptr5 = getelementptr inbounds nuw %ori.Container, ptr %burden.spill4, i32 0, i32 1
  %burden_dec_partial.16 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr5, align 8
  %rc.data_ptr7 = extractvalue { i64, i64, ptr } %burden_dec_partial.16, 2
  %rc_str.p2i8 = ptrtoint ptr %rc.data_ptr7 to i64
  %rc_str.sso_flag9 = and i64 %rc_str.p2i8, -9223372036854775808
  %rc_str.is_sso10 = icmp ne i64 %rc_str.sso_flag9, 0
  %rc_str.is_null11 = icmp eq i64 %rc_str.p2i8, 0
  %rc_str.skip_rc12 = or i1 %rc_str.is_sso10, %rc_str.is_null11
  %rc.str_safe_ptr13 = select i1 %rc_str.skip_rc12, ptr null, ptr %rc.data_ptr7
  call void @ori_rc_dec(ptr %rc.str_safe_ptr13, ptr @"_ori_drop$3")  ; RC-- str
  ret i64 %call

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  %burden.spill = alloca %ori.Container, align 8
  store %ori.Container %sret.load2, ptr %burden.spill, align 8
  %burden_dec_partial.1.ptr = getelementptr inbounds nuw %ori.Container, ptr %burden.spill, i32 0, i32 1
  %burden_dec_partial.1 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr, align 8
  %rc.data_ptr = extractvalue { i64, i64, ptr } %burden_dec_partial.1, 2
  %rc_str.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.sso_flag = and i64 %rc_str.p2i, -9223372036854775808
  %rc_str.is_sso = icmp ne i64 %rc_str.sso_flag, 0
  %rc_str.is_null = icmp eq i64 %rc_str.p2i, 0
  %rc_str.skip_rc = or i1 %rc_str.is_sso, %rc_str.is_null
  %rc.str_safe_ptr = select i1 %rc_str.skip_rc, ptr null, ptr %rc.data_ptr
  call void @ori_rc_dec(ptr %rc.str_safe_ptr, ptr @"_ori_drop$3")  ; RC-- str
  resume { ptr, i32 } %lp
}

; Function Attrs: uwtable
; --- @make_and_use ---
define fastcc noundef i64 @_ori_make_and_use(i64 noundef %0) #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg = alloca %ori.Container, align 8
  %sret.tmp = alloca %ori.Container, align 8
  call fastcc void @_ori_make_container(ptr %sret.tmp, i64 %0)
  %sret.load = load %ori.Container, ptr %sret.tmp, align 8
  store %ori.Container %sret.load, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_extract_and_use(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  %burden.spill1 = alloca %ori.Container, align 8
  store %ori.Container %sret.load, ptr %burden.spill1, align 8
  %burden_dec_partial.1.ptr2 = getelementptr inbounds nuw %ori.Container, ptr %burden.spill1, i32 0, i32 1
  %burden_dec_partial.13 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr2, align 8
  %rc.data_ptr4 = extractvalue { i64, i64, ptr } %burden_dec_partial.13, 2
  %rc_str.p2i5 = ptrtoint ptr %rc.data_ptr4 to i64
  %rc_str.sso_flag6 = and i64 %rc_str.p2i5, -9223372036854775808
  %rc_str.is_sso7 = icmp ne i64 %rc_str.sso_flag6, 0
  %rc_str.is_null8 = icmp eq i64 %rc_str.p2i5, 0
  %rc_str.skip_rc9 = or i1 %rc_str.is_sso7, %rc_str.is_null8
  %rc.str_safe_ptr10 = select i1 %rc_str.skip_rc9, ptr null, ptr %rc.data_ptr4
  call void @ori_rc_dec(ptr %rc.str_safe_ptr10, ptr @"_ori_drop$3")  ; RC-- str
  ret i64 %call

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  %burden.spill = alloca %ori.Container, align 8
  store %ori.Container %sret.load, ptr %burden.spill, align 8
  %burden_dec_partial.1.ptr = getelementptr inbounds nuw %ori.Container, ptr %burden.spill, i32 0, i32 1
  %burden_dec_partial.1 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr, align 8
  %rc.data_ptr = extractvalue { i64, i64, ptr } %burden_dec_partial.1, 2
  %rc_str.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.sso_flag = and i64 %rc_str.p2i, -9223372036854775808
  %rc_str.is_sso = icmp ne i64 %rc_str.sso_flag, 0
  %rc_str.is_null = icmp eq i64 %rc_str.p2i, 0
  %rc_str.skip_rc = or i1 %rc_str.is_sso, %rc_str.is_null
  %rc.str_safe_ptr = select i1 %rc_str.skip_rc, ptr null, ptr %rc.data_ptr
  call void @ori_rc_dec(ptr %rc.str_safe_ptr, ptr @"_ori_drop$3")  ; RC-- str
  resume { ptr, i32 } %lp
}

; Function Attrs: uwtable
; --- @nested_containers ---
define fastcc noundef i64 @_ori_nested_containers() #0 personality ptr @ori_eh_personality {
bb0:
  %str_len.self37 = alloca { i64, i64, ptr }, align 8
  %str_len.self = alloca { i64, i64, ptr }, align 8
  %ref_arg = alloca %ori.Container, align 8
  %sret.tmp1 = alloca { i64, i64, ptr }, align 8
  %sret.tmp = alloca %ori.Container, align 8
  call fastcc void @_ori_make_container(ptr %sret.tmp, i64 3)
  %sret.load = load %ori.Container, ptr %sret.tmp, align 8
  call void @ori_str_from_raw(ptr %sret.tmp1, ptr @str.1, i64 5)
  %sret.load2 = load { i64, i64, ptr }, ptr %sret.tmp1, align 8
  %rc_inc.f.0 = extractvalue %ori.Container %sret.load, 0
  %rc_inc.data = extractvalue { i64, i64, ptr } %rc_inc.f.0, 2
  %rc_inc.cap = extractvalue { i64, i64, ptr } %rc_inc.f.0, 1
  call void @ori_list_rc_inc(ptr %rc_inc.data, i64 %rc_inc.cap)
  %rc_inc.f.1 = extractvalue %ori.Container %sret.load, 1
  %rc_inc.data3 = extractvalue { i64, i64, ptr } %rc_inc.f.1, 2
  %rc_inc.str_cap = extractvalue { i64, i64, ptr } %rc_inc.f.1, 1
  call void @ori_str_rc_inc(ptr %rc_inc.data3, i64 %rc_inc.str_cap)
  %rc_inc.fat_data = extractvalue { i64, i64, ptr } %sret.load2, 2
  %rc_inc.fat_cap = extractvalue { i64, i64, ptr } %sret.load2, 1
  call void @ori_str_rc_inc(ptr %rc_inc.fat_data, i64 %rc_inc.fat_cap)
  %ctor = insertvalue %ori.Nested undef, %ori.Container %sret.load, 0
  %ctor4 = insertvalue %ori.Nested %ctor, { i64, i64, ptr } %sret.load2, 1
  %proj.0 = extractvalue %ori.Nested %ctor4, 0
  store %ori.Container %proj.0, ptr %ref_arg, align 8
  %call = invoke fastcc i64 @_ori_extract_and_use(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  %proj.1 = extractvalue %ori.Nested %ctor4, 1
  store { i64, i64, ptr } %proj.1, ptr %str_len.self, align 8
  %str.len = call i64 @ori_str_len(ptr %str_len.self)
  %0 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %str.len)
  %1 = extractvalue { i64, i1 } %0, 0
  %2 = extractvalue { i64, i1 } %0, 1
  br i1 %2, label %add.ovf_panic, label %add.ok

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  %burden.spill = alloca %ori.Container, align 8
  store %ori.Container %proj.0, ptr %burden.spill, align 8
  %burden_dec_partial.1.ptr = getelementptr inbounds nuw %ori.Container, ptr %burden.spill, i32 0, i32 1
  %burden_dec_partial.1 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr, align 8
  %rc.data_ptr = extractvalue { i64, i64, ptr } %burden_dec_partial.1, 2
  %rc_str.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.sso_flag = and i64 %rc_str.p2i, -9223372036854775808
  %rc_str.is_sso = icmp ne i64 %rc_str.sso_flag, 0
  %rc_str.is_null = icmp eq i64 %rc_str.p2i, 0
  %rc_str.skip_rc = or i1 %rc_str.is_sso, %rc_str.is_null
  %rc.str_safe_ptr = select i1 %rc_str.skip_rc, ptr null, ptr %rc.data_ptr
  call void @ori_rc_dec(ptr %rc.str_safe_ptr, ptr @"_ori_drop$3")  ; RC-- str
  %rc_dec.fat_data = extractvalue { i64, i64, ptr } %sret.load2, 2
  %rc_dec.fat_cap = extractvalue { i64, i64, ptr } %sret.load2, 1
  call void @ori_str_rc_dec(ptr %rc_dec.fat_data, i64 %rc_dec.fat_cap, ptr @"_ori_drop$3")
  %rc_dec.f.1 = extractvalue %ori.Nested %ctor4, 1
  %rc_dec.data = extractvalue { i64, i64, ptr } %rc_dec.f.1, 2
  %rc_dec.str_cap = extractvalue { i64, i64, ptr } %rc_dec.f.1, 1
  call void @ori_str_rc_dec(ptr %rc_dec.data, i64 %rc_dec.str_cap, ptr @"_ori_drop$3")
  %rc_dec.f.0 = extractvalue %ori.Nested %ctor4, 0
  %rc_dec.f.15 = extractvalue %ori.Container %rc_dec.f.0, 1
  %rc_dec.data6 = extractvalue { i64, i64, ptr } %rc_dec.f.15, 2
  %rc_dec.str_cap7 = extractvalue { i64, i64, ptr } %rc_dec.f.15, 1
  call void @ori_str_rc_dec(ptr %rc_dec.data6, i64 %rc_dec.str_cap7, ptr @"_ori_drop$3")
  %rc_dec.f.08 = extractvalue %ori.Container %rc_dec.f.0, 0
  %rc.data_ptr9 = extractvalue { i64, i64, ptr } %rc_dec.f.08, 2
  %rc.len = extractvalue { i64, i64, ptr } %rc_dec.f.08, 0
  %rc.cap = extractvalue { i64, i64, ptr } %rc_dec.f.08, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr9, i64 %rc.len, i64 %rc.cap, i64 8, ptr null)
  resume { ptr, i32 } %lp

add.ok:                                           ; preds = %bb1
  %proj.010 = extractvalue %ori.Nested %ctor4, 0
  %proj.111 = extractvalue %ori.Container %proj.010, 1
  %burden.spill12 = alloca %ori.Container, align 8
  store %ori.Container %proj.010, ptr %burden.spill12, align 8
  %burden_dec_partial.1.ptr13 = getelementptr inbounds nuw %ori.Container, ptr %burden.spill12, i32 0, i32 1
  %burden_dec_partial.114 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr13, align 8
  %rc.data_ptr15 = extractvalue { i64, i64, ptr } %burden_dec_partial.114, 2
  %rc_str.p2i16 = ptrtoint ptr %rc.data_ptr15 to i64
  %rc_str.sso_flag17 = and i64 %rc_str.p2i16, -9223372036854775808
  %rc_str.is_sso18 = icmp ne i64 %rc_str.sso_flag17, 0
  %rc_str.is_null19 = icmp eq i64 %rc_str.p2i16, 0
  %rc_str.skip_rc20 = or i1 %rc_str.is_sso18, %rc_str.is_null19
  %rc.str_safe_ptr21 = select i1 %rc_str.skip_rc20, ptr null, ptr %rc.data_ptr15
  call void @ori_rc_dec(ptr %rc.str_safe_ptr21, ptr @"_ori_drop$3")  ; RC-- str
  %rc_inc.fat_data22 = extractvalue { i64, i64, ptr } %proj.111, 2
  %rc_inc.fat_cap23 = extractvalue { i64, i64, ptr } %proj.111, 1
  call void @ori_str_rc_inc(ptr %rc_inc.fat_data22, i64 %rc_inc.fat_cap23)
  %rc_dec.fat_data24 = extractvalue { i64, i64, ptr } %proj.1, 2
  %rc_dec.fat_cap25 = extractvalue { i64, i64, ptr } %proj.1, 1
  call void @ori_str_rc_dec(ptr %rc_dec.fat_data24, i64 %rc_dec.fat_cap25, ptr @"_ori_drop$3")
  %rc_dec.f.126 = extractvalue %ori.Nested %ctor4, 1
  %rc_dec.data27 = extractvalue { i64, i64, ptr } %rc_dec.f.126, 2
  %rc_dec.str_cap28 = extractvalue { i64, i64, ptr } %rc_dec.f.126, 1
  call void @ori_str_rc_dec(ptr %rc_dec.data27, i64 %rc_dec.str_cap28, ptr @"_ori_drop$3")
  %rc_dec.f.029 = extractvalue %ori.Nested %ctor4, 0
  %rc_dec.f.130 = extractvalue %ori.Container %rc_dec.f.029, 1
  %rc_dec.data31 = extractvalue { i64, i64, ptr } %rc_dec.f.130, 2
  %rc_dec.str_cap32 = extractvalue { i64, i64, ptr } %rc_dec.f.130, 1
  call void @ori_str_rc_dec(ptr %rc_dec.data31, i64 %rc_dec.str_cap32, ptr @"_ori_drop$3")
  %rc_dec.f.033 = extractvalue %ori.Container %rc_dec.f.029, 0
  %rc.data_ptr34 = extractvalue { i64, i64, ptr } %rc_dec.f.033, 2
  %rc.len35 = extractvalue { i64, i64, ptr } %rc_dec.f.033, 0
  %rc.cap36 = extractvalue { i64, i64, ptr } %rc_dec.f.033, 1
  call void @ori_buffer_rc_dec(ptr %rc.data_ptr34, i64 %rc.len35, i64 %rc.cap36, i64 8, ptr null)
  store { i64, i64, ptr } %proj.111, ptr %str_len.self37, align 8
  %str.len38 = call i64 @ori_str_len(ptr %str_len.self37)
  %3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %1, i64 %str.len38)
  %4 = extractvalue { i64, i1 } %3, 0
  %5 = extractvalue { i64, i1 } %3, 1
  br i1 %5, label %add.ovf_panic43, label %add.ok42

add.ovf_panic:                                    ; preds = %bb1
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok42:                                         ; preds = %add.ok
  %rc_dec.fat_data44 = extractvalue { i64, i64, ptr } %proj.111, 2
  %rc_dec.fat_cap45 = extractvalue { i64, i64, ptr } %proj.111, 1
  call void @ori_str_rc_dec(ptr %rc_dec.fat_data44, i64 %rc_dec.fat_cap45, ptr @"_ori_drop$3")
  ret i64 %4

add.ovf_panic43:                                  ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: uwtable
; --- @main ---
define noundef i64 @_ori_main() #0 personality ptr @ori_eh_personality {
bb0:
  %ref_arg = alloca %ori.Container, align 8
  %sret.tmp = alloca %ori.Container, align 8
  %call = call fastcc i64 @_ori_make_and_use(i64 5)
  call fastcc void @_ori_make_container(ptr %sret.tmp, i64 3)
  %sret.load = load %ori.Container, ptr %sret.tmp, align 8
  store %ori.Container %sret.load, ptr %ref_arg, align 8
  %call1 = invoke fastcc i64 @_ori_extract_and_use(ptr %ref_arg)
          to label %bb1 unwind label %bb2

bb1:                                              ; preds = %bb0
  %burden.spill2 = alloca %ori.Container, align 8
  store %ori.Container %sret.load, ptr %burden.spill2, align 8
  %burden_dec_partial.1.ptr3 = getelementptr inbounds nuw %ori.Container, ptr %burden.spill2, i32 0, i32 1
  %burden_dec_partial.14 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr3, align 8
  %rc.data_ptr5 = extractvalue { i64, i64, ptr } %burden_dec_partial.14, 2
  %rc_str.p2i6 = ptrtoint ptr %rc.data_ptr5 to i64
  %rc_str.sso_flag7 = and i64 %rc_str.p2i6, -9223372036854775808
  %rc_str.is_sso8 = icmp ne i64 %rc_str.sso_flag7, 0
  %rc_str.is_null9 = icmp eq i64 %rc_str.p2i6, 0
  %rc_str.skip_rc10 = or i1 %rc_str.is_sso8, %rc_str.is_null9
  %rc.str_safe_ptr11 = select i1 %rc_str.skip_rc10, ptr null, ptr %rc.data_ptr5
  call void @ori_rc_dec(ptr %rc.str_safe_ptr11, ptr @"_ori_drop$3")  ; RC-- str
  %call12 = call fastcc i64 @_ori_pass_through_sum(i64 4)
  %call13 = call fastcc i64 @_ori_nested_containers()
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

bb2:                                              ; preds = %bb0
  %lp = landingpad { ptr, i32 }
          cleanup
  %burden.spill = alloca %ori.Container, align 8
  store %ori.Container %sret.load, ptr %burden.spill, align 8
  %burden_dec_partial.1.ptr = getelementptr inbounds nuw %ori.Container, ptr %burden.spill, i32 0, i32 1
  %burden_dec_partial.1 = load { i64, i64, ptr }, ptr %burden_dec_partial.1.ptr, align 8
  %rc.data_ptr = extractvalue { i64, i64, ptr } %burden_dec_partial.1, 2
  %rc_str.p2i = ptrtoint ptr %rc.data_ptr to i64
  %rc_str.sso_flag = and i64 %rc_str.p2i, -9223372036854775808
  %rc_str.is_sso = icmp ne i64 %rc_str.sso_flag, 0
  %rc_str.is_null = icmp eq i64 %rc_str.p2i, 0
  %rc_str.skip_rc = or i1 %rc_str.is_sso, %rc_str.is_null
  %rc.str_safe_ptr = select i1 %rc_str.skip_rc, ptr null, ptr %rc.data_ptr
  call void @ori_rc_dec(ptr %rc.str_safe_ptr, ptr @"_ori_drop$3")  ; RC-- str
  resume { ptr, i32 } %lp

add.ok:                                           ; preds = %bb1
  %add14 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call12)
  %add.val15 = extractvalue { i64, i1 } %add14, 0
  %add.ovf16 = extractvalue { i64, i1 } %add14, 1
  br i1 %add.ovf16, label %add.ovf_panic18, label %add.ok17

add.ovf_panic:                                    ; preds = %bb1
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok17:                                         ; preds = %add.ok
  %add19 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val15, i64 %call13)
  %add.val20 = extractvalue { i64, i1 } %add19, 0
  %add.ovf21 = extractvalue { i64, i1 } %add19, 1
  br i1 %add.ovf21, label %add.ovf_panic23, label %add.ok22

add.ovf_panic18:                                  ; preds = %add.ok
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable

add.ok22:                                         ; preds = %add.ok17
  ret i64 %add.val20

add.ovf_panic23:                                  ; preds = %add.ok17
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

; Function Attrs: nounwind uwtable
; --- @Error_ctor ---
define internal void @_ori_Error_ctor(ptr noalias sret(%ori.Error) %0, ptr noundef %1, ptr noundef nonnull dereferenceable(24) %2) #1 {
entry:
  %dst_msg_ptr = getelementptr inbounds nuw %ori.Error, ptr %0, i32 0, i32 0
  %src = getelementptr inbounds nuw { i64, i64, ptr }, ptr %2, i32 0, i32 0
  %fld = load i64, ptr %src, align 8
  %dst = getelementptr inbounds nuw { i64, i64, ptr }, ptr %dst_msg_ptr, i32 0, i32 0
  store i64 %fld, ptr %dst, align 8
  %src1 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %2, i32 0, i32 1
  %fld2 = load i64, ptr %src1, align 8
  %dst3 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %dst_msg_ptr, i32 0, i32 1
  store i64 %fld2, ptr %dst3, align 8
  %src4 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %2, i32 0, i32 2
  %fld5 = load ptr, ptr %src4, align 8
  %dst6 = getelementptr inbounds nuw { i64, i64, ptr }, ptr %dst_msg_ptr, i32 0, i32 2
  store ptr %fld5, ptr %dst6, align 8
  %dst_trace_ptr = getelementptr inbounds nuw %ori.Error, ptr %0, i32 0, i32 1
  store { i64, i64, ptr } zeroinitializer, ptr %dst_trace_ptr, align 8
  ret void
}
```
#### Disassembly

```asm
_ori_extract_and_use:
  call ori_list_rc_inc
  call ori_iter_from_list
  ; iteration
  call ori_iter_drop
  ret

_ori_make_and_use:
  call _ori_make_container
  call _ori_extract_and_use
  ; string-only cleanup emitted from RcDecPartial skip=[0]
  call ori_rc_dec
  ret

main:
  call _ori_main
  call ori_check_leaks
  cmovne %ecx, %eax
  ret
```

## Deep Scrutiny

### 1. Instruction Purity

| # | Function | Actual | Ideal | Ratio | Verdict |
|---|----------|--------|-------|-------|---------|
| 1 | @make_container | 48 | 48 | 1.00x | OPTIMAL by extractor |
| 2 | @extract_and_use | 27 | 27 | 1.00x | OPTIMAL by extractor |
| 3 | @pass_through | 3 | 3 | 1.00x | OPTIMAL |
| 4 | @pass_through_sum | 40 | 40 | 1.00x | INVALID: missing release |
| 5 | @make_and_use | 35 | 35 | 1.00x | INVALID: missing release |
| 6 | @nested_containers | 113 | 113 | 1.00x | INVALID: missing release |
| 7 | @main | 56 | 56 | 1.00x | INVALID: missing release |

The deterministic extractor reports no unjustified instructions, but instruction purity cannot certify omitted mandatory cleanup. The four affected functions are compact because a necessary field-0 release is absent, not because their ownership code is optimal. [CRITICAL-1]

### 2. ARC Purity

| Function | rc_inc | rc_dec | Balanced | Borrow Elision | Move Semantics |
|----------|--------|--------|----------|----------------|----------------|
| @make_container | 2 | 1 | transfer out | N/A | returns Container ownership |
| @extract_and_use | 2 | 2 | YES locally | aggregate parameter borrowed | iterator owns only the added retain |
| @pass_through | 0 | 0 | YES | N/A | zero-RC aggregate transfer |
| @pass_through_sum | 0 | 1 | NO | borrowed read | field 0 skipped |
| @make_and_use | 0 | 1 | NO | borrowed read | field 0 skipped |
| @nested_containers | 2 | 2 | NO globally | borrowed read | original field 0 survives |
| @main | 0 | 1 | NO | borrowed read | field 0 skipped |

**Verdict**: FAIL. Four `make_container` executions leave one list-buffer reference live apiece. The leak guard reports four 64-byte allocations, and the ARC IR explains all four without an unmatched string allocation. [CRITICAL-1]

The weighted violation count is 12 because the scoring rubric multiplies each of four unbalanced references by three; reproduce it with `python3 .claude/skills/calc/calc.py '4*3' -d 12`.

### 3. Attributes & Calling Convention

| Function | fastcc | nounwind | noalias | readonly | cold | Notes |
|----------|--------|----------|---------|----------|------|-------|
| @make_container | YES | NO | YES (sret) | NO | NO | may unwind through iterator calls |
| @extract_and_use | YES | NO | N/A | YES (param) | NO | may unwind through iterator-next |
| @pass_through | YES | YES | YES (sret) | N/A | NO | correct |
| @pass_through_sum | YES | NO | N/A | NO | NO | cleanup landing pad present |
| @make_and_use | YES | NO | N/A | NO | NO | cleanup landing pad present |
| @nested_containers | YES | NO | N/A | NO | NO | cleanup landing pads present |
| @_ori_main | entry ABI | NO | N/A | NO | NO | called by C entry wrapper |

The extractor reports 34 of 35 applicable checks. The remaining gap is the internal compiler-generated `_ori_Error_ctor`, which uses the C convention instead of `fastcc`. [HIGH-2]

### 4. Control Flow & Block Layout

| Function | Blocks | Empty Blocks | Redundant Branches | Phi Nodes | Notes |
|----------|--------|--------------|--------------------|-----------|-------|
| @make_container | 6 | 0 | 0 | 0 | range loop and overflow edge |
| @extract_and_use | 5 | 0 | 0 | 1 | loop accumulator |
| @pass_through | 1 | 0 | 0 | 0 | straight-line transfer |
| @pass_through_sum | 3 | 0 | 0 | 0 | normal/unwind cleanup |
| @make_and_use | 3 | 0 | 0 | 0 | normal/unwind cleanup |
| @nested_containers | 7 | 0 | 0 | 0 | two invokes and overflow edges |
| @main | 9 | 0 | 0 | 0 | unwind and checked additions |

No structural control-flow defect was detected. Both normal and unwind edges consistently carry the same incorrect field-0 skip.

### 5. Overflow Checking

**Status**: PASS

| Operation | Checked | Correct | Notes |
|-----------|---------|---------|-------|
| range element `i + 1` | YES | YES | `llvm.sadd.with.overflow.i64` |
| list accumulation | YES | YES | overflow edge calls panic |
| nested result additions | YES | YES | both additions checked |
| final result additions | YES | YES | all three additions checked |

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size | 7,264,296 bytes (6.92777252197 MiB) |
| `.text` section | 1,026,380 bytes |
| `.rodata` section | 143,454 bytes |
| Seven scenario functions | 3,373 bytes |
| Non-scenario share of `.text` | 99.6713692784% |
| Leak | 4 allocations, 256 bytes |

Binary conversions and the function-size sum are reproducible with:

```text
python3 .claude/skills/calc/calc.py '7264296/1048576' -d 12
python3 .claude/skills/calc/calc.py '394+174+53+544+420+1174+614' -d 12
python3 .claude/skills/calc/calc.py '100*(1026380-(394+174+53+544+420+1174+614))/1026380' -d 12
```

#### Disassembly: @make_and_use

```asm
call _ori_make_container
call _ori_extract_and_use
call ori_rc_dec            ; name only
ret                        ; no ori_buffer_rc_dec for items
```

### 7. Optimal IR Comparison

#### @make_and_use: Required vs Actual Cleanup

```llvm
; REQUIRED after the borrowed call
%items = extractvalue %ori.Container %c, 0
%data = extractvalue { i64, i64, ptr } %items, 2
%len = extractvalue { i64, i64, ptr } %items, 0
%cap = extractvalue { i64, i64, ptr } %items, 1
call void @ori_buffer_rc_dec(ptr %data, i64 %len, i64 %cap, i64 8, ptr null)
; release field 1 as generated today
```

```llvm
; ACTUAL RcDecPartial(skip=[0]) cleanup
%name.ptr = getelementptr inbounds %ori.Container, ptr %c, i32 0, i32 1
%name = load { i64, i64, ptr }, ptr %name.ptr, align 8
call void @ori_rc_dec(ptr %safe_name_data, ptr @_ori_drop_str)
; field 0 is omitted
```

| Function | Ideal | Actual | Delta | Justified | Verdict |
|----------|-------|--------|-------|-----------|---------|
| @make_container | complete transfer | complete transfer | none | N/A | correct |
| @extract_and_use | retain iterator input, drop iterator | same | none | YES | correct locally |
| @pass_through | zero-RC move | same | none | YES | correct |
| @pass_through_sum | release both Container fields | field 0 omitted | missing release | NO | INCORRECT |
| @make_and_use | release both Container fields | field 0 omitted | missing release | NO | INCORRECT |
| @nested_containers | balance original plus retained copies | original field 0 survives | missing release | NO | INCORRECT |
| @main | release both temporary fields | field 0 omitted | missing release | NO | INCORRECT |

### 8. Lists: Borrowed Iteration Ownership

`extract_and_use` borrows the `Container`, explicitly retains `items`, and transfers that retained reference into the iterator. `ori_iter_drop` consumes exactly that retained reference. No ownership event moves the caller's original `Container.items` field into the iterator, so a caller-side partial decrement has no authority to skip field 0.

### 9. Aggregates: Partial Cleanup Authority

The final `RcDecPartial(skip=[0])` is already present before LLVM emission. `ori_llvm::codegen::arc_emitter::instr_dispatch::emit_burden_dec_partial` correctly walks every owned field except the named skip. The likely origin is the class-ledger field-decomposition path incorrectly interpreting a borrow-only field view as a move-out when deriving the positional skip.

### 10. Tooling: Trace Completeness

The supplied `arc_trace.txt`, `llvm_ir.txt`, and `llvm_warn.txt` were empty even though both ordinary executions succeeded. Current final ARC and LLVM dumps were regenerated from the same compiler with `ORI_DUMP_AFTER_ARC=1 ori build` and `ORI_DUMP_AFTER_LLVM=1 ori build`. The deterministic extractor does not model `RcDecPartial` field omission against runtime leak evidence; it reported `arc_has_unbalanced: false`, so the runtime-proven four leaks were supplied to `score.py` as four unbalanced references under its documented multiplier.

## Findings

| # | Severity | Category | Description | Status | First Seen |
|---|----------|----------|-------------|--------|------------|
| 1 | CRITICAL | ARC | Borrowed field read produces `RcDecPartial(skip=[0])`, leaking four list buffers | REGRESSED | J19 |
| 2 | HIGH | Attributes | Internal generated `_ori_Error_ctor` lacks `fastcc` | NEW | J19 |
| 3 | NOTE | ARC | Callee iterator retain and drop are locally balanced | NEW | J19 |

### CRITICAL-1: Caller partial decrement skips an owned list field

**Location**: final ARC IR for `@main`, `@make_and_use`, `@pass_through_sum`, and `@nested_containers`

**Impact**: Four live 64-byte allocations remain after execution. The canonical leak guard fails despite correct exit-code output.

**Root cause hypothesis**: `ori_arc::aims::class_ledger` assigns positional skip authority to field 0 even though the only field use is borrowed iteration. Inspect `hazard/skip_derive.rs::view_projections_all_move_out`, `derive_constructless_positional_skip`, `hazard/decompose.rs`, and the interprocedural alias/ownership evidence feeding the hazard. The LLVM consumer `ori_llvm/src/codegen/arc_emitter/instr_dispatch.rs::emit_burden_dec_partial` is behaving consistently with the incorrect upstream instruction.

**Fix**: Preserve a whole-container decrement when a projected field is only borrowed, or otherwise prove a real transfer before adding the field ordinal to a partial-decrement skip set. Pin normal and unwind paths and require `ORI_CHECK_LEAKS=1` to report zero.

**First seen**: Journey 19 reanalysis

**Found in**: ARC Purity (Category 2), Optimal IR Comparison (Category 7)

### HIGH-2: Internal generated constructor lacks fastcc

**Location**: `_ori_Error_ctor`

**Impact**: One of 35 deterministic attribute checks fails for an internal generated function.

**Fix**: Apply the internal Ori calling convention to generated constructors that are not ABI entry points or function-pointer bridges.

**First seen**: Journey 19 reanalysis

**Found in**: Attributes & Calling Convention (Category 3)

### NOTE-3: Iterator ownership is locally balanced

**Location**: `@extract_and_use`

**Impact**: Positive diagnostic localization. The callee retain plus `ori_iter_drop` explains why the defect is caller cleanup rather than iterator teardown.

**Found in**: ARC Purity (Category 2), Lists: Borrowed Iteration Ownership (Category 8)

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Instruction Efficiency | 15% | 10/10 | 1.00x — OPTIMAL by static extractor |
| ARC Correctness | 20% | 3/10 | 12 weighted violations |
| Attributes & Safety | 10% | 9/10 | 97.1% compliance |
| Control Flow | 10% | 10/10 | 0 defects |
| IR Quality | 20% | 10/10 | 0 unjustified instructions detected |
| Binary Quality | 10% | 10/10 | correct ordinary exit codes |
| Other Findings | 15% | 10/10 | no uncategorized findings |

**Overall: 8.5 / 10**

Gates applied:

- `arc_unbalanced_gate`: unbalanced RC pair, ARC Correctness capped at 3

## Verdict

Journey 19 computes 51 in both ordinary executors but fails AOT memory correctness: four list buffers survive because caller cleanup skips `Container.items` after a borrow-only read. The final ARC IR localizes the defect upstream of LLVM emission to unsound field-decomposition authority, while the callee's iterator retain and drop remain balanced.

## Cross-Journey Observations

| Feature | First Tested | This Journey | Status |
|---------|--------------|--------------|--------|
| Struct field cleanup | J4 | J19 | REGRESSED for heap field 0 |
| List iterator lifecycle | J10 | J19 | callee-balanced, caller cleanup broken |
| Nested list cleanup | J15 | J19 | recurring ownership-boundary risk |
| Ownership transfer | J16 | J19 | identity transfer remains zero-RC |

The previous J19 result claimed a perfect score and zero leaks. Current canonical leak evidence supersedes that conclusion: value parity is intact, but ownership parity is not.
