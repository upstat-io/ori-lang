# Proposal: Comma-Separated Match Arms

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-20
**Approved:** 2026-02-20

---

## Summary

Change match arm separators from newlines to commas. Trailing commas are optional. This aligns match syntax with Rust, removes newline significance as a special case, and improves grammar consistency with the rest of Ori's explicit-punctuation style.

| Current (newline-separated) | Proposed (comma-separated) |
|-----------------------------|---------------------------|
| `match x { A -> 1 \n B -> 2 }` | `match x { A -> 1, B -> 2 }` |

---

## Motivation

### Inconsistency with Semicolons

The block-expression-syntax proposal introduced semicolons as explicit statement terminators throughout the language. Every construct now uses explicit punctuation — except match arms, which rely on newlines. This creates a "newlines matter here but nowhere else" special case.

```ori
// Semicolons terminate statements everywhere...
let x = 5;
let y = compute(x: x);
print(msg: y);

// ...but match arms are magically newline-separated?
match status {
    Pending -> "waiting"
    Running(p) -> str(p)
    _ -> "other"
}
```

This inconsistency must be learned and remembered. A comma after each arm makes the grammar uniform: blocks use `;`, match arms use `,`.

### Newline Significance Is Fragile

When newlines are significant, formatting changes can change semantics. Consider a one-line match that gets reformatted:

```ori
// Does this parse as two arms or one expression?
match x { Some(v) -> v None -> 0 }
```

With commas, the intent is unambiguous:

```ori
match x { Some(v) -> v, None -> 0 }
```

### Better Error Recovery

A missing comma is a specific, actionable parse error:

```
error[E1001]: expected `,` after match arm
 --> src/main.ori:5:25
  |
5 |     Some(v) -> v
  |                 ^ expected `,` here
```

A missing newline is invisible and produces confusing errors about unexpected tokens on the next line.

### One-Line Matches Become Natural

Short matches can stay on one line without ambiguity:

```ori
let sign = match n { x if x > 0 -> 1, x if x < 0 -> -1, _ -> 0 };
let label = match b { true -> "yes", false -> "no" };
```

Without commas, these require newlines even when the match is trivially short.

---

## Design

### Grammar Change

**Before:**

```ebnf
match_expr = "match" expression "{" match_arms "}" .
match_arms = { match_arm NEWLINE } [ match_arm ] .
match_arm  = match_pattern [ guard ] "->" expression .
```

**After:**

```ebnf
match_expr = "match" expression "{" match_arms "}" .
match_arms = [ match_arm { "," match_arm } [ "," ] ] .
match_arm  = match_pattern [ guard ] "->" expression .
```

The `[ "," ]` at the end allows an optional trailing comma on the last arm.

### Trailing Comma Rules

Trailing commas are **optional** — allowed but never required. This matches Ori's existing rules for function arguments, list literals, struct fields, and map entries.

```ori
// Both valid:
match color {
    Red -> "#ff0000",
    Green -> "#00ff00",
    Blue -> "#0000ff",   // trailing comma OK
}

match color {
    Red -> "#ff0000",
    Green -> "#00ff00",
    Blue -> "#0000ff"    // no trailing comma also OK
}
```

The formatter will emit trailing commas in multi-line matches and omit them in single-line matches.

### Block-Bodied Arms

Arms with block bodies use `},` — the block's closing brace followed by the arm comma:

```ori
match expr {
    Lit(value) -> Ok(value),
    Var(name) -> env[name].ok_or(error: "undefined: " + name),
    BinOp(op, left, right) -> {
        let $l = eval(expr: left, env:)?;
        let $r = eval(expr: right, env:)?;
        Ok(l + r)
    },
    _ -> Err("unsupported"),
}
```

This matches Rust's syntax exactly. The `},` pattern is visually heavier than a bare `}`, but it is unambiguous and already familiar to Rust developers.

### Guard Syntax: `if` Replaces `.match()`

This proposal formally adopts `if` guards for match arms, replacing the `.match(condition)` syntax. The block-expression-syntax proposal's grammar section already specified `match_arm = pattern [ "if" expr ] "->" expr .`, but the grammar.ebnf was not synced. This proposal completes that change.

**Before:**

```ebnf
match_arm  = match_pattern [ guard ] "->" expression .
guard      = ".match" "(" expression ")" .
```

**After:**

```ebnf
match_arm  = match_pattern [ "if" expression ] "->" expression .
```

**Before:**

```ori
match n {
    x.match(x > 0) -> "positive"
    x.match(x < 0) -> "negative"
    _ -> "zero"
}
```

**After:**

```ori
match n {
    x if x > 0 -> "positive",
    x if x < 0 -> "negative",
    _ -> "zero",
}
```

The `if` guard syntax:
- Is more familiar to Rust, Python, and Haskell developers
- Reads naturally as "x if x > 0"
- Resolves the ambiguity where `.match()` was overloaded: `.match(cond)` as guard vs `.match(arms)` as method-style match. With `if` guards, `.match()` exclusively means method-style pattern matching.

### Method-Style Match

The `.match()` method syntax already uses commas (it's a function call). This proposal makes standalone `match expr { }` consistent with `.match()`:

```ori
// Method-style (already comma-separated)
value.match(
    Some(x) -> x,
    None -> 0,
)

// Standalone (proposed: also comma-separated)
match value {
    Some(x) -> x,
    None -> 0,
}
```

### Formatter Rules

The formatter uses single-line format when all of:
1. Total width fits within the line limit
2. All arms are simple expressions (no block bodies)
3. There are no guards

Otherwise, one arm per line with trailing commas. Single-line matches omit the trailing comma.

```ori
// Single-line (short, no guards, no blocks)
let label = match b { true -> "yes", false -> "no" };

// Multi-line (guards present)
let sign = match n {
    x if x > 0 -> 1,
    x if x < 0 -> -1,
    _ -> 0,
};

// Multi-line (block body)
match expr {
    Lit(v) -> Ok(v),
    BinOp(op, l, r) -> {
        let $a = eval(expr: l)?;
        let $b = eval(expr: r)?;

        apply(op: op, left: a, right: b)
    },
};
```

---

## Examples

### Simple Match

```ori
@describe (shape: Shape) -> str = match shape {
    Circle(r) -> "circle with radius " + str(r),
    Rectangle(w, h) -> "rectangle " + str(w) + "x" + str(h),
    Triangle(a, b, c) -> "triangle",
};
```

### Nested Match

```ori
@process (result: Result<Option<int>, str>) -> str = match result {
    Ok(Some(n)) -> "got " + str(n),
    Ok(None) -> "empty",
    Err(msg) -> "error: " + msg,
};
```

### Match with Guards

```ori
@classify (n: int) -> str = match n {
    0 -> "zero",
    x if x > 0 -> "positive",
    _ -> "negative",
};
```

### Match with Block Arms

```ori
@eval (expr: Expr, env: Env) -> Result<Value, Error> = match expr {
    Lit(v) -> Ok(v),
    BinOp(op, left, right) -> {
        let $l = eval(expr: left, env:)?;
        let $r = eval(expr: right, env:)?;
        apply_op(op: op, left: l, right: r)
    },
    Let(name, value, body) -> {
        let $v = eval(expr: value, env:)?;
        let $new_env = env.insert(key: name, value: v);
        eval(expr: body, env: new_env)
    },
};
```

### One-Line Match

```ori
let abs = match n { x if x < 0 -> -x, _ -> n };
let label = match enabled { true -> "on", false -> "off" };
```

### At-Patterns

```ori
@process (s: Status) -> str = match s {
    status @ Failed(_) -> {
        log_failure(status: status);
        "failed"
    },
    _ -> "ok",
};
```

---

## Prior Art

| Language | Match Arm Separator | Trailing? |
|----------|-------------------|-----------|
| **Rust** | `,` (comma) | Optional |
| **Zig** | `,` (comma) | Required |
| **Go** (`switch`) | Newline | N/A |
| **Swift** (`switch`) | Newline | N/A |
| **Kotlin** (`when`) | Newline | N/A |
| **Scala** (`match`) | Newline | N/A |
| **Gleam** (`case`) | Newline | N/A |
| **Elm** (`case`) | Newline | N/A |
| **OCaml** (`match`) | `|` prefix | N/A |

Ori's syntax is closest to Rust. Adding commas makes the match syntax identical to Rust's, which is the most widely-adopted expression-based match syntax in a systems language.

---

## Impact

### Parser

`parse_match_arms_with_scrutinee()` in `compiler/ori_parse/src/grammar/expr/patterns.rs` already uses `paren_series_direct()` which parses comma-separated items. The new `match expr { }` parser will use the same comma-separated logic, just with `{`/`}` delimiters instead of `(`/`)`.

### Formatter

The formatter needs to decide when to use single-line vs multi-line match format. Rule: if total width fits in line width and all arms are simple expressions, use single-line with commas. Otherwise, one arm per line with trailing commas.

### Existing Code

All existing `.ori` code uses the old `match(expr, arm, ...)` syntax which already uses commas. The migration to `match expr { }` hasn't happened yet (parser not implemented). This proposal should be applied simultaneously with the `match expr { }` parser implementation.

### Documentation

All docs updated in the `block-syntax-semicolons` branch currently show newline-separated arms. If this proposal is approved, all match examples need commas added. This is a mechanical find-and-replace.

---

## Alternatives Considered

### Newline-Separated (Status Quo)

Keep the current spec. Pros: less punctuation, cleaner visual. Cons: inconsistent with semicolons everywhere else, fragile formatting, no one-liners, significant-newline special case.

### Semicolons Instead of Commas

Use `;` to separate match arms (same as block statements). Rejected because match arms are not statements — they are alternatives in a pattern match. Commas signal "one of these" while semicolons signal "then this". The semantic distinction matters.

### Required Trailing Comma (Zig-Style)

Always require the trailing comma, even on the last arm. Rejected as unnecessarily strict. Optional trailing commas reduce friction in single-line matches while allowing them in multi-line matches for diff-friendly editing.
