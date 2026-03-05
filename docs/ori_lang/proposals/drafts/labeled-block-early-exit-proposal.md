# Proposal: Labeled Block Early Exit

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-03-05

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

### Semantics

- `break:label value` exits the named block and produces `value` as the block's result
- All `break:label` paths and the final expression shall have compatible types
- The block has type equal to the unified type of all exit paths

This is the same as labeled loops, except applied to plain blocks.

### Why `block:name` and Not Just Labels on `{ }`?

Bare `{ }` already means a block or a map literal. Adding a label directly to `{` creates ambiguity. The `block` keyword disambiguates:

```ori
// Clear: this is a labeled block
block:result { ... break:result x ... }

// Ambiguous: is this a labeled map or a labeled block?
// :result { key: value }  -- confusing
```

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
labeled_block = "block" ":" identifier "{" block_body "}" .

// Update break to allow targeting labeled blocks:
// (Already supported — break:label targets any labeled construct)
```

**New context-sensitive keyword:** `block`

---

## Interaction with Other Features

### Try Blocks

`try { }` already provides early exit via `?`. Labeled blocks complement this for non-error early exits.

### Match

Many early-exit patterns can also use `match`. Labeled blocks are for cases where `match` doesn't fit (multiple conditions checked sequentially, with side effects between them).

---

## Migration / Compatibility

- **Non-breaking.** `block` is a new context-sensitive keyword (only meaningful before `:`).
- **Gradual adoption.** Deeply nested `if/else` chains can be refactored to use labeled blocks.

---

## Open Questions

1. **Keyword choice:** `block:name` vs `do:name` vs bare label syntax?
2. **Nesting:** Should labeled blocks be nestable? (Likely yes, for consistency with labeled loops.)
3. **`break` without label in blocks?** Should `break` (no label) exit the innermost labeled block, or remain loop-only? Keeping it loop-only is safer.

---

## References

- [Spec 16.3 — Labeled Loops](../../v2026/spec/16-control-flow.md)
- [Rust RFC 2046 — Label-Break-Value](https://rust-lang.github.io/rfcs/2046-label-break-value.html)
- [Gleam — No early return by design](https://gleam.run/book/tour/functions.html)

---

## Changelog

- 2026-03-05: Initial draft
