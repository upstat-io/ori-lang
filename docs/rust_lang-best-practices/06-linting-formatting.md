# Linting and Formatting

Guidelines for maintaining consistent code quality with Clippy and rustfmt.

## Quick Reference

- [ ] Run `cargo fmt` before committing
- [ ] Run `cargo clippy` and fix warnings
- [ ] Configure workspace-wide lints in `Cargo.toml`
- [ ] Use `#[allow(...)]` sparingly with justification
- [ ] Set up CI to enforce formatting and lints

## Rustfmt

### Running

```bash
# Format all code
cargo fmt

# Check formatting without changing files
cargo fmt -- --check

# Format specific file
rustfmt src/main.rs
```

### Configuration

Create `rustfmt.toml` in project root:
```toml
# rustfmt.toml
edition = "2021"
max_width = 100
tab_spaces = 4
use_small_heuristics = "Default"
```

### Common Options

```toml
# Maximum line width
max_width = 100

# Spaces per indentation
tab_spaces = 4

# Import grouping
imports_granularity = "Module"
group_imports = "StdExternalCrate"

# Use field init shorthand
use_field_init_shorthand = true

# Format string literals
format_strings = false

# Reorder imports
reorder_imports = true
reorder_modules = true

# Chain formatting
chain_width = 60
```

### Skipping Formatting

```rust
// Skip formatting for a block
#[rustfmt::skip]
fn intentionally_formatted() {
    let matrix = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
    ];
}

// Skip for an attribute
#[rustfmt::skip::attributes(derive)]
```

## Clippy

### Running

```bash
# Run clippy
cargo clippy

# Run on all targets
cargo clippy --all-targets --all-features

# Apply automatic fixes
cargo clippy --fix

# Deny warnings (for CI)
cargo clippy -- -D warnings
```

### Configuration in Cargo.toml

Configure workspace-wide lints:

```toml
# Cargo.toml (workspace root)
[workspace.lints.rust]
unsafe_code = "deny"
# missing_docs = "warn"

[workspace.lints.clippy]
# Lint groups
correctness = { level = "deny", priority = -1 }
suspicious = { level = "warn", priority = -1 }
style = { level = "warn", priority = -1 }
complexity = { level = "warn", priority = -1 }
perf = { level = "warn", priority = -1 }

# Specific lints
unwrap_used = "warn"
expect_used = "warn"
todo = "warn"
dbg_macro = "warn"

# Allow these
needless_return = "allow"
print_stdout = "allow"
```

Inherit in crate:
```toml
# crate/Cargo.toml
[lints]
workspace = true
```

### Lint Levels

| Level | Behavior |
|-------|----------|
| `allow` | Suppress the lint |
| `warn` | Emit warning, continue |
| `deny` | Emit error, stop |
| `forbid` | Like deny, cannot be overridden |

### Lint Categories

| Category | Description |
|----------|-------------|
| `correctness` | Code that is definitely wrong |
| `suspicious` | Code that is likely wrong |
| `style` | Code style issues |
| `complexity` | Overly complex code |
| `perf` | Performance issues |
| `pedantic` | Very strict, often opinionated |
| `nursery` | Experimental lints |
| `cargo` | Cargo.toml issues |

### In-Code Attributes

```rust
// Allow a lint for an item
#[allow(clippy::too_many_arguments)]
fn complex_function(a: i32, b: i32, c: i32, d: i32, e: i32, f: i32, g: i32) {
    // ...
}

// Allow for a scope
fn process() {
    #[allow(clippy::unwrap_used)]
    let value = some_option.unwrap(); // We know it's Some
}

// Allow with reason (requires reason lint)
#[allow(clippy::unwrap_used, reason = "guaranteed Some by invariant")]
let value = cache.get(&key).unwrap();
```

### Common Lints to Enable

```toml
[workspace.lints.clippy]
# Safety
unwrap_used = "warn"          # Prefer ? or expect
expect_used = "warn"          # Prefer ? or handle
panic = "warn"                # Avoid panics

# Code quality
todo = "warn"                 # Don't forget TODOs
unimplemented = "warn"        # Don't ship unimplemented
dbg_macro = "warn"            # Remove debug macros

# Performance
clone_on_ref_ptr = "warn"     # Clone Arc/Rc explicitly
inefficient_to_string = "warn"

# Correctness
suspicious_operation_groupings = "deny"
```

### Common Lints to Allow

```toml
[workspace.lints.clippy]
# Sometimes return keyword is clearer
needless_return = "allow"

# CLI programs need to print
print_stdout = "allow"
print_stderr = "allow"

# Single-character names ok for iterators
many_single_char_names = "allow"

# Sometimes explicit is clearer
redundant_field_names = "allow"
```

## CI Integration

### GitHub Actions

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Check formatting
        run: cargo fmt -- --check

      - name: Run clippy
        run: cargo clippy --all-targets -- -D warnings

      - name: Run tests
        run: cargo test
```

### Pre-commit Hook

```bash
#!/bin/sh
# .git/hooks/pre-commit

# Check formatting
cargo fmt -- --check
if [ $? -ne 0 ]; then
    echo "Please run 'cargo fmt' before committing"
    exit 1
fi

# Run clippy
cargo clippy -- -D warnings
if [ $? -ne 0 ]; then
    echo "Please fix clippy warnings before committing"
    exit 1
fi
```

## Editor Integration

### VS Code

Install `rust-analyzer` extension. Add to `settings.json`:
```json
{
  "rust-analyzer.check.command": "clippy",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### Neovim

With `nvim-lspconfig`:
```lua
require('lspconfig').rust_analyzer.setup {
  settings = {
    ['rust-analyzer'] = {
      check = {
        command = 'clippy'
      }
    }
  }
}
```

## Guidelines

### Do

- Run `cargo fmt` before every commit
- Configure workspace-wide lints
- Fix warnings rather than suppressing them
- Use `#[allow]` with explanatory comments
- Enable strict lints in CI

### Don't

- Don't suppress lints without justification
- Don't use `#[allow]` at crate level without good reason
- Don't ignore clippy suggestions without understanding them
- Don't disable entire lint categories

## Resources

- [Clippy Lints](https://rust-lang.github.io/rust-clippy/master/)
- [rustfmt Configuration](https://rust-lang.github.io/rustfmt/)
- [Configuring Lints in Cargo.toml](https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section)
