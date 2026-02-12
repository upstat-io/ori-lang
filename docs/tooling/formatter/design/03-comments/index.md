---
title: "Comments"
description: "Ori Formatter Design — Comment Formatting"
order: 1
section: "Comments"
---

# Comments

Formatting rules for regular comments and doc comments.

## Regular Comments

### Own Line Only

Comments must appear on their own line. Inline comments are prohibited:

```ori
// Valid - comment on its own line
let x = 42

// Invalid - inline comment
let y = 42  // this is a syntax error
```

### Space After //

A space is required after `//`:

```ori
// Correct
//Wrong (formatter adds space)
```

The formatter normalizes `//Wrong` to `// Wrong`.

### Comment Preservation

The formatter:
- **Preserves** comment content (does not reflow text)
- **Normalizes** spacing (adds space after `//`)
- **Maintains** position relative to code

### Comment Placement

Comments associate with the code that follows them:

```ori
// This comment describes the function
@add (a: int, b: int) -> int = a + b

// This comment describes the type
type Point = { x: int, y: int }
```

### Multiple Comment Lines

```ori
// This is a longer comment that spans
// multiple lines. Each line starts with //
// and a space.
@complex_function () -> int = do_work()
```

## Doc Comments

Doc comments use special markers after `//`. The formatter enforces specific rules.

### Markers

| Marker | Purpose | Example |
|--------|---------|---------|
| `#` | Description | `// #Computes the sum.` |
| `* name:` | Member docs (params/fields) | `// * n: Must be positive.` |
| `!` | Warning/panic | `// !Panics if n is negative.` |
| `>` | Example | `// >add(a: 2, b: 3) -> 5` |

> **Legacy markers**: `@param` and `@field` are still recognized by the lexer and classified as `DocMember`, but `* name:` is the canonical form.

### Spacing Rules

Space after `//`, no space after marker:

```ori
// #Correct format
// # Wrong (space after #)
//#Wrong (no space after //)
```

The formatter normalizes all variations:

| Input | Output |
|-------|--------|
| `//# Description` | `// #Description` |
| `// # Description` | `// #Description` |
| `//#Description` | `// #Description` |
| `// #Description` | `// #Description` (no change) |

### Required Order

Doc comments must appear in this order:

| Order | Marker | Content |
|-------|--------|---------|
| 1 | `#` | Description |
| 2 | `* name:` | Members (parameters or fields) |
| 3 | `!` | Warnings and panic conditions |
| 4 | `>` | Examples |

The formatter **reorders** doc comments if they're out of order:

```ori
// Input (wrong order)
// >example()
// #Description

// Output (corrected)
// #Description
// >example()
```

### Member Order

`DocMember` entries (`* name:`) must match the order of parameters in the function signature or fields in the struct:

```ori
// Input (wrong order)
// * b: The second operand.
// * a: The first operand.
@add (a: int, b: int) -> int = a + b

// Output (corrected to match signature)
// * a: The first operand.
// * b: The second operand.
@add (a: int, b: int) -> int = a + b
```

### Struct Member Order

```ori
// Input (wrong order)
// * y: The vertical coordinate.
// * x: The horizontal coordinate.
type Point = { x: int, y: int }

// Output (corrected to match struct)
// * x: The horizontal coordinate.
// * y: The vertical coordinate.
type Point = { x: int, y: int }
```

### Complete Example

```ori
// #Computes the factorial of n.
// #Returns 1 for n <= 1.
// * n: The number to compute factorial of. Must be non-negative.
// !Panics if n is negative.
// >factorial(n: 5) -> 120
// >factorial(n: 0) -> 1
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)
```

### Struct Doc Comments

```ori
// #Represents a 2D point in Cartesian coordinates.
// * x: The horizontal coordinate.
// * y: The vertical coordinate.
type Point = { x: int, y: int }
```

### Multi-Line Descriptions

Each continuation line repeats the marker:

```ori
// #Fetches user data from the remote API.
// #Returns cached data if available and fresh.
// #Falls back to default user on error.
// * id: The user ID to fetch.
// * use_cache: Whether to check cache first.
@fetch_user (id: int, use_cache: bool) -> User = do_fetch()
```

## Comment Blocks

### Before Imports

```ori
// Configuration for the user service module
use std.collections { HashMap }
use std.time { Duration }
```

### Before Sections

```ori
use std.math { sqrt }

// --- Helper Functions ---

@helper () -> int = 42

// --- Main API ---

@public_api () -> str = "result"
```

### Inside Functions

Comments inside function bodies follow the same rules:

```ori
@process (data: Data) -> Result<Output, Error> = run(
    // Validate input first
    let validated = validate(data),

    // Transform to intermediate format
    let intermediate = transform(validated),

    // Produce final output
    finalize(intermediate),
)
```

## What the Formatter Does NOT Do

The formatter does **not**:

- Reflow long comment text
- Add or remove comments
- Change comment wording
- Move comments to different locations (except reordering doc comment markers)
- Add doc comments for undocumented items

The formatter **only**:

- Normalizes `// ` spacing
- Reorders doc comment markers to correct order
- Reorders `DocMember` (`* name:`) entries to match signature/struct field order
