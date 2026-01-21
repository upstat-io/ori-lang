# Performance

Guidelines for writing efficient Rust code based on official documentation.

## Quick Reference

- [ ] Profile before optimizing
- [ ] Use `--release` for performance testing
- [ ] Prefer stack allocation when possible
- [ ] Pre-allocate collections when size is known
- [ ] Avoid unnecessary clones
- [ ] Use iterators over index loops

## Profiling First

**Never optimize without measuring.** Use profiling tools to identify actual bottlenecks:

```bash
# Always benchmark in release mode
cargo build --release

# Profile with system tools
# Linux: perf
perf record ./target/release/myprogram
perf report

# macOS: Instruments
instruments -t "Time Profiler" ./target/release/myprogram
```

## Release vs Debug Mode

Debug builds prioritize fast compilation over runtime speed:

```bash
# Debug: fast compile, slow runtime, includes debug info
cargo run

# Release: slower compile, fast runtime, optimizations enabled
cargo run --release
```

Always use `--release` for performance testing. Debug builds can be 10-100x slower.

## Memory Allocation

### Stack vs Heap

| Location | Characteristics | Use For |
|----------|-----------------|---------|
| Stack | Fast, automatic cleanup, limited size | Small, fixed-size values |
| Heap | Slower, explicit allocation, unlimited | Large or dynamic-size values |

```rust
// Stack allocation: fast, no allocator call
let array: [i32; 100] = [0; 100];
let x = 42;
let point = (1.0, 2.0);

// Heap allocation: requires allocator
let vector: Vec<i32> = vec![0; 100];
let boxed: Box<i32> = Box::new(42);
let string: String = String::from("hello");
```

### Pre-allocation

When you know the size, pre-allocate to avoid reallocations:

```rust
// Bad: multiple reallocations as vector grows
let mut results = Vec::new();
for item in items {
    results.push(process(item));
}

// Good: single allocation
let mut results = Vec::with_capacity(items.len());
for item in items {
    results.push(process(item));
}

// Also good: collect() uses size hints
let results: Vec<_> = items.iter().map(process).collect();
```

### String Pre-allocation

```rust
// Bad: may reallocate multiple times
let mut s = String::new();
for word in words {
    s.push_str(word);
    s.push(' ');
}

// Good: estimate capacity upfront
let total_len: usize = words.iter().map(|w| w.len() + 1).sum();
let mut s = String::with_capacity(total_len);
for word in words {
    s.push_str(word);
    s.push(' ');
}
```

## Avoiding Unnecessary Clones

### Borrow Instead of Clone

```rust
// Bad: unnecessary clone
fn process(data: &Data) {
    let copy = data.clone();
    analyze(&copy);
}

// Good: borrow directly
fn process(data: &Data) {
    analyze(data);
}
```

### Clone on Write

Use `Cow` when you might need to modify borrowed data:

```rust
use std::borrow::Cow;

fn normalize(s: &str) -> Cow<str> {
    if s.contains('\t') {
        // Only allocates if modification needed
        Cow::Owned(s.replace('\t', "    "))
    } else {
        // No allocation - just borrows
        Cow::Borrowed(s)
    }
}
```

### Take Ownership When Needed

If you need to store or consume the data, take ownership:

```rust
// If the function needs to own the data, take ownership
fn store_item(storage: &mut Vec<Item>, item: Item) {
    storage.push(item);
}
```

## Collection Selection

From [std::collections documentation](https://doc.rust-lang.org/std/collections/):

| Collection | Use When |
|------------|----------|
| `Vec<T>` | Default choice, contiguous memory, fast iteration |
| `VecDeque<T>` | Need efficient push/pop at both ends |
| `HashMap<K, V>` | Fast key lookup by hash |
| `BTreeMap<K, V>` | Need sorted keys or range queries |
| `HashSet<T>` | Unique items, fast membership test |
| `BTreeSet<T>` | Sorted unique items |

`Vec` is almost always the right choice unless you have specific needs.

## Iterator Optimization

### Prefer Iterators Over Index Loops

```rust
// Suboptimal: bounds checking on each access
for i in 0..items.len() {
    process(&items[i]);
}

// Better: no bounds checking, optimizer-friendly
for item in &items {
    process(item);
}

// With index if needed
for (i, item) in items.iter().enumerate() {
    process(i, item);
}
```

### Iterator Chains Are Lazy

Iterators don't allocate intermediate collections:

```rust
// No intermediate allocations - single pass
let sum: i32 = items
    .iter()
    .filter(|x| x.is_valid())
    .map(|x| x.value())
    .sum();
```

### Collect Uses Size Hints

`collect()` uses iterator size hints to pre-allocate:

```rust
// collect() knows the size and pre-allocates
let doubled: Vec<i32> = numbers.iter().map(|n| n * 2).collect();
```

## String Optimization

### Use &str When Possible

```rust
// Suboptimal: requires owned String
fn greet(name: String) {
    println!("Hello, {name}!");
}

// Better: accepts borrowed string
fn greet(name: &str) {
    println!("Hello, {name}!");
}
```

### String Formatting

```rust
// Suboptimal: multiple allocations
let msg = "Error: ".to_string() + &code + " - " + &description;

// Better: single allocation
let msg = format!("Error: {} - {}", code, description);
```

## Compiler Optimizations

### Profile Settings

```toml
# Cargo.toml

[profile.release]
opt-level = 3        # Maximum optimization
lto = true           # Link-time optimization
codegen-units = 1    # Better optimization, slower compile
```

### Inline Hints

```rust
// Suggest inlining for small functions
#[inline]
fn small_helper() -> i32 { 42 }

// Force inlining (use sparingly)
#[inline(always)]
fn critical_hot_path() { /* ... */ }

// Prevent inlining for cold paths
#[inline(never)]
fn error_handler() { /* ... */ }
```

## Data Layout

### Enum Size

Large enum variants make the entire enum large:

```rust
// Entire enum is 1000+ bytes due to one variant
enum Message {
    Quit,
    Data([u8; 1000]),
}

// Better: box large variants
enum Message {
    Quit,
    Data(Box<[u8; 1000]>),  // Enum is pointer-sized
}
```

## Hot Path Optimization

### Move Work Out of Loops

```rust
// Bad: repeated work each iteration
for line in lines {
    let re = Regex::new(r"\d+").unwrap();  // Compiled every iteration!
    if re.is_match(line) { /* ... */ }
}

// Good: do it once
let re = Regex::new(r"\d+").unwrap();
for line in lines {
    if re.is_match(line) { /* ... */ }
}
```

### Reuse Buffers

```rust
// Bad: allocates each iteration
for item in items {
    let temp = format!("{:?}", item);
    process(&temp);
}

// Good: reuse buffer
use std::fmt::Write;
let mut buf = String::with_capacity(100);
for item in items {
    buf.clear();
    write!(&mut buf, "{:?}", item).unwrap();
    process(&buf);
}
```

## Guidelines

### Do

- Profile before optimizing
- Use `--release` for performance testing
- Pre-allocate when size is known
- Prefer iterators over index loops
- Reuse buffers in hot loops
- Use `&str` instead of `String` in function parameters

### Don't

- Don't optimize without measuring
- Don't sacrifice readability for micro-optimizations
- Don't assume allocations are free
- Don't benchmark in debug mode

## Resources

- [std::collections - Rust Standard Library](https://doc.rust-lang.org/std/collections/)
- [Profiles - The Cargo Book](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [std::borrow::Cow](https://doc.rust-lang.org/std/borrow/enum.Cow.html)
