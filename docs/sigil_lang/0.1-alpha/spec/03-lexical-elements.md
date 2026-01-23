# Lexical Elements

This section defines the lexical structure of Sigil source code: tokens, comments, identifiers, keywords, and literals.

## Tokens

A token is the smallest lexical unit of the language.

```
token         = identifier | keyword | literal | operator | delimiter .
```

## Comments

Comments serve as documentation and are ignored by the compiler.

```
comment       = line_comment .
line_comment  = "//" { unicode_char - newline } newline .
```

A line comment starts with `//` and extends to the end of the line. Comments must appear on their own line; inline comments (comments following code on the same line) are not permitted.

```sigil
// This is a valid comment
@add (a: int, b: int) -> int = a + b
```

It is a syntax error to place a comment after code on the same line:

```sigil
// INVALID - inline comments are not allowed
@add (a: int, b: int) -> int = a + b  // syntax error
```

### Documentation Comments

Documentation comments use special marker prefixes:

```
doc_comment   = "//" doc_marker { unicode_char - newline } newline .
doc_marker    = "#" | "@param" | "@field" | "!" | ">" .
```

| Marker | Purpose |
|--------|---------|
| `#` | Description |
| `@param` | Parameter documentation |
| `@field` | Field documentation |
| `!` | Warning or important note |
| `>` | Example (input -> output) |

```sigil
// #Adds two integers
// @param a first operand
// @param b second operand
// >add(2, 3) -> 5
@add (a: int, b: int) -> int = a + b
```

## Identifiers

An identifier names a program entity such as a variable, function, type, or module.

```
identifier    = ( letter | "_" ) { letter | digit | "_" } .
letter        = 'A' ... 'Z' | 'a' ... 'z' .
digit         = '0' ... '9' .
```

An identifier must start with a letter or underscore and may contain letters, digits, and underscores. Identifiers are case-sensitive.

```sigil
x
_x
userName
user_name
Point2D
MAX_VALUE
```

### Identifier Restrictions

An identifier must not:

1. Start with a digit
2. Be a reserved keyword
3. Be empty

## Keywords

Keywords are reserved identifiers with special meaning.

### Reserved Keywords

The following keywords are reserved and cannot be used as identifiers:

```
async       break       continue    do          else        false
for         if          impl        in          let         loop
match       mut         pub         self        Self        then
trait       true        type        use         uses        void
where       with        yield
```

### Context-Sensitive Keywords

The following identifiers are keywords only in specific contexts (pattern expressions). Outside these contexts, they may be used as ordinary identifiers:

```
cache       collect     filter      find        fold
map         parallel    recurse     retry       run
timeout     try         validate
```

## Operators and Delimiters

### Operators

```
operator      = arith_op | comp_op | logic_op | bit_op | other_op .

arith_op      = "+" | "-" | "*" | "/" | "%" | "div" .
comp_op       = "==" | "!=" | "<" | ">" | "<=" | ">=" .
logic_op      = "&&" | "||" | "!" .
bit_op        = "&" | "|" | "^" | "~" .
other_op      = ".." | "..=" | "??" | "?" | "->" | "=>" .
```

### Delimiters

```
delimiter     = "(" | ")" | "[" | "]" | "{" | "}"
              | "," | ":" | ";" | "." | "@" | "$" | "#" .
```

### Operator Precedence

Operators have the following precedence, from highest (1) to lowest (13):

| Precedence | Operators | Associativity |
|------------|-----------|---------------|
| 1 | `.` `[]` `()` `?` | Left |
| 2 | `!` `-` `~` (unary) | Right |
| 3 | `*` `/` `%` `div` | Left |
| 4 | `+` `-` | Left |
| 5 | `..` `..=` | Left |
| 6 | `<` `>` `<=` `>=` | Left |
| 7 | `==` `!=` | Left |
| 8 | `&` | Left |
| 9 | `^` | Left |
| 10 | `|` | Left |
| 11 | `&&` | Left |
| 12 | `\|\|` | Left |
| 13 | `??` | Left |

## Literals

A literal represents a constant value.

```
literal       = int_literal | float_literal | string_literal | char_literal
              | bool_literal | duration_literal | size_literal .
```

### Integer Literals

```
int_literal   = decimal_lit | hex_lit .
decimal_lit   = digit { digit | "_" } .
hex_lit       = "0x" hex_digit { hex_digit | "_" } .
hex_digit     = digit | "a" ... "f" | "A" ... "F" .
```

Underscores may appear between digits for readability but have no semantic meaning.

```sigil
42
1_000_000
0
0x2a
0xFF
```

Integer literals represent values of type `int` (64-bit signed integer). Hex literals use the `0x` prefix.

### Floating-Point Literals

```
float_literal = decimal_lit "." decimal_lit [ exponent ] .
exponent      = ( "e" | "E" ) [ "+" | "-" ] decimal_lit .
```

```sigil
3.14159
0.5
1.5e10
2.5e-8
```

Floating-point literals represent values of type `float` (64-bit IEEE 754).

### String Literals

```
string_literal = '"' { string_char } '"' .
string_char    = unicode_char - ( '"' | '\' | newline ) | escape .
escape         = '\' ( '"' | '\' | 'n' | 't' | 'r' ) .
```

| Escape | Character |
|--------|-----------|
| `\\` | Backslash (U+005C) |
| `\"` | Double quote (U+0022) |
| `\n` | Newline (U+000A) |
| `\t` | Horizontal tab (U+0009) |
| `\r` | Carriage return (U+000D) |

```sigil
"hello"
""
"line1\nline2"
"She said \"hello\""
```

String literals represent values of type `str` (UTF-8 string).

### Character Literals

```
char_literal   = "'" char_char "'" .
char_char      = unicode_char - ( "'" | '\' | newline ) | char_escape .
char_escape    = '\' ( "'" | '\' | 'n' | 't' | 'r' | '0' ) .
```

| Escape | Character |
|--------|-----------|
| `\\` | Backslash (U+005C) |
| `\'` | Single quote (U+0027) |
| `\n` | Newline (U+000A) |
| `\t` | Horizontal tab (U+0009) |
| `\r` | Carriage return (U+000D) |
| `\0` | Null (U+0000) |

```sigil
'a'
'λ'
'🦀'
'\n'
'\''
```

Character literals represent values of type `char` (Unicode scalar value).

### Boolean Literals

```
bool_literal  = "true" | "false" .
```

Boolean literals represent values of type `bool`.

### Duration Literals

```
duration_literal = int_literal duration_unit .
duration_unit    = "ms" | "s" | "m" | "h" .
```

| Unit | Meaning |
|------|---------|
| `ms` | Milliseconds |
| `s` | Seconds |
| `m` | Minutes |
| `h` | Hours |

```sigil
100ms
30s
5m
2h
```

Duration literals represent values of type `Duration`.

### Size Literals

```
size_literal  = int_literal size_unit .
size_unit     = "b" | "kb" | "mb" | "gb" .
```

| Unit | Bytes |
|------|-------|
| `b` | 1 |
| `kb` | 1,024 |
| `mb` | 1,048,576 |
| `gb` | 1,073,741,824 |

```sigil
1024b
4kb
10mb
2gb
```

Size literals represent values of type `Size`.

## Semicolons

Sigil does not require semicolons as statement terminators. Newlines serve this purpose in most contexts. Within pattern expressions (inside parentheses), commas separate elements.

## Line Continuation

### Natural Continuation

A newline does not terminate an expression when it follows:

1. A binary operator (`+`, `-`, `*`, `/`, `%`, `div`, `==`, `!=`, `<`, `>`, `<=`, `>=`, `&&`, `||`, `&`, `|`, `^`, `..`, `..=`, `??`)
2. An opening delimiter (`(`, `[`, `{`)
3. A comma

This allows multi-line expressions without explicit continuation markers:

```sigil
if a > 0
   && b > 0
   && c > 0
then result
else fallback
```

### Continuation Within Delimiters

Within parentheses, brackets, or braces, newlines are treated as whitespace. Commas separate elements:

```sigil
fold(
    .over: items,
    .init: 0,
    .op: +,
)
```

## Trailing Commas

Trailing commas are syntactically permitted in all comma-separated lists:

- Function parameters and arguments
- Pattern properties
- List and map literals
- Struct fields
- Match arms

```
list_elements = element { "," element } [ "," ] .
```

> **Note:** The canonical formatter enforces trailing commas in multi-line constructs. Single-line constructs omit trailing commas.
