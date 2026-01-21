# REPL

Sigil's REPL (Read-Eval-Print Loop) supports interactive exploration and validation for both humans and AI. It features JSON output mode, semantic addressing integration, and built-in test execution.

---

## Design Principles

1. **Dual modes** - Human-readable and JSON output
2. **Stateful sessions** - Build up context across expressions
3. **Test integration** - Run tests directly from REPL
4. **Semantic addressing** - Query and modify code via addresses
5. **Minimal commands** - Focused set, easy to learn

---

## Starting the REPL

```bash
sigil repl              # Interactive mode
sigil repl --json       # JSON output mode
sigil repl --load src/  # Load files before starting
```

---

## Commands Overview

| Command | Purpose | Example |
|---------|---------|---------|
| `:help` | Show available commands | `:help` |
| `:quit` / `:q` | Exit REPL | `:q` |
| `:type <expr>` | Show type of expression | `:type map([1,2], x -> x * 2)` |
| `:ast <expr>` | Show AST (for debugging) | `:ast 1 + 2 * 3` |
| `:env` | Show current bindings | `:env` |
| `:clear` | Clear all bindings | `:clear` |
| `:load <file>` | Load and evaluate file | `:load src/utils.si` |
| `:reload` | Reload all loaded files | `:reload` |
| `:test <func>` | Run tests for a function | `:test @add` |
| `:check` | Type-check expression | `:check @myFunc` |
| `:json` | Toggle JSON output mode | `:json` |
| `:address <addr>` | Query semantic address | `:address @math.add` |
| `:get <addr>` | Get value at address | `:get @config.timeout` |
| `:set <addr> <val>` | Set value at address | `:set @config.timeout 60` |
| `:save <file>` | Save session state | `:save session.repl` |
| `:restore <file>` | Restore session state | `:restore session.repl` |
| `:history` | Show command history | `:history` |

---

## Output Modes

### Human Mode (Default)

```
> 1 + 2
3

> map([1, 2, 3], x -> x * 2)
[2, 4, 6]

> @add (a: int, b: int) -> int = a + b
Function @add defined

> add(5, 3)
8
```

### JSON Mode

```
> :json
JSON mode enabled

> 1 + 2
{"result": 3, "type": "int"}

> map([1, 2, 3], x -> x * 2)
{"result": [2, 4, 6], "type": "[int]"}

> @add (a: int, b: int) -> int = a + b
{"status": "defined", "name": "@add", "type": "(int, int) -> int"}

> add(5, 3)
{"result": 8, "type": "int"}
```

### Toggle Between Modes

```
> :json
JSON mode enabled

> :json
JSON mode disabled (human mode)
```

### Errors in Both Modes

**Human mode:**
```
> x + "hello"
error[E0308]: mismatched types
  |
1 | x + "hello"
  |     ^^^^^^^ expected int, found str
```

**JSON mode:**
```json
{
  "error": {
    "id": "E0308",
    "message": "mismatched types",
    "expected": "int",
    "found": "str",
    "line": 1,
    "column": 5,
    "suggestions": [
      {
        "message": "convert int to str",
        "replacement": "str(x) + \"hello\""
      }
    ]
  }
}
```

---

## Multi-Line Input

### Automatic Detection

The REPL automatically detects incomplete expressions:

```
> @factorial (n: int) -> int = recurse(
...     .cond: n <= 1,
...     .base: 1,
...     .step: n * self(n - 1)
... )
Function @factorial defined
```

### Detection Rules

Multi-line mode activates when:
- Unmatched `(`, `[`, or `{`
- Line ends with `=`
- Pattern started but not closed
- Line ends with `_` (explicit continuation)

### Explicit Continuation

Use `_` at end of line to force continuation:

```
> result = very_long_expression + _
...     another_long_expression
42
```

### Cancel Multi-Line

Press Ctrl+C to cancel and return to single-line mode:

```
> @incomplete (x: int) = run(
...     something,
... ^C
Cancelled

>
```

---

## Session State

### Building Up Context

Bindings persist within a session:

```
> x = 42
42

> y = x * 2
84

> z = x + y
126

> :env
Bindings:
  x: int = 42
  y: int = 84
  z: int = 126
```

### Defining Functions

Functions defined in REPL are available immediately:

```
> @double (n: int) -> int = n * 2
Function @double defined

> @quadruple (n: int) -> int = double(double(n))
Function @quadruple defined

> quadruple(5)
20
```

### Clearing State

```
> :clear
Session cleared

> x
error: unknown identifier 'x'
```

### Save and Restore

```
> x = 42
> @helper (n: int) -> int = n + 1
> :save mysession.repl
Session saved to mysession.repl (2 bindings, 1 function)

# Later or different session:
> :restore mysession.repl
Session restored (2 bindings, 1 function)
> x
42
```

---

## Loading Files

### Load Single File

```
> :load src/math.si
Loaded src/math.si: 5 functions, 2 types, 0 errors
```

### Load Directory

```
> :load src/
Loaded 12 files: 45 functions, 8 types, 0 errors
```

### Reload

After editing files externally:

```
> :reload
Reloaded 12 files: 45 functions, 8 types, 0 errors
```

### Load Errors

```
> :load broken.si
Load error in broken.si:
  line 15: syntax error: unexpected token

Partial load: 3 functions available
```

**JSON mode:**
```json
{
  "status": "partial",
  "file": "broken.si",
  "loaded": {
    "functions": 3,
    "types": 1
  },
  "errors": [{
    "line": 15,
    "message": "syntax error: unexpected token"
  }]
}
```

---

## Type Inspection

### Show Type

```
> :type map([1, 2, 3], x -> x * 2)
[int]

> :type @fetch_data
(str) -> Result<Data, Error>

> :type x -> x + 1
(int) -> int
```

### JSON Output

```json
{
  "expression": "map([1, 2, 3], x -> x * 2)",
  "type": "[int]",
  "type_details": {
    "kind": "list",
    "element_type": "int"
  }
}
```

### Check Without Running

```
> :check fetch_data("url") + 1
error[E0308]: mismatched types
  expected: numeric type
  found: Result<Data, Error>
```

---

## Testing Integration

### Run Tests for Function

```
> :test @add
Running tests for @add...
  [pass] @test_add_positive
  [pass] @test_add_negative
  [pass] @test_add_zero
3/3 tests passed
```

### Run All Tests

```
> :test
Running all tests...
  [pass] @test_add_positive
  [pass] @test_add_negative
  [pass] @test_process_empty
  [fail] @test_divide_by_zero
         assertion failed: expected panic, got 0
4/5 tests passed
```

### JSON Test Output

```json
{
  "command": "test",
  "target": "@add",
  "results": [
    {"test": "@test_add_positive", "status": "passed", "time_ms": 2},
    {"test": "@test_add_negative", "status": "passed", "time_ms": 1},
    {"test": "@test_add_zero", "status": "passed", "time_ms": 1}
  ],
  "summary": {
    "passed": 3,
    "failed": 0,
    "total": 3,
    "time_ms": 4
  }
}
```

### Test Failure Details

```
> :test @divide
Running tests for @divide...
  [pass] @test_divide_normal
  [fail] @test_divide_by_zero
         expected: panic
         actual: returned 0
         at: src/_test/math.test.si:25
1/2 tests passed
```

---

## Semantic Addressing in REPL

### Query Address

```
> :address @math.add
{
  "address": "@math.add",
  "file": "src/math.si",
  "line": 12,
  "type": "(int, int) -> int",
  "tests": ["@test_add_positive", "@test_add_negative"]
}
```

### Get Value

```
> :get @config.timeout
30

> :get @math.add.body
a + b
```

### Set Value

```
> :set @config.timeout 60
Updated @config.timeout: 30 -> 60
Note: Change is temporary (session only). Use 'sigil edit' for permanent changes.
```

### List Addresses

```
> :address @math.*
@math.add        (int, int) -> int
@math.subtract   (int, int) -> int
@math.multiply   (int, int) -> int
@math.divide     (int, int) -> Result<int, Error>
type math.Point  { x: int, y: int }
```

---

## History

### Show History

```
> :history
  1: x = 42
  2: y = x * 2
  3: :type y
  4: map([1,2,3], x -> x * 2)
  5: :test @add
```

### Search History

```
> :history search map
  4: map([1,2,3], x -> x * 2)
  7: map(items, transform)
  12: :type map
```

### Recall Previous

Use up/down arrows to navigate history.

### Persistent History

History is saved to `~/.sigil/repl_history` and persists across sessions.

---

## AST Inspection

For debugging and learning:

```
> :ast 1 + 2 * 3
BinaryOp {
  op: Add,
  left: Int(1),
  right: BinaryOp {
    op: Mul,
    left: Int(2),
    right: Int(3)
  }
}
```

### JSON AST

```json
{
  "kind": "binary_op",
  "op": "add",
  "left": {"kind": "int", "value": 1},
  "right": {
    "kind": "binary_op",
    "op": "mul",
    "left": {"kind": "int", "value": 2},
    "right": {"kind": "int", "value": 3}
  }
}
```

---

## Help System

### General Help

```
> :help
Sigil REPL Commands:

Evaluation:
  <expr>              Evaluate expression
  :type <expr>        Show type of expression
  :ast <expr>         Show AST of expression
  :check <expr>       Type-check without running

Session:
  :env                Show current bindings
  :clear              Clear all bindings
  :save <file>        Save session state
  :restore <file>     Restore session state

Files:
  :load <path>        Load file or directory
  :reload             Reload all loaded files

Testing:
  :test               Run all tests
  :test <func>        Run tests for function

Addressing:
  :address <addr>     Query semantic address
  :get <addr>         Get value at address
  :set <addr> <val>   Set value at address

Output:
  :json               Toggle JSON output mode
  :history            Show command history

Other:
  :help               Show this help
  :help <command>     Show help for command
  :quit / :q          Exit REPL
```

### Command-Specific Help

```
> :help test
:test [target]

Run tests. Without argument, runs all tests in loaded files.
With argument, runs tests for the specified function.

Examples:
  :test               Run all tests
  :test @add          Run tests for @add
  :test @math.add     Run tests for @math.add

Output:
  Human mode: Shows pass/fail with details
  JSON mode: Returns structured test results
```

---

## Error Display

### Syntax Errors

**Human mode:**
```
> 1 +
error: unexpected end of input
  |
1 | 1 +
  |    ^ expected expression
```

**JSON mode:**
```json
{
  "error": {
    "type": "syntax",
    "message": "unexpected end of input",
    "line": 1,
    "column": 4,
    "expected": "expression"
  }
}
```

### Type Errors

**Human mode:**
```
> "hello" + 5
error[E0308]: mismatched types
  |
1 | "hello" + 5
  |           ^ expected str, found int
  |
  = help: convert int to str with str(5)
```

### Runtime Errors

**Human mode:**
```
> divide(10, 0)
panic: division by zero
  at: divide (src/math.si:25)
```

**JSON mode:**
```json
{
  "error": {
    "type": "panic",
    "message": "division by zero",
    "location": {
      "function": "@divide",
      "file": "src/math.si",
      "line": 25
    }
  }
}
```

---

## AI Usage Patterns

### Exploration Loop

```bash
# Start REPL in JSON mode
sigil repl --json --load src/

# AI sends expressions, parses JSON responses
> :type @fetch_data
{"type": "(str) -> Result<Data, Error>"}

> fetch_data("https://api.example.com")
{"result": {"status": 200, "body": "..."}, "type": "Result<Data, Error>"}

> :test @fetch_data
{"passed": 3, "failed": 0, ...}
```

### Quick Validation

```bash
# AI validates code before committing
> :load src/changes.si
{"status": "ok", "functions": 5}

> :test @new_function
{"passed": 2, "failed": 0}

> :check new_function(test_input)
{"type": "Result<Output, Error>"}
```

### Semantic Exploration

```bash
# AI explores codebase structure
> :address @api.*
{"addresses": ["@api.fetch", "@api.post", ...]}

> :get @api.fetch.attempts
{"value": 3, "type": "int"}

> :set @api.fetch.attempts 5
{"updated": true, "old": 3, "new": 5}
```

---

## Configuration

### Startup Options

```bash
sigil repl --json           # Start in JSON mode
sigil repl --load src/      # Load files on start
sigil repl --no-history     # Don't save history
sigil repl --quiet          # Suppress banner
```

### Environment Variables

```bash
SIGIL_REPL_HISTORY=~/.sigil/history  # History file location
SIGIL_REPL_HISTSIZE=1000             # Max history entries
```

---

## Summary

| Feature | Description |
|---------|-------------|
| Output modes | Human-readable and JSON |
| Multi-line | Automatic detection |
| Session state | Persistent bindings and functions |
| File loading | Load and reload source files |
| Testing | Run tests directly |
| Semantic addressing | Query and modify via addresses |
| History | Persistent with search |

| Command | Purpose |
|---------|---------|
| `:type` | Inspect types |
| `:env` | View bindings |
| `:load` | Load files |
| `:test` | Run tests |
| `:address` | Query addresses |
| `:json` | Toggle JSON mode |

---

## See Also

- [Semantic Addressing](01-semantic-addressing.md) - Address syntax
- [Structured Errors](03-structured-errors.md) - Error format
- [Testing](../11-testing/index.md) - Test system
