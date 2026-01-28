---
title: "Lexical Elements"
description: "Ori Language Specification — Lexical Elements"
order: 3
---

# Lexical Elements

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § LEXICAL GRAMMAR

A _token_ is an identifier, keyword, literal, operator, or delimiter.

## Comments

Comments start with `//` and extend to end of line. Inline comments are not permitted.

```ori
// Valid comment
@add (a: int, b: int) -> int = a + b

@sub (a: int, b: int) -> int = a - b  // error: inline comment
```

### Doc Comments

Doc comments use special markers:

| Marker | Purpose |
|--------|---------|
| `#` | Description |
| `@param` | Parameter |
| `@field` | Field |
| `!` | Warning |
| `>` | Example |

## Identifiers

Identifiers are case-sensitive. Must not start with digit or be a reserved keyword.

## Keywords

### Reserved

```
async    break    continue  do       else     false
for      if       impl      in       let      loop
match    pub      self      Self     then
trait    true     type      use      uses     void
where    with     yield
```

### Context-Sensitive

Keywords only in pattern expressions:

```
cache    collect  filter    find     fold
map      parallel recurse   retry    run
timeout  try      validate
```

### Built-in Names

Reserved in call position (`name(`), usable as variables otherwise:

```
int      float    str       byte     len
is_empty is_some  is_none   is_ok    is_err
assert   assert_eq assert_ne compare  min
max      print    panic
```

## Operators

### Precedence

| Prec | Operators | Assoc |
|------|-----------|-------|
| 1 | `.` `[]` `()` `?` | Left |
| 2 | `!` `-` `~` (unary) | Right |
| 3 | `*` `/` `%` `div` | Left |
| 4 | `+` `-` | Left |
| 5 | `<<` `>>` | Left |
| 6 | `..` `..=` | Left |
| 7 | `<` `>` `<=` `>=` | Left |
| 8 | `==` `!=` | Left |
| 9 | `&` | Left |
| 10 | `^` | Left |
| 11 | `\|` | Left |
| 12 | `&&` | Left |
| 13 | `\|\|` | Left |
| 14 | `??` | Left |

## Delimiters

Delimiters include parentheses, brackets, braces, comma, colon, dot, and sigils (`@`, `$`).

## Sigils

Sigils are single-character prefixes with specific meanings:

| Sigil | Purpose | Example |
|-------|---------|---------|
| `@` | Function declaration | `@main ()` |
| `$` | Immutable binding | `let $timeout = 30s` |

The `$` sigil marks a binding as immutable. It appears at definition, import, and usage sites. See [Variables](05-variables.md) for details.

## Literals

### Integer

```ori
42
1_000_000
0xFF
```

### Float

```ori
3.14
2.5e-8
```

### String

```ori
"hello"
"line1\nline2"
```

### Character

```ori
'a'
'\n'
```

### Boolean

Boolean literals are `true` and `false`.

### Duration

```ori
100ms
30s
```

### Size

```ori
4kb
10mb
```

## Semicolons

Not required. Newlines terminate statements. Commas separate elements within delimiters.

## Trailing Commas

Permitted in all comma-separated lists. Required by formatter in multi-line constructs.

## Lexer-Parser Contract

The lexer produces _minimal tokens_. The parser combines adjacent tokens based on context.

### Greater-Than Sequences

The lexer produces individual `>` tokens. It never produces `>>`, `>=`, or `>>=` as single tokens.

In _expression context_, adjacent tokens form compound operators:
- `>` followed immediately by `>` (no whitespace) → right shift `>>`
- `>` followed immediately by `=` (no whitespace) → greater-equal `>=`

In _type context_, `>` closes a generic parameter list.

```ori
// Parses correctly: each > is a separate token
let x: Result<Result<int, str>, str> = Ok(Ok(1))

// In expressions, >> is right shift
let y = 8 >> 2  // y = 2
```

This enables nested generic types while preserving shift operators in expressions.

## Disambiguation

### Struct Literals

An uppercase identifier followed by `{` is interpreted as:
- A struct literal in expression context
- NOT a struct literal in `if` condition context

```ori
// Struct literal in expression
let p = Point { x: 1, y: 2 }

// In if condition, struct literal not allowed
// (the { would start a block in languages without `then`)
if condition then Point { x: 1, y: 2 } else default  // OK: in then branch

// Error: struct literal in condition
if Point { x: 1, y: 2 }.valid then ...  // must use parentheses
if (Point { x: 1, y: 2 }).valid then ...  // OK
```

### Soft Keywords

The following identifiers are keywords only when followed by `(` in expression position:

```
cache    catch    for      match    parallel
recurse  run      spawn    timeout  try
with
```

Outside this context, they may be used as variable names.

### Parenthesized Expressions

A parenthesized expression `(...)` is interpreted as:

1. Lambda parameters if followed by `->` and contents match parameter syntax
2. Tuple if it contains a comma: `(a, b)`
3. Unit if empty: `()`
4. Grouped expression otherwise

```ori
(x) -> x + 1          // lambda with one parameter
(x, y) -> x + y       // lambda with two parameters
(a, b)                // tuple
()                    // unit
(a + b) * c           // grouped expression
```
