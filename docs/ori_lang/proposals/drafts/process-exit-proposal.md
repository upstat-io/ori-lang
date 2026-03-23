# Proposal: Process Module & Exit Codes

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-16
**Affects:** Compiler (entry point codegen), stdlib, capabilities

---

## Summary

`@main () -> int` conflates "I computed a value" with "I'm setting process status." This proposal introduces a `std.process` module with `exit(code:)` as its first function, and reconsiders the role of `@main`'s return value.

This is intentionally the **first OS interaction surface** in Ori. The design must establish patterns that future OS modules (`std.env`, `std.fs`, etc.) will follow.

---

## Motivation

### The Problem

Code journeys return computed values from `@main () -> int`:

```ori
@main () -> int = {
    let result = (3 + 4) * 5 - 2   // = 33
    result
}
```

This program exits with code 33, which:
- Breaks tooling (Valgrind, CI, `ORI_CHECK_LEAKS`) that interprets non-zero as failure
- Violates POSIX convention (0 = success, non-zero = failure)
- Makes it impossible to distinguish "program returned data" from "program failed"

### Prior Art

| Language | Main signature | Exit function | Cleanup on exit? |
|----------|---------------|---------------|-------------------|
| **Rust** | `fn main() -> ExitCode` | `std::process::exit(i32)` | atexit only, no destructors |
| **Go** | `func main()` (void only) | `os.Exit(int)` | deferred functions NOT run |
| **Zig** | `pub fn main() void` or `u8` | `std.process.exit(u8)` | no cleanup |
| **Node.js** | N/A | `process.exit(code?)` | exit handlers run |
| **Python** | N/A | `sys.exit(code)` | finally/atexit run |
| **Swift** | `static func main()` | C `exit()` | no language-level wrapper |

**Key patterns:**
- Most languages make `main` return `void` as the default/common case
- Non-zero exit codes are set explicitly via a function call
- Go is the most opinionated: `main` is always void, `os.Exit()` is the only way
- Rust offers both paths (return `ExitCode` or call `process::exit()`)

---

## Design

### 1. `std.process` Module

```ori
use std.process { exit }
```

This is the first module in what will eventually be a broader OS interaction surface. Future modules might include `std.env`, `std.fs`, `std.io`, etc. — but this proposal only covers `std.process.exit`.

### 2. `exit` Function

```ori
// std/process.ori

/// Immediately terminate the process with the given exit code.
///
/// Exit code 0 indicates success. Any non-zero value indicates failure.
/// This function does not return — the process terminates immediately.
///
/// Cleanup behavior: ARC decrements for in-scope variables are NOT run.
/// Use this for abnormal termination. For normal termination, return
/// from `@main` instead.
@exit (code: int) -> Never uses Process
```

**Key decisions:**

- **Returns `Never`** — it doesn't return, same as `panic()`
- **`uses Process`** capability — OS interaction is an effect. This follows Ori's capability model and makes `exit` mockable in tests via `with Process = mock in { ... }`
- **`int` parameter** — full range, OS masks to 8 bits on POSIX. We don't restrict at the language level.
- **No cleanup** — immediate termination. ARC decrements for in-scope variables are skipped. This is the "emergency exit." For clean shutdown, return from `@main`.

### 3. `@main` Return Value Semantics (Unchanged For Now)

Current Ori supports four `@main` signatures:

```ori
@main () -> void                    // exit code 0
@main () -> int                     // return value = exit code
@main (args: [str]) -> void         // exit code 0, with args
@main (args: [str]) -> int          // return value = exit code, with args
```

**This proposal does NOT change these signatures.** The `-> int` form remains valid for programs that genuinely want to return an exit code. But the existence of `process.exit()` means:

- Programs that compute values should use `-> void` + `print()`
- Programs that need to signal failure should use `exit(code: 1)` or `-> int`
- The `-> int` form is a convenience, not the primary mechanism

### 4. Capability: `Process`

```ori
// In prelude or std.process
capability Process
```

`Process` is a new standard capability covering OS process interaction. Initially it only gates `exit`, but future functions like `process.id()`, `process.args()` (if not using `@main (args:)`), `process.env()` could live under it.

**Open question:** Should `Process` be a single capability, or should it be split? e.g.:
- `Process` — exit, pid, args
- `Env` — environment variables
- `FileSystem` — file I/O

The capability-per-resource model is more granular. But `exit` specifically is hard to categorize — it's not I/O, it's process lifecycle. `Process` seems right.

---

## Usage Examples

### Normal program (common case)

```ori
@main () -> void = {
    let result = compute_something()
    print(msg: result.to_str())
}
```

### Error exit

```ori
use std.process { exit }

@main () -> void uses Process = {
    let config = load_config()
    if config.is_err() then {
        print(msg: "Error: invalid config")
        exit(code: 1)
    }
    run_app(config: config.unwrap())
}
```

### Exit code via return (still valid)

```ori
@main () -> int = {
    let ok = run_checks()
    if ok then 0 else 1
}
```

### Testable with capability mocking

```ori
@run (args: [str]) -> void uses Process = {
    if args.is_empty() then {
        print(msg: "No args")
        exit(code: 1)
    }
    // ...
}

@t tests @run () -> void = {
    // exit(code:) is mockable via Process capability
    with Process = handler(state: ()) {
        exit: (s, code) -> (s, panic(msg: "unexpected exit")),
    } in {
        run(args: ["hello"])
    }
}
```

---

## Broader Context: First OS Surface

This proposal opens the door to OS interaction in Ori. Design principles established here should carry forward:

1. **Capabilities gate all OS interaction** — no raw syscalls without declaring effects
2. **Mockable by default** — `with Cap = mock in { ... }` works for all OS functions
3. **Module organization** — `std.process`, `std.env`, `std.fs`, `std.io` (not one giant `std.os`)
4. **`Never` for non-returning** — `exit`, `abort` return `Never`
5. **Explicit over implicit** — prefer `exit(code: 1)` over overloading return values

### Future Modules (Not In This Proposal)

These are listed only to show the pattern, not to propose them:

| Module | Examples | Capability |
|--------|----------|------------|
| `std.process` | `exit`, `id`, `abort` | `Process` |
| `std.env` | `get`, `set`, `vars` | `Env` (already exists) |
| `std.fs` | `read`, `write`, `exists` | `FileSystem` (already exists) |
| `std.io` | `stdin`, `stdout`, `stderr` | `Print` (already exists) |

---

## Implementation Notes

### Runtime

`exit` maps directly to the C `exit()` function (or `_exit()` for no-cleanup semantics):

```
extern "c" from "c" {
    @_exit (status: c_int) -> Never as "_exit"
}
```

Or implemented in `ori_rt`:

```rust
#[no_mangle]
pub extern "C" fn ori_process_exit(code: i32) -> ! {
    std::process::exit(code);
}
```

### Codegen

`exit` is a call to a runtime function that never returns. LLVM can mark it `noreturn` for optimization.

### Interaction with `ORI_CHECK_LEAKS`

When `exit(code:)` is called, `ORI_CHECK_LEAKS` should still report leaks before terminating. The implementation should call `ori_check_leaks()` before `_exit()` to preserve leak detection in test environments. This means the actual implementation is:

```rust
#[no_mangle]
pub extern "C" fn ori_process_exit(code: i32) -> ! {
    // Report leaks before exiting (if ORI_CHECK_LEAKS=1)
    let leak_code = check_leaks_and_exit();
    // Leak code takes precedence (same as main wrapper)
    let final_code = if leak_code != 0 { leak_code } else { code };
    std::process::exit(final_code);
}
```

---

## Open Questions

1. **Should `@main () -> int` be deprecated?** It works, but `exit(code:)` is clearer. Could emit a lint suggesting `-> void` + `exit()` when the return value isn't 0 or 1.

2. **Cleanup semantics:** Should `exit()` run ARC decrements for in-scope variables? Go says no (defers skipped). Rust says no (destructors skipped). The safe default is no cleanup — it's the "pull the plug" function.

3. **`abort()` vs `exit()`:** Should there be a separate `abort()` (signal-based, core dump) vs `exit()` (clean process termination)? Most languages have both. Could be added later.

4. **Capability granularity:** Is `Process` the right scope, or should `exit` have its own `Exit` capability since it's so drastic?

---

## Migration

No breaking changes. `@main () -> int` continues to work. `std.process.exit` is additive.

Code journeys and test programs should migrate to `@main () -> void` + `print()` for their computed values, using `exit(code:)` only when genuinely signaling failure.
