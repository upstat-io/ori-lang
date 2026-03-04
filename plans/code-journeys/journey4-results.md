# Journey 4: "I am a struct"

**Date**: 2026-03-03
**Status**: PASS (Eval=57, AOT=57)

## Source

```ori
type Point = { x: int, y: int }
type Rect = { origin: Point, width: int, height: int }
@area (r: Rect) -> int = r.width * r.height;
@main () -> int = {
    let p = Point { x: 3, y: 4 };
    let r = Rect { origin: p, width: 10, height: 5 };
    p.x + p.y + area(r: r)
}
```

**Expected**: `3 + 4 + (10 * 5) = 57`

## Features Exercised

- Struct type declarations (`type Point = { x: int, y: int }`)
- Nested struct types (Rect contains Point)
- Struct literal construction (`Point { x: 3, y: 4 }`)
- Field access (`p.x`, `p.y`)
- Struct passed as function argument
- Field access on parameter (`r.width`, `r.height`)

## Phase Results

### Lexer
- Source: 462 bytes, 116 tokens, 0 errors
- Prelude: 10,331 bytes, 1,516 tokens, 0 errors

### Parser
- User module: 2 functions, 2 types, 24 expressions, 0 errors, 0 warnings
- Prelude module: 9 functions, 39 traits, 46 expressions, 0 errors, 0 warnings
- Correctly parsed struct literal contexts and struct field initializers

### Type Checker
- Prelude: 9 functions, 0 tests, 0 impls -- registration, signatures, body checking all complete
- User module: 2 functions, 0 tests, 0 impls -- all phases complete
- Import resolution: hash-first hits on `compare`, `min`, `max`; AST fallback on generic builtins (`len`, `is_empty`, `is_some`, `is_none`, `is_ok`, `is_err`)
- No type errors

### Canonicalization
- User module: 24 canon nodes, 2 roots, 6 constants, 0 decision trees
- Prelude: 46 canon nodes, 9 roots, 6 constants, 4 decision trees

### Evaluator (Interpreter)
- Exit code: 57 (CORRECT)
- No stdout, no stderr
- Trace shows correct execution order:
  1. Block entry for `@main`
  2. `let p = Point { x: 3, y: 4 }` -- struct construction with `Int(3)`, `Int(4)`
  3. `let r = Rect { origin: p, width: 10, height: 5 }` -- nested struct with ident lookup for `p`, `Int(10)`, `Int(5)`
  4. `p.x` field access (Name `x`), `p.y` field access (Name `y`)
  5. `Add(p.x, p.y)` = `Add(3, 4)` = 7
  6. Call `area(r: r)` -- resolves ident `area`, passes `r`
  7. Inside `area`: `r.width` (Name `width`), `r.height` (Name `height`), `Mul(10, 5)` = 50
  8. `Add(7, 50)` = 57

### AOT Compilation
- Build: 0.25s compile time, 0 errors
- Binary: 6,561,376 bytes (6.3 MB debug)
- Exit code: 57 (CORRECT)
- No stdout output (return-code-only journey)

## LLVM Deep Scrutiny (9 Categories)

### 1. Type Representations

```llvm
%ori.Point = type { i64, i64 }
%ori.Rect  = type { %ori.Point, i64, i64 }
```

**Assessment**: CORRECT. Point is `{i64, i64}` (16 bytes), Rect is `{Point, i64, i64}` (32 bytes). Nested struct is inlined, not heap-allocated or pointer-indirected. This matches the `Boxed` category in type registration but the IR correctly represents it as a flat aggregate. Layout is natural alignment-friendly (all i64 fields).

### 2. Function Signatures & Calling Convention

```llvm
define fastcc i64 @_ori_area(ptr %0) #0    ; struct passed by pointer
define i64 @_ori_main() #0                  ; C calling convention (entry)
define i32 @main()                          ; C wrapper
```

**Assessment**: CORRECT.
- `_ori_area` takes `Rect` by pointer (`ptr %0`) since Rect is 32 bytes (>16B threshold for by-value). Uses `fastcc` for internal calls.
- `_ori_main` uses C calling convention as the program entry point. Returns `i64` directly.
- `main()` wrapper truncates `i64` to `i32` for C ABI exit code. This is the standard pattern.
- Both user functions marked `nounwind` (attribute `#0`).

### 3. Struct Construction & Field Access

**In `_ori_area`**: The function receives `Rect` by pointer and performs per-field GEP+load+insertvalue to reconstruct the LLVM aggregate value:

```llvm
%param.load.f0.ptr = getelementptr inbounds nuw %ori.Rect, ptr %0, i32 0, i32 0  ; origin (Point)
%param.load.f0.f0.ptr = getelementptr inbounds nuw %ori.Point, ptr %param.load.f0.ptr, i32 0, i32 0
%param.load.f0.f0 = load i64, ptr %param.load.f0.f0.ptr, align 8    ; origin.x
; ... builds up insertvalue chain ...
%proj.1 = extractvalue %ori.Rect %param.load.s2, 1    ; width
%proj.2 = extractvalue %ori.Rect %param.load.s2, 2    ; height
```

**Assessment**: FUNCTIONALLY CORRECT, but has an optimization opportunity. The function loads all 4 fields of Rect (including `origin.x` and `origin.y` via the nested Point) into an LLVM aggregate, then immediately extracts only `width` (index 1) and `height` (index 2). The `origin` fields are loaded but unused. LLVM's dead code elimination will likely remove these loads at -O1+, but the codegen could be more surgical -- only loading the fields actually accessed.

**In `_ori_main`**: Rect construction uses a constant aggregate store:

```llvm
store %ori.Rect { %ori.Point { i64 3, i64 4 }, i64 10, i64 5 }, ptr %ref_arg, align 8
```

**Assessment**: CORRECT. The struct is constructed as a constant aggregate and stored to the stack alloca in a single store. The nested Point `{i64 3, i64 4}` is correctly embedded inline. The `p.x` and `p.y` accesses use the original constant values (3 and 4) directly via `add(3, 4)`, not re-loading from the struct. This is a good optimization -- the compiler recognized that `p` was a known-constant struct.

### 4. Overflow Checking

```llvm
%mul = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %proj.1, i64 %proj.2)
%add = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 3, i64 4)
%add1 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %add.val, i64 %call)
```

**Assessment**: CORRECT. All three arithmetic operations (two adds, one multiply) use LLVM overflow-checking intrinsics (`smul.with.overflow`, `sadd.with.overflow`). Each branches to a panic path with `ori_panic_cstr` on overflow. The panic function is marked `cold` (`#2`), keeping it off the hot path. Three distinct overflow message constants are allocated:
- `@ovf.msg` = "integer overflow on multiplication" (35 bytes)
- `@ovf.msg.1` = "integer overflow on addition" (29 bytes)
- `@ovf.msg.2` = "integer overflow on addition" (29 bytes)

**Note**: `ovf.msg.1` and `ovf.msg.2` have identical content. LLVM's linker/optimizer may merge them, but the codegen could deduplicate at IR generation time.

### 5. Control Flow

The `_ori_main` function has a non-linear block layout:

```
bb0 -> add.ovf_panic (overflow)
bb0 -> add.ok (success) -> call _ori_area -> bb1
bb1 -> add.ovf_panic5 (overflow)
bb1 -> add.ok4 (success) -> ret
```

**Assessment**: CORRECT. The control flow properly sequences:
1. Compute `3 + 4` with overflow check
2. On success, construct Rect and call `_ori_area`
3. Add the result of `area(r)` to `p.x + p.y` with overflow check
4. Return final result

The block ordering in the disassembly (`bb0 -> add.ok -> bb1 -> add.ok4`) has a `jmp` from `1a0d5` to `1a0f1` (jump over the continuation block), then after the call jumps back to `1a0d9`. This is standard for non-optimized builds where LLVM doesn't reorder blocks for fall-through.

### 6. Memory Management (ARC/RC)

**Assessment**: NOT APPLICABLE. No heap allocations occur in this journey. Both `Point` and `Rect` are stack-allocated aggregates (value types). The Rect is stored on the stack via `alloca` and passed by pointer to `_ori_area`. No `ori_rc_inc`, `ori_rc_dec`, or any ARC runtime calls appear in the IR or disassembly. This is correct -- structs with only primitive fields have no reference-counted components.

### 7. Entry Point Wrapper

```llvm
define i32 @main() {
entry:
  %ori_main_result = call i64 @_ori_main()
  %exit_code = trunc i64 %ori_main_result to i32
  ret i32 %exit_code
}
```

**Assessment**: CORRECT. Standard C `main()` wrapper. `_ori_main` returns `i64`, truncated to `i32` for the process exit code. The ARC trace confirms: `has_args=false`, `returns_int=true`, `has_panic=false`. No `@panic` handler installed, no args processing needed.

### 8. Nounwind Analysis

From the ARC trace:
- Fixed-point analysis: 2 functions, 2 passes, 2 marked nounwind, 0 mono-propagated
- Both `_ori_area` and `_ori_main` marked `nounwind`

**Assessment**: CORRECT. Neither function can throw (no `panic!` in user code, only overflow panics which are `cold` and use `unreachable` after the call). The nounwind attribute enables better stack unwinding and exception handling optimizations.

### 9. Disassembly Quality

**`_ori_area`** (47 bytes, 0x1a090-0x1a0bf):
- `push rax` -- save for stack alignment
- Loads from struct pointer at offsets 0x0, 0x8, 0x10, 0x18 (4 fields of Rect: Point.x, Point.y, width, height)
- `imul rcx, rax` -- multiply width * height
- Overflow check via `jo` (jump on overflow)
- Return result in `rax`

**Observation**: The disassembly loads all 4 fields (including Point.x at offset 0x0 and Point.y at offset 0x8) but only uses the last two (width at 0x10, height at 0x18). The first two loads are dead code that the CPU will execute but whose results are overwritten. At `-O0` this is expected; at `-O1+` LLVM would eliminate them.

**`_ori_main`** (135 bytes, 0x1a0c0-0x1a147):
- `sub rsp, 0x38` -- 56 bytes stack frame (Rect=32 + alignment/spills)
- `mov $3, eax; add $4, rax` -- computes `p.x + p.y` with constants
- Overflow check on addition
- Stores Rect constant fields to stack (movq immediates at rsp+0x18..0x30)
- `lea rdi, [rsp+0x18]` -- pointer to stack Rect
- `call _ori_area`
- Second addition with overflow check
- Clean return

**Assessment**: CORRECT. Code is clean for `-O0`. The struct is constructed via 4 `movq` immediates to the stack, which is efficient for constant data. The `lea` + `call` pattern for by-pointer struct passing is standard.

## Summary

| Category | Verdict | Notes |
|----------|---------|-------|
| Type representations | CORRECT | Nested struct inlined as flat aggregate |
| Calling conventions | CORRECT | By-pointer for >16B struct, fastcc internal |
| Struct construction | CORRECT | Constant aggregate store, nested Point inlined |
| Field access | CORRECT | GEP+load for by-pointer, extractvalue for by-value |
| Overflow checking | CORRECT | All 3 arithmetic ops checked |
| Control flow | CORRECT | Proper sequencing of checks and calls |
| Memory management | N/A | No heap allocations, pure value types |
| Entry point | CORRECT | Standard i64->i32 truncation wrapper |
| Nounwind | CORRECT | Both functions correctly marked nounwind |

## Observations

1. **Dead loads in `_ori_area`**: The function loads all struct fields into an aggregate then extracts only the needed ones. At `-O0` this is harmless; LLVM DCE removes them at `-O1+`. Could be optimized at codegen time by only loading accessed fields.

2. **Duplicate overflow messages**: Two identical "integer overflow on addition" string constants are emitted. LLVM may merge them, but codegen-level dedup would save a trivial amount of rodata.

3. **No ARC overhead**: This journey demonstrates that pure value-type structs with primitive fields incur zero ARC runtime cost -- no heap allocation, no reference counting, fully stack-allocated. This is the ideal path for performance-critical data.

4. **Constant propagation**: The compiler correctly propagated `p.x = 3` and `p.y = 4` as immediate constants in the `add(3, 4)` instruction rather than re-loading from the struct. Good optimization even at `-O0`.

5. **Struct-by-pointer ABI**: Rect (32 bytes) correctly crosses the 16-byte threshold for by-pointer passing. Point (16 bytes) is within the by-value threshold but since it's nested inside Rect, it's accessed via GEP from the parent pointer.
