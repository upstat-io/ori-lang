---
title: "Foreign Function Interface"
description: "Ori Language Specification — FFI"
order: 24
section: "FFI"
---

# Foreign Function Interface

The foreign function interface (FFI) enables Ori programs to call functions implemented in other languages.

> **Grammar:** See [grammar.ebnf](https://github.com/upstat-io/ori-lang/blob/master/docs/ori_lang/0.1-alpha/spec/grammar.ebnf) § DECLARATIONS (extern_block)

## Overview

Ori supports two FFI backends:

| Backend | Syntax | Target |
|---------|--------|--------|
| Native | `extern "c"` | C ABI (LLVM targets) |
| JavaScript | `extern "js"` | WebAssembly/Browser |

All FFI calls require the `FFI` capability.

## Native FFI (C ABI)

### Declaration Syntax

```ori
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_sqrt (x: float) -> float as "sqrt"
}
```

The `from` clause specifies the library name. The `as` clause maps Ori function names to C function names.

### Library Specification

| Syntax | Meaning |
|--------|---------|
| `from "m"` | System library (libm) |
| `from "/usr/lib/libfoo.so"` | Absolute path |
| `from "./native/libcustom.so"` | Relative to project |
| `from "libc"` | Header-only/inline |

### Name Mapping

When C function names differ from desired Ori names:

```ori
extern "c" from "m" {
    @abs (value: float) -> float as "fabs"
    @ln (x: float) -> float as "log"
}
```

Without `as`, the Ori function name (without `@`) is used as the C name.

### Visibility

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

## JavaScript FFI (WASM Target)

### Declaration Syntax

```ori
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
    @_sqrt (x: float) -> float as "Math.sqrt"
    @_now () -> float as "Date.now"
}
```

### Module Imports

```ori
extern "js" from "./utils.js" {
    @_formatDate (timestamp: int) -> str as "formatDate"
}
```

### Async Functions

Async JavaScript functions return `JsPromise<T>`:

```ori
extern "js" {
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
}
```

See [JsPromise Type](#jspromise-type) for resolution semantics.

## Type Marshalling

### Primitive Types

| Ori Type | C Type | WASM Type | JS Type |
|----------|--------|-----------|---------|
| `int` | `int64_t` | `i64` | `BigInt` or `number` |
| `float` | `double` | `f64` | `number` |
| `bool` | `bool` | `i32` | `boolean` |
| `byte` | `uint8_t` | `i32` | `number` |

JavaScript `number` has 53-bit integer precision. Large `int` values use `BigInt`.

### C Type Aliases

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

### Strings

Strings are copied at FFI boundaries:

- **Native**: Ori string converted to null-terminated C string (allocated, copied)
- **WASM**: Ori string converted via TextEncoder/TextDecoder

### CPtr Type

`CPtr` represents an opaque pointer to C data structures:

```ori
extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: CPtr) -> int
    @sqlite3_close (db: CPtr) -> int
}
```

`CPtr` cannot be dereferenced in Ori code.

### Nullable Pointers

`Option<CPtr>` represents nullable pointers:

```ori
extern "c" from "foo" {
    @get_resource (id: int) -> Option<CPtr>
}
```

Returns `None` when the C function returns `NULL`.

### Byte Arrays

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

### JsValue Type

`JsValue` represents an opaque handle to a JavaScript object:

```ori
extern "js" {
    @_document_query (selector: str) -> JsValue as "document.querySelector"
    @_element_set_text (elem: JsValue, text: str) -> void
    @_drop_js_value (handle: JsValue) -> void
}
```

`JsValue` handles are reference counted in a heap slab and must be explicitly dropped.

### JsPromise Type

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

### C Structs

The `#repr` attribute controls struct memory layout. It applies only to struct types.

| Attribute | Effect |
|-----------|--------|
| `#repr("c")` | C-compatible field layout and alignment |
| `#repr("packed")` | No padding between fields |
| `#repr("transparent")` | Same layout as single field |
| `#repr("aligned", N)` | Minimum N-byte alignment (N must be power of two) |

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

### Callbacks

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

### C Variadics

C variadic functions are supported with untyped variadic parameters:

```ori
extern "c" {
    @printf (fmt: CPtr, ...) -> c_int
}
```

Calling C variadic functions requires `unsafe`.

## Unsafe Expressions

> **Proposal:** [unsafe-semantics-proposal.md](../../../proposals/approved/unsafe-semantics-proposal.md)

Operations that bypass Ori's safety guarantees require the `Unsafe` capability. The `unsafe { }` block discharges this capability locally:

```ori
@raw_memory_access (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe { ptr_read_byte(ptr: ptr, offset: offset) };
```

`Unsafe` is a _marker capability_ — it cannot be bound via `with...in` (E1203). A function that wraps unsafe operations in `unsafe { }` blocks does not propagate `Unsafe` to callers. See [Capabilities § Marker Capabilities](14-capabilities.md#marker-capabilities).

### Operations Requiring Unsafe

- Dereference raw pointers
- Pointer arithmetic
- Access mutable statics
- Transmute types
- Call C variadic functions

### Safe FFI Calls

Regular FFI calls (via `extern` declarations) are safe to call but require the `FFI` capability. Only operations Ori cannot verify require `unsafe`. The `FFI` capability tracks provenance (foreign code); the `Unsafe` capability tracks trust (safety bypasses).

## FFI Capability

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

## Compile-Time Errors

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

## Error Handling

### Native FFI Errors

C functions typically return error codes. Wrap in `Result`:

```ori
extern "c" from "libc" {
    @_open (path: str, flags: int, mode: int) -> int as "open"
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

### WASM FFI Errors

JavaScript exceptions become Ori errors:

```ori
extern "js" {
    @_json_parse (s: str) -> Result<JsValue, str> as "JSON.parse"
}
```

If JavaScript throws, the function returns `Err` with the exception message.

## Memory Management

### Native

Standard C memory management. Ori's ARC handles Ori objects; C objects follow C conventions.

### WASM

- **Linear memory**: Ori allocates from WASM linear memory
- **JS object handles**: Reference counted in a heap slab; must be explicitly dropped

```ori
@use_js_object () -> void uses FFI =
    {
        let elem = document_query(selector: "#app");
        element_set_text(elem: elem, text: "Hello");
        drop_js_value(handle: elem)
    }
```

## Platform-Specific Declarations

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
