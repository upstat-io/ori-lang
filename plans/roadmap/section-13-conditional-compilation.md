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
    status: partial
  - id: "13.6"
    title: File-Level Conditions
    status: partial
  - id: "13.7"
    title: Compile-Time Constants
    status: not-started
  - id: "13.8"
    title: Build Configuration
    status: partial
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

- [x] **Spec**: `spec/25-conditional-compilation.md` Clause 25.2 (verified 2026-03-29)
  - [x] Target attribute syntax
  - [x] OS, arch, family values
  - [x] Scope rules

- [x] **Lexer/Parser**: Parse target attributes (verified 2026-03-29)
  - [x] `#target(...)` syntax — `conditional.rs:parse_target_attr_body()`
  - [x] Named arguments: `os:`, `arch:`, `family:` — plus `not_os`, `not_arch`, `not_family`, `any_os`, `any_arch`
  - [x] Apply to items — `Function`, `TypeDecl`, `ImplDef`, `ConstDef`, `UseDef` all have `target_attr: Option<TargetAttr>`
  - [x] IR: `TargetAttr` struct in `ori_ir/src/ast/items/function.rs`
  - [x] Attribute dispatch: `AttrKind::Target` in `attr/mod.rs`
  - [x] Placement validation: `has_non_conditional_attrs()` + rejection for trait/extend/extern
  - [x] **Rust Tests**: 85+ parser tests pass — `attr_validation.rs` (32 tests), `file_attr.rs` (53 tests), semantic pins, diagnostic pins, negative tests (verified 2026-03-29)

- [ ] **Compiler**: Target evaluation — GAP-13-001
  - [ ] Evaluate against build target — no `evaluate_target()` function exists
  - [ ] Prune false branches from AST — all items type-checked regardless of target conditions
  - [ ] Track for error messages

- [ ] **Ori Tests**: `tests/spec/conditional/target_basic.ori` — GAP-13-006: no Ori spec tests exist
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

- [x] **Spec**: OR condition semantics — Clause 25.2.5 (verified 2026-03-29)
  - [x] `any_os`, `any_arch` with list syntax
  - [x] List syntax

- [x] **Parser**: Parse any_* variants (verified 2026-03-29)
  - [x] Array literal values — `conditional.rs:parse_attr_string_list()`
  - [x] IR: `TargetAttr::any_os` and `TargetAttr::any_arch` are `Vec<Name>`
  - [x] **Rust Tests**: 4 parse tests pass — 2-element list, single element, trailing comma (verified 2026-03-29)

- [ ] **Evaluator**: Evaluate OR conditions — GAP-13-001
  - [ ] Match if any element matches — no evaluation exists

- [ ] **Ori Tests**: `tests/spec/conditional/target_or.ori` — GAP-13-006
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

- [x] **Spec**: Negation semantics — Clause 25.2.6 (verified 2026-03-29)
  - [x] `not_*` prefix for all condition types
  - [x] Interaction with OR conditions

- [x] **Parser**: Parse not_* variants (verified 2026-03-29)
  - [x] Target: `not_os`, `not_arch`, `not_family` — `conditional.rs:99-101`
  - [x] Cfg: `not_debug` (bare), `not_feature` (keyed) — `conditional.rs:173-193`
  - [x] IR: `TargetAttr::not_os/not_arch/not_family`, `CfgAttr::not_debug/not_feature`
  - [x] **Rust Tests**: 5 parse tests pass — target negation (3), cfg negation (2) (verified 2026-03-29)

- [ ] **Evaluator**: Evaluate negation — GAP-13-001
  - [ ] Boolean NOT of underlying condition — no evaluation exists

- [ ] **Ori Tests**: `tests/spec/conditional/negation.ori` — GAP-13-006
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

- [x] **Spec**: Cfg attribute semantics — Clause 25.3 (verified 2026-03-29)
  - [x] `debug`, `release` flags
  - [x] `feature: "name"` syntax
  - [x] `any_feature`, `not_feature`

- [x] **Parser**: Parse cfg attributes (verified 2026-03-29)
  - [x] Boolean flags (debug, release) — `conditional.rs:parse_cfg_attr_body()`
  - [x] Keyed flags (feature: "...") — handles `feature`, `not_feature`, `any_feature`
  - [x] OR and negation variants
  - [x] IR: `CfgAttr` struct in `ori_ir/src/ast/items/function.rs`
  - [x] **Rust Tests**: 13+ parse tests pass — cfg on function/type/const/impl/use, feature variants, semantic pins (verified 2026-03-29)

- [ ] **Compiler**: Cfg evaluation — GAP-13-001, GAP-13-002
  - [x] `--release` flag exists in `BuildOptions` (verified 2026-03-29) — but NOT connected to cfg evaluation
  - [ ] Accept `--debug` flag — debug is default but no explicit flag
  - [ ] Accept `--feature name` flags — current `--features` is for CPU features, not language features
  - [ ] Prune based on configuration — no cfg evaluation logic exists

- [ ] **Ori Tests**: `tests/spec/conditional/cfg_basic.ori` — GAP-13-006
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

- [x] **Spec**: Feature flag semantics — Clause 25.3.2 (verified 2026-03-29)
  - [ ] Declaration in ori.toml
  - [ ] Dependency resolution
  - [ ] Default features
  - [x] Feature name validation — `is_valid_feature_name()` in `conditional.rs`

- [ ] **Build system**: Feature processing — GAP-13-002
  - [ ] Parse ori.toml features — no ori.toml parser exists
  - [ ] Resolve feature dependencies
  - [ ] Pass to compiler
  - [x] Validate feature names — parser-level validation with E0932 (verified 2026-03-29)

- [ ] **Compiler**: Feature evaluation — GAP-13-002
  - [ ] `--feature` flag — not implemented (current `--features` is for CPU features)
  - [ ] `--no-default-features` flag
  - [ ] `--all-features` flag
  - [x] **Rust Tests**: 11 feature name validation tests pass — valid names (3), invalid names (5), negation/list contexts (3) (verified 2026-03-29)

- [ ] **Ori Tests**: `tests/spec/conditional/features.ori` — GAP-13-006
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

- [x] **Spec**: File-level condition semantics — Clause 25.5 (verified 2026-03-29)
  - [x] `#!` syntax
  - [x] Position requirements
  - [x] Interaction with imports

- [x] **Lexer**: Recognize `#!` token (verified 2026-03-29)
  - [x] `TokenKind::HashBang` exists, tested in `ori_lexer/src/tests.rs:850-885`

- [x] **Parser**: Parse file-level conditions (verified 2026-03-29)
  - [x] `#!target(...)`, `#!cfg(...)` — `mod.rs:parse_file_attribute()` handles both, rejects other attrs
  - [x] IR: `FileAttr` enum with Target/Cfg variants, `Module::file_attr: Option<FileAttr>`
  - [x] Grammar: `grammar.ebnf:185` has `file_attribute` production
  - [x] **Rust Tests**: 25+ parse tests pass — target/cfg variants, edge cases, rejection tests (verified 2026-03-29)

- [ ] **Compiler**: File-level evaluation — GAP-13-001
  - [ ] Skip entire file if condition false — files with false conditions still fully type-checked
  - [ ] Track for IDE support

- [ ] **Ori Tests**: `tests/spec/conditional/file_level.ori` — GAP-13-006
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

- [ ] **Spec**: Compile-time constant semantics — GAP-13-003: zero implementation across all 4 crates (verified 2026-03-29)
  - [ ] Built-in constant names and types
  - [ ] DCE rules
  - [ ] Type-checking behavior

- [ ] **Lexer/Parser**: Recognize built-in constants
  - [ ] `$target_os`, `$target_arch`, etc. — no AST representation distinct from user `$` constants
  - [ ] Treat as config variables

- [ ] **Type checker**: Compile-time evaluation
  - [ ] Evaluate comparisons at compile time — no type signatures in `infer_ident()`
  - [ ] Skip type-checking false branches
  - [ ] Eliminate dead code
  - [ ] **Rust Tests**: Compile-time constant evaluation

- [ ] **Ori Tests**: `tests/spec/conditional/constants.ori`
  - [ ] $target_os checks
  - [ ] $target_arch checks
  - [ ] $debug/$release checks
  - [ ] Dead branch elimination

- [ ] **LLVM Support**: LLVM codegen for compile-time constants — no constant folding or DCE
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

- [ ] **Spec**: Build configuration — Clauses 25.8-25.9 (verified 2026-03-29)
  - [ ] ori.toml format
  - [ ] CLI flag reference
  - [ ] Precedence rules

- [ ] **Build system**: Configuration processing — GAP-13-002
  - [ ] Parse ori.toml — no ori.toml parser exists
  - [ ] Merge with CLI flags
  - [ ] Pass to compiler — build config not connected to conditional compilation evaluator
  - [ ] **Rust Tests**: Build configuration processing

- [ ] **Compiler**: Accept configuration
  - [x] `--target` flag — `parse_args.rs:20`, parsed into `BuildOptions::target` (verified 2026-03-29)
  - [ ] `--feature` flag — not implemented for conditional compilation
  - [x] `--release` flag — `parse_args.rs:16-19` (verified 2026-03-29) — but not connected to cfg evaluation
  - [ ] `--debug` flag — not implemented as explicit flag (debug is default)
  - [ ] `--no-default-features` / `--all-features` flags
  - [ ] `--cfg` flag

- [ ] **Ori Tests**: Integration tests — GAP-13-006
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
  - [x] Validate feature names — E0932 registered and emitted by parser (verified 2026-03-29)

- [ ] **Lints**: Condition validation
  - [ ] Warn on impossible conditions
  - [ ] Warn on unknown OS/arch values — no validation of OS/arch string values
  - [x] Error on invalid feature names — E0932 (verified 2026-03-29)
  - [x] **Rust Tests**: Feature name validation tests pass (verified 2026-03-29)

- [ ] **Error codes**: GAP-13-005 — missing error codes from spec
  - [ ] E0930: Invalid target OS — spec defines it, not registered in `ori_diagnostic`
  - [ ] E0931: Invalid target architecture — spec defines it, not registered in `ori_diagnostic`
  - [x] E0932: Invalid feature name — registered and working (verified 2026-03-29)
  - [ ] E0933: File-level condition must be first — spec defines it, not registered in `ori_diagnostic`
  - [ ] BUG: Parser uses generic E1006 for target parameter errors instead of domain-specific E0930/E0931

- [ ] **Ori Tests**: `tests/compile-fail/conditional/` — GAP-13-006
  - [ ] Platform mismatch errors
  - [ ] Invalid feature names
  - [ ] Unknown condition values

- [ ] **LLVM Support**: LLVM codegen for condition diagnostics
- [ ] **LLVM Rust Tests**: `ori_llvm/tests/conditional_tests.rs` — condition diagnostics codegen

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

- [ ] **Spec**: Add `compile_error` to `spec/annex-c-built-in-functions.md` — GAP-13-004: zero implementation (verified 2026-03-29)
  - [ ] Syntax and return type — listed in ori-syntax.md prelude but not in Annex C
  - [ ] Context restrictions (conditional compilation only)
  - [ ] Error message format

- [ ] **Lexer/Parser**: Reserve `compile_error` as built-in
  - [ ] Cannot define function with this name — not reserved, no parser recognition

- [ ] **Compiler**: compile_error evaluation
  - [ ] Type checker: not registered in `infer_ident()`, no type signature `(str) -> Never`
  - [ ] Evaluator: not registered in `function_val.rs`, not in eval dispatch
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

## Verification Summary (2026-03-29)

**Overall**: ~30% complete. Parser layer substantially done with 85+ passing Rust tests. Semantic evaluation entirely absent.

### GAP Items

- [ ] **GAP-13-001**: Parse-to-Evaluation Gap [CRITICAL] — Parsed `TargetAttr`/`CfgAttr` structs stored on AST nodes but never evaluated. All conditional code always type-checked and compiled. Need evaluation/pruning pass before type-checking. Affects 13.1-13.6.
- [ ] **GAP-13-002**: Build System Integration Gap [MAJOR] — No `--feature name` flag (current `--features` is CPU features), no `--no-default-features`/`--all-features`/`--cfg`/`--debug` flags, no ori.toml parser, build config not passed to evaluator. Affects 13.4, 13.5, 13.8.
- [ ] **GAP-13-003**: Compile-Time Constants Not Implemented [MAJOR] — `$target_os`/`$target_arch`/`$target_family`/`$debug`/`$release` have zero implementation across ori_ir, ori_types, ori_eval, ori_llvm. Affects 13.7.
- [ ] **GAP-13-004**: compile_error Built-in Not Implemented [MAJOR] — Documented in prelude/ori-syntax.md but has no implementation in any phase. Cross-ref with Section 11.8. Affects 13.10.
- [ ] **GAP-13-005**: Error Codes Not Registered [MINOR] — Spec defines E0930 (Invalid target OS), E0931 (Invalid target arch), E0933 (File-level condition must be first), but only E0932 is registered. Parser uses generic E1006 for target param errors. Affects 13.9.
- [ ] **GAP-13-006**: No Ori-Level Spec Tests [MINOR] — `tests/spec/conditional/` does not exist. All testing is Rust-level parser tests. Affects all subsections.

---

## Section Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [x] Spec written: `spec/25-conditional-compilation.md` (verified 2026-03-29)
- [x] Grammar: `grammar.ebnf` has `file_attribute` production (verified 2026-03-29)
- [x] Proposal: `proposals/approved/conditional-compilation-proposal.md` exists (verified 2026-03-29)
- [x] CLAUDE.md updated with conditional compilation syntax — ori-syntax.md has Conditional Compilation section (verified 2026-03-29)
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
