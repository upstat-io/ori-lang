# Proposal: While Loop Expression

**Status:** Approved
**Approved:** 2026-03-05
**Author:** Eric (with Claude)
**Created:** 2026-03-05

---

## Summary

Add `while condition do body` loop syntax as sugar over `loop { if !condition then break; body }`.

```ori
while self.pos < self.buf.len() do {
    self.pos += 1
}
```

---

## Motivation

### The Problem

Ori's only general-purpose loop is `loop { }` with explicit `break`. Conditional loops require a guard-and-break pattern:

```ori
// Current: verbose conditional loop
loop {
    if self.pos >= self.buf.len() then break;
    if !self.buf[self.pos].is_whitespace() then break;
    self.pos += 1
}
```

This is the most common loop pattern in systems code — lexers, parsers, protocol handlers, stream processors — and Ori makes it unnecessarily verbose.

### Comparison

Every language in the reference set has a while loop or equivalent:

| Language | Syntax |
|----------|--------|
| Rust | `while condition { body }` |
| Go | `for condition { body }` |
| Swift | `while condition { body }` |
| Zig | `while (condition) \|capture\| { body }` |
| Gleam | No while (uses recursion — different design space) |
| Koka | `while { condition } { body }` |

Ori's closest relatives (Rust, Swift, Zig) all have while loops. Gleam omits it but also has tail-call optimization making recursion zero-cost — Ori does not guarantee TCO.

### Real-World Impact

A lexer in Ori without `while`:

```ori
// Skip whitespace
loop {
    if self.pos >= self.buf.len() then break;
    match self.buf[self.pos] {
        ' ' | '\t' | '\n' | '\r' -> self.pos += 1,
        _ -> break,
    }
}

// Read identifier
let start = self.pos;
loop {
    if self.pos >= self.buf.len() then break;
    if !self.buf[self.pos].is_alphanumeric() then break;
    self.pos += 1
}
```

With `while`:

```ori
// Skip whitespace
while self.pos < self.buf.len() do
    match self.buf[self.pos] {
        ' ' | '\t' | '\n' | '\r' -> self.pos += 1,
        _ -> break,
    }

// Read identifier
let start = self.pos;
while self.pos < self.buf.len() && self.buf[self.pos].is_alphanumeric() do
    self.pos += 1
```

---

## Design

### Syntax

```ebnf
while_expr = "while" [ label ] expression "do" expression .
```

The `while` keyword introduces the condition. `do` separates condition from body (consistent with `for...do`).

```ori
while condition do body

while condition do {
    statement1;
    statement2;
}
```

### Semantics

`while condition do body` desugars to:

```ori
loop {
    if !condition then break;
    body
}
```

The desugaring is purely syntactic — no new runtime semantics.

### Type

`while...do` has type `void` (same as `for...do`). It does not produce a value.

`break value` inside a `while` loop is a compile-time error (E0860), just as in `for...do`. Use `loop { }` for value-producing loops.

`continue value` inside a `while` loop is a compile-time error (E0861), just as in `loop`. While loops do not accumulate values.

### Break and Continue

`break` and `continue` (without values) work inside `while` just as they do in `loop`:

```ori
while self.pos < self.buf.len() do {
    if self.buf[self.pos] == '#' then break;   // exit while
    if self.buf[self.pos] == ' ' then {
        self.pos += 1;
        continue                                // next iteration
    };
    process(self.buf[self.pos]);
    self.pos += 1
}
```

### Labels

Labels work on while loops just as on loop and for:

```ori
while:outer scanning do {
    while self.pos < self.buf.len() do {
        if done then break:outer
    }
}
```

### No `while...yield`

There is no `while...yield` form. A `while` loop has no inherent source to map from, making yield semantics unclear. Use `loop` with explicit accumulation or `for` with an iterator for collection building.

---

## Grammar Changes

```ebnf
// Add to expression productions:
while_expr = "while" [ label ] expression "do" expression .
```

**New reserved keyword:** `while` — must be added to the reserved keyword list (7.3.1).

---

## Interaction with Other Features

### Mutable Self (companion proposal)

`while` + mutable self makes imperative loops natural:

```ori
impl Lexer {
    @skip_whitespace (self) -> void = {
        while self.pos < self.buf.len() && self.buf[self.pos].is_whitespace() do
            self.pos += 1
    }
}
```

### Loop Expressions

`while` does not replace `loop`. Use `loop` when:
- You need an infinite loop
- You want to produce a value with `break value`
- The termination condition is complex or mid-body

---

## Error Messages

### Break With Value in While

```
error[E0860]: `break` with value in `while...do`
  --> src/main.ori:5:20
   |
 5 |     if done then break 42,
   |                  ^^^^^^^^ `while...do` does not produce a value
   |
   = help: use `loop { }` to produce a value with `break`
   = help: or remove the value: `break`
```

### Continue With Value in While

```
error[E0861]: `continue` with value in `while`
  --> src/main.ori:5:20
   |
 5 |     if skip then continue 42,
   |                  ^^^^^^^^^^^ `while` does not collect values
   |
   = help: use `break` to exit with a value
   = help: or remove the value: `continue`
```

---

## Migration / Compatibility

- **New keyword:** `while` must become reserved. Any identifier named `while` in existing code becomes invalid.
- **No breaking changes to existing constructs.** `loop` and `for` are unchanged.
- **Gradual adoption:** Existing `loop { if !cond then break; ... }` can be migrated to `while cond do ...` incrementally.

---

## References

- [Spec 16 — Control Flow](../../v2026/spec/16-control-flow.md)
- [Loop Expression Proposal](loop-expression-proposal.md)
- [For-Do Expression Proposal](for-do-expression-proposal.md)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — resolved open questions (no yield, do-only syntax, always void type), fixed grammar to use label production, added error messages for break/continue with value
