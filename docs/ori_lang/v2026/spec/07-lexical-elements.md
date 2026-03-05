---
title: "Lexical Elements"
description: "Clause 7: Ori Language Specification — Lexical Elements"
order: 7
section: "Language"
---

# 7 Lexical Elements

> **Grammar:** See [grammar.ebnf](grammar.md) § LEXICAL GRAMMAR

A _token_ is the smallest lexical unit of the language. The five token classes are: identifiers, keywords, literals, operators, and delimiters.

Whitespace — consisting of space (U+0020), horizontal tab (U+0009), and newline (U+000A, after normalization per [6.1.1](06-source-code.md#611-line-endings)) — separates tokens but is otherwise not significant. Ori is not indentation-sensitive.

At least one whitespace character or delimiter shall separate adjacent tokens that would otherwise form a single token. For instance, `intx` is the identifier `intx`, not the keyword `int` followed by the identifier `x`.

## 7.1 Comments

Comments begin with `//` and extend to the end of the line. Inline comments (comments following code on the same line) are not permitted.

```ori
// This is a valid comment
@add (a: int, b: int) -> int = a + b;

@sub (a: int, b: int) -> int = a - b;  // error: inline comment
```

The content of a comment is arbitrary text: any _unicode_char_ (see [6.1](06-source-code.md#61-characters)) except newline may appear in a comment.

### 7.1.1 Doc comments

Doc comments use special markers to classify documentation content:

| Marker | Purpose | Example |
|--------|---------|---------|
| *(none)* | Description | `// This is a description.` |
| `*` | Parameter or field | `// * name: Description` |
| `!` | Warning or panic | `// ! Panics if x is negative` |
| `>` | Example | `// > func(x: 1) -> 2` |

The canonical form for member documentation is `// * name: description` with a space after `*` and a colon always required.

Any comment immediately preceding a declaration is treated as documentation for that declaration. Non-documentation comments shall be separated from declarations by a blank line:

```ori
// TODO: refactor this later

// Computes the sum of two integers.
@add (a: int, b: int) -> int = a + b;
```

## 7.2 Identifiers

An _identifier_ names a program entity such as a variable, type, function, or module.

```ebnf
identifier = ( letter | "_" ) { letter | digit | "_" } .
```

where `letter` and `digit` are as defined in [6.1.2](06-source-code.md#612-letters-and-digits).

Identifiers are restricted to ASCII letters, ASCII digits, and underscore. They are case-sensitive: `count`, `Count`, and `COUNT` are three distinct identifiers.

An identifier shall not be a reserved keyword (see 7.3.1) or future-reserved keyword (see 7.3.2) unless it appears in member position (after `.`).

The maximum identifier length is implementation-defined; an implementation shall accept identifiers of at least 1 000 characters.

Identifiers are compared byte-for-byte. No Unicode normalization (NFC, NFD, or other) is applied.

NOTE  The `$` and `@` prefixes are _sigils_ (see 7.6), not part of the identifier. In the binding `let $timeout = 30s`, the identifier is `timeout`; the `$` marks it as immutable.

EXAMPLE  Valid identifiers: `x`, `_count`, `Point`, `my_function`, `x2`, `HTTP_STATUS`.

EXAMPLE  The following are not identifiers: `2fast` (starts with digit), `my-var` (contains hyphen), `let` (reserved keyword).

### 7.2.1 Predeclared identifiers

Certain identifiers are _predeclared_ in the universe scope (see [11.1](11-blocks-and-scope.md)). These include type names (`int`, `float`, `str`, `byte`, `bool`, `void`, `Never`), built-in functions (`print`, `panic`, `len`, etc.), and predefined types (`Option`, `Result`, `Error`, `Ordering`). See [Annex C](annex-c-built-in-functions.md) for the complete list.

Predeclared identifiers may be shadowed by user declarations in inner scopes.

### 7.2.2 Exported identifiers

An identifier prefixed with `pub` in its declaration is _exported_ and visible to importing modules. See [Clause 18](18-modules.md).

### 7.2.3 Blank identifier

The underscore `_` used alone as a pattern is the _blank identifier_. It matches any value and discards it.

```ori
let _ = compute();     // discard result
match x { _ -> 0 }    // match anything
```

## 7.3 Keywords

> **Grammar:** See [grammar.ebnf](grammar.md) § Keywords for the complete keyword listing.

### 7.3.1 Reserved

Reserved keywords shall not be used as identifiers except in member position (after `.`). The reserved keywords are:

```
as      break    continue  def     div     do      else    extend
extension extern  false     for     if      impl    in      let
loop    match    Never     pub     self    Self    suspend tests
then    trait    true      type    unsafe  use     uses    void
where   while    with      yield
```

**Exception:** In _member position_ (after `.`), any keyword may be used as a field or method name. The `.` prefix provides unambiguous context, so `x.then(y)` is a method call, not part of an `if`/`then` expression. See [grammar.ebnf](grammar.md) § `member_name`.

### 7.3.2 Reserved (future)

The following keywords are reserved for future language features. Their use as identifiers is a compile-time error with an informative message:

```
asm   inline   static   union   view
```

### 7.3.3 Context-sensitive

Context-sensitive keywords are recognized as keywords only in specific syntactic positions. Outside those positions, they are valid identifiers. See [grammar.ebnf](grammar.md) § Keywords for the complete listing and position rules.

### 7.3.4 Built-in names

Built-in names are reserved in call position (when followed by `(`). Outside call position they may be used as variable names. See [Annex C](annex-c-built-in-functions.md) for semantics.

## 7.4 Operators

> **Rules:** See [Annex B](operator-rules.md) for operator semantics.

### 7.4.1 Precedence

Operators are listed from highest to lowest precedence:

| Prec | Operators | Associativity |
|------|-----------|---------------|
| 1 | `.` `[]` `()` `?` `as` `as?` | Left |
| 2 | `**` | Right |
| 3 | `!` `-` `~` (unary) | Right |
| 4 | `*` `/` `%` `div` `@` | Left |
| 5 | `+` `-` | Left |
| 6 | `<<` `>>` | Left |
| 7 | `..` `..=` | Non-associative |
| 8 | `<` `>` `<=` `>=` | Non-associative |
| 9 | `==` `!=` | Non-associative |
| 10 | `&` | Left |
| 11 | `^` | Left |
| 12 | `\|` | Left |
| 13 | `&&` | Left |
| 14 | `\|\|` | Left |
| 15 | `??` | Right |
| 16 | `\|>` | Left |

NOTE  Range (`..`, `..=`), comparison (`<`, `>`, `<=`, `>=`), and equality (`==`, `!=`) operators are non-associative: `a == b == c` is a compile-time error rather than chaining.

### 7.4.2 Short-circuit evaluation

The logical operators `&&` and `||` use _short-circuit evaluation_: the right operand is evaluated only if the left operand does not determine the result.

- `a && b`: if `a` is `false`, the result is `false` and `b` is not evaluated.
- `a || b`: if `a` is `true`, the result is `true` and `b` is not evaluated.

The null coalescing operator `??` also short-circuits: `a ?? b` evaluates `b` only if `a` is `None`.

### 7.4.3 Compound assignment operators

Compound assignment operators combine a binary operation with assignment:

```
+=  -=  *=  /=  %=  **=  @=  &=  |=  ^=  <<=  >>=  &&=  ||=
```

A compound assignment `x op= y` desugars to `x = x op y` at the parser level. The target `x` shall be a mutable binding (not `$`-prefixed). Compound assignments are statements, not expressions.

The operators `&&=` and `||=` preserve short-circuit semantics: `x &&= y` evaluates `y` only if `x` is `true`.

## 7.5 Delimiters

The following characters serve as delimiters:

| Delimiter | Name | Usage |
|-----------|------|-------|
| `(` `)` | Parentheses | Grouping, function parameters, tuples |
| `[` `]` | Brackets | List literals, indexing, fixed-capacity |
| `{` `}` | Braces | Blocks, struct/map literals, interpolation |
| `,` | Comma | Element separator |
| `:` | Colon | Type annotation, named argument, map entry |
| `.` | Dot | Member access, module path |
| `;` | Semicolon | Statement terminator |

## 7.6 Sigils

Sigils are single-character prefixes with specific meanings:

| Sigil | Purpose | Example |
|-------|---------|---------|
| `@` | Function declaration | `@main ()` |
| `$` | Immutable binding | `let $timeout = 30s;` |

The `@` sigil marks a function declaration. It is required before function names at declaration sites and optional at call sites.

The `$` sigil marks a binding as immutable. It appears at the definition site, import site, and all usage sites. See [Clause 13](13-variables.md) for details.

## 7.7 Literals

A _literal_ represents a fixed value of a specific type in source code.

### 7.7.1 Integer literals

An _integer literal_ is a sequence of digits representing a value of type `int` (64-bit signed integer).

```ebnf
int_lit     = decimal_lit | hex_lit | binary_lit .
decimal_lit = "0" | non_zero_digit { [ "_" ] digit } .
non_zero_digit = "1" … "9" .
hex_lit     = "0" ( "x" | "X" ) hex_digit { [ "_" ] hex_digit } .
binary_lit  = "0" ( "b" | "B" ) bin_digit { [ "_" ] bin_digit } .
```

Leading zeros in decimal literals are not permitted. `007` is a compile-time error; write `7` instead. The single digit `0` is valid.

There is no octal literal prefix.

Digit separators (`_`) may appear between digits for readability. The following restrictions apply:

- A separator shall not appear at the beginning of the digit sequence (after the prefix, if any).
- A separator shall not appear at the end of the digit sequence.
- Two adjacent separators shall not appear.

EXAMPLE  Valid integer literals: `42`, `1_000_000`, `0xFF`, `0xFF_FF`, `0b1010_0101`, `0`.

EXAMPLE  The following are not valid: `007` (leading zero), `42_` (trailing separator), `4__2` (adjacent separators), `0x` (no digits after prefix), `0b` (no digits after prefix).

An integer literal shall represent a value in the range 0 to 2^63 − 1 (9 223 372 036 854 775 807). A literal value outside this range is a compile-time error.

NOTE  The minimum `int` value (−2^63) cannot be written as a literal because the positive value 2^63 exceeds the literal range. It is available as the associated constant `int.min`.

Both uppercase and lowercase hex digits are accepted: `0xAB`, `0xab`, and `0xAb` are equivalent.

### 7.7.2 Float literals

A _float literal_ is a decimal representation of a value of type `float` (IEEE 754 binary64).

```ebnf
float_lit       = decimal_digits "." decimal_digits [ exponent ]
                | decimal_digits exponent .
exponent        = ( "e" | "E" ) [ "+" | "-" ] decimal_digits .
decimal_digits  = digit { [ "_" ] digit } .
```

A float literal shall have at least one digit before and after the decimal point. Leading-dot (`.5`) and trailing-dot (`5.`) forms are not permitted.

Digit separators follow the same rules as integer literals: `1_000.000_001` is valid.

A float literal that overflows the representable range of IEEE 754 binary64 is a compile-time error.

NOTE  The special values positive infinity, negative infinity, and NaN are not literals. They are accessed via associated constants: `float.inf`, `float.neg_inf`, `float.nan`. Additional constants are `float.max`, `float.min` (smallest positive normal), and `float.epsilon` (machine epsilon).

EXAMPLE  Valid float literals: `3.14`, `2.5e-8`, `1_000.0`, `1.0e10`, `6.022E23`.

EXAMPLE  The following are not valid: `.5` (no leading digit), `5.` (no trailing digit), `1e` (no exponent digits), `0x1.0p10` (no hex floats).

### 7.7.3 String literals

A _string literal_ represents a value of type `str` (UTF-8 encoded text). String literals are delimited by double quotes (`"`).

```ebnf
string_lit     = '"' { string_char | escape_seq } '"' .
string_char    = unicode_char - '"' - '\' - newline .
```

Regular strings do not support interpolation. Braces `{` and `}` are literal characters in regular strings.

A string literal shall not contain an unescaped newline. Multi-line text shall use `\n` escape sequences or template strings (7.7.4).

#### 7.7.3.1 Escape sequences

The following escape sequences are recognized in string literals, template strings, and character literals:

| Escape | Unicode | Name |
|--------|---------|------|
| `\\` | U+005C | Backslash |
| `\"` | U+0022 | Double quote |
| `\n` | U+000A | Newline (line feed) |
| `\t` | U+0009 | Horizontal tab |
| `\r` | U+000D | Carriage return |
| `\0` | U+0000 | Null |
| `\u{`*H*`}` | U+*HHHH* | Unicode code point |

This table is exhaustive. Any other `\`-prefixed sequence is a compile-time error.

NOTE  Legacy escape sequences (`\a`, `\b`, `\f`, `\v`) are not supported. Use `\u{7}` for bell (U+0007), `\u{8}` for backspace (U+0008), etc.

#### 7.7.3.2 Unicode escape sequences

A _unicode escape sequence_ `\u{`*H*`}` represents a single Unicode code point. The braces contain 1 to 6 hexadecimal digits (case-insensitive).

The value shall be a valid Unicode scalar value: U+0000 through U+D7FF or U+E000 through U+10FFFF. Surrogate code points U+D800 through U+DFFF are not valid and produce a compile-time error.

EXAMPLE  `\u{1F600}` (grinning face), `\u{0041}` ('A'), `\u{7}` (bell).

EXAMPLE  `\u{D800}` is an error (surrogate code point).

### 7.7.4 Template strings

A _template string_ is delimited by backticks (`` ` ``) and supports expression interpolation.

```ebnf
template_lit = '`' { template_char | escape_seq | template_escape | interpolation } '`' .
template_char = unicode_char - '`' - '\' - '{' - '}' - newline .
template_escape = "{{" | "}}" .
interpolation = '{' expression [ ':' format_spec ] '}' .
```

Interpolated expressions are enclosed in `{` `}`. Each interpolated expression shall implement the `Printable` trait (or `Formattable` if a format specifier is present).

Literal braces within template strings are written as `{{` (left brace) and `}}` (right brace). Literal backticks are written as `` \` ``.

The standard escape sequences (see 7.7.3.1) are valid in template strings.

Template strings may span multiple lines. Whitespace, including newlines, is preserved exactly as written.

Nested template strings are valid. An interpolation `{...}` may contain any expression, including another template string.

EXAMPLE
```ori
let name = "World";
`Hello, {name}!`          // "Hello, World!"
`{value:.2}`              // 2 decimal places
`{count:05}`              // zero-pad to 5 digits
`outer {`inner {x}`}`    // nested template strings
```

### 7.7.5 Character literals

A _character literal_ represents a single Unicode scalar value of type `char`.

```ebnf
char_lit  = "'" ( char_char | char_escape ) "'" .
char_char = unicode_char - "'" - '\' - newline .
char_escape = escape_seq | "\'" .
```

A character literal shall contain exactly one character or escape sequence. The following are compile-time errors:

- Empty character literal: `''`
- Multi-character literal: `'ab'`
- Surrogate code point (via unicode escape): `'\u{D800}'`

The escape sequence `\'` (single quote) is valid in character literals but not in string or template literals.

EXAMPLE  Valid character literals: `'a'`, `'\n'`, `'\u{1F600}'`, `'\0'`, `'\''`.

### 7.7.6 Boolean literals

The boolean literals are `true` and `false`. They have type `bool`.

### 7.7.7 Duration literals

A _duration literal_ represents a time span of type `Duration`. The internal representation is a 64-bit signed integer count of nanoseconds.

```ebnf
duration_lit    = duration_number duration_suffix .
duration_number = decimal_lit | decimal_lit "." digit { digit } .
duration_suffix = "ns" | "us" | "ms" | "s" | "m" | "h" .
```

Digit separators are permitted in the numeric part: `1_000ms` is valid.

The suffixes denote the following units:

| Suffix | Unit | Nanoseconds |
|--------|------|-------------|
| `ns` | Nanoseconds | 1 |
| `us` | Microseconds | 1 000 |
| `ms` | Milliseconds | 1 000 000 |
| `s` | Seconds | 1 000 000 000 |
| `m` | Minutes | 60 000 000 000 |
| `h` | Hours | 3 600 000 000 000 |

Decimal syntax is compile-time sugar using integer arithmetic. The result shall be a whole number of nanoseconds; otherwise it is a compile-time error.

EXAMPLE  `0.5s` = 500 000 000 ns (valid). `1.5ms` = 1 500 000 ns (valid). `1.5ns` is an error (0.5 nanoseconds is not representable).

The maximum duration is limited by the `i64` range of nanoseconds, approximately ±292 years.

### 7.7.8 Size literals

A _size literal_ represents a quantity of data of type `Size`. The internal representation is a 64-bit signed integer count of bytes (non-negative).

```ebnf
size_lit    = size_number size_suffix .
size_number = decimal_lit | decimal_lit "." digit { digit } .
size_suffix = "b" | "kb" | "mb" | "gb" | "tb" .
```

Digit separators are permitted in the numeric part: `10_000kb` is valid.

Size units use SI prefixes (1000-based, not 1024-based):

| Suffix | Unit | Bytes |
|--------|------|-------|
| `b` | Bytes | 1 |
| `kb` | Kilobytes | 1 000 |
| `mb` | Megabytes | 1 000 000 |
| `gb` | Gigabytes | 1 000 000 000 |
| `tb` | Terabytes | 1 000 000 000 000 |

Decimal syntax follows the same rules as duration literals: the result shall be a whole number of bytes.

EXAMPLE  `1.5kb` = 1 500 bytes (valid). `0.5b` is an error (0.5 bytes is not representable).

Negative size literals are compile-time errors. A `Size` value shall be non-negative.

## 7.8 Semicolons

Semicolons (`;`) serve as statement terminators inside block expressions. Outside block expressions, newlines terminate top-level declarations.

### 7.8.1 Block semicolons

Within a block expression `{ ... }`, semicolons govern statement termination and the block's result value:

1. Every statement in a block shall be terminated by `;`.
2. The last element of a block, if not terminated by `;`, is the _result expression_ — its value becomes the value of the block.
3. If every element in a block is terminated by `;`, the block has type `void`.

The following constructs are _statements_ (require `;` when not the last element):

- `let` bindings
- `use` imports
- assignments and compound assignments
- expression statements (an expression evaluated for its side effect)

EXAMPLE
```ori
{
    let $x = 1;    // statement: needs ;
    let $y = 2;    // statement: needs ;
    x + y          // result expression: no ;
}
// Block has type int, value 3
```

EXAMPLE
```ori
{
    print(msg: "hello");
    print(msg: "world");
}
// All elements have ;  →  block has type void
```

### 7.8.2 Declaration termination

A top-level declaration whose body is an expression (not a block) shall be terminated by `;`. A declaration whose body ends with `}` does not require `;`.

EXAMPLE
```ori
@add (a: int, b: int) -> int = a + b;          // expression body: needs ;
@process (x: int) -> int = { let $y = x * 2; y }  // block body: no ;
type Point = { x: int, y: int }                 // ends with }: no ;
let $MAX = 100;                                  // module-level constant: needs ;
```

## 7.9 Trailing commas

Trailing commas are permitted after the last element in all comma-separated lists: parameter lists, argument lists, list literals, map literals, struct fields, variant payloads, match arms, and generic parameter lists.

The formatter inserts trailing commas in multi-line constructs and removes them in single-line constructs.

EXAMPLE
```ori
type Color = {
    r: int,
    g: int,
    b: int,   // trailing comma permitted
}
```

## 7.10 Lexer-parser contract

The lexer produces a _flat stream_ of minimal tokens. The parser combines adjacent tokens based on context.

### 7.10.1 Greater-than sequences

The lexer produces individual `>` tokens. It never produces `>>`, `>=`, or `>>=` as single tokens.

In _expression context_, adjacent tokens form compound operators:
- `>` followed immediately by `>` (no whitespace) → right shift `>>`
- `>` followed immediately by `=` (no whitespace) → greater-or-equal `>=`

In _type context_, `>` closes a generic parameter list.

```ori
// Each > is a separate token — nested generics parse correctly
let x: Result<Result<int, str>, str> = Ok(Ok(1));

// In expressions, >> is right shift
let y = 8 >> 2;
```

### 7.10.2 Token classification

The lexer classifies tokens by their first character:

- Digit → integer or float literal (or duration/size suffix)
- Letter or underscore → identifier or keyword
- `"` → string literal
- `` ` `` → template string literal
- `'` → character literal
- Operator character → operator

Reserved keywords take precedence over identifiers: the sequence `let` is always the keyword `let`, never the identifier `let`.

## 7.11 Disambiguation

### 7.11.1 Struct literals

An uppercase identifier followed by `{` is interpreted as a struct literal in expression context. In `if` condition context, a struct literal is not permitted because the `{` would be ambiguous with a block.

```ori
// Struct literal in expression — valid
let p = Point { x: 1, y: 2 };

// In if condition — error: struct literal not allowed
if Point { x: 1, y: 2 }.valid then ...  // error

// Use parentheses to disambiguate
if (Point { x: 1, y: 2 }).valid then ...  // OK

// In then-branch — valid: no ambiguity
if condition then Point { x: 1, y: 2 } else default
```

NOTE  The empty brace pair `{ }` (with whitespace) is parsed as an empty map literal, not an empty block.

### 7.11.2 Soft keywords

The following identifiers are keywords only when followed by `(` in expression position:

```
cache    catch    for      match    parallel
recurse  spawn   timeout  try      with
```

The identifier `by` is a keyword only when it follows a range expression (`..` or `..=`):

```ori
0..10 by 2         // by is a keyword (range step)
let by = 2;
0..10 by by        // first by is keyword, second is variable
```

Outside these contexts, soft keywords may be used as variable names.

### 7.11.3 Parenthesized expressions

A parenthesized expression `(...)` is interpreted as:

1. Lambda parameters if followed by `->` and contents match parameter syntax
2. Tuple if it contains a comma: `(a, b)`
3. Unit if empty: `()`
4. Grouped expression otherwise

NOTE  A single-element tuple is not supported. `(a,)` is parsed as a parenthesized expression (the trailing comma is ignored), not a single-element tuple.

```ori
(x) -> x + 1          // lambda with one parameter
(x, y) -> x + y       // lambda with two parameters
(a, b)                 // tuple
()                     // unit
(a + b) * c           // grouped expression
```
