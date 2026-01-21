# Source Code Representation

This section defines how source code is represented and processed.

## Source Files

A Sigil source file is a sequence of Unicode code points encoded in UTF-8. Each source file must be valid UTF-8.

### File Extension

Source files use the `.si` extension. Test files use the `.test.si` extension.

### File Names

A source file name must:

1. Start with an ASCII letter (`a`-`z`, `A`-`Z`) or underscore (`_`)
2. Contain only ASCII letters, digits (`0`-`9`), and underscores
3. End with `.si` or `.test.si`

The file name (excluding extension) determines the module name.

## Characters

### Unicode

Source code uses the Unicode character set.

```
unicode_char = /* any Unicode code point */ .
```

### Letters and Digits

```
letter        = 'A' ... 'Z' | 'a' ... 'z' .
digit         = '0' ... '9' .
```

### Line Terminators

```
newline       = /* U+000A (LF) */ .
```

A line terminator is a newline character (U+000A). Carriage return (U+000D) followed by newline is normalized to a single newline. A lone carriage return is treated as a newline.

### Whitespace

```
whitespace    = ' ' | '\t' | '\r' | newline .
```

Whitespace characters are space (U+0020), horizontal tab (U+0009), carriage return (U+000D), and newline (U+000A).

## Source Structure

A source file consists of a sequence of tokens separated by whitespace and comments.

```
source_file   = { whitespace | comment | token } .
```

### Line Structure

Lines continue naturally after:

- Binary operators: `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `..`, `..=`
- Opening delimiters: `(`, `[`, `{`
- Comma: `,`
- Assignment: `=`
- Arrow: `->`
- Colon: `:`

A newline following these tokens does not terminate the logical line.

### Significant Newlines

A newline is significant (terminates a statement) when it follows a token that could end an expression and is not followed by a continuation token.

## Encoding

### UTF-8 Requirement

Source files must be encoded in UTF-8 without a byte order mark (BOM). If a BOM is present, it is an error.

### Invalid UTF-8

If a source file contains invalid UTF-8 sequences, compilation fails with a diagnostic indicating the byte offset of the first invalid sequence.

### NUL Characters

The NUL character (U+0000) is not permitted in source files. Its presence is an error.

## Source Organization

### Program Structure

A program consists of one or more source files organized in a directory hierarchy.

```
program       = { source_file } .
```

### Source Root

The source root is the directory from which module paths are resolved. It is typically `src/` or configured in the project manifest.

### Module Mapping

Each source file defines exactly one module. The module name is derived from the file path relative to the source root:

| File Path | Module Name |
|-----------|-------------|
| `src/main.si` | `main` |
| `src/math.si` | `math` |
| `src/http/client.si` | `http.client` |

See [Modules](12-modules.md) for complete module system specification.
