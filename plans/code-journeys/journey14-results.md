# Journey 14: COW String Operations — Results

**Date**: 2026-03-02
**Theme**: COW string creation, concatenation, substring (SSO), starts_with, length
**Expected result**: 20 (5 + 11 + 3 + 1)
**Eval exit code**: 20 (correct)
**AOT exit code**: 20 (correct)

## Source

```ori
@main () -> int = {
    let $s1 = "hello";
    let $s2 = s1 + " world";
    let $s3 = s1.substring(start: 0, end: 3);
    let $check = if s2.starts_with(prefix: "hello") then 1 else 0;

    s1.length() + s2.length() + s3.length() + check
}
```

## Phase Analysis

### 1. Lexer

- **User module**: 576 bytes, 93 tokens, 0 errors
- **Prelude**: 10,331 bytes, 1,516 tokens, 0 errors (M1 CONFIRMED, 13th journey)
- Clean tokenization of all string literals, method calls, and named arguments

### 2. Parser

- **User module**: 1 function, 25 expressions, 0 errors, 0 warnings
- **Prelude**: 9 functions, 39 traits, 46 expressions, 4 decision trees, 0 errors
- Method call contexts correctly parsed for `.substring(start:, end:)`, `.starts_with(prefix:)`, `.length()`
- Named argument syntax (`start:`, `end:`, `prefix:`) parsed without issue

### 3. Type Checker

- Registration, signature collection, body checking all complete with 0 errors
- Correctly resolves:
  - `+` on `(str, str)` as string concatenation (Add trait)
  - `.substring(start: int, end: int) -> str` as str builtin method
  - `.starts_with(prefix: str) -> bool` as str builtin method
  - `.length() -> int` as str builtin method
- Hash-first lookup hit for `compare`, `min`, `max`; AST fallback for generics (`len`, `is_empty`, `is_some`, etc.)

### 4. Canonicalizer

- **User module**: 25 source exprs -> 29 canon nodes (16% expansion, within L1 range 0-25%)
- **Prelude**: 46 source exprs -> 46 canon nodes (0% expansion)
- 6 constants materialized (string literals, integer args 0 and 3)
- 0 decision trees for user code (no pattern matching)
- Canon correctly lowers all 4 let bindings, 3 method calls, 1 binary `+`, 1 if/then/else

### 5. Eval Trace

- Clean evaluation path: 40 trace lines
- **String operations**:
  - `CanId(0)` = `Str("hello")` -- string literal creation
  - `CanId(4)` = `Binary(Add, s1, " world")` -- string concatenation, left_type="str", right_type="str"
  - `CanId(9)` = `MethodCall(s1, "substring", [0, 3])` -- substring extraction
  - `CanId(13)` = `MethodCall(s2, "starts_with", ["hello"])` -- prefix check returns true
  - `CanId(14)` = `Int(1)` -- true branch taken
- **Arithmetic**: 3 integer additions (5+11=16, 16+3=19, 19+1=20)
- Total eval calls: ~40 (lightweight for 4 let bindings + 4 method calls + 3 additions)

### 6. LLVM Codegen

#### Build Stats
- Compile time: 0.64s (first run, includes ARC pipeline)
- Nounwind analysis: 1 pass, 0 nounwind functions, 0 mono-propagated
- Entry point: `_ori_main() -> i64`, no args, returns int, C main wrapper

#### Type Registration
- `Ordering` (Less/Equal/Greater), `TraceEntry`, `Error`, `FormatSpec` registered (prelude types)
- No user-defined types in this journey

#### String Representation
- `{ i64, i64, ptr }` -- 24-byte fat pointer (len, cap/sso-data, data-ptr/sso-flags)
- All allocas correctly `align 8`
- String constants: `@str = [6 x i8] c"hello\00"`, `@str.1 = [7 x i8] c" world\00"`, `@str.2 = [6 x i8] c"hello\00"`
- Note: `@str` and `@str.2` are duplicate "hello" constants (both 6 bytes including null) -- missed dedup

#### Runtime Functions Called
| Function | Calls | Purpose |
|----------|-------|---------|
| `ori_str_from_raw` | 3 | Create OriStr from C string literal |
| `ori_str_concat` | 1 | String concatenation (s1 + " world") |
| `ori_str_substring` | 1 | Substring extraction (s1, 0, 3) |
| `ori_str_starts_with` | 1 | Prefix check (s2, "hello") |
| `ori_str_len` | 3 | Length of s1, s2, s3 |
| `ori_rc_inc` | 3 | RC increment (SSO-guarded) |
| `ori_rc_dec` | 16 | RC decrement (SSO-guarded) |
| `ori_rc_free` | 1 | Drop function body |

### 7. ARC Trace

- Build stderr shows type registration + function compilation, no ARC-specific dumps
- ARC pipeline runs but no explicit trace (would need `ORI_DUMP_AFTER_ARC=1`)

### 8. Binary Analysis

- **Binary size**: 6,708,784 bytes (~6.4 MB) -- consistent with previous journeys
- **`_ori_main` size**: 0x5e3 = 1,507 bytes (largest user function seen in any journey)
- **`_ori_drop$3` size**: 0x12 = 18 bytes -- calls `ori_rc_free(ptr, 24, 8)` (24-byte str, 8-byte align)
- **Sections**: .text 942,585 bytes, .rodata 135,946 bytes

#### Key Runtime Symbol Sizes
| Symbol | Size (bytes) | Notes |
|--------|-------------|-------|
| `ori_str_concat` | 1,927 | Largest string op (allocation + copy) |
| `ori_str_substring` | 1,683 | Zero-copy slice for heap, copy for SSO |
| `ori_str_starts_with` | 205 | Delegates to Rust `str::starts_with` |
| `ori_str_from_raw` | 214 | C string -> OriStr construction |
| `ori_str_len` | 166 | SSO-safe length extraction |
| `ori_rc_inc` | 405 | Full RC increment with header checks |
| `ori_rc_dec` | 496 | RC decrement + conditional free |
| `ori_rc_free` | 384 | Deallocation |

#### Disassembly Observations
- `_ori_main` at 0x1eb00: `sub $0x1c8, %rsp` -- 456 bytes of stack (large, 12 alloca slots)
- SSO check: `movabs $0x8000000000000000, %rdx` + `and` + `cmp` + `setne` + `or` with null check -- 7-instruction sequence repeated many times
- `ori_str_starts_with` calls `deref_str` (Rust `str::starts_with` via `core::str`) -- zero-copy comparison confirmed
- `ori_str_contains` and `ori_str_ends_with` are also linked (sibling implementations)

## LLVM Deep Scrutiny (9 Categories)

### S1. Control Flow Graph Integrity

- **Basic blocks**: 14 named blocks (bb0-bb13) + 42 RC blocks = 56 total
- **Orphaned landing pads**: 5 blocks with "No predecessors" (bb2, bb4, bb9, bb11, bb13) -- **M11 CONFIRMED** (14th journey)
- All orphaned blocks contain landingpad cleanup code for exception unwind paths that are never reached
- **If/else for starts_with**: Clean `br i1 %ori_str_starts_with, label %bb5, label %bb6` -> phi merge at bb7
- **Observation**: bb5 and bb6 are trivial single-instruction blocks (`br label %bb7`) feeding a phi node -- L3 (trivial if/else) CONFIRMED again

### S2. Type Layout and ABI

- **OriStr representation**: `{ i64, i64, ptr }` = 24 bytes, align 8 -- correct
- **All sret allocas**: `align 8` -- correct for 24-byte struct
- **All stores**: `align 8` -- correct
- **Field loads from sret**: `align 4` for i64 fields (lines 28, 31, 167, 170, 179, 182, 232, 235, 284, 287) -- **M5 CONFIRMED** (14th journey)
  - `load i64, ptr %str.val.f0.ptr, align 4` should be `align 8`
  - `load ptr, ptr %str.val.f2.ptr, align 8` -- ptr field correctly aligned
  - The i64 fields at offsets 0 and 8 in a `{ i64, i64, ptr }` struct are naturally 8-byte aligned, but codegen emits `align 4`
- **Sret calling convention**: All string-returning functions (`ori_str_from_raw`, `ori_str_concat`, `ori_str_substring`) use `ptr noalias sret(...)` -- correct
- **Pointer passing**: `ori_str_len`, `ori_str_starts_with` take `ptr` (pointer to OriStr on stack) -- SSO-safe, no raw field extraction

### S3. SSA Form and Value Flow

- Clean SSA with `insertvalue` chains for building fat pointer values from sret results
- Each sret result loaded field-by-field (GEP+load+insertvalue x3) -- verbose but correct
- **3 string values tracked as SSA values**:
  - `%str.val.s2` = s1 ("hello")
  - `%ori_str_concat.s2` = s2 ("hello world")
  - `%ori_str_substring.s2` = s3 ("hel")
- Phi node at bb7: `%v16 = phi i64 [ 0, %bb6 ], [ 1, %bb5 ]` -- correctly captures if/else result

### S4. ARC / Reference Counting

- **RC inc**: 3 calls total
  1. `rc_inc` on s1 before passing to `ori_str_concat` (s1 survives beyond concat)
  2. `rc_inc` on s1 again before `ori_str_substring` (s1 used after substring)
  3. `rc_inc` on s2 before `ori_str_starts_with` (s2 used after starts_with)
- **RC dec**: 16 calls total
  - 5 on normal path (cleanup: rhs of concat, s1 copy after concat, s1 after substring, s2 after starts_with+len, s2 final, s3 after len, prefix "hello" temporary)
  - 11 on exception cleanup paths (orphaned landing pads, never reached)
- **SSO guards**: Every RC inc/dec is guarded by SSO check (MSB of ptr field) + null check
  - "hello" (5 bytes) will be SSO -- RC operations correctly skipped at runtime
  - "hello world" (11 bytes) will be SSO (< 23 bytes threshold) -- also skipped
  - "hel" (3 bytes) will be SSO -- also skipped
  - All 3 strings fit in SSO inline storage, so NO heap RC operations execute at runtime
- **RC balance**: For heap strings, inc/dec pairing is correct (each string created, possibly incremented, eventually decremented to 0)
- **Drop function**: `_ori_drop$3` calls `ori_rc_free(ptr, 24, 8)` -- correct for 24-byte OriStr

**NEW FINDING (M17)**: The RC inc/dec ratio is heavily skewed (3 inc vs 16 dec on all paths combined). While 11 of the 16 decs are on unreachable exception paths, this indicates the ARC pipeline conservatively inserts cleanup for exception handling that will never execute. In this specific journey all strings are SSO, so all 19 RC operations are no-ops at runtime (SSO guard skips them all). But for programs with heap strings (> 22 bytes), the 16 dec calls on the normal + exception paths represent significant overhead.

**NEW FINDING (M18)**: Redundant SSO check sequences. Each SSO check is 7 instructions (`ptrtoint` + `and` + `icmp ne` + `ptrtoint` + `icmp eq` + `or` + `br`). With 19 total inc/dec guarded sites (3 inc + 16 dec), that's 133 instructions of SSO checking. The null check (`ptrtoint` + `icmp eq 0`) is redundant with the SSO check when the pointer has already been checked -- but each check recomputes from scratch because SSA values differ per-site. A dedicated "RC-eligible" flag computed once per string value would reduce this significantly.

### S5. String Operation Correctness

- **`ori_str_from_raw`**: Creates OriStr from C string literal + length. Called 3 times (s1, " world", "hello" for starts_with). Correct sret pattern.
- **`ori_str_concat`**: Takes two `*const OriStr` pointers, returns sret OriStr. Called correctly with s1 and " world". The runtime will produce "hello world" (11 bytes, fits SSO).
- **`ori_str_substring`**: Takes `*const OriStr` + start (i64 0) + end (i64 3), returns sret OriStr. Called on s1 ("hello"), producing "hel" (3 bytes, fits SSO). The comment in `string_builtins.rs` says "zero-copy seamless slice for heap strings" -- but since s1 is SSO, the runtime will copy (no shareable heap buffer for SSO).
- **`ori_str_starts_with`**: Takes two `*const OriStr`, returns `i1`. Delegates to Rust `core::str::starts_with` via `deref_str` -- zero-copy comparison confirmed from disassembly.
- **`ori_str_len`**: Takes `*const OriStr`, returns i64. SSO-safe dispatch in runtime.
- **Duplicate constant**: `@str` and `@str.2` are both `[6 x i8] c"hello\00"` -- LLVM will likely merge these via GlobalMerge, but the codegen emits them separately.

**NEW FINDING (L8)**: Duplicate string constants. `@str` and `@str.2` are identical `[6 x i8] c"hello\00"`. The codegen creates a fresh global for each string literal occurrence rather than deduplicating at the LLVM level. While LLVM's constant merging handles this in optimized builds, it adds unnecessary bloat in debug/unoptimized IR.

### S6. Dead Code and Unreachable Paths

- **5 orphaned landing pads** (bb2, bb4, bb9, bb11, bb13): All have `; No predecessors!` comments. These contain cleanup code for exception unwind paths from `invoke` instructions that don't exist in this function (all calls are plain `call`, not `invoke`).
  - bb2: cleanup for s1 + concat
  - bb4: cleanup for s1 + concat + substring
  - bb9: cleanup for concat + substring (around starts_with)
  - bb11: cleanup for substring (around len)
  - bb13: bare `resume` -- outermost cleanup
- **M11 CONFIRMED**: 14th consecutive journey with orphaned landing pads
- **Dead RC on exception paths**: 11 of 16 RC dec calls are in these unreachable blocks

### S7. Calling Convention and Attributes

- **`_ori_main`**: No `nounwind` attribute -- **M10 CONFIRMED** (14th journey)
  - Nounwind analysis reports 0 nounwind functions. This is because string runtime functions (`ori_str_concat`, `ori_str_substring`) can allocate and potentially panic.
  - However, `ori_str_len`, `ori_str_starts_with` are non-allocating and could be marked `nounwind`
- **`_ori_drop$3`**: `#2 = { cold nounwind }` -- correct (cold path, no throws)
- **`ori_rc_inc`**: `#1 = { nounwind memory(inaccessiblemem: readwrite) }` -- correct (only touches RC header)
- **`ori_rc_dec`**: `#1 = { nounwind memory(inaccessiblemem: readwrite) }` -- correct
- **`ori_rc_free`**: `#0 = { nounwind }` -- correct
- **`ori_str_from_raw`**: No attributes (allocates, may panic) -- conservative but correct
- **`ori_str_concat`**: No attributes (allocates) -- correct
- **`ori_str_substring`**: No attributes -- correct (may allocate for SSO copy path)
- **`ori_str_starts_with`**: No attributes -- should be `nounwind readonly` since it only reads
- **`ori_str_len`**: No attributes -- should be `nounwind readonly` since it only reads the flags byte
- **personality**: `@rust_eh_personality` declared -- Rust unwinding ABI

### S8. Instruction Quality / Optimization Opportunities

- **M3 CONFIRMED**: Unnecessary `br label` (unconditional branches to immediately following block) throughout. 26 total `br label` instructions.
- **Verbose sret unpacking**: Each sret result goes through 3x (GEP + load + insertvalue). This is the correct safe pattern but verbose -- LLVM optimization passes will clean this up.
- **Large stack frame**: 456 bytes (`sub $0x1c8, %rsp`) for 12 alloca slots. Several could be eliminated:
  - `str_len.self`, `str_len.self151`, `str_len.self172` -- three separate allocas for the same operation (store + call `ori_str_len`). A single reusable alloca would suffice.
  - `str_op.lhs` + `str_op.rhs` and `str_op.lhs75` + `str_op.rhs76` -- two pairs for concat and starts_with

**NEW FINDING (L9)**: Non-reusable temporary allocas. The IR creates separate alloca slots for each `ori_str_len` call site (`str_len.self`, `str_len.self151`, `str_len.self172`) even though they're used sequentially and never overlap. A single temporary alloca could serve all three calls, reducing the stack frame by 48 bytes.

- **Instruction count**: `_ori_main` is 1,507 bytes of native code for a 7-line function. The vast majority is SSO check sequences and RC cleanup on unreachable paths.

### S9. Comparison with Eval Path

- **Eval**: 40 trace events, direct method dispatch, no RC overhead
- **AOT**: 56 basic blocks, 19 SSO checks, 5 orphaned landing pads, 456-byte stack frame
- **Behavioral parity**: Both produce 20 -- correct
- **Performance characteristics**: For this program, ALL strings fit SSO (5, 11, 3 bytes -- all < 23). The AOT path executes ~133 SSO check instructions that all resolve to "skip", plus 3 `ori_str_len` calls, 1 `ori_str_concat`, 1 `ori_str_substring`, 1 `ori_str_starts_with`. The eval path executes 4 method dispatches + 3 integer adds directly.
- **Key advantage of AOT**: For larger programs with heap strings, the SSO guard avoids unnecessary RC operations. The runtime's zero-copy substring and starts_with (via Rust `core::str`) are well-optimized.

## Findings Summary

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| M1 | MEDIUM | Prelude overhead (10,331 bytes constant) | CONFIRMED (13/13) |
| M3 | MEDIUM | Unnecessary `br label` after function calls | CONFIRMED (13/13) |
| M5 | MEDIUM | `align 4` on i64 struct field loads -- should be `align 8` | CONFIRMED |
| M10 | MEDIUM | `_ori_main` missing `nounwind` | CONFIRMED |
| M11 | MEDIUM | Orphaned landing pads with no predecessors (5 this journey) | CONFIRMED |
| M17 | MEDIUM | Excessive RC dec on exception paths (16 dec vs 3 inc) | NEW |
| M18 | MEDIUM | Redundant SSO check sequences (133 instructions of guard checks) | NEW |
| L3 | LOW | Trivial if/else -> branch+phi instead of select | CONFIRMED |
| L8 | LOW | Duplicate string constants (`@str` and `@str.2` both "hello") | NEW |
| L9 | LOW | Non-reusable temporary allocas for sequential `ori_str_len` calls | NEW |

## COW-Specific Analysis

### SSO Optimization Visibility

SSO is fully visible in the IR through the guard pattern:
```llvm
%rc_inc.p2i = ptrtoint ptr %data to i64
%rc_inc.sso_flag = and i64 %rc_inc.p2i, -9223372036854775808  ; 0x8000...
%rc_inc.is_sso = icmp ne i64 %rc_inc.sso_flag, 0
%rc_inc.null = icmp eq i64 %rc_inc.p2i, 0
%rc_inc.skip_rc = or i1 %rc_inc.is_sso, %rc_inc.null
br i1 %rc_inc.skip_rc, label %sso_skip, label %heap
```
This pattern correctly detects SSO strings by checking the MSB of the ptr field (which overlaps with the SSO flags byte on little-endian x86-64). Source: `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs:269-292`.

### OriStr Fat Pointer Layout

The layout `{ i64, i64, ptr }` is visible throughout:
- **Heap interpretation**: `{ len, cap, data_ptr }`
- **SSO interpretation**: `{ inline[0:7], inline[8:15], inline[16:22] | flags }`
- The two layouts share the same 24-byte footprint. The SSO flag (MSB of byte 23) determines which interpretation is active.
- All property access goes through runtime helpers (`ori_str_len`, `ori_str_data`) which dispatch on the SSO flag -- the codegen never directly interprets field 0 as "len" (correct per the SSO invariant documented in `string_builtins.rs:6-12`).

### String Concatenation (ori_str_concat)

- Called via sret: `call void @ori_str_concat(ptr %sret, ptr %lhs, ptr %rhs)`
- Both operands passed by pointer (alloca + store before call)
- Result unpacked via 3x GEP+load+insertvalue
- RC of `" world"` temporary correctly decremented after concat
- RC of `s1` correctly incremented before concat (s1 survives)
- For this program: "hello" (5B) + " world" (6B) = "hello world" (11B) -- fits SSO, no heap allocation

### Substring (ori_str_substring)

- Called via sret: `call void @ori_str_substring(ptr %sret, ptr %self, i64 0, i64 3)`
- Source string `s1` passed by pointer
- Runtime behavior: since s1 is SSO, the substring copies inline bytes (no shared buffer possible for SSO). If s1 were heap-allocated, the runtime would create a zero-copy view with shared backing buffer (RC inc on original).
- Result "hel" (3 bytes) fits SSO -- stored inline

### starts_with (ori_str_starts_with)

- Called directly: `%result = call i1 @ori_str_starts_with(ptr %lhs, ptr %rhs)`
- Returns `i1` (boolean) -- no sret needed
- Disassembly confirms delegation to `core::str::starts_with` via `deref_str` -- true zero-copy comparison on the underlying byte data
- RC of prefix "hello" temporary correctly decremented after call

### ARC Lifecycle Correctness

For this program, all strings are SSO (< 23 bytes), so the RC lifecycle is a no-op at runtime. But the codegen correctly handles the general case:

1. `s1 = from_raw("hello", 5)` -- creates SSO string
2. `rc_inc(s1)` -- guard skips (SSO)
3. `s2 = concat(s1, from_raw(" world", 6))` -- creates SSO "hello world"
4. `rc_dec(" world" temp)` -- guard skips (SSO)
5. `rc_dec(s1)` -- guard skips (SSO), balances the inc from step 2
6. `rc_inc(s1)` -- guard skips (SSO), for substring
7. `s3 = substring(s1, 0, 3)` -- creates SSO "hel"
8. `rc_dec(s1)` -- after substring path completes
9. `rc_inc(s2)` -- for starts_with
10. `starts_with(s2, from_raw("hello", 5))` -- returns true
11. `rc_dec(s2)` + `rc_dec("hello" temp)` -- cleanup after starts_with
12. `len(s1)` -> 5, `len(s2)` -> 11, `len(s3)` -> 3
13. `rc_dec(s1)`, `rc_dec(s2)`, `rc_dec(s3)` -- final cleanup
14. Return 5 + 11 + 3 + 1 = 20

## Cross-Reference with Previous Journeys

| Finding | J9 (Strings) | J14 (COW Strings) | Delta |
|---------|-------------|-------------------|-------|
| String layout | `{ i64, i64, ptr }` | `{ i64, i64, ptr }` | Same |
| SSO guard | Present | Present (expanded) | More call sites |
| RC lifecycle | Correct | Correct | Confirmed |
| Orphaned landing pads | Present | Present (5) | Consistent |
| `align 4` on i64 | Present | Present | Persistent |
| `.length()` impl | `extractvalue` (zero-cost) | `ori_str_len` call | CHANGED -- J9 was direct field extraction, J14 calls runtime function |

**Notable J9-J14 difference**: Journey 9 compiled `.length()` as a zero-cost `extractvalue` (field 0 extraction). Journey 14 compiles it as a function call to `ori_str_len`. This is actually **correct** for J14 -- the SSO invariant means field 0 cannot be directly interpreted as length (it could be inline byte data). The J9 optimization was likely pre-SSO-aware codegen or a simpler string without SSO. The `ori_str_len` call is the safe path that dispatches on the SSO flag.

## Responsible Source Files

| File | Relevance |
|------|-----------|
| `compiler/ori_llvm/src/codegen/arc_emitter/rc_buffer_ops.rs` | SSO check emission, fat pointer RC inc/dec |
| `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/string_builtins.rs` | String method codegen (length, substring, starts_with, etc.) |
| `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/mod.rs` | Builtin method dispatch table |
| `compiler/ori_llvm/src/codegen/arc_emitter/operators.rs` | String `+` operator (concat) codegen |
| `compiler/ori_llvm/src/codegen/arc_emitter/value_emission.rs` | String literal -> `ori_str_from_raw` emission |
| `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs` | Runtime function declarations |
| `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` | `_ori_drop$3` generation |
