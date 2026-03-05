---
title: "Conditional Compilation"
description: "Ori Compiler Design — Conditional Compilation"
order: 1501
section: "Platform Targets"
---

# Conditional Compilation

Conditional compilation is the ability to include or exclude code from a program based on conditions known at compile time. Every compiled language must eventually solve this problem: how do you write one source tree that produces correct programs for Linux, macOS, Windows, WebAssembly, debug builds, release builds, and every combination of feature flags a project might need?

This chapter examines how the Ori compiler addresses conditional compilation at two levels -- as a Rust program that must itself compile to different targets, and as a language that offers conditional compilation to its users.

## Conceptual Foundations

### The Problem

A program that must run on multiple platforms faces a fundamental tension. The logic for opening a file is different on Linux (`epoll`) than on Windows (`IOCP`) than on WebAssembly (no native file I/O at all). The naive solution -- ship all three implementations and pick at runtime -- wastes binary size, slows startup, and forces the compiler to type-check code that can never execute on the current target. Conditional compilation resolves this by making the selection at compile time, so the unused code never reaches the binary.

### A Brief History

The [C preprocessor](https://en.cppreference.com/w/c/preprocessor) introduced the first widely adopted solution in the 1970s. `#ifdef`, `#ifndef`, and `#if` operate on a textual level, splicing or removing lines before the compiler sees them. This approach is powerful but fragile: macro expansion is untyped, nesting is error-prone, and the preprocessor's notion of "code" is limited to lines of text rather than syntactic constructs.

Subsequent languages took different approaches, each making distinct tradeoffs:

| Language | Mechanism | Granularity | Type-Checks False Branch? |
|----------|-----------|-------------|---------------------------|
| C/C++ | `#ifdef` / `#if` preprocessor | Textual (line-level) | No (text is removed) |
| [Rust](https://doc.rust-lang.org/reference/conditional-compilation.html) | `#[cfg(...)]` attributes | Item-level | No |
| [Go](https://pkg.go.dev/cmd/go#hdr-Build_constraints) | `//go:build` comments | File-level | No |
| [D](https://dlang.org/spec/version.html) | `static if`, `version` | Statement-level | No |
| [Zig](https://ziglang.org/documentation/master/#comptime) | `comptime` + `@import("builtin")` | Expression-level | No (comptime branch) |
| [Swift](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/statements/#Compiler-Control-Statements) | `#if`, `#elseif`, `#else` | Statement-level | Partial (syntax-checked) |
| Python | `sys.platform` checks | Runtime | Yes (both branches) |

These designs cluster around a core question: at what granularity does the condition apply, and how much of the false branch does the compiler process?

### Two Levels of Conditional Compilation in Ori

The Ori project involves conditional compilation at two distinct levels that should not be confused:

1. **Compiler-internal**: The Ori compiler is a Rust program. It uses Rust's `#[cfg(...)]` attributes to compile *itself* differently for native and WebAssembly targets. This is Rust conditional compilation, not Ori conditional compilation.

2. **Language-level**: Ori provides `#target(...)` and `#cfg(...)` attributes for users to conditionally compile their own Ori source code. This is part of the Ori language specification.

The distinction matters because they operate at different times, on different source languages, and solve different problems. The compiler-internal level ensures the Ori toolchain can run in a browser (WASM) as well as on a developer's machine (native). The language level ensures Ori programs can target multiple platforms from a single source tree.

```flowchart TB
    A["Ori Source Code<br/>#target(os: 'linux')"]
    B["Ori Compiler<br/>(Rust program)"]
    C["Rust #[cfg(target_arch)]<br/>within compiler crates"]
    D["Compiled Ori Binary<br/>(Linux-only code included)"]

    A --> B
    C --> B
    B --> D

    classDef frontend fill:#1e3a5f,stroke:#60a5fa,color:#dbeafe
    classDef canon fill:#3b1f6e,stroke:#a78bfa,color:#e9d5ff
    classDef native fill:#5c3a1e,stroke:#f59e0b,color:#fef3c7

    class A frontend
    class B,C canon
    class D native
```

## What Makes Ori's Approach Distinctive

Ori's conditional compilation borrows ideas from Rust and Zig but combines them in a way shaped by its own design goals: expression-based syntax, mandatory testing, and capability-based effects.

**Attribute-based, not preprocessor-based.** Like Rust, Ori uses structured attributes (`#target(...)`, `#cfg(...)`) rather than textual preprocessing. This means the parser always sees well-formed syntax, and attributes attach to specific items (functions, types, constants) rather than arbitrary lines of text.

**Two orthogonal attribute families.** `#target(...)` addresses *where* the code runs (OS, architecture, platform family). `#cfg(...)` addresses *how* the code is built (debug/release mode, feature flags). This separation reflects the observation that platform and build-configuration questions are logically independent and compose through AND semantics when both appear.

**Item-level and file-level.** Attributes can be placed on individual declarations (`#target(os: "linux") @platform_func () -> void`), or on an entire file using the `#!` prefix (`#!target(os: "linux")`). File-level conditions are syntactic sugar: every item in the file is treated as if it bore the same attribute.

**Parsed but not type-checked.** Code in false conditions is fully parsed (syntax errors are always reported) but not type-checked. This follows Rust's model and is a deliberate middle ground between C (which does not even parse the false branch) and Python (which type-checks everything). Parsing the false branch catches syntax drift; skipping type-checking avoids requiring platform-specific types to exist in all build contexts.

**Compile-time constants as an alternative.** In addition to attributes, Ori provides compile-time constants (`$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release`) that can be used in ordinary `if` expressions. When a branch condition is a compile-time constant comparison, dead branch elimination applies: the false branch is not type-checked, just as with attribute-based conditions.

## Compiler-Internal Conditional Compilation

The Ori compiler itself must run on native platforms (x86_64, aarch64) and on WebAssembly (for the browser-based playground). Rust's `#[cfg(...)]` attributes handle this.

### The `ori_stack` Crate

The most visible use of compiler-internal conditional compilation is the `ori_stack` crate, which provides stack safety for deeply recursive operations (parsing, type inference, evaluation).

On native targets, `ori_stack` wraps the [`stacker`](https://crates.io/crates/stacker) crate to dynamically grow the call stack. When the remaining stack space drops below a 100KB red zone, `stacker::maybe_grow` allocates 1MB of additional stack space. This allows the interpreter to handle arbitrarily deep recursion limited only by available memory.

On WebAssembly, the stack cannot be grown at runtime. The WASM execution environment provides a fixed stack (typically around 1MB in browsers), and there is no system call to extend it. The `ori_stack` crate compiles to a no-op passthrough on WASM:

```rust
#[cfg(not(target_arch = "wasm32"))]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(RED_ZONE, STACK_PER_RECURSION, f)
}

#[cfg(target_arch = "wasm32")]
pub fn ensure_sufficient_stack<R>(f: impl FnOnce() -> R) -> R {
    f()
}
```

The key design principle here is **API parity**: both versions of `ensure_sufficient_stack` have the same signature and semantics (from the caller's perspective). The caller wraps recursive calls without knowing or caring about the target platform.

### Recursion Limits via `EvalMode`

Stack safety involves a second mechanism: recursion depth limits. Rather than scattering `#[cfg]` attributes throughout the interpreter, Ori pushes the platform difference into a single policy method on `EvalMode`:

```rust
pub fn max_recursion_depth(&self) -> Option<usize> {
    match self {
        Self::Interpret => {
            #[cfg(target_arch = "wasm32")]
            { Some(200) }
            #[cfg(not(target_arch = "wasm32"))]
            { None }
        }
        Self::ConstEval { .. } => Some(64),
        Self::TestRun { .. } => Some(500),
    }
}
```

On native, `Interpret` mode returns `None` (no limit -- `stacker` handles stack growth). On WASM, it returns `Some(200)` to fail gracefully before the browser's stack overflows. The interpreter's `check_recursion_limit()` method reads this value without any `#[cfg]` of its own:

```rust
pub(crate) fn check_recursion_limit(&self) -> Result<(), EvalError> {
    if let Some(max_depth) = self.mode.max_recursion_depth() {
        if self.call_stack.depth() >= max_depth {
            return Err(recursion_limit_exceeded(max_depth));
        }
    }
    Ok(())
}
```

This pattern -- **unified code with cfg-gated data** -- is the compiler's preferred approach. One implementation, one control flow path, with the platform difference pushed into a data value that a single `#[cfg]` method provides.

### Linker Flavor Selection

The LLVM backend selects linkers using enum dispatch rather than `#[cfg]`. `LinkerFlavor::for_target()` maps a `TargetTripleComponents` to the appropriate linker family:

```rust
pub enum LinkerFlavor { Gcc, Lld, Msvc, WasmLd }

impl LinkerFlavor {
    pub fn for_target(target: &TargetTripleComponents) -> Self {
        if target.is_wasm() { Self::WasmLd }
        else if target.is_windows()
             && target.env.as_deref() == Some("msvc") { Self::Msvc }
        else { Self::Gcc }
    }
}
```

This is not `#[cfg]`-based conditional compilation -- the compiler binary contains all linker drivers. The selection is a runtime decision based on the user's `--target` flag. The distinction is significant: compiler-internal `#[cfg]` controls what code exists in the compiler binary itself; linker selection controls what the compiler does when it runs.

### Summary of Compiler-Internal Uses

| Crate | Mechanism | Purpose |
|-------|-----------|---------|
| `ori_stack` | `#[cfg(target_arch = "wasm32")]` | `stacker` vs no-op passthrough |
| `ori_eval` | `#[cfg]` inside `EvalMode` | WASM recursion limit (200) vs unlimited |
| `ori_llvm` | Enum dispatch on `TargetTripleComponents` | Linker and sysroot selection |
| `playground-wasm` | WASM-only crate | JavaScript interop bindings |

## Language-Level Conditional Compilation

The Ori language provides two attribute families that users place on their own source code.

### The `TargetAttr` Structure

The `#target(...)` attribute maps to a `TargetAttr` in the AST, with fields for each condition axis:

```rust
pub struct TargetAttr {
    pub os: Option<Name>,        // #target(os: "linux")
    pub arch: Option<Name>,      // #target(arch: "x86_64")
    pub family: Option<Name>,    // #target(family: "unix")
    pub any_os: Vec<Name>,       // #target(any_os: ["linux", "macos"])
    pub not_os: Option<Name>,    // #target(not_os: "windows")
}
```

Each field corresponds to a condition keyword in the attribute syntax. When multiple fields are set (e.g., `#target(os: "linux", arch: "x86_64")`), all conditions must match -- AND semantics. The `any_os` field is the one exception: it matches if the target OS is any value in the list (OR semantics within that single field).

Multiple `#target` attributes on the same item also combine with AND:

```ori
#target(os: "linux")
#target(arch: "x86_64")
@linux_x64_only () -> void = ...;
```

### The `CfgAttr` Structure

The `#cfg(...)` attribute maps to a `CfgAttr`:

```rust
pub struct CfgAttr {
    pub debug: bool,              // #cfg(debug)
    pub release: bool,            // #cfg(release)
    pub not_debug: bool,          // #cfg(not_debug)
    pub feature: Option<Name>,    // #cfg(feature: "ssl")
    pub any_feature: Vec<Name>,   // #cfg(any_feature: ["ssl", "tls"])
    pub not_feature: Option<Name>, // #cfg(not_feature: "ssl")
}
```

Unlike `TargetAttr` (which takes string values), `CfgAttr` includes bare boolean flags (`debug`, `release`, `not_debug`) that require no value argument. Feature flags use the named `feature:` syntax with a string value.

### File-Level Attributes

The `#!` prefix applies a condition to an entire file. The parser represents this as a `FileAttr` enum on the `Module` AST node:

```rust
pub enum FileAttr {
    Target { attr: TargetAttr, span: Span },
    Cfg { attr: CfgAttr, span: Span },
}

pub struct Module {
    pub file_attr: Option<FileAttr>,
    pub imports: Vec<UseDef>,
    pub functions: Vec<Function>,
    // ...
}
```

File-level attributes must appear before any imports or declarations. The parser enforces this ordering -- placing `#!target(...)` after a `use` statement produces error E0933. Only `target` and `cfg` are valid at file level; attempting to use other attributes (like `#!derive(...)`) produces a parse error.

### Applicable Items

Conditional attributes can be placed on every item type in the language:

| Item | Syntax |
|------|--------|
| Functions | `#target(os: "linux") @platform_func () -> void = ...` |
| Types | `#target(os: "windows") type Handle = int` |
| Trait implementations | `#target(os: "linux") impl Socket: FileDescriptor { ... }` |
| Constants | `#cfg(debug) let $log_level = "debug"` |
| Imports | `#target(os: "linux") use "./linux/io" { epoll_create }` |

### Parser Implementation

The parser handles attributes through a single entry point, `parse_attributes()`, which loops over `#` tokens and dispatches to specialized parsers based on the attribute name. For `target` and `cfg`, the dispatch calls `parse_target_attr_body()` and `parse_cfg_attr_body()` respectively, which parse the parenthesized argument lists into the corresponding AST structures.

File-level attributes use a separate entry point, `parse_file_attribute()`, which looks for the `#!` token kind (distinct from `#`). This method reuses the same `parse_target_attr_body()` and `parse_cfg_attr_body()` methods, wrapping their results in a `FileAttr` with a merged span.

```flowchart TB
    A["parse_file_attribute()"]
    B["parse_attributes()"]
    C["parse_attr_name()"]
    D["parse_target_attr_body()"]
    E["parse_cfg_attr_body()"]
    F["TargetAttr"]
    G["CfgAttr"]
    H["FileAttr::Target"]
    I["FileAttr::Cfg"]
    J["ParsedAttrs.target"]
    K["ParsedAttrs.cfg"]

    A --> C
    B --> C
    C -->|"target"| D
    C -->|"cfg"| E
    D --> F
    E --> G
    F -->|file-level| H
    F -->|item-level| J
    G -->|file-level| I
    G -->|item-level| K

    classDef frontend fill:#1e3a5f,stroke:#60a5fa,color:#dbeafe
    classDef canon fill:#3b1f6e,stroke:#a78bfa,color:#e9d5ff

    class A,B,C,D,E frontend
    class F,G,H,I,J,K canon
```

### Parse-but-Not-Type-Check Semantics

When a condition evaluates to false, the decorated item is parsed (syntax errors are still reported) but not type-checked, evaluated, or emitted into the binary. This design has several consequences:

- Syntax errors in platform-specific code are caught on every platform.
- Type errors in platform-specific code are only caught when building for that platform.
- Platform-specific types (like `WindowsApi` or `epoll_event`) do not need to exist in all build contexts.
- Dead code from false conditions contributes zero bytes to the output binary.

## Compile-Time Constants

Ori provides five built-in compile-time constants that expose target and build information:

| Constant | Type | Source |
|----------|------|--------|
| `$target_os` | `str` | `TargetTripleComponents.os` |
| `$target_arch` | `str` | `TargetTripleComponents.arch` |
| `$target_family` | `str` | `TargetTripleComponents.family()` |
| `$debug` | `bool` | Build profile |
| `$release` | `bool` | Build profile |

These constants can be used in ordinary expressions:

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/";
```

When the condition of an `if` expression is a comparison against a compile-time constant, the compiler applies dead branch elimination. The false branch is not type-checked, giving compile-time constants the same power as `#target`/`#cfg` attributes but with the ergonomics of regular expressions:

```ori
@get_window_handle () -> WindowHandle =
    if $target_os == "windows" then
        WinApi.get_hwnd()     // Only type-checked on Windows
    else
        panic(msg: "Not supported on this platform");
```

The constants are populated from the same `TargetTripleComponents` structure that the LLVM backend uses for linker selection and sysroot discovery, ensuring consistency between the compiler's target knowledge and the values visible to user code.

## Error Codes

The conditional compilation system defines four diagnostic error codes:

| Code | Condition | Message |
|------|-----------|---------|
| **E0930** | Invalid target OS | The `os:` value in `#target(os: "...")` is not a recognized operating system name (linux, macos, windows, freebsd, android, ios). |
| **E0931** | Invalid target architecture | The `arch:` value in `#target(arch: "...")` is not a recognized architecture name (x86_64, aarch64, arm, wasm32, riscv64). |
| **E0932** | Invalid feature name | The `feature:` value in `#cfg(feature: "...")` is not a valid Ori identifier (must start with a letter or underscore, contain only letters, digits, and underscores). |
| **E0933** | File-level condition placement | A `#!target(...)` or `#!cfg(...)` attribute appears after imports or declarations instead of at the beginning of the file. |

These diagnostics are emitted during parsing (E0933) or during condition evaluation (E0930, E0931, E0932), ensuring that typos in platform names or malformed feature flags are caught early.

## Prior Art

Ori's conditional compilation draws from a well-established lineage.

**C/C++ preprocessor** ([cppreference](https://en.cppreference.com/w/c/preprocessor)). The original `#ifdef` / `#if` system operates on text before parsing. Powerful but untyped: the preprocessor has no knowledge of syntax, types, or scoping. Errors in false branches are invisible until the condition becomes true on some platform. The `#ifdef` model also makes tooling (IDEs, linters, formatters) significantly harder because the tool must model preprocessor state to parse the file.

**Rust `#[cfg]`** ([Rust Reference](https://doc.rust-lang.org/reference/conditional-compilation.html)). The most direct influence on Ori's design. Attributes attach to items (functions, modules, structs). False branches are parsed but not type-checked. Boolean algebra (`all`, `any`, `not`) composes conditions. Ori simplifies this by using named parameters (`os:`, `arch:`) instead of nested predicate syntax (`cfg(target_os = "linux")`).

**Go build constraints** ([Go specification](https://pkg.go.dev/cmd/go#hdr-Build_constraints)). Go uses `//go:build` comments and file-name suffixes (`_linux.go`, `_amd64.go`) for conditional compilation. The granularity is the entire file -- there is no way to conditionally compile a single function within a file. This pushes developers toward one-file-per-platform organization. Ori provides both file-level (`#!target`) and item-level (`#target`) granularity.

**Zig `comptime`** ([Zig documentation](https://ziglang.org/documentation/master/#comptime)). Zig takes the most radical approach: arbitrary compile-time execution. `if (comptime builtin.os.tag == .linux)` evaluates at compile time, and the false branch is not analyzed. This is expression-level granularity with the full power of the language available at compile time. Ori's compile-time constants (`$target_os`, `$debug`) occupy a similar niche but with a more constrained model.

**D `static if` and `version`** ([D specification](https://dlang.org/spec/version.html)). D provides `version(linux)` blocks and `static if` for compile-time conditional compilation. `static if` can appear at any scope level and can test arbitrary compile-time expressions. Ori's attribute model is more restrictive (item-level, not block-level) but avoids the complexity of D's scope interaction rules.

**Swift `#if`** ([Swift documentation](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/statements/#Compiler-Control-Statements)). Swift's `#if os(Linux)` blocks can appear at the statement level. The false branch is syntax-checked but not type-checked. Ori follows the same parse-but-don't-type-check model.

## Design Tradeoffs

### Attributes vs Preprocessor

Ori uses structured attributes rather than textual preprocessing. This means the parser always operates on syntactically complete Ori code -- there is no phase before parsing that could splice arbitrary text. The benefit is that tooling (formatters, IDEs, linters) can parse every file without simulating preprocessor state. The cost is reduced flexibility: you cannot conditionally compile half of an expression or vary the shape of a type declaration based on a condition. Every conditioned unit must be a complete item (function, type, constant, import).

### Item-Level vs Block-Level Granularity

Some languages (D, Swift) allow conditional compilation at the statement or block level. Ori restricts conditions to items and files. This simplifies the interaction between conditional compilation and scoping -- a `let` binding inside a false branch never exists, so there is no question about what names are in scope after the conditional block. The tradeoff is that platform-specific logic within a function body must use compile-time constants (`if $target_os == "windows"`) rather than attributes, or the function must be split into platform-specific variants.

### Parsing False Branches

Ori parses code in false conditions but does not type-check it. This is a middle ground between two extremes:

- **C preprocessor**: False branches are not even tokenized. Syntax errors lurk undetected until the condition becomes true, potentially years later on a different platform.
- **Full analysis**: Both branches are fully type-checked. This requires all types, functions, and modules to exist in all build configurations, which defeats much of the purpose of conditional compilation.

Parsing the false branch catches syntax drift (renamed keywords, changed delimiters) without requiring platform-specific types to exist. The tradeoff is that type-level errors in rarely-built configurations (embedded targets, exotic architectures) can still go undetected for long periods.

### Separate `target` and `cfg` vs Unified Syntax

Ori separates platform conditions (`#target`) from build-configuration conditions (`#cfg`). Rust, by contrast, puts everything under a single `#[cfg(...)]` with sub-predicates (`target_os`, `debug_assertions`, `feature`). Ori's separation makes the two concerns visually distinct: a `#target` attribute is always about the deployment platform, a `#cfg` attribute is always about build settings. The cost is two attribute names to learn instead of one, and the inability to express mixed conditions (e.g., "Linux AND debug mode") in a single attribute -- these require two separate attributes that combine with AND semantics.
