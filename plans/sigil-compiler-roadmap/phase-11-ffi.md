# Phase 11: Foreign Function Interface (FFI)

**Goal**: Enable Sigil to call C libraries and system APIs

**Criticality**: **CRITICAL** — Without FFI, Sigil cannot integrate with the software ecosystem

---

## Design Decisions

### Key Questions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Should FFI require unsafe? | Yes | C code can violate Sigil's safety guarantees |
| How to integrate with capabilities? | `uses Unsafe` capability | Consistent with effect tracking |
| Support C++ directly? | No | C ABI only; C++ via extern "C" |
| Support callbacks? | Yes, Phase 1.4 | Required for many C APIs |
| Memory management? | Manual in unsafe blocks | C doesn't know about ARC |

### Capability Integration

```sigil
// FFI functions require Unsafe capability
@call_c_function () -> int uses Unsafe = extern("c_function")

// Callers must have capability
@main () -> void uses Unsafe = run(
    let result = call_c_function(),
    print(str(result)),
)

// Or provide it explicitly in tests
@test_c_call tests @call_c_function () -> void = run(
    with Unsafe = AllowUnsafe in
        assert_eq(actual: call_c_function(), expected: 42),
)
```

---

## Reference Implementation

### Rust FFI

```
~/lang_repos/rust/compiler/rustc_hir/src/def.rs        # ForeignItem definitions
~/lang_repos/rust/library/std/src/ffi/                 # C types (CStr, CString)
~/lang_repos/rust/compiler/rustc_codegen_llvm/src/abi.rs  # ABI handling
```

### Go CGO

```
~/lang_repos/golang/src/cmd/cgo/                       # CGO implementation
~/lang_repos/golang/src/runtime/cgo/                   # Runtime support
```

---

## 11.1 Extern Blocks

**Spec section**: `spec/23-ffi.md § Extern Blocks`

### Syntax

```ebnf
ExternBlock = 'extern' [ StringLiteral ] '{' { ExternItem } '}' ;
ExternItem  = ExternFunction | ExternStatic ;
ExternFunction = '@' Identifier '(' [ ParamList ] ')' '->' Type ;
ExternStatic   = '$' Identifier ':' Type ;
```

### Semantics

```sigil
// Declare external C functions
extern "C" {
    @strlen (s: *byte) -> int
    @malloc (size: int) -> *byte
    @free (ptr: *byte) -> void

    $errno: int  // External static
}
```

### Implementation

- [ ] **Spec**: Add `spec/23-ffi.md` with extern block syntax
  - [ ] Define extern block grammar
  - [ ] Define calling conventions ("C" default, future: "stdcall", etc.)
  - [ ] Define linkage semantics

- [ ] **Lexer**: Add tokens
  - [ ] `extern` keyword
  - [ ] String literal for ABI (already exists)

- [ ] **Parser**: Parse extern blocks
  - [ ] `parse_extern_block()` in parser
  - [ ] Add `ExternBlock` to AST
  - [ ] Add `ExternItem` variants

- [ ] **Type checker**: Validate extern declarations
  - [ ] Ensure types are FFI-safe
  - [ ] Check for `uses Unsafe` in callers

- [ ] **Codegen**: Generate external references
  - [ ] Emit LLVM `declare` for functions
  - [ ] Handle calling convention
  - [ ] Link external symbols

- [ ] **Test**: `tests/spec/ffi/extern_blocks.si`
  - [ ] Basic extern function declaration
  - [ ] Multiple functions in one block
  - [ ] External statics

---

## 11.2 C ABI Types

**Spec section**: `spec/23-ffi.md § C Types`

### Primitive Mappings

| Sigil Type | C Type | Size |
|------------|--------|------|
| `c_char` | `char` | 1 byte |
| `c_short` | `short` | 2 bytes |
| `c_int` | `int` | 4 bytes |
| `c_long` | `long` | platform |
| `c_longlong` | `long long` | 8 bytes |
| `c_float` | `float` | 4 bytes |
| `c_double` | `double` | 8 bytes |
| `c_void` | `void` | 0 bytes |
| `c_size` | `size_t` | platform |

### Pointer Types

```sigil
*T           // Raw pointer to T (nullable)
*mut T       // Mutable raw pointer
*byte        // void* equivalent
```

### Struct Layout

```sigil
#repr(C)
type Point = {
    x: c_int,
    y: c_int,
}
```

### Implementation

- [ ] **Spec**: Add C types section
  - [ ] Primitive type mappings
  - [ ] Pointer syntax
  - [ ] `#repr(C)` attribute

- [ ] **Types**: Add C primitive types
  - [ ] Add to `Type` enum
  - [ ] Size/alignment handling
  - [ ] Platform-dependent sizes

- [ ] **Parser**: Parse pointer types
  - [ ] `*T` syntax
  - [ ] `*mut T` syntax

- [ ] **Type checker**: FFI type validation
  - [ ] Warn on non-FFI-safe types
  - [ ] Validate `#repr(C)` structs

- [ ] **Test**: `tests/spec/ffi/c_types.si`
  - [ ] All primitive C types
  - [ ] Pointer operations
  - [ ] Repr(C) structs

---

## 11.3 Unsafe Blocks

**Spec section**: `spec/23-ffi.md § Unsafe Blocks`

### Syntax

```sigil
unsafe {
    // Operations that require manual safety guarantees
    let ptr = malloc(100)
    *ptr = 42  // Dereference raw pointer
    free(ptr)
}
```

### Semantics

Inside `unsafe`:
- Dereference raw pointers
- Call extern functions
- Access mutable statics
- Transmute types

### Implementation

- [ ] **Spec**: Define unsafe block semantics
  - [ ] List of unsafe operations
  - [ ] Scoping rules
  - [ ] Interaction with capabilities

- [ ] **Parser**: Parse unsafe blocks
  - [ ] `unsafe` keyword
  - [ ] Block expression

- [ ] **Type checker**: Track unsafe context
  - [ ] Set `in_unsafe` flag
  - [ ] Allow unsafe operations only in context

- [ ] **Evaluator**: Execute unsafe operations
  - [ ] Pointer dereference
  - [ ] Raw memory access

- [ ] **Test**: `tests/spec/ffi/unsafe_blocks.si`
  - [ ] Basic unsafe block
  - [ ] Nested unsafe
  - [ ] Unsafe operations outside block (compile error)

- [ ] **Test**: `tests/compile-fail/ffi/unsafe_required.si`
  - [ ] Pointer deref outside unsafe
  - [ ] Extern call without capability

---

## 11.4 Raw Pointers

**Spec section**: `spec/23-ffi.md § Raw Pointers`

### Operations

```sigil
// Creation
let ptr: *int = raw_ptr(some_int)      // Take address
let null_ptr: *int = null()            // Null pointer

// Comparison
ptr == null_ptr                        // Pointer equality
ptr != other

// Arithmetic (unsafe)
unsafe {
    let next = ptr.offset(1)           // Pointer arithmetic
    let value = *ptr                   // Dereference
    *ptr = new_value                   // Write through pointer
}

// Conversion
let opt = ptr.as_option()              // *T -> Option<&T>
```

### Implementation

- [ ] **Spec**: Raw pointer operations
  - [ ] Creation from references
  - [ ] Null pointers
  - [ ] Pointer arithmetic
  - [ ] Dereference

- [ ] **Types**: Pointer type representation
  - [ ] `*T` in type system
  - [ ] Nullability tracking
  - [ ] Size (platform pointer size)

- [ ] **Operators**: Pointer operations
  - [ ] Dereference `*ptr`
  - [ ] Address-of `raw_ptr(x)`
  - [ ] Offset `.offset(n)`

- [ ] **Test**: `tests/spec/ffi/raw_pointers.si`
  - [ ] Pointer creation
  - [ ] Null checks
  - [ ] Dereference (in unsafe)
  - [ ] Pointer arithmetic

---

## 11.5 Capability Integration

**Spec section**: `spec/23-ffi.md § Unsafe Capability`

### Design

```sigil
// The Unsafe capability
trait Unsafe {
    // Marker trait - no methods
    // Allows: unsafe blocks, extern calls, raw pointer ops
}

// Real implementation (production)
type AllowUnsafe impl Unsafe = {}

// Disabled implementation (safe tests)
type DenyUnsafe impl Unsafe = {}  // Compile error if used

// Usage
@dangerous_operation () -> Result<int, Error> uses Unsafe = run(
    unsafe {
        let ptr = malloc(100)
        // ...
    },
)
```

### Implementation

- [ ] **Spec**: Unsafe capability definition
  - [ ] As a marker capability
  - [ ] Propagation rules
  - [ ] Testing patterns

- [ ] **Capability system**: Add `Unsafe` capability
  - [ ] Define in prelude
  - [ ] Track in function signatures
  - [ ] Propagate to callers

- [ ] **Type checker**: Enforce capability requirement
  - [ ] Require `uses Unsafe` for extern calls
  - [ ] Require `uses Unsafe` for unsafe blocks

- [ ] **Test**: `tests/spec/ffi/unsafe_capability.si`
  - [ ] Function requiring Unsafe
  - [ ] Providing Unsafe in tests
  - [ ] Missing capability error

---

## 11.6 Build System Integration

**Spec section**: `spec/23-ffi.md § Linking`

### Link Attributes

```sigil
#link(name: "sqlite3")]
extern "C" {
    @sqlite3_open (filename: *byte, ppDb: **sqlite3) -> c_int
}

#link(name: "m", kind: "dylib")]
extern "C" {
    @sin (x: c_double) -> c_double
}
```

### Implementation

- [ ] **Spec**: Link specification
  - [ ] `#link(...)` attribute syntax
  - [ ] Library kinds (static, dylib, framework)
  - [ ] Search paths

- [ ] **Codegen**: Emit link directives
  - [ ] LLVM link metadata
  - [ ] Library search

- [ ] **Build system**: Handle native dependencies
  - [ ] `sigil.toml` native deps section
  - [ ] pkg-config integration

- [ ] **Test**: `tests/spec/ffi/linking.si`
  - [ ] Link to libc
  - [ ] Link to libm
  - [ ] Custom library

---

## 11.7 Callbacks

**Spec section**: `spec/23-ffi.md § Callbacks`

### Syntax

```sigil
// C function that takes a callback
extern "C" {
    @qsort (
        base: *byte,
        nmemb: c_size,
        size: c_size,
        compar: extern fn(*byte, *byte) -> c_int,
    ) -> void
}

// Creating a callback
@compare_ints (a: *byte, b: *byte) -> c_int uses Unsafe = run(
    unsafe {
        let a_val = *(a as *c_int)
        let b_val = *(b as *c_int)
        compare(left: a_val, right: b_val) |> match(
            Less -> -1,
            Equal -> 0,
            Greater -> 1,
        )
    },
)

// Using it
let callback: extern fn(*byte, *byte) -> c_int = extern_fn(compare_ints)
qsort(base: arr_ptr, nmemb: len, size: 4, compar: callback)
```

### Implementation

- [ ] **Spec**: Callback semantics
  - [ ] `extern fn` type syntax
  - [ ] Conversion from Sigil functions
  - [ ] Lifetime considerations

- [ ] **Types**: Extern function type
  - [ ] `extern fn(...) -> T` in type system
  - [ ] ABI specification

- [ ] **Codegen**: Generate callback wrappers
  - [ ] Trampoline functions
  - [ ] ABI adaptation

- [ ] **Test**: `tests/spec/ffi/callbacks.si`
  - [ ] Simple callback
  - [ ] qsort example
  - [ ] Callback with userdata

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/23-ffi.md` complete
- [ ] CLAUDE.md updated with FFI syntax
- [ ] Can call libc functions (strlen, malloc, free)
- [ ] Can call libm functions (sin, cos, sqrt)
- [ ] Can create and use SQLite binding
- [ ] All tests pass: `cargo test && sigil test tests/spec/ffi/`
- [ ] `uses Unsafe` properly enforced

**Exit Criteria**: Can write a program that opens and queries a SQLite database

---

## Example: SQLite Binding

Target capability demonstration:

```sigil
#link(name: "sqlite3")]
extern "C" {
    @sqlite3_open (filename: *byte, ppDb: **Sqlite3) -> c_int
    @sqlite3_close (db: *Sqlite3) -> c_int
    @sqlite3_exec (
        db: *Sqlite3,
        sql: *byte,
        callback: extern fn(*byte, c_int, **byte, **byte) -> c_int,
        userdata: *byte,
        errmsg: **byte,
    ) -> c_int
}

type Sqlite3 = {}  // Opaque type

@open_database (path: str) -> Result<*Sqlite3, Error> uses Unsafe = run(
    unsafe {
        let db: *Sqlite3 = null()
        let result = sqlite3_open(
            filename: path.as_c_str(),
            ppDb: raw_ptr(db),
        )
        if result == 0
            then Ok(db)
            else Err(Error { message: "Failed to open database" })
    },
)
```
