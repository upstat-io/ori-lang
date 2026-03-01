---
title: "Token Spacing"
description: "Ori Compiler Design — Formatter Spacing Rules"
order: 1201
section: "Formatter"
---

# Token Spacing (Layer 1)

The spacing layer determines what whitespace to place between adjacent tokens. It is
the lowest layer of the formatter architecture: a purely declarative, O(1) lookup from
token pairs to spacing actions. Higher layers (packing, shape, rules, orchestration)
never override spacing decisions -- they control line breaking and indentation, while
this layer controls horizontal spacing within a line.

## Architecture

The spacing system has four components:

1. **`SpaceAction`** -- the output: what spacing to emit between two tokens.
2. **`TokenCategory`** -- the input: abstract token types that ignore literal values.
3. **`TokenMatcher`** -- flexible matching for rule definitions.
4. **`RulesMap`** -- pre-computed O(1) lookup table.

### SpaceAction

The `SpaceAction` enum has four variants:

| Variant | Meaning | Example |
|---------|---------|---------|
| `None` | No space between tokens | `foo()`, `list[0]`, `@name` |
| `Space` | Single space | `a + b`, `x: int`, `if cond` |
| `Newline` | Line break (rarely used at this layer) | -- |
| `Preserve` | Keep source spacing | -- |

The default action is `None` -- rules explicitly add spaces where needed.

### TokenCategory

The `TokenCategory` enum abstracts `ori_ir::TokenKind` into categories suitable for
spacing decisions. It strips literal values (an `Int(42)` token becomes `TokenCategory::Int`)
and groups some keywords into the `Ident` category (e.g., `async`, `extern`, `unsafe`).

Categories include:
- **Literals**: `Int`, `Float`, `String`, `Char`, `Duration`, `Size`.
- **Identifiers**: `Ident` (covers identifiers and some context-sensitive keywords).
- **Keywords**: `Break`, `Continue`, `For`, `If`, `Let`, `Match`, `Pub`, `Type`, `Trait`, etc.
- **Type keywords**: `IntType`, `FloatType`, `BoolType`, `StrType`, etc.
- **Wrappers**: `Ok`, `Err`, `Some`, `None`.
- **Compiler constructs**: `Cache`, `Catch`, `Parallel`, `Recurse`, `Run`, `Try`, etc.
- **Delimiters**: `LParen`, `RParen`, `LBrace`, `RBrace`, `LBracket`, `RBracket`.
- **Punctuation**: `At`, `Dollar`, `Hash`, `Colon`, `Comma`, `Dot`, `Arrow`, `Semicolon`, etc.
- **Operators**: `Plus`, `Minus`, `Star`, `EqEq`, `AmpAmp`, `PipePipe`, `CompoundAssign`, etc.

The `From<&TokenKind>` implementation maps every `TokenKind` variant to a `TokenCategory`.
Predicate methods (`is_binary_op()`, `is_unary_op()`, `is_open_delim()`, `is_close_delim()`,
`is_literal()`, `is_keyword()`) support category-level matching in rules.

### TokenMatcher

The `TokenMatcher` enum provides flexible matching for rule definitions:

- `Any` -- matches any token category.
- `Exact(cat)` -- matches a specific category.
- `OneOf(&[cat])` -- matches any category in a static slice.
- `Category(fn)` -- matches via a predicate function (e.g., `is_binary_op`).

Pre-defined matcher constants: `BINARY_OP`, `UNARY_OP`, `OPEN_DELIM`, `CLOSE_DELIM`,
`LITERAL`, `KEYWORD`.

## Rule Definitions

All spacing rules are defined as static entries in the `SPACE_RULES` array in
`spacing/rules.rs`. Each rule is a `SpaceRule` struct with five fields:

```rust
pub struct SpaceRule {
    pub name: &'static str,      // Human-readable name for debugging
    pub left: TokenMatcher,       // Matcher for left (preceding) token
    pub right: TokenMatcher,      // Matcher for right (following) token
    pub action: SpaceAction,      // The spacing to apply
    pub priority: u8,             // Lower = higher priority
}
```

Rules are evaluated by priority (lower number = checked first), then by definition order
within the same priority level. The first matching rule determines the action.

### Priority Bands

| Priority | Category | Examples |
|----------|----------|---------|
| 10 | Empty delimiters (most specific) | `()`, `[]`, `{}` -- no space inside |
| 20 | Delimiter adjacency | After `(`, `[` -- no space; before `)`, `]`, `}` -- no space |
| 25 | Field access and double colon | `x.y`, `Mod::item` -- no space around `.` or `::` |
| 30 | Punctuation | `,` -- space after; `:` -- space after; `;` -- space after; `?` -- no space before; `..`/`..=` -- no space |
| 35 | Prefix sigils | `@foo`, `$name`, `#derive`, `#[...]`, `#!...` -- no space between sigil and content |
| 40 | Binary and assignment operators | `a + b`, `x = 1`, `x += 1`, `->`, `=>`, `??` -- space around |
| 45 | Unary operators | `!x`, `~z` -- no space after; `-` before literal -- no space |
| 50 | Keyword spacing | `pub `, `let `, `if `, `for `, `where `, `as `, etc. -- space after |
| 55 | Construct-paren adjacency | `run(`, `try(`, `match(`, `Ok(`, `print(` -- no space between keyword and `(` |
| 60 | Sum type pipe | `A \| B` -- space around `\|` |
| 70 | Generic bounds | `T: A + B` -- space around `+` |
| 90 | Default fallback | `(Any, Any)` -- no space |

### Context-Sensitive Spacing

Some spacing decisions are inherently context-sensitive. The priority system resolves
most ambiguities statically:

- **Minus as unary vs binary**: The binary operator rule (priority 40) adds space around
  `-`. The unary minus rule (priority 45, lower priority) removes space after `-` when
  followed by a literal. Since the `RulesMap` processes exact rules first by priority,
  the context where `-` appears determines which rule wins.

- **Pipe in sum types vs expressions**: The pipe rule at priority 60 adds space around
  `|`, which applies to both sum type variants (`A | B`) and bitwise OR. The binary
  operator rule at priority 40 also matches `|` (via `is_binary_op()`), but exact rules
  take precedence in the lookup table.

- **Construct keywords before parens**: Keywords like `run`, `try`, and `match` normally
  get space after (priority 50), but when followed by `(`, the priority 55 rule removes
  the space: `run(...)`, not `run (...)`. This also applies to wrapper types (`Ok(`,
  `Err(`, `Some(`) and built-in functions (`print(`, `panic(`, `todo(`).

## Lookup Table Design

The `RulesMap` struct pre-computes an `FxHashMap` from `(TokenCategory, TokenCategory)`
pairs to `SpaceAction` values. Construction works as follows:

1. Sort all rules by priority.
2. For each rule, check its matchers:
   - `Exact` x `Exact`: insert directly into the hash map (first insertion wins).
   - `Exact` x `OneOf` or `OneOf` x `Exact`: expand into individual exact entries.
   - `OneOf` x `OneOf`: expand the cartesian product into exact entries.
   - Complex matchers (`Any`, `Category`): store in a fallback list for linear scan.
3. At lookup time, check the hash map first (O(1)). If no exact match, scan the
   fallback list linearly (typically 3-5 rules including the default).

A global singleton (`GLOBAL_RULES_MAP`, initialized via `OnceLock`) avoids rebuilding
the table on every formatting operation. The primary API is:

```rust
pub fn lookup_spacing(left: TokenCategory, right: TokenCategory) -> SpaceAction
```

## Integration with FormatContext

The `FormatContext` (in `context/mod.rs`) tracks the last emitted `TokenCategory` and
provides two methods for Layer 1 integration:

- `spacing_for(next_token)` -- looks up the spacing action between the last emitted
  token and `next_token`. Returns `None` if no previous token was recorded (e.g., at
  line start).

- `emit_token(category, text)` -- the full pipeline: checks spacing, emits a space or
  newline if needed, emits the token text, and updates the last-token state.

After a newline, `clear_last_token()` is called so spacing rules do not apply at the
start of the next line. The orchestration layer (Layer 5) uses `emit_token()` for
token-aware output and `emit()` for raw text when spacing is handled manually.

## Examples

Given the rules, here is how specific token pairs are resolved:

| Left | Right | Rule | Action | Result |
|------|-------|------|--------|--------|
| `Ident` | `Plus` | BeforeBinaryOp (p40) | Space | `x + ...` |
| `Plus` | `Ident` | AfterBinaryOp (p40) | Space | `... + y` |
| `LParen` | `Ident` | AfterLParen (p20) | None | `(x` |
| `Ident` | `RParen` | BeforeRParen (p20) | None | `x)` |
| `Ident` | `Dot` | BeforeDot (p25) | None | `x.` |
| `Dot` | `Ident` | AfterDot (p25) | None | `.y` |
| `Comma` | `Ident` | AfterComma (p30) | Space | `, x` |
| `At` | `Ident` | AtIdent (p35) | None | `@foo` |
| `Pub` | `At` | AfterPub (p50) | Space | `pub @` |
| `Run` | `LParen` | RunParen (p55) | None | `run(` |
| `If` | `Ident` | AfterIf (p50) | Space | `if cond` |
| `LParen` | `RParen` | EmptyParens (p10) | None | `()` |
| `Ident` | `Eq` | BeforeEq (p40) | Space | `x = ...` |
| `Eq` | `Int` | AfterEq (p40) | Space | `= 42` |
| `Ident` | `CompoundAssign` | BeforeCompoundAssign (p40) | Space | `x += ...` |
