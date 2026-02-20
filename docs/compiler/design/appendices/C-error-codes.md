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
| E0006 | Unterminated Template | Template literal not closed | ✓ |
| E0007 | Semicolon | Cross-language habit: semicolons | ✓ |
| E0008 | Triple-Equals | Cross-language habit: `===` | ✓ |
| E0009 | Single-Quote String | Cross-language habit: single-quote strings | ✓ |
| E0010 | Increment/Decrement | Cross-language habit: `++`/`--` | ✓ |
| E0011 | Unicode Confusable | Unicode confusable character | ✓ |
| E0012 | Detached Doc Comment | Doc comment not attached to declaration | |
| E0013 | Standalone Backslash | Standalone backslash in source | |
| E0014 | Decimal Not Representable | Decimal duration/size not representable as whole base units | |
| E0015 | Reserved-Future Keyword | Reserved-future keyword used as identifier | |
| E0911 | Float Duration/Size | Floating-point duration/size literal not supported | |
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
| E1012 | Invalid Block Expression | Block expression syntax error | ✓ |
| E1013 | Named Properties Required | Control flow expression needs named properties | ✓ |
| E1014 | Reserved Name | Reserved built-in function name | ✓ |
| E1015 | Unsupported Keyword | Unsupported keyword (e.g., `return` is not valid in Ori) | ✓ |
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
| **ARC Analysis (E4xxx)** |
| E4001 | Unsupported ARC Expr | Unsupported expression in ARC IR lowering | |
| E4002 | Unsupported ARC Pattern | Unsupported pattern in ARC IR lowering | |
| E4003 | ARC Internal Error | ARC internal error (invariant violation) | |
| **Codegen / LLVM (E5xxx)** |
| E5001 | LLVM Verification | LLVM module verification failed (ICE) | |
| E5002 | Optimization Failed | Optimization pipeline failed | |
| E5003 | Emission Failed | Object/assembly/bitcode emission failed | |
| E5004 | Target Not Supported | Target not supported / target configuration failed | |
| E5005 | Runtime Not Found | Runtime library (`libori_rt.a`) not found | |
| E5006 | Linker Failed | Linker failed | |
| E5007 | Debug Info Failed | Debug info creation failed | |
| E5008 | WASM Error | WASM-specific error | |
| E5009 | Module Target Error | Module target configuration failed | |
| **Runtime / Eval (E6xxx)** |
| E6001 | Division By Zero | Division by zero | |
| E6002 | Modulo By Zero | Modulo by zero | |
| E6003 | Integer Overflow | Integer overflow | |
| E6010 | Runtime Type Mismatch | Type mismatch at runtime | |
| E6011 | Invalid Binary Op | Invalid binary operator for type | |
| E6012 | Binary Type Mismatch | Binary operand type mismatch | |
| E6020 | Undefined Variable | Undefined variable | |
| E6021 | Undefined Function | Undefined function | |
| E6022 | Undefined Constant | Undefined constant | |
| E6023 | Undefined Field | Undefined field | |
| E6024 | Undefined Method | Undefined method | |
| E6025 | Index Out Of Bounds | Index out of bounds | |
| E6026 | Key Not Found | Key not found | |
| E6027 | Immutable Binding | Immutable binding | |
| E6030 | Arity Mismatch | Arity mismatch | |
| E6031 | Stack Overflow | Stack overflow (recursion limit) | |
| E6032 | Not Callable | Value is not callable | |
| E6040 | Non-Exhaustive Match | Non-exhaustive match | |
| E6050 | Assertion Failed | Assertion failed | |
| E6051 | Panic Called | Panic called | |
| E6060 | Missing Capability | Missing capability at runtime | |
| E6070 | Const-Eval Budget | Const-eval budget exceeded | |
| E6080 | Not Implemented | Not implemented feature | |
| E6099 | Custom Runtime Error | Custom runtime error | |
| **Internal (E9xxx)** |
| E9001 | Internal Error | Compiler bug | ✓ |
| E9002 | Too Many Errors | Error limit reached | ✓ |
| **Warnings (W1xxx)** |
| W1001 | Detached Doc Comment | Parser warning: detached doc comment | |

**✓** = Detailed documentation available via `ori --explain`

## Error Code Ranges

| Range | Category | Description |
|-------|----------|-------------|
| E0xxx | Lexer | Tokenization errors |
| E1xxx | Parser | Syntax errors |
| E2xxx | Type Checker | Type errors |
| E3xxx | Patterns | Pattern errors |
| E4xxx | ARC Analysis | ARC IR lowering errors |
| E5xxx | Codegen / LLVM | Code generation and linking errors |
| E6xxx | Runtime / Eval | Evaluator runtime errors |
| E9xxx | Internal | Compiler bugs |
| W1xxx | Warnings | Non-fatal diagnostics |

**Note:** Import errors are reported as type errors (E2xxx) since they are caught during type checking. Runtime errors now have dedicated E6xxx codes with structured `EvalErrorKind` variants.

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

## ARC Analysis Errors (E4xxx)

### E4001: Unsupported ARC Expression

```
error[E4001]: unsupported expression in ARC IR lowering
 --> src/mainsi:5:10
  |
5 |     some_unsupported_expr
  |     ^^^^^^^^^^^^^^^^^^^^^ cannot be lowered to ARC IR
```

### E4002: Unsupported ARC Pattern

```
error[E4002]: unsupported pattern in ARC IR lowering
 --> src/mainsi:3:5
  |
3 |     complex_pattern -> ...
  |     ^^^^^^^^^^^^^^^ cannot be lowered to ARC IR
```

### E4003: ARC Internal Error

```
error[E4003]: ARC internal error (invariant violation)
  |
  = note: this is a bug in the ARC analysis pass
```

## Codegen / LLVM Errors (E5xxx)

### E5001: LLVM Module Verification Failed

```
error[E5001]: LLVM module verification failed
  |
  = note: this is an internal compiler error
```

### E5005: Runtime Library Not Found

```
error[E5005]: runtime library `libori_rt.a` not found
  |
  = help: build the runtime with `cargo bl` or `cargo blr`
```

### E5006: Linker Failed

```
error[E5006]: linker failed
  |
  = note: cc returned exit code 1
```

## Runtime / Eval Errors (E6xxx)

Runtime errors have dedicated error codes organized by category:

### Arithmetic (E6001-E6003)

```
error[E6001]: division by zero
error[E6002]: modulo by zero
error[E6003]: integer overflow
```

### Type Errors (E6010-E6012)

```
error[E6010]: type mismatch
error[E6011]: invalid binary operator for type
error[E6012]: binary operand type mismatch
```

### Lookup Errors (E6020-E6027)

```
error[E6020]: undefined variable `foo`
error[E6021]: undefined function `bar`
error[E6023]: undefined field `x`
error[E6024]: undefined method `len`
error[E6025]: index out of bounds: index 10 is out of range for list of length 3
error[E6026]: key not found
error[E6027]: immutable binding
```

### Call Errors (E6030-E6032)

```
error[E6030]: arity mismatch
error[E6031]: stack overflow (recursion limit exceeded)
error[E6032]: not callable
```

### Control Flow (E6040-E6051)

```
error[E6040]: non-exhaustive match
error[E6050]: assertion failed: x > 10
error[E6051]: panic called
```

### Other (E6060-E6099)

```
error[E6060]: missing capability `Http`
error[E6070]: const-eval budget exceeded
error[E6080]: not implemented
error[E6099]: custom runtime error
```

## Import Errors

Import errors are reported as type errors (E2xxx) since they are caught during type checking:

- Module not found → reported via diagnostic system
- Item not exported → E2003 (unknown identifier)
- Circular import → detected during module resolution

## Internal Errors (E9xxx)

### E9001: Internal Compiler Error

```
error[E9001]: internal compiler error
  |
  = note: this is a bug in the compiler
  = note: please report at https://github.com/ori-lang/ori/issues
  = note: message: unexpected None in type_of_expr
```
