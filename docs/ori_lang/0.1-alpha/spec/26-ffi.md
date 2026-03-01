---
title: "Foreign Function Interface"
description: "Clause 26: Ori Language Specification — Foreign Function Interface"
order: 26
section: "Language"
---

# 26 Foreign function interface

The foreign function interface (FFI) enables Ori programs to call functions implemented in other languages.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § DECLARATIONS (extern_block)

## 26.1 Overview

Ori supports two FFI backends:

| Backend | Syntax | Target |
|---------|--------|--------|
| Native | `extern "c"` | C ABI (LLVM targets) |
| JavaScript | `extern "js"` | WebAssembly/Browser |

All FFI calls require the `FFI` capability.

## 26.2 Native FFI (C ABI)

### 26.2.1 Declaration syntax

```ori
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_sqrt (x: float) -> float as "sqrt"
}
```

The `from` clause specifies the library name. The `as` clause maps Ori function names to C function names.

### 26.2.2 Library specification

| Syntax | Meaning |
|--------|---------|
| `from "m"` | System library (libm) |
| `from "/usr/lib/libfoo.so"` | Absolute path |
| `from "./native/libcustom.so"` | Relative to project |
| `from "libc"` | Header-only/inline |

### 26.2.3 Name mapping

When C function names differ from desired Ori names:

```ori
extern "c" from "m" {
    @abs (value: float) -> float as "fabs"
    @ln (x: float) -> float as "log"
}
```

Without `as`, the Ori function name (without `@`) is used as the C name.

### 26.2.4 Visibility

External declarations are private by default:

```ori
// Private
extern "c" from "m" {
    @sin (x: float) -> float
}

// Public
pub extern "c" from "m" {
    @sin (x: float) -> float
}
```

## 26.3 JavaScript FFI (WASM target)

### 26.3.1 Declaration syntax

```ori
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
    @_sqrt (x: float) -> float as "Math.sqrt"
    @_now () -> float as "Date.now"
}
```

### 26.3.2 Module imports

```ori
extern "js" from "./utils.js" {
    @_formatDate (timestamp: int) -> str as "formatDate"
}
```

### 26.3.3 Async functions

Async JavaScript functions return `JsPromise<T>`:

```ori
extern "js" {
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
}
```

See [JsPromise Type](#2648-jspromise-type) for resolution semantics.

## 26.4 Type marshalling

### 26.4.1 Primitive types

| Ori Type | C Type | WASM Type | JS Type |
|----------|--------|-----------|---------|
| `int` | `int64_t` | `i64` | `BigInt` or `number` |
| `float` | `double` | `f64` | `number` |
| `bool` | `bool` | `i32` | `boolean` |
| `byte` | `uint8_t` | `i32` | `number` |

JavaScript `number` has 53-bit integer precision. Large `int` values use `BigInt`.

### 26.4.2 C type aliases

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

### 26.4.3 Strings

Strings are copied at FFI boundaries:

- **Native**: Ori string converted to null-terminated C string (allocated, copied)
- **WASM**: Ori string converted via TextEncoder/TextDecoder

### 26.4.4 CPtr type

`CPtr` represents an opaque pointer to C data structures:

```ori
extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: CPtr) -> int
    @sqlite3_close (db: CPtr) -> int
}
```

`CPtr` cannot be dereferenced in Ori code.

### 26.4.5 Nullable pointers

`Option<CPtr>` represents nullable pointers:

```ori
extern "c" from "foo" {
    @get_resource (id: int) -> Option<CPtr>
}
```

Returns `None` when the C function returns `NULL`.

### 26.4.6 Byte arrays

```ori
extern "c" from "z" {
    @compress (
        dest: [byte],
        dest_len: int,
        source: [byte],
        source_len: int
    ) -> int
}
```

- `[byte]` as input: Pointer to data, length passed separately
- `[byte]` as output: Pre-allocated buffer, modified in place
- Bounds checking occurs on the Ori side before the call

### 26.4.7 JsValue type

`JsValue` represents an opaque handle to a JavaScript object:

```ori
extern "js" {
    @_document_query (selector: str) -> JsValue as "document.querySelector"
    @_element_set_text (elem: JsValue, text: str) -> void
    @_drop_js_value (handle: JsValue) -> void
}
```

`JsValue` handles are reference counted in a heap slab and shall be explicitly dropped.

### 26.4.8 JsPromise type

`JsPromise<T>` represents a JavaScript Promise:

```ori
extern "js" {
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
    @_response_text (resp: JsValue) -> JsPromise<str>
}
```

**Implicit Resolution:**

`JsPromise<T>` is implicitly resolved at binding sites in functions with `uses Suspend`:

```ori
@fetch_text (url: str) -> str uses Suspend, FFI =
    {
        let response = _fetch(url: url),   // JsPromise<JsValue> resolved
        let text = _response_text(resp: response),  // JsPromise<str> resolved
        text
    }
```

**Semantics:**

1. When `JsPromise<T>` is assigned to a binding or used where `T` is expected, the compiler inserts suspension/resolution
2. Resolution only occurs in functions with `uses Suspend` capability
3. Assigning `JsPromise<T>` in a non-async context is a compile-time error

### 26.4.9 C structs

The `#repr` attribute controls struct memory layout. It applies only to struct types.

| Attribute | Effect |
|-----------|--------|
| `#repr("c")` | C-compatible field layout and alignment |
| `#repr("packed")` | No padding between fields |
| `#repr("transparent")` | Same layout as single field |
| `#repr("aligned", N)` | Minimum N-byte alignment (N shall be power of two) |

```ori
#repr("c")
type CTimeSpec = {
    tv_sec: c_long,
    tv_nsec: c_long
}

#repr("packed")
type PacketHeader = {
    version: byte,
    flags: byte,
    length: c_short
}

#repr("transparent")
type FileHandle = { fd: c_int }

#repr("aligned", 64)
type CacheAligned = { value: int }
```

**Combining attributes:**

`#repr("c")` may combine with `#repr("aligned", N)`. Other combinations are invalid:

```ori
// Valid
#repr("c")
#repr("aligned", 16)
type CAligned = { x: int, y: int }

// Invalid - packed and aligned are contradictory
#repr("packed")
#repr("aligned", 16)  // Error
type Invalid = { x: int }
```

**Newtypes:**

Newtypes (`type T = U`) are implicitly transparent — they have identical layout to their underlying type without requiring an explicit attribute.

```ori
extern "c" from "libc" {
    @_clock_gettime (clock_id: int, ts: CTimeSpec) -> int as "clock_gettime"
}
```

### 26.4.10 Callbacks

Ori functions can be passed to C as callbacks:

```ori
extern "c" from "libc" {
    @qsort (
        base: [byte],
        nmemb: int,
        size: int,
        compar: (CPtr, CPtr) -> int
    ) -> void
}
```

### 26.4.11 C variadics

C variadic functions are supported with untyped variadic parameters:

```ori
extern "c" {
    @printf (fmt: CPtr, ...) -> c_int
}
```

Calling C variadic functions requires `unsafe`.

## 26.5 Unsafe expressions

> **Proposal:** [unsafe-semantics-proposal.md](../../proposals/approved/unsafe-semantics-proposal.md)

Operations that bypass Ori's safety guarantees require the `Unsafe` capability. The `unsafe { }` block discharges this capability locally:

```ori
@raw_memory_access (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe { ptr_read_byte(ptr: ptr, offset: offset) };
```

`Unsafe` is a _marker capability_ — it cannot be bound via `with...in` (E1203). A function that wraps unsafe operations in `unsafe { }` blocks does not propagate `Unsafe` to callers. See [Capabilities § Marker Capabilities](20-capabilities.md#2092-marker-capabilities).

### 26.5.1 Operations requiring unsafe

- Dereference raw pointers
- Pointer arithmetic
- Access mutable statics
- Transmute types
- Call C variadic functions

### 26.5.2 Safe FFI calls

Regular FFI calls (via `extern` declarations) are safe to call but require the `FFI` capability. Only operations Ori cannot verify require `unsafe`. The `FFI` capability tracks provenance (foreign code); the `Unsafe` capability tracks trust (safety bypasses).

## 26.6 FFI capability

All FFI calls require the `FFI` capability:

```ori
@call_c_function () -> int uses FFI =
    some_c_function();

@manipulate_dom () -> void uses FFI =
    {
        let elem = document_query(selector: "#app");
        element_set_text(elem: elem, text: "Hello");
        drop_js_value(handle: elem)
    }
```

Standard library functions internally use FFI but expose clean Ori APIs without requiring the `FFI` capability from callers.

## 26.7 Compile-time errors

The `compile_error` built-in triggers a compile-time error:

```ori
#target(arch: "wasm32")
compile_error("std.fs is not available for WASM");

#target(not_arch: "wasm32")
pub use "./read" { read, read_bytes };
```

**Semantics:**

- Evaluated during conditional compilation
- Only triggers if the code path is active
- Useful for platform availability errors

## 26.8 Error handling

### 26.8.1 Native FFI errors

C functions typically return error codes. Deep FFI provides declarative error protocols that automate error checking and `Result` wrapping:

```ori
// Declarative: #error(errno) generates Result wrapping automatically
extern "c" from "libc" #error(errno) {
    @open (path: str, flags: c_int, mode: c_int) -> c_int as "open"
}

// Effective Ori signature: @open (...) -> Result<int, FfiError> uses FFI("libc")
let fd = open(path: "/etc/hostname", flags: O_RDONLY, mode: 0)?
```

Available error protocols: `#error(errno)`, `#error(nonzero)`, `#error(null)`, `#error(negative)`, `#error(success: N)`, `#error(none)`. See [Deep FFI proposal](../../proposals/approved/deep-ffi-proposal.md) for full specification.

Manual wrapping remains supported for custom error handling:

```ori
extern "c" from "libc" {
    @_open (path: str, flags: c_int, mode: c_int) -> c_int as "open"
}

pub @open_file (path: str) -> Result<int, FileError> uses FFI =
    {
        let fd = _open(path: path, flags: 0, mode: 0);
        if fd < 0 then
            Err(errno_to_error())
        else
            Ok(fd)
    }
```

### 26.8.2 WASM FFI errors

JavaScript exceptions become Ori errors:

```ori
extern "js" {
    @_json_parse (s: str) -> Result<JsValue, str> as "JSON.parse"
}
```

If JavaScript throws, the function returns `Err` with the exception message.

## 26.9 Memory management

### 26.9.1 Native

Ori's ARC handles Ori objects. For C objects, Deep FFI provides ownership annotations that integrate with ARC:

- `owned CPtr` returns with `#free(fn)`: The compiler generates a `Drop` impl that calls the specified cleanup function. The value is tracked by ARC like any Ori value.
- `borrowed CPtr` returns: Ori does not free the pointer. For `borrowed str`, Ori copies the string immediately.
- Unannotated `CPtr`: Follows C conventions (untracked). Deep FFI phases enforcement from optional to warning to error.

See [Deep FFI proposal](../../proposals/approved/deep-ffi-proposal.md) for the full ownership model.

### 26.9.2 WASM

- **Linear memory**: Ori allocates from WASM linear memory
- **JS object handles**: Reference counted in a heap slab; shall be explicitly dropped

```ori
@use_js_object () -> void uses FFI =
    {
        let elem = document_query(selector: "#app");
        element_set_text(elem: elem, text: "Hello");
        drop_js_value(handle: elem)
    }
```

## 26.10 Platform-specific declarations

Use conditional compilation to provide platform-specific FFI:

```ori
#target(not_arch: "wasm32")
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
}

#target(arch: "wasm32")
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
}

// Public API works on both platforms
pub @sin (angle: float) -> float = _sin(x: angle);
```

See [Conditional Compilation](25-conditional-compilation.md) for attribute syntax.

## 26.11 Deep FFI extensions

> **Proposal:** [deep-ffi-proposal.md](../../proposals/approved/deep-ffi-proposal.md)

Deep FFI is a set of opt-in annotations that layer on top of the extern declaration syntax. All existing FFI code continues to work unchanged.

### 26.11.1 Parameter modifiers

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § FFI (`param_modifier`)

Two modifiers for extern parameters:

| Modifier | Meaning | Compiler Action |
|----------|---------|-----------------|
| `out` | C writes to this address; value folded into return type | Allocate stack slot, pass address, extract value after call |
| `mut` | C may modify this buffer in place | Pass pointer to existing buffer |

```ori
extern "c" from "sqlite3" #error(nonzero) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
}

// Effective Ori signature:
// @sqlite3_open (filename: str) -> Result<CPtr, FfiError> uses FFI("sqlite3")
```

### 26.11.2 Ownership annotations

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § FFI (`ownership`)

Two keywords — `owned` and `borrowed` — specify memory ownership transfer at the FFI boundary.

| Annotation | On Return Type | On Parameter |
|------------|---------------|--------------|
| `owned` | Ori takes ownership; cleanup via `#free` | Ori transfers ownership to C |
| `borrowed` | Ori copies immediately (str, [byte]) or non-owning view (CPtr) | C borrows temporarily |
| _(none)_ | **str: borrowed** (safe default). Primitives: by value. CPtr: warning → error | Primitives: by value. CPtr: borrowed by default |

The `#free(fn)` attribute specifies a cleanup function for `owned CPtr` returns. The compiler generates a `Drop` impl that calls this function when the value goes out of scope:

```ori
extern "c" from "sqlite3" #free(sqlite3_close) #error(nonzero) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
    @sqlite3_close (db: owned CPtr) -> c_int
}
```

### 26.11.3 Error protocol mapping

A block-level `#error(...)` attribute specifies how C return values map to `Result<T, FfiError>`:

| Protocol | Attribute | Success Condition |
|----------|-----------|-------------------|
| POSIX errno | `#error(errno)` | Return >= 0 |
| Non-zero is error | `#error(nonzero)` | Return = 0 |
| Negative is error | `#error(negative)` | Return >= 0 |
| NULL is error | `#error(null)` | Return != NULL |
| Specific success | `#error(success: N)` | Return = N |
| No protocol | `#error(none)` | (no check) |

Per-function `#error(...)` overrides the block default. `FfiError` is defined in `std.ffi`.

### 26.11.4 Parametric FFI capability

`FFI` is a _parametric capability_. Each extern block's `from "..."` clause defines a distinct capability namespace:

```ori
@query (path: str) -> Result<Row, FfiError> uses FFI("sqlite3") = ...
```

Unparameterized `uses FFI` is shorthand for all FFI capabilities (backward compatible).

### 26.11.5 Capability-gated testability

Each extern block generates a compiler-internal trait. The `with FFI("lib") = handler { ... } in` construct redirects extern calls to mock implementations:

```ori
@test tests query {
    with FFI("sqlite3") = handler {
        sqlite3_open: (filename: str) -> Result<CPtr, FfiError> = Ok(CPtr.null()),
    } in {
        test_database_logic()
    }
}
```

The stateless `handler { ... }` form (without state) is syntactic sugar for `handler(state: ()) { ... }` from the stateful-mock-testing system. Unmocked functions fall through to the real C implementation.
