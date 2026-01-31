---
title: "Appendix C: Error Codes"
description: "Ori Compiler Design — Appendix C: Error Codes"
order: 1003
section: "Appendices"
---

# Appendix C: Error Codes

Complete reference of Ori compiler error codes.

## Getting Detailed Help

Use `ori --explain <code>` to get detailed documentation for an error code:

```bash
$ ori --explain E2001
# E2001: Type Mismatch

An expression has a different type than expected in the given context.
...
```

## Error Code Reference Table

| Code | Name | Description | Docs |
|------|------|-------------|------|
| **Lexer (E0xxx)** |
| E0001 | Unterminated String | String literal not closed | ✓ |
| E0002 | Invalid Character | Invalid character in source | ✓ |
| E0003 | Invalid Number | Malformed number literal | ✓ |
| E0004 | Unterminated Char | Character literal not closed | ✓ |
| E0005 | Invalid Escape | Unknown escape sequence | ✓ |
| **Parser (E1xxx)** |
| E1001 | Unexpected Token | Parser found unexpected token | ✓ |
| E1002 | Expected Expression | Expression expected but not found | ✓ |
| E1003 | Unclosed Delimiter | Missing ), ], or } | ✓ |
| E1004 | Expected Identifier | Identifier expected | ✓ |
| E1005 | Expected Type | Type annotation expected | ✓ |
| E1006 | Invalid Function | Invalid function definition | ✓ |
| E1007 | Missing Function Body | Function has no body | ✓ |
| E1008 | Invalid Pattern Syntax | Malformed pattern | ✓ |
| E1009 | Missing Pattern Arg | Required pattern argument missing | ✓ |
| E1010 | Unknown Pattern Arg | Unrecognized pattern argument | ✓ |
| E1011 | Named Args Required | Multi-arg call needs named args | ✓ |
| E1012 | Invalid function_seq | function_seq syntax error | ✓ |
| E1013 | function_exp Named | function_exp needs named properties | ✓ |
| E1014 | Reserved Name | Reserved built-in function name | ✓ |
| **Type Checker (E2xxx)** |
| E2001 | Type Mismatch | Types don't match | ✓ |
| E2002 | Unknown Type | Type not defined | ✓ |
| E2003 | Unknown Identifier | Name not in scope | ✓ |
| E2004 | Arg Count Mismatch | Wrong number of arguments | ✓ |
| E2005 | Cannot Infer | Type inference failed | ✓ |
| E2006 | Duplicate Definition | Name defined twice | ✓ |
| E2007 | Closure Self-Ref | Closure captures itself | ✓ |
| E2008 | Cyclic Type | Type definition is cyclic | ✓ |
| E2009 | Missing Trait Bound | Required trait not implemented | ✓ |
| E2010 | Coherence Violation | Conflicting implementations | ✓ |
| E2011 | Named Args Required | Named arguments required | ✓ |
| E2012 | Unknown Capability | Capability not defined | ✓ |
| E2013 | Provider Mismatch | Provider doesn't implement capability | ✓ |
| E2014 | Missing Capability | Capability used but not declared | ✓ |
| E2015 | Type Param Order | Non-default type param after default | |
| E2016 | Missing Type Arg | Missing type argument (no default) | |
| E2017 | Too Many Type Args | Too many type arguments provided | |
| E2018 | Missing Assoc Type | Impl missing required associated type | ✓ |
| **Patterns (E3xxx)** |
| E3001 | Unknown Pattern | Pattern name not recognized | ✓ |
| E3002 | Invalid Pattern Args | Pattern arguments invalid | ✓ |
| E3003 | Pattern Type Error | Pattern type mismatch | ✓ |
| **Internal (E9xxx)** |
| E9001 | Internal Error | Compiler bug | ✓ |
| E9002 | Too Many Errors | Error limit reached | ✓ |

**✓** = Detailed documentation available via `ori --explain`

## Error Code Ranges

| Range | Category | Description |
|-------|----------|-------------|
| E0xxx | Lexer | Tokenization errors |
| E1xxx | Parser | Syntax errors |
| E2xxx | Type Checker | Type errors |
| E3xxx | Patterns | Pattern errors |
| E4xxx | Evaluator | Runtime errors |
| E5xxx | Imports | Module errors |
| E9xxx | Internal | Compiler bugs |

## Lexer Errors (E0xxx)

### E0001: Invalid Character

```
error[E0001]: invalid character '@#' in source
 --> src/mainsi:5:10
  |
5 |     let x@# = 1
  |          ^^ invalid character
```

### E0002: Unterminated String

```
error[E0002]: unterminated string literal
 --> src/mainsi:3:10
  |
3 |     let s = "hello
  |             ^ string literal never closed
```

### E0003: Invalid Escape

```
error[E0003]: invalid escape sequence '\q'
 --> src/mainsi:2:15
  |
2 |     let s = "\q"
  |               ^^ unknown escape
  |
  = help: valid escapes are: \\, \", \n, \t, \r
```

### E0004: Invalid Number

```
error[E0004]: invalid number literal '12.34.56'
 --> src/mainsi:1:9
  |
1 |     let x = 12.34.56
  |             ^^^^^^^^ invalid number
```

## Parser Errors (E1xxx)

### E1001: Unexpected Token

```
error[E1001]: unexpected token
 --> src/mainsi:5:10
  |
5 |     let x + = 1
  |           ^ expected '=' or ':', found '+'
```

### E1002: Expected Expression

```
error[E1002]: expected expression
 --> src/mainsi:3:15
  |
3 |     let x = if then 1
  |                ^^^^ expected expression after 'if'
```

### E1003: Missing Closing Delimiter

```
error[E1003]: missing closing delimiter
 --> src/mainsi:2:10
  |
2 |     let list = [1, 2, 3
  |                ^ opening '[' here
  |
5 | }
  | ^ expected ']' before this
```

### E1004: Invalid Pattern

```
error[E1004]: invalid pattern in match arm
 --> src/mainsi:4:5
  |
4 |     1 + 2 -> "sum"
  |     ^^^^^ expected pattern, found expression
```

## Type Errors (E2xxx)

### E2001: Type Mismatch

```
error[E2001]: type mismatch
 --> src/mainsi:3:15
  |
3 |     let x: int = "hello"
  |            ---   ^^^^^^^ expected `int`, found `str`
  |            |
  |            expected due to this annotation
```

### E2002: Undefined Variable

```
error[E2002]: cannot find value `foo` in this scope
 --> src/mainsi:5:10
  |
5 |     let x = foo + 1
  |             ^^^ not found in this scope
  |
  = help: did you mean `for`?
```

### E2003: Missing Capability

```
error[E2003]: missing capability `Http`
 --> src/mainsi:8:5
  |
8 |     http_get(url)
  |     ^^^^^^^^^^^^^ requires `uses Http`
  |
  = help: add `uses Http` to function signature
```

### E2004: Infinite Type

```
error[E2004]: infinite type detected
 --> src/mainsi:3:5
  |
3 |     let xs = [xs]
  |         ^^ type `T` would be `[T]`
```

### E2005: Not Callable

```
error[E2005]: cannot call value of type `int`
 --> src/mainsi:4:5
  |
4 |     x(1, 2)
  |     ^ not a function
```

### E2006: Wrong Argument Count

```
error[E2006]: function takes 2 arguments but 3 were supplied
 --> src/mainsi:5:5
  |
2 | @add (a: int, b: int) -> int = a + b
  |       ---------------- defined here
  |
5 |     add(1, 2, 3)
  |         ^^^^^^^ expected 2 arguments
```

## Pattern Errors (E3xxx)

### E3001: Unknown Pattern

```
error[E3001]: unknown pattern `mapp`
 --> src/mainsi:3:5
  |
3 |     mapp(over: items, transform: fn)
  |     ^^^^ not a known pattern
  |
  = help: did you mean `map`?
```

### E3002: Missing Required Argument

```
error[E3002]: missing required argument `.transform` for pattern `map`
 --> src/mainsi:4:5
  |
4 |     map(over: items)
  |     ^^^^^^^^^^^^^^^^^ missing `.transform`
```

### E3003: Unexpected Argument

```
error[E3003]: unexpected argument `.foo` for pattern `map`
 --> src/mainsi:5:20
  |
5 |     map(over: items, foo: 1)
  |                       ^^^^ not a valid argument
  |
  = help: valid arguments are: .over, .transform
```

## Runtime Errors (E4xxx)

### E4001: Division by Zero

```
error[E4001]: division by zero
 --> src/mainsi:3:10
  |
3 |     let x = 10 / 0
  |             ^^^^^^ attempted to divide by zero
```

### E4002: Index Out of Bounds

```
error[E4002]: index out of bounds
 --> src/mainsi:4:10
  |
4 |     list[10]
  |          ^^ index 10 is out of range for list of length 3
```

### E4003: Assertion Failed

```
error[E4003]: assertion failed
 --> src/mainsi:5:5
  |
5 |     assert(cond: x > 10)
  |     ^^^^^^^^^^^^^^^^^^^^^ condition was false
```

## Import Errors (E5xxx)

### E5001: Module Not Found

```
error[E5001]: cannot find module './utils'
 --> src/mainsi:1:5
  |
1 | use './utils' { helper }
  |     ^^^^^^^^^^ file not found
  |
  = help: looked for: src/utils.ori
```

### E5002: Item Not Exported

```
error[E5002]: `helper` is not exported from module './utils'
 --> src/mainsi:1:15
  |
1 | use './utils' { helper }
  |                 ^^^^^^ not public
  |
  = help: add `pub` to make it public, or use `::helper` for private access
```

### E5003: Circular Import

```
error[E5003]: circular import detected
 --> src/asi:1:1
  |
1 | use './b' { foo }
  | ^^^^^^^^^^^^^^^^^
  |
  = note: import cycle: a.ori -> b.ori -> a.ori
```

## Internal Errors (E9xxx)

### E9001: Internal Compiler Error

```
error[E9001]: internal compiler error
  |
  = note: this is a bug in the compiler
  = note: please report at https://github.com/ori-lang/ori/issues
  = note: message: unexpected None in type_of_expr
```
