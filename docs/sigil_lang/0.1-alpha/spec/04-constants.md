# Constants

Constant expressions are evaluated at compile time.

## Constant Expressions

```
const_expr = literal
           | const_expr binary_op const_expr
           | unary_op const_expr
           | "(" const_expr ")" .
```

Literals are constant. Arithmetic, comparison, logical, and string concatenation operations are constant if all operands are constant.

```sigil
42                          // constant
1 + 2 * 3                   // constant
"hello" + " world"          // constant
true && false               // constant
```

Non-constant expressions:
- Function calls (except compile-time built-ins)
- Variable references
- Expressions using capabilities

## Config Variables

```
config_decl = [ "pub" ] "$" identifier "=" literal .
```

Config variables are module-level compile-time constants declared with `$` prefix.

```sigil
$max_retries = 3
$timeout = 30s
$api_base = "https://api.example.com"
pub $default_limit = 100
```

- Must be initialized with a literal value.
- Cannot be reassigned.
- Type is inferred from the literal.
- Private by default; `pub` makes them visible to other modules.

### Usage

Reference with `$` prefix:

```sigil
retry(op: fetch(url), attempts: $max_retries, timeout: $timeout)
```

### Constraints

```sigil
// Invalid
$computed = 1 + f()   // error: must be literal
$x = 10
$x = 20               // error: cannot reassign
```
