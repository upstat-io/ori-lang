# Cargo and Dependencies

Guidelines for managing Cargo workspaces, dependencies, and features.

## Quick Reference

- [ ] Use workspaces for multi-crate projects
- [ ] Share dependencies via `[workspace.dependencies]`
- [ ] Use workspace inheritance for package metadata
- [ ] Pin dependencies with exact versions in applications
- [ ] Use semver ranges for libraries
- [ ] Minimize feature flags complexity

## Workspace Configuration

### Basic Workspace Setup

```toml
# Cargo.toml (workspace root)
[workspace]
resolver = "2"
members = [
    "crate_a",
    "crate_b",
    "tools/*",  # Glob patterns supported
]

# Exclude certain directories
exclude = ["experimental"]
```

### Workspace Package Metadata

Share common metadata across all crates:

```toml
# Cargo.toml (workspace root)
[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/org/project"
authors = ["Your Name <you@example.com>"]
rust-version = "1.70"
```

Inherit in member crates:

```toml
# crate_a/Cargo.toml
[package]
name = "crate_a"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
```

## Shared Dependencies

### Declaring Workspace Dependencies

```toml
# Cargo.toml (workspace root)
[workspace.dependencies]
# External crates
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
thiserror = "1.0"

# Internal crates
my_core = { path = "crates/core" }
my_utils = { path = "crates/utils" }
```

### Using Workspace Dependencies

```toml
# crate_a/Cargo.toml
[dependencies]
serde.workspace = true
tokio.workspace = true
my_core.workspace = true

# Override features for this crate
tokio = { workspace = true, features = ["rt-multi-thread"] }
```

## Dependency Version Specification

### Version Syntax

| Syntax | Meaning |
|--------|---------|
| `"1.2.3"` | `>=1.2.3, <2.0.0` (caret default) |
| `"^1.2.3"` | `>=1.2.3, <2.0.0` (explicit caret) |
| `"~1.2.3"` | `>=1.2.3, <1.3.0` (tilde) |
| `"1.2.*"` | `>=1.2.0, <1.3.0` (wildcard) |
| `">=1.2.3"` | At least this version |
| `"=1.2.3"` | Exactly this version |

### Version Guidelines

**For Libraries:**
```toml
# Use semver ranges - allows users flexibility
[dependencies]
serde = "1.0"  # Any 1.x compatible version
```

**For Applications:**
```toml
# Pin more strictly or use Cargo.lock
[dependencies]
serde = "1.0.193"  # Specific version
```

### Cargo.lock

- **Libraries**: Don't commit `Cargo.lock` (let users resolve)
- **Applications**: Commit `Cargo.lock` (reproducible builds)
- **Workspaces**: One `Cargo.lock` at workspace root

## Feature Flags

### Declaring Features

```toml
[features]
default = ["std"]
std = []
serde = ["dep:serde"]
full = ["std", "serde", "async"]
async = ["dep:tokio"]

[dependencies]
serde = { version = "1.0", optional = true }
tokio = { version = "1", optional = true }
```

### Feature Patterns

```rust
// Conditional compilation
#[cfg(feature = "serde")]
use serde::{Serialize, Deserialize};

#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Config {
    pub name: String,
}

// Feature-gated modules
#[cfg(feature = "async")]
pub mod async_api;

// Feature-gated functions
#[cfg(feature = "std")]
pub fn use_std_feature() { ... }
```

### Feature Best Practices

```toml
[features]
# Default features should be minimal
default = []

# Group related functionality
full = ["feature-a", "feature-b", "feature-c"]

# Use dep: prefix for optional dependencies
json = ["dep:serde_json"]

# No-std support
std = []
alloc = []  # Requires alloc but not full std
```

## Conditional Dependencies

### Platform-Specific

```toml
[target.'cfg(unix)'.dependencies]
nix = "0.27"

[target.'cfg(windows)'.dependencies]
windows = "0.48"

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

### Development Only

```toml
[dev-dependencies]
criterion = "0.5"
proptest = "1.4"
pretty_assertions = "1.4"

# Only for tests
[dev-dependencies.tempfile]
version = "3.8"
```

### Build Dependencies

```toml
[build-dependencies]
cc = "1.0"          # For building C code
bindgen = "0.69"    # For generating bindings
```

## Workspace Lints

### Configuring Workspace Lints

```toml
# Cargo.toml (workspace root)
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "warn"

[workspace.lints.clippy]
correctness = { level = "deny", priority = -1 }
suspicious = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
```

### Inheriting Lints

```toml
# crate_a/Cargo.toml
[lints]
workspace = true
```

## Build Profiles

### Standard Profiles

```toml
# Development: fast compile, no optimizations
[profile.dev]
opt-level = 0
debug = true

# Release: slow compile, full optimizations
[profile.release]
opt-level = 3
lto = true
codegen-units = 1

# Testing
[profile.test]
opt-level = 0
debug = true

# Benchmarks
[profile.bench]
opt-level = 3
debug = false
```

### Custom Profiles

```toml
# Custom profile inheriting from release
[profile.production]
inherits = "release"
lto = "fat"
strip = true
panic = "abort"
```

## Useful Commands

```bash
# Update dependencies
cargo update

# Update specific dependency
cargo update -p serde

# Check for outdated dependencies
cargo outdated  # requires cargo-outdated

# Audit dependencies for vulnerabilities
cargo audit  # requires cargo-audit

# Show dependency tree
cargo tree

# Show why a dependency is included
cargo tree -i serde

# Check all features compile
cargo check --all-features

# Build docs for all crates
cargo doc --workspace --no-deps
```

## Guidelines

### Do

- Use workspaces for related crates
- Share common dependencies at workspace level
- Use workspace inheritance for metadata
- Commit `Cargo.lock` for applications
- Run `cargo update` regularly
- Audit dependencies periodically

### Don't

- Don't commit `Cargo.lock` for libraries
- Don't use `*` versions
- Don't add unnecessary dependencies
- Don't enable features you don't need
- Don't mix workspace and non-workspace deps inconsistently

## Resources

- [The Cargo Book](https://doc.rust-lang.org/cargo/)
- [Cargo Workspaces](https://doc.rust-lang.org/book/ch14-03-cargo-workspaces.html)
- [Features - Cargo Reference](https://doc.rust-lang.org/cargo/reference/features.html)
- [Specifying Dependencies](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
