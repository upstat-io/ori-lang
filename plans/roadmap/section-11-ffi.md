---
section: 11
title: Foreign Function Interface (FFI)
status: in-progress
last_verified: "2026-03-29"
verified_by: "Claude Opus 4.6 (1M context)"
reviewed: true
tier: 4
goal: Enable Ori to call C libraries, system APIs, and JavaScript APIs (WASM target)
estimated_completion: "~30% infrastructure, ~0% end-to-end functionality"
spec:
  - spec/26-ffi.md
verification_notes:
  - "11.1 parser/IR/formatter COMPLETE (20 Rust tests), typeck/codegen NEEDS IMPLEMENTATION"
  - "11.3 parser/IR/typeck COMPLETE (17+ spec tests incl 14 compile-fail), codegen NEEDS IMPLEMENTATION"
  - "11.4 parser/typeck/eval/ARC COMPLETE (6 spec tests), WEAK TESTS on capability enforcement"
  - "Zero E2E FFI capability -- no Ori program can call a C function today"
  - "Stale plan annotation TPR-01-043 in ori_parse/src/grammar/attr/repr.rs:5"
sections:
  - id: "11.1"
    title: Extern Block Syntax
    status: partial
    notes: "parse+IR+fmt complete, typeck+codegen missing"
  - id: "11.2"
    title: C ABI Types
    status: not-started
  - id: "11.3"
    title: "#repr Attribute"
    status: partial
    notes: "parse+IR+typeck+repr-opt complete (57 repr tests, 17+ spec tests), codegen missing"
  - id: "11.4"
    title: Unsafe Blocks
    status: partial
    notes: "parse+typeck+eval+ARC complete (6 spec tests), capability enforcement missing, WEAK TESTS"
  - id: "11.5"
    title: FFI Capability
    status: not-started
  - id: "11.6"
    title: Callbacks (Native)
    status: not-started
  - id: "11.7"
    title: Build System Integration
    status: not-started
  - id: "11.8"
    title: compile_error Built-in
    status: not-started
    notes: "cross-ref Section 13.10"
  - id: "11.9"
    title: WASM Target (Section 2)
    status: not-started
    notes: "WASM infra exists in ori_llvm but not connected to user FFI"
  - id: "11.10"
    title: JsValue and Async (Section 3-4)
    status: not-started
  - id: "11.11"
    title: Deep FFI -- Higher-Level FFI Abstractions
    status: not-started
---

# Section 11: Foreign Function Interface (FFI)

**Goal**: Enable Ori to call C libraries, system APIs, and JavaScript APIs (WASM target)

**Criticality**: **CRITICAL** -- Without FFI, Ori cannot integrate with the software ecosystem

**Proposal**: `proposals/approved/platform-ffi-proposal.md`

**Dependencies**: Section 6 (Capabilities -- `uses FFI` capability system must be in place), Section 18 (Const Generics -- Deep FFI Phase 5)

> **Verification summary (2026-03-29)**: ~30% infrastructure exists (parser/IR/formatter/typeck), concentrated in 11.1, 11.3, and 11.4. Zero end-to-end FFI capability -- no Ori program can call a C function today. The heavy lifting (codegen, linking, CPtr type system, capability enforcement) remains. Grammar (`grammar.ebnf`) and spec (`spec/26-ffi.md`, 554 lines) are comprehensive.

### Sync Points: CPtr and C ABI Types (multi-crate sync required)


Adding `CPtr` and C ABI types requires updates across these crates:

1. **`ori_ir`** -- Add `CPtr` type variant, C type aliases (`c_int`, `c_long`, etc.)
2. **`ori_types`** -- Register CPtr type, FFI-safety validation, `Option<CPtr>` nullability handling
3. **`ori_eval`** -- CPtr runtime value representation, pointer operations in unsafe blocks
4. **`ori_llvm`** -- CPtr maps to LLVM opaque `ptr` type, C calling convention codegen
5. **`library/std/ffi.ori`** -- CPtr type definition, FfiError type (for Deep FFI)

---

## Design Decisions (Approved)

| Question | Decision | Rationale |
|----------|----------|-----------|
| Should FFI require capability? | Yes, `FFI` capability | Consistent with effect tracking |
| Single or multiple capabilities? | Single `FFI` | Platform-agnostic user code |
| Support C++ directly? | No | C ABI only; C++ via extern "C" |
| Support callbacks? | Yes (native) | Required for many C APIs |
| Memory management? | Manual in unsafe blocks | C doesn't know about ARC |
| Async WASM handling? | Implicit JsPromise resolution | Preserves Ori's "no await" philosophy |
| Unsafe operations? | `unsafe { ... }` blocks | Explicit marking for unverifiable ops |

---

## 11.1 Extern Block Syntax

**Spec section**: `spec/26-ffi.md § Extern Blocks`

### Native (C ABI)

```ori
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_sqrt (x: float) -> float as "sqrt"
}
```

### JavaScript (WASM)

```ori
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
    @_now () -> float as "Date.now"
}

extern "js" from "./utils.js" {
    @_formatDate (timestamp: int) -> str as "formatDate"
}
```

### Implementation

- [x] **Spec**: Add `spec/26-ffi.md` with extern block syntax (verified 2026-03-29)
  - [x] Define extern block grammar -- `grammar.ebnf` lines 209-215 (verified 2026-03-29)
  - [x] Define calling conventions ("c", "js") (verified 2026-03-29)
  - [x] Define linkage semantics (verified 2026-03-29)

- [x] **Lexer**: Add tokens (verified 2026-03-29)
  - [x] `extern` keyword -- `TokenKind::Extern` reserved keyword (verified 2026-03-29)
  - [x] String literals for ABI ("c", "js") -- standard string tokens, validated in parser (verified 2026-03-29)

- [x] **Parser**: Parse extern blocks (verified 2026-03-29) -- 20 Rust parser phase tests pass in `oric/tests/phases/parse/extern_def.rs`
  - [x] `parse_extern_block()` in parser -- `ori_parse/src/grammar/item/extern_def.rs:22` (verified 2026-03-29)
  - [x] Add `ExternBlock` to AST -- `ori_ir/src/ast/items/extern_def.rs:64` (verified 2026-03-29)
  - [x] Add `ExternItem` variants -- `ori_ir/src/ast/items/extern_def.rs:37` (verified 2026-03-29)
  - [x] `from "lib"` library specification -- contextual keyword check (verified 2026-03-29)
  - [x] `as "name"` name mapping (verified 2026-03-29)
  - [x] C variadic (`...`) parsing with `is_c_variadic` flag (verified 2026-03-29)
  - [x] Unknown convention warning emitted (verified 2026-03-29)
  - [x] Formatter: `ori_fmt/src/declarations/extern_def.rs` (verified 2026-03-29)

- [ ] **Type checker**: Validate extern declarations -- NEEDS IMPLEMENTATION
  - [ ] Ensure types are FFI-safe
  - [ ] Check for `uses FFI` in callers

- [ ] **Codegen**: Generate external references -- NEEDS IMPLEMENTATION (GAP: parsed ExternBlock AST is never consumed by codegen; `declare_extern_function` exists only for runtime functions)
  - [ ] Emit LLVM `declare` for C functions
  - [ ] Handle calling convention
  - [ ] Link external symbols

- [ ] **LLVM Support**: LLVM codegen for extern blocks
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- extern blocks codegen (NOTE: file does not exist yet)
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/extern_blocks.ori` -- NEEDS TESTS (no spec tests exist; only 20 Rust parser phase tests)
  - [ ] Basic extern function declaration
  - [ ] Multiple functions in one block
  - [ ] Name mapping with `as`
  - [ ] Library specification with `from`
  - [ ] End-to-end test (parse -> typeck -> codegen -> execute C call)

> **NOTE**: Zero end-to-end FFI capability -- no Ori program can actually call a C function today. Parser infrastructure is complete but codegen gap means extern blocks are parsed and stored but never processed into callable functions.

---

## 11.2 C ABI Types

**Spec section**: `spec/26-ffi.md § C Types`

### Primitive Mappings

| Ori Type | C Type | Size |
|----------|--------|------|
| `c_char` | `char` | 1 byte |
| `c_short` | `short` | 2 bytes |
| `c_int` | `int` | 4 bytes |
| `c_long` | `long` | platform |
| `c_longlong` | `long long` | 8 bytes |
| `c_float` | `float` | 4 bytes |
| `c_double` | `double` | 8 bytes |
| `c_size` | `size_t` | platform |

### CPtr Type

```ori
type CPtr  // Opaque pointer - cannot be dereferenced in Ori

extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: CPtr) -> int
    @sqlite3_close (db: CPtr) -> int
}

// Nullable pointers
extern "c" from "foo" {
    @get_resource (id: int) -> Option<CPtr>
}
```

### Implementation

- [ ] **Spec**: Add C types section
  - [ ] Primitive type mappings
  - [ ] `CPtr` opaque pointer type
  - [ ] `Option<CPtr>` for nullable pointers

- [ ] **Types**: Add C primitive types
  - [ ] Add `CPtr` to type system
  - [ ] Add C type aliases (`c_int`, `c_long`, etc.) with correct Pool widths
  - [ ] Size/alignment handling — c_char=i8, c_short=i16, c_int=i32, c_longlong=i64, c_float=f32, c_double=f64
  - [ ] Platform-dependent sizes (`c_long`, `c_size`) — i32 on Win64/ILP32, i64 on LP64
  - [ ] **Fixes BUG-02-004**: Replace `resolve_ffi_concrete()` (well_known/mod.rs:327) which currently collapses ALL c_* types to `Idx::INT`/`Idx::FLOAT` (i64/f64), losing C ABI widths. Touches: `ori_types/pool/` (new Tag variants + Idx constants), `ori_types/check/well_known/`, `ori_llvm/codegen/type_info/store.rs` (new TypeInfo variants), `ori_repr/` (triviality), `ori_eval/` (Value representation decision). Decide: implicit `int` ↔ `c_int` promotion vs explicit cast (resolves spec ambiguity). Pool resolution-redirect path was added by BUG-04-021 fix as a placeholder for ARC classification — must be removed in favor of proper variants.

- [ ] **Type checker**: FFI type validation
  - [ ] Warn on non-FFI-safe types
  - [ ] Validate CPtr usage

- [ ] **LLVM Support**: LLVM codegen for C ABI types
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- C ABI types codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/c_types.ori`
  - [ ] All primitive C types
  - [ ] CPtr operations
  - [ ] Option<CPtr> for nullable

---

## 11.3 #repr Attribute

**Spec section**: `spec/26-ffi.md § C Structs`
**Proposal**: `proposals/approved/repr-extensions-proposal.md`

### Syntax

| Attribute | Effect |
|-----------|--------|
| `#repr("c")` | C-compatible field layout and alignment |
| `#repr("packed")` | No padding between fields |
| `#repr("transparent")` | Same layout as single field |
| `#repr("aligned", N)` | Minimum N-byte alignment (N must be power of two) |

```ori
#repr("c")
type CTimeSpec = { tv_sec: c_long, tv_nsec: c_long }

#repr("packed")
type PacketHeader = { version: byte, flags: byte, length: c_short }

#repr("transparent")
type FileHandle = { fd: c_int }

#repr("aligned", 64)
type CacheAligned = { value: int }
```

**Combining:** `#repr("c")` may combine with `#repr("aligned", N)`. Other combinations are invalid.

**Newtypes:** Newtypes (`type T = U`) are implicitly transparent.

### Implementation

- [x] **IR**: `ReprAttrKind` enum in `ori_ir/src/ast/items/types.rs` -- C, Packed, Transparent, Aligned(u64) (verified 2026-03-29)

- [x] **Parser**: Parse `#repr` attribute variants (verified 2026-03-29) -- `ori_parse/src/grammar/attr/repr.rs:77`
  - [x] `#repr("c")` (verified 2026-03-29)
  - [x] `#repr("packed")` (verified 2026-03-29)
  - [x] `#repr("transparent")` (verified 2026-03-29)
  - [x] `#repr("aligned", N)` -- validate power of two (verified 2026-03-29)
  - [x] Combined syntax `#repr("c", "aligned", 16)` (verified 2026-03-29)
  - [x] Unknown repr value error reporting (verified 2026-03-29)

- [x] **Type checker**: Validate #repr usage (verified 2026-03-29) -- `validate_and_merge_repr_attrs` in `ori_types/src/check/registration/user_types.rs`
  - [x] Only valid on struct types, not sum types -- E2041 (verified 2026-03-29)
  - [x] `transparent` requires exactly one field -- E2041 (verified 2026-03-29)
  - [x] `aligned` N must be power of two -- E2041 (verified 2026-03-29)
  - [x] Reject `packed` + `aligned` combination -- E2041 (verified 2026-03-29)
  - [x] Reject `c` + `packed` combination -- E2041 (verified 2026-03-29)
  - [x] Reject duplicate attrs -- E2041 (verified 2026-03-29)
  - [x] Reject repr on newtypes -- E2041 (verified 2026-03-29)

- [x] **Repr-opt crate**: `ori_repr` has `ReprAttribute` enum and `StructRepr` with layout computation -- 57 tests pass (verified 2026-03-29)

- [ ] **Codegen**: Generate appropriate LLVM layout -- NEEDS IMPLEMENTATION
  - [ ] `#repr("c")` -- default struct, no packed
  - [ ] `#repr("packed")` -- LLVM packed struct type
  - [ ] `#repr("transparent")` -- same type as inner field
  - [ ] `#repr("aligned", N)` -- align N on allocations

- [ ] **LLVM Support**: LLVM codegen for #repr structs
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- #repr struct codegen (NOTE: file does not exist yet)
- [ ] **AOT Tests**: No AOT coverage yet

- [x] **Spec tests** (verified 2026-03-29) -- 17+ tests in `tests/spec/types/repr_attr*.ori`
  - [x] Positive: `repr_attr.ori` (c, packed, transparent, aligned)
  - [x] Positive: `repr_attr_c_aligned.ori` (stacked c + aligned)
  - [x] Positive: `repr_attr_c_aligned_combined.ori` (combined syntax)
  - [x] Negative (compile-fail): `repr_attr_aligned_not_power_of_two.ori` (E2041)
  - [x] Negative: `repr_attr_transparent_two_fields.ori` (E2041)
  - [x] Negative: `repr_attr_transparent_zero_fields.ori` (E2041)
  - [x] Negative: `repr_attr_aligned_zero.ori` (E2041)
  - [x] Negative: `repr_attr_packed_aligned.ori` (E2041)
  - [x] Negative: `repr_attr_c_packed.ori` (E2041)
  - [x] Negative: `repr_attr_duplicate_c.ori` (E2041)
  - [x] Negative: `repr_attr_duplicate_aligned.ori` (E2041)
  - [x] Negative: `repr_attr_c_on_sum.ori` (E2041)
  - [x] Negative: `repr_attr_aligned_on_sum.ori` (E2041)
  - [x] Negative: `repr_attr_c_on_newtype.ori` (E2041)
  - [x] Negative: `repr_attr_packed_on_newtype.ori` (E2041)
  - [x] Negative: `repr_attr_aligned_on_newtype.ori` (E2041)
  - [x] Negative: `repr_attr_transparent_on_newtype.ori` (E2041)
  - [x] Fmt tests: `tests/fmt/declarations/types/repr_attr.ori`, `repr_with_target.ori`
  - [ ] Codegen verification tests -- deferred until codegen implemented

> **NOTE**: E2041 compile-fail tests serve as semantic pins -- they would break if repr validation logic is removed. Good positive + negative pairing (3 positive, 14 negative).
>
> **HYGIENE**: `ori_parse/src/grammar/attr/repr.rs` line 5 contains stale plan annotation `(TPR-01-043)` -- should be cleaned up per CLAUDE.md plan annotation rules.

---

## 11.4 Unsafe Expressions

**Spec section**: `spec/26-ffi.md § Unsafe Expressions`

### Syntax

```ori
@raw_memory_access (ptr: CPtr, offset: int) -> byte uses FFI =
    // Direct pointer arithmetic - Ori cannot verify safety
    unsafe { ptr_read_byte(ptr: ptr, offset: offset) }
```

### Semantics

Inside `unsafe`:
- Dereference raw pointers
- Pointer arithmetic
- Access mutable statics
- Transmute types

### Implementation

- [ ] **Spec**: Define unsafe block semantics
  - [ ] List of unsafe operations
  - [ ] Scoping rules
  - [ ] Interaction with FFI capability

- [x] **Parser**: Parse unsafe blocks (verified 2026-03-29) -- `parse_unsafe_expr` at `ori_parse/src/grammar/expr/primary/specials.rs:111`
  - [x] `unsafe` keyword -- parses `unsafe { block_body }` into `ExprKind::Unsafe(inner)` (verified 2026-03-29)
  - [x] Block expression (verified 2026-03-29)

- [x] **Type checker**: Transparent pass-through (verified 2026-03-29) -- `ExprKind::Unsafe(inner) => infer_expr(engine, arena, *inner)` at `ori_types/src/infer/expr/mod.rs:216`
  - [ ] Set `in_unsafe` flag -- NOT IMPLEMENTED
  - [ ] Allow unsafe operations only in context -- NOT IMPLEMENTED (no `uses Unsafe` capability enforcement)

- [x] **Evaluator**: Execute unsafe blocks (verified 2026-03-29) -- `CanExpr::Unsafe(inner) => self.eval_can(inner)` at `ori_eval/src/interpreter/can_eval/mod.rs:337`
  - [ ] Pointer dereference -- NOT IMPLEMENTED (pointer ops do not exist yet)
  - [ ] Raw memory access -- NOT IMPLEMENTED

- [x] **ARC lowering**: `CanExpr::Unsafe(inner) => self.lower_expr(inner)` at `ori_arc/src/lower/expr/mod.rs:297` (verified 2026-03-29)

- [ ] **LLVM Support**: LLVM codegen for unsafe blocks (transparent -- same as eval)
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- unsafe blocks codegen (NOTE: file does not exist yet)
- [ ] **AOT Tests**: No AOT coverage yet

- [x] **Spec tests**: `tests/spec/capabilities/unsafe_block.ori` -- 6 tests pass (verified 2026-03-29) WEAK TESTS
  - [x] test_unsafe_single_expr (verified 2026-03-29)
  - [x] test_unsafe_multi_stmt (verified 2026-03-29)
  - [x] test_unsafe_nested (verified 2026-03-29)
  - [x] test_unsafe_in_block (verified 2026-03-29)
  - [x] test_unsafe_type (str) (verified 2026-03-29)
  - [x] test_unsafe_bool (verified 2026-03-29)
  - [ ] Capability enforcement tests -- NEEDS TESTS
  - [ ] Compile-fail: unsafe ops outside unsafe block -- NEEDS TESTS
  - [ ] Interaction with closures, loops, match -- NEEDS TESTS

> **WEAK TESTS**: Current tests only verify `unsafe { expr }` preserves expression type. No tests for capability enforcement (`uses Unsafe`), no tests for operations that should require unsafe context, no interaction tests. Tests are correct for what they verify but incomplete for safety enforcement.

- [ ] **Test**: `tests/compile-fail/ffi/unsafe_required.ori`
  - [ ] Pointer deref outside unsafe
  - [ ] Unsafe op without unsafe block

---

## 11.5 FFI Capability

**Spec section**: `spec/26-ffi.md § FFI Capability`

### Design

```ori
// FFI functions require FFI capability
@call_c_function () -> int uses FFI = some_c_function()

// Callers must have capability
@main () -> void uses FFI = {
    let result = call_c_function()
    print(msg: `Result: {result}`)
}

// Or provide it explicitly in tests
@test_c_call tests @call_c_function () -> void = {
    with FFI = AllowFFI in
        assert_eq(actual: call_c_function(), expected: 42)
}
```

### Implementation

- [ ] **Spec**: FFI capability definition
  - [ ] As a marker capability (like Async)
  - [ ] Propagation rules
  - [ ] Testing patterns

- [ ] **Capability system**: Add `FFI` capability
  - [ ] Define in prelude
  - [ ] Track in function signatures
  - [ ] Propagate to callers

- [ ] **Type checker**: Enforce capability requirement
  - [ ] Require `uses FFI` for extern calls
  - [ ] Require `uses FFI` for unsafe blocks

- [ ] **LLVM Support**: LLVM codegen for FFI capability
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- FFI capability codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/ffi_capability.ori`
  - [ ] Function requiring FFI
  - [ ] Providing FFI in tests
  - [ ] Missing capability error

---

## 11.6 Callbacks (Native)

**Spec section**: `spec/26-ffi.md § Callbacks`

### Syntax

```ori
extern "c" from "libc" {
    @qsort (
        base: [byte],
        nmemb: int,
        size: int,
        compar: (CPtr, CPtr) -> int
    ) -> void
}

@compare_ints (a: CPtr, b: CPtr) -> int uses FFI = ...
qsort(base: data, nmemb: len, size: 4, compar: compare_ints)
```

### Implementation

- [ ] **Spec**: Callback semantics
  - [ ] Function pointer type syntax
  - [ ] Conversion from Ori functions
  - [ ] Lifetime considerations

- [ ] **Types**: Function pointer type
  - [ ] `(CPtr, CPtr) -> int` in type system
  - [ ] ABI specification

- [ ] **Codegen**: Generate callback wrappers
  - [ ] Trampoline functions
  - [ ] ABI adaptation

- [ ] **LLVM Support**: LLVM codegen for callbacks
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- callbacks codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/callbacks.ori`
  - [ ] Simple callback
  - [ ] qsort example
  - [ ] Callback with userdata

---

## 11.7 Build System Integration

**Spec section**: `spec/26-ffi.md § Linking`

### ori.toml Configuration

```toml
[native]
libraries = ["m", "z", "pthread"]
library_paths = ["/usr/local/lib", "./native/lib"]

[native.linux]
libraries = ["m", "rt"]

[native.macos]
libraries = ["m"]
frameworks = ["Security", "Foundation"]

[native.windows]
libraries = ["msvcrt"]
```

### Implementation

- [ ] **Spec**: Link specification
  - [ ] ori.toml native section
  - [ ] Library kinds (static, dylib, framework)
  - [ ] Search paths

- [ ] **Codegen**: Emit link directives
  - [ ] LLVM link metadata
  - [ ] Library search

- [ ] **Build system**: Handle native dependencies
  - [ ] ori.toml parsing
  - [ ] pkg-config integration

- [ ] **LLVM Support**: LLVM codegen for link directives
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- linking codegen
- [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Test**: `tests/spec/ffi/linking.ori`
  - [ ] Link to libc
  - [ ] Link to libm
  - [ ] Custom library

---

## 11.8 compile_error Built-in

> **CROSS-REFERENCE**: Section 13.10 also covers `compile_error` in the context of conditional compilation.
> Implementation should be done once and shared; avoid duplicate work.

**Spec section**: `spec/annex-c-built-in-functions.md § Compile-Time Functions`

### Syntax

```ori
#target(arch: "wasm32")
compile_error("std.fs is not available for WASM.")
```

### Implementation

- [ ] **Spec**: Define compile_error semantics
  - [ ] Compile-time error with custom message
  - [ ] Works with conditional compilation

- [ ] **Parser**: Parse compile_error
  - [ ] Built-in function syntax
  - [ ] String literal argument

- [ ] **Type checker**: Trigger compile error
  - [ ] Evaluate during type checking
  - [ ] Only if code path is active

- [ ] **Test**: `tests/compile-fail/compile_error.ori`
  - [ ] Basic compile_error
  - [ ] With conditional compilation

---

## 11.9 WASM Target (Section 2)

### JS FFI

```ori
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
}
```

### Implementation

- [ ] **Codegen**: WASM code generation
  - [ ] WASM binary output
  - [ ] Import generation

- [ ] **Glue generation**: Generate JS glue code
  - [ ] String marshalling (TextEncoder/TextDecoder)
  - [ ] Object heap slab

- [ ] **Test**: `tests/spec/ffi/js_ffi.ori`
  - [ ] Basic JS function call
  - [ ] String marshalling
  - [ ] Object handles

---

## 11.10 JsValue and Async (Section 3-4)

### JsValue Type

```ori
type JsValue = { _handle: int }

extern "js" {
    @_document_query (selector: str) -> JsValue
    @_drop_js_value (handle: JsValue) -> void
}
```

### JsPromise with Implicit Resolution

```ori
extern "js" {
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
}

// JsPromise auto-resolved at binding sites
@fetch_text (url: str) -> str uses Async, FFI =
    {
        let response = _fetch(url: url),  // auto-resolved
        text
    }
```

### Implementation

- [ ] **Types**: JsValue opaque handle type
  - [ ] Define in stdlib
  - [ ] Handle tracking

- [ ] **Types**: JsPromise<T> type
  - [ ] Compiler-recognized generic
  - [ ] Implicit resolution rules

- [ ] **Codegen**: JSPI/Asyncify integration
  - [ ] Stack switching for async
  - [ ] Promise resolution glue

- [ ] **Test**: `tests/spec/ffi/js_async.ori`
  - [ ] JsPromise implicit resolution
  - [ ] Async function with FFI

---

## 11.11 Deep FFI -- Higher-Level FFI Abstractions

**Proposal**: `proposals/approved/deep-ffi-proposal.md`

Deep FFI layers five opt-in abstractions on top of the base FFI syntax: error protocols, ownership annotations, declarative marshalling, capability-gated testability, and const-generic safety. Each is independently useful and backward-compatible.

### 11.11.1 Error Protocols + `out` Parameters (Phase 1)

- [ ] **Implement**: `#error(errno | nonzero | null | negative | success: N | none)` block/function attributes
  - [ ] **Parser**: Parse `#error(...)` on extern blocks and extern items
  - [ ] **IR**: Represent error protocol in extern block IR
  - [ ] **Type checker**: Transform return types when error protocol is active
  - [ ] **Codegen**: Generate error check + `Result` wrapping code after C calls
  - [ ] **Rust Tests**: `ori_parse/src/tests/` -- error protocol parsing
  - [ ] **Ori Tests**: `tests/spec/ffi/error_protocol.ori`
- [ ] **Implement**: `FfiError` type in `std.ffi`
  - [ ] **Library**: Define `FfiError` type in `library/std/ffi.ori`
- [ ] **Implement**: `out` parameter modifier (parser → IR → codegen)
  - [ ] **Parser**: Parse `out` modifier on extern params
  - [ ] **IR**: Represent `out` params in extern item IR
  - [ ] **Type checker**: Fold `out` params into return type
  - [ ] **Codegen**: Allocate stack slot, pass address, extract value
  - [ ] **Rust Tests**: `ori_parse/src/tests/` -- `out` param parsing
  - [ ] **Ori Tests**: `tests/spec/ffi/out_params.ori`
- [ ] **Implement**: Errno reading infrastructure
  - [ ] **Runtime**: `get_errno()` as compiler intrinsic
  - [ ] **Codegen**: Read errno after C calls when `#error(errno)` active
  - [ ] **LLVM Support**: LLVM codegen for errno reading
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- errno codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 11.11.2 Ownership Annotations (Phase 2)

- [ ] **Implement**: `owned` / `borrowed` annotations
  - [ ] **Parser**: Parse ownership annotations on extern params and return types
  - [ ] **IR**: Represent ownership in extern item IR
  - [ ] **Type checker**: Validate ownership combinations
  - [ ] **Rust Tests**: `ori_parse/src/tests/` -- ownership annotation parsing
  - [ ] **Ori Tests**: `tests/spec/ffi/ownership.ori`
- [ ] **Implement**: `#free(fn)` attribute (block-level and per-function)
  - [ ] **Parser**: Parse `#free(...)` on extern blocks and items
  - [ ] **Codegen**: Wire up cleanup function
- [ ] **Implement**: Auto-generated Drop impls for `owned CPtr` with `#free`
  - [ ] **Type checker**: Generate synthetic Drop impl for owned CPtr types
  - [ ] **Codegen**: Emit cleanup call on scope exit
  - [ ] **LLVM Support**: LLVM codegen for auto-Drop
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- ownership codegen
  - [ ] **AOT Tests**: No AOT coverage yet
- [ ] **Implement**: `str` return defaults to borrowed (copy, don't free)
  - [ ] **Ori Tests**: `tests/spec/ffi/str_return_borrowed.ori`
- [ ] **Implement**: Compiler warnings for unannotated CPtr returns

### 11.11.3 Declarative Marshalling Extensions (Phase 3)

- [ ] **Implement**: `[byte]` length elision -- adjacent `(ptr, len)` pair insertion
  - [ ] **Type checker**: Detect `[byte]` params and expand to two C args
  - [ ] **Codegen**: Insert length argument at call site
  - [ ] **Ori Tests**: `tests/spec/ffi/byte_length_elision.ori`
- [ ] **Implement**: `mut [byte]` parameter handling -- adjacent `(ptr, &len)` pair
- [ ] **Implement**: `int` ↔ `c_int` automatic narrowing/widening with bounds checks
- [ ] **Implement**: `bool` ↔ `c_int` conversion
  - [ ] **LLVM Support**: LLVM codegen for marshalling extensions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` -- marshalling codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### 11.11.4 Capability-Gated Testability (Phase 4)

- [ ] **Implement**: Compiler generates internal traits from extern blocks
  - [ ] **Type checker**: Auto-generate trait from each extern block's `from` clause
- [ ] **Implement**: Parametric `FFI("lib")` capability
  - [ ] **Type checker**: Parse and validate parametric FFI capability
  - [ ] **Backward compat**: Unparameterized `uses FFI` remains shorthand for all
- [ ] **Implement**: `with FFI("lib") = handler { ... } in` dispatch routing
  - [ ] Handler signature validation against generated traits
  - [ ] Stateless `handler { ... }` as sugar for `handler(state: ()) { ... }`
  - [ ] Fall-through to real C implementation for unmocked functions
  - [ ] **Ori Tests**: `tests/spec/ffi/mock_handler.ori`

### 11.11.5 Const-Generic Safety (Phase 5 -- Future)

- [ ] **Design**: Where clauses on extern items with const expressions
  - [ ] Depends on Section 18 (Const Generics) being complete
- [ ] **Implement**: Buffer size validation at compile time
- [ ] **Implement**: Fixed-capacity list `[T, max N]` at FFI boundary

---

## Section Completion Checklist

- [ ] All items above have checkboxes marked `[ ]`
- [ ] Spec file `spec/26-ffi.md` complete
- [ ] CLAUDE.md updated with FFI syntax
- [ ] grammar.ebnf updated with extern blocks
- [ ] Can call libc functions (strlen, malloc, free)
- [ ] Can call libm functions (sin, cos, sqrt)
- [ ] Can create and use SQLite binding
- [ ] All tests pass: `./test-all.sh`
- [ ] `uses FFI` properly enforced
- [ ] `unsafe` blocks working
- [ ] `/tpr-review` passed -- independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Can write a program that opens and queries a SQLite database

---

## Example: SQLite Binding

Target capability demonstration:

```ori
extern "c" from "sqlite3" {
    @_sqlite3_open (filename: str, ppDb: CPtr) -> int as "sqlite3_open"
    @_sqlite3_close (db: CPtr) -> int as "sqlite3_close"
    @_sqlite3_exec (
        db: CPtr,
        sql: str,
        callback: (CPtr, int, CPtr, CPtr) -> int,
        userdata: CPtr,
        errmsg: CPtr
    ) -> int as "sqlite3_exec"
}

type SqliteDb = { handle: CPtr }

impl SqliteDb {
    pub @open (path: str) -> Result<SqliteDb, str> uses FFI =
        {
            let handle = CPtr.null()
            let result = _sqlite3_open(filename: path, ppDb: handle)
            if result == 0 then
                Ok(SqliteDb { handle: handle })
            else
                Err("Failed to open database")
        }

    pub @close (self) -> void uses FFI =
        _sqlite3_close(db: self.handle)
}
```
