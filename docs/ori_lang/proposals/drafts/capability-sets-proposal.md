# Proposal: Capability Sets

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-02-03
**Affects:** Compiler, capability system, grammar

---

## Summary

This proposal introduces **capability sets** — named, reusable collections of capability bindings that can be defined once and applied with `with`. This eliminates repetitive capability wiring while maintaining Ori's explicit, side-effect-free execution model.

---

## Problem Statement

Currently, providing capabilities requires listing each binding at every use site:

```ori
@main () -> void =
    with Http = ProdHttpClient,
         Cache = RedisCache { host: "localhost" },
         Logger = StdoutLogger
    in
        run_app()

@test_app tests @run_app () -> void =
    with Http = MockHttp { responses: {} },
         Cache = InMemoryCache {},
         Logger = NoOpLogger
    in
        run_app()

@test_fetch tests @fetch () -> void =
    with Http = MockHttp { responses: {} },
         Cache = InMemoryCache {},
         Logger = NoOpLogger  // Same three lines repeated
    in
        fetch(url: "...")
```

Problems:
1. **Repetition**: Same capability configurations repeated across tests and entry points
2. **Maintenance burden**: Changing a configuration requires updating multiple locations
3. **No composition**: Cannot build configurations from shared base configurations
4. **Verbosity**: Simple main functions become cluttered with wiring

---

## Proposed Solution

Introduce capability sets as compile-time constant values using `let $name = with { ... }`:

```ori
let $production = with {
    Http = ProdHttpClient,
    Cache = RedisCache { host: env("REDIS_HOST") },
    Logger = StdoutLogger
}

let $testing = with {
    Http = MockHttp { responses: {} },
    Cache = InMemoryCache {},
    Logger = NoOpLogger
}

@main () -> void =
    with $production in
        run_app()

@test_app tests @run_app () -> void =
    with $testing in
        run_app()
```

---

## Grammar Changes

### Capability Set Definition

```ebnf
capability_set_def = "let" "$" identifier "=" "with" "{" capability_binding_list "}" .
capability_binding_list = capability_binding { "," capability_binding } [ "," ] .
capability_binding = identifier "=" expression
                   | "..." "$" identifier .
```

### Extended with Expression

```ebnf
with_expr = "with" capability_source [ "," override_list ] "in" expression .
capability_source = "$" identifier
                  | capability_binding_list .
override_list = capability_binding { "," capability_binding } .
```

---

## Detailed Semantics

### 1. Capability Set Definition

A capability set is a compile-time constant that bundles capability bindings:

```ori
let $base = with {
    Logger = StdoutLogger
}
```

**Rules:**
- Must use `$` prefix (compile-time constant)
- Bindings are evaluated at definition site
- Can only reference other compile-time constants
- Module-level only (not inside functions)

### 2. Capability Set Composition

Capability sets can extend other sets using spread syntax:

```ori
let $base = with {
    Logger = StdoutLogger
}

let $production = with {
    ...$base,
    Http = ProdHttpClient,
    Cache = RedisCache { host: "localhost" }
}

let $staging = with {
    ...$production,
    Logger = VerboseLogger  // Override from $production (which got it from $base)
}
```

**Spread rules:**
- `...$name` includes all bindings from the referenced set
- Later bindings override earlier ones (including spread)
- Multiple spreads allowed: `{ ...$a, ...$b, X = impl }`
- Spread must reference a capability set (not arbitrary expressions)

### 3. Using Capability Sets

Apply a capability set with `with $name in`:

```ori
@main () -> void =
    with $production in
        run_app()
```

**Equivalent to:**

```ori
@main () -> void =
    with Http = ProdHttpClient,
         Cache = RedisCache { host: "localhost" },
         Logger = StdoutLogger
    in
        run_app()
```

### 4. Inline Overrides

Override specific capabilities while using a set:

```ori
@debug_main () -> void =
    with $production, Logger = DebugLogger in
        run_app()
```

The override applies after the set's bindings, shadowing any matching capability.

**Multiple overrides:**

```ori
with $production, Logger = DebugLogger, Cache = NoOpCache in
    run_app()
```

### 5. Conditional Capability Sets

Use standard Ori conditionals to select sets:

```ori
let $environment = if $release
    then $production
    else $development

@main () -> void =
    with $environment in
        run_app()
```

### 6. Visibility and Export

Capability sets follow standard visibility rules:

```ori
// config.ori
pub let $production = with {
    Http = ProdHttpClient,
    Cache = RedisCache { host: "localhost" },
    Logger = StdoutLogger
}

let $internal = with {  // Private to this module
    ...$production,
    Logger = InternalLogger
}
```

```ori
// main.ori
use "./config" { $production }

@main () -> void =
    with $production in
        run_app()
```

---

## Type System Integration

### Capability Set Type

A capability set has an implicit type representing its bound capabilities:

```ori
let $http_only = with {
    Http = ProdHttpClient
}
// Type: CapabilitySet<Http>

let $full = with {
    Http = ProdHttpClient,
    Cache = RedisCache {},
    Logger = StdoutLogger
}
// Type: CapabilitySet<Http, Cache, Logger>
```

### Compatibility Checking

When using `with $set in expr`, the compiler verifies that `$set` provides all capabilities required by `expr`:

```ori
@needs_all () -> void uses Http, Cache, Logger = ...

with $http_only in
    needs_all()  // ERROR: $http_only lacks Cache, Logger
```

Error:

```
error[E1210]: capability set `$http_only` does not provide required capabilities
  --> src/main.ori:3:5
   |
3  |     needs_all()
   |     ^^^^^^^^^^^ requires `Cache`, `Logger`
   |
   = note: `$http_only` provides: Http
   = help: use a capability set that provides all required capabilities
   = help: or add overrides: `with $http_only, Cache = impl, Logger = impl in`
```

### Unused Capability Warning

If a set provides capabilities not used by the body, emit a warning:

```ori
@needs_http () -> void uses Http = ...

with $full in  // WARNING: Cache, Logger provided but unused
    needs_http()
```

Warning:

```
warning[W1211]: capability set provides unused capabilities
  --> src/main.ori:1:6
   |
1  | with $full in
   |      ^^^^^ provides `Cache`, `Logger` which are not used
   |
   = help: consider using a more specific capability set
```

---

## Composition Patterns

### Layered Configuration

Build configurations in layers:

```ori
// Layer 1: Logging (used by everything)
let $logging = with {
    Logger = StdoutLogger
}

// Layer 2: Add HTTP for network services
let $with_http = with {
    ...$logging,
    Http = ProdHttpClient
}

// Layer 3: Add persistence for stateful services
let $with_persistence = with {
    ...$logging,
    Database = PostgresDb { conn: env("DB_URL") },
    Cache = RedisCache { host: env("REDIS_HOST") }
}

// Layer 4: Full stack
let $full_stack = with {
    ...$with_http,
    ...$with_persistence
}
```

### Environment-Specific Configuration

```ori
let $common = with {
    Logger = StdoutLogger
}

let $production = with {
    ...$common,
    Http = ProdHttpClient { timeout: 30s },
    Cache = RedisCache { host: env("REDIS_HOST") },
    Database = PostgresDb { pool_size: 20 }
}

let $staging = with {
    ...$production,
    Database = PostgresDb { pool_size: 5 },  // Smaller pool
    Logger = VerboseLogger  // More logging
}

let $development = with {
    ...$common,
    Http = ProdHttpClient { timeout: 5s },  // Shorter timeout for dev
    Cache = InMemoryCache {},
    Database = SqliteDb { path: "./dev.db" }
}

let $testing = with {
    ...$common,
    Http = MockHttp { responses: {} },
    Cache = InMemoryCache {},
    Database = InMemoryDb {}
}
```

### Test Fixture Sets

```ori
// test_fixtures.ori
pub let $unit_test = with {
    Http = MockHttp { responses: {} },
    Cache = InMemoryCache {},
    Logger = NoOpLogger
}

pub let $integration_test = with {
    Http = ProdHttpClient,  // Real HTTP
    Cache = InMemoryCache {},  // But in-memory cache
    Logger = TestLogger { capture: true }
}

pub let $e2e_test = with {
    Http = ProdHttpClient,
    Cache = RedisCache { host: "localhost" },
    Logger = TestLogger { capture: true }
}
```

### Per-Request Scoping

```ori
let $base_request = with {
    Http = ProdHttpClient,
    Database = PostgresDb { conn: env("DB_URL") }
}

@handle_request (req: Request) -> Response uses Http, Database, Logger =
    // Add request-specific logger with request ID
    with $base_request, Logger = RequestLogger { id: req.id } in
        route(req)
```

---

## Interaction with Existing Features

### With def impl

Capability sets do not replace `def impl`. They compose:

```ori
// Module provides default Logger
def impl Logger { @info (message: str) -> void = print(msg: message) }

// Capability set provides Http and Cache, but not Logger
let $network = with {
    Http = ProdHttpClient,
    Cache = RedisCache {}
}

@fetch () -> Result uses Http, Cache, Logger =
    // Http, Cache from $network
    // Logger from def impl
    with $network in
        Http.get(url: "...")
```

Resolution order (unchanged from capability-composition-proposal):
1. Innermost `with...in` binding (including from capability set)
2. Outer `with...in` bindings
3. Imported `def impl`
4. Module-local `def impl`
5. Error

### With Partial Provision

Capability sets support partial provision naturally:

```ori
let $http_cache = with {
    Http = ProdHttpClient,
    Cache = RedisCache {}
}

@fetch () -> Result uses Http, Cache, Logger = ...

// Logger must come from def impl or be provided
with $http_cache in
    fetch()  // OK if def impl Logger exists, ERROR otherwise
```

### With Nested with...in

Capability sets and inline bindings can be nested:

```ori
let $outer = with {
    Http = HttpA,
    Logger = LoggerA
}

with $outer in
    with Logger = LoggerB in
        // Http from $outer (HttpA)
        // Logger from inner with (LoggerB)
        operation()
```

---

## Error Messages

### E1210: Missing Required Capabilities

```
error[E1210]: capability set `$http_only` does not provide required capabilities
  --> src/main.ori:5:5
   |
5  |     needs_all()
   |     ^^^^^^^^^^^ requires `Cache`, `Logger`
   |
   = note: `$http_only` provides: Http
   = help: use a capability set that provides all required capabilities
   = help: or add overrides: `with $http_only, Cache = impl, Logger = impl in`
```

### E1212: Invalid Spread Target

```
error[E1212]: cannot spread non-capability-set value
  --> src/config.ori:3:5
   |
3  |     ...$not_a_set,
   |     ^^^^^^^^^^^^^ `$not_a_set` is not a capability set
   |
   = note: spread (`...`) can only be used with capability sets defined via `with { }`
```

### E1213: Duplicate Capability in Set

```
error[E1213]: duplicate capability binding in set
  --> src/config.ori:4:5
   |
3  |     Http = HttpA,
   |     ---- first binding here
4  |     Http = HttpB,
   |     ^^^^^^^^^^^^ duplicate binding for `Http`
   |
   = help: remove one binding, or use spread with override pattern
```

### E1214: Non-Constant in Capability Set

```
error[E1214]: capability set binding must be a compile-time constant
  --> src/config.ori:3:12
   |
3  |     Http = get_http_client(),
   |            ^^^^^^^^^^^^^^^^^^ not a compile-time constant
   |
   = note: capability sets are evaluated at compile time
   = help: use a constant expression or type constructor
```

### W1211: Unused Capabilities Warning

```
warning[W1211]: capability set provides unused capabilities
  --> src/main.ori:1:6
   |
1  | with $full_stack in
   |      ^^^^^^^^^^^ provides `Cache`, `Database` which are not used
   |
   = help: consider using a more specific capability set
   = note: to suppress this warning, use `#allow(unused_capabilities)`
```

---

## Examples

### Complete Application Structure

```ori
// config/capabilities.ori

pub let $logging = with {
    Logger = StdoutLogger
}

pub let $production = with {
    ...$logging,
    Http = ProdHttpClient {
        timeout: 30s,
        retry_count: 3
    },
    Cache = RedisCache {
        host: env("REDIS_HOST"),
        port: 6379,
        ttl: 5m
    },
    Database = PostgresDb {
        conn: env("DATABASE_URL"),
        pool_size: 20
    }
}

pub let $testing = with {
    ...$logging,
    Http = MockHttp { responses: {} },
    Cache = InMemoryCache {},
    Database = InMemoryDb {}
}
```

```ori
// main.ori

use "./config/capabilities" { $production }

@main () -> void =
    with $production in
        run(
            init_app(),
            serve(port: 8080)
        )
```

```ori
// app.ori

@init_app () -> void uses Logger =
    Logger.info(message: "Application starting")

@serve (port: int) -> void uses Http, Cache, Database, Logger =
    run(
        Logger.info(message: `Listening on port {port}`),
        loop(handle_requests())
    )
```

```ori
// tests/app_test.ori

use "../config/capabilities" { $testing }

@test_init tests @init_app () -> void =
    let logger = MockLogger { messages: [] }
    with $testing, Logger = logger in
        run(
            init_app(),
            assert(condition: logger.messages.contains("Application starting"))
        )
```

### Dynamic Configuration Selection

```ori
// config/capabilities.ori

pub let $production = with { ... }
pub let $staging = with { ... }
pub let $development = with { ... }

pub @get_environment () -> CapabilitySet<Http, Cache, Database, Logger> =
    let env_name = env("ORI_ENV")
    if env_name == "production" then $production
    else if env_name == "staging" then $staging
    else $development
```

```ori
// main.ori

use "./config/capabilities" { get_environment }

@main () -> void =
    with get_environment() in
        run_app()
```

---

## Implementation

### Compiler Changes

1. **Lexer**: No changes (uses existing tokens)

2. **Parser**:
   - Add `capability_set_def` production
   - Extend `with_expr` to accept `$identifier`
   - Add spread support in capability binding list

3. **AST**:
   ```rust
   pub enum Item {
       // ... existing variants
       CapabilitySet(CapabilitySetDef),
   }

   pub struct CapabilitySetDef {
       pub name: Name,
       pub bindings: Vec<CapabilitySetBinding>,
       pub span: Span,
   }

   pub enum CapabilitySetBinding {
       Direct { capability: Name, implementation: Expr },
       Spread { source: Name },
   }
   ```

4. **Type Checker**:
   - Validate capability set definitions at module level
   - Track provided capabilities per set
   - Verify spread targets are capability sets
   - Check for duplicate bindings
   - Implement compatibility checking at use sites

5. **Lowering**:
   - Expand `with $set in expr` to equivalent `with A = a, B = b, ... in expr`
   - Apply spreads in definition order
   - Apply overrides after set expansion

### Test Cases

1. Basic capability set definition and use
2. Spread from single parent
3. Spread from multiple parents
4. Override after spread
5. Inline override when using set
6. Nested with expressions with sets
7. Visibility/export of capability sets
8. Error: missing required capabilities
9. Error: spread non-capability-set
10. Error: duplicate binding
11. Error: non-constant binding
12. Warning: unused capabilities
13. Interaction with def impl
14. Conditional capability set selection

---

## Spec Changes Required

### New Section: 14.5 Capability Sets

Add after section 14.4 (Capability Resolution):

```markdown
## 14.5 Capability Sets

A **capability set** is a named, reusable collection of capability bindings.

### Syntax

capability_set_def = "let" "$" identifier "=" "with" "{" capability_binding_list "}" .
capability_binding_list = capability_binding { "," capability_binding } [ "," ] .
capability_binding = identifier "=" expression
                   | "..." "$" identifier .

### Semantics

[Content from this proposal's Detailed Semantics section]
```

### Update grammar.ebnf

Add:

```ebnf
capability_set_def = "let" "$" identifier "=" "with" "{" capability_binding_list "}" .
capability_binding_list = capability_binding { "," capability_binding } [ "," ] .
capability_set_binding = identifier "=" expression
                       | "..." "$" identifier .
```

Modify `with_expr`:

```ebnf
with_expr = "with" capability_source [ "," capability_override_list ] "in" expression .
capability_source = "$" identifier
                  | capability_binding_list .
capability_override_list = capability_binding { "," capability_binding } .
```

---

## Alternatives Considered

### Alternative 1: `capabilities` Keyword

```ori
capabilities Production {
    Http = ProdHttpClient,
    Cache = RedisCache {}
}
```

**Rejected because:**
- Introduces new keyword
- `let $` already exists for compile-time constants
- Capability sets are values, not a new declaration category

### Alternative 2: Type-Level Capability Sets

```ori
type ProductionEnv = capabilities {
    Http: ProdHttpClient,
    Cache: RedisCache
}
```

**Rejected because:**
- Conflates types and values
- Implementation types aren't type parameters
- Awkward interaction with existing type system

### Alternative 3: `env` Keyword

```ori
env Production {
    Http = ProdHttpClient
}

env Staging: Production {  // Inheritance syntax
    Logger = VerboseLogger
}
```

**Rejected because:**
- New keyword when spread provides same functionality
- Inheritance syntax is less flexible than spread composition
- "env" suggests runtime environment, not compile-time configuration

---

## Future Considerations

### Parameterized Capability Sets

Could allow capability sets with parameters:

```ori
let $with_redis (host: str, port: int) = with {
    Cache = RedisCache { host: host, port: port }
}

with $with_redis(host: "localhost", port: 6379) in
    operation()
```

**Deferred** — adds complexity, unclear if needed.

### Capability Set Constraints

Could allow constraining what capabilities a set must provide:

```ori
let $web_service: CapabilitySet<Http, Cache, Logger> = with {
    // Must provide Http, Cache, Logger
}
```

**Deferred** — the type system already catches missing capabilities at use sites.

---

## Summary

| Aspect | Specification |
|--------|---------------|
| Syntax | `let $name = with { Cap = impl, ... }` |
| Composition | `...$other` spread syntax |
| Usage | `with $set in expr` |
| Overrides | `with $set, Cap = override in expr` |
| Visibility | Standard `pub` modifier |
| Evaluation | Compile-time constant |
| Resolution | Set bindings follow existing priority order |
| Type | `CapabilitySet<Cap1, Cap2, ...>` |

Capability sets provide:
- **DRY configuration**: Define once, use everywhere
- **Composable layering**: Build complex configs from simple bases
- **Explicit dependencies**: Clear what capabilities are provided
- **Zero runtime cost**: Expanded at compile time
- **Full compatibility**: Works with existing `def impl` and `with...in`
