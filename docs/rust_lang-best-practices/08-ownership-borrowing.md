# Ownership and Borrowing

Guidelines for working with Rust's ownership system based on official documentation.

## Quick Reference

- [ ] Prefer borrowing (`&T`) over ownership (`T`) in parameters
- [ ] Use `&mut T` only when mutation is needed
- [ ] Return owned values from constructors
- [ ] Clone explicitly when sharing is needed
- [ ] Use `Rc`/`Arc` for shared ownership
- [ ] Minimize lifetime annotations

## Ownership Rules

1. Each value has exactly one owner
2. When the owner goes out of scope, the value is dropped
3. Ownership can be transferred (moved) or borrowed

```rust
fn main() {
    let s1 = String::from("hello");  // s1 owns the string
    let s2 = s1;                      // ownership moved to s2
    // println!("{}", s1);            // Error: s1 no longer valid

    let s3 = s2.clone();              // explicit clone
    println!("{} {}", s2, s3);        // both valid
}
```

## Borrowing Patterns

### Immutable Borrows (`&T`)

```rust
// Good: borrows the string
fn calculate_length(s: &str) -> usize {
    s.len()
}

// Usage
let s = String::from("hello");
let len = calculate_length(&s);  // s still valid after
```

### Mutable Borrows (`&mut T`)

```rust
// Mutate through a mutable reference
fn append_world(s: &mut String) {
    s.push_str(" world");
}

// Usage
let mut s = String::from("hello");
append_world(&mut s);
```

### Borrowing Rules

1. Can have multiple `&T` OR exactly one `&mut T`
2. References must always be valid

```rust
fn main() {
    let mut s = String::from("hello");

    let r1 = &s;     // Ok: first immutable borrow
    let r2 = &s;     // Ok: second immutable borrow
    println!("{} {}", r1, r2);

    let r3 = &mut s; // Ok: immutable borrows done
    r3.push_str("!");
}
```

## Function Parameter Guidelines

### Prefer Borrowing

```rust
// Good: borrows the data
fn process(items: &[Item]) { ... }

// Bad: takes ownership unnecessarily
fn process(items: Vec<Item>) { ... }
```

### Take Ownership When Needed

```rust
// Good: needs to store the value
fn push(vec: &mut Vec<String>, item: String) {
    vec.push(item);
}

// Good: transforms the value
fn into_uppercase(s: String) -> String {
    s.to_uppercase()
}
```

### Use Generic Bounds

```rust
// Accept anything that can be borrowed as &str
fn greet(name: impl AsRef<str>) {
    println!("Hello, {}!", name.as_ref());
}

// Works with both
greet("world");           // &str
greet(String::from("you")); // String
```

## Return Value Guidelines

### Return Owned Values

```rust
// Good: returns owned value
fn create_greeting(name: &str) -> String {
    format!("Hello, {}!", name)
}
```

### Return References When Tied to Input

```rust
// Good: lifetime tied to input
fn first_word(s: &str) -> &str {
    s.split_whitespace().next().unwrap_or("")
}
```

## Lifetime Annotations

### When to Use

Lifetimes are needed when returning references:

```rust
// Explicit: return lifetime tied to input
fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() { x } else { y }
}

// Elision: single input, single output
fn first_char(s: &str) -> &str {
    &s[0..1]
}
```

### Lifetime Elision Rules

1. Each input reference gets its own lifetime
2. If one input lifetime, output gets that lifetime
3. If `&self` or `&mut self`, output gets that lifetime

```rust
// These are equivalent
fn foo(x: &str) -> &str { x }
fn foo<'a>(x: &'a str) -> &'a str { x }

// These are equivalent
impl Token {
    fn text(&self) -> &str { &self.text }
}
impl Token {
    fn text<'a>(&'a self) -> &'a str { &self.text }
}
```

### Named Lifetimes

Use descriptive names for clarity:

```rust
struct Parser<'input> {
    source: &'input str,
    position: usize,
}

fn parse_with_context<'input, 'ctx>(
    input: &'input str,
    context: &'ctx Context,
) -> ParseResult<'input> {
    // ...
}
```

## Smart Pointers

### `Box<T>` - Heap Allocation

```rust
// Recursive types need indirection
enum List {
    Cons(i32, Box<List>),
    Nil,
}

// Large values to avoid stack copies
fn process(data: Box<[u8; 1_000_000]>) { ... }
```

### `Rc<T>` - Shared Ownership (Single-threaded)

```rust
use std::rc::Rc;

let data = Rc::new(vec![1, 2, 3]);
let data2 = Rc::clone(&data);  // Increment reference count

// Both references valid
println!("{:?} {:?}", data, data2);
```

### `Arc<T>` - Shared Ownership (Thread-safe)

```rust
use std::sync::Arc;
use std::thread;

let data = Arc::new(vec![1, 2, 3]);
let data2 = Arc::clone(&data);

thread::spawn(move || {
    println!("{:?}", data2);
});
```

### When to Use What

| Type | Use Case |
|------|----------|
| `T` | Single owner, stack or moves |
| `Box<T>` | Single owner, heap allocated |
| `Rc<T>` | Multiple owners, single-threaded |
| `Arc<T>` | Multiple owners, multi-threaded |

## Interior Mutability

### `Cell<T>` - Copy Types

```rust
use std::cell::Cell;

struct Counter {
    count: Cell<u32>,
}

impl Counter {
    fn increment(&self) {  // Note: &self, not &mut self
        self.count.set(self.count.get() + 1);
    }
}
```

### `RefCell<T>` - Runtime Borrow Checking

```rust
use std::cell::RefCell;

let data = RefCell::new(vec![1, 2, 3]);

{
    let mut borrowed = data.borrow_mut();
    borrowed.push(4);
}  // borrow ends

println!("{:?}", data.borrow());
```

### `Mutex<T>` - Thread-safe Interior Mutability

```rust
use std::sync::Mutex;

let data = Mutex::new(0);

{
    let mut num = data.lock().unwrap();
    *num += 1;
}  // lock released

println!("{}", *data.lock().unwrap());
```

## Common Patterns

### Clone When Needed

```rust
// Explicit clone to share data
let original = vec![1, 2, 3];
let copy = original.clone();

process(&copy);
consume(original);  // consumes original
```

### Cow (Clone on Write)

```rust
use std::borrow::Cow;

fn process(input: Cow<str>) -> Cow<str> {
    if input.contains("bad") {
        // Only clones if modification needed
        Cow::Owned(input.replace("bad", "good"))
    } else {
        input
    }
}

// No allocation if no change needed
let result = process(Cow::Borrowed("hello"));
```

### Entry API for Maps

```rust
use std::collections::HashMap;

let mut map = HashMap::new();

// Efficient: only one lookup
map.entry("key")
    .or_insert_with(Vec::new)
    .push(value);

// Inefficient: two lookups
if !map.contains_key("key") {
    map.insert("key", Vec::new());
}
map.get_mut("key").unwrap().push(value);
```

## Guidelines

### Do

- Prefer `&T` over `T` in function parameters
- Use `Clone` explicitly when sharing data
- Return owned values from constructors
- Use `Cow<T>` for functions that sometimes modify
- Minimize scope of mutable borrows

### Don't

- Don't clone unnecessarily (consider borrowing)
- Don't fight the borrow checker with unsafe
- Don't use `Rc`/`Arc` when a reference suffices
- Don't use `RefCell` when regular mutability works
- Don't return references to local variables

## Resources

- [Understanding Ownership - The Rust Book](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html)
- [References and Borrowing](https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html)
- [Smart Pointers](https://doc.rust-lang.org/book/ch15-00-smart-pointers.html)
- [Lifetimes](https://doc.rust-lang.org/book/ch10-03-lifetime-syntax.html)
