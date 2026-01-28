# Proposal: Print Capability with String Interpolation

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-26
**Depends on:** String Interpolation Proposal (Draft)
**Affects:** Capabilities, interpreter, WASM support

---

## Summary

Extend the `Print` capability to work seamlessly with string interpolation, enabling formatted output that can be redirected to different channels (stdout, buffers, logs).

```ori
@greet (name: str) -> void uses Print =
    print(msg: `Hello, {name}!`)

// In tests - capture output
@test_greet tests @greet () -> void =
    with Print = BufferPrint.new() in
    run(
        greet(name: "Alice"),
        assert_eq(actual: Print.output(), expected: "Hello, Alice!\n"),
    )

// In WASM - output captured automatically
// In native - writes to stdout
```

---

## Motivation

### Current State

The `print` function currently uses `println!` directly, which:
1. Cannot be captured for testing
2. Doesn't work in WASM (no stdout)
3. Cannot be redirected to files, buffers, or log systems

### With Print Capability

The `Print` capability makes output explicit and injectable:
- **Testing**: Capture output for assertions
- **WASM**: Buffer output for display in browser
- **Embedding**: Redirect to host application's logging
- **Consistency**: Same code works everywhere

### Integration with String Interpolation

Once string interpolation lands, `print` becomes the primary way to output formatted text:

```ori
print(msg: `User {user.name} logged in from {user.ip}`)
print(msg: `Processing {items.len()} items...`)
print(msg: `Result: {result:>10.2}`)  // with format specs
```

The Print capability ensures this output can be directed appropriately.

---

## Design

### Print Capability Trait

```ori
trait Print {
    @print (msg: str) -> void
    @println (msg: str) -> void  // print with newline
    @output () -> str            // get captured output (for testing)
    @clear () -> void            // clear captured output
}
```

### Built-in Functions

The `print` built-in becomes sugar for the capability:

```ori
// This:
print(msg: "Hello")

// Desugars to:
Print.println(msg: "Hello")
```

### Standard Implementations

```ori
// Default: writes to stdout
type StdoutPrint = {}

impl Print for StdoutPrint {
    @print (msg: str) -> void = // native stdout write
    @println (msg: str) -> void = // native stdout writeln
    @output () -> str = ""  // stdout doesn't capture
    @clear () -> void = ()
}

// For testing/WASM: captures to buffer
type BufferPrint = { buffer: mut str }

impl Print for BufferPrint {
    @print (msg: str) -> void =
        self.buffer = self.buffer + msg

    @println (msg: str) -> void =
        self.buffer = self.buffer + msg + "\n"

    @output () -> str = self.buffer

    @clear () -> void =
        self.buffer = ""
}
```

### Default Capability

Unlike other capabilities, `Print` has a default:
- Native: `StdoutPrint` is implicitly available
- WASM: `BufferPrint` is implicitly provided by runtime

This means simple programs don't need `uses Print`:

```ori
// Works without explicit capability (default Print used)
@main () -> void = print(msg: "Hello, World!")
```

But you can still override it:

```ori
@main () -> void =
    with Print = BufferPrint.new() in
    run(
        print(msg: "Captured"),
        let output = Print.output(),
        // output == "Captured\n"
    )
```

---

## String Interpolation Integration

### Template Strings in Print

```ori
let name = "Alice"
let age = 30

// Simple interpolation
print(msg: `Hello, {name}!`)
// Output: Hello, Alice!

// Multiple values
print(msg: `{name} is {age} years old`)
// Output: Alice is 30 years old

// With format specifiers
print(msg: `Balance: {balance:>10.2}`)
// Output: Balance:    1234.56

// Multi-line
print(msg: `
    User: {user.name}
    Email: {user.email}
    Status: {if user.active then "Active" else "Inactive"}
`)
```

### Type Safety

Interpolated values must implement `Printable`:

```ori
type Secret = { key: str }
// No Printable impl

let s = Secret { key: "abc123" }
print(msg: `Secret: {s}`)  // ERROR: Secret does not implement Printable
```

### Format Specifiers

All format specifiers from string interpolation work:

```ori
// Alignment
print(msg: `|{name:<10}|{name:>10}|{name:^10}|`)
// Output: |Alice     |     Alice|  Alice   |

// Numbers
print(msg: `Hex: {value:08x}, Binary: {value:b}`)
// Output: Hex: 000000ff, Binary: 11111111

// Precision
print(msg: `Pi: {pi:.4}`)
// Output: Pi: 3.1416
```

---

## WASM Runtime Integration

### Automatic Buffer Setup

The WASM runtime automatically provides `BufferPrint`:

```javascript
// JavaScript side
const result = run_ori(source);
console.log(result.printed);  // Contains all print output
```

### Implementation

```rust
// In playground WASM
thread_local! {
    static PRINT_BUFFER: RefCell<String> = RefCell::new(String::new());
}

// Print capability writes to buffer
fn wasm_print(msg: &str) {
    PRINT_BUFFER.with(|buf| {
        buf.borrow_mut().push_str(msg);
    });
}

// After execution, return buffer contents
fn get_print_output() -> String {
    PRINT_BUFFER.with(|buf| buf.borrow().clone())
}
```

---

## Testing

### Capturing Output

```ori
@greet (name: str) -> void uses Print =
    print(msg: `Hello, {name}!`)

@test_greet tests @greet () -> void =
    with Print = BufferPrint.new() in
    run(
        greet(name: "World"),
        assert_eq(actual: Print.output(), expected: "Hello, World!\n"),
    )
```

### Multiple Prints

```ori
@countdown (n: int) -> void uses Print =
    for i in n..0 do
        print(msg: `{i}...`)
    print(msg: "Liftoff!")

@test_countdown tests @countdown () -> void =
    with Print = BufferPrint.new() in
    run(
        countdown(n: 3),
        assert_eq(
            actual: Print.output(),
            expected: "3...\n2...\n1...\n0...\nLiftoff!\n",
        ),
    )
```

### No Output Expected

```ori
@silent_operation () -> int = 42

@test_silent tests @silent_operation () -> void =
    with Print = BufferPrint.new() in
    run(
        let result = silent_operation(),
        assert_eq(actual: Print.output(), expected: ""),
        assert_eq(actual: result, expected: 42),
    )
```

---

## Future Extensions

### Stderr Capability

```ori
trait Stderr {
    @eprint (msg: str) -> void
    @eprintln (msg: str) -> void
}

@warn (msg: str) -> void uses Stderr =
    eprintln(msg: `Warning: {msg}`)
```

### Colored Output

```ori
trait ColorPrint: Print {
    @print_colored (msg: str, color: Color) -> void
}

print_colored(msg: `Error: {err}`, color: Color.Red)
```

### Structured Output

```ori
// Future: typed print for structured logging
Print.structured(
    level: Info,
    message: `Request processed`,
    fields: { duration: elapsed, status: 200 },
)
```

---

## Implementation Notes

1. **Phase 1** (Current): Add `Print` capability with basic output redirection
2. **Phase 2** (After string interpolation): Full integration with template strings
3. **Phase 3** (Future): Stderr, colored output, structured logging

---

## Summary

The `Print` capability:
- Makes output explicit and testable
- Enables WASM support via buffer capture
- Integrates seamlessly with string interpolation
- Has sensible defaults (stdout for native, buffer for WASM)
- Follows Ori's capability philosophy while remaining ergonomic
