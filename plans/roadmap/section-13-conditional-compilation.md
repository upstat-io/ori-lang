---
section: 13
title: Conditional Compilation
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 5
goal: Enable platform-specific code and feature flags
spec:
  - spec/25-conditional-compilation.md
sections:
  - id: "13.1"
    title: Target Attribute
    status: partial
  - id: "13.2"
    title: OR Conditions
    status: partial
  - id: "13.3"
    title: Negation
    status: partial
  - id: "13.4"
    title: Cfg Attribute
    status: partial
  - id: "13.5"
    title: Feature Flags
    status: not-started
  - id: "13.6"
    title: File-Level Conditions
    status: partial
  - id: "13.7"
    title: Compile-Time Constants
    status: not-started
  - id: "13.8"
    title: Build Configuration
    status: not-started
  - id: "13.9"
    title: Diagnostics
    status: partial
  - id: "13.10"
    title: compile_error Built-in
    status: not-started
---

# Section 13: Conditional Compilation

**Goal**: Enable platform-specific code and feature flags

**Criticality**: High — Required for cross-platform support and feature management

**Proposal**: `proposals/approved/conditional-compilation-proposal.md`

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Syntax | `#target(...)` and `#cfg(...)` | Separate platform from features, clear intent |
| File-level | `#!target(...)` | Directive at top of file |
| OR conditions | `any_os`, `any_arch`, `any_feature` | Essential for real cross-platform code |
| Negation | `not_*` prefix | `not_os`, `not_debug`, `not_feature` — consistent |
| Families | `family: "unix"/"windows"/"wasm"` | Group related platforms |
| DCE | Full elimination | False branches not type-checked |
| Feature names | Valid identifiers only | Consistent with Ori naming |

---

## Reference Implementation

### Rust

```
~/projects/reference_repos/lang_repos/rust/compiler/rustc_attr_parsing/src/cfg.rs  # cfg parsing
~/projects/reference_repos/lang_repos/rust/compiler/rustc_expand/src/cfg.rs        # cfg evaluation
```

### Go

```
~/projects/reference_repos/lang_repos/golang/src/go/build/constraint/             # Build constraints
~/projects/reference_repos/lang_repos/golang/src/cmd/go/internal/work/build.go    # Build tags handling
```

### Swift

```
# Swift uses #if os(...), #if arch(...), #if canImport(...)
~/projects/reference_repos/lang_repos/swift/lib/Parse/ParseIfConfig.cpp           # #if config parsing
~/projects/reference_repos/lang_repos/swift/lib/AST/PlatformKind.cpp              # Platform enumeration
```

---

## 13.1 Target Attribute

**Spec section**: `spec/25-conditional-compilation.md § Target Attribute`

### Syntax

```ori
// Operating system
#target(os: "linux")
@linux_specific () -> void = ...

#target(os: "windows")
@windows_specific () -> void = ...

// Architecture
#target(arch: "x86_64")
@x64_specific () -> void = ...

// Target families
#target(family: "unix")
@unix_like () -> void = ...

// Combined (AND)
#target(os: "linux", arch: "x86_64")
@linux_x64 () -> void = ...
```

### Implementation

- [x] **Spec**: Add `spec/25-conditional-compilation.md` (verified 2026-03-29)
  - [x] Target attribute syntax — `docs/ori_lang/v2026/spec/25-conditional-compilation.md` exists
  - [x] OS, arch, family values
  - [ ] Scope rules

- [x] **Lexer/Parser**: Parse target attributes (verified 2026-03-29)
  - [x] `#target(...)` syntax — `parse_target_attr_body()` in `ori_parse/src/grammar/attr/conditional.rs:32`
  - [x] Named arguments: `os:`, `arch:`, `family:`, `not_os:`, `not_arch:`, `not_family:`, `any_os:`, `any_arch:` — all parsed
  - [x] Apply to items — stored on `FunctionDef::target_attr`, `TypeDecl::target_attr`, imports, impl blocks

- [ ] **Compiler**: Target evaluation
  - [ ] Evaluate against build target — no target evaluation exists
  - [ ] Prune false branches from AST
  - [ ] Track for error messages
  - [x] **Rust Tests**: Target attribute parsing — 27 parser tests in `compiler/oric/tests/phases/parse/file_attr.rs` (verified 2026-03-29)

- [ ] **Ori Tests**: `tests/spec/conditional/target_basic.ori` — NEEDS TESTS: file does not exist
  - [ ] OS-specific code
  - [ ] Arch-specific code
  - [ ] Family-specific code
  - [ ] Combined conditions

- [ ] **LLVM Support**: LLVM codegen for target attribute
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — target attribute codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.2 OR Conditions

**Spec section**: `spec/25-conditional-compilation.md § OR Conditions`

### Syntax

```ori
// Match any OS in list
#target(any_os: ["linux", "macos", "freebsd"])
@unix_variants () -> void = ...

// Match any architecture
#target(any_arch: ["x86_64", "aarch64"])
@desktop_arch () -> void = ...
```

### Implementation

- [ ] **Spec**: OR condition semantics
  - [ ] `any_os`, `any_arch`, `any_family`
  - [ ] List syntax

- [x] **Parser**: Parse any_* variants (verified 2026-03-29)
  - [x] Array literal values — `any_os`/`any_arch` with list syntax in `conditional.rs:102-103`
  - [x] Validate all elements are strings — `parse_attr_string_list()` handles comma-separated strings

- [ ] **Evaluator**: Evaluate OR conditions
  - [ ] Match if any element matches
  - [ ] **Rust Tests**: OR condition evaluation — parser tests exist but no evaluator tests

- [ ] **Ori Tests**: `tests/spec/conditional/target_or.ori` — NEEDS TESTS: file does not exist
  - [ ] any_os conditions
  - [ ] any_arch conditions
  - [ ] Combined with AND

- [ ] **LLVM Support**: LLVM codegen for OR conditions
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — OR conditions codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.3 Negation

**Spec section**: `spec/25-conditional-compilation.md § Negation`

### Syntax

```ori
// Negated conditions
#target(not_os: "windows")
@non_windows () -> void = ...

#target(not_family: "wasm")
@native_only () -> void = ...

#cfg(not_debug)
@release_only () -> void = ...

#cfg(not_feature: "ssl")
@insecure_fallback () -> void = ...
```

### Implementation

- [ ] **Spec**: Negation semantics
  - [ ] `not_*` prefix for all condition types
  - [ ] Interaction with OR conditions

- [x] **Parser**: Parse not_* variants (verified 2026-03-29)
  - [x] Recognize all negation forms — `not_os`, `not_arch`, `not_family` in `conditional.rs:99-101`; `not_debug`, `not_feature` in `conditional.rs:173-175`; `not_debug` as bare identifier at `conditional.rs:193`
  - [x] IR fields: `not_os`, `not_arch`, `not_family` on `TargetAttr`; `not_debug`, `not_feature` on `CfgAttr`

- [ ] **Evaluator**: Evaluate negation
  - [ ] Boolean NOT of underlying condition
  - [ ] **Rust Tests**: Negation evaluation — parser tests exist but no evaluator tests

- [ ] **Ori Tests**: `tests/spec/conditional/negation.ori` — NEEDS TESTS: file does not exist
  - [ ] not_os, not_arch, not_family
  - [ ] not_debug, not_release
  - [ ] not_feature

- [ ] **LLVM Support**: LLVM codegen for negation
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — negation codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.4 Cfg Attribute

**Spec section**: `spec/25-conditional-compilation.md § Cfg Attribute`

### Syntax

```ori
// Build mode flags
#cfg(debug)
@debug_only () -> void = ...

#cfg(release)
@release_only () -> void = ...

// Feature flags
#cfg(feature: "ssl")
@secure_connect () -> void = ...

#cfg(any_feature: ["ssl", "tls"])
@encrypted_connect () -> void = ...
```

### Implementation

- [ ] **Spec**: Cfg attribute semantics
  - [ ] `debug`, `release` flags
  - [ ] `feature: "name"` syntax
  - [ ] `any_feature`, `not_feature`

- [x] **Parser**: Parse cfg attributes (verified 2026-03-29)
  - [x] Boolean flags (debug, release) — `parse_cfg_attr_body()` in `conditional.rs:139`; debug, release, not_debug as bare identifiers
  - [x] Keyed flags (feature: "...") — `feature`, `not_feature` handled
  - [x] OR and negation variants — `any_feature` with list, all negation forms parsed
  - [x] IR: `CfgAttr` struct at `ori_ir/src/ast/items/function.rs:339` with fields: `debug`, `release`, `not_debug`, `feature`, `any_feature`, `not_feature`

- [ ] **Compiler**: Cfg evaluation
  - [ ] Accept `--debug` / `--release` flags — no cfg evaluation exists
  - [ ] Accept `--feature name` flags
  - [ ] Prune based on configuration
  - [x] **Rust Tests**: Cfg attribute parsing — tests in `file_attr.rs` for debug, release, not_debug, feature, any_feature (verified 2026-03-29)

- [ ] **Ori Tests**: `tests/spec/conditional/cfg_basic.ori` — NEEDS TESTS: file does not exist
  - [ ] debug/release flags
  - [ ] feature flags
  - [ ] any_feature, not_feature

- [ ] **LLVM Support**: LLVM codegen for cfg attribute
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — cfg attribute codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.5 Feature Flags

**Spec section**: `spec/25-conditional-compilation.md § Features`

### Syntax

```toml
# In ori.toml
[features]
default = ["logging"]
logging = []
metrics = ["logging"]  # depends on logging
experimental = []
ssl = []
```

```ori
// In code
#cfg(feature: "logging")
@log_message (msg: str) -> void uses Logger = ...

#cfg(not_feature: "logging")
@log_message (msg: str) -> void = ()  // No-op

// Feature-gated imports
#cfg(feature: "metrics")
use std.metrics { Counter, Gauge }
```

### Feature Name Validation

Feature names must be valid Ori identifiers:
- Start with a letter or underscore
- Contain only letters, digits, and underscores

```ori
#cfg(feature: "ssl")           // valid
#cfg(feature: "async_io")      // valid
#cfg(feature: "my-feature")    // error: invalid feature name
```

### Implementation

- [ ] **Spec**: Feature flag semantics
  - [ ] Declaration in ori.toml
  - [ ] Dependency resolution
  - [ ] Default features
  - [x] Feature name validation — `is_valid_feature_name()` at `conditional.rs:17`, produces E0932 error (verified 2026-03-29)

- [ ] **Build system**: Feature processing
  - [ ] Parse ori.toml features
  - [ ] Resolve feature dependencies
  - [ ] Pass to compiler
  - [x] Validate feature names — parser validates at parse time, tests in `file_attr.rs:277-377` (verified 2026-03-29)

- [ ] **Compiler**: Feature evaluation
  - [ ] `--feature` flag
  - [ ] `--no-default-features` flag
  - [ ] `--all-features` flag
  - [ ] **Rust Tests**: Feature flag processing

- [ ] **Ori Tests**: `tests/spec/conditional/features.ori`
  - [ ] Basic feature gating
  - [ ] Feature dependencies
  - [ ] Default features

- [ ] **LLVM Support**: LLVM codegen for feature flags
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — feature flags codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.6 File-Level Conditions

**Spec section**: `spec/25-conditional-compilation.md § File-Level Conditions`

### Syntax

```ori
// file: linux_impl.ori
#!target(os: "linux")

// Everything in this file is Linux-only
@epoll_create () -> int = ...
@epoll_wait (fd: int) -> [Event] = ...
```

The `#!` prefix indicates a file-level condition. It must appear before any declarations (after comments).

### Implementation

- [ ] **Spec**: File-level condition semantics
  - [ ] `#!` syntax
  - [ ] Position requirements
  - [ ] Interaction with imports

- [x] **Lexer**: Recognize `#!` token (verified 2026-03-29)
  - [x] Only at file start — file-level condition parsing handled by lexer/parser

- [x] **Parser**: Parse file-level conditions (verified 2026-03-29)
  - [x] `#!target(...)`, `#!cfg(...)` — `#!` prefix detected, `FileAttr::Target` and `FileAttr::Cfg` variants in IR
  - [x] Apply to entire file — `Module::file_attr: Option<FileAttr>` field
  - [x] Position requirement enforced (before imports/declarations)

- [ ] **Compiler**: File-level evaluation
  - [ ] Skip entire file if condition false — no evaluation logic exists
  - [ ] Track for IDE support
  - [x] **Rust Tests**: File-level condition parsing — 27 parser tests in `compiler/oric/tests/phases/parse/file_attr.rs` (verified 2026-03-29)

- [ ] **Ori Tests**: `tests/spec/conditional/file_level.ori` — NEEDS TESTS: file does not exist
  - [ ] File-level target
  - [ ] File-level cfg

- [ ] **LLVM Support**: LLVM codegen for file-level conditions
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — file-level conditions codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.7 Compile-Time Constants

**Spec section**: `spec/25-conditional-compilation.md § Compile-Time Constants`

### Sync Points: Compile-Time Constants (multi-crate sync required)

Adding `$target_os`, `$target_arch`, `$target_family`, `$debug`, `$release` requires:

1. **`ori_ir`** — Represent compile-time constants in AST (distinguish from user `$` constants)
2. **`ori_types`** — Register type signatures (`$target_os: str`, `$debug: bool`, etc.) in `infer_ident()`
3. **`ori_eval`** — Provide runtime values from build configuration
4. **`ori_llvm`** — Fold to constants during codegen, enable dead branch elimination

### Built-in Constants

```ori
$target_os: str       // "linux", "macos", "windows", etc.
$target_arch: str     // "x86_64", "aarch64", etc.
$target_family: str   // "unix", "windows", "wasm"
$debug: bool          // true in debug builds
$release: bool        // true in release builds
```

### Usage

```ori
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/"
```

### Dead Code Elimination

Branches conditioned on compile-time constants are eliminated and not type-checked:

```ori
@get_window_handle () -> WindowHandle =
    if $target_os == "windows" then
        WinApi.get_hwnd()  // Only type-checked on Windows
    else
        panic(msg: "Not supported on this platform")
```

### Implementation

- [ ] **Spec**: Compile-time constant semantics
  - [ ] Built-in constant names and types
  - [ ] DCE rules
  - [ ] Type-checking behavior

- [ ] **Lexer/Parser**: Recognize built-in constants
  - [ ] `$target_os`, `$target_arch`, etc.
  - [ ] Treat as config variables

- [ ] **Type checker**: Compile-time evaluation
  - [ ] Evaluate comparisons at compile time
  - [ ] Skip type-checking false branches
  - [ ] Eliminate dead code
  - [ ] **Rust Tests**: Compile-time constant evaluation

- [ ] **Ori Tests**: `tests/spec/conditional/constants.ori`
  - [ ] $target_os checks
  - [ ] $target_arch checks
  - [ ] $debug/$release checks
  - [ ] Dead branch elimination

- [ ] **LLVM Support**: LLVM codegen for compile-time constants
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — compile-time constants codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.8 Build Configuration

**Spec section**: `spec/25-conditional-compilation.md § Build Configuration`

### CLI Flags

```bash
# Target specification
ori build --target linux-x86_64
ori build --target macos-aarch64
ori build --target windows-x86_64

# Features
ori build --feature ssl --feature async
ori build --no-default-features --feature minimal

# Build mode
ori build --debug    # sets cfg(debug)
ori build --release  # sets cfg(release)

# Custom cfg flags
ori build --cfg experimental
```

### Project Configuration

```toml
# ori.toml
[package]
name = "myapp"
version = "1.0.0"

[features]
default = ["ssl"]
ssl = []
async = ["dep:async-runtime"]
experimental = []

[target.linux]
dependencies = ["libc"]

[target.windows]
dependencies = ["winapi"]
```

### Implementation

- [ ] **Spec**: Build configuration
  - [ ] ori.toml format
  - [ ] CLI flag reference
  - [ ] Precedence rules

- [ ] **Build system**: Configuration processing
  - [ ] Parse ori.toml
  - [ ] Merge with CLI flags
  - [ ] Pass to compiler
  - [ ] **Rust Tests**: Build configuration processing

- [ ] **Compiler**: Accept configuration
  - [ ] `--target` flag
  - [ ] `--feature` flag
  - [ ] `--debug` / `--release` flags
  - [ ] `--cfg` flag

- [ ] **Ori Tests**: Integration tests
  - [ ] Build with features
  - [ ] Cross-compilation cfg
  - [ ] Custom cfg values

- [ ] **LLVM Support**: LLVM codegen for build configuration
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — build configuration codegen
- [ ] **AOT Tests**: No AOT coverage yet

---

## 13.9 Diagnostics

**Spec section**: `spec/25-conditional-compilation.md § Diagnostics`

### Error Messages

```ori
// Clear errors for cfg mismatch
#target(os: "linux")
@linux_only () -> void = ...

// On Windows:
// error: function `linux_only` is not available on this platform
//   --> src/main.ori:5:1
//   |
// 5 | linux_only()
//   | ^^^^^^^^^^ requires #target(os: "linux")
//   |
//   = note: current target: windows-x86_64
//   = help: this function is only available on Linux
```

### Invalid Feature Names

```ori
#cfg(feature: "my-feature")
// error: invalid feature name "my-feature"
//   --> src/main.ori:1:15
//   |
// 1 | #cfg(feature: "my-feature")
//   |               ^^^^^^^^^^^^ feature names must be valid identifiers
//   |
//   = help: use "my_feature" instead
```

### Implementation

- [ ] **Diagnostics**: Condition-aware error messages
  - [ ] Show active configuration when relevant
  - [ ] Suggest alternative platforms/features
  - [x] Validate feature names — `E0932` error code in `conditional.rs:262`; valid: `ssl`, `_private`, `Feature123`; invalid: hyphenated, digit-start (verified 2026-03-29)

- [ ] **Lints**: Condition validation
  - [ ] Warn on impossible conditions
  - [ ] Warn on unknown OS/arch values
  - [x] Error on invalid feature names — E0932 implemented (verified 2026-03-29)
  - [x] **Rust Tests**: Feature name validation tests — `file_attr.rs:277-377` (verified 2026-03-29)

- [ ] **Ori Tests**: `tests/compile-fail/conditional/` — NEEDS TESTS: directory does not exist
  - [ ] Platform mismatch errors
  - [ ] Invalid feature names
  - [ ] Unknown condition values

- [ ] **LLVM Support**: LLVM codegen for condition diagnostics
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — condition diagnostics codegen

- **Additional diagnostics (verified 2026-03-29)**:
  - [x] Attribute placement validation — `attr_validation.rs` (28+ tests): rejects `#target`, `#cfg`, `#derive`, `#skip`, `#repr` on wrong item kinds (traits, extern blocks, etc.); accepts on functions, types, constants, imports, impls
  - [x] Orphaned attribute diagnostics — `attr_validation.rs:196-267`: EOF orphan detection

---

## 13.10 compile_error Built-in

> **CROSS-REFERENCE**: Section 11.8 also covers `compile_error` in the FFI context.
> Implementation should be done once and shared; avoid duplicate work.

**Proposal**: `proposals/approved/additional-builtins-proposal.md`

### Syntax

```ori
@compile_error (msg: str) -> Never
```

Causes a compile-time error with the given message. Valid only in compile-time evaluable contexts.

### Constraints

```ori
// ERROR: compile_error in unconditional code
@bad () -> void = compile_error(msg: "always fails")

// OK: compile_error in dead branch
@platform_check () -> void =
    if $target_os == "windows" then
        compile_error(msg: "Windows not supported")
    else
        real_impl()

// OK: compile_error in #target block
#target(os: "windows")
@platform_specific () -> void = compile_error(msg: "Not supported")
```

### Implementation

- [ ] **Spec**: Add `compile_error` to `spec/annex-c-built-in-functions.md`
  - [ ] Syntax and return type
  - [ ] Context restrictions (conditional compilation only)
  - [ ] Error message format

- [ ] **Lexer/Parser**: Reserve `compile_error` as built-in
  - [ ] Cannot define function with this name

- [ ] **Compiler**: compile_error evaluation
  - [ ] Detect in conditional compilation branches
  - [ ] Verify not in runtime-reachable code
  - [ ] Emit compile-time error with user message
  - [ ] **Rust Tests**: compile_error evaluation tests

- [ ] **Ori Tests**: `tests/spec/conditional/compile_error.ori`
  - [ ] In #target blocks
  - [ ] In #cfg blocks
  - [ ] In if $constant branches
  - [ ] **Compile-fail tests**: `tests/compile-fail/compile_error_unconditional.ori`

- [ ] **LLVM Support**: LLVM codegen for compile_error
  - [ ] Should never reach LLVM (compile-time only)
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — verify compile_error not in codegen

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Spec updated: `spec/25-conditional-compilation.md` complete
- [ ] CLAUDE.md updated with conditional compilation syntax
- [ ] `#target(...)` works on items
- [ ] `#cfg(...)` works on items
- [ ] `#!target(...)` works on files
- [ ] OR conditions work (`any_*`)
- [ ] Negation works (`not_*`)
- [ ] Target families work
- [ ] Feature flags work
- [ ] Compile-time constants work
- [ ] Dead code elimination works
- [ ] Build system integration complete
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: Can build a cross-platform CLI tool with platform-specific implementations

---

## Example: Cross-Platform Path Handling

```ori
// Platform-specific path separator using compile-time constant
@get_path_separator () -> str =
    if $target_os == "windows" then "\\" else "/"

// Platform-specific home directory using target attribute
#target(family: "unix")
@home_dir () -> Option<str> = Env.get(key: "HOME")

#target(os: "windows")
@home_dir () -> Option<str> = Env.get(key: "USERPROFILE")

// Platform-specific file operations
#target(family: "unix")
@set_permissions (path: str, mode: int) -> Result<void, Error> uses FileSystem = {
    // Unix chmod
    Unix.chmod(path: path, mode: mode)
}

#target(os: "windows")
@set_permissions (path: str, mode: int) -> Result<void, Error> uses FileSystem =
    // Windows handles permissions differently
    Ok(())

// Debug-only logging
#cfg(debug)
@debug_log (msg: str) -> void = print(msg: `[DEBUG] {msg}`)

#cfg(not_debug)
@debug_log (msg: str) -> void = ()
```
