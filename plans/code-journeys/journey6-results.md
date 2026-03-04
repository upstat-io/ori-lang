# Journey 6 Results: "I am a match"

**Date**: 2026-03-03
**Status**: PASS -- both eval and AOT produce correct result (exit code 41)

## Source

```ori
// Journey 6: "I am a match"
// Features: sum types, pattern matching, match expressions
// Expected: extract(Success(42)) + extract(Failure(-1)) + to_code(Pending) = 42 + (-1) + 0 = 41
type Status = Pending | Running | Completed;
type Result2 = Success(value: int) | Failure(code: int);
@to_code (s: Status) -> int = match s { Pending -> 0, Running -> 1, Completed -> 2 }
@extract (r: Result2) -> int = match r { Success(v) -> v, Failure(c) -> c }
@main () -> int = {
    let a = extract(r: Success(value: 42));
    let b = extract(r: Failure(code: -1));
    let c = to_code(s: Pending);
    a + b + c
}
```

**Features exercised**: sum types (unit variants and record variants), pattern matching with `match`, variant construction with named fields, variant destructuring, multiple function calls, named arguments, let bindings, integer addition with negative literals.

## Execution Results

| Backend | Exit Code | Expected | Stdout | Stderr | Status |
|---------|-----------|----------|--------|--------|--------|
| Eval    | 41        | 41       | (none) | (none) | PASS   |
| AOT     | 41        | 41       | (none) | (none) | PASS   |

## Pipeline Trace Summary

### Lexer
- Source: 720 bytes, 160 tokens, 0 errors
- Prelude: 10331 bytes, 1516 tokens, 0 errors
- Clean pass, no issues.

### Parser
- 3 functions (`to_code`, `extract`, `main`), 2 type definitions (`Status`, `Result2`), 28 expressions, 0 errors, 0 warnings
- Correctly parsed `match` expressions with variant patterns
- Parse contexts: 3x "function definition", 2x "match expression", 1x "expression" (block body), 4x "function call"
- Primary nodes include integers (0, 1, 2, 42, -1) and identifiers (variant names, function names, binding names)
- Prelude: 9 functions, 39 traits, 46 expressions, 0 errors

### Type Checker
- Prelude: registration, signatures, body checking -- all complete for 9 functions, 0 tests, 0 impls
- User module: registration, signatures, body checking -- all complete for 3 functions, 0 tests, 0 impls
- Import resolution: hash-first miss (AST fallback) for 6 generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`); hash-first hit for `compare`, `min`, `max`
- No type errors. Sum types `Status` and `Result2` correctly registered and pattern matching verified exhaustive.

### Canonicalization
- User module: 31 canon nodes, 3 roots, 0 method roots, 6 constants, 2 decision trees
- Prelude: 46 canon nodes, 9 roots, 6 constants, 4 decision trees
- The 2 decision trees correspond to the 2 `match` expressions (`to_code` and `extract`).
- The 3 roots are `to_code`, `extract`, and `main`.

### ARC Pipeline
- Type registration: 8 user types registered:
  - `Status` (Idx 202): enum with 3 unit variants (Pending, Running, Completed)
  - `Result2` (Idx 204): enum with 2 record variants (Success(value: int), Failure(code: int))
  - Plus prelude types: Ordering, PanicInfo, TraceEntry, FormatType, Sign, Alignment
- 3 user functions declared:
  - `to_code`: 1 param, FastCC, direct return
  - `extract`: 1 param, FastCC, direct return
  - `main`: 0 params, C ABI, direct return
- Nounwind analysis: 2 fixed-point passes, all 3 functions marked nounwind, 0 mono-propagated
- Entry point wrapper: `main()` -> `_ori_main()` with `returns_int=true`, `has_args=false`, `has_panic=false`

### Evaluator
- Traced full execution:
  - `extract(Success(value: 42))`: match on `r`, decision tree routes to Success arm, binds `v = 42`, returns 42
  - `extract(Failure(code: -1))`: match on `r`, decision tree routes to Failure arm, binds `c = -1`, returns -1
  - `to_code(Pending)`: match on `s`, decision tree routes to Pending arm, returns 0
  - `Add(42, -1)` = 41, `Add(41, 0)` = 41
- All operations correct, no errors.

## LLVM Deep Scrutiny (9 Categories)

### 1. Type Representations

```llvm
%ori.Status = type { i64 }
%ori.Result2 = type { i64, [1 x i64] }
```

**`Status`**: A pure enum (all unit variants). Represented as a single i64 discriminant. Variants: Pending=0, Running=1, Completed=2. This is optimal -- a single machine word holds the tag.

**`Result2`**: A sum type with record payloads. Represented as `{ i64, [1 x i64] }` -- a discriminant (i64) followed by a payload array of 1 element. Both variants (Success and Failure) carry a single `int` field, so the payload is `[1 x i64]`. Variants: Success=0 (value in payload[0]), Failure=1 (code in payload[0]).

The `[1 x i64]` array type for the payload is a fixed-size union representing the maximum payload across all variants. Since both Success and Failure carry exactly one `int`, this is the correct minimum.

**Verdict**: Type layouts are correct and compact. No wasted padding.

**Severity**: None.

### 2. Attributes & Calling Convention

| Function | fastcc | nounwind | Status |
|----------|--------|----------|--------|
| `_ori_to_code` | Yes | Yes | OK |
| `_ori_extract` | Yes | Yes | OK |
| `_ori_main` | No (C ABI) | Yes | OK |
| `main` (wrapper) | No (C ABI) | No | OK (known L-2) |
| `ori_panic_cstr` | No | No | OK -- `cold` present |

**Verdict**: All correct. Internal functions use `fastcc`, entry point uses C ABI. All user functions marked `nounwind`. Consistent with J1/J2 patterns.

**Severity**: None (existing L-2 about wrapper `nounwind` still applies).

### 3. Match Codegen: `@to_code` (Unit Variant Match)

```llvm
define fastcc i64 @_ori_to_code(%ori.Status %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Status %0, 0
  %eq = icmp eq i64 %proj.0, 0
  %sel = select i1 %eq, i64 0, i64 2
  %eq1 = icmp eq i64 %proj.0, 1
  %sel2 = select i1 %eq1, i64 1, i64 %sel
  br label %bb1

bb1:
  %v2 = phi i64 [ %sel2, %bb0 ]
  ret i64 %v2
}
```

**Analysis**: The codegen compiles the 3-arm unit-variant match into a chain of `select` instructions with a default-last strategy:
1. Extract the discriminant from the Status struct
2. Check if Pending (0) -> select 0, else default to 2 (Completed)
3. Check if Running (1) -> select 1, else keep previous result

This is a decision tree lowered to a linear scan with `select`. The approach is:
- Start with the last arm's value (2 for Completed) as the default
- Check middle arm (Running=1): if match, override to 1
- Check first arm (Pending=0): if match, override to 0

Wait -- the actual ordering is reversed: the default starts at 2 (Completed), then checks discriminant 0 (Pending) to select 0, then checks discriminant 1 (Running) to select 1. The final `%sel2` holds the correct value for all 3 cases.

**Correctness verification**:
- Pending (tag=0): `%eq` true -> `%sel=0`; `%eq1` false -> `%sel2=0`. Correct.
- Running (tag=1): `%eq` false -> `%sel=2`; `%eq1` true -> `%sel2=1`. Correct.
- Completed (tag=2): `%eq` false -> `%sel=2`; `%eq1` false -> `%sel2=2`. Correct.

**Quality**: This is good codegen. Using `select` instead of a branch diamond for each arm avoids control flow complexity. The ideal for a 3-way switch would be a `switch` instruction or a jump table, but for 3 small-constant-result arms, the `select` chain is arguably better -- it avoids branch misprediction entirely and runs as straight-line code.

**Issue (LOW)**: The `br label %bb1` + single-predecessor `phi` at the end is a redundant block boundary. The `phi` has only one incoming edge and could be eliminated, with `ret i64 %sel2` directly in `bb0`. This is the same M-1 pattern from J1.

**Severity**: LOW (existing M-1).

### 4. Match Codegen: `@extract` (Record Variant Match)

```llvm
define fastcc i64 @_ori_extract(%ori.Result2 %0) #0 {
bb0:
  %proj.0 = extractvalue %ori.Result2 %0, 0
  switch i64 %proj.0, label %bb4 [
    i64 0, label %bb2
    i64 1, label %bb3
  ]

bb1:                              ; merge block
  %v2 = phi i64 [ %proj.1, %bb3 ], [ %proj.14, %bb2 ]
  ret i64 %v2

bb2:                              ; Success arm
  %proj.alloca1 = alloca %ori.Result2, align 8
  store %ori.Result2 %0, ptr %proj.alloca1, align 8
  %proj.payload2 = getelementptr inbounds nuw %ori.Result2, ptr %proj.alloca1, i32 0, i32 1
  %proj.1.gep3 = getelementptr inbounds i64, ptr %proj.payload2, i64 0
  %proj.14 = load i64, ptr %proj.1.gep3, align 8
  br label %bb1

bb3:                              ; Failure arm
  %proj.alloca = alloca %ori.Result2, align 8
  store %ori.Result2 %0, ptr %proj.alloca, align 8
  %proj.payload = getelementptr inbounds nuw %ori.Result2, ptr %proj.alloca, i32 0, i32 1
  %proj.1.gep = getelementptr inbounds i64, ptr %proj.payload, i64 0
  %proj.1 = load i64, ptr %proj.1.gep, align 8
  br label %bb1

bb4:                              ; unreachable default
  unreachable
}
```

**Analysis**: The codegen uses a `switch` instruction to dispatch on the discriminant, with separate blocks for each variant. Each arm:
1. Allocates the full `%ori.Result2` on the stack
2. Stores the SSA value into the alloca
3. GEP to the payload field (offset 1 in the struct)
4. GEP to the specific field within the payload (offset 0)
5. Loads the i64 field value
6. Branches to the merge block

The merge block uses a `phi` to select between the two arms' extracted values.

**Correctness verification**:
- Success(42) -> tag=0, payload=[42]: switch to bb2, extract payload[0] = 42. Correct.
- Failure(-1) -> tag=1, payload=[-1]: switch to bb3, extract payload[0] = -1. Correct.
- Default (bb4) is `unreachable` -- correct since the match is exhaustive over 2 variants.

**Issue (MEDIUM): Payload extraction via alloca+store+GEP+load is heavyweight**

Both arms perform identical work: they want to read field 1 (the payload) from the `%ori.Result2` SSA value. The codegen spills the entire struct to the stack, then GEPs into it. This is necessary because LLVM's `extractvalue` only works on aggregate types in SSA, and the `[1 x i64]` payload requires an index into the array.

However, the codegen could use `extractvalue` directly:
```llvm
; Ideal for both arms (since payload layout is identical):
%payload_arr = extractvalue %ori.Result2 %0, 1
%field = extractvalue [1 x i64] %payload_arr, 0
```

This avoids the alloca+store+load entirely. The current approach generates 5 instructions per arm where 2 would suffice. For `_ori_extract`, since both arms extract from the same position, the entire function could be:

```llvm
; Ideal _ori_extract:
define fastcc i64 @_ori_extract(%ori.Result2 %0) #0 {
  %payload_arr = extractvalue %ori.Result2 %0, 1
  %field = extractvalue [1 x i64] %payload_arr, 0
  ret i64 %field
}
```

Since both Success and Failure carry a single `int` at the same payload offset, the discriminant check is unnecessary -- both arms return payload[0]. But the codegen correctly generates the full switch for the general case where different arms might have different payload layouts.

**Severity**: MEDIUM -- the alloca+store+GEP+load pattern for payload extraction is unnecessarily expensive when `extractvalue` could be used instead. This will affect all pattern matches that destructure sum type fields. LLVM's optimization passes (mem2reg, SROA) will eliminate this at `-O1`+, but the unoptimized IR is significantly bloated. Each arm has 5 instructions instead of 2 (2.5x overhead).

**Issue (LOW): Duplicated payload extraction across arms**

Both bb2 and bb3 contain identical alloca+store+GEP+load sequences (only differing in SSA names). Since both variants have identical payload layout (one `int`), a single extraction before the switch would suffice. However, this is specific to the case where all variants share the same layout -- the general approach of per-arm extraction is correct.

**Severity**: LOW -- correct but redundant for this specific case.

### 5. Main Function & Call Sequence

```llvm
define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_extract(%ori.Result2 { i64 0, [1 x i64] [i64 42] })
  br label %bb1

bb1:
  %call1 = call fastcc i64 @_ori_extract(%ori.Result2 { i64 1, [1 x i64] [i64 -1] })
  br label %bb3

bb3:
  %call2 = call fastcc i64 @_ori_to_code(%ori.Status zeroinitializer)
  br label %bb5

bb5:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)
  ...overflow check...
add.ok:
  %add3 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)
  ...overflow check...
add.ok6:
  ret i64 %add.val4
}
```

**Variant construction**: Inline constant aggregates are used:
- `%ori.Result2 { i64 0, [1 x i64] [i64 42] }` for `Success(value: 42)` -- tag=0, payload=42
- `%ori.Result2 { i64 1, [1 x i64] [i64 -1] }` for `Failure(code: -1)` -- tag=1, payload=-1
- `%ori.Status zeroinitializer` for `Pending` -- tag=0 (zeroinitializer is {i64 0})

This is efficient: constant variants are passed as inline aggregate constants, avoiding any allocation.

**Overflow checks**: Both additions use `@llvm.sadd.with.overflow.i64` with dedicated panic messages. Correct per spec.

**Block boundaries**: 3 unnecessary `br label` instructions between sequential blocks (bb0->bb1, bb1->bb3, bb3->bb5). Same M-1 pattern from J1/J2 -- each let binding boundary creates a new block.

**Severity**: LOW (existing M-1).

### 6. Overflow Checking

Two additions in `_ori_main`, both correctly checked:
- `%add = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %call, i64 %call1)` -- checks 42 + (-1) = 41
- `%add3 = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call2)` -- checks 41 + 0 = 41

Each has a dedicated overflow panic path with `ori_panic_cstr` + `unreachable`.

**Duplicate overflow message strings**:
```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
@ovf.msg.1 = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00"
```
Two identical 29-byte strings. Same L-4 (string dedup) pattern from J2.

**Severity**: LOW (existing finding).

### 7. ARC Purity

Zero ARC operations in the generated IR. This is correct -- `Status` is a unit-variant enum (single i64), `Result2` is a small struct with only `int` payloads. Neither contains heap-allocated data requiring reference counting.

**Verdict**: PERFECT -- No RC on scalar-only types.

### 8. Native Code Quality (Disassembly)

**`_ori_to_code`** (39 bytes, 11 instructions):
```asm
mov    $0x2,%eax          ; default = 2 (Completed)
xor    %ecx,%ecx          ; ecx = 0
cmp    $0x0,%rdi           ; tag == 0 (Pending)?
cmove  %rcx,%rax           ; if yes, result = 0
mov    $0x1,%ecx           ; ecx = 1
cmp    $0x1,%rdi           ; tag == 1 (Running)?
cmove  %rcx,%rax           ; if yes, result = 1
mov    %rax,-0x8(%rsp)     ; spill to stack
mov    -0x8(%rsp),%rax     ; reload from stack
ret
```

The `select` chain compiled to `cmov` instructions -- excellent. No branches, fully branchless. The stack spill+reload at the end is a redundant round-trip from the M-1 block boundary pattern (the phi node lowering). Ideal would be:
```asm
mov    $0x2,%eax
xor    %ecx,%ecx
cmp    $0x0,%rdi
cmove  %rcx,%rax
mov    $0x1,%ecx
cmp    $0x1,%rdi
cmove  %rcx,%rax
ret
```
9 instructions ideal vs 11 actual. Overhead: 1.22x. The `cmov` approach is good -- avoids branch misprediction for a 3-way enum.

**`_ori_extract`** (112 bytes, ~30 instructions):
- Stack frame: 32 bytes (`sub $0x20, %rsp`)
- The `switch` lowers to `test %rdi, %rdi` (tag==0?) + `je` for Success, then falls through to Failure
- Each arm stores the full 16-byte struct to the stack (`mov %rsi, -0x10(%rbp)` + `mov %rdi, -0x8(%rbp)`), then GEPs to load the payload
- The alloca+store+load pattern manifests as actual memory operations in the native code

**Issue**: The native code for `_ori_extract` has significant overhead from the alloca+store+GEP+load pattern. Each arm stores 2 qwords and loads 1 back. The ideal would be:
```asm
; Both arms return %rsi (the payload), so:
mov    %rsi, %rax
ret
```
2 instructions ideal vs ~30 actual. Overhead: ~15x. This is the native cost of the MEDIUM finding about payload extraction via alloca.

**`_ori_main`** (139 bytes, 34 instructions):
- Clean call sequence: 3 `call` instructions for the 3 user functions
- Variant construction passes tag in `%rdi` and payload in `%rsi` (fastcc struct passing)
- `Success(42)`: `xor %eax,%eax` + `mov %eax,%edi` (tag=0), `mov $0x2a,%esi` (payload=42)
- `Failure(-1)`: `mov $0x1,%edi` (tag=1), `mov $0xffffffffffffffff,%rsi` (payload=-1)
- `Pending`: `xor %eax,%eax` + `mov %eax,%edi` (tag=0, no payload for Status)
- Overflow checks use `seto` + `jo` pattern

**Severity**: LOW for to_code/main (debug build quality). MEDIUM for extract (alloca overhead visible in native code).

### 9. Binary Size & Sections

| Metric | Value |
|--------|-------|
| Binary size | 6,561,416 bytes (6.26 MiB) |
| .text | 889,793 bytes (869 KiB) |
| .rodata | 136,504 bytes (133 KiB) |
| User code | ~290 bytes (to_code: 39, extract: 112, main: 139, wrapper: ~8) |
| Debug info | ~4.8 MiB (.debug_*) |
| Symbols | 3,709 total, 3 user (`_ori_to_code`, `_ori_extract`, `_ori_main`) |

User code is ~290 bytes out of ~890 KB .text (0.033%). Consistent with previous journeys -- runtime dominates.

**Severity**: None -- expected for debug builds with statically-linked runtime.

## Findings Summary

| # | Category | Severity | Description | Cross-ref |
|---|----------|----------|-------------|-----------|
| 1 | Payload extraction | MEDIUM | Record variant destructuring uses alloca+store+GEP+load (5 instr) where `extractvalue` (2 instr) would suffice | NEW |
| 2 | Redundant blocks | LOW | `_ori_to_code` has single-predecessor phi + unnecessary block; `_ori_main` has 3 unnecessary `br label` | M-1 (J1) |
| 3 | String dedup | LOW | Two identical `ovf.msg` constants not merged | J2 |
| 4 | Duplicate arm code | LOW | Both arms in `_ori_extract` have identical alloca+store+GEP+load sequences | NEW |

## Cross-Journey Observations (vs J1/J2)

- **New features working**: Sum types (unit + record variants), `match` expressions, variant construction, variant destructuring, decision trees
- **Sum type representation**: Unit enums (`Status`) are a single i64. Record enums (`Result2`) are `{i64, [N x i64]}` with discriminant + max-payload union. Both are correct and compact.
- **Match strategies**: Unit-variant match compiles to branchless `select` chain (excellent). Record-variant match compiles to `switch` + per-arm extraction (correct but has alloca overhead).
- **Consistent patterns**: Same `fastcc` + `nounwind` hygiene. Same M-1 block boundary pattern. Same overflow checking. Same binary size profile.
- **New finding**: Payload extraction via alloca is the main quality issue. This will affect all record variant matching and is worth optimizing in the codegen (use `extractvalue` instead of stack spill).
- **Nounwind propagation**: All 3 user functions correctly marked nounwind in 2 fixed-point passes.

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Correctness | 30% | 10/10 | Both backends produce 41. Match exhaustiveness correct. Unreachable default for complete switch. |
| Instruction Purity | 20% | 7/10 | `to_code` is near-optimal (select chain), but `extract` has 5 instr/arm where 2 suffice (alloca overhead) |
| ARC Purity | 15% | 10/10 | Zero RC ops on scalar-only sum types |
| Attributes | 15% | 8/10 | All user functions nounwind+fastcc. Missing noreturn on panic (M-2). Missing nounwind on wrapper (L-2). |
| Type Layout | 10% | 10/10 | Unit enum = single i64, record enum = {i64, [N x i64]} -- compact and correct |
| Block Layout | 10% | 7/10 | Select chain for to_code is excellent. Switch for extract is good. But redundant blocks persist (M-1). |

**Overall Score: 8.5 / 10**

The codegen demonstrates solid sum type support with correct type representations, branchless unit-variant matching, and proper exhaustiveness checking. The main quality gap is the payload extraction pattern for record variants -- using alloca+store+GEP+load when `extractvalue` would produce much cleaner IR. This is the dominant optimization opportunity for match codegen.
