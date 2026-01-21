# Constants

This section defines constant expressions and compile-time values.

## Constant Expressions

A constant expression is an expression whose value can be determined at compile time.

### Literal Constants

All literals are constant expressions:

```sigil
42          // int constant
3.14        // float constant
"hello"     // str constant
true        // bool constant
30s         // Duration constant
4kb         // Size constant
```

### Constant Evaluation

The following expressions are constant if their operands are constant:

1. Arithmetic operations on numeric constants
2. Comparison operations on constants
3. Logical operations on boolean constants
4. String concatenation of string constants

```sigil
// Constant expressions
1 + 2           // 3
10 * 5          // 50
"hello" + " " + "world"  // "hello world"
true && false   // false
```

### Non-Constant Expressions

The following are never constant:

1. Function calls (except built-in compile-time functions)
2. Variable references
3. Operations with side effects
4. Expressions involving capabilities

## Config Variables

Config variables are module-level constant bindings declared with the `$` prefix.

### Syntax

```
config        = [ "pub" ] "$" identifier "=" literal .
```

### Semantics

A config variable:

1. Must be initialized with a literal value
2. Is immutable (cannot be reassigned)
3. Is evaluated at compile time
4. Has module scope

```sigil
$max_retries = 3
$timeout = 30s
$api_base = "https://api.example.com"
$debug_mode = false
```

### Type Inference

The type of a config variable is inferred from its literal:

| Literal | Inferred Type |
|---------|---------------|
| Integer literal | `int` |
| Float literal | `float` |
| String literal | `str` |
| Boolean literal | `bool` |
| Duration literal | `Duration` |
| Size literal | `Size` |

### Visibility

Config variables are private by default. The `pub` modifier makes them visible to other modules:

```sigil
pub $default_timeout = 30s    // visible to other modules
$internal_limit = 100         // private to this module
```

### Usage

Config variables are referenced using the `$` prefix:

```sigil
@fetch (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: $max_retries,
    .timeout: $timeout,
)
```

### Constraints

It is an error if:

1. A config variable is declared without initialization
2. A config variable is initialized with a non-literal expression
3. A config variable is reassigned
4. The `$` prefix is used in a non-config context

```sigil
// ERROR: config must be initialized with a literal
$computed = 1 + some_function()

// ERROR: config cannot be reassigned
$value = 10
$value = 20  // error
```
