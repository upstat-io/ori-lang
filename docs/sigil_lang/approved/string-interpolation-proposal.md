# Proposal: String Interpolation

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Lexer, parser, type system, standard library

---

## Summary

Add string interpolation to Sigil using **template strings** with backtick delimiters. Regular double-quoted strings remain unchanged (no interpolation).

```sigil
let name = "Alice"
let age = 30
print(`Hello, {name}! You are {age} years old.`)
// Output: Hello, Alice! You are 30 years old.
```

**Two string types:**
- `"..."` — regular strings, no interpolation, no escaping of braces
- `` `...` `` — template strings, `{expr}` interpolation

---

## Motivation

### The Problem

Currently, Sigil requires verbose string concatenation:

```sigil
// Current approach - verbose and error-prone
let message = "User " + name + " (id: " + str(id) + ") logged in at " + str(time)

// Multi-line is even worse
let report = "Report for " + date + "\n" +
    "Total: " + str(total) + "\n" +
    "Average: " + str(average)
```

Problems with concatenation:
1. **Verbose** - lots of `+` and `str()` calls
2. **Error-prone** - easy to forget spaces or `str()` conversions
3. **Hard to read** - the structure of the output is obscured
4. **Inconsistent** - different types need different conversion functions

### Prior Art

| Language | Syntax | Notes |
|----------|--------|-------|
| Python | `f"Hello {name}"` | f-strings with format specs |
| JavaScript | `` `Hello ${name}` `` | Template literals with `${}` |
| Rust | `format!("Hello {name}")` | Macro-based |
| Kotlin | `"Hello $name"` or `"Hello ${expr}"` | Direct in strings |
| Swift | `"Hello \(name)"` | Backslash-paren |
| C# | `$"Hello {name}"` | Prefix marker |

### Design Goals

1. **Readable** - interpolated strings should look like the output
2. **Explicit** - clear which strings support interpolation (backticks)
3. **Type-safe** - compile-time checking of interpolated expressions
4. **Consistent** - works with Sigil's existing `Printable` trait
5. **Ergonomic** - no escaping needed for braces in regular strings

---

## Design

### Two String Types

**Regular strings** (`"..."`) — no interpolation:

```sigil
let greeting = "Hello, World!"
let json = "{\"key\": \"value\"}"  // braces are just characters
let empty = "{}"                    // no escaping needed
```

**Template strings** (`` `...` ``) — with interpolation:

```sigil
let name = "World"
`Hello, {name}!`  // "Hello, World!"
```

### Basic Syntax

Expressions inside `{...}` in template strings are interpolated:

```sigil
let name = "World"
`Hello, {name}!`  // "Hello, World!"
```

**Key points:**
- Only backtick strings support interpolation
- Expressions must implement `Printable` trait
- Curly braces are the interpolation delimiter

### Expressions

Any expression can be interpolated in template strings:

```sigil
// Variables
`Name: {name}`

// Field access
`Position: {point.x}, {point.y}`

// Method calls
`Length: {items.len()}`

// Arithmetic
`Sum: {a + b}`

// Function calls
`Absolute: {abs(value)}`

// Conditionals
`Status: {if active then "on" else "off"}`

// Complex expressions (parentheses for clarity)
`Result: {(x * 2 + y) / z}`
```

### Escaping

**In template strings:**
- `{{` and `}}` for literal braces
- `` \` `` for literal backtick
- Standard escapes: `\\`, `\n`, `\t`, `\r`

```sigil
`Use {{braces}} for interpolation`  // "Use {braces} for interpolation"
`JSON: {{"key": {value}}}`          // JSON: {"key": 42}
`Code uses \` backticks`            // "Code uses ` backticks"
```

**In regular strings:**
- Braces are literal (no escaping needed)
- `\"` for literal quote
- Standard escapes: `\\`, `\n`, `\t`, `\r`

```sigil
"{\"key\": \"value\"}"  // {"key": "value"}
```

**Best practice:** Use regular strings for brace-heavy content:

```sigil
// Better: use regular string for JSON templates
let template = "{\"key\": \"value\"}"

// Only use template string when interpolating
let filled = `{"key": "{value}"}`
```

### Multi-line Strings

Both string types support multi-line:

```sigil
// Multi-line template string with interpolation
let report = `
    Report for {date}
    ================
    Total items: {total}
    Average: {average}
    Status: {status}
`

// Multi-line regular string (no interpolation)
let json_template = "
    {
        \"users\": [],
        \"count\": 0
    }
"
```

### Type Requirements

Interpolated expressions must implement `Printable`:

```sigil
trait Printable {
    @to_str (self) -> str
}

// All primitives implement Printable
`Number: {42}`        // OK
`Float: {3.14}`       // OK
`Bool: {true}`        // OK
`Char: {'x'}`         // OK

// Custom types need Printable impl
type Point = { x: int, y: int }

impl Printable for Point {
    @to_str (self) -> str = `({self.x}, {self.y})`
}

let p = Point { x: 10, y: 20 }
`Location: {p}`  // "Location: (10, 20)"
```

### Compile-Time Errors

```sigil
type Secret = { key: str }
// No Printable impl for Secret

let s = Secret { key: "abc123" }
`Value: {s}`  // ERROR: Secret does not implement Printable
```

---

## Format Specifiers

### Basic Formatting

Optional format specifiers after a colon in template strings:

```sigil
// Width and alignment
`{name:10}`      // right-align in 10 chars (default)
`{name:<10}`     // left-align in 10 chars
`{name:^10}`     // center in 10 chars

// Numeric formatting
`{price:.2}`     // 2 decimal places: "19.99"
`{count:05}`     // zero-pad to 5 digits: "00042"
`{hex:x}`        // hexadecimal: "ff"
`{hex:X}`        // uppercase hex: "FF"
`{num:b}`        // binary: "101010"

// Combined
`{price:>10.2}`  // right-align, 10 wide, 2 decimals
```

### Format Spec Grammar

```
format_spec := [[fill]align][width][.precision][type]
fill        := <any character>
align       := '<' | '>' | '^'
width       := <integer>
precision   := <integer>
type        := 'b' | 'x' | 'X' | 'o' | 'e' | 'E'
```

### Examples

```sigil
// Table formatting
for item in items do
    print(`{item.name:<20} {item.price:>8.2} {item.qty:>5}`)

// Output:
// Apple                   1.99    10
// Banana                  0.59    25
// Orange Juice            4.99     3

// Debug output
print(`Value: {x:08x}`)  // "Value: 0000002a"

// Percentages
let ratio = 0.756
`{ratio:.1%}`  // "75.6%" (if we support % type)
```

---

## Implementation

### Lexer Changes

The lexer handles two string literal types:

```
// Regular string - no interpolation
STRING_LITERAL   := '"' (string_char)* '"'
string_char      := <any char except '"', '\'>
                 | escape_sequence

// Template string - with interpolation
TEMPLATE_LITERAL := '`' (template_char | interpolation)* '`'
template_char    := <any char except '`', '\', '{', '}'>
                 | escape_sequence
                 | '{{' | '}}'
interpolation    := '{' expression [':' format_spec] '}'
```

### Parser Changes

Regular strings remain simple string literals. Template strings become a sequence of parts:

```sigil
// Internal representation
type StringPart =
    | Literal(text: str)
    | Interpolation(expr: Expr, format: Option<FormatSpec>)

// `Hello, {name}!` becomes:
[Literal("Hello, "), Interpolation(name, None), Literal("!")]

// "Hello, {name}!" remains a plain string containing literal braces
```

### Desugaring

Template strings desugar to concatenation with formatting:

```sigil
// Source
`Hello, {name}! You are {age} years old.`

// Desugars to (conceptually)
str_concat([
    "Hello, ",
    name.to_str(),
    "! You are ",
    age.to_str(),
    " years old."
])

// With format specifiers
`{value:.2}`

// Desugars to
format(value, FormatSpec { precision: Some(2), ... })
```

### Standard Library Additions

```sigil
// Format trait for custom formatting
trait Formattable {
    @format (self, spec: FormatSpec) -> str
}

type FormatSpec = {
    fill: Option<char>,
    align: Option<Alignment>,
    width: Option<int>,
    precision: Option<int>,
    format_type: Option<FormatType>,
}

type Alignment = Left | Right | Center
type FormatType = Binary | Hex | HexUpper | Octal | Exp | ExpUpper

// Default: Formattable delegates to Printable
impl<T: Printable> Formattable for T {
    @format (self, spec: FormatSpec) -> str =
        apply_format(self.to_str(), spec)
}
```

---

## Examples

### Error Messages

```sigil
@validate_age (age: int) -> Result<int, str> =
    if age < 0 then Err(`Age cannot be negative: {age}`)
    else if age > 150 then Err(`Age seems unrealistic: {age}`)
    else Ok(age)
```

### Logging

```sigil
@process_request (req: Request) -> Response uses Logger =
    run(
        Logger.info(`Processing request {req.id} from {req.client_ip}`),
        let result = handle(req),
        Logger.info(`Request {req.id} completed in {result.duration}`),
        result.response,
    )
```

### SQL Queries (Parameterized)

Note: For SQL, use parameterized queries, not interpolation:

```sigil
// WRONG - SQL injection risk
query(`SELECT * FROM users WHERE name = '{name}'`)

// RIGHT - use query builder or parameters
query(.sql: "SELECT * FROM users WHERE name = ?", .params: [name])
```

### HTML Templates

```sigil
@render_greeting (user: User) -> str =
    `
    <div class="greeting">
        <h1>Welcome, {user.name}!</h1>
        <p>You have {user.unread_count} unread messages.</p>
        <p>Last login: {user.last_login}</p>
    </div>
    `
```

### JSON (Clean with Regular Strings)

```sigil
// Use regular strings for JSON structure, template strings when interpolating
@to_json (user: User) -> str =
    `{"name": "{user.name}", "age": {user.age}, "active": {user.active}}`

// No interpolation needed? Use regular string - no escaping
@empty_response () -> str = "{\"status\": \"ok\", \"data\": []}"
```

### Debug Output

```sigil
@debug_point (p: Point) -> void =
    print(`Point { x: {p.x}, y: {p.y} }`)
    // Output: Point { x: 10, y: 20 }
```

---

## Design Decisions

### Why Two String Types?

We chose backtick template strings (`` `...` ``) separate from regular strings (`"..."`) because:

1. **Explicit opt-in** - clear at a glance which strings support interpolation
2. **No escaping for brace-heavy content** - JSON, CSS, code snippets work naturally in regular strings
3. **Familiar** - JavaScript developers know backticks mean "template"
4. **Backwards compatible** - existing `"..."` strings unchanged

### Why Not `${expr}` (JavaScript Style)?

Sigil uses `$name` for constants (compile-time values):

```sigil
$timeout = 30s
$max_retries = 3
```

Using `${expr}` would create visual confusion:
- `$timeout` outside strings = constant reference
- `${timeout}` inside strings = interpolation... of what?

We avoid this by using `{expr}` without the `$` prefix.

### Why Curly Braces?

| Option | Example | Problem |
|--------|---------|---------|
| `$name` | `` `Hello $name` `` | Conflicts with Sigil's `$constants` |
| `${expr}` | `` `Hello ${name}` `` | Same conflict |
| `\(expr)` | `` `Hello \(name)` `` | Escapes are for special chars |
| `#{expr}` | `` `Hello #{name}` `` | Conflicts with `#` length syntax |
| **`{expr}`** | `` `Hello {name}` `` | Clean, common, no conflicts |

Curly braces are:
- Familiar (Python, Rust, C#, Kotlin use them)
- Don't conflict with Sigil syntax
- Easy to type
- Clear visual boundary

### Why Require Printable?

Explicit trait requirement because:
1. Not all types should be stringifiable (e.g., secrets, handles)
2. Compile-time error is better than runtime surprise
3. Consistent with Sigil's explicit philosophy
4. Custom types control their representation

### Format Specifiers: Optional Complexity

Format specifiers are optional. Simple interpolation covers 90% of cases:

```sigil
// Most common usage - no format spec needed
`Hello, {name}!`
`Count: {items.len()}`
```

Format specs are there when you need them (tables, debugging, specific formats).

---

## Alternatives Considered

### 1. Single String Type with `{expr}` (Original Proposal)

```sigil
"Hello, {name}!"  // interpolation in regular strings
"JSON: {{"key": {value}}}"  // must escape braces
```

Rejected: Too much escaping for JSON, CSS, and other brace-heavy content.

### 2. JavaScript-Style `${expr}`

```sigil
`Hello, ${name}!`
```

Rejected: Conflicts visually with Sigil's `$constant` syntax.

### 3. Macro-Based (Rust style)

```sigil
format!("Hello, {name}!")
```

Rejected: Requires macro system, more verbose for common case.

### 4. Method-Based

```sigil
"Hello, {}!".format(name)
```

Rejected: Positional arguments are error-prone, doesn't show structure.

### 5. Tagged Templates (JavaScript style)

```sigil
sql`SELECT * FROM users WHERE name = ${name}`
```

Rejected: Adds complexity for specialized use case. Better to have query builders.

### 6. No Interpolation (Status Quo)

Keep concatenation only.

Rejected: Too verbose, hurts readability, common source of bugs.

---

## Migration

This is purely additive. Existing code is unaffected:

```sigil
// Still valid - regular strings unchanged
"Hello, " + name + "!"
"{}" // still just a string containing braces

// New alternative - use template strings
`Hello, {name}!`
```

No breaking changes. Regular `"..."` strings behave exactly as before.

---

## Future Extensions

### 1. Raw Template Strings

If Sigil adds raw strings (no escape processing):

```sigil
r`Path: {path}\n stays literal`
// vs
`Path: {path}\n becomes newline`
```

### 2. Custom Formatters

User-defined format types:

```sigil
impl Formattable for Money {
    @format (self, spec: FormatSpec) -> str =
        match(spec.format_type,
            Some(Currency) -> `${self.dollars}.{self.cents:02}`,
            _ -> self.to_str(),
        )
}

`{price:$}`  // "$19.99"
```

### 3. Compile-Time Format Validation

Validate format specs at compile time:

```sigil
`{name:.2}`  // ERROR: precision not valid for str type
```

---

## Summary

Two string types:
- **`"..."`** — regular strings, no interpolation, braces are literal
- **`` `...` ``** — template strings with `{expr}` interpolation

Key features:
- Type-safe via `Printable` trait
- Optional format specifiers for advanced use
- Escape with `{{` and `}}` only in template strings
- No conflict with `$constants`

```sigil
let user = "Alice"
let items = 3
print(`Hello, {user}! You have {items} new messages.`)
```
