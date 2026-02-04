# Proposal: Minimal FFI for Console Support

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-03
**Affects:** Compiler (FFI), Standard library
**Depends on:** Capabilities System (Section 6)
**Enables:** `std.console` API

---

## Summary

This proposal defines the **minimal FFI features** required to implement console/terminal support in Ori. Rather than implementing the full FFI specification (Section 11) upfront, we identify the precise subset needed to call POSIX termios and Windows Console API functions.

---

## Motivation

Console support is a high-value first project for maturing Ori because:

1. **Exercises critical language features** — FFI, capabilities, traits, conditional compilation
2. **Delivers immediate utility** — Every CLI tool needs terminal support
3. **Showcases Ori's strengths** — Capability system is perfect for terminal state management
4. **Drives ecosystem development** — FFI unlocks integration with existing C libraries

However, the full FFI specification (Section 11) is large. This proposal identifies the **minimum viable FFI** needed specifically for console support, allowing incremental implementation.

---

## Required FFI Features

### 1. Extern Block Syntax (11.1 - Partial)

**Needed:**
```ori
extern "c" from "c" {
    @_tcgetattr (fd: c_int, termios: CPtr) -> c_int as "tcgetattr"
    @_tcsetattr (fd: c_int, actions: c_int, termios: CPtr) -> c_int as "tcsetattr"
    @_read (fd: c_int, buf: CPtr, count: c_size) -> c_ssize as "read"
    @_write (fd: c_int, buf: CPtr, count: c_size) -> c_ssize as "write"
    @_ioctl (fd: c_int, request: c_ulong, arg: CPtr) -> c_int as "ioctl"
}
```

**Not needed yet:**
- `extern "js"` — WASM target can come later
- Variadic C functions — Can use fixed-arity wrappers

### 2. C ABI Types (11.2 - Partial)

**Minimum required types:**

| Ori Type | C Type | Size | Notes |
|----------|--------|------|-------|
| `c_int` | `int` | 4 bytes | Most common C type |
| `c_uint` | `unsigned int` | 4 bytes | For flags |
| `c_long` | `long` | platform | termios fields |
| `c_ulong` | `unsigned long` | platform | ioctl requests |
| `c_size` | `size_t` | platform | Buffer sizes |
| `c_ssize` | `ssize_t` | platform | Read/write returns |
| `CPtr` | `void*` | platform | Opaque pointers |

**Not needed yet:**
- `c_char`, `c_short`, `c_longlong` — Not used by termios
- `c_float`, `c_double` — Not used by console APIs

### 3. Structs with #repr("c") (11.3 - Partial)

**Needed for termios struct:**
```ori
#repr("c")
type Termios = {
    c_iflag: c_uint,   // Input flags
    c_oflag: c_uint,   // Output flags
    c_cflag: c_uint,   // Control flags
    c_lflag: c_uint,   // Local flags
    c_cc: [byte, max 32],  // Control characters (NCCS varies by platform)
}

#repr("c")
type WinSize = {
    ws_row: c_ushort,
    ws_col: c_ushort,
    ws_xpixel: c_ushort,
    ws_ypixel: c_ushort,
}
```

**Not needed yet:**
- `#repr("packed")` — Not needed for console
- `#repr("aligned", N)` — Not needed for console

### 4. CPtr Operations

**Minimum operations:**
```ori
impl CPtr {
    @null () -> CPtr                           // Null pointer
    @is_null (self) -> bool                    // Check for null
    @from_bytes (bytes: [byte]) -> CPtr        // Get pointer to byte array
    @from_struct<T> (value: T) -> CPtr         // Get pointer to struct (T must be #repr("c"))
}
```

**Not needed yet:**
- Pointer arithmetic
- Dereferencing (all done via FFI calls)
- Casting between pointer types

### 5. FFI Capability (11.5)

**Required:**
```ori
trait FFI {}

// Functions calling extern must declare FFI
@get_terminal_size () -> (int, int) uses FFI = ...

// Callers must have capability
@main () -> void uses FFI = ...

// Or provide in tests
@test tests @get_terminal_size () -> void =
    with FFI = AllowFFI in assert(get_terminal_size() != (0, 0))
```

### 6. Conditional Compilation (Section 13 - Partial)

**Needed for platform-specific code:**
```ori
#target(os: "linux")
extern "c" from "c" {
    @_tcgetattr (fd: c_int, termios: CPtr) -> c_int as "tcgetattr"
}

#target(os: "windows")
extern "c" from "kernel32" {
    @_GetConsoleMode (handle: CPtr, mode: CPtr) -> c_int as "GetConsoleMode"
}
```

**Minimum attributes:**
- `#target(os: "linux")` / `#target(os: "macos")` / `#target(os: "windows")`
- `#target(family: "unix")` / `#target(family: "windows")`
- `$target_os` and `$target_family` compile-time constants

---

## POSIX Functions Required

For Linux/macOS console support:

| Function | Purpose | Header |
|----------|---------|--------|
| `tcgetattr` | Get terminal attributes | `<termios.h>` |
| `tcsetattr` | Set terminal attributes | `<termios.h>` |
| `ioctl` | Get terminal size (TIOCGWINSZ) | `<sys/ioctl.h>` |
| `read` | Read from stdin | `<unistd.h>` |
| `write` | Write to stdout/stderr | `<unistd.h>` |
| `isatty` | Check if fd is a terminal | `<unistd.h>` |

**Constants needed (as Ori constants):**
```ori
// termios flags
let $ECHO: c_uint = 0x00000008       // Echo input
let $ICANON: c_uint = 0x00000100     // Canonical mode
let $ISIG: c_uint = 0x00000080       // Enable signals
let $IEXTEN: c_uint = 0x00000400     // Extended processing
let $ICRNL: c_uint = 0x00000100      // CR to NL
let $IXON: c_uint = 0x00000200       // XON/XOFF flow control
let $OPOST: c_uint = 0x00000001      // Output processing

// tcsetattr actions
let $TCSANOW: c_int = 0
let $TCSAFLUSH: c_int = 2

// ioctl requests
let $TIOCGWINSZ: c_ulong = 0x5413    // Get window size (Linux)
// Note: Different on macOS (0x40087468)
```

---

## Windows Functions Required

For Windows console support:

| Function | Purpose | Library |
|----------|---------|---------|
| `GetStdHandle` | Get stdin/stdout/stderr handle | kernel32 |
| `GetConsoleMode` | Get console mode flags | kernel32 |
| `SetConsoleMode` | Set console mode flags | kernel32 |
| `GetConsoleScreenBufferInfo` | Get terminal size | kernel32 |
| `ReadConsoleInputW` | Read input events | kernel32 |
| `WriteConsoleW` | Write output | kernel32 |

**Constants needed:**
```ori
#target(os: "windows")
let $STD_INPUT_HANDLE: c_uint = -10
let $STD_OUTPUT_HANDLE: c_uint = -11
let $ENABLE_VIRTUAL_TERMINAL_PROCESSING: c_uint = 0x0004
let $ENABLE_VIRTUAL_TERMINAL_INPUT: c_uint = 0x0200
```

---

## Implementation Order

### Phase 1: Core FFI

1. **Lexer**: Add `extern` keyword
2. **Parser**: Parse `extern "c" from "lib" { ... }` blocks
3. **IR**: Add `ExternBlock` and `ExternItem` AST nodes
4. **Type checker**:
   - Add C types (`c_int`, `c_uint`, etc.)
   - Add `CPtr` opaque type
   - Validate FFI-safe types in extern declarations
5. **LLVM codegen**: Generate `declare` statements for extern functions

### Phase 2: FFI Capability

1. **Prelude**: Add `FFI` trait and `AllowFFI` type
2. **Type checker**: Require `uses FFI` for extern calls
3. **Tests**: Verify capability enforcement

### Phase 3: Struct Layout

1. **Parser**: Parse `#repr("c")` attribute
2. **Type checker**: Validate #repr on structs
3. **LLVM codegen**: Generate C-compatible struct layouts

### Phase 4: CPtr Operations

1. **Evaluator**: Implement `CPtr.null()`, `CPtr.is_null()`
2. **LLVM codegen**: Implement `CPtr.from_bytes()`, `CPtr.from_struct()`
3. **Tests**: Verify pointer operations

### Phase 5: Conditional Compilation

1. **Parser**: Parse `#target(os: "...")` attribute
2. **Type checker**: Evaluate target conditions
3. **Codegen**: Exclude non-matching code

---

## Example: Complete Termios Binding

```ori
// std/console/sys/unix/termios.ori

#target(family: "unix")

// C types
extern "c" from "c" {
    @_tcgetattr (fd: c_int, termios: CPtr) -> c_int as "tcgetattr"
    @_tcsetattr (fd: c_int, actions: c_int, termios: CPtr) -> c_int as "tcsetattr"
    @_ioctl (fd: c_int, request: c_ulong, arg: CPtr) -> c_int as "ioctl"
    @_isatty (fd: c_int) -> c_int as "isatty"
}

// Constants (Linux values - macOS needs separate file)
#target(os: "linux")
let $TIOCGWINSZ: c_ulong = 0x5413

#target(os: "macos")
let $TIOCGWINSZ: c_ulong = 0x40087468

// Termios struct
#repr("c")
type Termios = {
    c_iflag: c_uint,
    c_oflag: c_uint,
    c_cflag: c_uint,
    c_lflag: c_uint,
    c_line: byte,
    c_cc: [byte, max 32],
    c_ispeed: c_uint,
    c_ospeed: c_uint,
}

// Flags
let $ECHO: c_uint = 8
let $ICANON: c_uint = 2
let $ISIG: c_uint = 1
let $IEXTEN: c_uint = 0x8000
let $TCSAFLUSH: c_int = 2

// Window size struct
#repr("c")
type WinSize = {
    ws_row: c_ushort,
    ws_col: c_ushort,
    ws_xpixel: c_ushort,
    ws_ypixel: c_ushort,
}

// High-level API
type TermiosError = NotATty | IoError(code: c_int)

@get_termios (fd: c_int) -> Result<Termios, TermiosError> uses FFI = run(
    if _isatty(fd: fd) == 0 then Err(NotATty)
    else run(
        let termios = Termios.default(),
        let ptr = CPtr.from_struct(value: termios),
        let result = _tcgetattr(fd: fd, termios: ptr),
        if result == 0 then Ok(termios)
        else Err(IoError(code: result)),
    ),
)

@set_termios (fd: c_int, termios: Termios) -> Result<void, TermiosError> uses FFI = run(
    let ptr = CPtr.from_struct(value: termios),
    let result = _tcsetattr(fd: fd, actions: $TCSAFLUSH, termios: ptr),
    if result == 0 then Ok(())
    else Err(IoError(code: result)),
)

@get_winsize (fd: c_int) -> Result<(int, int), TermiosError> uses FFI = run(
    let ws = WinSize { ws_row: 0, ws_col: 0, ws_xpixel: 0, ws_ypixel: 0 },
    let ptr = CPtr.from_struct(value: ws),
    let result = _ioctl(fd: fd, request: $TIOCGWINSZ, arg: ptr),
    if result == 0 then Ok((ws.ws_col as int, ws.ws_row as int))
    else Err(IoError(code: result)),
)

@enable_raw_mode (fd: c_int) -> Result<Termios, TermiosError> uses FFI = run(
    let original = get_termios(fd: fd)?,
    let raw = Termios {
        ...original,
        c_lflag: original.c_lflag & !($ECHO | $ICANON | $ISIG | $IEXTEN),
    },
    set_termios(fd: fd, termios: raw)?,
    Ok(original),  // Return original for restoration
)

@disable_raw_mode (fd: c_int, original: Termios) -> Result<void, TermiosError> uses FFI =
    set_termios(fd: fd, termios: original)
```

---

## Deferred Features

The following FFI features are **not needed** for console support and can be implemented later:

| Feature | Section | Rationale |
|---------|---------|-----------|
| `extern "js"` | 11.1 | WASM console support is Phase 2+ |
| C variadics | 11.6 | Can use fixed-arity wrappers |
| Callbacks | 11.6 | Not needed for termios/Console API |
| `#repr("packed")` | 11.3 | No packed structs in console APIs |
| `#repr("transparent")` | 11.3 | Convenience, not essential |
| Pointer arithmetic | 11.4 | All access via FFI calls |
| `unsafe` blocks | 11.4 | Not needed with safe CPtr API |

---

## Testing Strategy

### Unit Tests

```ori
// tests/spec/ffi/extern_blocks.ori
@test_extern_declaration tests _ () -> void = {
    // Verify extern blocks parse
    // Verify FFI capability required
}

@test_c_types tests _ () -> void = {
    // Verify c_int, c_uint, etc. exist
    // Verify sizes are correct
}

@test_repr_c tests _ () -> void = {
    // Verify #repr("c") structs have expected layout
}
```

### Integration Tests

```ori
// tests/spec/ffi/termios_integration.ori
#target(family: "unix")

@test_raw_mode tests _ () -> void uses FFI = {
    with FFI = AllowFFI in run(
        let original = enable_raw_mode(fd: 0)?,
        // ... test raw mode ...
        disable_raw_mode(fd: 0, original: original)?,
    )
}
```

---

## Success Criteria

The minimal FFI is complete when:

1. `extern "c" from "lib" { ... }` blocks parse and type-check
2. C types (`c_int`, `c_uint`, `c_long`, `c_ulong`, `c_size`, `c_ssize`, `CPtr`) work
3. `#repr("c")` structs have correct memory layout
4. `CPtr.from_struct()` and `CPtr.from_bytes()` work
5. FFI capability is enforced
6. Conditional compilation (`#target(os: ...)`) works
7. Can successfully call `tcgetattr`/`tcsetattr` on Unix
8. Can successfully call `GetConsoleMode`/`SetConsoleMode` on Windows

---

## References

- Section 11: FFI Roadmap (`plans/roadmap/section-11-ffi.md`)
- Section 13: Conditional Compilation (`plans/roadmap/section-13-conditional-compilation.md`)
- Crossterm source: `~/projects/reference_repos/console_repos/crossterm/src/terminal/sys/unix.rs`
- Rust libc crate: termios struct definitions
