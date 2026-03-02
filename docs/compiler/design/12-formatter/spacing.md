---
title: "Token Spacing"
description: "Ori Compiler Design — Formatter Spacing Rules"
order: 1201
section: "Formatter"
---

# Token Spacing (Layer 1)

## What Is Token Spacing?

**Token spacing** is the simplest and most fundamental formatting decision a code formatter must make: given two adjacent tokens, what whitespace — if any — belongs between them? The formatter just emitted `x`; it is about to emit `+`. Should there be a space? Yes — `x + y`. Now the formatter just emitted `(`; it is about to emit `x`. Space? No — `(x`. What about `@` followed by `name`? No — `@name`. What about `pub` followed by `@`? Yes — `pub @name`.

These answers seem obvious when stated in isolation. The challenge is that a language with even modest syntactic richness has dozens of distinct token types, and the number of possible adjacent pairs grows as the square of that count. Ori has roughly 80 distinct token categories. The number of possible token pairs is 80² = 6,400. Specifying the spacing for every pair individually would require a table with thousands of entries — most of them identical — and would be nearly impossible to audit or modify with confidence.

This is not a new problem. Formatting tools have attacked it in four distinct ways, each with different tradeoffs.

### Classical Approaches

**Grammar-embedded spacing** is the oldest approach, dating to COBOL-era language tooling. Whitespace requirements are expressed directly in the grammar: certain grammar rules produce output tokens with mandatory leading or trailing spaces. This is simple and self-consistent — the grammar simultaneously describes syntax and rendering. But it is inflexible: the spacing is baked into the grammar, impossible to adjust without modifying the language definition, and unable to express context-sensitive rules (like "space around `-` when used as a binary operator, but not when used as unary negation").

**Context-sensitive emission** is the approach used by `gofmt` and `rustfmt`. As the formatter walks the AST and emits tokens, it decides spacing case-by-case, using knowledge of the surrounding syntactic context. A function that formats a binary expression emits spaces around the operator; a function that formats a function call emits no space between the callee and the opening parenthesis. This works well and can handle arbitrarily subtle context-sensitive rules. The cost is that spacing rules are scattered throughout the formatting codebase — to answer "what is the spacing between a `pub` keyword and an `@` sigil?", you must trace through whichever formatting function handles `pub` declarations and find the relevant emit call. Adding a new token type requires auditing every formatting function to see whether it can appear adjacent to the new token.

**Declarative rule tables** centralize all spacing rules in a single location, independent of the AST walk. The formatter consults a lookup structure keyed on the pair of adjacent token categories, retrieves a spacing action, and applies it. This is Ori's approach. The advantage is transparency: every spacing rule is in one file, the complete specification is auditable in one reading, and adding or modifying a rule requires touching exactly one place. The challenge is that a naïve rule table requires a rule for every pair — the N² problem — so the table is paired with a classification layer that groups tokens into categories and supports wildcard patterns. A small number of rules (roughly 30) then covers all 6,400 pairs.

**Document IR embedding** is Prettier's approach. The AST is converted into an intermediate document representation — a tree of `text`, `group`, `indent`, `line`, and `softline` nodes. Spacing is implicit in how nodes are adjacent in this document tree: two `text` nodes placed next to each other with no `line` node between them have no spacing. This is highly flexible and naturally handles all context-sensitive cases. The tradeoff is that spacing rules are implicit in the document construction code spread across every node type's renderer — the same auditing problem as context-sensitive emission, expressed differently.

### Why Ori Uses Declarative Rules with a Priority System

Ori's formatter is designed to be auditable. When a contributor asks "why is there no space between `run` and `(`?" the answer should be findable in one file, in one rule, in under a minute. The declarative rule table achieves this. The priority system (described in detail below) resolves the cases where multiple rules match the same token pair — the combinator that would otherwise force explicit conflict resolution for every ambiguous pair.

The spacing layer is also the foundation that higher layers build on. Packing decisions (Layer 2), shape tracking (Layer 3), and construct-specific breaking rules (Layer 4) all rely on the spacing layer for correct intra-line whitespace. Keeping the spacing layer purely declarative — a function from token pairs to spacing actions with no AST awareness — means it can be tested independently, modified without touching the formatter's formatting logic, and reasoned about in isolation.

## How Ori Handles Token Spacing

The spacing system is built from four interlocking components: a **SpaceAction** type (what to emit), a **TokenCategory** type (a classification of TokenKind), a **TokenMatcher** type (pattern matching over categories), and a **SpaceRule** struct (a single spacing rule combining two matchers, an action, and a priority). These four components, plus the **RulesMap** lookup table, constitute the complete spacing subsystem.

### SpaceAction: The Output Type

`SpaceAction` is the result of every spacing decision — the whitespace to emit between two tokens:

| Variant | Meaning | Example |
|---------|---------|---------|
| `None` | No whitespace | `foo()`, `list[0]`, `@name`, `x.y` |
| `Space` | A single space character | `a + b`, `x: int`, `if cond`, `pub @fn` |
| `Newline` | A line break (used rarely at this layer) | — |
| `Preserve` | Retain whatever whitespace appeared in the source | — |

The **default action is `None`** — when no rule matches a given token pair, the formatter emits no whitespace. This is a deliberate design choice called **default-closed** design. The alternative would be to default to `Space`, treating every token pair as spaced unless a rule says otherwise. Default-`Space` sounds convenient, but it requires explicit `None` rules for every pair that should be tight — parentheses, brackets, dots, sigils, and many more — which would make the rule table far larger and harder to read. Default-`None` requires explicit `Space` rules only for the cases where spaces are wanted, which is the minority of all possible pairs.

The practical consequence of default-`None` is also important for maintenance: a missing rule produces tight spacing, which is visually obvious and immediately noticed. A missing rule under default-`Space` would produce an extra space, which can be subtle — `foo( x )` looks almost right, but `foo(x)` looks clearly wrong. Tight spacing failures are easy to find; loose spacing failures are easy to miss.

### TokenCategory: The Abstraction Layer

`TokenCategory` is an abstraction over `ori_ir::TokenKind`. Its purpose is to strip information that is irrelevant to spacing decisions while grouping related tokens so that rules can match them in bulk.

A `TokenKind::Int(42)` and a `TokenKind::Int(17)` have identical spacing behavior — the integer value `42` vs `17` makes no difference to whether there should be a space before or after the token. `TokenCategory::Int` collapses all integer literals into a single category. Similarly, `TokenKind::Plus` and `TokenKind::Minus` are different tokens but share the same spacing behavior in the vast majority of contexts: `TokenCategory::Plus` and `TokenCategory::Minus` are separate (because they need to be distinguished for unary `-` detection), but they both respond to the `is_binary_op()` predicate, which lets rules match both with a single predicate pattern.

The full set of categories, organized by group:

**Literals** — `Int`, `Float`, `String`, `Char`, `Duration`, `Size`. All carry a value in `TokenKind` that is irrelevant to spacing.

**Identifiers** — `Ident`. Covers all user-defined names, and also context-sensitive keywords (words that are keywords in some positions but identifiers in others, like `from`, `by`, `handler`).

**Keywords** — `Break`, `Continue`, `For`, `If`, `Let`, `Loop`, `Match`, `Pub`, `Type`, `Trait`, `Where`, `With`, `Yield`, `As`, `Extend`, `Extension`, `Tests`. These are reserved words with specific formatting behavior (most take a space after them, some interact with parentheses in special ways).

**Type keywords** — `IntType`, `FloatType`, `BoolType`, `StrType`. The primitive type names (`int`, `float`, `bool`, `str`). Grouped separately from other keywords because they appear in type positions and need slightly different spacing rules around generic parameters.

**Wrappers** — `Ok`, `Err`, `Some`, `None`. Constructor-like names that are tightly bound to the parenthesized arguments that follow them: `Ok(v)`, not `Ok (v)`.

**Constructs** — `Cache`, `Catch`, `Parallel`, `Spawn`, `Recurse`, `Run`, `Try`, `Timeout`. Function-expression forms that always take parenthesized arguments with no space: `run(...)`, `try(...)`.

**Delimiters** — `LParen`, `RParen`, `LBrace`, `RBrace`, `LBracket`, `RBracket`. The six paired brackets that enclose argument lists, blocks, and array literals.

**Operators** — `Plus`, `Minus`, `Star`, `Slash`, `EqEq`, `AmpAmp`, `PipePipe`, `CompoundAssign` (covers `+=`, `-=`, `*=`, etc.), and many others. These are tokens that appear in expression positions and generally take spaces on both sides as binary operators, or no space on one side as unary operators.

**Punctuation** — `At`, `Dollar`, `Hash`, `Colon`, `DoubleColon`, `Comma`, `Dot`, `DotDot`, `DotDotEq`, `Arrow`, `FatArrow`, `Pipe`, `Question`, `DoubleQuestion`, `Semicolon`. Tokens that have specific, idiosyncratic spacing rules.

The category type also provides predicate methods for use in `Category` matchers:

- `is_binary_op()` — true for `Plus`, `Minus`, `Star`, `Slash`, `EqEq`, `AmpAmp`, `PipePipe`, and all other infix operator categories
- `is_unary_op()` — true for `Bang`, `Tilde`, `Minus` (the unary subset)
- `is_open_delim()` — true for `LParen`, `LBrace`, `LBracket`
- `is_close_delim()` — true for `RParen`, `RBrace`, `RBracket`
- `is_literal()` — true for `Int`, `Float`, `String`, `Char`, `Duration`, `Size`
- `is_keyword()` — true for all keyword categories

### TokenMatcher: Pattern Matching

A spacing rule needs to express patterns like "any binary operator on the left side" or "exactly `LParen` on the right side". The `TokenMatcher` enum provides four matching forms:

```rust
pub enum TokenMatcher {
    Any,                             // Wildcard — matches any category
    Exact(TokenCategory),            // Matches exactly one specific category
    OneOf(&'static [TokenCategory]), // Matches any category in a static list
    Category(fn(TokenCategory) -> bool), // Matches via a predicate function
}
```

`Any` is the catch-all wildcard, used in fallback rules. `Exact` is the most common form — most rules specify exact token categories for both sides. `OneOf` is a convenience for rules that share an action across a fixed set of categories, avoiding the need to write one rule per category. `Category` enables rules that match on structural properties — "any binary operator", "any delimiter" — without enumerating every member of the group.

Pre-defined constants provide the most common `Category` matchers:

- `BINARY_OP` — `Category(TokenCategory::is_binary_op)`
- `UNARY_OP` — `Category(TokenCategory::is_unary_op)`
- `OPEN_DELIM` — `Category(TokenCategory::is_open_delim)`
- `CLOSE_DELIM` — `Category(TokenCategory::is_close_delim)`
- `LITERAL` — `Category(TokenCategory::is_literal)`
- `KEYWORD` — `Category(TokenCategory::is_keyword)`

### SpaceRule: A Single Spacing Rule

`SpaceRule` is the core data type — one row in the declarative specification:

```rust
pub struct SpaceRule {
    pub name: &'static str,      // Human-readable name for debugging and error messages
    pub left: TokenMatcher,      // Matcher for the left (preceding) token
    pub right: TokenMatcher,     // Matcher for the right (following) token
    pub action: SpaceAction,     // The spacing to emit between them
    pub priority: u8,            // Lower number = higher precedence; checked first
}
```

The `name` field serves two purposes: it makes the `SPACE_RULES` array self-documenting when read in source, and it appears in debug output when `ORI_LOG=ori_fmt=debug` is enabled, showing which rule resolved each token pair.

All rules are defined as entries in a static `SPACE_RULES` array. There is no programmatic rule construction at runtime. The complete spacing specification is the contents of that array — nothing more.

## The Priority System

When the rule table is constructed, rules are sorted by their `priority` field. Lower numbers are checked first. **The first matching rule wins.** This priority system is how the spacing layer resolves situations where multiple rules could plausibly match the same token pair.

Priority-based resolution is conceptually simple: write the specific rules first (lower priority numbers) and the general rules last (higher priority numbers). A rule for "no space inside empty parentheses" at priority 10 will be checked before a rule for "space after keywords" at priority 50, so `()` is correctly tight even when the token before the `(` was a keyword.

The alternative to priority-based resolution would be explicit conflict resolution — giving every rule a set of rule names it overrides. This is more transparent (you can trace the exact reason one rule wins over another) but more verbose and fragile: adding a new rule requires auditing all existing rules to see whether any conflicts need to be declared.

### Priority Bands

The rules are organized into priority bands. Each band covers a conceptually distinct class of spacing decisions:

| Priority | Band Name | Coverage |
|----------|-----------|---------|
| 10 | Empty delimiters | `()`, `[]`, `{}` are always tight |
| 20 | Delimiter adjacency | No space immediately inside `(`, `[`; no space immediately before `)`, `]` |
| 25 | Field and path access | No space around `.` or `::` |
| 30 | Punctuation | Space after `,`; space after `:` in non-type contexts; no space before `?`; no space around `..`/`..=` |
| 35 | Prefix sigils | No space between `@`, `$`, `#` and the name that follows |
| 40 | Binary and assignment operators | Space around `+`, `-`, `*`, `/`, `=`, `->`, `??`, compound assignments |
| 45 | Unary operators | No space after `!`, `~`; no space between `-` and a following literal |
| 50 | Keyword spacing | Space after `pub`, `let`, `if`, `for`, `where`, `as`, `use`, and other control-flow keywords |
| 55 | Construct-paren adjacency | No space between construct names, wrappers, and built-ins, and the `(` that follows |
| 60 | Sum type pipe | Space around `\|` in type definitions and expressions |
| 70 | Generic bounds | Space around `+` in trait bound expressions |
| 90 | Default fallback | `(Any, Any) → None` — the universal catch-all |

**Priority 10: Empty Delimiters.** The rule at this priority handles the case `LParen → RParen`, `LBracket → RBracket`, and `LBrace → RBrace`, ensuring empty containers are always rendered without interior whitespace: `()`, `[]`, `{}`. This rule must be at the highest priority because later delimiter rules (priority 20) would otherwise apply — an `LParen` followed by `RParen` would match "after open delimiter, no space" and "before close delimiter, no space", producing the correct result only coincidentally. The empty delimiter rule makes the intent explicit.

**Priority 20: Delimiter Adjacency.** These rules establish the general principle that tokens adjacent to open delimiters have no space: `(x`, `[0`, `{key`. Similarly, tokens adjacent to close delimiters have no space: `x)`, `0]`, `}`. This is the widest-reaching spacing rule and covers the vast majority of token pairs involving delimiters. It must come before operator rules (priority 40) because `(` followed by `-` (as in `(-x)`) should be tight, not spaced.

**Priority 25: Field and Path Access.** The `.` operator (field access, method calls) and `::` (module path separator) take no space on either side: `x.field`, `std::io`. These must take priority over operator rules that would otherwise add space around operators — `::` is an operator category that is_binary_op() is false for, but `.` might interact with expression operators.

**Priority 30: Punctuation.** Commas take a space after them (`a, b`), but no space before (`a , b` is wrong). Colons in certain contexts take a space after them (`x: int`, `key: value`). The `?` error propagation operator takes no space before it (`result?`, not `result ?`). Range operators `..` and `..=` take no space on either side (`0..10`, `a..=b`).

**Priority 35: Prefix Sigils.** The sigil characters `@` (function declaration marker), `$` (constant/immutable binding marker), and `#` (attribute marker) are followed immediately by an identifier with no space: `@foo`, `$name`, `#derive`. Without this rule, keyword spacing at priority 50 would produce `# derive` for `#derive(Eq)`.

**Priority 40: Binary and Assignment Operators.** Arithmetic operators (`+`, `-`, `*`, `/`), comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`), logical operators (`&&`, `||`), the arrow `->`, the null-coalescing `??`, and compound assignments (`+=`, `-=`, etc.) all take space on both sides. This priority level handles the common expression formatting case. It comes after delimiter adjacency (priority 20) so that `(-x)` is `(-x)` not `(- x)`, and before unary operators (priority 45) so that the unary rules can override the binary behavior for specific right-hand contexts.

**Priority 45: Unary Operators.** The logical NOT `!`, bitwise NOT `~`, and unary negation `-` take no space between the operator and its operand. This rule overrides the binary operator rule (priority 40) for specific cases: `-` followed by a literal becomes tight (`-42`), while `-` followed by an identifier remains governed by the binary rule context. This is one of the most subtle conflicts in the priority system — see the conflict resolution section below.

**Priority 50: Keyword Spacing.** Most reserved keywords take a space after them: `pub @name`, `let x`, `if cond`, `for i in`, `where T`, `as float`, `use std`, `type Foo`, `trait Bar`, `impl Foo`. This is a broad rule that applies to nearly every keyword in the language. It comes after sigils (priority 35) so that `#derive` is not affected, and before construct-paren (priority 55) so that the construct rules can tighten the space for specific constructs.

**Priority 55: Construct-Paren Adjacency.** A set of construct names, wrapper types, and built-in functions are immediately followed by `(` with no space: `run(`, `try(`, `Ok(`, `Err(`, `Some(`, `print(`, `panic(`, `todo(`, `unreachable(`. Without this rule, the keyword spacing rule at priority 50 would produce `run (...)` and `Ok (value)`. The construct-paren rule narrows the gap by saying: "even if the left token is a keyword or a construct form, if the right token is `LParen`, suppress the space."

**Priority 60: Sum Type Pipe.** The `|` token in sum type variant declarations and in pattern matching takes a space on both sides: `A | B | C`. This rule is separate from the binary operator rules at priority 40 because `|` in Ori plays multiple roles — it is the bitwise OR operator (where it takes spaces), the sum type separator (where it also takes spaces), and the pattern alternation separator (where it also takes spaces). In all of these contexts, the rule at priority 60 applies uniformly.

**Priority 70: Generic Bounds.** In trait bound expressions like `T: A + B`, the `+` takes spaces around it even though the general binary operator rule at priority 40 would also match. The separate priority 70 rule for generic bounds makes the intent explicit, even though the effect is the same as the priority 40 rule in this particular context. Its lower priority means it acts as a semantic clarification, not a functional override.

**Priority 90: Default Fallback.** The catch-all rule `(Any, Any) → None` applies to every token pair not matched by any earlier rule. Its effect is identical to the default-`None` behavior, but making it an explicit rule rather than an implicit default means the rule table is self-contained — the complete specification is the rule array itself, without any implicit behavior.

### Conflict Resolution: Three Worked Examples

**Unary minus vs. binary minus.** The minus token `Minus` appears in two syntactic roles: as a binary subtraction operator (`a - b`) and as a unary negation operator (`-42`). The priority 40 rule adds space on both sides of `Minus` as a binary operator. The priority 45 rule says: when `Minus` is followed by a literal, suppress the space. The priority system resolves this automatically: both the `Minus → Int` pair (unary, e.g., `-42`) and the `Int → Minus` pair (binary operand, e.g., `x - 42`) can be individually queried. The hash map stores `(Minus, Int) → None` (from the unary rule at priority 45) and `(Int, Minus) → Space` (from the binary rule at priority 40). The two cases are distinct token pairs and do not conflict.

The subtler conflict is `(Minus, Ident)` — is `-x` unary negation, or the start of `a - x`? The spacing layer cannot resolve this without AST context, because both cases produce the same adjacent token pair. This is one of the genuine limitations of a purely token-level spacing system. Ori resolves this by treating `(Minus, Ident)` as binary (space), which means unary negation of an identifier is `- x` when spacing is computed in isolation. In practice, unary negation of an identifier is always wrapped in an expression context that the higher-level formatter handles — the token-level rule is a safe approximation, not the complete story.

**Pipe in sum types vs. bitwise OR.** The `|` token is `TokenCategory::Pipe`. The priority 60 rule specifies `(Any, Pipe) → Space` and `(Pipe, Any) → Space`. The binary operator rule at priority 40 also matches `Pipe` via `is_binary_op()`. Since priority 60 is lower precedence than priority 40, one might expect the binary operator rule to win. But note that the priority 40 rule is stored in the hash map as expanded `(Category, Pipe)` fallback entries — the exact entries take precedence over category predicate fallbacks. The net result is that `Pipe` always gets spaces on both sides, which is correct for all three of its uses: bitwise OR, sum type separator, and pattern alternation.

**Construct keywords before parentheses.** Consider `run(tasks:, ...)`. The token pair is `(Run, LParen)`. The keyword spacing rule at priority 50 would match this via a `Category(is_keyword)` predicate on the left side, producing `run (`. The construct-paren rule at priority 55 has `Exact(Run)` on the left and `Exact(LParen)` on the right, producing `None`. Since priority 50 < priority 55, the keyword rule is checked first. In the hash map construction, exact matches are inserted first (higher priority = inserted later, but first-insertion-wins means they are already set). The keyword rule's category predicate goes into the fallback list; the construct-paren rule's exact pair goes into the hash map. At lookup time, the hash map hit for `(Run, LParen)` returns `None` from the construct-paren rule. The keyword category predicate in the fallback list is never reached.

### Worked Examples Table

| Left Token | Right Token | Matching Rule | Priority | Action | Result |
|------------|-------------|---------------|----------|--------|--------|
| `Ident` | `Plus` | BeforeBinaryOp | 40 | Space | `x +` |
| `Plus` | `Ident` | AfterBinaryOp | 40 | Space | `+ y` |
| `LParen` | `Ident` | AfterOpenDelim | 20 | None | `(x` |
| `Ident` | `RParen` | BeforeCloseDelim | 20 | None | `x)` |
| `LParen` | `RParen` | EmptyDelimiters | 10 | None | `()` |
| `Ident` | `Dot` | BeforeDot | 25 | None | `x.` |
| `Dot` | `Ident` | AfterDot | 25 | None | `.y` |
| `Comma` | `Ident` | AfterComma | 30 | Space | `, x` |
| `At` | `Ident` | SigilIdent | 35 | None | `@foo` |
| `Pub` | `At` | AfterPub | 50 | Space | `pub @` |
| `If` | `Ident` | AfterKeyword | 50 | Space | `if cond` |
| `Run` | `LParen` | ConstructParen | 55 | None | `run(` |
| `Ok` | `LParen` | WrapperParen | 55 | None | `Ok(` |
| `Minus` | `Int` | UnaryMinusLiteral | 45 | None | `-42` |
| `Ident` | `Question` | BeforeQuestion | 30 | None | `result?` |
| `Ident` | `DoubleColon` | BeforePath | 25 | None | `std::` |
| `DoubleColon` | `Ident` | AfterPath | 25 | None | `::io` |

## The Lookup Table

The `RulesMap` is the compiled form of the spacing specification — a data structure that can answer the query "what spacing belongs between category A and category B?" in O(1) time for the common case.

### Construction Algorithm

Building the `RulesMap` from the `SPACE_RULES` array happens in four steps:

**Step 1: Sort by priority.** The rules array is sorted ascending by `priority`. This ensures that when two rules compete to claim the same token pair entry in the hash map, the higher-priority rule (lower number) claims it first and the lower-priority rule is rejected (first-insertion-wins).

**Step 2: Expand exact pairs into the hash map.** For each rule, if both matchers are `Exact` or `OneOf`, all resulting `(TokenCategory, TokenCategory)` pairs are computed and inserted into an `FxHashMap<(TokenCategory, TokenCategory), SpaceAction>`. `FxHashMap` is used rather than the standard `HashMap` because the key type is a pair of small integer-like enums — `FxHashMap`'s non-cryptographic hash is significantly faster for this key type. First insertion wins, so lower-priority (numerically smaller) rules that are processed first take precedence.

**Step 3: Store predicate rules in the fallback list.** Rules with `Any` or `Category` matchers cannot be pre-expanded into individual hash map entries — a `Category(is_binary_op)` matcher covers an open-ended set of categories. These rules are stored in a separate `Vec<SpaceRule>`, sorted by priority, for linear scan.

**Step 4: Construct the singleton.** The completed `RulesMap` (hash map + fallback list) is stored in a `OnceLock<RulesMap>` global. The singleton is initialized on first access and shared for all formatting operations. This avoids rebuilding the lookup structure on every file format — the construction cost is paid once.

### Lookup Algorithm

The `lookup_spacing(left: TokenCategory, right: TokenCategory) -> SpaceAction` function:

1. Query the `FxHashMap` with the pair `(left, right)`. If found, return the stored action.
2. Scan the fallback list linearly. For each rule, test the left matcher against `left` and the right matcher against `right`. Return the action of the first matching rule.
3. If no fallback rule matches, return `SpaceAction::None` (the default).

In practice, step 1 handles the vast majority of lookups — most token pairs appear explicitly in the hash map. The fallback list has roughly 5 entries (including the default catch-all). The full lookup cost is O(1) hash + O(k) linear scan where k ≈ 3–5.

### Memory Footprint

The hash map stores roughly 200 entries — the set of all `(Exact, Exact)` and `(OneOf, Exact)` expanded pairs. Each entry is `(u8, u8) → u8` (two enum discriminants as the key, one enum discriminant as the value), padded by the hash map's internal layout. Total footprint is approximately 4 KB. A full 2D matrix indexed by both category discriminants would be 80 × 80 = 6,400 entries at 1 byte each — roughly 6 KB of raw data, but 25 KB once the array is aligned and the code for matrix indexing is included. The hash map uses less memory and requires no pre-allocation for the 6,200 pairs that carry the default `None` action.

## Integration with the Formatter

The spacing layer is integrated into the formatter through `FormatContext`, which maintains formatting state across the emission of tokens.

`FormatContext` tracks `last_token: Option<TokenCategory>` — the category of the most recently emitted token. When the formatter is about to emit a new token, it calls `spacing_for(next_token: TokenCategory) -> SpaceAction`, which calls `lookup_spacing(last_token, next_token)` if `last_token` is `Some`, and returns `None` otherwise (the start of a line has no preceding token).

The `emit_token(category: TokenCategory, text: &str)` method is the full pipeline:

1. If `last_token` is `Some`, call `lookup_spacing(last_token, category)` to get the action
2. Emit the whitespace specified by the action (nothing, a space character, or a newline)
3. Emit the token text
4. Set `last_token = Some(category)`

After emitting a newline (for line breaks in the packing and breaking layers), `clear_last_token()` sets `last_token = None`. This is essential: spacing rules govern intra-line spacing, not inter-line spacing. The horizontal position at the start of a new line is determined by indentation, not by whatever token ended the previous line. If `last_token` were preserved across newlines, a rule like "space after keyword" would insert a spurious space at the start of a line that begins with a keyword continuation.

The separation of concerns is clean: `FormatContext` owns the per-token state and the call to `lookup_spacing`; the `RulesMap` owns the rules and the lookup logic; the higher-level formatter owns the decision of when to emit a newline vs. stay on the current line.

## Prior Art

**[`gofmt`](https://pkg.go.dev/cmd/gofmt)** handles all whitespace — both token spacing and line breaking — through a unified AST walk. Spacing decisions are made inline, case by case, by the formatting functions that handle each node type. There is no separate spacing layer. This approach works well for Go's syntax: Go's expression forms are relatively regular, the operator set is modest, and the number of idiosyncratic spacing rules is small. For a language like Ori with richer operator syntax (bitwise ops, matrix multiplication `@`, range operators `..`/`..=`, null coalescing `??`, pipeline `|>`, capabilities syntax) the case-by-case approach would scatter dozens of spacing rules across the codebase. The declarative table keeps them in one place.

**[Prettier](https://prettier.io/)** handles spacing through its document IR. Adjacent `text()` nodes in the document have no whitespace between them; explicit `line`, `softline`, and `hardline` nodes control breaks; `group` nodes control inline vs. broken rendering. Spacing is embedded in how the document is constructed rather than declared in a rule table. Prettier's approach is more flexible — it can express "no space here, but a soft break here if the group breaks" in a single construct — but answering "what is the spacing between token type X and token type Y?" requires reading through every construct renderer that can produce that adjacency. Ori's table gives a definitive answer in one lookup.

**[`rustfmt`](https://github.com/rust-lang/rustfmt)** uses the same case-by-case approach as `gofmt`, extended to Rust's more complex syntax. The spacing rules for individual token pairs are implemented in various helper methods throughout the codebase. `rustfmt` also inherits the auditability problem: there is no single location where all spacing rules are enumerated. For a contributor debugging an incorrect space, the debugging process involves reading formatting logic rather than reading a rule table.

**[`clang-format`](https://clang.llvm.org/docs/ClangFormat.html)** takes a different approach: a per-token-pair configurability system with style presets. `clang-format` annotates tokens with spacing flags that are computed by inspecting surrounding tokens, then applies a rule-based system to determine actual spacing from those flags. The result is highly configurable but complex — `clang-format`'s spacing logic spans thousands of lines of C++ and is notoriously difficult to reason about. Ori's approach trades configurability for simplicity: the rule table is readable without deep knowledge of the formatter's internals.

Ori's declarative approach has one clear advantage over all of these: the complete spacing specification is a single static array. Adding a new token type or a new spacing rule is a one-line change with no risk of missing a location. Testing the complete spacing behavior requires iterating the rules array — there is nothing hidden.

## Design Tradeoffs

**Priority rules vs. explicit conflict resolution.** The priority system resolves rule conflicts implicitly — the first matching rule wins, and rules are ordered by their priority number. An alternative design would require each rule to declare which other rules it overrides: `rule "ConstructParen" overrides ["KeywordSpace"]`. Explicit conflict declarations are more transparent for any individual conflict but more verbose and harder to maintain as the rule set grows. The priority band system achieves most of the transparency benefit — all rules in a band share a conceptual purpose, and the band ordering reflects a clear design rationale.

**Hash map vs. 2D matrix.** A full 2D matrix with one entry per `(TokenCategory, TokenCategory)` pair would offer guaranteed O(1) lookup with no hash computation. The cost is ~25 KB of allocated memory and a larger binary footprint from the table data. The hash map approach uses ~4 KB and allocates only for pairs with non-default actions. Since the vast majority of pairs (roughly 6,200 of 6,400) carry the default `None` action, the hash map is a natural fit. The O(1) hash + O(k) fallback scan where k ≈ 3–5 is effectively constant-time for any practical formatting workload.

**Default `None` vs. default `Space`.** Default-`None` means missing rules produce tight spacing — visually obvious, easy to detect and fix. Default-`Space` would require explicit `None` rules for every pair that should be tight, which is the majority. The default-`None` design keeps the rule count low and makes missing-rule bugs immediately visible in formatted output.

**Category abstraction vs. raw `TokenKind`.** Abstracting `TokenKind` into `TokenCategory` loses information — the integer value `42` in `Int(42)` is discarded. This is correct because that value is never relevant to spacing decisions. The benefit is that rules can express "before any integer literal" without enumerating every possible integer. The cost is that the classification layer can introduce bugs — a `TokenKind` variant that maps to the wrong `TokenCategory` will silently produce wrong spacing. The category mapping is a simple function, tested directly, so this risk is managed.

**Static singleton vs. per-formatter instance.** The `GLOBAL_RULES_MAP` singleton is initialized once and shared. An alternative design would construct the `RulesMap` on `Formatter::new()`. The singleton avoids the construction cost on every formatting operation and eliminates any possibility of divergence between instances. The tradeoff is that the rules cannot be modified at runtime — all customization must happen at compile time, in the static `SPACE_RULES` array. Given that Ori offers no spacing configuration options, this is not a limitation.

## Related Documents

- [Formatter Overview](./index.md) — five-layer architecture, two-pass algorithm, prior art
- [Packing](./packing.md) — Layer 2: container inline vs. stacked decisions
- [Formatting Rules](./rules.md) — Layers 2–4: width calculation, packing strategies, breaking rules
