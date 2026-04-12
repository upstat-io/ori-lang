# Proposal: Entry Point Requirements and Diagnostics

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-04-12
**Affects:** Spec (Clause 23, Clause 19), compiler (`ori_llvm`, `oric`), grammar
**Depends On:** None

---

## Summary

Tighten Clause 23 (Program Execution) to define compiler behavior when `@main` is absent. Currently the spec requires `@main` for "executable programs" but does not define (1) the distinction between a _compilation unit_ and an _executable program_, (2) what diagnostic the compiler shall produce when `@main` is missing, or (3) whether test-only files can be compiled into test executables. This proposal closes all three gaps.

---

## Motivation

### The Problem in Practice

A user writes a test file:

```ori
use std.testing { assert_eq }

@join_ints () -> str = {
    [1, 2, 3].iter().join(separator: ", ")
}

@test_join_ints tests @join_ints () -> void = {
    assert_eq(actual: join_ints(), expected: "1, 2, 3")
}
```

They want to build it for AOT debugging (leak checking, valgrind):

```bash
ori build tests/spec/traits/iterator/join.ori -o /tmp/join_test
```

**Current behavior:** The compiler silently proceeds through compilation, emits an object file, invokes the linker, and the linker fails with:

```
(.text+0x1b): undefined reference to `main'
collect2: error: ld returned 1 exit status
```

This is a poor diagnostic. The user sees a linker error with zero context about what went wrong or how to fix it. The compiler _knew_ `@main` was missing (it logs at debug level) but chose not to tell the user.

### When This Matters

1. **AOT debugging workflows** — building test files for valgrind, leak checking, profiling
2. **New users** — forgetting `@main` or confusing `ori build` with `ori test`
3. **CI pipelines** — `ori build` on the wrong file produces an opaque linker error instead of a structured compiler diagnostic

### Root Cause

The spec says "Every executable program shall have exactly one `@main` function" (Clause 23.1) but:

- Does not define what a "compilation unit" is vs. an "executable program"
- Does not require the compiler to produce a diagnostic when `@main` is absent
- Does not address whether test-only files (files with `@test` annotations but no `@main`) can be built as executables
- Does not define whether `ori build` should handle test files differently from program files

---

## Design

### 1. Define Compilation Unit vs. Executable Program

Add to Clause 23.1:

> A _compilation unit_ is any `.ori` source file or set of source files presented to the compiler. A compilation unit becomes an _executable program_ only when it contains exactly one `@main` function with a valid entry-point signature.

### 2. Require Compiler Diagnostic for Missing Entry Point

Add to Clause 23.1:

> When `ori build` is invoked on a compilation unit that does not contain an `@main` function, the compiler shall produce a diagnostic error before the linking phase. The diagnostic shall:
>
> (a) identify the absence of `@main` as the error condition,
> (b) list the valid `@main` signatures, and
> (c) if the compilation unit contains `@test` declarations, suggest `ori test` as an alternative.

**Proposed error format:**

```
error[E5001]: no `@main` entry point found

  This file cannot be compiled into an executable because it does not
  define an `@main` function.

  Valid signatures:
    @main () -> void
    @main () -> int
    @main (args: [str]) -> void
    @main (args: [str]) -> int

  help: this file contains test declarations — did you mean `ori test`?
```

### 3. Test Executable Generation

Add a new subclause 23.1.1:

> When `ori build --test` is invoked on a compilation unit containing `@test` declarations, the compiler shall generate a test runner entry point that discovers and executes all `@test` functions in the compilation unit, producing a native test executable. The generated entry point shall behave identically to `ori test` but produce a standalone binary.

This enables AOT debugging workflows:

```bash
ori build --test tests/spec/traits/iterator/join.ori -o /tmp/join_test
ORI_CHECK_LEAKS=1 /tmp/join_test
valgrind /tmp/join_test
```

**When `--test` is provided:**
- The compiler generates a synthetic `main` that invokes the test runner
- All `@test` functions are compiled and linked
- The binary returns 0 on all-pass, non-zero on any failure
- `@main`, if present, is ignored (tests take priority)

**When `--test` is NOT provided:**
- `@main` is required (existing behavior, now with diagnostic)
- `@test` functions are compiled but not linked into the entry point
- `@test` declarations are still type-checked regardless

### Error Code Assignment

| Code | Condition |
|------|-----------|
| E5001 | `ori build` without `--test` on a compilation unit with no `@main` |

### Semantics

- `ori build file.ori` — requires `@main`, produces executable
- `ori build --test file.ori` — requires `@test`, produces test executable
- `ori build --test file.ori` with no `@test` — error: no test declarations found
- `ori test file.ori` — unchanged (interpreter/JIT test runner)
- `ori check file.ori` — unchanged (type-checks only, no entry point requirement)

---

## Alternatives Considered

### Alternative 1: Only Add the Diagnostic (No `--test`)

Just require `@main` for `ori build` and add a clear error message. Users who want AOT test executables write a wrapper file with `@main`.

**Rejected because:** Forces boilerplate for a common debugging workflow. Every test file would need a companion `@main` wrapper for AOT debugging. The `--test` flag is a small addition with high ergonomic value.

### Alternative 2: Auto-Detect Test Files

If `@main` is absent but `@test` is present, automatically generate a test runner without requiring `--test`.

**Rejected because:** Implicit behavior is surprising. A user who forgot `@main` would get a test executable instead of an error. Explicit `--test` makes intent clear.

### Alternative 3: `ori test --aot`

Add an `--aot` flag to `ori test` instead of `--test` flag to `ori build`.

**Viable alternative.** This keeps `ori build` strictly for programs and adds AOT capability to `ori test`. However, it conflates the test runner's responsibilities (discovery, execution, reporting) with compilation concerns (linking, optimization, debug info). `ori build --test` is more composable — it produces a binary that can be used with any external tool (valgrind, perf, gdb).

---

## Purity Analysis

**Can be pure Ori?** NO
**If not, why:** This is about compiler behavior (diagnostics, entry point generation) and CLI semantics — inherently compiler concerns.
**Recommendation:** Proceed as compiler/spec change.

---

## Spec & Grammar Impact

| Artifact | Change |
|----------|--------|
| Clause 23.1 | Add compilation unit definition, diagnostic requirement, valid entry-point enumeration |
| Clause 23.1.1 (new) | Test executable generation via `--test` |
| Clause 19 | Cross-reference to 23.1.1 for test executable semantics |
| Grammar | No syntax changes — `--test` is a CLI flag, not language syntax |
| Error codes | New E5001 (missing entry point) |

---

## Prior Art

| Language | `build` on test file | Behavior |
|----------|---------------------|----------|
| **Rust** | `cargo build` on lib with `#[test]` | Builds lib only; `cargo test` generates test binary |
| **Go** | `go build` on `_test.go` | Ignores test files; `go test -c` produces test binary |
| **Zig** | `zig build-exe` on test file | Error: no entry point; `zig test` generates test binary |
| **Swift** | `swiftc` on XCTest file | Linker error; `swift test` generates test binary |

All four languages require an explicit "test build" invocation and do not silently accept test-only files as regular executables. Ori's proposed `ori build --test` aligns with Go's `go test -c` pattern.
