---
title: "Layer 1: Token Spacing"
description: "Ori Formatter Design — Declarative Token Spacing"
order: 2
section: "Layers"
---

# Layer 1: Token Spacing

The spacing layer provides O(1) declarative rules for spacing between adjacent tokens. It abstracts concrete tokens into categories and maps token pairs to spacing actions.

## Architecture

```
TokenKind → TokenCategory → RulesMap → SpaceAction
                 │              │
                 │              └── O(1) hash lookup
                 └── Ignores literal values
```

## Key Types

### SpaceAction

What spacing to emit between two tokens:

```rust
pub enum SpaceAction {
    /// No space: `foo()`, `list[0]`
    None,

    /// Single space: `a + b`, `x: int`
    Space,

    /// Line break (rarely used in token spacing)
    Newline,

    /// Preserve source spacing
    Preserve,
}
```

The default is `None` — explicit rules add spaces. This ensures tight formatting unless rules specify otherwise.

### TokenCategory

Abstract token types for matching. The key insight is that literal *values* don't affect spacing — only token *types* matter:

```rust
pub enum TokenCategory {
    // Literals (ignore values)
    Int,       // 42, 1_000
    Float,     // 3.14, 2.5e-8
    String,    // "hello"
    Char,      // 'a'
    Duration,  // 100ms, 5s
    Size,      // 4kb, 10mb

    // Keywords
    If, Then, Else, For, In, Let, ...

    // Delimiters
    LParen, RParen, LBrace, RBrace, LBracket, RBracket,

    // Operators
    Plus, Minus, Star, Slash, ...

    // ... (see category.rs for full list)
}
```

The `From<TokenKind>` implementation maps concrete tokens to categories:

```rust
impl From<&TokenKind> for TokenCategory {
    fn from(kind: &TokenKind) -> Self {
        match kind {
            TokenKind::Int(_) => TokenCategory::Int,      // Ignores value
            TokenKind::String(_) => TokenCategory::String, // Ignores content
            TokenKind::Ident(_) => TokenCategory::Ident,   // Ignores name
            // ...
        }
    }
}
```

### TokenMatcher

Flexible matching patterns for rules:

```rust
pub enum TokenMatcher {
    /// Match any token
    Any,

    /// Match exact category
    Exact(TokenCategory),

    /// Match any of several categories
    OneOf(Vec<TokenCategory>),

    /// Match by category property
    Category(CategoryPredicate),
}
```

### RulesMap

Hybrid lookup table with O(1) for exact matches and fallback for pattern rules:

```rust
pub struct RulesMap {
    /// Direct lookup for exact (left, right) pairs.
    exact: FxHashMap<(TokenCategory, TokenCategory), SpaceAction>,

    /// Rules with Any or Category matchers (need linear scan).
    fallback_rules: Vec<&'static SpaceRule>,
}

impl RulesMap {
    pub fn lookup(&self, left: TokenCategory, right: TokenCategory) -> SpaceAction {
        // First try exact match (O(1))
        if let Some(action) = self.exact.get(&(left, right)).copied() {
            return action;
        }

        // Fall back to pattern rules (linear scan, sorted by priority)
        for rule in &self.fallback_rules {
            if rule.matches(left, right) {
                return rule.action;
            }
        }

        SpaceAction::None  // Default
    }
}
```

**Hybrid lookup strategy:**
- Exact `(TokenCategory, TokenCategory)` pairs → HashMap O(1)
- Rules using `Any` or `Category` matchers → Linear scan with priority ordering
- Rules are sorted by priority to ensure correct precedence
```

## Spacing Rules

Rules are declared as `(left, right) → action` tuples. Here are key examples:

### Binary Operators

Space around all binary operators:

```rust
// Space around arithmetic operators
(Ident, Plus, SpaceAction::Space),
(Plus, Ident, SpaceAction::Space),
(Int, Plus, SpaceAction::Space),
(Plus, Int, SpaceAction::Space),
// ... similar for -, *, /, etc.
```

Example: `a + b`, not `a+b`

### Type Annotations

Space after colon in type annotations:

```rust
(Colon, Ident, SpaceAction::Space),
(Colon, IntType, SpaceAction::Space),
(Colon, LBracket, SpaceAction::Space),  // [T]
```

Example: `x: int`, not `x:int`

### Delimiters

No space inside delimiters:

```rust
(LParen, Any, SpaceAction::None),
(Any, RParen, SpaceAction::None),
(LBracket, Any, SpaceAction::None),
(Any, RBracket, SpaceAction::None),
```

Example: `foo(x)`, not `foo( x )`

### Keywords

Space after control flow keywords:

```rust
(If, Any, SpaceAction::Space),
(Then, Any, SpaceAction::Space),
(Else, Any, SpaceAction::Space),
(For, Any, SpaceAction::Space),
(In, Any, SpaceAction::Space),
```

Example: `if x then y else z`, not `if(x)then y`

### No Space

Explicit no-space rules:

```rust
// No space before comma
(Any, Comma, SpaceAction::None),

// No space after @
(At, Ident, SpaceAction::None),

// No space around dots
(Any, Dot, SpaceAction::None),
(Dot, Any, SpaceAction::None),
```

Example: `@foo`, `a.b`, `[1, 2]`

## Usage

### Direct Lookup

```rust
use ori_fmt::spacing::{lookup_spacing, SpaceAction, TokenCategory};

let action = lookup_spacing(TokenCategory::Ident, TokenCategory::Plus);
assert_eq!(action, SpaceAction::Space);

let action = lookup_spacing(TokenCategory::LParen, TokenCategory::Ident);
assert_eq!(action, SpaceAction::None);
```

### In Formatter

The formatter uses spacing rules when emitting tokens:

```rust
fn emit_between(&mut self, left: TokenCategory, right: TokenCategory) {
    match lookup_spacing(left, right) {
        SpaceAction::None => {}
        SpaceAction::Space => self.ctx.emit_space(),
        SpaceAction::Newline => self.ctx.emit_newline(),
        SpaceAction::Preserve => { /* check source */ }
    }
}
```

## Helper Methods

`TokenCategory` provides utility methods:

```rust
impl TokenCategory {
    /// Check if binary operator
    pub fn is_binary_op(self) -> bool {
        matches!(self, Plus | Minus | Star | Slash | ...)
    }

    /// Check if unary operator
    pub fn is_unary_op(self) -> bool {
        matches!(self, Minus | Bang | Tilde)
    }

    /// Check if opening delimiter
    pub fn is_open_delim(self) -> bool {
        matches!(self, LParen | LBrace | LBracket)
    }

    /// Check if closing delimiter
    pub fn is_close_delim(self) -> bool {
        matches!(self, RParen | RBrace | RBracket)
    }

    /// Check if literal
    pub fn is_literal(self) -> bool {
        matches!(self, Int | Float | String | Char | True | False | Duration | Size)
    }
}
```

## Adding New Rules

1. **Add category** (if new token type):
   ```rust
   // In category.rs
   pub enum TokenCategory {
       // ...existing...
       MyNewToken,
   }

   // In From impl
   TokenKind::MyNewToken => TokenCategory::MyNewToken,
   ```

2. **Add spacing rules**:
   ```rust
   // In rules.rs
   (MyNewToken, Ident, SpaceAction::Space),
   (Ident, MyNewToken, SpaceAction::Space),
   ```

3. **Test**:
   ```rust
   #[test]
   fn my_new_token_spacing() {
       assert_eq!(
           lookup_spacing(TokenCategory::MyNewToken, TokenCategory::Ident),
           SpaceAction::Space
       );
   }
   ```

## Performance

- **Hybrid lookup**: O(1) for exact pairs via hash map, linear scan for pattern rules
- **No allocation**: Categories are `Copy`, lookup returns `Copy`
- **Compile-time rules**: Rules are static, map built once at startup
- **110+ categories**: Comprehensive coverage without explosion (count grows as language evolves)
- **Priority ordering**: Fallback rules sorted by priority for correct precedence

## Spec Reference

The spacing rules implement:
- Lines 25-47: Spacing table in formatting spec
- Lines 902-936: Comment normalization
