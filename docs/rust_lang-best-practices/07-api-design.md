# API Design

Guidelines for designing ergonomic and idiomatic Rust APIs.

## Quick Reference

- [ ] Follow naming conventions (see [02-naming-conventions](02-naming-conventions.md))
- [ ] Implement standard traits (`Debug`, `Clone`, `Default`, etc.)
- [ ] Use newtypes for type safety
- [ ] Prefer borrowing over ownership in parameters
- [ ] Use builder pattern for complex construction
- [ ] Make invalid states unrepresentable

## Checklist from Rust API Guidelines

### Naming

- [ ] Casing follows conventions (C-CASE)
- [ ] Ad-hoc conversions follow `as_`, `to_`, `into_` patterns (C-CONV)
- [ ] Getter names follow conventions (C-GETTER)
- [ ] Methods on collections follow `iter`, `iter_mut`, `into_iter` (C-ITER)
- [ ] Constructor is `new` or type name in module (C-CTOR)

### Interoperability

- [ ] Types eagerly implement common traits (C-COMMON-TRAITS)
- [ ] Conversions use standard traits (C-CONV-TRAITS)
- [ ] Collections implement `FromIterator` and `Extend` (C-COLLECT)
- [ ] Types are `Send` and `Sync` where possible (C-SEND-SYNC)

### Macros

- [ ] Input syntax is evocative of output (C-EVOCATIVE)
- [ ] Macros compose well with attributes (C-MACRO-ATTR)
- [ ] Item macros work anywhere items are allowed (C-ANYWHERE)

### Documentation

- [ ] Crate-level docs include examples (C-CRATE-DOC)
- [ ] All items have rustdoc (C-DOC)
- [ ] Examples use `?`, not `unwrap` (C-QUESTION-MARK)
- [ ] Function docs include Error, Panic, Safety sections (C-FAILURE)
- [ ] Prose links to relevant items (C-LINK)

### Predictability

- [ ] Smart pointers don't add inherent methods (C-SMART-PTR)
- [ ] Conversions are infallible or checked (C-CONV-SPECIFIC)
- [ ] Functions with no meaningful receiver are static (C-METHOD)

### Flexibility

- [ ] Functions minimize assumptions on parameters (C-GENERIC)
- [ ] Traits are object-safe if useful that way (C-OBJECT)

### Type Safety

- [ ] Newtypes provide static distinctions (C-NEWTYPE)
- [ ] Arguments convey meaning (C-CUSTOM-TYPE)
- [ ] Types for validated data (C-VALIDATE)
- [ ] Builders for complex values (C-BUILDER)

### Dependability

- [ ] Functions validate arguments (C-VALIDATE)
- [ ] Destructors never fail (C-DTOR-FAIL)
- [ ] Destructors don't block (C-DTOR-BLOCK)

### Debuggability

- [ ] All public types implement `Debug` (C-DEBUG)
- [ ] `Debug` output is not empty (C-DEBUG-NONEMPTY)

### Future Proofing

- [ ] Sealed traits prevent downstream implementations (C-SEALED)
- [ ] Structs have private fields (C-STRUCT-PRIVATE)
- [ ] Newtypes encapsulate implementation details (C-NEWTYPE-HIDE)
- [ ] Data structures don't duplicate derived trait bounds (C-STRUCT-BOUNDS)

## Common Traits to Implement

### Minimum Viable Traits

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
```

### Standard Trait Implementations

| Trait | When to Implement |
|-------|-------------------|
| `Debug` | Always for public types |
| `Clone` | When copying is meaningful |
| `Default` | When a sensible default exists |
| `PartialEq`, `Eq` | For comparable types |
| `PartialOrd`, `Ord` | For sortable types |
| `Hash` | For use as HashMap keys |
| `Display` | For user-facing output |
| `From`/`Into` | For type conversions |
| `AsRef`/`AsMut` | For cheap reference conversions |

## Builder Pattern

For types with many optional parameters:

```rust
#[derive(Debug, Clone)]
pub struct Config {
    timeout: Duration,
    retries: u32,
    verbose: bool,
}

#[derive(Default)]
pub struct ConfigBuilder {
    timeout: Option<Duration>,
    retries: Option<u32>,
    verbose: bool,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn retries(mut self, retries: u32) -> Self {
        self.retries = Some(retries);
        self
    }

    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn build(self) -> Config {
        Config {
            timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
            retries: self.retries.unwrap_or(3),
            verbose: self.verbose,
        }
    }
}

// Usage
let config = ConfigBuilder::new()
    .timeout(Duration::from_secs(60))
    .verbose(true)
    .build();
```

## Newtype Pattern

Use newtypes for type safety:

```rust
// Without newtype: easy to mix up
fn create_user(name: String, email: String, phone: String) { ... }

// With newtypes: compile-time safety
pub struct UserName(String);
pub struct Email(String);
pub struct Phone(String);

fn create_user(name: UserName, email: Email, phone: Phone) { ... }

// Implementing common traits
impl UserName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UserName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

## Accept Generics, Return Concrete

### Parameter Types

```rust
// Good: accepts anything that can become a String
pub fn new(filename: impl Into<String>, range: Range<usize>) -> Self {
    Span {
        filename: filename.into(),
        range,
    }
}

// Usage
let span = Span::new("file.si", 0..10);  // &str works
let span = Span::new(filename, 0..10);   // String works too
```

### Common Generic Bounds

```rust
// Accept &str or String
fn process(name: impl Into<String>) { ... }

// Accept slices, arrays, or vecs
fn sum(items: impl AsRef<[i32]>) -> i32 {
    items.as_ref().iter().sum()
}

// Accept paths
fn read(path: impl AsRef<Path>) -> io::Result<String> {
    fs::read_to_string(path)
}
```

## Method Receiver Patterns

| Receiver | Use Case |
|----------|----------|
| `&self` | Read-only access (most common) |
| `&mut self` | Modify in place |
| `self` | Consuming/transforming |
| `mut self` | Consume and modify (builder pattern) |

```rust
impl Token {
    // Read-only: most methods
    pub fn kind(&self) -> TokenKind { self.kind }

    // Modify in place: mutators
    pub fn set_kind(&mut self, kind: TokenKind) {
        self.kind = kind;
    }

    // Consuming: transformations
    pub fn into_string(self) -> String {
        self.text
    }

    // Consume and modify: builders
    pub fn with_span(mut self, span: Span) -> Self {
        self.span = span;
        self
    }
}
```

## Making Invalid States Unrepresentable

```rust
// Bad: invalid states possible
struct Connection {
    is_connected: bool,
    stream: Option<TcpStream>,  // Can be Some when is_connected is false!
}

// Good: type system prevents invalid states
enum Connection {
    Disconnected,
    Connected(TcpStream),
}

// Bad: status and data can be inconsistent
struct Response {
    status: Status,
    data: Option<Data>,
    error: Option<Error>,
}

// Good: states are mutually exclusive
enum Response {
    Success(Data),
    Failure(Error),
    Pending,
}
```

## Sealed Traits

Prevent downstream implementations while allowing use:

```rust
mod private {
    pub trait Sealed {}
}

pub trait MyTrait: private::Sealed {
    fn method(&self);
}

// Only your types can implement it
impl private::Sealed for MyType {}
impl MyTrait for MyType {
    fn method(&self) { ... }
}
```

## Guidelines

### Do

- Derive standard traits liberally
- Use `impl Into<T>` for string-like parameters
- Make constructors return `Self` or `Result<Self, E>`
- Provide `Default` when sensible
- Use builder pattern for complex types
- Document trait implementations

### Don't

- Don't expose internal collections directly
- Don't require callers to construct complex values
- Don't use `String` when `&str` suffices
- Don't make all fields public
- Don't add methods to smart pointers

## Resources

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Rust API Guidelines Checklist](https://rust-lang.github.io/api-guidelines/checklist.html)
