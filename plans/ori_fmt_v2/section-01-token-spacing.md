---
section: "01"
title: Token Spacing Rules
status: not-started
goal: Declarative O(1) token spacing via SpaceRule entries
sections:
  - id: "01.1"
    title: SpaceRule Types
    status: not-started
  - id: "01.2"
    title: Token Matcher
    status: not-started
  - id: "01.3"
    title: Binary Operator Rules
    status: not-started
  - id: "01.4"
    title: Delimiter Rules
    status: not-started
  - id: "01.5"
    title: Keyword Rules
    status: not-started
  - id: "01.6"
    title: Context-Dependent Rules
    status: not-started
  - id: "01.7"
    title: RulesMap Lookup
    status: not-started
---

# Section 01: Token Spacing Rules

**Status:** 📋 Planned
**Goal:** Declarative O(1) lookup for spacing between any two adjacent tokens

> **Spec Reference:** Lines 25-47 (Spacing table), Lines 902-936 (Comment normalization)

---

## 01.1 SpaceRule Types

Define the core types for spacing rules.

- [ ] **Create** `ori_fmt/src/spacing/mod.rs`
  - [ ] `SpaceAction` enum: `None`, `Space`, `Newline`, `Preserve`
  - [ ] `SpaceRule` struct with name, left, right, context, action

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceAction {
    None,           // No space between tokens
    Space,          // Single space
    Newline,        // Line break
    Preserve,       // Keep source spacing
}

pub struct SpaceRule {
    pub name: &'static str,
    pub left: TokenMatcher,
    pub right: TokenMatcher,
    pub context: Option<fn(&FormattingContext) -> bool>,
    pub action: SpaceAction,
}
```

- [ ] **Tests**: Unit tests for SpaceAction equality and debug output

---

## 01.2 Token Matcher

Implement flexible token matching for rules.

- [ ] **Implement** `TokenMatcher` enum
  - [ ] `Any` — matches any token
  - [ ] `Exact(TokenKind)` — matches specific token
  - [ ] `OneOf(&'static [TokenKind])` — matches any in set
  - [ ] `Not(Box<TokenMatcher>)` — inverts match

```rust
pub enum TokenMatcher {
    Any,
    Exact(TokenKind),
    OneOf(&'static [TokenKind]),
    Not(Box<TokenMatcher>),
}

impl TokenMatcher {
    pub fn matches(&self, token: TokenKind) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(k) => *k == token,
            Self::OneOf(kinds) => kinds.contains(&token),
            Self::Not(inner) => !inner.matches(token),
        }
    }
}
```

- [ ] **Tests**: All matcher variants with various token kinds

---

## 01.3 Binary Operator Rules

Rules for spacing around operators (Spec lines 25-30).

- [ ] **Implement** binary operator spacing rules

```rust
// Space around binary operators
rule("SpaceBeforeBinOp", Any, BinaryOp, Space),
rule("SpaceAfterBinOp", BinaryOp, Any, Space),

// Space around arrows
rule("SpaceBeforeArrow", Any, Arrow, Space),
rule("SpaceAfterArrow", Arrow, Any, Space),

// Space around as/as?
rule("SpaceBeforeAs", Any, KwAs, Space),
rule("SpaceAfterAs", KwAs, Any, Space),

// Space around by (ranges)
rule("SpaceBeforeBy", Any, KwBy, Space),
rule("SpaceAfterBy", KwBy, Any, Space),
```

- [ ] **Tests**: Each operator rule with before/after cases

---

## 01.4 Delimiter Rules

Rules for parentheses, brackets, braces (Spec lines 31-41).

- [ ] **Implement** delimiter spacing rules

```rust
// Parentheses: No space inside
rule("NoSpaceAfterLParen", LParen, Not(RParen), None),
rule("NoSpaceBeforeRParen", Not(LParen), RParen, None),

// Brackets: No space inside
rule("NoSpaceAfterLBracket", LBracket, Not(RBracket), None),
rule("NoSpaceBeforeRBracket", Not(LBracket), RBracket, None),

// Struct braces: Space inside (context-dependent)
rule("SpaceAfterLBrace", LBrace, Not(RBrace), Space, ctx: is_struct),
rule("SpaceBeforeRBrace", Not(LBrace), RBrace, Space, ctx: is_struct),

// Empty delimiters: No space
rule("NoSpaceEmptyParens", LParen, RParen, None),
rule("NoSpaceEmptyBrackets", LBracket, RBracket, None),
rule("NoSpaceEmptyBraces", LBrace, RBrace, None),
```

- [ ] **Tests**: Empty vs non-empty delimiters, struct vs non-struct braces

---

## 01.5 Keyword Rules

Rules for keywords, colons, commas (Spec lines 32-35, 42-43).

- [ ] **Implement** keyword and punctuation spacing rules

```rust
// Colons: Space after (type annotations)
rule("SpaceAfterColon", Colon, Any, Space),
rule("NoSpaceBeforeColon", Any, Colon, None),

// Commas: Space after
rule("SpaceAfterComma", Comma, Any, Space),
rule("NoSpaceBeforeComma", Any, Comma, None),

// Visibility: Space after pub
rule("SpaceAfterPub", KwPub, Any, Space),

// Sum type variants: Space around |
rule("SpaceAroundPipe", Any, Pipe, Space, ctx: is_sum_type),
```

- [ ] **Tests**: Colons in type annotations vs labels, pub with various decls

---

## 01.6 Context-Dependent Rules

Rules requiring context functions (Spec lines 36-47).

- [ ] **Implement** context-dependent spacing rules

```rust
// Unary operators: No space after (context-dependent)
rule("NoSpaceAfterUnaryMinus", Minus, Any, None, ctx: is_unary),
rule("NoSpaceAfterNot", Bang, Any, None, ctx: is_unary),
rule("NoSpaceAfterBitNot", Tilde, Any, None),

// Labels: No space around :
rule("NoSpaceLabelColon", Ident, Colon, None, ctx: is_label),
rule("NoSpaceAfterLabelColon", Colon, Ident, None, ctx: after_label),

// Field/member access: No space around .
rule("NoSpaceBeforeDot", Any, Dot, None),
rule("NoSpaceAfterDot", Dot, Any, None),

// Range operators: No space around ../..=
rule("NoSpaceBeforeRange", Any, DotDot, None),
rule("NoSpaceAfterRange", DotDot, Any, None),

// Spread: No space after ...
rule("NoSpaceAfterSpread", DotDotDot, Any, None),

// Error propagation: No space before ?
rule("NoSpaceBeforeQuestion", Any, Question, None),

// Generic bounds: Space after :, around +
rule("SpaceAfterBoundColon", Colon, Any, Space, ctx: is_bound),
rule("SpaceAroundPlus", Any, Plus, Space, ctx: is_bound),

// Default type params: Space around =
rule("SpaceAroundDefaultEq", Any, Eq, Space, ctx: is_generic_default),

// Comments: Space after //
rule("SpaceAfterComment", CommentStart, Any, Space),
```

- [ ] **Implement** context functions
  - [ ] `is_unary(ctx) -> bool`
  - [ ] `is_label(ctx) -> bool`
  - [ ] `after_label(ctx) -> bool`
  - [ ] `is_struct(ctx) -> bool`
  - [ ] `is_sum_type(ctx) -> bool`
  - [ ] `is_bound(ctx) -> bool`
  - [ ] `is_generic_default(ctx) -> bool`

- [ ] **Tests**: Each context function with positive/negative cases

---

## 01.7 RulesMap Lookup

Pre-computed O(1) rule lookup.

- [ ] **Implement** `RulesMap` struct

```rust
pub struct RulesMap {
    /// (left_kind, right_kind) -> applicable rules
    buckets: HashMap<(TokenKind, TokenKind), Vec<&'static SpaceRule>>,
    /// Default rules for Any matchers
    any_left: HashMap<TokenKind, Vec<&'static SpaceRule>>,
    any_right: HashMap<TokenKind, Vec<&'static SpaceRule>>,
    any_any: Vec<&'static SpaceRule>,
}

impl RulesMap {
    pub fn new(rules: &'static [SpaceRule]) -> Self {
        // Build lookup tables at init time
    }

    pub fn lookup(&self, left: TokenKind, right: TokenKind) -> &[&SpaceRule] {
        // O(1) lookup, check context, return first matching rule
    }
}
```

- [ ] **Tests**: Lookup performance, rule priority, context evaluation

---

## 01.8 Completion Checklist

- [ ] All ~35 spacing rules implemented
- [ ] All context functions implemented
- [ ] RulesMap provides O(1) lookup
- [ ] Unit tests for each rule category
- [ ] Integration with FormattingContext

**Exit Criteria:** Token spacing is fully declarative; adding new rules requires only adding entries to `SPACE_RULES` array.
