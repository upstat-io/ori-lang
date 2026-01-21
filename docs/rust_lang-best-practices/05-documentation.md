# Documentation

Guidelines for documenting Rust code with rustdoc based on official documentation.

## Quick Reference

- [ ] Use `///` for item documentation (functions, types, etc.)
- [ ] Use `//!` for module/crate-level documentation
- [ ] Include `# Examples` section for public items
- [ ] Use `# Panics`, `# Errors`, `# Safety` sections as needed
- [ ] Write doc tests that compile and run
- [ ] Build docs with `cargo doc --open`

## Documentation Comments

### Item Documentation (`///`)

```rust
/// Parses the input string into a structured document.
///
/// This function processes the input character by character,
/// building a tree of document nodes.
///
/// # Arguments
///
/// * `input` - The string to parse
/// * `options` - Parsing options
///
/// # Returns
///
/// A parsed document, or an error if parsing fails.
///
/// # Examples
///
/// ```
/// use my_crate::parse;
///
/// let doc = parse("hello", Default::default()).unwrap();
/// assert_eq!(doc.text(), "hello");
/// ```
pub fn parse(input: &str, options: Options) -> Result<Document, ParseError> {
    // ...
}
```

### Module Documentation (`//!`)

```rust
//! # Parser Module
//!
//! This module provides parsing functionality for the document format.
//!
//! ## Overview
//!
//! The parser converts raw text into a structured document tree
//! that can be processed by other components.
//!
//! ## Example
//!
//! ```
//! use my_crate::parser::parse;
//!
//! let doc = parse("hello world")?;
//! # Ok::<(), my_crate::ParseError>(())
//! ```

mod lexer;
mod ast;
```

## Documentation Sections

### Standard Sections

| Section | When to Use |
|---------|-------------|
| `# Examples` | Always for public items |
| `# Panics` | When function can panic |
| `# Errors` | For `Result`-returning functions |
| `# Safety` | For `unsafe` functions |
| `# Arguments` | For complex parameter lists |
| `# Returns` | When return value needs explanation |

### Examples Section

```rust
/// Creates a new configuration with the given name.
///
/// # Examples
///
/// ```
/// use my_crate::Config;
///
/// let config = Config::new("production");
/// assert_eq!(config.name(), "production");
/// ```
pub fn new(name: impl Into<String>) -> Self {
    // ...
}
```

### Errors Section

```rust
/// Loads a configuration file from the given path.
///
/// # Errors
///
/// Returns `Err` if:
/// - The file does not exist
/// - The file is not valid UTF-8
/// - The file contains invalid configuration syntax
///
/// # Examples
///
/// ```no_run
/// use my_crate::Config;
///
/// let config = Config::load("config.toml")?;
/// # Ok::<(), my_crate::ConfigError>(())
/// ```
pub fn load(path: &str) -> Result<Config, ConfigError> {
    // ...
}
```

### Panics Section

```rust
/// Returns the element at the given index.
///
/// # Panics
///
/// Panics if `index >= self.len()`.
///
/// # Examples
///
/// ```
/// let v = vec![1, 2, 3];
/// assert_eq!(v[1], 2);
/// ```
pub fn index(&self, index: usize) -> &T {
    // ...
}
```

### Safety Section

```rust
/// Dereferences a raw pointer to read the value.
///
/// # Safety
///
/// The caller must ensure that:
/// - `ptr` is valid and properly aligned
/// - `ptr` points to an initialized value of type `T`
/// - No mutable references to the pointed value exist
///
/// # Examples
///
/// ```
/// let x = 42;
/// let ptr = &x as *const i32;
/// let value = unsafe { read_ptr(ptr) };
/// assert_eq!(value, 42);
/// ```
pub unsafe fn read_ptr<T>(ptr: *const T) -> T {
    // ...
}
```

## Doc Tests

### Basic Doc Test

```rust
/// Adds two numbers together.
///
/// ```
/// assert_eq!(add(2, 3), 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### Hiding Lines

Use `#` to hide setup code:

```rust
/// Processes configuration.
///
/// ```
/// # use std::collections::HashMap;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let config = load_config("settings.toml")?;
/// assert!(config.debug);
/// # Ok(())
/// # }
/// ```
```

### Testing Errors

```rust
/// Parses an integer from a string.
///
/// ```
/// # use std::num::ParseIntError;
/// let num: i32 = "42".parse()?;
/// assert_eq!(num, 42);
/// # Ok::<(), ParseIntError>(())
/// ```
///
/// Invalid input returns an error:
///
/// ```
/// let result: Result<i32, _> = "not a number".parse();
/// assert!(result.is_err());
/// ```
```

### Ignoring Tests

```rust
/// This example requires external setup.
///
/// ```ignore
/// let db = connect_to_database();
/// db.query("SELECT 1")?;
/// ```
```

### Compile-Only Tests

```rust
/// This example shows the API but shouldn't be run.
///
/// ```no_run
/// let server = Server::bind("0.0.0.0:8080")?;
/// server.run(); // Would block forever
/// ```
```

### Should Panic Tests

```rust
/// Divides two numbers.
///
/// # Panics
///
/// Panics if `b` is zero.
///
/// ```should_panic
/// divide(1, 0); // Panics!
/// ```
pub fn divide(a: i32, b: i32) -> i32 {
    a / b
}
```

## Type Documentation

### Structs

```rust
/// A configuration for the application.
///
/// This struct holds all the settings needed to run the application,
/// including timeouts, paths, and feature flags.
///
/// # Examples
///
/// ```
/// use my_crate::Config;
///
/// let config = Config::builder()
///     .timeout(30)
///     .verbose(true)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct Config {
    /// The name of this configuration.
    pub name: String,
    /// Timeout in seconds.
    pub timeout: u32,
    /// Whether to enable verbose output.
    pub verbose: bool,
}
```

### Enums

```rust
/// The type of a binary operation.
///
/// Binary operations take two operands and produce a result.
///
/// # Examples
///
/// ```
/// use my_crate::BinaryOp;
///
/// let op = BinaryOp::Add;
/// assert!(matches!(op, BinaryOp::Add));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    /// Addition: `a + b`
    Add,
    /// Subtraction: `a - b`
    Sub,
    /// Multiplication: `a * b`
    Mul,
    /// Division: `a / b`
    Div,
}
```

## Linking in Documentation

### Link to Types

```rust
/// Converts this [`Token`] into a [`String`].
///
/// See also: [`TokenStream`], [`Lexer::tokenize`]
pub fn to_string(&self) -> String { ... }
```

### Link to Methods

```rust
/// Creates a new span. See [`Span::merge`] for combining spans.
pub fn new(...) -> Self { ... }
```

### External Links

```rust
/// Parses JSON according to [RFC 8259](https://tools.ietf.org/html/rfc8259).
pub fn parse_json(input: &str) -> Result<Value, Error> { ... }
```

## Building Documentation

```bash
# Build documentation
cargo doc

# Build and open in browser
cargo doc --open

# Include private items
cargo doc --document-private-items

# Build for all workspace members
cargo doc --workspace

# Build without dependencies
cargo doc --no-deps
```

## Guidelines

### Do

- Document all public items
- Include working examples
- Explain error conditions
- Use standard section names
- Link to related items
- Keep first line as summary

### Don't

- Don't document obvious things
- Don't repeat the type signature
- Don't leave TODO in published docs
- Don't write examples that can't compile
- Don't document private implementation details

### First Line Rule

The first line should be a brief summary:

```rust
// Good: clear summary first
/// Creates a new token stream from source code.
///
/// This function processes... (more details)

// Bad: no clear summary
/// This is a complex function that does many things including
/// tokenization and validation and error reporting...
```

## Resources

- [The Rustdoc Book](https://doc.rust-lang.org/rustdoc/)
- [How to Write Documentation](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)
- [Documentation - Rust API Guidelines](https://rust-lang.github.io/api-guidelines/documentation.html)
