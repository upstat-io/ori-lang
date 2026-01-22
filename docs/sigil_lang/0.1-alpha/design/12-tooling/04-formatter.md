# Formatter

Sigil has one canonical format with zero configuration. All Sigil code looks the same, eliminating style decisions for both humans and AI.

---

## Design Principles

1. **One format, no options** - The style is whatever the formatter outputs
2. **Deterministic** - Same input always produces same output
3. **Idempotent** - Formatting formatted code produces identical output
4. **Semantic preservation** - Only whitespace changes, never meaning
5. **Integration with edits** - Edit operations auto-format

---

## Philosophy: No Configuration

```bash
sigil fmt src/          # Format all files
sigil fmt --check src/  # Check without modifying (for CI)
```

**No options for:**
- Indent size (always 4 spaces)
- Line length (always 100)
- Brace style (deterministic rules)
- Trailing commas (always on multi-line)
- Any other style preference

### Rationale

- Zero style decisions for AI
- All Sigil code looks identical
- Diffs are purely semantic, never stylistic
- No bikeshedding or team disagreements
- "Gofmt style" - the style is whatever the formatter outputs

---

## Indentation

### Rule: 4 Spaces, No Tabs

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(items, x -> x * 2),
    let filtered = filter(doubled, x -> x > 10),
    fold(filtered, 0, +),
)
```

### Rationale

- Spaces are unambiguous (tabs render differently across editors)
- 4 spaces provides clear visual hierarchy
- AI generates spaces consistently
- Matches common industry standard

### Nested Indentation

Each nesting level adds 4 spaces:

```sigil
@complex () -> Result<int, FetchError> = try(
    let data = fetch()?,
    let processed = match(data,
        Some(d) -> run(
            let validated = validate(d),
            transform(validated),
        ),
        None -> Err(FetchError.NotFound),
    ),
    Ok(processed),
)
```

---

## Line Length

### Rule: 100 Characters, Hard Limit

Lines exceeding 100 characters are broken at natural points.

### Under Limit: Single Line

```sigil
@add (a: int, b: int) -> int = a + b

@greet (name: str) -> str = "Hello, " + name + "!"

$timeout = 30
```

### Over Limit: Break Required

When a line exceeds 100 characters, the formatter breaks it according to deterministic rules.

---

## Breaking Rules

### Function Signatures

**Short signature - single line:**
```sigil
@add (a: int, b: int) -> int = a + b
```

**Long signature - break after arrow:**
```sigil
@process_with_long_name (first_param: int, second_param: str, third_param: bool) ->
    Result<ProcessedData, Error> = ...
```

**Very long signature - break parameters:**
```sigil
@very_long_function_name (
    first_parameter: int,
    second_parameter: str,
    third_parameter: bool,
    fourth_parameter: Option<Config>,
) -> Result<ProcessedData, Error> = ...
```

### Pattern Arguments

**Rule: All named properties are always stacked vertically.**

| Properties | Format |
|------------|--------|
| 1+ properties | Must stack vertically |

**Single property (stacked):**
```sigil
@get_length (arr: [int]) -> int = fold(
    .over: arr,
)
```

**Multiple properties (stacked):**

```sigil
@fetch_with_retry () -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
    .on: [Timeout, ConnectionError],
)

@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true,
)

@doubled (items: [int]) -> [int] = map(
    .over: items,
    .transform: x -> x * 2,
)
```

**Rationale:**
- The `.property:` sigil creates a visual rail down the left side when stacked
- Scanability: dots align vertically, making parameters instantly visible
- Consistent format makes code predictable — no "sometimes inline, sometimes stacked"
- Easy to scan and modify individual properties
- Diffs are cleaner when adding/removing properties

**Exception:** `run` and `try` patterns use `let` binding syntax, not named properties:

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: x -> x * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: x -> x > 10,
    ),
    fold(
        .over: filtered,
        .init: 0,
        .op: +,
    ),
)
```

### Lists and Arrays

**Short - single line:**
```sigil
items = [1, 2, 3, 4, 5]
```

**Long - each element on its own line:**
```sigil
items = [
    first_long_item,
    second_long_item,
    third_long_item,
    fourth_long_item,
]
```

### Struct Literals

**Short - single line:**
```sigil
point = { x: 10, y: 20 }
```

**Long - each field on its own line:**
```sigil
user = {
    id: generate_id(),
    name: format_name(first, last),
    email: validate_email(input),
    role: default_role,
}
```

### Binary Expressions

**Short - single line:**
```sigil
result = a + b * c - d
```

**Long - break before operators:**
```sigil
result = first_long_operand
    + second_long_operand
    - third_long_operand
    * fourth_long_operand
```

### Conditionals

**Short - single line:**
```sigil
@abs (n: int) -> int = if n >= 0 then n else -n
```

**Long - break at keywords:**
```sigil
@categorize (value: int) -> str =
    if value < 0 then "negative"
    else if value == 0 then "zero"
    else if value < 10 then "small"
    else if value < 100 then "medium"
    else "large"
```

---

## Trailing Commas

### Rule: Always on Multi-Line Constructs

```sigil
// Single line - no trailing comma
items = [1, 2, 3]
point = { x: 10, y: 20 }

// Multi-line - always trailing comma
items = [
    first,
    second,
    third,
]

point = {
    x: 10,
    y: 20,
}

@fetch () -> Data = retry(
    .op: get(url),
    .attempts: 3,
)
```

### Rationale

- Cleaner diffs when adding/removing items
- Consistent - never wonder "does this need a comma?"
- AI can add items without touching previous line

---

## Spacing Rules

### Binary Operators

Space around all binary operators:

```sigil
a + b
x == y
n >= 0
left && right
value | default
```

### Commas

Space after, not before:

```sigil
f(a, b, c)
[1, 2, 3]
{ x: 1, y: 2 }
```

### Colons

Space after, not before:

```sigil
// Type annotations
x: int
name: str

// Pattern properties
.key: value
.attempts: 3

// Struct fields
{ name: "Alice", age: 30 }
```

### Parentheses and Brackets

No inner space:

```sigil
f(x)           // not f( x )
[1, 2, 3]      // not [ 1, 2, 3 ]
{ x: 1 }       // not { x: 1 }
```

### Arrow Operator

Space around:

```sigil
x -> x * 2
(a, b) -> a + b
```

### Function Definition

```sigil
@name (params) -> return_type = body
     ^       ^  ^             ^
     space   space  space    space
```

---

## Blank Lines

### After Import Block

```sigil
use std.math { sqrt, abs }
use std.io { print }
                              // one blank line
$config_value = 42
```

### After Config Block

```sigil
$timeout = 30
$retries = 3
                              // one blank line
type Config = { ... }
```

### After Type Definitions

```sigil
type Point = { x: int, y: int }
type Color = Red | Green | Blue
                              // one blank line
@first_function () -> int = 1
```

### Between Functions

```sigil
@function1 () -> int = 1
                              // one blank line
@function2 () -> int = 2
                              // one blank line
@function3 () -> int = 3
```

### Rules Summary

- One blank line after import block
- One blank line after config block
- One blank line after type definitions (as a group)
- One blank line between functions
- No multiple consecutive blank lines
- No trailing blank lines at end of file
- No leading blank lines at start of file

---

## Comments

### Single-Line Comments

Space after `//`:

```sigil
// This is a comment
@add (a: int, b: int) -> int = a + b
```

### Inline Comments

Two spaces before `//`:

```sigil
@add (a: int, b: int) -> int = a + b  // adds two integers
```

### Doc Comments

See [Documentation](../13-documentation/index.md) for doc comment formatting:

```sigil
// #Fetches user from database
// @param id must be positive
// !NotFound: user doesn't exist
@fetch_user (id: int) -> Result<User, Error> = ...
```

### Comment Preservation

The formatter:
- Preserves comment content (does not reflow)
- Normalizes spacing (adds space after `//`)
- Maintains comment position relative to code

---

## Idempotence

### Guarantee

```
format(format(code)) == format(code)
```

Running the formatter twice produces the same output as running it once.

### Verification

```bash
sigil fmt src/
sigil fmt --check src/  # Should report "0 files would change"
```

### CI Integration

```yaml
# In CI pipeline
- name: Check formatting
  run: sigil fmt --check src/
  # Fails if any file needs formatting
```

---

## Semantic Preservation

The formatter NEVER changes semantics.

### Does Not:

- Reorder imports
- Reorder functions
- Remove dead code
- Add imports
- Remove comments
- Change string contents
- Modify any values

### Only Changes:

- Whitespace (spaces, tabs, newlines)
- Line breaks
- Indentation

### Safety

Formatting is always safe to run:
- No surprises
- No "helpful" transformations
- Clear separation: formatter = style, compiler = semantics

---

## Integration with Edit Operations

Edit operations automatically format affected code.

### Behavior

```bash
sigil edit set @fetch.attempts 5
# Automatically formats the modified function
```

### Result

Edits always produce canonical code without a separate format step.

### Disable Auto-Format

```bash
sigil edit set @fetch.attempts 5 --no-format
```

Or in JSON:
```json
{
  "edit": { "op": "set", "address": "@fetch.attempts", "value": "5" },
  "options": { "format": false }
}
```

---

## Handling Parse Errors

### Partial Formatting

When a file has syntax errors, the formatter:
1. Formats valid portions
2. Reports errors for broken sections
3. Exits with code 1

```bash
sigil fmt broken.si
# Formats valid portions, outputs error for broken section
```

### JSON Output

```json
{
  "formatted_files": ["valid.si", "other.si"],
  "partial_files": ["broken.si"],
  "errors": [{
    "file": "broken.si",
    "line": 15,
    "message": "syntax error: unmatched parenthesis",
    "formatted_before_error": true
  }]
}
```

### Rationale

- Don't lose work because of one error
- AI can see what's broken and fix it
- Partial formatting is better than none

---

## CLI Commands

### Format Files

```bash
# Format all .si files in directory
sigil fmt src/

# Format specific file
sigil fmt src/main.si

# Format multiple paths
sigil fmt src/api.si src/utils.si
```

### Check Mode

```bash
# Check without modifying (for CI)
sigil fmt --check src/

# Exit code 0: all formatted
# Exit code 1: some files need formatting
```

### Diff Mode

```bash
# Show what would change
sigil fmt --diff src/

# Output shows unified diff
```

### Stdin/Stdout

```bash
# Format from stdin
cat code.si | sigil fmt -

# Output to stdout (no file modification)
sigil fmt --stdout src/main.si
```

### JSON Output

```bash
sigil fmt src/ --json
```

```json
{
  "files_checked": 10,
  "files_formatted": 3,
  "files_unchanged": 7,
  "formatted": [
    "src/api.si",
    "src/handlers.si",
    "src/utils.si"
  ]
}
```

---

## Editor Integration

### Format on Save

Editors should format on save. The formatter is fast enough for this.

### Performance Target

| Operation | Target |
|-----------|--------|
| Single file | < 50ms |
| 100 files | < 1s |
| 1000 files | < 5s |

### LSP Integration

The LSP provides `textDocument/formatting`:

```json
{
  "method": "textDocument/formatting",
  "params": {
    "textDocument": { "uri": "file:///src/main.si" },
    "options": { "tabSize": 4, "insertSpaces": true }
  }
}
```

Note: Options are ignored - Sigil always uses its canonical format.

---

## Example: Before and After

### Before (Inconsistent)

```sigil
use std.math {sqrt,abs}
use std.io{print}
$timeout=30
type Point={x:int,y:int}

@add(a:int,b:int)->int=a+b

@process(items:[int])->int=run(
doubled=map(items,x->x*2),
filtered=filter(doubled,x->x>10),
    fold(filtered,0,+)
)


@fetch()->Result<Data,Error>=retry(.op:get(url),.attempts:3,.backoff:exponential(base:100ms,max:5s))
```

### After (Canonical)

```sigil
use std.math { sqrt, abs }
use std.io { print }

$timeout = 30

type Point = { x: int, y: int }

@add (a: int, b: int) -> int = a + b

@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: x -> x * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: x -> x > 10,
    ),
    fold(
        .over: filtered,
        .init: 0,
        .op: +,
    ),
)

@fetch () -> Result<Data, Error> = retry(
    .op: get(url),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
)
```

---

## Summary

| Rule | Specification |
|------|---------------|
| Indentation | 4 spaces, no tabs |
| Line length | 100 characters hard limit |
| Breaking | Deterministic at natural points |
| Trailing commas | Always on multi-line |
| Binary operators | Space around |
| Commas | Space after |
| Colons | Space after |
| Parentheses | No inner space |
| Arrows | Space around |
| Blank lines | One between sections, none consecutive |
| Comments | Space after `//` |

| Feature | Guarantee |
|---------|-----------|
| No configuration | One canonical format |
| Idempotent | format(format(x)) == format(x) |
| Semantic preservation | Only whitespace changes |
| Edit integration | Auto-format on edit |
| Partial formatting | Handles parse errors |

---

## See Also

- [Edit Operations](02-edit-operations.md) - Auto-formatting on edit
- [LSP](05-lsp.md) - Format on save
- [Documentation](../13-documentation/index.md) - Doc comment formatting
