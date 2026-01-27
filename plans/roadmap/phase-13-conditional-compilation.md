# Phase 13: Conditional Compilation

**Goal**: Enable platform-specific code and feature flags

**Criticality**: High — Required for cross-platform support and feature management

---

## Design Decisions

| Question | Decision | Rationale |
|----------|----------|-----------|
| Syntax | `#cfg(...)` attribute | Simplified (Phase 15.1), cleaner than Rust |
| Evaluation | Compile-time only | No runtime overhead |
| Scope | Items and expressions | Flexible application |
| Predicates | Platform, feature, custom | Cover common cases |
| Negation | `not(...)` | Clear semantics |

---

## Reference Implementation

### Rust

```
~/lang_repos/rust/compiler/rustc_attr_parsing/src/cfg.rs  # cfg parsing
~/lang_repos/rust/compiler/rustc_expand/src/cfg.rs        # cfg evaluation
```

### Go

```
~/lang_repos/golang/src/go/build/constraint/             # Build constraints
~/lang_repos/golang/src/cmd/go/internal/work/build.go    # Build tags handling
```

---

## 13.1 Attribute Syntax

**Spec section**: `spec/24-conditional-compilation.md § Cfg Attribute`

### Syntax

```ori
// On items
#cfg(target_os: "linux")]
@linux_specific () -> void = ...

#cfg(target_os: "windows")]
@windows_specific () -> void = ...

// On expressions (cfg_if pattern)
let path_sep = cfg_match(
    cfg(target_os: "windows") -> "\\",
    cfg(target_os: "linux") -> "/",
    cfg(target_os: "macos") -> "/",
)

// Cfg block
#cfg(feature: "logging")]
{
    use std.logging { Logger }
    $LOGGER = Logger { level: "debug" }
}
```

### Grammar

```ebnf
CfgAttribute = '#' 'cfg' '(' CfgPredicate ')' ;
CfgPredicate = CfgOption | CfgAll | CfgAny | CfgNot ;
CfgOption    = Identifier ':' StringLiteral ;
CfgAll       = 'all' '(' CfgPredicate { ',' CfgPredicate } ')' ;
CfgAny       = 'any' '(' CfgPredicate { ',' CfgPredicate } ')' ;
CfgNot       = 'not' '(' CfgPredicate ')' ;
```

> **Note**: Uses simplified attribute syntax from Phase 15.1 (`#cfg(...)` not `#[cfg(...)]`).

### Implementation

- [ ] **Spec**: Add `spec/24-conditional-compilation.md`
  - [ ] Attribute syntax
  - [ ] Predicate grammar
  - [ ] Scope rules

- [ ] **Lexer/Parser**: Parse cfg attributes
  - [ ] `#cfg(...)]` syntax
  - [ ] Nested predicates
  - [ ] Apply to items

- [ ] **Compiler**: Cfg evaluation
  - [ ] Evaluate at compile time
  - [ ] Prune false branches
  - [ ] Track for error messages

- [ ] **Test**: `tests/spec/cfg/basic.ori`
  - [ ] Simple cfg
  - [ ] Nested predicates
  - [ ] cfg on various items

---

## 13.2 Platform Predicates

**Spec section**: `spec/24-conditional-compilation.md § Platform Predicates`

### Built-in Predicates

```ori
// Operating system
#cfg(target_os: "linux")]
#cfg(target_os: "macos")]
#cfg(target_os: "windows")]
#cfg(target_os: "freebsd")]
#cfg(target_os: "android")]
#cfg(target_os: "ios")]

// Architecture
#cfg(target_arch: "x86_64")]
#cfg(target_arch: "aarch64")]
#cfg(target_arch: "arm")]
#cfg(target_arch: "wasm32")]

// Pointer width
#cfg(target_pointer_width: "64")]
#cfg(target_pointer_width: "32")]

// Endianness
#cfg(target_endian: "little")]
#cfg(target_endian: "big")]

// OS family
#cfg(target_family: "unix")]
#cfg(target_family: "windows")]

// Vendor
#cfg(target_vendor: "apple")]
#cfg(target_vendor: "unknown")]
```

### Implementation

- [ ] **Spec**: Platform predicate reference
  - [ ] All built-in predicates
  - [ ] Values for each platform
  - [ ] Detection mechanism

- [ ] **Compiler**: Target detection
  - [ ] Detect from build environment
  - [ ] Cross-compilation support
  - [ ] `--target` flag

- [ ] **Stdlib**: Platform constants
  - [ ] `std.env.TARGET_OS`
  - [ ] `std.env.TARGET_ARCH`
  - [ ] Runtime equivalents (for dynamic checks)

- [ ] **Test**: `tests/spec/cfg/platform.ori`
  - [ ] OS-specific code
  - [ ] Arch-specific code
  - [ ] Cross-platform fallback

---

## 13.3 Feature Flags

**Spec section**: `spec/24-conditional-compilation.md § Features`

### Syntax

```ori
// In ori.toml
[features]
default = ["logging"]
logging = []
metrics = ["logging"]  // depends on logging
experimental = []

// In code
#cfg(feature: "logging")]
@log_message (msg: str) -> void uses Logger = ...

#cfg(not(feature: "logging"))]
@log_message (msg: str) -> void = void  // No-op

// Feature-gated imports
#cfg(feature: "metrics")]
use std.metrics { Counter, Gauge }
```

### Implementation

- [ ] **Spec**: Feature flag semantics
  - [ ] Declaration in ori.toml
  - [ ] Dependency resolution
  - [ ] Default features

- [ ] **Build system**: Feature processing
  - [ ] Parse ori.toml features
  - [ ] Resolve feature dependencies
  - [ ] Pass to compiler

- [ ] **Compiler**: Feature evaluation
  - [ ] `--features` flag
  - [ ] `--no-default-features` flag
  - [ ] `--all-features` flag

- [ ] **Test**: `tests/spec/cfg/features.ori`
  - [ ] Basic feature gating
  - [ ] Feature dependencies
  - [ ] Default features

---

## 13.4 Compound Predicates

**Spec section**: `spec/24-conditional-compilation.md § Compound Predicates`

### Combinators

```ori
// All (AND)
#cfg(all(target_os: "linux", target_arch: "x86_64"))]
@linux_x64_only () -> void = ...

// Any (OR)
#cfg(any(target_os: "linux", target_os: "macos"))]
@unix_like () -> void = ...

// Not (negation)
#cfg(not(target_os: "windows"))]
@non_windows () -> void = ...

// Complex combinations
#cfg(all(
    target_family: "unix",
    not(target_os: "macos"),
    any(target_arch: "x86_64", target_arch: "aarch64"),
))]
@linux_arm_or_x64 () -> void = ...
```

### Implementation

- [ ] **Spec**: Combinator semantics
  - [ ] Short-circuit evaluation
  - [ ] Nesting rules
  - [ ] Precedence

- [ ] **Parser**: Parse compound predicates
  - [ ] `all(...)`, `any(...)`, `not(...)`
  - [ ] Recursive parsing
  - [ ] Error on invalid nesting

- [ ] **Evaluator**: Evaluate compound predicates
  - [ ] Boolean logic
  - [ ] Short-circuit for efficiency

- [ ] **Test**: `tests/spec/cfg/compound.ori`
  - [ ] all() combinations
  - [ ] any() combinations
  - [ ] not() negation
  - [ ] Deep nesting

---

## 13.5 Cfg in Expressions

**Spec section**: `spec/24-conditional-compilation.md § Cfg Expressions`

### cfg_match

```ori
// Multi-way cfg selection
let line_ending = cfg_match(
    cfg(target_os: "windows") -> "\r\n",
    cfg(target_family: "unix") -> "\n",
    _ -> "\n",  // Default fallback
)

// With complex expressions
let max_threads = cfg_match(
    cfg(target_arch: "wasm32") -> 1,
    cfg(all(target_os: "linux", target_arch: "x86_64")) -> 16,
    _ -> 4,
)
```

### cfg! predicate function

```ori
// Check cfg at compile time, returns bool
if cfg!(feature: "logging") then
    print("Logging is enabled")
else
    void

// Use in expressions
let debug_mode = cfg!(debug_assertions)
```

### Implementation

- [ ] **Spec**: Expression-level cfg
  - [ ] `cfg_match` syntax and semantics
  - [ ] `cfg!` predicate function
  - [ ] Exhaustiveness requirements

- [ ] **Parser**: Parse cfg expressions
  - [ ] `cfg_match` as pattern
  - [ ] `cfg!` as built-in function

- [ ] **Type checker**: Cfg expression types
  - [ ] All branches same type
  - [ ] Dead branch elimination

- [ ] **Test**: `tests/spec/cfg/expressions.ori`
  - [ ] cfg_match
  - [ ] cfg! function
  - [ ] Type consistency

---

## 13.6 Build Configuration

**Spec section**: `spec/24-conditional-compilation.md § Build Configuration`

### ori.toml

```toml
[package]
name = "myapp"
version = "1.0.0"

[features]
default = ["std"]
std = []
alloc = []
logging = []
metrics = ["logging"]

[target.linux]
dependencies = ["libc"]

[target.windows]
dependencies = ["winapi"]

[cfg]
# Custom cfg values
custom_key = "custom_value"
```

### CLI Flags

```bash
# Enable features
ori build --features logging,metrics

# Disable default features
ori build --no-default-features --features alloc

# Enable all features
ori build --all-features

# Set custom cfg
ori build --cfg custom_key="value"

# Target cross-compilation
ori build --target aarch64-unknown-linux-gnu
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

- [ ] **Compiler**: Accept cfg from build system
  - [ ] `--cfg` flag
  - [ ] Environment-based cfg
  - [ ] Target specification

- [ ] **Test**: Integration tests
  - [ ] Build with features
  - [ ] Cross-compilation cfg
  - [ ] Custom cfg values

---

## 13.7 Diagnostics

**Spec section**: `spec/24-conditional-compilation.md § Diagnostics`

### Error Messages

```ori
// Clear errors for cfg mismatch
#cfg(target_os: "linux")]
@linux_only () -> void = ...

// On Windows:
// error: function `linux_only` is not available on this platform
//   --> src/main.ori:5:1
//   |
// 5 | linux_only()
//   | ^^^^^^^^^^ requires cfg(target_os: "linux")
//   |
//   = note: current target: windows-x86_64
//   = help: this function is only available on Linux
```

### Dead Code Warnings

```ori
// Warn about never-true cfg
#cfg(all(target_os: "linux", target_os: "windows"))]
@impossible () -> void = ...
// warning: cfg predicate is always false
//   = note: target_os cannot be both "linux" and "windows"
```

### Implementation

- [ ] **Diagnostics**: Cfg-aware error messages
  - [ ] Show active cfg when relevant
  - [ ] Suggest alternative platforms
  - [ ] Link to platform docs

- [ ] **Lints**: Cfg validation
  - [ ] Warn on impossible cfg
  - [ ] Warn on redundant cfg
  - [ ] Warn on unknown cfg keys

- [ ] **Test**: `tests/compile-fail/cfg/`
  - [ ] Platform mismatch errors
  - [ ] Invalid cfg syntax
  - [ ] Impossible cfg warnings

---

## Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/24-conditional-compilation.md` complete
- [ ] CLAUDE.md updated with cfg syntax
- [ ] `#cfg(...)` works on items
- [ ] Platform predicates work
- [ ] Feature flags work
- [ ] `cfg_match` expression works
- [ ] Build system integration complete
- [ ] All tests pass: `cargo test && ori test tests/spec/cfg/`

**Exit Criteria**: Can build a cross-platform CLI tool with platform-specific implementations

---

## Example: Cross-Platform Path Handling

```ori
// Platform-specific path separator
$PATH_SEP: str = cfg_match(
    cfg(target_os: "windows") -> "\\",
    _ -> "/",
)

// Platform-specific home directory
@home_dir () -> Option<str> = cfg_match(
    cfg(target_family: "unix") -> run(
        use std.env { get_var }
        get_var(name: "HOME")
    ),
    cfg(target_os: "windows") -> run(
        use std.env { get_var }
        get_var(name: "USERPROFILE")
    ),
)

// Platform-specific file operations
#cfg(target_family: "unix")]
@set_permissions (path: str, mode: int) -> Result<void, Error> uses FileSystem = run(
    // Unix chmod
    extern "C" { @chmod (path: *byte, mode: c_int) -> c_int }
    unsafe {
        let result = chmod(path: path.as_c_str(), mode: c_int(mode))
        if result == 0 then Ok(void) else Err(Error { message: "chmod failed" })
    }
)

#cfg(target_os: "windows")]
@set_permissions (path: str, mode: int) -> Result<void, Error> uses FileSystem = run(
    // Windows ACL (simplified)
    Ok(void)  // Windows handles permissions differently
)
```
