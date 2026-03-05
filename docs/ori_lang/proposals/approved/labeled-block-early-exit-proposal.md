# Proposal: Labeled Block Early Exit

**Status:** Approved
**Author:** Eric (with Claude)
**Created:** 2026-03-05
**Approved:** 2026-03-05

---

## Summary

Allow `break:label value` to exit named blocks early with a value, providing early-exit ergonomics without adding a `return` keyword.

```ori
@classify (ch: byte) -> TokenKind = block:result {
    if ch == b'(' then break:result TokenKind.LParen;
    if ch == b')' then break:result TokenKind.RParen;
    if ch.is_digit() then break:result TokenKind.Number;
    if ch.is_alpha() then break:result TokenKind.Ident;

    TokenKind.Unknown
}
```

---

## Motivation

### The Design Constraint

Ori deliberately has no `return` keyword. The last expression in a block is its value. This is a design pillar:

> **NO `return`**: last expression = block value. Exit via `?`/`break`/`panic`. Similar to Rust, Gleam, Roc.

This works beautifully for simple functions. But for functions with many early exits — lexers, parsers, validators, dispatch tables — the absence of early return creates deeply nested code.

### The Problem

A lexer character classifier without early exit:

```ori
@classify (ch: byte) -> TokenKind =
    if ch == b'(' then TokenKind.LParen
    else if ch == b')' then TokenKind.RParen
    else if ch == b'{' then TokenKind.LBrace
    else if ch == b'}' then TokenKind.RBrace
    else if ch.is_digit() then TokenKind.Number
    else if ch.is_alpha() then TokenKind.Ident
    else if ch.is_whitespace() then TokenKind.Whitespace
    else TokenKind.Unknown
```

This is acceptable for short chains, but becomes unwieldy with 20+ cases, especially when each branch has setup logic:

```ori
@next_token (self) -> Token =
    if self.buf[self.pos] == b'"' then {
        let start = self.pos;
        self.advance();
        // ... 10 lines to handle string ...
        Token { kind: TokenKind.String, span: Span { start, end: self.pos } }
    } else if self.buf[self.pos].is_digit() then {
        let start = self.pos;
        // ... 10 lines to handle number ...
        Token { kind: TokenKind.Number, span: Span { start, end: self.pos } }
    } else {
        // ... every branch indented deeper ...
    }
```

### The Solution: Labeled Blocks

Rust solved this same problem with labeled blocks (RFC 2046, stabilized in Rust 1.65):

```rust
let result = 'block: {
    if condition { break 'block value1; }
    if condition { break 'block value2; }
    default_value
};
```

Ori already has labeled loops (`loop:name`, `for:name`). Extending labels to blocks is a natural, minimal addition.

---

## Design

### Syntax

```ebnf
labeled_block = "block" ":" identifier "{" block_body "}" .
```

A labeled block uses the `block:name` syntax followed by a block body:

```ori
let x = block:done {
    if condition1 then break:done value1;
    if condition2 then break:done value2;
    default_value
}
```

### Keyword

`block` is a **context-sensitive keyword** — it is only recognized as a keyword when immediately followed by `:`. Outside that position, `block` is a valid identifier:

```ori
let block = 5;           // OK: `block` as variable name
let x = block:done { 1 } // OK: `block:done` is a labeled block
```

This is consistent with other context-sensitive keywords like `handler`, `try`, `from`, etc.

### Semantics

- `break:label value` exits the named block and produces `value` as the block's result
- All `break:label` paths and the final expression shall have compatible types
- The block has type equal to the unified type of all exit paths
- Labeled blocks are nestable — inner blocks can be exited independently of outer blocks
- The no-shadowing rule applies: block labels share the label namespace with loop labels

### Unlabeled `break` is Loop-Only

Bare `break` (without a label) always targets the innermost **loop**, not the innermost labeled block. Inside a labeled block, `break` without a label passes through the block and targets an enclosing loop:

```ori
loop {
    let x = block:result {
        if done then break;  // Exits the LOOP, not the block
        if found then break:result value;
        default
    };
    process(x)
}
```

This prevents confusion when a labeled block is added around existing loop code. It is consistent with Rust's labeled block behavior.

### `continue:block_label` is an Error

`continue:label` targeting a labeled block is a compile-time error. Blocks do not iterate — there is no "next iteration" to skip to:

```ori
block:result {
    continue:result;  // ERROR: cannot `continue` a labeled block
}
```

`continue:loop_label` *through* a labeled block is valid — see "Loop Control Flow" below.

### Loop Control Flow Transparency

Labeled blocks are transparent to loop control flow. `break:loop_label` and `continue:loop_label` pass through labeled blocks, just like `try { }`:

```ori
loop:outer {
    let x = block:inner {
        if skip then continue:outer;    // OK: passes through block, continues loop
        if done then break:outer;       // OK: passes through block, exits loop
        if found then break:inner 42;   // OK: exits block with value
        0
    };
    process(x)
}
```

### Why `block:name` and Not Just Labels on `{ }`?

Bare `{ }` already means a block or a map literal. Adding a label directly to `{` creates ambiguity. The `block` keyword disambiguates:

```ori
// Clear: this is a labeled block
block:result { ... break:result x ... }

// Ambiguous: is this a labeled map or a labeled block?
// :result { key: value }  -- confusing
```

The `block:name` syntax also follows the established `keyword:label` pattern: `loop:name`, `for:name`, `while:name`, `block:name`.

### Desugaring (Conceptual)

A labeled block is equivalent to a `loop` that always breaks on first iteration:

```ori
block:name { body }
// Equivalent to:
loop:name { break:name { body } }
```

But labeled blocks are first-class — no loop overhead or confusion.

### Examples

#### Lexer Classification

```ori
@next_token (self) -> Token = block:emit {
    let start = self.pos;

    match self.buf[self.pos] {
        b'"' -> {
            self.read_string();
            break:emit Token.string(start:, end: self.pos)
        },
        b'(' -> {
            self.advance();
            break:emit Token.lparen(start:)
        },
        ch if ch.is_digit() -> {
            self.read_number();
            break:emit Token.number(start:, end: self.pos)
        },
        _ -> (),
    };

    // Fallthrough: try identifier
    if self.buf[self.pos].is_alpha() then {
        self.read_identifier();
        break:emit Token.ident(start:, end: self.pos)
    };

    Token.unknown(start:)
}
```

#### Validation with Early Exit

```ori
@validate (input: Request) -> Result<ValidRequest, Error> = block:done {
    if input.name.is_empty() then
        break:done Err(Error { message: "name required" });

    if input.age < 0 then
        break:done Err(Error { message: "age must be non-negative" });

    if input.email.is_empty() then
        break:done Err(Error { message: "email required" });

    Ok(ValidRequest { name: input.name, age: input.age, email: input.email })
}
```

#### Nested with Loops

Labeled blocks compose with labeled loops:

```ori
let result = block:outer {
    for item in items do {
        if item.matches() then break:outer item;
    }

    default_item
}
```

---

## Why Not `return`?

| Approach | Pros | Cons |
|----------|------|------|
| `return` keyword | Familiar, universal | Non-local control flow; breaks expression-based model; function body is no longer "just an expression" |
| Labeled blocks | Local, explicit scope; composable; consistent with existing labels | New `block` keyword; slightly more verbose than `return` |

Labeled blocks are **scoped** — you see exactly where the exit targets. `return` is **implicit** — it always targets the enclosing function, which can be far away. In an expression-based language, scoped exits are the right tool.

---

## Grammar Changes

```ebnf
// New production:
labeled_block = "block" label block_expr .

// Update expression to include labeled_block:
// expression = ... | labeled_block | ...

// break already supports labels — no change needed:
// break_expr = "break" [ label ] [ expression ] .
```

**New context-sensitive keyword:** `block` (only meaningful before `:`)

---

## Spec Changes: Break/Continue Summary (16.9)

The following rows shall be added to the break/continue summary table:

| Form | Valid in | Effect |
|------|---------|--------|
| `break:label` | Labeled block | Exit labeled block |
| `break:label value` | Labeled block | Exit labeled block with value |
| `continue:label` targeting block | — | **Error**: blocks do not iterate |

---

## Interaction with Other Features

### Try Blocks

`try { }` already provides early exit via `?`. Labeled blocks complement this for non-error early exits.

### Match

Many early-exit patterns can also use `match`. Labeled blocks are for cases where `match` doesn't fit (multiple conditions checked sequentially, with side effects between them).

### Loop Control Flow

Labeled blocks are transparent to `break` and `continue` targeting outer loops. This matches the behavior of `try { }`:

```ori
for:search items in collection do {
    let result = block:check {
        if invalid(items) then continue:search;   // OK: continue outer for loop
        if found(items) then break:search items;   // OK: break outer for loop
        transform(items)
    };
    process(result)
}
```

---

## Decisions

1. **Keyword choice:** `block:name` — follows the established `keyword:label` pattern.
2. **Nesting:** Yes — labeled blocks are nestable, with the same no-shadowing rule as loops.
3. **`break` without label in blocks:** Loop-only — bare `break` always targets the innermost loop, never a labeled block. You must use `break:name` to exit a block.

---

## Migration / Compatibility

- **Non-breaking.** `block` is a new context-sensitive keyword (only meaningful before `:`).
- **Gradual adoption.** Deeply nested `if/else` chains can be refactored to use labeled blocks.

---

## References

- [Spec 16.3 — Labeled Loops](../../v2026/spec/16-control-flow.md)
- [Rust RFC 2046 — Label-Break-Value](https://rust-lang.github.io/rfcs/2046-label-break-value.html)
- [Gleam — No early return by design](https://gleam.run/book/tour/functions.html)

---

## Changelog

- 2026-03-05: Initial draft
- 2026-03-05: Approved — resolved open questions, added continue transparency, continue:block error, spec table updates
