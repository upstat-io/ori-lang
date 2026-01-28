# Parser v2: Tristate Disambiguation

## Overview

This document describes the tristate lookahead pattern for handling syntactic ambiguity, inspired by TypeScript's parser. Instead of always speculating or always committing, we use three-valued logic to make optimal decisions.

## The Ambiguity Problem

### Ori's Ambiguous Constructs

Several Ori constructs share initial tokens:

| Input | Possible Interpretations |
|-------|-------------------------|
| `(x)` | Grouped expression, single-element tuple, lambda param |
| `(x, y)` | Tuple, lambda params |
| `(x) -> ...` | Lambda |
| `{ x: 1 }` | Map literal, struct literal (context-dependent) |
| `Foo { ... }` | Struct literal (not in if condition), struct pattern |
| `x.match(...)` | Method call named "match", guard pattern |

### Current Approach Problems

The current parser uses complex nested conditionals:

```rust
// Current: Complex disambiguation logic
fn parse_paren_expr(&mut self) -> Result<Expr> {
    if self.is_lambda_start() {           // Lookahead
        self.parse_lambda()
    } else if self.peek_after_paren_is_arrow() {  // More lookahead
        self.try_parse_lambda_or_tuple()
    } else if ...                         // Even more branches
}
```

**Problems:**
1. Scattered lookahead logic
2. Duplicate parsing attempts
3. Hard to maintain
4. Error messages can be confusing

## Tristate Pattern

### The Tristate Enum

```rust
/// Result of disambiguation lookahead
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tristate {
    /// Definitely this interpretation
    True,
    /// Definitely not this interpretation
    False,
    /// Could be either - need speculation
    Unknown,
}

impl Tristate {
    /// Combine with another tristate (AND logic)
    pub fn and(self, other: Tristate) -> Tristate {
        match (self, other) {
            (Tristate::False, _) | (_, Tristate::False) => Tristate::False,
            (Tristate::True, Tristate::True) => Tristate::True,
            _ => Tristate::Unknown,
        }
    }

    /// Combine with another tristate (OR logic)
    pub fn or(self, other: Tristate) -> Tristate {
        match (self, other) {
            (Tristate::True, _) | (_, Tristate::True) => Tristate::True,
            (Tristate::False, Tristate::False) => Tristate::False,
            _ => Tristate::Unknown,
        }
    }
}
```

### Decision Flow

```
                    ┌─────────────────┐
                    │  Quick Lookahead │
                    │   (1-2 tokens)   │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
         ┌────────┐    ┌──────────┐    ┌────────┐
         │  True  │    │  Unknown │    │ False  │
         └────┬───┘    └────┬─────┘    └────┬───┘
              │             │               │
              ▼             ▼               ▼
         ┌────────┐    ┌──────────┐    ┌────────┐
         │ Commit │    │Speculate │    │  Try   │
         │ Parse  │    │   Parse  │    │ Other  │
         └────────┘    └──────────┘    └────────┘
```

### Benefits

1. **Fast path for common cases** - True/False avoid speculation
2. **Safe path for edge cases** - Unknown triggers backtracking
3. **Clear decision points** - Logic is explicit and testable
4. **Better errors** - Know why we chose each interpretation

## Lambda Disambiguation

### The Challenge

```ori
// These all start with `(`
()                      // Unit
(x)                     // Grouped expression OR lambda param
(x, y)                  // Tuple OR lambda params
(x) -> x + 1            // Lambda (single param)
(x, y) -> x + y         // Lambda (multi param)
(x: int) -> int = x * 2 // Typed lambda
```

### Tristate Implementation

```rust
impl Parser<'_> {
    /// Determine if parenthesized expression is a lambda
    fn is_lambda(&self) -> Tristate {
        debug_assert!(self.at(TokenKind::LParen));

        // Save position for lookahead
        let start_idx = self.cursor.index();

        // Look at what's inside the parens
        self.advance(); // Skip (

        match self.current_kind() {
            // () -> definitely unit or lambda with no params
            TokenKind::RParen => {
                self.cursor.set_index(start_idx);
                // Need to check for arrow after )
                if self.peek_at(1) == TokenKind::Arrow {
                    Tristate::True  // () ->
                } else {
                    Tristate::False // () - unit
                }
            }

            // (x: - definitely typed param, so lambda
            TokenKind::Ident if self.peek_next() == TokenKind::Colon => {
                self.cursor.set_index(start_idx);
                Tristate::True
            }

            // (x) - could be grouped expr or lambda
            TokenKind::Ident => {
                let after_ident = self.peek_next();
                self.cursor.set_index(start_idx);

                match after_ident {
                    // (x) - need to check for arrow
                    TokenKind::RParen => Tristate::Unknown,
                    // (x, - could be tuple or lambda
                    TokenKind::Comma => Tristate::Unknown,
                    // (x + - definitely expression
                    _ if is_binary_op(after_ident) => Tristate::False,
                    // (x. - could be method chain in expr
                    TokenKind::Dot => Tristate::False,
                    // (x( - function call in expr
                    TokenKind::LParen => Tristate::False,
                    // Other - probably expression
                    _ => Tristate::False,
                }
            }

            // ([ or ({ - definitely expression (array/map in parens)
            TokenKind::LBracket | TokenKind::LBrace => {
                self.cursor.set_index(start_idx);
                Tristate::False
            }

            // Literal - definitely expression
            TokenKind::Int | TokenKind::Float | TokenKind::String | TokenKind::True | TokenKind::False => {
                self.cursor.set_index(start_idx);
                Tristate::False
            }

            // Other - probably expression
            _ => {
                self.cursor.set_index(start_idx);
                Tristate::False
            }
        }
    }

    /// Parse parenthesized expression with tristate disambiguation
    fn parse_paren_expr(&mut self) -> ParseResult<NodeId> {
        let start = self.current_span();

        match self.is_lambda() {
            Tristate::True => {
                // Definitely lambda - parse directly
                self.parse_lambda(start)
            }

            Tristate::False => {
                // Definitely not lambda - parse as grouped/tuple
                self.parse_grouped_or_tuple(start)
            }

            Tristate::Unknown => {
                // Could be either - speculate
                let snapshot = self.snapshot();

                // Try lambda first (more specific)
                match self.parse_lambda_params_and_arrow() {
                    Ok((params, _arrow_span)) => {
                        // Successfully parsed params and arrow - commit to lambda
                        self.parse_lambda_body(start, params)
                    }
                    Err(_) => {
                        // Not a lambda - restore and parse as expression
                        self.restore(snapshot);
                        self.parse_grouped_or_tuple(start)
                    }
                }
            }
        }
    }
}
```

## Struct Literal vs Map Literal

### The Challenge

```ori
// In expression context:
{ x: 1, y: 2 }     // Map literal (key: value pairs)
Point { x: 1, y: 2 } // Struct literal (Type { fields })

// In if condition (NO_STRUCT_LIT context):
if x { ... }       // Block, not struct literal
```

### Tristate Implementation

```rust
impl Parser<'_> {
    /// Determine if uppercase identifier starts struct literal
    fn is_struct_literal(&self) -> Tristate {
        debug_assert!(self.at(TokenKind::UpperIdent));

        // In NO_STRUCT_LIT context, always false
        if self.context.contains(ParseContext::NO_STRUCT_LIT) {
            return Tristate::False;
        }

        // Check next token
        match self.peek_next() {
            // Type { - definitely struct literal
            TokenKind::LBrace => Tristate::True,

            // Type( - variant with fields or function call
            TokenKind::LParen => Tristate::Unknown,

            // Type< - generic type, not struct literal
            TokenKind::Lt => Tristate::False,

            // Type. - qualified name, need more context
            TokenKind::Dot => Tristate::Unknown,

            // Other - not struct literal
            _ => Tristate::False,
        }
    }

    fn parse_primary_upper_ident(&mut self) -> ParseResult<NodeId> {
        match self.is_struct_literal() {
            Tristate::True => {
                self.parse_struct_literal()
            }

            Tristate::False => {
                // Just a variant constructor or type reference
                self.parse_variant_or_type_ref()
            }

            Tristate::Unknown => {
                // Need to look further
                if self.peek_next() == TokenKind::LParen {
                    // Could be variant with fields: Some(x)
                    // Or generic type constructor
                    self.parse_variant_with_fields()
                } else {
                    // Qualified name: Module.Type
                    self.parse_qualified_name()
                }
            }
        }
    }
}
```

## Soft Keywords

### The Challenge

Ori has soft keywords that are only keywords in specific contexts:

```ori
// 'run' as keyword
run(let x = 1, x)

// 'run' as identifier
let run = 42
run + 1

// 'match' as pattern keyword
match(x, 1 -> "one", _ -> "other")

// 'match' in guard pattern
x.match(is_valid)
```

### Tristate Implementation

```rust
impl Parser<'_> {
    /// Check if identifier is being used as pattern keyword
    fn is_pattern_keyword(&self, name: &str) -> Tristate {
        // Must be followed by (
        if self.peek_next() != TokenKind::LParen {
            return Tristate::False;
        }

        match name {
            "run" | "try" => Tristate::True,

            "match" => {
                // Could be pattern keyword: match(x, ...)
                // Or method call: something.match(...)
                // Check if we're at start of expression
                if self.at_expression_start() {
                    Tristate::True
                } else {
                    Tristate::Unknown
                }
            }

            "for" => {
                // for(over: ...) - pattern
                // for x in xs do ... - loop
                // Need to check argument style
                Tristate::Unknown
            }

            "recurse" | "parallel" | "spawn" | "timeout" | "cache" | "with" | "catch" => {
                Tristate::True
            }

            _ => Tristate::False,
        }
    }

    fn parse_ident_or_pattern(&mut self) -> ParseResult<NodeId> {
        let name = self.current_text();

        match self.is_pattern_keyword(name) {
            Tristate::True => {
                self.parse_pattern_expr(name)
            }

            Tristate::False => {
                self.parse_identifier()
            }

            Tristate::Unknown => {
                // Look at arguments to disambiguate
                let snapshot = self.snapshot();

                self.advance(); // Skip identifier
                if self.eat(TokenKind::LParen) {
                    // Check first argument
                    if self.is_named_argument() {
                        // Named arg: for(over: ...) - pattern
                        self.restore(snapshot);
                        return self.parse_pattern_expr(name);
                    }
                }

                // Not pattern - restore and parse as identifier
                self.restore(snapshot);
                self.parse_identifier()
            }
        }
    }
}
```

## Guard Pattern vs Method Call

### The Challenge

```ori
// Guard pattern in match
match(x,
    n.match(n > 0) -> "positive",  // Guard: n where n > 0
    _ -> "other",
)

// Method call named 'match'
let result = strategy.match(input)  // Method call
```

### Tristate Implementation

```rust
impl Parser<'_> {
    /// In pattern context, is .match(...) a guard or method?
    fn is_guard_pattern(&self) -> Tristate {
        if !self.context.contains(ParseContext::IN_PATTERN) {
            return Tristate::False;
        }

        // In pattern context, check if makes sense as guard
        // x.match(pred) where pred is expression
        Tristate::True  // In pattern context, always treat as guard
    }
}
```

## Lookahead Caching

### The Problem

Multiple tristate checks might examine the same tokens:

```rust
// Each check does lookahead
let is_lambda = self.is_lambda();
let is_tuple = self.is_tuple();
let is_unit = self.is_unit();
```

### Solution: Lookahead Cache

```rust
/// Cache for expensive lookahead results
pub struct LookaheadCache {
    /// Token index when cache was computed
    position: u32,
    /// Cached results (invalidated on advance)
    is_lambda: Option<Tristate>,
    is_struct_literal: Option<Tristate>,
    is_pattern_keyword: Option<Tristate>,
}

impl LookaheadCache {
    pub fn new() -> Self {
        Self {
            position: u32::MAX, // Invalid position
            is_lambda: None,
            is_struct_literal: None,
            is_pattern_keyword: None,
        }
    }

    pub fn invalidate(&mut self) {
        self.position = u32::MAX;
        self.is_lambda = None;
        self.is_struct_literal = None;
        self.is_pattern_keyword = None;
    }
}

impl Parser<'_> {
    fn advance(&mut self) {
        self.cursor.advance();
        self.lookahead_cache.invalidate();
    }

    fn is_lambda_cached(&mut self) -> Tristate {
        let pos = self.cursor.index();

        if self.lookahead_cache.position != pos {
            self.lookahead_cache = LookaheadCache::new();
            self.lookahead_cache.position = pos;
        }

        *self.lookahead_cache.is_lambda.get_or_insert_with(|| {
            self.compute_is_lambda()
        })
    }
}
```

## Testing Disambiguation

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // === Lambda Tests ===

    #[test]
    fn test_definitely_lambda() {
        let parser = parser_at("(x: int) -> x + 1");
        assert_eq!(parser.is_lambda(), Tristate::True);
    }

    #[test]
    fn test_definitely_not_lambda() {
        let parser = parser_at("(1 + 2)");
        assert_eq!(parser.is_lambda(), Tristate::False);
    }

    #[test]
    fn test_unknown_lambda() {
        let parser = parser_at("(x)");
        assert_eq!(parser.is_lambda(), Tristate::Unknown);
    }

    #[test]
    fn test_unknown_resolves_to_lambda() {
        let (ast, _) = parse("(x) -> x + 1");
        assert!(matches!(ast, Expr::Lambda { .. }));
    }

    #[test]
    fn test_unknown_resolves_to_grouped() {
        let (ast, _) = parse("(x) + 1");
        assert!(matches!(ast, Expr::Binary { .. }));
    }

    // === Struct Literal Tests ===

    #[test]
    fn test_struct_literal_after_type() {
        let parser = parser_at("Point { x: 1 }");
        assert_eq!(parser.is_struct_literal(), Tristate::True);
    }

    #[test]
    fn test_no_struct_literal_in_if() {
        let mut parser = parser_at("Foo { x }");
        parser.context |= ParseContext::NO_STRUCT_LIT;
        assert_eq!(parser.is_struct_literal(), Tristate::False);
    }

    // === Soft Keyword Tests ===

    #[test]
    fn test_run_as_keyword() {
        let parser = parser_at("run(let x = 1, x)");
        assert_eq!(parser.is_pattern_keyword("run"), Tristate::True);
    }

    #[test]
    fn test_run_as_identifier() {
        let parser = parser_at("run + 1");
        assert_eq!(parser.is_pattern_keyword("run"), Tristate::False);
    }

    // === Cache Tests ===

    #[test]
    fn test_lookahead_cache() {
        let mut parser = parser_at("(x) -> x");

        // First call computes
        let result1 = parser.is_lambda_cached();

        // Second call uses cache
        let result2 = parser.is_lambda_cached();

        assert_eq!(result1, result2);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut parser = parser_at("(x) -> x");

        let result1 = parser.is_lambda_cached();
        parser.advance();
        // Cache should be invalidated
        assert!(parser.lookahead_cache.is_lambda.is_none());
    }
}
```

## Best Practices

### When to Use Tristate

| Situation | Approach |
|-----------|----------|
| Single token decides | Simple conditional |
| 2 tokens decide | Tristate check |
| N tokens decide | Tristate with speculation |
| Context-dependent | Tristate with context flags |

### Tristate Design Rules

1. **Quick checks first** - Start with fast O(1) checks
2. **Common cases fast** - True/False should cover 90%+ of inputs
3. **Unknown is rare** - Speculation should be uncommon
4. **Cache expensive checks** - Don't recompute on same position
5. **Clear error paths** - Each branch should have good errors

### Debugging Tristate

```rust
#[cfg(debug_assertions)]
impl Parser<'_> {
    fn is_lambda(&mut self) -> Tristate {
        let result = self.compute_is_lambda();

        if std::env::var("ORI_DEBUG").map(|v| v.contains("tristate")).unwrap_or(false) {
            eprintln!(
                "[tristate] is_lambda at {:?}: {:?}",
                self.current_span(),
                result
            );
        }

        result
    }
}
```

## Summary

The tristate pattern provides:

1. **Fast common cases** - True/False avoid speculation
2. **Safe edge cases** - Unknown triggers careful handling
3. **Clear decision tree** - Each branch is explicit
4. **Testable logic** - Tristate values are easy to test
5. **Cacheable results** - Avoid redundant lookahead
6. **Better errors** - Know which interpretation was attempted

This is the same approach TypeScript uses to handle its complex grammar with arrow functions, generics, JSX, and other ambiguous constructs.
