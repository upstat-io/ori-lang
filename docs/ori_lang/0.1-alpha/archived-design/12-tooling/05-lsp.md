# Language Server Protocol (LSP)

Ori's LSP is designed for reviewing AI-generated code. It prioritizes verification, test visibility, and quick understanding over authoring assistance.

---

## Design Principles

1. **Show what matters, hide what does not** - No information overload
2. **Instant response** - No waiting, no spinners
3. **Accurate or nothing** - Wrong info is worse than no info
4. **Semantic, not syntactic** - Show meaning, not just structure
5. **Test-first visibility** - Every view shows test status

---

## Hover Information

### Basic Hover

Hovering over any identifier shows its type and signature:

```
@fetch_data (url: str) -> Result<Data, Error>
```

### Expanded Hover

Click or hold to expand with additional context:

```
@fetch_data (url: str) -> Result<Data, Error>

Pattern: retry
  .op: http_get(url)
  .attempts: 3
  .backoff: exponential(base: 100ms, max: 5s)

Defined in: src/api.ori:42
Tests: @test_fetch_success, @test_fetch_retry (3/3 passing)
```

### Function Hover

```
@process (items: [int]) -> int

Transforms a list of integers by doubling and filtering.

Body: run(
    let doubled = map(items, item -> item * 2),
    let filtered = filter(doubled, item -> item > 10),
    fold(filtered, 0, +),
)

Tests: 2/2 passing
  - @test_process_empty
  - @test_process_values
```

### Type Hover

```
type User = {
    id: int,
    name: str,
    email: str,
    role: Role
}

Defined in: src/types.ori:15
Used by: @fetch_user, @create_user, @update_user
```

### Config Hover

```
$timeout: int = 30

Configuration for request timeout in seconds.

Used by: @fetch_data, @api_call
```

### Closure Capture Hover

When hovering over a lambda, show captured variables:

```
filtered: [int]

Value derived from: filter(items, item -> item > threshold)
  Captures:
    threshold = 10 (from outer scope, line 5)
```

### Pattern Property Hover

```
.attempts: int = 3

retry pattern property: Maximum number of retry attempts.

Valid range: 1-100
Default: 3
```

---

## Go-to-Definition

### Single Definition

When there is one definition, jump directly (no popup).

### Multiple Relevant Locations

Show a picker with categorized options:

```
@fetch_data - Go to:
  > Definition        src/api.ori:42
    Tests             src/_test/api.test.ori:15
    Usages (12)
    Type definition   Result<Data, Error>
```

### Navigation Targets

| Target | Description |
|--------|-------------|
| Definition | Where the element is defined |
| Tests | Test functions for this element |
| Usages | All references in codebase |
| Type definition | For typed elements, the type's definition |
| Implementation | For pattern usage, the underlying implementation |

---

## Find References

### Categorized Results

```
References to @fetch_data (12 total)

Calls (8):
  src/handlers.ori:23    response = fetch_data(url)
  src/handlers.ori:45    data = fetch_data(api_url)
  src/main.ori:12        result = fetch_data(input)
  ...

Tests (3):
  src/_test/api.test.ori:15   @test_fetch_success tests @fetch_data
  src/_test/api.test.ori:28   @test_fetch_retry tests @fetch_data
  src/_test/api.test.ori:41   @test_fetch_error tests @fetch_data

Re-exports (1):
  src/lib.ori:5   pub use api { fetch_data }
```

### Filter Options

- All references
- Calls only
- Tests only
- Definitions only
- Current file only

---

## Diagnostics

### Speed Requirement

Diagnostics update within 50ms of keystroke.

### Precision

Underline exactly what is wrong, not the whole line:

```ori
result = value + "hello"
                 ^^^^^^^ expected int, found str
```

### Inline Quick Fixes

```
error[E0308]: type mismatch, expected int, found str

Quick fixes:
  > Convert right to int: int(right)
    Change parameter type to str
    Convert left to str: str(left)
```

### Diagnostic Levels

| Level | Display | Example |
|-------|---------|---------|
| Error | Red underline | Type mismatch |
| Warning | Yellow underline | Unused variable |
| Info | Blue underline | Suggestion |
| Hint | Faded text | Style suggestion |

### Test-Related Diagnostics

```ori
@helper (value: int) -> int = value * 2
~~~~~~~~ warning: function has no tests
```

---

## Completions

### Minimal, Ranked Results

Show 10-15 items maximum, not 200. Quality over quantity.

### Context-Aware for Patterns

When inside a pattern:

```ori
@f () -> int = retry(
    .
```

Completions:
```
.op         (required) - the operation to retry
.attempts   (required) - max retry count
.backoff    (optional) - backoff strategy
.on         (optional) - errors that trigger retry
.jitter     (optional) - add randomness
```

### Function Completions

```ori
result = fet
```

Completions:
```
fetch_data    (url: str) -> Result<Data, Error>
fetch_user    (id: int) -> Result<User, Error>
fetch_config  () -> Config
```

### Type Completions

```ori
@process (value: int) -> Res
```

Completions:
```
Result<T, E>  - Success or error type
Response      - HTTP response type (use std.http)
```

### Import Completions

```ori
use std.
```

Completions:
```
std.math     sqrt, abs, sin, cos, ...
std.io       print, read_file, write_file, ...
std.json     parse, stringify
std.http     get, post, put, delete
std.string   split, join, trim, ...
```

---

## Inlay Hints

### Type Hints

Show inferred types inline:

```ori
doubled: [int] = map(items, item -> item * 2)
```

### Closure Capture Hints

Make captured variables visible:

```ori
filter(items, item -> item > threshold)
                       [captured: 10]
```

### Pattern Property Hints

Show default values:

```ori
retry(
    .op: fetch(),
    // default: 3
    .attempts: 3,
)
```

### Configuration

Users can toggle:
- Type hints: on/off
- Capture hints: on/off
- Default value hints: on/off

---

## Code Actions

### Function-Level Actions

Right-click or lightbulb on a function:

```
@fetch_data (url: str) -> Result<Data, Error>

Actions:
  > Run tests for @fetch_data
    Go to tests
    Generate test skeleton
    Extract to module
    Inline function
    Rename symbol
```

### Expression-Level Actions

```
doubled = map(items, item -> item * 2)

Actions:
  > Extract to function
    Inline variable
    Convert to fold
```

### Error-Level Actions

```
error[E0308]: type mismatch

Actions:
  > Convert to int: int(value)
    Change expected type to str
    See error documentation
```

### Test-Centric Actions

| Action | Description |
|--------|-------------|
| Run tests | Execute tests for this function |
| Go to tests | Navigate to test file |
| Generate test | Create test skeleton |
| Debug test | Run test with debugger |
| Show coverage | Highlight tested lines |

---

## Semantic Highlighting

### Ori-Specific Highlighting

| Element | Style |
|---------|-------|
| `@function_name` | Function color + bold |
| `$config` | Constant color + italic |
| Pattern keywords | Keyword color (context-sensitive) |
| `.property:` | Parameter color |
| Captured variables | Different shade + underline |
| Type names | Type color |
| Variants | Enum member color |

### Context-Sensitive Keywords

Pattern keywords like `map`, `filter`, `fold` are highlighted as keywords only in pattern contexts:

```ori
// 'map' highlighted as keyword
result = map(items, transform)

// 'map' highlighted as identifier (function name)
@map (value: int) -> int = value
```

### Capture Highlighting

Variables captured by closures are visually distinct:

```ori
threshold = 10
filtered = filter(items, item -> item > threshold)
                                   ^^^^^^^^^
                                   captured (underlined, different shade)
```

---

## Document Outline

### Hierarchical View

```
api.ori
+-- Imports
|   +-- use std.http { get }
|   +-- use std.json { parse }
+-- Config
|   +-- $timeout = 30
|   +-- $base_url = "..."
+-- Types
|   +-- type Response = { ... }
+-- Functions
|   +-- @fetch_data (3 tests)
|   +-- @process (2 tests)
|   +-- @helper (0 tests) [!]
+-- Tests
    +-- @test_fetch_success
    +-- @test_fetch_error
```

### Coverage Indicators

| Icon | Meaning |
|------|---------|
| (checkmark) | Function has passing tests |
| (warning) | Function has no tests |
| (x) | Function has failing tests |
| (number) | Number of tests |

---

## Test Integration

### Inline Test Status

Show test status next to functions:

```ori
@fetch_data (url: str) -> Result<Data, Error> = ...
// 3/3 tests passing
```

### Code Lens

```
[Run Tests] [Debug] [Coverage]
@fetch_data (url: str) -> Result<Data, Error> = ...
```

### Test Explorer

Sidebar view of all tests:

```
Tests
+-- api.test.ori
|   +-- @test_fetch_success (passed)
|   +-- @test_fetch_error (passed)
|   +-- @test_fetch_retry (passed)
+-- math.test.ori
|   +-- @test_add (passed)
|   +-- @test_divide (failed)
```

### Run Tests Command

```
Command Palette > Ori: Run Tests

Options:
  > Run all tests
    Run tests in current file
    Run test at cursor
    Run failed tests
    Run tests for @fetch_data
```

---

## Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Diagnostics | < 50ms | After keystroke |
| Hover | < 20ms | Immediate feel |
| Completions | < 100ms | Before user notices |
| Go-to-definition | < 50ms | Instant navigation |
| Find references | < 200ms | Can show progress for large codebases |
| Document symbols | < 100ms | For outline view |
| Formatting | < 100ms | Per file |

### Caching Strategy

- Parse results cached until file changes
- Type information cached across files
- Reference graph incrementally updated
- Test results cached until source changes

---

## Workspace Features

### Multi-Root Support

Handle multiple Ori projects in one workspace.

### Project Detection

Automatically detect Ori projects by:
- Presence of `ori.toml`
- Presence of `src/` with `.ori` files

### Workspace-Wide Operations

- Find all references across workspace
- Rename across all projects
- Global type checking

---

## Configuration

Minimal configuration - most behavior is fixed for consistency.

### Available Settings

```json
{
  "ori.inlayHints.types": true,
  "ori.inlayHints.captures": true,
  "ori.inlayHints.defaults": false,
  "ori.diagnostics.delay": 50,
  "ori.testing.runOnSave": false,
  "ori.formatting.formatOnSave": true
}
```

### Non-Configurable

- Formatting style (always canonical)
- Diagnostic rules (always all enabled)
- Completion ranking (always semantic)

---

## Error Recovery

### Partial Analysis

When files have errors, LSP still provides:
- Syntax highlighting
- Navigation to valid parts
- Completions based on context
- Diagnostics for the errors

### Graceful Degradation

| File State | Available Features |
|------------|-------------------|
| Valid | All features |
| Parse errors | Highlighting, error diagnostics |
| Type errors | Navigation, completions, error diagnostics |
| Missing imports | Navigation within file, import suggestions |

---

## Integration Points

### With Formatter

- Format on save (configurable)
- Format selection
- Format on paste (optional)

### With Edit Operations

LSP code actions can invoke edit operations:

```
Code Action: Rename @old_func to @new_func
  -> Invokes: ori edit rename @old_func @new_func
```

### With Refactoring API

Complex refactorings available through code actions:

```
Code Action: Extract function
  -> Invokes: ori refactor extract-function @process.body[2:4] --name @helper
```

### With Test Runner

- Run tests from code lens
- Show test results inline
- Navigate to test failures

---

## Summary

| Feature | Design Goal |
|---------|-------------|
| Hover | Quick understanding without navigation |
| Go-to-definition | Smart multi-target with tests |
| Find references | Categorized by usage type |
| Diagnostics | Instant, precise, actionable |
| Completions | Minimal, context-aware |
| Inlay hints | Verify types, see captures |
| Code actions | Test-centric operations |
| Highlighting | Make captures visible |
| Outline | Show coverage status |
| Performance | Sub-100ms for everything |
| Test integration | First-class visibility |

---

## See Also

- [Semantic Addressing](01-semantic-addressing.md) - Navigation targets
- [Edit Operations](02-edit-operations.md) - Code action implementation
- [Structured Errors](03-structured-errors.md) - Diagnostic format
- [Formatter](04-formatter.md) - Format on save
- [Refactoring API](07-refactoring-api.md) - Complex code actions
- [Testing](../11-testing/index.md) - Test integration details
