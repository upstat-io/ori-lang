---
title: "Constants"
description: "Module-level constants and const functions."
order: 11
part: "Program Structure"
---

# Constants

Constants provide named values that are known at compile time or cannot change during program execution. This guide covers module-level constants and const functions.

## Module-Level Constants

Constants defined at the module level must use `$` (immutable binding):

```ori
let $MAX_CONNECTIONS = 100;
let $DEFAULT_TIMEOUT = 30s;
let $API_BASE_URL = "https://api.example.com";
let $SUPPORTED_FORMATS = ["json", "xml", "csv"];
```

### Why `$` is Required

Module-level bindings are shared across the program. Making them immutable prevents one module from changing a value and breaking another:

```ori
// This would be dangerous if allowed
let shared_counter = 0;       // ERROR: module-level bindings must be immutable
shared_counter = 1;           // Could break other modules

// This is safe
let $shared_value = 42;       // Cannot be changed
```

### Public Constants

Export constants with `pub`:

```ori
pub let $MAX_RETRIES = 3;
pub let $API_VERSION = "v2";

// Private constant (default)
let $INTERNAL_BUFFER_SIZE = 1024;
```

## Referencing Constants

Constants can reference other constants:

```ori
let $BASE_TIMEOUT = 10s;
let $EXTENDED_TIMEOUT = $BASE_TIMEOUT * 3;     // 30s
let $MAX_TIMEOUT = $EXTENDED_TIMEOUT * 2;      // 60s

let $API_VERSION = "v2";
let $API_BASE = "https://api.example.com";
let $API_URL = `{$API_BASE}/{$API_VERSION}`;   // Full URL
```

### Reference Order

Constants must be defined before they're used:

```ori
// OK: $A is defined before $B
let $A = 10;
let $B = $A * 2;

// ERROR: $Y used before definition
let $X = $Y * 2;    // $Y doesn't exist yet
let $Y = 10;
```

## Importing Constants

Import constants with their `$` prefix:

```ori
use "./config" { $MAX_CONNECTIONS, $DEFAULT_TIMEOUT };
use "./api" { $API_URL, $API_VERSION };

@make_request () -> Result<Response, Error> uses Http =
    Http.get(url: $API_URL, timeout: $DEFAULT_TIMEOUT);
```

### Aliasing Constants

```ori
use "./config" { $MAX_CONNECTIONS as $MAX_CONN };

@check_capacity (current: int) -> bool =
    current < $MAX_CONN;
```

## Const Functions

For computed constants, use const functions:

```ori
let $square = (x: int) -> int = x * x;
let $double = (x: int) -> int = x * 2;
```

### Recursive Const Functions

Const functions can be recursive:

```ori
let $factorial = (n: int) -> int =
    if n <= 1 then 1 else n * $factorial(n: n - 1);

let $fibonacci = (n: int) -> int =
    if n <= 1 then n else $fibonacci(n: n - 1) + $fibonacci(n: n - 2);
```

### Using Const Functions

When called with constant arguments, they're evaluated at compile time:

```ori
// Computed at compile time
let $GRID_SIZE = $square(x: 10);        // 100
let $PERMUTATIONS = $factorial(n: 5);   // 120
let $FIB_10 = $fibonacci(n: 10);        // 55
```

When called with runtime arguments, they're evaluated at runtime:

```ori
@compute (n: int) -> int = $square(x: n);  // Evaluated at runtime
```

### Const Function Restrictions

Const functions must be pure — they cannot:

- Use capabilities (`uses Http`, etc.)
- Perform I/O
- Access mutable state
- Call non-const functions

```ori
// OK: pure computation
let $add = (a: int, b: int) -> int = a + b;

// ERROR: uses capability
let $fetch = (url: str) -> str uses Http = Http.get(url: url);

// ERROR: calls non-const function
let $random_add = (a: int) -> int = a + get_random();
```

## Constant Expressions

### Supported Operations

Constants support:

```ori
// Arithmetic
let $SUM = 10 + 20;
let $PRODUCT = 5 * 6;
let $QUOTIENT = 100 / 4;

// String operations
let $GREETING = "Hello, " + "World!";
let $FORMATTED = `Value: {$SUM}`;

// Collections
let $ITEMS = [1, 2, 3];
let $CONFIG = {"key": "value"};

// Const function calls
let $SQUARED = $square(x: 5);
```

### Compile-Time Evaluation

The compiler evaluates constant expressions where possible:

```ori
let $A = 10;
let $B = 20;
let $C = $A + $B;    // Compiler computes: 30

// Used in code as literal 30
@example () -> int = $C;
```

## Configuration Patterns

### Application Configuration

```ori
// config.ori
pub let $ENV = "production";

pub let $DATABASE_URL = match $ENV {
    "production" -> "postgres://prod.db:5432/app"
    "staging" -> "postgres://staging.db:5432/app"
    _ -> "postgres://localhost:5432/app_dev"
};

pub let $LOG_LEVEL = match $ENV {
    "production" -> "warn"
    "staging" -> "info"
    _ -> "debug"
};
```

### Feature Flags

```ori
pub let $FEATURES = {
    "new_ui": true,
    "beta_api": false,
    "analytics": true,
};

@is_enabled (feature: str) -> bool =
    $FEATURES[feature] ?? false;

@test_features tests @is_enabled () -> void = {
    assert(condition: is_enabled(feature: "new_ui"));
    assert(condition: !is_enabled(feature: "beta_api"));
    assert(condition: !is_enabled(feature: "unknown"))
}
```

### Size and Time Constants

```ori
pub let $MAX_FILE_SIZE = 10mb;
pub let $REQUEST_TIMEOUT = 30s;
pub let $RETRY_DELAY = 100ms;
pub let $SESSION_DURATION = 2h;

pub let $BUFFER_SIZE = 4kb;
pub let $CACHE_LIMIT = 100mb;
```

## Complete Example

```ori
// constants.ori - Application constants

// API Configuration
pub let $API_VERSION = "v2";
pub let $API_HOST = "api.example.com";
pub let $API_BASE_URL = `https://{$API_HOST}/{$API_VERSION}`;

// Limits
pub let $MAX_PAGE_SIZE = 100;
pub let $DEFAULT_PAGE_SIZE = 20;
pub let $MAX_RETRIES = 3;

// Timeouts
pub let $CONNECTION_TIMEOUT = 10s;
pub let $REQUEST_TIMEOUT = 30s;
pub let $LONG_POLL_TIMEOUT = 5m;

// Computed constants
let $calculate_backoff = (attempt: int) -> Duration =
    if attempt <= 0 then 100ms
    else 100ms * $power(base: 2, exp: attempt);

let $power = (base: int, exp: int) -> int =
    if exp <= 0 then 1 else base * $power(base: base, exp: exp - 1);

pub let $RETRY_BACKOFFS = [
    $calculate_backoff(attempt: 0),  // 100ms
    $calculate_backoff(attempt: 1),  // 200ms
    $calculate_backoff(attempt: 2),  // 400ms
];

// Validation
pub let $MIN_PASSWORD_LENGTH = 8;
pub let $MAX_USERNAME_LENGTH = 50;
pub let $ALLOWED_EXTENSIONS = ["jpg", "png", "gif", "webp"];

// Feature flags
pub let $ENABLE_CACHING = true;
pub let $ENABLE_COMPRESSION = true;
pub let $ENABLE_RATE_LIMITING = true;
```

```ori
// usage.ori - Using the constants

use "./constants" {
    $API_BASE_URL,
    $REQUEST_TIMEOUT,
    $MAX_RETRIES,
    $RETRY_BACKOFFS,
    $ENABLE_CACHING,
};

@fetch_data (endpoint: str) -> Result<str, Error> uses Http, Cache = {
    let url = `{$API_BASE_URL}/{endpoint}`;

    // Check cache if enabled
    if $ENABLE_CACHING then {
        let cached = Cache.get(key: url);
        if is_some(option: cached) then return Ok(cached.unwrap_or(default: ""))
    };


    // Fetch with retries
    let result = fetch_with_retry(
        url: url
        timeout: $REQUEST_TIMEOUT
        max_retries: $MAX_RETRIES
        backoffs: $RETRY_BACKOFFS
    );

    // Cache successful result
    if is_ok(result: result) && $ENABLE_CACHING then
        match result { Ok(data) -> Cache.set(key: url, value: data, ttl: 5m), Err(_) -> ()};

    result
}

@fetch_with_retry (
    url: str,
    timeout: Duration,
    max_retries: int,
    backoffs: [Duration],
) -> Result<str, Error> uses Http = {
    let attempt = 0;
    loop {
        let result = Http.get(url: url, timeout: timeout);
        if is_ok(result: result) then break result;
        if attempt >= max_retries then break result;
        let delay = backoffs[attempt] ?? 1s;
        sleep(duration: delay);
        attempt = attempt + 1
    }
}

// Placeholder
@sleep (duration: Duration) -> void = ();

@test_fetch tests @fetch_data () -> void =
    with Http = MockHttp { responses: {} },
    Cache = MockCache {} in {
        // Test would go here
        ()
    }
```

## Quick Reference

### Module-Level Constants

```ori
let $NAME = value;           // Private constant
pub let $NAME = value;       // Public constant
```

### Const Functions

```ori
let $fn_name = (param: Type) -> ReturnType = expression;
```

### Importing Constants

```ori
use "./module" { $CONSTANT };
use "./module" { $CONSTANT as $ALIAS };
```

### Common Patterns

```ori
// Computed from other constants
let $DERIVED = $BASE * 2;

// Conditional based on environment
let $VALUE = match $ENV { "prod" -> x, _ -> y};

// Collection constants
let $ITEMS = [a, b, c];
let $CONFIG = {"key": value};
```

## What's Next

Now that you understand constants:

- **[Testing](/guide/12-testing)** — Comprehensive testing strategies
- **[Capabilities](/guide/13-capabilities)** — Explicit effects and testing
