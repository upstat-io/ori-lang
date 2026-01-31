# Proposal: Formattable Trait and Format Specs

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, string formatting

---

## Summary

This proposal formalizes the `Formattable` trait and format specification syntax for customized string formatting.

---

## Problem Statement

The spec mentions `Formattable` but leaves unclear:

1. **Trait definition**: What is the exact signature?
2. **Format spec syntax**: What specifiers are supported?
3. **Relationship to Printable**: How do they differ?
4. **Integration**: How does formatting work in templates?
5. **Custom formats**: How to implement for user types?

---

## Formattable Trait

### Definition

```ori
trait Formattable {
    @format (self, spec: FormatSpec) -> str
}
```

### FormatSpec Type

```ori
type FormatSpec = {
    fill: Option<char>,
    align: Option<Alignment>,
    sign: Option<Sign>,
    width: Option<int>,
    precision: Option<int>,
    format_type: Option<FormatType>,
}

type Alignment = Left | Center | Right

type Sign = Plus | Minus | Space

type FormatType = Binary | Octal | Hex | HexUpper | Exp | ExpUpper | Fixed | Percent
```

---

## Format Spec Syntax

Format specifications use the syntax:

```
[[fill]align][sign][#][0][width][.precision][type]
```

### Components

| Component | Syntax | Description |
|-----------|--------|-------------|
| Fill | Any character | Padding character (default: space) |
| Align | `<` `>` `^` | Left, right, center alignment |
| Sign | `+` `-` ` ` | Sign display for numbers |
| `#` | `#` | Alternate form (prefix for hex, etc.) |
| `0` | `0` | Zero-pad (implies right-align) |
| Width | Integer | Minimum field width |
| Precision | `.N` | Decimal places or max string length |
| Type | Letter | Format type (b, o, x, X, e, E, f, %) |

### Examples

| Spec | Input | Output |
|------|-------|--------|
| `{:>10}` | `"hello"` | `"     hello"` |
| `{:<10}` | `"hello"` | `"hello     "` |
| `{:^10}` | `"hello"` | `"  hello   "` |
| `{:*^10}` | `"hello"` | `"**hello***"` |
| `{:08}` | `42` | `"00000042"` |
| `{:+}` | `42` | `"+42"` |
| `{:.2}` | `3.14159` | `"3.14"` |
| `{:b}` | `42` | `"101010"` |
| `{:x}` | `255` | `"ff"` |
| `{:X}` | `255` | `"FF"` |
| `{:#x}` | `255` | `"0xff"` |
| `{:e}` | `1234.5` | `"1.2345e+03"` |
| `{:.0%}` | `0.75` | `"75%"` |

---

## Format Types

### Integer Types

| Type | Description | Example |
|------|-------------|---------|
| `b` | Binary | `42` → `"101010"` |
| `o` | Octal | `42` → `"52"` |
| `x` | Hex (lowercase) | `255` → `"ff"` |
| `X` | Hex (uppercase) | `255` → `"FF"` |

### Float Types

| Type | Description | Example |
|------|-------------|---------|
| `e` | Scientific (lowercase) | `1234.5` → `"1.2345e+03"` |
| `E` | Scientific (uppercase) | `1234.5` → `"1.2345E+03"` |
| `f` | Fixed-point | `1234.5` → `"1234.500000"` |
| `%` | Percentage | `0.75` → `"75%"` |

### Alternate Form (`#`)

| Type | Without `#` | With `#` |
|------|-------------|----------|
| `b` | `"101010"` | `"0b101010"` |
| `o` | `"52"` | `"0o52"` |
| `x` | `"ff"` | `"0xff"` |
| `X` | `"FF"` | `"0xFF"` |

---

## String Template Integration

### Basic

```ori
let name = "world"
`hello {name}`  // Uses Printable.to_str()
```

### With Format Spec

```ori
let n = 42
`value: {n:08}`      // "value: 00000042"
`hex: {n:#x}`        // "hex: 0x2a"

let pi = 3.14159
`pi = {pi:.2}`       // "pi = 3.14"
`pi = {pi:>10.2}`    // "pi =       3.14"
```

### Escaping

```ori
`literal braces: \{\}`  // "literal braces: {}"
```

---

## Relationship to Printable

### Printable

```ori
trait Printable {
    @to_str (self) -> str
}
```

- Used for basic string conversion
- Required for `{x}` in templates (no format spec)
- Returns human-readable default representation

### Formattable

- Used for formatted string conversion
- Required for `{x:spec}` in templates
- Accepts format specification

### Blanket Implementation

All `Printable` types have a blanket `Formattable` implementation:

```ori
impl<T: Printable> Formattable for T {
    @format (self, spec: FormatSpec) -> str = run(
        let base = self.to_str(),
        apply_format(s: base, spec: spec),
    )
}
```

This applies width, alignment, and fill. Type-specific formatting (binary, hex, etc.) is only available for types that implement Formattable directly.

---

## Standard Implementations

### int

```ori
impl Formattable for int {
    @format (self, spec: FormatSpec) -> str = match(spec.format_type,
        Some(Binary) -> format_int_base(n: self, base: 2, spec: spec),
        Some(Octal) -> format_int_base(n: self, base: 8, spec: spec),
        Some(Hex) -> format_int_base(n: self, base: 16, lowercase: true, spec: spec),
        Some(HexUpper) -> format_int_base(n: self, base: 16, lowercase: false, spec: spec),
        _ -> format_int_decimal(n: self, spec: spec),
    )
}
```

### float

```ori
impl Formattable for float {
    @format (self, spec: FormatSpec) -> str = match(spec.format_type,
        Some(Exp) -> format_scientific(n: self, uppercase: false, spec: spec),
        Some(ExpUpper) -> format_scientific(n: self, uppercase: true, spec: spec),
        Some(Fixed) -> format_fixed(n: self, spec: spec),
        Some(Percent) -> format_percent(n: self, spec: spec),
        _ -> format_float_default(n: self, spec: spec),
    )
}
```

### str

```ori
impl Formattable for str {
    @format (self, spec: FormatSpec) -> str = run(
        let s = match(spec.precision,
            Some(n) -> self.take(count: n),
            None -> self,
        ),
        apply_alignment(s: s, spec: spec),
    )
}
```

---

## Custom Implementation

### For User Types

```ori
type Money = { cents: int }

impl Formattable for Money {
    @format (self, spec: FormatSpec) -> str = run(
        let dollars = self.cents / 100,
        let cents = self.cents % 100,
        let base = `${dollars}.{cents:02}`,
        apply_alignment(s: base, spec: spec),
    )
}
```

```ori
let price = Money { cents: 1995 }
`Price: {price:>10}`  // "Price:     $19.95"
```

### Delegating to Inner Value

```ori
type UserId = int

impl Formattable for UserId {
    @format (self, spec: FormatSpec) -> str = self.0.format(spec: spec)
}
```

---

## Parsing Format Specs

The compiler parses format specs at compile time:

```ori
`{value:>10.2f}`
// Parsed to:
// FormatSpec {
//     fill: None,
//     align: Some(Right),
//     sign: None,
//     width: Some(10),
//     precision: Some(2),
//     format_type: Some(Fixed),
// }
```

Invalid specs are compile errors:

```ori
`{value:abc}`  // ERROR: invalid format spec
```

---

## Error Messages

### Invalid Spec Syntax

```
error[E0970]: invalid format specification
  --> src/main.ori:5:15
   |
 5 | let s = `{n:xyz}`
   |               ^^^ unknown format type 'xyz'
   |
   = note: valid types: b, o, x, X, e, E, f, %
```

### Type Mismatch

```
error[E0971]: format type not supported
  --> src/main.ori:5:15
   |
 5 | let s = `{name:x}`
   |               ^ hex format not supported for `str`
   |
   = note: hex format only works with integers
```

### Not Formattable

```
error[E0972]: `MyType` does not implement `Formattable`
  --> src/main.ori:5:10
   |
 5 | let s = `{value:>10}`
   |          ^^^^^^^^^^^ trait not implemented
   |
   = help: implement `Formattable` or `Printable` for `MyType`
```

---

## Spec Changes Required

### Update `16-formatting.md`

Expand with:
1. Complete FormatSpec structure
2. All format types
3. Parsing rules
4. Standard implementations

---

## Summary

| Aspect | Details |
|--------|---------|
| Trait | `trait Formattable { @format (self, spec: FormatSpec) -> str }` |
| Spec syntax | `[[fill]align][sign][#][0][width][.precision][type]` |
| Alignments | `<` left, `>` right, `^` center |
| Int types | `b` binary, `o` octal, `x`/`X` hex |
| Float types | `e`/`E` scientific, `f` fixed, `%` percent |
| Alternate | `#` adds prefix (0b, 0o, 0x) |
| Blanket | Printable types get basic Formattable |
