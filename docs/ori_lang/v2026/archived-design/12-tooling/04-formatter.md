# Formatter

Ori has one canonical format with zero configuration. All Ori code looks the same, eliminating style decisions for both humans and AI.

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
ori fmt src/          # Format all files
ori fmt --check src/  # Check without modifying (for CI)
```

**No options for:**
- Indent size (always 4 spaces)
- Line length (always 100)
- Brace style (deterministic rules)
- Trailing commas (always on multi-line)
- Any other style preference

### Rationale

- Zero style decisions for AI
- All Ori code looks identical
- Diffs are purely semantic, never stylistic
- No bikeshedding or team disagreements
- "Gofmt style" - the style is whatever the formatter outputs

---

## Indentation

### Rule: 4 Spaces, No Tabs

```ori
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)
```

### Rationale

- Spaces are unambiguous (tabs render differently across editors)
- 4 spaces provides clear visual hierarchy
- AI generates spaces consistently
- Matches common industry standard

### Nested Indentation

Each nesting level adds 4 spaces:

```ori
@complex () -> Result<int, FetchError> = try(
    let data = fetch()?,
    let processed = match(data,
        Some(data) -> run(
            let validated = validate(data),
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

```ori
@add (left: int, right: int) -> int = left + right

@greet (name: str) -> str = "Hello, " + name + "!"

$timeout = 30
```

### Over Limit: Break Required

When a line exceeds 100 characters, the formatter breaks it according to deterministic rules.

---

## Breaking Rules

### Function Signatures

**Short signature - single line:**
```ori
@add (left: int, right: int) -> int = left + right
```

**Long signature - break after arrow:**
```ori
@process_with_long_name (first_param: int, second_param: str, third_param: bool) ->
    Result<ProcessedData, Error> = ...
```

**Very long signature - break parameters:**
```ori
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
```ori
@get_length (arr: [int]) -> int = fold(
    .over: arr,
)
```

**Multiple properties (stacked):**

```ori
@fetch_with_retry () -> Result<Data, Error> = retry(
    .operation: http_get(url),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
    .on: [Timeout, ConnectionError],
)

@fibonacci (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: number,
    .step: self(number - 1) + self(number - 2),
    .memo: true,
)

@doubled (items: [int]) -> [int] = map(
    .over: items,
    .transform: item -> item * 2,
)
```

**Rationale:**
- The `.property:` ori creates a visual rail down the left side when stacked
- Scanability: dots align vertically, making parameters instantly visible
- Consistent format makes code predictable — no "sometimes inline, sometimes stacked"
- Easy to scan and modify individual properties
- Diffs are cleaner when adding/removing properties

**Exception:** `run` and `try` patterns use `let` binding syntax, not named properties:

```ori
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)
```

### Lists and Arrays

List literals prefer inline formatting, with smart wrapping when they exceed column width.

**Short - inline:**
```ori
items = [1, 2, 3, 4, 5]
names = ["alice", "bob", "charlie"]
```

**Exceeds column width - bump brackets, wrap values:**
```ori
numbers = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10,
    11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
]

names = [
    "alpha", "beta", "gamma", "delta", "epsilon",
    "zeta", "eta", "theta", "iota", "kappa",
]

tasks = [
    get_user(1), get_user(2), get_user(3),
    get_user(4), get_user(5), get_user(6),
]
```

**Inside function_exp - same rules apply:**
```ori
// Short list stays inline
sum(
    .values: [1, 2, 3, 4, 5],
)

// Long list bumps brackets, wraps values
process(
    .items: [
        "first", "second", "third", "fourth", "fifth",
        "sixth", "seventh", "eighth", "ninth", "tenth",
    ],
)

filter(
    .over: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    .predicate: item -> item > 5,
)
```

**Rules:**
1. List fits in column width → inline: `[1, 2, 3]`
2. List exceeds column width → bump brackets to own lines, indent contents
3. Values wrap at column width, multiple values per line
4. Trailing comma on last line when wrapped
5. Named params (`.name:`) always stack; list literals inside them wrap independently

**Contrast with named params:**

| Construct | Format |
|-----------|--------|
| Named params (`.name:`) | Always stack, one per line |
| List literals (`[...]`) | Inline, wrap at column width |

This distinction exists because:
- Named params are configuration — one per line aids scanning
- List literals are data — inline is natural, wrap when needed

### Struct Literals

**Short - single line:**
```ori
point = { x: 10, y: 20 }
```

**Long - each field on its own line:**
```ori
user = {
    id: generate_id(),
    name: format_name(first, last),
    email: validate_email(input),
    role: default_role,
}
```

### Binary Expressions

**Short - single line:**
```ori
result = first + second * third - fourth
```

**Long - break before operators:**
```ori
result = first_long_operand
    + second_long_operand
    - third_long_operand
    * fourth_long_operand
```

### Conditionals

**Short - single line:**
```ori
@abs (number: int) -> int = if number >= 0 then number else -number
```

**Long - break at keywords:**
```ori
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

```ori
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
    .operation: get(url),
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

```ori
a + b
x == y
n >= 0
left && right
value | default
```

### Commas

Space after, not before:

```ori
process(first, second, third)
[1, 2, 3]
{ x: 1, y: 2 }
```

### Colons

Space after, not before:

```ori
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

```ori
// CORRECT:
f(x)
[1, 2, 3]
{ x: 1 }

// INCORRECT:
// f( x )
// [ 1, 2, 3 ]
```

### Arrow Operator

Space around:

```ori
item -> item * 2
(left, right) -> left + right
```

### Function Definition

```ori
@name (params) -> return_type = body
     ^       ^  ^             ^
     space   space  space    space
```

---

## Blank Lines

### After Import Block

```ori
use std.math { sqrt, abs }
use std.io { print }
// one blank line below
$config_value = 42
```

### After Config Block

```ori
$timeout = 30
$retries = 3
// one blank line below
type Config = { ... }
```

### After Type Definitions

```ori
type Point = { x: int, y: int }
type Color = Red | Green | Blue
// one blank line below
@first_function () -> int = 1
```

### Between Functions

```ori
@function1 () -> int = 1
// one blank line below
@function2 () -> int = 2
// one blank line below
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

```ori
// This is a comment
@add (left: int, right: int) -> int = left + right
```

### No Inline Comments

Comments must appear on their own line, not after code:

```ori
// adds two integers
@add (left: int, right: int) -> int = left + right
```

### Doc Comments

See [Documentation](../13-documentation/index.md) for doc comment formatting:

```ori
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
ori fmt src/
ori fmt --check src/  # Should report "0 files would change"
```

### CI Integration

```yaml
# In CI pipeline
- name: Check formatting
  run: ori fmt --check src/
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
ori edit set @fetch.attempts 5
# Automatically formats the modified function
```

### Result

Edits always produce canonical code without a separate format step.

### Disable Auto-Format

```bash
ori edit set @fetch.attempts 5 --no-format
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
ori fmt broken.ori
# Formats valid portions, outputs error for broken section
```

### JSON Output

```json
{
  "formatted_files": ["valid.ori", "other.ori"],
  "partial_files": ["broken.ori"],
  "errors": [{
    "file": "broken.ori",
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
# Format all .ori files in directory
ori fmt src/

# Format specific file
ori fmt src/main.ori

# Format multiple paths
ori fmt src/api.ori src/utils.ori
```

### Check Mode

```bash
# Check without modifying (for CI)
ori fmt --check src/

# Exit code 0: all formatted
# Exit code 1: some files need formatting
```

### Diff Mode

```bash
# Show what would change
ori fmt --diff src/

# Output shows unified diff
```

### Stdin/Stdout

```bash
# Format from stdin
cat code.ori | ori fmt -

# Output to stdout (no file modification)
ori fmt --stdout src/main.ori
```

### JSON Output

```bash
ori fmt src/ --json
```

```json
{
  "files_checked": 10,
  "files_formatted": 3,
  "files_unchanged": 7,
  "formatted": [
    "src/api.ori",
    "src/handlers.ori",
    "src/utils.ori"
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
    "textDocument": { "uri": "file:///src/main.ori" },
    "options": { "tabSize": 4, "insertSpaces": true }
  }
}
```

Note: Options are ignored - Ori always uses its canonical format.

---

## Example: Before and After

### Before (Inconsistent)

```ori
use std.math {sqrt,abs}
use std.io{print}
$timeout=30
type Point={x:int,y:int}

@add(left:int,right:int)->int=left+right

@process(items:[int])->int=run(
doubled=map(items,item->item*2),
filtered=filter(doubled,item->item>10),
    fold(filtered,0,+)
)


@fetch()->Result<Data,Error>=retry(.operation:get(url),.attempts:3,.backoff:exponential(base:100ms,max:5s))
```

### After (Canonical)

```ori
use std.math { sqrt, abs }
use std.io { print }

$timeout = 30

type Point = { x: int, y: int }

@add (left: int, right: int) -> int = left + right

@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)

@fetch () -> Result<Data, Error> = retry(
    .operation: get(url),
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
