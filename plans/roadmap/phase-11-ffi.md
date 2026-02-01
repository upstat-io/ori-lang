---
phase: 11
title: Foreign Function Interface (FFI)
status: not-started
tier: 4
goal: Enable Ori to call C libraries, system APIs, and JavaScript APIs (WASM target)
spec:
  - spec/23-ffi.md
sections:
  - id: "11.1"
    title: Extern Block Syntax
    status: not-started
  - id: "11.2"
    title: C ABI Types
    status: not-started
  - id: "11.3"
    title: "#repr Attribute"
    status: not-started
  - id: "11.4"
    title: Unsafe Blocks
    status: not-started
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
  - id: "11.9"
    title: WASM Target (Phase 2)
    status: not-started
  - id: "11.10"
    title: JsValue and Async (Phase 3-4)
    status: not-started
---

# Phase 11: Foreign Function Interface (FFI)

**Goal**: Enable Ori to call C libraries, system APIs, and JavaScript APIs (WASM target)

**Criticality**: **CRITICAL** — Without FFI, Ori cannot integrate with the software ecosystem

**Proposal**: `proposals/approved/platform-ffi-proposal.md`

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
| Unsafe operations? | `unsafe { }` blocks | Explicit marking for unverifiable ops |

---

## 11.1 Extern Block Syntax

**Spec section**: `spec/23-ffi.md § Extern Blocks`

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

- [ ] **Spec**: Add `spec/23-ffi.md` with extern block syntax
  - [ ] Define extern block grammar
  - [ ] Define calling conventions ("c", "js")
  - [ ] Define linkage semantics

- [ ] **Lexer**: Add tokens
  - [ ] `extern` keyword
  - [ ] String literals for ABI ("c", "js")

- [ ] **Parser**: Parse extern blocks
  - [ ] `parse_extern_block()` in parser
  - [ ] Add `ExternBlock` to AST
  - [ ] Add `ExternItem` variants
  - [ ] `from "lib"` library specification
  - [ ] `as "name"` name mapping

- [ ] **Type checker**: Validate extern declarations
  - [ ] Ensure types are FFI-safe
  - [ ] Check for `uses FFI` in callers

- [ ] **Codegen**: Generate external references
  - [ ] Emit LLVM `declare` for C functions
  - [ ] Handle calling convention
  - [ ] Link external symbols

- [ ] **LLVM Support**: LLVM codegen for extern blocks
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — extern blocks codegen

- [ ] **Test**: `tests/spec/ffi/extern_blocks.ori`
  - [ ] Basic extern function declaration
  - [ ] Multiple functions in one block
  - [ ] Name mapping with `as`
  - [ ] Library specification with `from`

---

## 11.2 C ABI Types

**Spec section**: `spec/23-ffi.md § C Types`

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
  - [ ] Add C type aliases (`c_int`, `c_long`, etc.)
  - [ ] Size/alignment handling
  - [ ] Platform-dependent sizes

- [ ] **Type checker**: FFI type validation
  - [ ] Warn on non-FFI-safe types
  - [ ] Validate CPtr usage

- [ ] **LLVM Support**: LLVM codegen for C ABI types
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — C ABI types codegen

- [ ] **Test**: `tests/spec/ffi/c_types.ori`
  - [ ] All primitive C types
  - [ ] CPtr operations
  - [ ] Option<CPtr> for nullable

---

## 11.3 #repr Attribute

**Spec section**: `spec/23-ffi.md § Struct Layout`

### Syntax

```ori
#repr("c")
type CTimeSpec = {
    tv_sec: int,
    tv_nsec: int
}
```

### Implementation

- [ ] **Spec**: Define `#repr` attribute semantics
  - [ ] `"c"` — C ABI compatible layout
  - [ ] Future: `"packed"`, `"aligned(N)"`

- [ ] **Parser**: Parse `#repr` attribute
  - [ ] Add to attribute handling
  - [ ] Validate string argument

- [ ] **Type checker**: Validate #repr usage
  - [ ] Only valid on struct types
  - [ ] Validate field types are FFI-compatible

- [ ] **Codegen**: Generate C-compatible layout
  - [ ] LLVM struct type with correct alignment
  - [ ] No padding optimization

- [ ] **LLVM Support**: LLVM codegen for #repr structs
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — #repr struct codegen

- [ ] **Test**: `tests/spec/ffi/repr_c.ori`
  - [ ] Basic #repr("c") struct
  - [ ] Nested #repr structs
  - [ ] Invalid #repr (compile error)

---

## 11.4 Unsafe Blocks

**Spec section**: `spec/23-ffi.md § Unsafe Blocks`

### Syntax

```ori
@raw_memory_access (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe {
        // Direct pointer arithmetic - Ori cannot verify safety
        ptr_read_byte(ptr: ptr, offset: offset)
    }
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

- [ ] **Parser**: Parse unsafe blocks
  - [ ] `unsafe` keyword
  - [ ] Block expression

- [ ] **Type checker**: Track unsafe context
  - [ ] Set `in_unsafe` flag
  - [ ] Allow unsafe operations only in context

- [ ] **Evaluator**: Execute unsafe operations
  - [ ] Pointer dereference
  - [ ] Raw memory access

- [ ] **LLVM Support**: LLVM codegen for unsafe blocks
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — unsafe blocks codegen

- [ ] **Test**: `tests/spec/ffi/unsafe_blocks.ori`
  - [ ] Basic unsafe block
  - [ ] Nested unsafe
  - [ ] Unsafe operations outside block (compile error)

- [ ] **Test**: `tests/compile-fail/ffi/unsafe_required.ori`
  - [ ] Pointer deref outside unsafe
  - [ ] Unsafe op without unsafe block

---

## 11.5 FFI Capability

**Spec section**: `spec/23-ffi.md § FFI Capability`

### Design

```ori
// FFI functions require FFI capability
@call_c_function () -> int uses FFI = some_c_function()

// Callers must have capability
@main () -> void uses FFI = run(
    let result = call_c_function(),
    print(msg: `Result: {result}`),
)

// Or provide it explicitly in tests
@test_c_call tests @call_c_function () -> void = run(
    with FFI = AllowFFI in
        assert_eq(actual: call_c_function(), expected: 42),
)
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
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — FFI capability codegen

- [ ] **Test**: `tests/spec/ffi/ffi_capability.ori`
  - [ ] Function requiring FFI
  - [ ] Providing FFI in tests
  - [ ] Missing capability error

---

## 11.6 Callbacks (Native)

**Spec section**: `spec/23-ffi.md § Callbacks`

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
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — callbacks codegen

- [ ] **Test**: `tests/spec/ffi/callbacks.ori`
  - [ ] Simple callback
  - [ ] qsort example
  - [ ] Callback with userdata

---

## 11.7 Build System Integration

**Spec section**: `spec/23-ffi.md § Linking`

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
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/ffi_tests.rs` — linking codegen

- [ ] **Test**: `tests/spec/ffi/linking.ori`
  - [ ] Link to libc
  - [ ] Link to libm
  - [ ] Custom library

---

## 11.8 compile_error Built-in

**Spec section**: `spec/11-built-in-functions.md § Compile-Time Functions`

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

## 11.9 WASM Target (Phase 2)

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

## 11.10 JsValue and Async (Phase 3-4)

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
    run(
        let response = _fetch(url: url),  // auto-resolved
        text
    )
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

## Phase Completion Checklist

- [ ] All items above have checkboxes marked `[x]`
- [ ] Spec file `spec/23-ffi.md` complete
- [ ] CLAUDE.md updated with FFI syntax
- [ ] grammar.ebnf updated with extern blocks
- [ ] Can call libc functions (strlen, malloc, free)
- [ ] Can call libm functions (sin, cos, sqrt)
- [ ] Can create and use SQLite binding
- [ ] All tests pass: `./test-all`
- [ ] `uses FFI` properly enforced
- [ ] `unsafe` blocks working

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
        run(
            let handle = CPtr.null(),
            let result = _sqlite3_open(filename: path, ppDb: handle),
            if result == 0 then
                Ok(SqliteDb { handle: handle })
            else
                Err("Failed to open database")
        )

    pub @close (self) -> void uses FFI =
        _sqlite3_close(db: self.handle)
}
```
