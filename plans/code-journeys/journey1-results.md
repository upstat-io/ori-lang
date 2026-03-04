# Journey 1: "I am arithmetic" -- Results

## Source Code

```ori
// Journey 1: "I am arithmetic"
// Features: int literals, let bindings, arithmetic ops, one function call
// Expected: (3 + 4) * 5 - 2 = 33

@add (a: int, b: int) -> int = a + b;

@main () -> int = {
    let x = 3;
    let y = 4;
    let sum = add(a: x, b: y);   // = 7
    let result = sum * 5 - 2;  // = 35 - 2 = 33
    result
}
```

## Results

| Backend | Expected | Actual | Status |
|---------|----------|--------|--------|
| Eval (interpreter) | exit 33 | exit 33 | PASS |
| AOT (LLVM native)  | exit 33 | exit 33 | PASS |

---

## Transformation Timeline

### 1. Lexer

Source: 332 bytes, 75 tokens, 0 errors.

The lexer processes the user module first, producing 75 tokens from the 332-byte source. The prelude (10,331 bytes) is lexed separately, producing 1,516 tokens. Both passes complete with zero errors. Token types observed: identifiers (`@add`, `@main`, `x`, `y`, `sum`, `result`, `add`), keywords (`let`), integers (`3`, `4`, `5`, `2`), operators (`+`, `*`, `-`), punctuation (`(`, `)`, `{`, `}`, `:`, `;`, `,`).

### 2. Parser

User module: 2 functions, 16 expressions, 0 errors, 0 warnings.
Prelude: 9 functions, 39 traits, 46 expressions, 4 decision trees, 0 errors.

Parse contexts entered for user code:
- 2x "function definition" (`@add`, `@main`)
- 1x "expression" (block body)
- 1x "function call" (`add(a: x, b: y)`)
- Primary nodes: identifiers (`a`, `b`, `add`, `x`, `y`, `sum`, `result`) and integers (`3`, `4`, `5`, `2`)

The parser correctly identifies the block structure, let bindings, function call with named arguments, and binary operations with proper precedence (`*` before `-`).

### 3. Type Checker

Prelude: 9 functions, 0 tests, 0 impls -- registration, signatures, body checking all complete.
User module: 2 functions, 0 tests, 0 impls -- all three passes complete.

Import resolution:
- Hash-first miss (AST fallback): `len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err` (generic builtins)
- Hash-first hit: `compare`, `min`, `max`

No type errors. The type checker correctly infers:
- `x: int`, `y: int` from integer literals
- `sum: int` from `add(a: x, b: y)` return type
- `result: int` from `sum * 5 - 2`
- `@main` returns `int` (last expression `result`)

### 4. Canonicalization

User module: 20 canon nodes, 2 roots, 0 method roots, 6 constants, 0 decision trees.
Prelude: 46 canon nodes, 9 roots, 6 constants, 4 decision trees.

The canon IR lowers the AST to a flat representation. The 6 constants correspond to the integer literals (3, 4, 5, 2) plus function-level constants. The 2 roots are `@add` and `@main`. No decision trees needed (no match/pattern matching).

### 5. Interpreter Evaluation

Execution trace (CanId-level):

```
eval @main body: Block(CanRange(2..6), CanId(18))
  let x = 3           -- CanId(4): Let(pat0, CanId(3)=Int(3), Mutable)
  let y = 4           -- CanId(6): Let(pat1, CanId(5)=Int(4), Mutable)
  let sum = add(x, y) -- CanId(11): Let(pat2, CanId(10)=Call(...), Mutable)
    resolve add        -- CanId(7): Ident("add")
    arg a: x           -- CanId(8): Ident("x") -> 3
    arg b: y           -- CanId(9): Ident("y") -> 4
    eval @add body:
      a + b            -- CanId(2): Binary(Add, CanId(0)=Ident("a"), CanId(1)=Ident("b"))
                          evaluate_binary Add int int -> 7
  let result = ...     -- CanId(17): Let(pat3, CanId(16)=Binary(Sub, ...), Mutable)
    sum * 5            -- CanId(14): Binary(Mul, CanId(12)=Ident("sum"), CanId(13)=Int(5))
                          evaluate_binary Mul int int -> 35
    35 - 2             -- CanId(16): Binary(Sub, CanId(14), CanId(15)=Int(2))
                          evaluate_binary Sub int int -> 33
  result               -- CanId(18): Ident("result") -> 33
```

Operator precedence correctly applied: `sum * 5` evaluated first (Mul), then `- 2` (Sub). The call to `add` correctly dispatches to the user-defined function with named argument resolution.

Exit code: 33 (correct).

### 6. LLVM Codegen

#### ARC Pipeline

The ARC pipeline registered 6 user types (Ordering, PanicInfo, TraceEntry, FormatType, Sign, Alignment -- all from prelude). Two functions declared:
- `_ori_add`: 2 params, FastCC, direct return
- `_ori_main`: 0 params, C calling convention, direct return

Nounwind analysis: 2 passes, both functions marked `nounwind`. Zero mono-propagated (no monomorphization needed).

Entry point wrapper: C `main()` generated with `returns_int=true`, `has_args=false`, `has_panic=false`.

#### Generated LLVM IR

```llvm
@ovf.msg = private unnamed_addr constant [29 x i8] c"integer overflow on addition\00", align 1
@ovf.msg.1 = private unnamed_addr constant [35 x i8] c"integer overflow on multiplication\00", align 1
@ovf.msg.2 = private unnamed_addr constant [32 x i8] c"integer overflow on subtraction\00", align 1

define fastcc i64 @_ori_add(i64 %0, i64 %1) #0 {
bb0:
  %add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %0, i64 %1)
  %add.val = extractvalue { i64, i1 } %add, 0
  %add.ovf = extractvalue { i64, i1 } %add, 1
  br i1 %add.ovf, label %add.ovf_panic, label %add.ok

add.ok:
  ret i64 %add.val

add.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}

define i64 @_ori_main() #0 {
bb0:
  %call = call fastcc i64 @_ori_add(i64 3, i64 4)
  br label %bb1

bb1:
  %mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %call, i64 5)
  %mul.val = extractvalue { i64, i1 } %mul, 0
  %mul.ovf = extractvalue { i64, i1 } %mul, 1
  br i1 %mul.ovf, label %mul.ovf_panic, label %mul.ok

mul.ok:
  %sub = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %mul.val, i64 2)
  %sub.val = extractvalue { i64, i1 } %sub, 0
  %sub.ovf = extractvalue { i64, i1 } %sub, 1
  br i1 %sub.ovf, label %sub.ovf_panic, label %sub.ok

mul.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable

sub.ok:
  ret i64 %sub.val

sub.ovf_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}

define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

---

## LLVM Deep Scrutiny Report

### 1. Instruction Purity

**@_ori_add** -- 7 IR instructions (excl. panic path):
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `call @llvm.sadd.with.overflow.i64` | YES | Checked addition per spec (overflow panics) |
| 2 | `extractvalue {i64,i1} %add, 0` | YES | Extract result |
| 3 | `extractvalue {i64,i1} %add, 1` | YES | Extract overflow flag |
| 4 | `br i1 %add.ovf` | YES | Branch on overflow |
| 5 | `ret i64 %add.val` | YES | Return result |
| 6 | `call void @ori_panic_cstr` | YES | Overflow panic (cold path) |
| 7 | `unreachable` | YES | After noreturn panic |

Optimal instruction count (with overflow checks): 7. Actual: 7. **Ratio: 1.00**

**@_ori_main** -- 16 IR instructions (excl. panic paths):
| # | Instruction | Necessary? | Notes |
|---|-------------|-----------|-------|
| 1 | `call fastcc @_ori_add(i64 3, i64 4)` | YES | Call add with constants |
| 2 | `br label %bb1` | NO | Redundant unconditional branch |
| 3 | `call @llvm.smul.with.overflow.i64` | YES | Checked multiply |
| 4 | `extractvalue %mul, 0` | YES | |
| 5 | `extractvalue %mul, 1` | YES | |
| 6 | `br i1 %mul.ovf` | YES | |
| 7 | `call @llvm.ssub.with.overflow.i64` | YES | Checked subtract |
| 8 | `extractvalue %sub, 0` | YES | |
| 9 | `extractvalue %sub, 1` | YES | |
| 10 | `br i1 %sub.ovf` | YES | |
| 11 | `ret i64 %sub.val` | YES | |
| 12-16 | Panic paths (2x call+unreachable) | YES | Cold path |

Optimal (with overflow checks): 15. Actual: 16. **Ratio: 1.07** (1 redundant branch)

**@main (wrapper)** -- 3 instructions:
| # | Instruction | Necessary? |
|---|-------------|-----------|
| 1 | `call i64 @_ori_main()` | YES |
| 2 | `trunc i64 to i32` | YES (exit code is i32) |
| 3 | `ret i32` | YES |

Optimal: 3. Actual: 3. **Ratio: 1.00**

### 2. ARC Purity

Zero ARC operations in the generated IR. This is correct -- the program uses only `int` scalars (i64), which are value types requiring no reference counting. No `ori_rc_inc`, `ori_rc_dec`, `ori_buffer_rc_dec`, or any RC-related calls present.

**Verdict: PERFECT** -- No RC on scalars.

### 3. Attribute Audit

| Function | nounwind | noalias | readonly | memory | fastcc | cold |
|----------|----------|---------|----------|--------|--------|------|
| `_ori_add` | YES | n/a | missing | missing | YES | n/a |
| `_ori_main` | YES | n/a | n/a | missing | no (C) | n/a |
| `main` (wrapper) | missing | n/a | n/a | n/a | no (C) | n/a |
| `ori_panic_cstr` | n/a | n/a | n/a | n/a | n/a | YES |
| `llvm.sadd/smul/ssub` | YES | n/a | n/a | memory(none) | n/a | n/a |

Findings:
- **`_ori_add` missing `readonly` or `memory(none)`**: This function only reads its arguments and computes a result. It could be marked `memory(none)` or at minimum `readnone` (ignoring the panic path, which is `cold`/`unreachable`). However, since the panic path calls `ori_panic_cstr` (which writes), LLVM semantics prevent `readnone` on the function. The current attributes are correct given the panic semantics.
- **`_ori_main` uses C calling convention**: Correct -- `@main` is the program entry point and must use C convention for the OS to call it. The internal `_ori_main` also uses C convention since it's called from the C `main()` wrapper. This is acceptable.
- **`main` wrapper missing `nounwind`**: Minor -- the wrapper calls `_ori_main` which is nounwind, so the wrapper is transitively nounwind.
- **`ori_panic_cstr` marked `cold`**: Correct -- panic paths are cold. However, it should also be marked `noreturn` for better optimization.

### 4. Optimal IR Comparison

**Ideal `_ori_add` (hand-written):**
```llvm
define fastcc i64 @_ori_add(i64 %a, i64 %b) #0 {
  %r = call {i64, i1} @llvm.sadd.with.overflow.i64(i64 %a, i64 %b)
  %val = extractvalue {i64, i1} %r, 0
  %ovf = extractvalue {i64, i1} %r, 1
  br i1 %ovf, label %panic, label %ok
ok:
  ret i64 %val
panic:
  call void @ori_panic_cstr(ptr @ovf.msg)
  unreachable
}
```
Generated matches ideal exactly. **OPTIMAL.**

**Ideal `_ori_main` (hand-written):**
```llvm
define i64 @_ori_main() #0 {
  %sum = call fastcc i64 @_ori_add(i64 3, i64 4)
  ; multiply
  %mul = call {i64, i1} @llvm.smul.with.overflow.i64(i64 %sum, i64 5)
  %mul.v = extractvalue {i64, i1} %mul, 0
  %mul.o = extractvalue {i64, i1} %mul, 1
  br i1 %mul.o, label %mul_panic, label %mul_ok
mul_ok:
  ; subtract
  %sub = call {i64, i1} @llvm.ssub.with.overflow.i64(i64 %mul.v, i64 2)
  %sub.v = extractvalue {i64, i1} %sub, 0
  %sub.o = extractvalue {i64, i1} %sub, 1
  br i1 %sub.o, label %sub_panic, label %sub_ok
sub_ok:
  ret i64 %sub.v
mul_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.1)
  unreachable
sub_panic:
  call void @ori_panic_cstr(ptr @ovf.msg.2)
  unreachable
}
```

Generated vs. ideal: 1 extra instruction (`br label %bb1` between bb0 and bb1). LLVM's optimizer should eliminate this in optimized builds, but it is present in the unoptimized IR.

**Let binding elimination**: All four `let` bindings (`x`, `y`, `sum`, `result`) are correctly eliminated -- no `alloca`/`store`/`load` chains. Constants `3` and `4` are passed directly as arguments. The call result feeds directly into the multiply. **This is excellent codegen for an unoptimized build.**

### 5. Constant Folding Opportunities

| Expression | Foldable? | Status |
|------------|-----------|--------|
| `add(a: 3, b: 4)` | YES (inter-procedural) | NOT FOLDED -- call emitted. Acceptable: inter-procedural const folding is an optimization pass. |
| `7 * 5` | Moot | Would require folding add first |
| `35 - 2` | Moot | Would require folding multiply first |

The codegen does not perform inter-procedural constant folding, which is expected -- `@add` is a separate function and might have side effects from the overflow check. LLVM's optimization passes (`-O1`+) would inline `@add` and fold the entire main to `ret i64 33`. The current behavior for `-O0` equivalent is correct.

### 6. Binary Analysis

| Metric | Value |
|--------|-------|
| Binary size (on disk) | 6,561,376 bytes (6.26 MiB) |
| .text section | 889,633 bytes (868 KiB) |
| .rodata section | 136,568 bytes (133 KiB) |
| .debug_info | 1,642,452 bytes (1.57 MiB) |
| Debug total | ~4.7 MiB |
| User code (.text) | ~134 bytes (add: 31, main: 101, wrapper: 8) |

The binary is large due to static linking of `ori_rt` (the Ori runtime, which includes Rust stdlib for panic handling, I/O, etc.) and debug symbols. The user's actual code is 140 bytes of machine code -- the rest is runtime infrastructure.

**`_ori_add` disassembly (31 bytes, 10 instructions):**
```asm
push   %rax              ; frame setup
add    %rsi,%rdi          ; a + b
mov    %rdi,(%rsp)        ; save result
seto   %al                ; check overflow
jo     panic              ; jump if overflow
mov    (%rsp),%rax        ; load result to return reg
pop    %rcx               ; frame teardown
ret
```

Note: The machine code shows unnecessary stack spill. The x86 `add` sets the overflow flag directly, so `seto` + `jo` is redundant (just `jo` suffices). The `mov %rdi,(%rsp)` / `mov (%rsp),%rax` is an unnecessary round-trip through memory. An optimal sequence would be:
```asm
add    %rsi,%rdi
jo     panic
mov    %rdi,%rax
ret
```
This is 4 instructions vs. 8 (excluding panic path). However, this is unoptimized (`-O0` equivalent) output -- LLVM's register allocator at `-O0` does not elide stack spills.

**`_ori_main` disassembly (101 bytes, 25 instructions):**
The main function shows similar unoptimized patterns -- stack frame allocation (`sub $0x18,%rsp`), redundant loads after stores. The multiply uses `imul` (signed multiply without overflow flag extraction via hardware), with a separate `seto` check. The subtraction at constant `2` uses `sub $0x2,%rax` with a separate `seto`.

### 7. Calling Convention Audit

- `_ori_add`: `fastcc` -- Correct. Internal function, can use fast calling convention.
- `_ori_main`: C convention -- Correct. Called from C `main()` wrapper.
- `main` wrapper: C convention -- Correct. OS entry point.
- `ori_panic_cstr`: External C call -- Correct.

### 8. Overflow Check Correctness

All three arithmetic operations use LLVM overflow intrinsics:
- `llvm.sadd.with.overflow.i64` for `+`
- `llvm.smul.with.overflow.i64` for `*`
- `llvm.ssub.with.overflow.i64` for `-`

Each has a dedicated panic message string with operation-specific text. The panic path calls `ori_panic_cstr` (marked `cold`) followed by `unreachable`. This is correct per the Ori spec: "overflow panics."

### 9. Block Layout

**`_ori_add`**: 3 blocks (bb0, add.ok, add.ovf_panic). Happy path is fallthrough from bb0 to add.ok. Panic block at end. **Optimal layout.**

**`_ori_main`**: 6 blocks (bb0, bb1, mul.ok, mul.ovf_panic, sub.ok, sub.ovf_panic).
- `bb0 -> bb1`: Unconditional branch -- **redundant**. These two blocks should be merged.
- `bb1 -> mul.ok`: Conditional (overflow check). Happy path fallthrough. Good.
- `mul.ok -> sub.ok`: Conditional (overflow check). Happy path NOT fallthrough -- `mul.ovf_panic` is placed between `mul.ok` and `sub.ok`, causing an extra branch on the happy path in the IR (though LLVM's backend may reorder). The IR shows `sub.ok` after `mul.ovf_panic`, which is fine for readability but in the disassembly the panic blocks are placed after the happy path (verified in disasm).

---

## Issues Found

### MEDIUM-1: Redundant unconditional branch in `_ori_main`

**Severity**: MEDIUM (overhead >1.0, empty basic block boundary)
**Location**: `_ori_main`, `bb0 -> bb1`
**Details**: After the call to `_ori_add`, the codegen emits `br label %bb1` to jump to a new basic block that could be merged with `bb0`. This creates an empty block transition.
**Impact**: Minimal at runtime (LLVM backend eliminates it), but indicates the codegen is emitting unnecessary block boundaries at let-binding boundaries.
**Fix**: Merge sequential blocks when no control flow divergence occurs. The block splitter should not create a new block after a simple call expression.

### MEDIUM-2: Missing `noreturn` on `ori_panic_cstr` declaration

**Severity**: MEDIUM (missing function attribute)
**Location**: `declare void @ori_panic_cstr(ptr) #2` -- only has `cold`, missing `noreturn`
**Details**: `ori_panic_cstr` never returns (it calls `longjmp` or `abort`). Marking it `noreturn` would let LLVM eliminate dead code after panic calls and improve branch prediction hints.
**Impact**: LLVM may not fully optimize code paths after panic calls without `noreturn`.
**Fix**: Add `noreturn` to the `ori_panic_cstr` declaration attributes.

### LOW-1: Missing `noundef` on function parameters

**Severity**: LOW (missing parameter attribute)
**Location**: `_ori_add` parameters, `_ori_main` return
**Details**: The `i64` parameters to `_ori_add` are always defined (Ori has no undefined values). Adding `noundef` would enable LLVM to assume defined values.
**Impact**: Minimal -- LLVM can usually infer this.

### LOW-2: Missing `nounwind` on `main` wrapper

**Severity**: LOW (missing attribute on wrapper)
**Location**: `define i32 @main()`
**Details**: The C `main()` wrapper calls only `_ori_main` (which is nounwind). The wrapper itself should be nounwind.
**Impact**: Minimal -- affects exception table generation.

### LOW-3: `_ori_add` could be inlined

**Severity**: LOW (optimization opportunity, not a defect)
**Location**: `_ori_add` is a trivial function (single checked-add)
**Details**: For `-O1`+ builds, `_ori_add` should be inlined into `_ori_main`. The codegen could add `alwaysinline` for trivial functions, or rely on LLVM's inliner.
**Impact**: In optimized builds LLVM will inline this anyway. In debug builds, the call overhead is acceptable for debuggability.

---

## Codegen Quality Score

| Category | Weight | Score | Notes |
|----------|--------|-------|-------|
| Correctness | 30% | 10/10 | Both backends produce 33. Overflow checks correct. |
| Instruction Purity | 20% | 9/10 | 1 redundant branch, otherwise optimal |
| ARC Purity | 15% | 10/10 | Zero RC ops on scalar-only program |
| Attributes | 15% | 7/10 | Missing noreturn on panic, missing noundef, missing nounwind on wrapper |
| Constant Folding | 10% | 8/10 | No interprocedural folding (acceptable for -O0) |
| Block Layout | 10% | 8/10 | 1 redundant block boundary, panic blocks placed correctly |

**Overall Score: 8.8 / 10**

The codegen quality is strong for a debug/unoptimized build. The generated IR is close to hand-written quality with only minor inefficiencies (1 redundant branch, missing `noreturn` on panic). The critical aspects -- correctness, ARC purity, overflow checking, and calling conventions -- are all perfect.
