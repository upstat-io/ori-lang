# Proposal: Deep FFI — Higher-Level Foreign Function Interface

**Status:** Draft
**Author:** Eric
**Created:** 2026-02-21
**Affects:** Language core (parser, IR, type checker), compiler (codegen), standard library
**Depends on:** Platform FFI (approved), Unsafe Semantics (approved)
**Extends:** `spec/24-ffi.md` — all existing FFI syntax remains valid

---

## Summary

Deep FFI is a set of opt-in annotations and compiler features that layer on top of the existing `extern` declaration syntax. Five abstractions — declarative marshalling, ownership annotations, error protocol mapping, capability-gated testability, and const-generic boundary safety — each independently useful, each progressively higher-level. No existing FFI code breaks. The goal: library authors write annotated extern blocks once; consumers see clean, safe, idiomatic Ori APIs with no FFI awareness.

---

## Motivation

### The Boilerplate Problem

Every stdlib FFI module follows the same pattern:

1. Declare raw extern functions with C types
2. Write an Ori wrapper that converts types
3. Check error codes and map to `Result`
4. Manage ownership (remember to free, on every exit path)

The crypto proposal has ~300 lines of extern declarations and ~600 lines of wrapper code. The wrappers are 2x the declarations — and they are almost entirely mechanical. Every `open()` call checks `< 0` and reads errno. Every `create_*()` call pairs with a `destroy_*()` on every exit path.

```ori
// Today: 12 lines for one safe wrapper
extern "c" from "libc" {
    @_open (path: str, flags: c_int, mode: c_int) -> c_int as "open"
}

pub @open_file (path: str, flags: c_int) -> Result<FileDescriptor, FfiError> uses FFI =
    {
        let fd = _open(path: path, flags: flags, mode: 0);
        if fd < 0 then
            Err(FfiError { code: get_errno(), message: strerror(get_errno()), source: "libc" })
        else
            Ok(FileDescriptor { fd: fd })
    }
```

```ori
// Deep FFI: 3 lines — the compiler generates the rest
extern "c" from "libc" #error(errno) {
    @open_file (path: str, flags: c_int, mode: c_int) -> c_int as "open"
}
```

### The Safety Gap

The current spec says "C objects follow C conventions" (section 24, Memory Management). Ownership is untracked. Every `CPtr` returned from C is a potential leak. The `sign_rsa` function in the crypto proposal has 8 potential exit paths — miss one cleanup call and you leak. A language that tracks every Ori object via ARC but abandons tracking at the FFI boundary has a safety gap proportional to FFI usage.

### The Testability Gap

The `FFI` capability exists and is bindable (not a marker). `with FFI = mock in { ... }` is syntactically valid today. But there is no infrastructure for what the mock provides or how extern calls are redirected. You cannot unit-test `open_file` without a real filesystem. You cannot test crypto wrappers without libsodium installed. Every other capability (Http, FileSystem) can be mocked via `with...in` handlers — FFI cannot.

### What Other Languages Get Wrong

| Language | Declaration | Marshalling | Ownership | Errors | Testability |
|----------|------------|-------------|-----------|--------|-------------|
| **Rust** | `extern "C" {}` | Manual (`CString`, `as_ptr`) | Untracked (`*mut`) | Manual errno | Link-time tricks |
| **Zig** | `@cImport` | Automatic but raw | Untracked | Manual | None |
| **Go** | Comment preamble | `C.CString()` everywhere | GC + pinning | Auto errno (one bright spot) | None |
| **Swift** | Auto-bridging | Automatic | Bridge keywords | Manual | None |

Go captures errno automatically — that is the single bright spot across all FFI designs. Everything else is manual, error-prone, and untestable.

---

## Design

Deep FFI introduces five abstractions that layer on top of existing extern syntax. Each is independently useful and opt-in.

### 1. Declarative Type Marshalling

Ori types in extern declarations trigger automatic compiler-generated conversion code.

#### Parameter Modifiers

Two new modifiers for extern parameters:

```ori
extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: out CPtr) -> c_int as "sqlite3_open"
}
```

| Modifier | Meaning | Compiler Action |
|----------|---------|-----------------|
| `out` | C writes to this address; value becomes part of return | Allocate stack slot, pass address, extract value after call |
| `mut` | C may modify this buffer in place | Pass pointer to existing buffer, mark as mutated after call |

#### Automatic Marshalling Table

When Ori types appear in extern declarations, the compiler generates marshalling code:

| Ori Type in Extern | C ABI Translation | Generated Code |
|--------------------|-------------------|----------------|
| `str` (input) | `const char*` | Allocate null-terminated copy, pass pointer, free after return |
| `str` (return) | `const char*` | Copy into Ori string, do not free (C owns the pointer) |
| `[byte]` (input) | `const uint8_t*, size_t` | Pass data pointer + length as two C arguments |
| `mut [byte]` | `uint8_t*` | Pass pointer to existing buffer |
| `int` → `c_int` | `int32_t` | Narrow with bounds check (panic on overflow) |
| `float` → `c_float` | `float` | Narrow (precision loss is silent, matches C semantics) |
| `bool` | `int` | Convert `true`→1, `false`→0; reverse on return |
| `out CPtr` | `void**` | Allocate stack slot, pass `&slot`, return `slot` value |
| `out T` (any #repr("c") type) | `T*` | Allocate stack space, pass address, return value |

#### `out` Parameter Semantics

`out` parameters are removed from the caller's signature and folded into the return:

```ori
extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: out CPtr) -> c_int
}

// Caller sees:
let (status, db) = sqlite3_open(filename: "test.db")

// With error protocol (see §3), caller sees:
let db = sqlite3_open(filename: "test.db")?
```

If a function has one `out` parameter and an error protocol is active, the `out` value becomes the success payload of the `Result`:

```ori
// Declaration:
extern "c" from "sqlite3" #error(nonzero) {
    @sqlite3_open (filename: str, db: out CPtr) -> c_int
}

// Effective Ori signature:
@sqlite3_open (filename: str) -> Result<CPtr, FfiError> uses FFI
```

Multiple `out` parameters become a tuple:

```ori
extern "c" from "mylib" #error(nonzero) {
    @get_size (handle: CPtr, width: out c_int, height: out c_int) -> c_int
}

// Effective Ori signature:
@get_size (handle: CPtr) -> Result<(int, int), FfiError> uses FFI
```

#### `[byte]` Length Parameter Elision

When `[byte]` appears in an extern declaration, the compiler generates the length argument automatically. The extern declaration does not include a separate length parameter — the compiler inserts it at the C ABI level:

```ori
// Deep FFI:
extern "c" from "z" {
    @compress (dest: mut [byte], source: [byte]) -> c_int
}

// C function actually called:
// int compress(uint8_t* dest, size_t* destLen, const uint8_t* source, size_t sourceLen)
```

When the C function's argument order does not match the default `(ptr, len)` pair layout, the `as` clause maps to the C name and the compiler matches by parameter name:

```ori
extern "c" from "z" {
    @compress (
        dest: mut [byte],
        source: [byte],
    ) -> c_int as "compress"
}
```

If the compiler cannot automatically determine the length argument pairing (e.g., unusual C signature), fall back to explicit C types:

```ori
// Explicit fallback — no auto-marshalling
extern "c" from "unusual_lib" {
    @weird_func (data: CPtr, len: c_size) -> c_int
}
```

#### String Return Marshalling

Strings returned from C have ambiguous ownership. The `borrowed` annotation (see §2) resolves this:

```ori
extern "c" from "sqlite3" {
    @sqlite3_errmsg (db: CPtr) -> borrowed str  // C owns the string; Ori copies immediately
}
```

Without `borrowed`, a `str` return means Ori takes ownership of the null-terminated C string (and frees it). This matches the principle: explicit ownership at the boundary.

### 2. Ownership Annotations

Two keywords — `owned` and `borrowed` — specify memory ownership transfer at the FFI boundary.

#### Syntax

```ebnf
extern_param  = [ param_modifier ] identifier ":" [ ownership ] type .
extern_item   = "@" identifier extern_params "->" [ ownership ] type
                [ "as" string_literal ]
                [ "#" identifier "(" { attribute_arg } ")" ]
                [ where_clause ] .
ownership     = "owned" | "borrowed" .
param_modifier = "out" | "mut" .
```

#### Semantics

| Annotation | On Return Type | On Parameter |
|------------|---------------|--------------|
| `owned` | Ori takes ownership; cleanup via `#free` | Ori transfers ownership to C; drops its reference |
| `borrowed` | Ori copies immediately (str, [byte]) or creates non-owning view (CPtr) | C borrows temporarily; must not store past call return |
| _(none)_ | Primitives: pass by value. CPtr: compile warning → error (ambiguous) | Primitives: pass by value. CPtr: borrowed by default |

#### The `#free` Annotation

`owned CPtr` returns require a cleanup function. Specified at block level or per function:

```ori
// Block-level default: all owned CPtr returns freed with sqlite3_close
extern "c" from "sqlite3" #free(sqlite3_close) #error(nonzero) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
    @sqlite3_close (db: owned CPtr) -> c_int  // this IS the free function
}
```

```ori
// Per-function override
extern "c" from "openssl" {
    @RSA_new () -> owned CPtr #free(RSA_free)
    @EVP_CIPHER_CTX_new () -> owned CPtr #free(EVP_CIPHER_CTX_free)
    @RSA_free (rsa: owned CPtr) -> void
    @EVP_CIPHER_CTX_free (ctx: owned CPtr) -> void
}
```

#### Integration with ARC and Drop

When a function returns `owned CPtr` with a `#free` function, the compiler generates an opaque wrapper type with a `Drop` impl:

```ori
// Conceptual expansion (not user-visible):
type __Owned_sqlite3_db = { ptr: CPtr }

impl Drop for __Owned_sqlite3_db {
    @drop (self) -> void uses FFI = sqlite3_close(db: self.ptr)
}
```

The user sees an opaque value that automatically cleans up when it goes out of scope, just like any Ori value. No manual `close()` on every exit path. No leaks on early returns. ARC handles the rest.

```ori
// Before Deep FFI:
pub @query (path: str, sql: str) -> Result<[Row], DbError> uses FFI = {
    let db = sqlite3_open(filename: path)?
    let result = sqlite3_exec(db: db, sql: sql)
    sqlite3_close(db: db)   // Must remember this!
    result                   // And what if sqlite3_exec panics? Leak.
}

// After Deep FFI:
pub @query (path: str, sql: str) -> Result<[Row], DbError> uses FFI = {
    let db = sqlite3_open(filename: path)?   // owned CPtr, auto-freed on scope exit
    sqlite3_exec(db: db, sql: sql)           // if this fails, db is still freed
}
```

#### Phased Enforcement

- **Phase 1:** Ownership annotations are optional. No warnings.
- **Phase 2:** Unannotated `CPtr` returns produce a compiler warning.
- **Phase 3:** Unannotated `CPtr` returns are a compile error.

This gives library authors time to annotate while moving toward full safety.

### 3. Error Protocol Mapping

A block-level `#error(...)` attribute specifies how C return values map to `Result<T, FfiError>`.

#### Syntax

```ori
// POSIX: negative return → read errno
extern "c" from "libc" #error(errno) {
    @open (path: str, flags: c_int, mode: c_int) -> c_int as "open"
    @read (fd: c_int, buf: mut [byte]) -> c_int as "read"
    @strerror (errnum: c_int) -> borrowed str #error(none)  // opt out
}

// SQLite: non-zero return → error code
extern "c" from "sqlite3" #error(nonzero) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
    @sqlite3_exec (db: CPtr, sql: str) -> c_int
    @sqlite3_errmsg (db: CPtr) -> borrowed str #error(none)
}

// Specific success value
extern "c" from "libfoo" #error(success: 0) {
    @foo_init () -> c_int
}

// NULL return → error
extern "c" from "mylib" #error(null) {
    @create_thing () -> CPtr
}
```

#### Error Protocol Variants

| Protocol | Attribute | Success Condition | Failure Action |
|----------|-----------|-------------------|----------------|
| POSIX errno | `#error(errno)` | Return ≥ 0 | Read `errno`, `strerror()` for message |
| Non-zero is error | `#error(nonzero)` | Return = 0 | Return value is error code |
| Negative is error | `#error(negative)` | Return ≥ 0 | Return value is error code |
| NULL is error | `#error(null)` | Return ≠ NULL | Read `errno` if available |
| Specific success | `#error(success: N)` | Return = N | Return value is error code |
| No protocol | `#error(none)` | (no check) | (raw return value) |

Per-function `#error(...)` overrides the block default. `#error(none)` opts out for functions that don't follow the library's convention (e.g., `strerror` returns a string, not an error code).

#### FfiError Type

```ori
// Defined in std.ffi (or prelude)
type FfiError = {
    code: int,
    message: str,
    source: str,      // library name from `from` clause
}
```

The `message` field is populated from:
- `strerror(errno)` for `#error(errno)` protocol
- Custom lookup if `#error_codes({...})` is provided (future extension)
- `"FFI error code: {code}"` as fallback

#### Return Type Transformation

When `#error(...)` is active, the Ori-visible return type changes:

| C Return | Protocol | Ori Return Type |
|----------|----------|-----------------|
| `c_int` | `#error(errno)` | `Result<int, FfiError>` |
| `c_int` with `out CPtr` | `#error(nonzero)` | `Result<CPtr, FfiError>` |
| `CPtr` | `#error(null)` | `Result<CPtr, FfiError>` |
| `void` | `#error(nonzero)` | `Result<void, FfiError>` |
| any | `#error(none)` | (unchanged) |

When error protocol is active AND there are `out` parameters, the `out` values become the success payload and the raw return value is consumed by the error check.

### 4. Capability-Gated Testability

The `FFI` capability is already bindable. Deep FFI adds mock infrastructure so that `with FFI = mock in { ... }` actually works.

#### The Problem

```ori
// This Ori function requires a real C library to test
pub @hash_password (password: str) -> str uses FFI = {
    let salt = crypto_random_bytes(count: 16)
    argon2_hash(password: password, salt: salt)
}
```

Testing requires libsodium installed, linked, and available. This makes tests slow, non-deterministic, and non-portable.

#### The Solution: Handler-Based Mocking

Ori's capability system already supports handler binding via `with Cap = handler(...) in`. Deep FFI extends this to extern functions:

```ori
@test tests hash_password {
    with FFI = handler {
        crypto_random_bytes: (count: int) -> [byte] =
            [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
        argon2_hash: (password: str, salt: [byte]) -> str =
            "mocked_hash_{password}",
    } in {
        let result = hash_password(password: "secret")
        assert_eq(result, "mocked_hash_secret")
    }
}
```

#### Dispatch Semantics

When `with FFI = handler { ... } in { body }`:

1. Within `body`, calls to extern functions listed in the handler are redirected to the mock implementations
2. Extern functions NOT listed in the handler fall through to the real C implementation
3. The handler functions must match the Ori-visible signature (after marshalling and error protocol transformation)
4. Type checking verifies handler function signatures against the extern declarations

#### Selective Mocking

Only mock specific extern blocks, not all FFI:

```ori
with FFI("sqlite3") = handler {
    sqlite3_open: (filename: str) -> Result<CPtr, FfiError> = Ok(CPtr.null()),
} in {
    // sqlite3 calls are mocked
    // libm calls (sin, cos, etc.) still go to real C
    test_database_logic()
}
```

The string argument to `FFI(...)` matches the `from` clause in the extern declaration.

#### Future: Auto-Generated Mock Traits

In a future phase, the compiler could auto-generate a trait from each extern block:

```ori
// Compiler-generated (not user-visible):
trait __FFI_sqlite3 {
    @sqlite3_open (filename: str) -> Result<CPtr, FfiError>
    @sqlite3_close (db: owned CPtr) -> Result<void, FfiError>
    @sqlite3_exec (db: CPtr, sql: str) -> Result<void, FfiError>
}
```

This enables IDE autocompletion for mock implementations and compile-time verification that mocks are complete.

### 5. Const-Generic Safety at Boundaries

**Depends on:** Const Generics (Section 18) — **deferred to future phase.**

With const generics, Ori can verify buffer sizes at compile time:

```ori
extern "c" from "openssl" {
    @SHA256 (
        data: [byte],
        digest: mut [byte, max $N],
    ) -> CPtr
    where N >= 32
}
```

The `where` clause enforces at compile time that the caller passes a buffer of sufficient size. The compiler rejects `SHA256(data: input, digest: small_buffer)` if `small_buffer` has capacity < 32.

This feature depends on:
- Const generic type parameters being fully implemented
- Fixed-capacity lists (`[T, max N]`) being functional
- Const expressions in where clauses being evaluated by the type checker

**Design direction documented here; implementation deferred to post-const-generics.**

---

## Interaction with Existing FFI

Every existing extern declaration continues to work unchanged:

| Existing FFI Feature | Deep FFI Impact |
|----------------------|-----------------|
| `extern "c" from "lib" { ... }` | Unchanged — new features are opt-in annotations |
| `str` params | Already auto-marshalled per spec §24 (no change) |
| `CPtr` params/returns | Unchanged without annotations; warnings in Phase 2 |
| `[byte]` params | Unchanged; explicit length still works alongside new auto-elision |
| `Option<CPtr>` for nullable | Unchanged |
| C type aliases (`c_int`, etc.) | Unchanged |
| `#repr("c")` structs | Unchanged |
| Callbacks `(CPtr, CPtr) -> int` | Unchanged |
| `unsafe { }` for variadics | Unchanged |
| `uses FFI` capability | Unchanged — mock infrastructure adds to it, doesn't change it |

**New features are additive.** Nothing in the existing grammar or spec is modified — only extended.

---

## Grammar Changes

Additive changes to `grammar.ebnf`:

```ebnf
(* Updated extern_param — adds optional modifier and ownership *)
extern_param    = [ param_modifier ] identifier ":" [ ownership ] type .
param_modifier  = "out" | "mut" .
ownership       = "owned" | "borrowed" .

(* Updated extern_item — adds optional ownership on return type *)
extern_item     = "@" identifier extern_params "->" [ ownership ] type
                  [ "as" string_literal ]
                  [ "#" identifier "(" { attribute_arg } ")" ]
                  [ where_clause ] .
```

The `#error(...)` and `#free(...)` attributes use the existing attribute syntax — no grammar change needed.

---

## Examples

### SQLite (Full Deep FFI)

```ori
// Raw FFI declarations — library author writes this ONCE
extern "c" from "sqlite3" #error(nonzero) #free(sqlite3_close) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
    @sqlite3_close (db: owned CPtr) -> c_int
    @sqlite3_exec (db: CPtr, sql: str) -> c_int
    @sqlite3_errmsg (db: CPtr) -> borrowed str #error(none)
}

// Safe public API — nearly boilerplate-free
pub type Database = { handle: owned CPtr }

pub @open (path: str) -> Result<Database, FfiError> uses FFI =
    Ok(Database { handle: sqlite3_open(filename: path)? })

pub @exec (db: Database, sql: str) -> Result<void, FfiError> uses FFI =
    sqlite3_exec(db: db.handle, sql: sql)

// User code — zero FFI awareness
use std.db { Database }
@main () -> void = {
    let db = Database.open(path: "app.db").unwrap()
    db.exec(sql: "CREATE TABLE users (name TEXT)").unwrap()
    // db auto-closed when scope exits (ARC + Drop + #free)
}
```

### BLAS for ML (Motivation Use Case)

```ori
extern "c" from "openblas" #error(none) {
    @cblas_dgemm (
        order: c_int,
        transA: c_int, transB: c_int,
        m: c_int, n: c_int, k: c_int,
        alpha: float,
        a: [float], lda: c_int,
        b: [float], ldb: c_int,
        beta: float,
        c: mut [float], ldc: c_int,
    ) -> void as "cblas_dgemm"
}

// Safe, shape-checked API (with const generics)
pub @matmul<$M: int, $N: int, $P: int> (
    a: Matrix<M, N>,
    b: Matrix<N, P>,
) -> Matrix<M, P> uses FFI = {
    let result = Matrix.zeros()
    unsafe {
        cblas_dgemm(
            order: 101,       // CblasRowMajor
            transA: 111, transB: 111,  // CblasNoTrans
            m: M as c_int, n: P as c_int, k: N as c_int,
            alpha: 1.0,
            a: a.data, lda: N as c_int,
            b: b.data, ldb: P as c_int,
            beta: 0.0,
            c: result.data, ldc: P as c_int,
        )
    }
    result
}

// Test — no BLAS library needed!
@test tests matmul {
    with FFI = handler {
        cblas_dgemm: (...) -> void = {
            // Naive O(n³) implementation for testing
            // ...
        },
    } in {
        let a = Matrix.from_rows([[1.0, 2.0], [3.0, 4.0]])
        let b = Matrix.from_rows([[5.0, 6.0], [7.0, 8.0]])
        let c = matmul(a, b)
        assert_eq(c.get(row: 0, col: 0), 19.0)
    }
}
```

### POSIX File I/O (Error Protocol)

```ori
extern "c" from "libc" #error(errno) {
    @open (path: str, flags: c_int, mode: c_int) -> c_int as "open"
    @read (fd: c_int, buf: mut [byte]) -> c_int as "read"
    @close (fd: c_int) -> c_int as "close"
    @strerror (errnum: c_int) -> borrowed str #error(none)
}

// User code:
let fd = open(path: "/etc/hostname", flags: O_RDONLY, mode: 0)?
let buf = [byte].with_capacity(1024)
let n = read(fd: fd, buf: buf)?
close(fd: fd)?
```

Compare with today's equivalent which requires manual errno checking on each call.

---

## Design Decisions

### Why are ownership annotations eventually required for CPtr?

Every CPtr has an ownership story. Forcing the programmer to state it prevents silent leaks. This is stricter than Rust (which allows raw pointers without annotation) but matches Ori's philosophy of explicit effects. Phased enforcement (optional → warning → error) avoids breaking existing code.

**Alternative considered:** Always optional. Rejected: undiscoverable leaks are worse than annotation burden.

### Why block-level error protocols with per-function override?

Most C libraries use a consistent error convention across all functions. Block-level captures this once. Per-function `#error(none)` handles the exceptions (e.g., `strerror` returns a string, not an error code). This is more ergonomic than annotating every function.

**Alternative considered:** Per-function only. Rejected: too verbose for libraries with 50+ functions sharing a convention.

### Why does `out` convert parameters to return values?

Ori is expression-based. Side-effect-only parameters are an anti-pattern. Converting `out` params to return values is idiomatic Ori — the function returns all its outputs. This also enables `?` propagation when combined with error protocols.

**Alternative considered:** Keep as `mut` parameters. Rejected: `mut` parameters for the sole purpose of returning a value through them is a C-ism that Ori should not propagate.

### Why does `borrowed str` copy immediately?

Ori has no lifetime system. Borrowed string views would require tracking how long the C string remains valid — which requires lifetime annotations Ori deliberately avoids. Copying is safe and consistent with the existing marshalling behavior. When/if borrowed views are implemented, this can evolve to zero-copy.

**Alternative considered:** Lifetime-bounded views. Rejected: Ori has no lifetimes. The slot is reserved but unimplemented.

### Why handler-based mocking rather than a new mock framework?

Ori's capability system already supports `with Cap = handler { ... } in` for stateful effect handling. FFI is a capability. Using the same mechanism maintains conceptual consistency and avoids introducing a separate test-specific framework.

**Alternative considered:** `#[mock]` attribute on extern blocks. Rejected: test infrastructure should not require language syntax changes.

### Why not auto-import C headers (like Zig's @cImport)?

Zig's approach is magical — it hides the FFI boundary entirely. Ori's design principle is "explicit boundaries": FFI calls are clearly marked, not hidden. The boundary should be *visible but low-friction*. Deep FFI reduces friction (less boilerplate) without hiding the boundary (extern blocks are still explicit).

**Alternative considered:** `ori bindgen header.h` tool. Deferred to future work — useful but orthogonal to the language design.

---

## Prior Art

| Language/Tool | What They Do | What Ori Learns |
|---------------|-------------|-----------------|
| **Swift** | Auto-bridging of `String` ↔ `const char*`; `__bridge_retained` / `__bridge_transfer` for ARC-FFI ownership | Ori's `str` marshalling is similar. Swift's bridge keywords inspired `owned`/`borrowed`. |
| **Rust** | Manual `CString`/`CStr`; `Box::from_raw`/`into_raw`; `bindgen` for header parsing | Too verbose. Ori can do better with compiler-assisted marshalling. |
| **Go CGo** | Auto errno capture; `C.CString`/`C.GoString`; GC handles most cleanup | Go's errno capture inspired `#error(errno)`. GC makes ownership easy — Ori needs explicit annotations since it uses ARC. |
| **CXX (Rust)** | Shared type definitions; `UniquePtr` ↔ `Box` ownership transfer; compile-time checked | CXX's `UniquePtr` model directly inspired `owned CPtr` + `#free`. |
| **Python CFFI** | Declarative C declarations; `ffi.gc()` for destructor attachment | CFFI's `ffi.gc()` is conceptually what `#free(fn)` does. |
| **Zig** | Direct C header import; automatic type translation | Too magical — hides the boundary. Ori wants visible but low-friction. |

---

## Implementation Phases

### Phase 1: Error Protocols + `out` Parameters

**Scope:** The features that eliminate the most boilerplate with the least change.

1. `#error(errno | nonzero | null | negative | success: N | none)` block/function attributes
2. `FfiError` type in std.ffi
3. Automatic `Result<T, FfiError>` wrapping when error protocol is active
4. `out` parameter modifier (parser → IR → codegen)
5. `out` params folded into return type
6. Errno reading infrastructure (`get_errno()` as compiler intrinsic)

**Exit criteria:** Can write the SQLite example above with `#error(nonzero)` and `out CPtr`.

### Phase 2: Ownership Annotations

**Scope:** Memory safety across the boundary. Depends on Drop trait.

1. `owned` / `borrowed` annotations (parser → IR → type checker)
2. `#free(fn)` attribute (block-level and per-function)
3. Auto-generated Drop impls for `owned CPtr` with `#free`
4. Compiler warnings for unannotated CPtr returns
5. `borrowed str` → immediate copy semantics

**Exit criteria:** The OpenSSL RSA example auto-frees on scope exit with zero manual cleanup.

### Phase 3: Declarative Marshalling Extensions

**Scope:** Automatic type conversion beyond what the base spec provides.

1. `[byte]` length elision (compiler inserts length argument)
2. `mut [byte]` parameter handling
3. `int` ↔ `c_int` automatic narrowing/widening with bounds checks
4. `bool` ↔ `c_int` conversion

**Exit criteria:** The zlib compress example works with `[byte]` params and no explicit length.

### Phase 4: Capability-Gated Testability

**Scope:** Mock infrastructure for FFI. Architecturally complex.

1. `with FFI = handler { ... } in` dispatch routing
2. Handler function signature validation against extern declarations
3. Selective mocking via `FFI("library_name")`
4. Fall-through to real implementation for unmocked functions

**Exit criteria:** Can test the BLAS matmul wrapper without linking OpenBLAS.

### Phase 5: Const-Generic Safety (Future)

**Scope:** Depends on const generics (Section 18) being complete.

1. Where clauses on extern items with const expressions
2. Buffer size validation at compile time
3. Fixed-capacity list (`[T, max N]`) integration at FFI boundary

**Exit criteria:** SHA256 example rejects buffers smaller than 32 bytes at compile time.

---

## Future Work

Explicitly deferred:

1. **`ori bindgen header.h`** — auto-generate extern blocks from C headers (tool, not language feature)
2. **C++ interop** (`extern "c++"`) — name mangling, vtables, exceptions
3. **WIT integration** — generate extern blocks from WebAssembly Interface Types
4. **Callback ownership** — ownership annotations on callback parameters
5. **Struct-level marshalling** — automatic conversion between Ori record types and C structs (beyond `#repr("c")`)
6. **Arena-scoped FFI** — allocate FFI temporaries in an arena, free all at once
7. **Custom error code maps** — `#error_codes({ 5: "SQLITE_BUSY", 7: "SQLITE_NOMEM" })` for rich error messages
8. **Compile-time layout verification** — verify `#repr("c")` structs match actual C layout

---

## Verification

After implementation, verify with:

1. **Existing FFI tests pass unchanged** — backward compatibility
2. **SQLite example compiles and runs** — `#error(nonzero)` + `out` + `owned` + `#free`
3. **POSIX file I/O example** — `#error(errno)` protocol
4. **Mock test example** — `with FFI = handler { ... } in` successfully mocks extern calls
5. **Spec conformance** — update `spec/24-ffi.md` with new syntax and semantics
6. **Grammar sync** — update `grammar.ebnf` with new productions
7. **Syntax reference sync** — update `.claude/rules/ori-syntax.md`
