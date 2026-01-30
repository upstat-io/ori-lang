# Proposal: Platform FFI (Native & WASM)

**Status:** Approved
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Language core, compiler, runtime, standard library
**Depends on:** None

---

## Summary

A unified foreign function interface supporting both native platforms (via C ABI) and WebAssembly (via JavaScript interop). The same Ori code can target either platform with appropriate FFI backends, enabling true cross-platform compilation including browser-based execution.

---

## Motivation

Ori must run:
1. **Native** (Linux, macOS, Windows) - calling C libraries (libm, libsodium, libc)
2. **WASM/Browser** - calling JavaScript APIs (Math, crypto.subtle, fetch)

A unified FFI design allows:
- Standard library works on both platforms
- User code can target either platform
- Platform-specific code is clearly marked
- No runtime overhead for unused FFI

### Design Principles

1. **Explicit boundaries**: FFI calls are clearly marked, not hidden
2. **Safety at the edge**: Ori validates inputs/outputs at FFI boundaries
3. **No hidden allocations**: Memory ownership is explicit
4. **Capability-aware**: FFI calls require the `FFI` capability
5. **Deterministic linking**: Libraries are declared, not implicitly discovered

---

## Prior Art Analysis

| Language | Native FFI | WASM FFI | Glue Generation |
|----------|-----------|----------|-----------------|
| **Rust** | `extern "C"` | wasm-bindgen | Automatic, tree-shaken |
| **C/C++** | Native | Emscripten | Automatic, large |
| **Go** | cgo | syscall/js | Runtime, dynamic |
| **Zig** | `extern` | `extern` | Manual |
| **AssemblyScript** | N/A | `@external` | Automatic |

**Best practices extracted:**
- wasm-bindgen's heap slab for object references
- Tree-shaken glue code generation
- Explicit async handling
- TextEncoder/TextDecoder for strings
- Attribute-based declaration syntax

---

## FFI Declaration Syntax

### Native FFI (C ABI)

```ori
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_sqrt (x: float) -> float as "sqrt"
}
```

The `from "m"` specifies the library name (`libm.so` on Linux, `libm.dylib` on macOS).

### Library Specification

```ori
// System library (searched in standard paths)
extern "c" from "z" { ... }           // libz

// Explicit path
extern "c" from "/usr/lib/libfoo.so" { ... }

// Relative to project
extern "c" from "./native/libcustom.so" { ... }

// Header-only (inline/macro, linked at compile time)
extern "c" from "libc" { ... }
```

### Name Mapping

When C function names differ from desired Ori names:

```ori
extern "c" from "m" {
    // C: double fabs(double x)
    // Ori: abs(value: float) -> float
    @abs (value: float) -> float as "fabs"

    // C: double log(double x)
    @ln (x: float) -> float as "log"
}
```

### JavaScript FFI (WASM target)

```ori
extern "js" {
    // Global namespace
    @_sin (x: float) -> float as "Math.sin"
    @_sqrt (x: float) -> float as "Math.sqrt"
    @_now () -> float as "Date.now"

    // Async functions return JsPromise
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
}

extern "js" from "./utils.js" {
    // Import from local JS module
    @_formatDate (timestamp: int) -> str as "formatDate"
}
```

### Combined Platform FFI

```ori
// std/math/ffi.ori
#target(not_arch: "wasm32")
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_sqrt (x: float) -> float as "sqrt"
}

#target(arch: "wasm32")
extern "js" {
    @_sin (x: float) -> float as "Math.sin"
    @_sqrt (x: float) -> float as "Math.sqrt"
}

// Public API - works on both platforms
pub @sin (angle: float) -> float = _sin(x: angle)
pub @sqrt (value: float) -> Result<float, MathError> =
    if value < 0.0 then
        Err(MathError.DomainError(message: "sqrt undefined for negative"))
    else
        Ok(_sqrt(x: value))
```

### Visibility

External declarations can be private (default) or public:

```ori
// Private - only usable within this module
extern "c" from "m" {
    @sin (x: float) -> float
}

// Public - re-exported for other modules
pub extern "c" from "m" {
    @sin (x: float) -> float
}
```

---

## Type Marshalling

### Primitive Types

| Ori Type | C Type | WASM Type | JS Type |
|----------|--------|-----------|---------|
| `int` | `int64_t` | `i64` | `BigInt` or `number`* |
| `float` | `double` | `f64` | `number` |
| `bool` | `bool` | `i32` | `boolean` |
| `byte` | `uint8_t` | `i32` | `number` |

*Note: JS `number` only has 53-bit integer precision. Large `int` values use `BigInt`.

### C Type Aliases

For direct C interop:

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

Strings are copied at FFI boundaries using pointer + length:

**Native (C):**
```ori
// Ori string → null-terminated C string (allocated, copied)
// C string → Ori string (copied)
```

**WASM (JS):**
```ori
// Ori string → JS string via TextDecoder
// JS string → Ori string via TextEncoder
```

**Generated glue (WASM):**
```javascript
const decoder = new TextDecoder();
const encoder = new TextEncoder();

function getStringFromWasm(ptr, len) {
    return decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len));
}

function passStringToWasm(str) {
    const bytes = encoder.encode(str);
    const ptr = wasm.exports.__ori_alloc(bytes.length);
    new Uint8Array(wasm.memory.buffer, ptr, bytes.length).set(bytes);
    return [ptr, bytes.length];
}
```

### The CPtr Type (Native)

For opaque pointers to C data structures:

```ori
// Opaque pointer - cannot be dereferenced in Ori
type CPtr

extern "c" from "sqlite3" {
    // sqlite3* sqlite3_open(const char* filename, sqlite3** ppDb)
    @sqlite3_open (filename: str, db: CPtr) -> int

    // int sqlite3_close(sqlite3* db)
    @sqlite3_close (db: CPtr) -> int
}
```

### Nullable Pointers

```ori
extern "c" from "foo" {
    // Returns NULL on failure
    @get_resource (id: int) -> Option<CPtr> as "get_resource"
}
```

### Byte Arrays

```ori
extern "c" from "z" {
    // int compress(Bytef* dest, uLongf* destLen, const Bytef* source, uLong sourceLen)
    @compress (
        dest: [byte],           // out: mutable buffer
        dest_len: int,          // in/out: buffer size / compressed size
        source: [byte],         // in: source data
        source_len: int         // in: source length
    ) -> int
}
```

Buffer semantics:
- `[byte]` as input: Pointer to data, length passed separately
- `[byte]` as output: Pre-allocated buffer, modified in place
- Bounds checking occurs at the Ori side before the call

### JsValue Type (WASM)

For complex JS objects, use opaque handles:

```ori
// Opaque handle to a JS object
type JsValue = { _handle: int }

extern "js" {
    @_document_query (selector: str) -> JsValue as "document.querySelector"
    @_element_set_text (elem: JsValue, text: str) -> void
    @_drop_js_value (handle: JsValue) -> void
}
```

**Heap slab design (from wasm-bindgen):**
```javascript
// Object heap for JS values
let heap = new Array(128).fill(undefined);
heap.push(undefined, null, true, false);  // Reserved indices 0-3
let heapNext = heap.length;

function addHeapObject(obj) {
    if (heapNext === heap.length) heap.push(heap.length + 1);
    const idx = heapNext;
    heapNext = heap[idx];
    heap[idx] = obj;
    return idx;
}

function getObject(idx) {
    return heap[idx];
}

function dropObject(idx) {
    if (idx < 132) return;  // Don't drop reserved
    heap[idx] = heapNext;
    heapNext = idx;
}
```

### C Structs with #repr

```ori
#repr("c")
type CTimeSpec = {
    tv_sec: int,
    tv_nsec: int
}

extern "c" from "libc" {
    @_clock_gettime (clock_id: int, ts: CTimeSpec) -> int as "clock_gettime"
}
```

The `#repr("c")` attribute ensures C-compatible memory layout.

### Callbacks (Native)

Ori functions can be passed to C as callbacks:

```ori
extern "c" from "libc" {
    // void qsort(void* base, size_t nmemb, size_t size,
    //            int (*compar)(const void*, const void*))
    @qsort (
        base: [byte],
        nmemb: int,
        size: int,
        compar: (CPtr, CPtr) -> int
    ) -> void
}

// Usage
@compare_ints (a: CPtr, b: CPtr) -> int = ...

qsort(
    base: data,
    nmemb: len(collection: data) / 4,
    size: 4,
    compar: compare_ints
)
```

---

## Async Handling (WASM)

### The Problem

WASM cannot block on JavaScript Promises. Calling async JS from synchronous WASM deadlocks.

### Solution: JsPromise with Implicit Resolution

```ori
// JsPromise<T> represents a JS Promise
type JsPromise<T>

extern "js" {
    // Async functions return JsPromise
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
    @_response_text (resp: JsValue) -> JsPromise<str>
}

// JsPromise is implicitly resolved at binding sites in async context
@fetch_text (url: str) -> str uses Async, FFI =
    run(
        let response = _fetch(url: url),  // JsPromise<JsValue> auto-resolved
        let text = _response_text(resp: response),  // JsPromise<str> auto-resolved
        text
    )
```

**Semantics:**
- `JsPromise<T>` is a compiler-recognized type
- When a `JsPromise<T>` is assigned to a binding or used where `T` is expected, the compiler inserts suspension/resolution
- This only occurs in functions with `uses Async` capability
- Error: assigning `JsPromise<T>` in a non-async context

**Compiler transforms at resolution points:**
1. Suspends Ori execution (saves stack)
2. Attaches `.then()` callback
3. Resumes when Promise resolves

This requires **Asyncify-style stack switching** or **JSPI (JavaScript Promise Integration)** when targeting WASM.

### JSPI (Preferred, when available)

JSPI is a WASM proposal that allows synchronous-looking code to await Promises:

```javascript
// With JSPI, the import is marked as suspending
const imports = {
    env: {
        fetch: WebAssembly.promising(async (url) => {
            const resp = await fetch(url);
            return await resp.text();
        })
    }
};
```

**Compiler flag:** `--wasm-async=jspi` (default when available) or `--wasm-async=asyncify` (fallback)

---

## Unsafe Blocks

For operations that bypass Ori's safety guarantees:

```ori
@raw_memory_access (ptr: CPtr, offset: int) -> byte uses FFI =
    unsafe {
        // Direct pointer arithmetic - Ori cannot verify safety
        ptr_read_byte(ptr: ptr, offset: offset)
    }
```

### Unsafe Operations

Inside `unsafe`:
- Dereference raw pointers
- Pointer arithmetic
- Access mutable statics
- Transmute types

### Outside Unsafe

Regular FFI calls (via `extern` declarations) are safe to call but require the `FFI` capability. Only operations Ori cannot verify require `unsafe`.

---

## Compile-Time Errors

The `compile_error` built-in triggers a compile-time error:

```ori
// std/fs/mod.ori
#target(arch: "wasm32")
compile_error("std.fs is not available for WASM. Use std.storage for browser persistence.")

#target(not_arch: "wasm32")
pub use "./read" { read, read_bytes }
pub use "./write" { write, write_bytes }
```

**Semantics:**
- `compile_error("message")` causes a compile-time error with the given message
- Evaluated during conditional compilation — only triggers if the code path is active
- Useful for platform availability errors and unsupported configurations

---

## FFI Capability

All FFI calls require the `FFI` capability:

```ori
@call_c_function () -> int uses FFI =
    some_c_function()

@manipulate_dom () -> void uses FFI =
    run(
        let elem = document_query(selector: "#app"),
        element_set_text(elem: elem, text: "Hello"),
        drop_js_value(handle: elem)
    )
```

### Standard Library Hides FFI

Users don't see the FFI capability for stdlib:

```ori
// User code - no FFI capability needed
use std.math { sin, sqrt }

@compute (x: float) -> float =
    sin(angle: x) + sqrt(value: x).unwrap_or(default: 0.0)
```

The stdlib internally uses FFI but exposes clean Ori APIs.

---

## Memory Management

### Native

Standard C memory management. Ori's ARC handles Ori objects; C objects follow C conventions.

### WASM

**Linear memory:** Ori allocates from WASM linear memory. Exports `__ori_alloc` and `__ori_free` for JS glue.

**JS object handles:** Reference counted in the heap slab. Must be explicitly dropped.

```ori
@use_js_object () -> void uses FFI =
    run(
        let elem = document_query(selector: "#app"),
        element_set_text(elem: elem, text: "Hello"),
        drop_js_value(handle: elem)  // Release handle
    )
```

**With-pattern for automatic cleanup:**

```ori
@with_js_value<T> (acquire: () -> JsValue, use: (JsValue) -> T) -> T uses FFI =
    with(
        acquire: acquire,
        use: use,
        release: v -> drop_js_value(handle: v)
    )
```

### Resource Wrappers (Native)

Pattern for wrapping C resources:

```ori
type SqliteDb = { handle: CPtr }

impl SqliteDb {
    pub @open (path: str) -> Result<SqliteDb, str> uses FFI =
        run(
            let handle = CPtr.null(),
            let result = sqlite3_open(filename: path, db: handle),
            if result == 0 then
                Ok(SqliteDb { handle: handle })
            else
                Err("Failed to open database")
        )

    pub @close (self) -> void uses FFI =
        sqlite3_close(db: self.handle)
}
```

---

## Error Handling

### Native FFI Errors

C functions typically return error codes. Wrap in Result:

```ori
extern "c" from "libc" {
    @_open (path: str, flags: int, mode: int) -> int as "open"
}

pub @open_file (path: str) -> Result<int, FileError> uses FFI =
    run(
        let fd = _open(path: path, flags: 0, mode: 0),
        if fd < 0 then
            Err(errno_to_error())
        else
            Ok(fd)
    )
```

### WASM FFI Errors

JS exceptions become Ori errors:

```ori
extern "js" {
    // If JS throws, returns Err
    @_json_parse (s: str) -> Result<JsValue, str> as "JSON.parse"
}
```

**Generated glue:**
```javascript
"JSON.parse": (ptr, len) => {
    try {
        return { ok: true, value: addHeapObject(JSON.parse(getStringFromWasm(ptr, len))) };
    } catch (e) {
        return { ok: false, error: e.message };
    }
}
```

---

## Generated Glue Code

### Native Target

No glue code needed. Compiler emits direct calls to C functions via LLVM.

### WASM Target

Compiler generates `<module>_bg.js`:

```javascript
// ori_std_math_bg.js (generated)
let wasm;

// String handling
const decoder = new TextDecoder();
const getStringFromWasm = (ptr, len) =>
    decoder.decode(new Uint8Array(wasm.memory.buffer, ptr, len));

// Heap for JS objects
let heap = new Array(128).fill(undefined);
let heapNext = 128;
const addHeapObject = (obj) => { /* ... */ };
const getObject = (idx) => heap[idx];
const dropObject = (idx) => { /* ... */ };

// Import object for WebAssembly.instantiate
export const imports = {
    env: {
        "Math.sin": (x) => Math.sin(x),
        "Math.sqrt": (x) => Math.sqrt(x),
        "Math.cos": (x) => Math.cos(x),
    },
    ori_runtime: {
        __ori_string_new: (ptr, len) => addHeapObject(getStringFromWasm(ptr, len)),
        __ori_object_drop: (idx) => dropObject(idx),
    }
};

export async function init(wasmPath) {
    const { instance } = await WebAssembly.instantiateStreaming(
        fetch(wasmPath),
        imports
    );
    wasm = instance.exports;
    return wasm;
}
```

**Tree shaking:** Only imports actually used are included in glue code.

---

## Platform Availability

### Runtime Platform Detection

```ori
pub let $is_wasm: bool = $target_arch == "wasm32"
pub let $is_browser: bool = $target_arch == "wasm32"  // For now, WASM = browser

@platform_specific_init () -> void =
    if $is_wasm then
        init_browser()
    else
        init_native()
```

---

## Standard Library Platform Mapping

| Module | Native Backend | WASM Backend |
|--------|---------------|--------------|
| `std.math` | libm | `Math.*` |
| `std.crypto` | libsodium | `crypto.subtle`, `crypto.getRandomValues` |
| `std.time` | libc (clock_gettime) | `Date`, `performance.now()` |
| `std.json` | yyjson | `JSON.parse`, `JSON.stringify` |
| `std.fs` | libc (POSIX) | **Not available** |
| `std.http` | libcurl or custom | `fetch` |
| `std.storage` | N/A | IndexedDB wrapper |

---

## Build Configuration

### ori.toml

```toml
[project]
name = "my-app"

[targets.native]
# Default native target

[targets.wasm]
arch = "wasm32"
async = "jspi"  # or "asyncify"
# Features not available in WASM
disabled_std = ["fs", "net", "process"]

[native]
libraries = ["m", "sodium"]

# Platform-specific
[native.linux]
libraries = ["m", "rt"]

[native.macos]
libraries = ["m"]
frameworks = ["Security", "Foundation"]

[native.windows]
libraries = ["msvcrt"]

[wasm]
# JS modules to include in glue
js_imports = ["./custom_bindings.js"]
```

### Compiler Flags

```bash
# Native build (default)
ori build

# WASM build
ori build --target wasm32

# WASM with specific async strategy
ori build --target wasm32 --wasm-async=asyncify
```

---

## Example: Cross-Platform HTTP

```ori
// src/http.ori

#target(not_arch: "wasm32")
use std.http { get }  // Uses libcurl

#target(arch: "wasm32")
extern "js" {
    @_fetch (url: str) -> JsPromise<JsValue> as "fetch"
    @_response_ok (resp: JsValue) -> bool
    @_response_text (resp: JsValue) -> JsPromise<str>
}

#target(arch: "wasm32")
@get (url: str) -> Result<str, HttpError> uses Async, FFI =
    run(
        let resp = _fetch(url: url),  // JsPromise auto-resolved
        if !_response_ok(resp: resp) then
            Err(HttpError.RequestFailed)
        else
            Ok(_response_text(resp: resp))  // JsPromise auto-resolved
    )

// User code works on both platforms
@fetch_data (url: str) -> Result<str, HttpError> uses Async =
    get(url: url)
```

---

## Example: Wrapping libm

```ori
// std/math/ffi.ori (internal)
extern "c" from "m" {
    @_sin (x: float) -> float as "sin"
    @_cos (x: float) -> float as "cos"
    @_tan (x: float) -> float as "tan"
    @_sqrt (x: float) -> float as "sqrt"
    @_log (x: float) -> float as "log"
    @_exp (x: float) -> float as "exp"
    @_pow (base: float, exp: float) -> float as "pow"
    @_floor (x: float) -> float as "floor"
    @_ceil (x: float) -> float as "ceil"
    @_fabs (x: float) -> float as "fabs"
    @_fmod (x: float, y: float) -> float as "fmod"
    @_atan2 (y: float, x: float) -> float as "atan2"
    @_asin (x: float) -> float as "asin"
    @_acos (x: float) -> float as "acos"
    @_sinh (x: float) -> float as "sinh"
    @_cosh (x: float) -> float as "cosh"
    @_tanh (x: float) -> float as "tanh"
}

// std/math/trig.ori (public API)
use "./ffi" { _sin, _cos, _asin, _acos }

pub @sin (angle: float) -> float = _sin(x: angle)
pub @cos (angle: float) -> float = _cos(x: angle)

pub @asin (value: float) -> Result<float, MathError> =
    if value < -1.0 || value > 1.0 then
        Err(MathError.DomainError(message: "asin domain is [-1, 1]"))
    else
        Ok(_asin(x: value))

pub @acos (value: float) -> Result<float, MathError> =
    if value < -1.0 || value > 1.0 then
        Err(MathError.DomainError(message: "acos domain is [-1, 1]"))
    else
        Ok(_acos(x: value))
```

---

## Future: WebAssembly Component Model

The Component Model and WIT (WebAssembly Interface Types) are the future of WASM interop:

```wit
// math.wit
interface math {
    sqrt: func(x: float64) -> float64
    sin: func(x: float64) -> float64
}
```

**Ori compatibility:**
- FFI declarations could be generated from WIT files
- `ori bindgen math.wit` generates Ori FFI declarations
- Future-proofs against WASM ecosystem evolution

---

## Implementation Phases

### Phase 1: Native C FFI
- `extern "c"` parsing and code generation
- LLVM backend integration
- libm, libc, libsodium bindings for stdlib
- `CPtr` type and `Option<CPtr>` for nullable pointers
- Callbacks: `(CPtr, CPtr) -> int`
- `unsafe` blocks
- `#repr("c")` attribute

### Phase 2: WASM Target
- WASM code generation
- Basic JS glue generation
- Primitive type marshalling

### Phase 3: JS FFI
- `extern "js"` parsing
- String/array marshalling
- `JsValue` object handle heap

### Phase 4: Async WASM
- `JsPromise<T>` type with implicit resolution
- JSPI or Asyncify integration
- Async capability bridging

### Phase 5: Polish
- Tree-shaking glue code
- Source maps
- WIT integration

---

## Summary

| Feature | Native | WASM |
|---------|--------|------|
| Declaration | `extern "c" from "lib"` | `extern "js"` |
| Primitives | Direct C types | Direct WASM types |
| Strings | Null-terminated copy | TextEncoder/Decoder |
| Objects | `CPtr` (opaque), `#repr("c")` structs | `JsValue` heap slab handles |
| Async | Blocking OK | `JsPromise<T>` + implicit resolution |
| Glue code | None | Generated JS |
| Capability | `uses FFI` | `uses FFI` |
| Unsafe ops | `unsafe { }` blocks | `unsafe { }` blocks |

**The key insight:** One Ori codebase, two FFI backends. Platform differences are isolated in `#target` blocks. User code stays clean and portable.

---

## Design Decisions

### Why single `FFI` capability?

A unified capability simplifies user code and is platform-agnostic. The target determines whether native or JS FFI is used. Users writing cross-platform code don't need to think about capability differences.

### Why implicit JsPromise resolution?

Ori's async model has no explicit `await` keyword — functions with `uses Async` just call other async functions normally. Implicit resolution preserves this philosophy while enabling JS async interop. The compiler handles the complexity transparently.

### Why `extern "c"` syntax?

Familiar to Rust/C++ developers, clearly indicates the calling convention, and allows future extension to `extern "c++"` or other ABIs.

### Why require explicit library names?

Implicit library discovery is a source of build reproducibility issues. Explicit names ensure builds are deterministic and errors are clear.

### Why copy strings at boundaries?

Ori strings are UTF-8, length-prefixed, and immutable. C strings are null-terminated and mutable. Copying ensures Ori's string invariants aren't violated by C code.

### Why no 32-bit integers or floats?

Ori uses 64-bit `int` and `float` exclusively for simplicity. At FFI boundaries, values are converted as needed. This may introduce overhead for APIs using 32-bit types heavily, but maintains Ori's type system simplicity.

### Why unsafe blocks?

Some FFI operations (raw pointer manipulation, unchecked array access) cannot be verified by Ori. An explicit `unsafe` block clearly marks dangerous code, documents that safety is the programmer's responsibility, and allows auditing of safety-critical sections.
