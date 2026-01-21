# Error Handling

Guidelines for handling errors in Rust based on official documentation.

## Quick Reference

- [ ] Use `Result<T, E>` for recoverable errors
- [ ] Use `Option<T>` for optional values (not errors)
- [ ] Propagate errors with `?` operator
- [ ] Implement `std::error::Error` for custom error types
- [ ] Use `panic!` only for unrecoverable programmer errors
- [ ] Provide context in error messages

## Result and Option

### When to Use

| Type | Use Case |
|------|----------|
| `Result<T, E>` | Operation that can fail |
| `Option<T>` | Value that may or may not exist |
| `panic!` | Unrecoverable programmer errors (bugs) |

```rust
// Result: operation can fail
fn read_config(path: &str) -> Result<Config, io::Error> {
    let content = fs::read_to_string(path)?;
    // parse content...
}

// Option: value might not exist
fn find_user(id: u64) -> Option<User> {
    users.get(&id).cloned()
}

// Panic: invariant violation (bug in code)
fn get_element(slice: &[i32], index: usize) -> i32 {
    assert!(index < slice.len(), "index out of bounds");
    slice[index]
}
```

### The ? Operator

Propagate errors concisely with `?`:

```rust
fn process_file(path: &str) -> Result<Data, Error> {
    let content = fs::read_to_string(path)?;  // Returns early on Err
    let parsed = parse(&content)?;
    let validated = validate(parsed)?;
    Ok(validated)
}
```

For `Option`:

```rust
fn get_user_email(users: &HashMap<u64, User>, id: u64) -> Option<&str> {
    let user = users.get(&id)?;  // Returns None if not found
    Some(&user.email)
}
```

## Defining Custom Error Types

### Simple Error Enum

```rust
use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    UnexpectedToken { expected: String, found: String },
    UnterminatedString,
    InvalidNumber(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedToken { expected, found } => {
                write!(f, "expected {expected}, found {found}")
            }
            Self::UnterminatedString => write!(f, "unterminated string literal"),
            Self::InvalidNumber(s) => write!(f, "invalid number: {s}"),
        }
    }
}

impl std::error::Error for ParseError {}
```

### Error with Source

For errors that wrap other errors:

```rust
use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Parse(ParseError),
    Config(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Parse(e) => write!(f, "parse error: {e}"),
            Self::Config(msg) => write!(f, "config error: {msg}"),
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            Self::Config(_) => None,
        }
    }
}
```

### Error Conversion with From

Implement `From` to enable automatic conversion with `?`:

```rust
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<ParseError> for AppError {
    fn from(err: ParseError) -> Self {
        AppError::Parse(err)
    }
}

// Now ? automatically converts
fn load_config(path: &str) -> Result<Config, AppError> {
    let content = fs::read_to_string(path)?;  // io::Error -> AppError
    let config = parse_config(&content)?;      // ParseError -> AppError
    Ok(config)
}
```

## Type Aliases

For cleaner function signatures:

```rust
// Define a type alias for your error type
pub type Result<T> = std::result::Result<T, AppError>;

// Use it in function signatures
pub fn parse(input: &str) -> Result<Ast> {
    // ...
}
```

## Handling Errors

### Match on Error Variants

```rust
match load_config("config.toml") {
    Ok(config) => use_config(config),
    Err(AppError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
        println!("Config not found, using defaults");
        Config::default()
    }
    Err(e) => return Err(e),
}
```

### Transforming Errors

```rust
// Transform success value
let length = content.map(|s| s.len());

// Transform error
let result = parse(input)
    .map_err(|e| AppError::Parse(e))?;
```

### Unwrap Variants

| Method | Behavior |
|--------|----------|
| `unwrap()` | Panics on Err/None |
| `expect("msg")` | Panics with custom message |
| `unwrap_or(default)` | Returns default on Err/None |
| `unwrap_or_else(\|\| ...)` | Computes default lazily |
| `unwrap_or_default()` | Uses Default trait |
| `ok()` | Converts Result to Option |
| `err()` | Gets Err variant as Option |

```rust
// Prefer these over unwrap() in library code
let value = result.unwrap_or(0);
let value = result.unwrap_or_else(|| compute_default());
let value = option.unwrap_or_default();

// expect() is acceptable when failure indicates a bug
let config = CONFIG.get().expect("config must be initialized before use");
```

## Panic vs Result

### Use Panic For

- Programmer errors (violated invariants)
- Impossible states that indicate bugs
- Prototype code and examples
- Test assertions

```rust
// Bug in code - this should never happen
fn divide(a: i32, b: i32) -> i32 {
    assert!(b != 0, "division by zero is a bug");
    a / b
}

// Initialization that must succeed
fn init() -> &'static Config {
    CONFIG.get().expect("init() called before setup()")
}
```

### Use Result For

- Expected failure conditions
- User input errors
- External system failures (file not found, network errors)
- Anything the caller might want to handle

```rust
fn load_file(path: &str) -> Result<String, io::Error> {
    fs::read_to_string(path)
}

fn parse_number(s: &str) -> Result<i32, ParseIntError> {
    s.parse()
}
```

## Guidelines

### Do

- Define specific error types for libraries
- Implement `std::error::Error` for error types
- Implement `Display` with user-friendly messages
- Implement `From` for automatic conversion with `?`
- Include context in error messages
- Use `?` for error propagation

### Don't

- Don't use `unwrap()` in library code
- Don't return `String` as an error type
- Don't ignore errors with `let _ = ...`
- Don't panic for expected failures
- Don't lose error context when converting

## Resources

- [Error Handling - The Rust Book](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Recoverable Errors with Result](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html)
- [To panic! or Not to panic!](https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html)
- [std::error::Error](https://doc.rust-lang.org/std/error/trait.Error.html)
- [Error Handling - Rust API Guidelines](https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err)
