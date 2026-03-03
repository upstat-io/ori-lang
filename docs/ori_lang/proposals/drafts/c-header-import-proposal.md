# Proposal: @cImport — Compile-Time C Header Import

**Status:** Draft
**Author:** Eric
**Created:** 2026-02-23
**Affects:** Compiler (parser, codegen), LLVM backend
**Depends on:** Platform FFI (approved), Deep FFI (approved)
**Extends:** Deep FFI `extern "c"` declarations — `@cImport` generates them, does not replace them

---

## Summary

`@cImport` reads C header files at compile time using libclang (already bundled via LLVM), translates C declarations into Ori `extern "c"` blocks, and applies automatic type mapping from C types to Ori types. Developers can layer Deep FFI safety annotations on top via `@cAnnotate` blocks or manual `extern "c"` overrides. The result: Zig's header import convenience with Ori's safety guarantees.

---

## Motivation

### The Declaration Tax

Every C library binding in Ori requires manually transcribing C function signatures into `extern "c"` blocks. For a library like SQLite (150+ API functions), OpenSSL (500+), or POSIX libc (hundreds), this is tedious, error-prone, and creates a maintenance burden — when the C library updates, someone must manually sync the extern declarations.

```ori
// Today: manually copy every signature from sqlite3.h
extern "c" from "sqlite3" {
    @sqlite3_open (filename: str, db: out CPtr) -> c_int as "sqlite3_open"
    @sqlite3_close (db: CPtr) -> c_int as "sqlite3_close"
    @sqlite3_exec (db: CPtr, sql: str, ...) -> c_int as "sqlite3_exec"
    @sqlite3_errmsg (db: CPtr) -> str as "sqlite3_errmsg"
    // ... 150 more functions ...
}
```

This is the same work that Zig's `@cImport` eliminates entirely.

### We Already Have The Parser

Ori depends on LLVM 21, which includes **libclang** — a complete, production-grade C parser. It is already linked into the compiler binary. The infrastructure to parse C headers exists; the missing piece is a translation layer from libclang's AST to Ori's `extern "c"` declarations.

### Manual Transcription Introduces Bugs

When a developer manually writes extern declarations by reading a C header:

1. **Wrong types** — `size_t` transcribed as `c_int` instead of `c_size` (silent truncation)
2. **Wrong parameter order** — C has no named parameters, easy to swap adjacent same-typed args
3. **Missing `const`** — losing const-correctness information that affects ownership inference
4. **Missed functions** — large headers have functions that are easy to overlook
5. **Version drift** — C library updates, extern declarations don't

`@cImport` eliminates all five by reading the authoritative source: the header file itself.

---

## Design

### Basic Syntax

```ori
@cImport("sqlite3.h", lib: "sqlite3")
```

This single line:

1. Invokes libclang to parse `sqlite3.h` (following standard include paths)
2. Extracts all function declarations, type definitions, enum constants, and `#define` integer constants
3. Generates `extern "c" from "sqlite3" { ... }` declarations internally
4. Makes all imported symbols available in the current module

### Type Mapping

The compiler translates C types to Ori types using deterministic rules:

| C Type | Ori Type | Notes |
|--------|----------|-------|
| `int`, `int32_t` | `c_int` | |
| `unsigned int`, `uint32_t` | `c_uint` | |
| `long`, `int64_t` | `c_long` | |
| `size_t` | `c_size` | |
| `float` | `c_float` | |
| `double` | `float` | Ori `float` is 64-bit |
| `char` | `c_char` | |
| `const char*` | `str` | Auto-marshalled (borrowed by default per Deep FFI) |
| `char*` (non-const) | `mut str` | Mutable string buffer |
| `void*`, `T*` (opaque) | `CPtr` | Opaque pointer |
| `T**` | `out CPtr` | Detected as out-parameter pattern |
| `_Bool`, `bool` | `bool` | |
| `void` (return) | `void` | |
| `enum { A, B, C }` | Named integer constants | |
| `struct T { ... }` | `#repr("c") type T = { ... }` | Field-by-field translation |
| `typedef` | Type alias | Resolved transitively |

#### Unmappable Types

Some C constructs cannot be automatically mapped:

| C Construct | Behavior |
|-------------|----------|
| Variadic functions (`printf(fmt, ...)`) | Skipped with compiler note |
| Function pointers | Mapped to Ori function types where possible; `CPtr` fallback |
| Bitfields | Skipped with compiler note |
| Inline functions | Skipped (no symbol to link against) |
| Preprocessor macros (non-constant) | Skipped |
| `union` | Mapped to `CPtr` with size annotation |
| Flexible array members | Skipped with compiler note |

Skipped declarations produce a compiler note (not warning, not error) listing what was skipped and why. The developer can manually declare these via `extern "c"` if needed.

### Include Path Resolution

```ori
// System header — searches standard include paths
@cImport("sqlite3.h", lib: "sqlite3")

// Explicit path relative to project root
@cImport("vendor/libfoo/include/foo.h", lib: "foo")

// Multiple headers from the same library
@cImport(["openssl/ssl.h", "openssl/err.h"], lib: "ssl")

// With additional include directories
@cImport("mylib.h", lib: "mylib", include: ["vendor/mylib/include"])
```

Include path search order:

1. Paths specified in `include:` parameter
2. Project-local `vendor/` and `include/` directories
3. Standard system include paths (platform-dependent, same as `cc -I`)
4. LLVM/Clang default include paths

### Selective Import

Large headers export hundreds of symbols. Import only what you need:

```ori
// Import everything (default)
@cImport("sqlite3.h", lib: "sqlite3")

// Import only specific functions
@cImport("sqlite3.h", lib: "sqlite3", only: [
    "sqlite3_open",
    "sqlite3_close",
    "sqlite3_exec",
    "sqlite3_errmsg",
])

// Import everything except specific functions
@cImport("sqlite3.h", lib: "sqlite3", except: [
    "sqlite3_test_control",    // internal testing API
    "sqlite3_snapshot_*",      // experimental API
])
```

The `only` and `except` parameters accept exact names and glob patterns (`*` suffix for prefix matching).

### Layering Safety Annotations

`@cImport` generates raw bindings — equivalent to what a developer would write manually in `extern "c"` blocks. Deep FFI safety annotations are applied via `@cAnnotate`:

```ori
@cImport("sqlite3.h", lib: "sqlite3")

@cAnnotate("sqlite3") {
    // Block-level defaults
    #error(nonzero)
    #free(sqlite3_close)

    // Per-function annotations
    sqlite3_open: { db: out owned },
    sqlite3_close: { db: owned },
    sqlite3_errmsg: #error(none),
}
```

`@cAnnotate` modifies the auto-generated declarations in place:

1. Adds `#error` protocol (block-level or per-function)
2. Adds `#free` cleanup function
3. Adds `owned`/`borrowed` ownership annotations to specific parameters
4. Adds `out` modifier to parameters detected as or intended as out-params
5. Any function not mentioned inherits block-level defaults only

### Manual Override

Manual `extern "c"` declarations take precedence over auto-generated ones for the same function name and library:

```ori
// Auto-import everything
@cImport("sqlite3.h", lib: "sqlite3")

// Override specific functions with manual declarations
extern "c" from "sqlite3" #error(nonzero) #free(sqlite3_close) {
    @sqlite3_open (filename: str, db: out owned CPtr) -> c_int
}
```

When the compiler sees both an auto-generated and manual declaration for `sqlite3_open`, the manual one wins. This provides an escape hatch for any case where the auto-mapping produces incorrect types.

### Pattern-Based Annotation Inference

The compiler can optionally infer common C patterns and suggest annotations:

```ori
// Opt in to inference suggestions
@cImport("sqlite3.h", lib: "sqlite3", suggest: true)
```

When `suggest: true` is set, the compiler emits suggestions (not warnings) based on pattern recognition:

| Pattern Detected | Suggestion |
|-----------------|------------|
| Returns `int`, library has `errno` usage | Suggest `#error(errno)` |
| Function named `*_close`, `*_free`, `*_destroy`, `*_release` | Suggest this is a destructor |
| Function named `*_create`, `*_new`, `*_open`, `*_alloc`, `*_init` | Suggest return is `owned` |
| `T**` parameter | Suggest `out` modifier |
| `const char*` return | Confirm borrowed default |
| Paired create/destroy functions (name-matched) | Suggest `#free` pairing |

Suggestions are informational — they appear as compiler notes that the developer can accept by adding to `@cAnnotate`. They are never applied automatically.

### Struct Import

C structs with `#repr("c")`-compatible layouts are translated to Ori struct types:

```c
// In mylib.h:
typedef struct {
    int x;
    int y;
    const char* label;
} Point;

void process_point(const Point* p);
```

```ori
// Generated by @cImport:
#repr("c")
type Point = { x: c_int, y: c_int, label: str }

// In extern block:
@process_point (p: Point) -> void
```

Nested structs, arrays within structs, and pointer-to-struct parameters are all handled. Self-referential structs (linked lists, trees) use `CPtr` for the recursive pointer.

### Enum and Constant Import

```c
// In sqlite3.h:
#define SQLITE_OK 0
#define SQLITE_ERROR 1
#define SQLITE_BUSY 5

enum {
    SQLITE_INTEGER = 1,
    SQLITE_FLOAT = 2,
    SQLITE_TEXT = 3,
};
```

```ori
// Generated by @cImport:
const SQLITE_OK: c_int = 0
const SQLITE_ERROR: c_int = 1
const SQLITE_BUSY: c_int = 5

const SQLITE_INTEGER: c_int = 1
const SQLITE_FLOAT: c_int = 2
const SQLITE_TEXT: c_int = 3
```

Integer `#define` constants and C enums are imported as Ori constants. Non-integer `#define` macros are skipped.

---

## Interaction with Existing Features

| Feature | Interaction |
|---------|------------|
| `extern "c" from "lib"` | `@cImport` generates these. Manual declarations override auto-generated ones. |
| Deep FFI annotations | Applied via `@cAnnotate` after import. All Deep FFI features work on imported declarations. |
| `uses FFI("lib")` capability | Auto-generated declarations use the same parametric capability as manual ones. |
| `with FFI("lib") = handler` | Mocking works identically — the import source doesn't affect dispatch. |
| Selective import (`only`/`except`) | Reduces symbol noise and compilation overhead for large headers. |
| WASM (`extern "js"`) | Not affected. `@cImport` is C-only. JS interop remains manual. |

---

## Examples

### Minimal: libm

```ori
@cImport("math.h", lib: "m", only: ["sin", "cos", "sqrt", "pow", "floor", "ceil"])

@main () -> int = {
    let x = sin(x: 1.0);
    let y = cos(x: 0.0);
    if y == 1.0 then 0 else 1
}
```

### Full Stack: SQLite with Safety

```ori
// Step 1: Import the raw C API
@cImport("sqlite3.h", lib: "sqlite3", only: [
    "sqlite3_open", "sqlite3_close", "sqlite3_exec",
    "sqlite3_errmsg", "sqlite3_prepare_v2", "sqlite3_step",
    "sqlite3_finalize", "sqlite3_column_*", "SQLITE_*",
])

// Step 2: Layer safety annotations
@cAnnotate("sqlite3") {
    #error(nonzero)
    #free(sqlite3_close)

    sqlite3_open: { db: out owned },
    sqlite3_close: { db: owned },
    sqlite3_finalize: { stmt: owned },
    sqlite3_errmsg: #error(none),
    sqlite3_step: #error(none),  // SQLITE_ROW/SQLITE_DONE are not errors
}

// Step 3: Build a safe Ori API on top
pub type Database = { handle: owned CPtr }

pub @open (path: str) -> Result<Database, FfiError> uses FFI("sqlite3") =
    Ok(Database { handle: sqlite3_open(filename: path)? })

pub @exec (db: Database, sql: str) -> Result<void, FfiError> uses FFI("sqlite3") =
    sqlite3_exec(db: db.handle, sql: sql)
```

### Cross-Platform Library

```ori
// Platform-specific imports
#target("native") {
    @cImport("zlib.h", lib: "z", only: ["compress", "uncompress", "compressBound"])

    @cAnnotate("z") {
        #error(negative)
    }
}

#target("wasm") {
    extern "js" from "./compression.js" {
        @_compress (data: [byte]) -> [byte] as "compress"
        @_uncompress (data: [byte]) -> [byte] as "uncompress"
    }
}

// Unified public API — platform-agnostic
pub @compress (data: [byte]) -> Result<[byte], CompressionError> uses FFI = ...
```

### Testing Imported Functions

```ori
@cImport("sqlite3.h", lib: "sqlite3")

@cAnnotate("sqlite3") {
    #error(nonzero)
    #free(sqlite3_close)
    sqlite3_open: { db: out owned },
    sqlite3_close: { db: owned },
    sqlite3_errmsg: #error(none),
}

// Tests mock the imported functions identically to manual extern declarations
@test tests database_open {
    with FFI("sqlite3") = handler {
        sqlite3_open: (filename: str) -> Result<CPtr, FfiError> =
            Ok(CPtr.from_address(0x1234)),
    } in {
        let db = sqlite3_open(filename: "test.db")?
        assert(db != CPtr.null())
    }
}
```

---

## Design Decisions

### Why a language feature, not a standalone tool?

The Deep FFI proposal deferred header import as `ori bindgen header.h` — a CLI tool that generates `.ori` source files. `@cImport` is better because:

1. **Always fresh** — reads the header at compile time. No generated files to keep in sync. When the C library updates, recompile and you get the new API.
2. **No generated code in the repo** — avoids committing thousands of lines of auto-generated extern declarations.
3. **Composable with `@cAnnotate`** — annotations reference the auto-generated names directly. A separate tool would require a two-step pipeline.
4. **IDE integration** — the compiler knows the imported symbols, enabling autocomplete, go-to-definition (into the C header), and inline documentation.

**Alternative considered:** Standalone `ori bindgen` tool. Not rejected outright — may still be useful for inspecting what `@cImport` generates. But the primary interface should be a language feature.

### Why libclang, not a custom C parser?

libclang handles every C dialect, preprocessor configuration, platform-specific headers, and compiler extension in existence. Writing a custom C parser that handles real-world headers (which rely on GCC/Clang extensions, complex `#ifdef` trees, and platform-specific macros) would be a multi-year effort with permanent maintenance burden. libclang is already linked — use it.

### Why `@cAnnotate` as a separate block instead of inline in `@cImport`?

Separation of concerns:

1. **`@cImport`** = what the C library provides (facts, derived from the header)
2. **`@cAnnotate`** = how Ori should interact with it (policy, authored by the developer)

Mixing them would require interleaving auto-generated and hand-written code, making it unclear what came from the header vs what the developer added.

### Why are unmappable types skipped silently?

Not silently — skipped with a compiler note. But notes, not warnings or errors, because:

1. Large headers contain many internal/private functions that the user doesn't need
2. Variadic functions and bitfields are common in C but rarely used from higher-level languages
3. Errors would make `@cImport` unusable on real-world headers that inevitably contain some unmappable constructs

The `only` parameter lets developers scope the import to just the functions they care about, avoiding noise from irrelevant skipped declarations.

### Why not infer safety annotations automatically?

Pattern-based inference (create/destroy pairing, errno detection) is useful as suggestions but dangerous as automatic behavior:

1. **False positives** — `list_destroy()` might not be the destructor for the pointer returned by `list_create()` if they operate on different types
2. **Semantic ambiguity** — a function returning `int` might use errno, or might return a meaningful integer. The compiler cannot know.
3. **Ori's philosophy** — explicit boundaries. The developer should consciously decide what ownership semantics apply.

`suggest: true` provides the convenience of inference while keeping the developer in control. Suggestions can be promoted to annotations with a single copy-paste.

### Why do manual declarations override auto-generated ones?

The auto-mapping is necessarily imperfect. C headers contain patterns that don't map cleanly to Ori's type system. Manual override is the escape hatch — the developer says "I know better than the auto-mapper for this specific function." Without this, `@cImport` would be unusable for any library with even one unusual function signature.

---

## Prior Art

| Language/Tool | Approach | Tradeoffs |
|---------------|----------|-----------|
| **Zig `@cImport`** | Compile-time header import, direct C type usage | Zero friction, zero safety. No ownership, no error mapping, no marshalling. |
| **Rust `bindgen`** | Standalone tool, generates `extern "C"` Rust code | Generated code committed to repo, gets stale. No safety annotations. |
| **Swift** | Compiler auto-bridges Objective-C and C headers | Deeply integrated but only works with Apple's Clang. No error protocol mapping. |
| **Go `cgo`** | Comment-based `#include`, auto-bridges at compile time | Compile-time import (like `@cImport`) but with heavy runtime overhead and GC pinning. |
| **Python `cffi`** | Paste C declarations as strings, runtime parsing | Declarative but runtime, no compile-time type checking. |

Ori's approach combines:
- **Zig's convenience** (compile-time header parsing, no generated files)
- **Swift's type bridging** (automatic C-to-Ori type translation)
- **Rust's safety** (ownership tracking via Deep FFI annotations)
- **Go's errno capture** (via `#error` protocols from Deep FFI)

No existing system combines all four.

---

## Implementation

### Architecture

```
@cImport("header.h", lib: "lib")
    │
    ▼
libclang (parse C header)
    │
    ▼
C AST (functions, types, constants)
    │
    ▼
Type mapping (C types → Ori types)
    │
    ▼
Generated extern "c" from "lib" { ... }
    │
    ▼
@cAnnotate("lib") { ... }  ──► Merge safety annotations
    │
    ▼
Manual extern "c" overrides ──► Replace specific declarations
    │
    ▼
Final extern declarations (fed into existing FFI pipeline)
```

### Implementation Steps

1. **libclang binding** — Create a Rust wrapper around libclang's C API (or use the `clang-sys`/`clang` crate). Extract function declarations, type definitions, enum constants, and `#define` integer constants from a translation unit.

2. **Type mapper** — Translate libclang's type representation to Ori's `extern` type vocabulary. Handle pointers, const qualifiers, typedefs, structs, enums. Produce skip notes for unmappable constructs.

3. **Declaration generator** — Produce Ori `extern "c"` AST nodes from the mapped types. These feed into the existing parser/IR pipeline as if they were hand-written.

4. **`@cAnnotate` parser** — Parse the annotation block and merge attributes into the generated declarations. Validate that referenced function names exist in the import.

5. **Override resolution** — When both auto-generated and manual `extern "c"` declarations exist for the same function in the same library, discard the auto-generated one.

6. **Caching** — Cache the parsed header output (keyed by header path + modification time + include paths). Re-parse only when the header changes.

7. **Suggest mode** — When `suggest: true` is set, run pattern matchers over the generated declarations and emit compiler notes with suggested `@cAnnotate` entries.

### Crate Placement

- **`oric`** — `@cImport` invocation and caching (IO-bound, belongs in the CLI crate)
- **`ori_parse`** — `@cAnnotate` syntax parsing
- **`ori_ir`** — `CImport` and `CAnnotate` AST nodes
- **`ori_types`** — Validation that `@cAnnotate` references valid imported names
- **`ori_llvm`** — No changes needed. Generated declarations use the existing extern pipeline.

---

## Future Work

Explicitly deferred:

1. **`ori bindgen` CLI tool** — Generate `.ori` source files from headers for inspection/debugging. Complements `@cImport`, does not replace it.
2. **C++ header import** (`@cxxImport`) — Would require name mangling, vtable layout, and exception handling. Significantly more complex than C.
3. **Incremental re-parse** — Only re-parse headers that changed, not the entire translation unit. Optimization for large header trees.
4. **Cross-reference documentation** — Link auto-generated declarations back to their source location in the C header for IDE navigation.
5. **Annotation files** — `.ori-ffi.toml` or similar for sharing `@cAnnotate` blocks across projects (community-maintained safety annotations for popular C libraries).

---

## Verification

After implementation, verify with:

1. **libm** — `@cImport("math.h")` produces working `sin`, `cos`, `sqrt` bindings
2. **SQLite** — Full import + `@cAnnotate` with error protocol + ownership + `#free`
3. **zlib** — `[byte]` parameter marshalling through auto-generated declarations
4. **Large header stress test** — Import a large header (`openssl/ssl.h`) and verify reasonable compilation time and correct skip notes
5. **Override precedence** — Manual `extern "c"` correctly overrides auto-generated declaration for same function
6. **Caching** — Second compilation with unchanged headers does not re-invoke libclang
7. **Suggest mode** — Pattern matcher produces reasonable suggestions for SQLite create/close pairs
8. **Capability integration** — Imported functions respect `uses FFI("lib")` and are mockable via `with FFI("lib") = handler`
