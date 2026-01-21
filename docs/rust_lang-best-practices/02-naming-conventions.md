# Naming Conventions

Guidelines for naming items in Rust code.

## Quick Reference

| Item | Convention | Example |
|------|------------|---------|
| Crates | `snake_case` | `my_crate` |
| Modules | `snake_case` | `lexer`, `type_check` |
| Files | `snake_case` | `my_module.rs` |
| Types | `CamelCase` | `TokenStream`, `AstNode` |
| Traits | `CamelCase` | `Iterator`, `Display` |
| Enums | `CamelCase` | `Option`, `Result` |
| Enum variants | `CamelCase` | `Some`, `None`, `Ok` |
| Functions | `snake_case` | `parse_expr`, `to_string` |
| Methods | `snake_case` | `push`, `is_empty` |
| Local variables | `snake_case` | `item_count`, `idx` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_SIZE`, `PI` |
| Statics | `SCREAMING_SNAKE_CASE` | `GLOBAL_CONFIG` |
| Type parameters | Single uppercase or `CamelCase` | `T`, `E`, `Item` |
| Lifetimes | Short lowercase | `'a`, `'src`, `'input` |

## Detailed Guidelines

### Types and Traits

```rust
// Types: CamelCase
struct TokenStream { ... }
struct AbstractSyntaxTree { ... }

// Traits: CamelCase, often adjectives or -able
trait Parseable { ... }
trait Iterator { ... }
trait Display { ... }

// Type aliases: CamelCase
type Result<T> = std::result::Result<T, Error>;
```

### Enums and Variants

```rust
// Enum: CamelCase
// Variants: CamelCase (no prefix)
pub enum BinaryOp {
    Add,      // Not "OpAdd" or "BinaryOpAdd"
    Sub,
    Mul,
    Div,
}

// For Option/Result-like enums
pub enum ParseResult<T> {
    Ok(T),
    Err(ParseError),
}
```

### Functions and Methods

```rust
// Functions: snake_case
fn parse_expression(tokens: &[Token]) -> Expr { ... }
fn calculate_offset(base: usize, index: usize) -> usize { ... }

// Methods: snake_case
impl Span {
    pub fn new(filename: impl Into<String>, range: Range<usize>) -> Self { ... }
    pub fn merge(&self, other: &Span) -> Span { ... }
}
```

### Conversion Methods

Follow standard naming patterns:

| Pattern | Use case | Example |
|---------|----------|---------|
| `as_*` | Cheap reference conversion | `as_str()`, `as_slice()` |
| `to_*` | Expensive conversion | `to_string()`, `to_vec()` |
| `into_*` | Consuming conversion | `into_inner()`, `into_iter()` |
| `from_*` | Construction from other type | `from_str()`, `from_utf8()` |

```rust
impl Token {
    // Cheap: returns reference
    pub fn as_str(&self) -> &str { ... }

    // Expensive: allocates new String
    pub fn to_string(&self) -> String { ... }

    // Consuming: takes ownership
    pub fn into_inner(self) -> TokenInner { ... }
}
```

### Predicate Methods

Use `is_*` or `has_*` for boolean-returning methods:

```rust
impl Token {
    pub fn is_keyword(&self) -> bool { ... }
    pub fn is_operator(&self) -> bool { ... }
    pub fn has_value(&self) -> bool { ... }
}
```

### Getter/Setter Methods

```rust
impl Config {
    // Getter: just the field name
    pub fn name(&self) -> &str { ... }

    // Setter: set_ prefix
    pub fn set_name(&mut self, name: String) { ... }

    // Mutable getter: _mut suffix
    pub fn name_mut(&mut self) -> &mut String { ... }
}
```

### Constructor Methods

```rust
impl Span {
    // Primary constructor: new
    pub fn new(filename: impl Into<String>, range: Range<usize>) -> Self { ... }

    // Alternative constructors: with_, from_
    pub fn with_filename(filename: &str) -> Self { ... }
    pub fn from_token(token: &Token) -> Self { ... }
}

// Default for "empty" or "zero" construction
impl Default for Span {
    fn default() -> Self { ... }
}
```

### Constants and Statics

```rust
// Constants: SCREAMING_SNAKE_CASE
const MAX_BUFFER_SIZE: usize = 1024;
const DEFAULT_TIMEOUT_MS: u64 = 5000;

// Statics: SCREAMING_SNAKE_CASE
static GLOBAL_ALLOCATOR: MyAllocator = MyAllocator::new();
```

### Type Parameters

```rust
// Single letter for simple generics
fn identity<T>(value: T) -> T { value }

// Descriptive names for complex bounds
fn process<Item, Error>(items: Vec<Item>) -> Result<(), Error>
where
    Item: Parse,
    Error: From<ParseError>,
{ ... }

// Common conventions
// T - generic type
// E - error type
// K, V - key/value types
// I - iterator type
// R - return type
```

### Lifetimes

```rust
// Short names for simple cases
fn parse<'a>(input: &'a str) -> &'a str { ... }

// Descriptive names for clarity
fn parse_with_context<'input, 'ctx>(
    input: &'input str,
    context: &'ctx Context,
) -> ParseResult<'input> { ... }
```

### Modules and Files

```rust
// File names: snake_case
// lexer.rs, type_check.rs, code_gen.rs

// Module names match file names
mod lexer;        // -> lexer.rs or lexer/mod.rs
mod type_check;   // -> type_check.rs
mod code_gen;     // -> code_gen.rs
```

## Common Patterns

### Avoid Redundant Prefixes

```rust
// Bad: redundant prefixes
mod token {
    pub struct Token { ... }
    pub enum TokenKind { ... }        // "Token" prefix is redundant
    pub fn tokenize() { ... }         // "token" prefix is redundant
}

// Good: context provides clarity
mod token {
    pub struct Token { ... }
    pub enum Kind { ... }             // token::Kind
    pub fn parse() { ... }            // token::parse()
}
```

### Acronyms

Treat acronyms as words:

```rust
// Good
struct HttpClient { ... }
struct JsonParser { ... }
fn parse_url() { ... }

// Bad
struct HTTPClient { ... }
struct JSONParser { ... }
fn parse_URL() { ... }
```

## Guidelines

### Do

- Follow Rust naming conventions consistently
- Use descriptive names that convey meaning
- Keep names concise but clear
- Match method naming to Rust stdlib patterns

### Don't

- Don't use Hungarian notation (`strName`, `iCount`)
- Don't abbreviate excessively (`calc_ofst` → `calculate_offset`)
- Don't add type suffixes (`name_string`, `count_int`)
- Don't prefix enum variants with enum name

## Resources

- [Naming - Rust API Guidelines](https://rust-lang.github.io/api-guidelines/naming.html)
- [Rust Style Guide - Naming](https://doc.rust-lang.org/nightly/style-guide/)
