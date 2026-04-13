# Proposal: Optional Semicolon After Block-Ending Expression Statements

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-04-13
**Affects:** Compiler (parser), spec (Clause 11), grammar (Annex A)
**Amends:** block-expression-syntax.md

---

## Summary

Make the trailing `;` optional on expression statements whose last token is `}`. This aligns the grammar (`grammar.ebnf` line 391) with the documented rule in `ori-syntax.md` ("Ends with `}`? No `;`") and resolves a parser behavior that contradicts the language's own documentation. The change is strictly about statement-level semicolons for block-ending expressions — it does not affect the block-value rule (last expression without `;` is the block's value).

---

## Motivation

### The Mismatch

The Ori syntax quick reference (`.claude/rules/ori-syntax.md`) states:

> **Semicolon rule:** Ends with `}`? No `;`. Everything else: `;`.

However, `grammar.ebnf` line 391 defines:

```ebnf
statement = ( let_expr | assignment | expression ) ";" .
```

This requires `;` unconditionally on ALL expression statements, including those ending with `}`. The parser enforces the grammar, not the documented rule.

### The Problem in Practice

```ori
@simulate () -> [int] = {
    let doors = for _ in 0..100 yield false;

    // This for...do block ends with } — the documented rule says no ; needed
    // But the parser REQUIRES ; here, contradicting the documented rule
    for pass in 1..=100 do {
        for idx in (pass - 1)..100 by pass do {
            doors[idx] = !doors[idx]
        }
    }   // <-- ERROR: expected `;` or `}` after expression in block

    for i in 0..100 if doors[i] yield i + 1
}
```

The user must write `};` after the `for...do` block — adding a semicolon after a closing brace that the documented rule says needs no semicolon.

### When This Matters

- **Every multi-statement function** that mixes block-ending expressions (`for...do`, `while...do`, `loop`, `if...then...else`) with subsequent statements
- **New users** who read the documented semicolon rule and write code that the parser rejects
- **The Rosetta Code stress test** — nearly every program hits this mismatch when a `for...do` loop precedes another expression

---

## Design

### Grammar Change

Replace the unconditional `;` requirement on `statement` with a conditional rule:

```ebnf
// Current (line 391):
statement = ( let_expr | assignment | expression ) ";" .

// Proposed:
statement = let_expr ";"
          | assignment ";"
          | block_ending_expression [ ";" ]
          | expression ";" .
```

Where `block_ending_expression` matches any expression whose last token is `}`:

```ebnf
block_ending_expression = for_do_expr
                        | while_do_expr
                        | loop_expr
                        | if_else_expr      // only when else branch ends with }
                        | match_expr
                        | unsafe_expr
                        | labeled_block
                        | block_expr .
```

### Semantics

- **No semantic change.** The `;` after a `}`-ending expression is already a statement terminator, not a value suppressor. Making it optional changes parsing only — the expression is still treated as a void statement, not a block value.
- **`;` remains valid.** Writing `};` is still accepted — the semicolon is optional, not forbidden. This avoids breaking existing code.
- **The block-value rule is unchanged.** The last expression without `;` in a block is still the block's value. This proposal only affects non-final expression statements.

### Parser Implementation

In `compiler/ori_parse/src/grammar/expr/blocks.rs` (or the equivalent statement-parsing function), after parsing an expression:

1. Check if the expression ends with `}`
2. If yes: consume `;` if present, but do not require it
3. If no: require `;` as before

This is the same strategy Rust uses in its parser (`rustc_parse/src/parser/stmt.rs`).

### Examples

```ori
// All of these are valid under this proposal:

@main () -> void = {
    // for...do without ; — now valid (was error)
    for i in 0..10 do {
        print(msg: `{i}`)
    }

    // for...do with ; — still valid
    for i in 0..10 do {
        print(msg: `{i}`)
    };

    // if...then...else without ; — now valid
    if condition then {
        do_a()
    } else {
        do_b()
    }

    // match without ; — now valid
    match value {
        Some(x) -> handle(x:),
        None -> default(),
    }

    // Non-block expressions still require ;
    let $x = compute();
    print(msg: `done`)
}
```

### Error Handling

No new error codes. The existing `E1002` ("expected `;` or `}` after expression in block") still fires for non-block expressions missing `;`. The only change is that block-ending expressions no longer trigger it.

---

## Alternatives Considered

### Alternative 1: Require `;` everywhere (update documentation)

Fix the mismatch by changing the documented rule to match the grammar — require `;` after everything.

**Rejected:** This is the opposite of what users expect. Rust, Zig, Kotlin, Swift, and Go all treat `}`-ending constructs as self-terminating. Requiring `};` is unusual and feels wrong to developers from any of these backgrounds. The documentation was right; the grammar was wrong.

### Alternative 2: Newline-based statement separation

Use newlines as statement separators (like Go or Swift) instead of semicolons.

**Rejected:** This is a much larger language change with ambiguity in multi-line expressions. The semicolon model works well for Ori — this proposal fixes a specific inconsistency, not the overall model.

### Alternative 3: Always require `;` including on last expression

Remove the "last expression without `;` is the block value" rule entirely — every statement including the value-producing one gets `;`.

**Rejected:** This would break the expression-based design. The block-value rule is fundamental to Ori's identity and works well for sub-blocks (`if`, `match`, `let = { }`).

---

## Purity Analysis

**Can be pure Ori?** NO
**If not, why:** This is a parser grammar change — requires modifying the compiler's statement parsing logic.
**Recommendation:** Proceed as compiler change.

---

## Spec & Grammar Impact

| File | Change |
|------|--------|
| `grammar.ebnf` line 391 | Split `statement` production to make `;` optional for block-ending expressions |
| `docs/ori_lang/v2026/spec/11-blocks-and-scope.md` | Clarify the semicolon rule — `;` is optional after `}`-ending expression statements |
| `.claude/rules/ori-syntax.md` | Already correct ("Ends with `}`? No `;`") — no change needed |

---

## Prior Art

### Rust

Rust's parser makes `;` optional after expressions ending with `}`. The grammar distinguishes "expression statements" from "expression-with-block statements" — the latter don't need `;`. This is the direct precedent for this proposal. See `rustc_parse/src/parser/stmt.rs`.

### Zig

Zig had the exact same debate across multiple issues:
- **zig#1677** — "Syntax flaw: Block statements and terminating semicolon" — noted that block expressions (`for`, `if`, `while`) are only terminated with `;` if the ending block is not a block, making the grammar hard to formalize as context-free.
- **zig#629** — "explicitly return from blocks instead of last statement being expression value" — explored labeled blocks with explicit return as an alternative to the semicolon sensitivity problem.
- **zig#8856** — "Grammar change - require semicolons at the end of every statement" — proposed the opposite direction (always require `;`), but the discussion highlighted the same confusion this proposal addresses.

### Kotlin / Swift / Go

All three languages treat `}`-ending constructs as self-terminating — no trailing `;` required. This is the universal expectation among developers.
