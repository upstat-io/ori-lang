# Semantic Addressing

Semantic addressing allows precise, targeted operations on code elements without regenerating entire files. Every structural element in Sigil is addressable by name, enabling AI to make surgical edits.

---

## Design Principles

1. **Address by meaning, not location** - References are semantic (`@fetch_data.attempts`), not positional (`line 42, char 15`)
2. **Stable across formatting** - Addresses survive reformatting, comment changes, and reordering
3. **Project-wide scope** - AI does not need to track which file contains what
4. **Hierarchical structure** - Addresses follow the natural nesting of code elements

---

## Addressable Elements

### Functions

```
@function_name                    # The entire function
@function_name.body               # The function body expression
@function_name.params             # Parameter list
@function_name.params.param_name  # Specific parameter
@function_name.return_type        # Return type annotation
```

**Example:**
```sigil
@fetch_data (url: str, timeout: int) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

Addresses for this function:
```
@fetch_data                       # Entire function definition
@fetch_data.params.url            # The 'url' parameter
@fetch_data.params.timeout        # The 'timeout' parameter
@fetch_data.return_type           # Result<Data, Error>
@fetch_data.body                  # The retry(...) expression
```

### Pattern Properties

When a function body uses a pattern, the pattern's properties become addressable:

```
@fetch_data.op                    # http_get(url)
@fetch_data.attempts              # 3
@fetch_data.backoff               # exponential(base: 100ms, max: 5s)
@fetch_data.backoff.base          # 100ms
@fetch_data.backoff.max           # 5s
```

This enables targeted edits like:
```json
{ "op": "set", "address": "@fetch_data.attempts", "value": "5" }
```

### Config Variables

```
$config_name                      # The config definition
$config_name.value                # The config value
```

**Example:**
```sigil
$max_retries = 3
$api_base = "https://api.example.com"
```

Addresses:
```
$max_retries                      # Entire config
$max_retries.value                # 3
$api_base                         # Entire config
$api_base.value                   # "https://api.example.com"
```

### Types

```
type TypeName                     # The type definition
type TypeName.field_name          # Struct field
type TypeName.VariantName         # Sum type variant
type TypeName.VariantName.field   # Field within variant
```

**Struct example:**
```sigil
type User = {
    id: int,
    name: str,
    email: str,
    role: Role
}
```

Addresses:
```
type User                         # Entire type definition
type User.id                      # Field: id: int
type User.name                    # Field: name: str
type User.email                   # Field: email: str
type User.role                    # Field: role: Role
```

**Sum type example:**
```sigil
type Status = Pending | Running(progress: float) | Done | Failed(error: str)
```

Addresses:
```
type Status                       # Entire type definition
type Status.Pending               # Variant: Pending
type Status.Running               # Variant: Running(progress: float)
type Status.Running.progress      # Field within variant
type Status.Done                  # Variant: Done
type Status.Failed                # Variant: Failed(error: str)
type Status.Failed.error          # Field within variant
```

### Imports

```
@module.imports                   # All imports in module
@module.imports[0]                # First import statement
@module.imports["std.json"]       # Import of std.json module
```

### Tests

```
@test_name                        # Test function
@test_name.target                 # The function being tested
@test_name.assertions             # All assertions in test
@test_name.assertions[0]          # First assertion
```

---

## Address Syntax

### Dot Notation

Addresses use dot notation to traverse the hierarchy:

```
@function.property.subproperty
type TypeName.field
$config.value
```

**Rationale:**
- Matches how you would reference these elements in code
- Familiar: `@func.prop` looks like field access
- Simple and readable for both humans and AI

### Module Prefixes

For project-wide addressing, module names prefix element addresses:

```
@module.function                  # Function in module
@http.client.fetch                # fetch in http/client module
type math.Vector                  # Vector type in math module
$config.settings.timeout          # timeout config in config/settings module
```

**File resolution:**
```
@math.add           -> src/math.si
@http.client.fetch  -> src/http/client.si
type db.models.User -> src/db/models.si
```

### Escaping

When names contain special characters (rare but possible):

```
@module.`function-with-dashes`
type `Type.With.Dots`
```

Backticks escape names that would otherwise be ambiguous.

---

## Index Addressing

Collections support both numeric and content-based addressing.

### Numeric Index

```
@validate.rules[0]                # First rule
@validate.rules[2]                # Third rule
@func.body.statements[0]          # First statement in run block
```

**Pros:** Precise, unambiguous
**Cons:** Brittle if order changes

### Content-Based Index

```
@validate.rules["name required"]  # Rule containing this text
@func.imports["std.json"]         # Import of std.json
```

**Pros:** Stable across reordering
**Cons:** May be ambiguous if multiple matches

### Range Addressing

```
@func.body.statements[2:5]        # Statements 2, 3, 4
@func.body.statements[3:]         # From statement 3 to end
@func.body.statements[:3]         # First 3 statements
```

Useful for extract operations and batch modifications.

---

## Address Resolution

### Resolution Algorithm

1. Parse address into segments
2. Start at project root (or specified file)
3. For each segment:
   - If module name: narrow to that module
   - If element name: find element in current scope
   - If index: resolve to collection element
4. Return the AST node at final location

### Ambiguity Handling

When an address is ambiguous:

```json
{
  "error": "ambiguous_address",
  "address": "@helper",
  "matches": [
    { "file": "src/math.si", "line": 15 },
    { "file": "src/utils.si", "line": 8 }
  ],
  "suggestion": "Use fully qualified address: @math.helper or @utils.helper"
}
```

### Missing Elements

When an address does not resolve:

```json
{
  "error": "address_not_found",
  "address": "@api.fetch_data.retries",
  "nearest_match": "@api.fetch_data.attempts",
  "suggestion": "Did you mean @api.fetch_data.attempts?"
}
```

---

## Project-Wide Addresses

AI should not need to track which file contains what. Tooling resolves addresses to files automatically.

### Resolution Examples

```
@math.add           -> src/math.si:12
@http.client.fetch  -> src/http/client.si:45
type db.User        -> src/db.si:8
$config.timeout     -> src/config.si:3
```

### Listing Addresses

```bash
sigil address list                # List all addresses in project
sigil address list @math          # List addresses in math module
sigil address list --type func    # List all function addresses
sigil address list --type type    # List all type addresses
```

**Output:**
```json
{
  "addresses": [
    { "address": "@math.add", "file": "src/math.si", "line": 12, "kind": "function" },
    { "address": "@math.subtract", "file": "src/math.si", "line": 18, "kind": "function" },
    { "address": "type math.Point", "file": "src/math.si", "line": 5, "kind": "type" }
  ]
}
```

### Querying Addresses

```bash
sigil address info @fetch_data
```

**Output:**
```json
{
  "address": "@api.fetch_data",
  "kind": "function",
  "file": "src/api.si",
  "line": 42,
  "signature": "(url: str, timeout: int) -> Result<Data, Error>",
  "pattern": "retry",
  "properties": ["op", "attempts", "backoff"],
  "tests": ["@test_fetch_success", "@test_fetch_error"],
  "children": {
    "op": "@api.fetch_data.op",
    "attempts": "@api.fetch_data.attempts",
    "backoff": "@api.fetch_data.backoff"
  }
}
```

---

## Address Hierarchy

### Complete Hierarchy Example

Given this code:

```sigil
// src/api.si
use std.http { get }

$base_url = "https://api.example.com"

type Response = { status: int, body: str }

@fetch (path: str) -> Result<Response, Error> = retry(
    .op: get($base_url + path),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)

@test_fetch tests @fetch () -> void = run(
    result = fetch("/users"),
    assert(is_ok(result))
)
```

**Complete address tree:**
```
@api
├── imports
│   └── [0]: use std.http { get }
├── $base_url
│   └── value: "https://api.example.com"
├── type Response
│   ├── status: int
│   └── body: str
├── @fetch
│   ├── params
│   │   └── path: str
│   ├── return_type: Result<Response, Error>
│   ├── body: retry(...)
│   ├── op: get($base_url + path)
│   ├── attempts: 3
│   └── backoff
│       ├── base: 100ms
│       └── max: 5s
└── @test_fetch
    ├── target: @fetch
    └── assertions
        └── [0]: assert(is_ok(result))
```

---

## Integration with Other Tools

### Edit Operations

Addresses are the target of edit operations (see [Edit Operations](02-edit-operations.md)):

```json
{ "op": "set", "address": "@fetch.attempts", "value": "5" }
{ "op": "rename", "address": "@fetch", "new_name": "fetch_data" }
{ "op": "remove", "address": "type Response.body" }
```

### Structured Errors

Errors include semantic addresses (see [Structured Errors](03-structured-errors.md)):

```json
{
  "error": {
    "id": "E0308",
    "location": {
      "file": "src/api.si",
      "line": 15,
      "address": "@api.fetch.op"
    }
  }
}
```

### LSP

The LSP uses addresses for navigation and refactoring (see [LSP](05-lsp.md)):

```
Hover on "fetch" -> Shows @api.fetch info
Go-to-definition -> Uses @api.fetch to find location
Rename symbol -> Uses address for cross-file rename
```

### REPL

The REPL supports address queries (see [REPL](06-repl.md)):

```
> :address @math.add
{ "file": "src/math.si", "line": 12, "type": "(int, int) -> int" }

> :get @config.timeout
30
```

### Refactoring

Refactoring operations use addresses as targets (see [Refactoring API](07-refactoring-api.md)):

```bash
sigil refactor rename @old_name @new_name
sigil refactor extract-function @process.body[2:4] --name @helper
sigil refactor move @helper --to utils
```

---

## CLI Interface

### Query Commands

```bash
# Get info about an address
sigil address info @fetch_data

# List all addresses matching pattern
sigil address list "@api.*"

# Check if address exists
sigil address exists @fetch_data

# Get the value at an address
sigil address get @fetch_data.attempts
```

### JSON Mode

All commands support JSON output for AI consumption:

```bash
sigil address info @fetch_data --json
```

```json
{
  "address": "@api.fetch_data",
  "kind": "function",
  "file": "src/api.si",
  "line": 42,
  "exists": true
}
```

---

## Best Practices

### For AI

1. **Use fully qualified addresses** - `@api.fetch` not just `@fetch`
2. **Prefer content-based indexing** - More stable across refactoring
3. **Query before editing** - Verify address exists before operating
4. **Use hierarchical queries** - Get parent info to understand context

### For Tooling

1. **Always resolve to canonical form** - Normalize addresses
2. **Provide helpful suggestions** - On ambiguity or not-found
3. **Cache resolution** - Addresses are queried frequently
4. **Update on file changes** - Invalidate cache when files change

---

## Summary

| Element | Address Format | Example |
|---------|---------------|---------|
| Function | `@[module.]name` | `@api.fetch_data` |
| Pattern property | `@func.property` | `@fetch.attempts` |
| Nested property | `@func.prop.subprop` | `@fetch.backoff.base` |
| Config | `$[module.]name` | `$config.timeout` |
| Type | `type [module.]Name` | `type db.User` |
| Struct field | `type Name.field` | `type User.email` |
| Sum variant | `type Name.Variant` | `type Status.Running` |
| Variant field | `type Name.Variant.field` | `type Status.Running.progress` |
| Index | `@func.items[n]` | `@validate.rules[0]` |
| Content match | `@func.items["text"]` | `@validate.rules["name required"]` |
| Range | `@func.items[n:m]` | `@func.body[2:5]` |

---

## See Also

- [Edit Operations](02-edit-operations.md) - Operations using addresses
- [Structured Errors](03-structured-errors.md) - Addresses in error output
- [REPL](06-repl.md) - Interactive address queries
- [Refactoring API](07-refactoring-api.md) - Refactoring with addresses
