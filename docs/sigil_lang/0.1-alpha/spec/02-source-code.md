# Source Code Representation

Source code is Unicode text encoded in UTF-8.

## Characters

```
unicode_char  = /* any Unicode code point except NUL (U+0000) */ .
letter        = 'A' … 'Z' | 'a' … 'z' .
digit         = '0' … '9' .
newline       = /* U+000A */ .
whitespace    = ' ' | '\t' | '\r' | newline .
```

Carriage return (U+000D) followed by newline is normalized to newline. A lone carriage return is treated as newline.

## Source Files

```
source_file = { import } { declaration } .
```

Source files use `.si` extension. Test files use `.test.si` extension.

File names must:
- Start with ASCII letter or underscore
- Contain only ASCII letters, digits, underscores
- End with `.si` or `.test.si`

## Encoding

Source files must be valid UTF-8 without byte order mark. Invalid UTF-8 or presence of BOM is an error.

## Line Continuation

A newline does not terminate the logical line when preceded by:
- Binary operators: `+`, `-`, `*`, `/`, `%`, `&&`, `||`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `..`, `..=`
- Opening delimiters: `(`, `[`, `{`
- Comma, assignment (`=`), arrow (`->`), colon (`:`)

## Module Mapping

Each source file defines one module. Module name derives from file path relative to source root:

| File Path | Module Name |
|-----------|-------------|
| `src/main.si` | `main` |
| `src/http/client.si` | `http.client` |

See [Modules](12-modules.md).
