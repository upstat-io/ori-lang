---
title: "Constants"
description: "Clause 12: Ori Language Specification — Constants"
order: 12
section: "Language"
---

# 12 Constants

Constants are immutable bindings declared with the `$` prefix.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § DECLARATIONS (constant_decl), CONSTANT EXPRESSIONS

## 12.1 Immutable Bindings

A binding prefixed with `$` is immutable — it cannot be reassigned after initialization.

```ori
let $timeout = 30s;
let $api_base = "https://api.example.com";
let $max_retries = 3;
pub let $default_limit = 100;
```

The `$` prefix appears at definition, import, and usage sites:

```ori
// Definition
let $timeout = 30s;

// Usage
retry(op: fetch(url), attempts: $max_retries, timeout: $timeout);
```

## 12.2 Module-Level Constants

All module-level bindings shall be immutable. Mutable state is not permitted at module scope.

```ori
let $timeout = 30s;      // OK: immutable
pub let $api_base = "https://...";  // OK: public, immutable

let counter = 0;         // error: module-level bindings must be immutable
```

Module-level constants may be initialized with any expression:

```ori
let $a = 5;                    // literal
let $b = $a * 2;               // constant expression
let $c = $square(x: 10);       // const function call
```

The compiler evaluates constant expressions at compile time when possible. Expressions that cannot be evaluated at compile time produce runtime immutable bindings.

## 12.3 Local Immutable Bindings

The `$` prefix may be used in local scope to create immutable local bindings:

```ori
@process (input: int) -> int = {
    let $base = expensive_calculation(input);
    // ... $base cannot be reassigned ...
    $base * 2
}
```

## 12.4 Identifier Rules

The `$` prefix is a modifier on the identifier, not part of the name. A binding for `$x` and a binding for `x` refer to the same name — they cannot coexist in the same scope.

```ori
let x = 5;
let $x = 10;  // error: 'x' is already defined in this scope
```

The `$` shall match between definition and usage:

```ori
let $timeout = 30s;
$timeout       // OK
timeout        // error: undefined variable 'timeout'
```

## 12.5 Const Functions

A const function is a pure function bound to an immutable name. Const functions may be evaluated at compile time when all arguments are constant.

```ori
let $square = (x: int) -> int = x * x;
let $factorial = (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1);

// Evaluated at compile time
let $fact_10 = $factorial(n: 10);  // 3628800
```

Const functions shall be pure:
- No capabilities (`uses` clause)
- No side effects
- No mutable state access

If called with non-constant arguments, the call is evaluated at runtime.

## 12.6 Constant Expressions

Literals are constant. Arithmetic, comparison, logical, and string concatenation operations are constant if all operands are constant.

```ori
42                          // constant
1 + 2 * 3                   // constant
"hello" + " world"          // constant
true && false               // constant
```

Non-constant expressions include:
- Non-pure function calls
- Mutable variable references
- Expressions using capabilities

## 12.7 Imports

When importing immutable bindings, the `$` shall be included:

```ori
// config.ori
pub let $timeout = 30s;

// client.ori
use "./config" { $timeout };  // OK
use "./config" { timeout };   // error: 'timeout' not found
```

## 12.8 Constraints

- Module-level bindings shall use `$` prefix (immutable required)
- `$`-prefixed bindings cannot be reassigned
- `$` and non-`$` bindings with the same name cannot coexist in the same scope
