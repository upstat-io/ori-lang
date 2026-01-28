# Parser v2: Indentation Handling

## Overview

This document describes first-class indentation handling for Ori, inspired by Elm and Roc. Instead of treating indentation as post-processing, we track and validate it during parsing.

## Why First-Class Indentation?

### Current Approach Problems

Most parsers ignore whitespace and validate indentation later:

```rust
// Current: Parse first, validate later
let ast = parse(source);        // Ignores indentation
validate_indent(ast)?;          // Separate pass
```

**Problems:**
1. Error positions are imprecise (point to statement, not indent)
2. Can't give "expected indent X, found Y" messages
3. Duplicate work (traverse AST twice)
4. Can't use indentation to disambiguate syntax

### First-Class Approach

Track indentation in parser state, validate during parsing:

```rust
// Proposed: Track during parse
struct ParserState {
    min_indent: u16,    // Minimum required indentation
    // ...
}

// Validate as we go
if self.column() < self.min_indent {
    return self.error(IndentError::TooShallow { ... });
}
```

**Benefits:**
1. Precise error positions (exact column)
2. Clear error messages ("expected indent ≥4, found 2")
3. Single pass
4. Can use indentation in grammar rules

## Indentation Model

### State Fields

```rust
pub struct ParserState<'a> {
    // Token stream
    tokens: &'a [Token],
    token_index: u32,

    // Position tracking
    current_pos: Position,

    // Indentation
    /// Minimum indentation required for current context
    min_indent: u16,
    /// Stack of indentation contexts (for nested scopes)
    indent_stack: SmallVec<[u16; 8]>,
}

/// Position in source
#[derive(Clone, Copy, Debug)]
pub struct Position {
    pub line: u32,
    pub column: u16,
    pub offset: u32,
}
```

### Indentation Invariant

At any point during parsing:
- `min_indent` is the minimum column for the current construct
- All tokens must have `column >= min_indent`
- Nested constructs push/pop `min_indent`

## Indentation Combinators

### Core Combinators

```rust
impl Parser<'_> {
    /// Get current column (1-based)
    #[inline]
    pub fn column(&self) -> u16 {
        self.current_pos.column
    }

    /// Check if current position meets minimum indent
    pub fn check_indent(&self) -> Result<(), IndentError> {
        let col = self.column();
        if col < self.min_indent {
            Err(IndentError::TooShallow {
                position: self.position(),
                expected: self.min_indent,
                found: col,
            })
        } else {
            Ok(())
        }
    }

    /// Push new minimum indent
    pub fn push_indent(&mut self, new_min: u16) {
        self.indent_stack.push(self.min_indent);
        self.min_indent = new_min;
    }

    /// Pop to previous minimum indent
    pub fn pop_indent(&mut self) {
        self.min_indent = self.indent_stack.pop().unwrap_or(1);
    }

    /// Execute parser with increased minimum indent
    pub fn with_indent<T>(
        &mut self,
        new_min: u16,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.push_indent(new_min);
        let result = f(self);
        self.pop_indent();
        result
    }

    /// Execute parser requiring indent greater than current column
    pub fn indented<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let new_min = self.column() + 1;
        self.with_indent(new_min, f)
    }

    /// Execute parser requiring same indent as current column
    pub fn at_column<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let new_min = self.column();
        self.with_indent(new_min, f)
    }
}
```

### Higher-Level Combinators

```rust
impl Parser<'_> {
    /// Parse sequence where continuation must be indented more than start
    ///
    /// Used for: function bodies, block contents, etc.
    pub fn indented_seq<A, B>(
        &mut self,
        parse_first: impl FnOnce(&mut Self) -> ParseResult<A>,
        parse_rest: impl FnOnce(&mut Self) -> ParseResult<B>,
    ) -> ParseResult<(A, B)> {
        // Parse first element
        let first_col = self.column();
        let first = parse_first(self)?;

        // Rest must be indented more
        let result = self.with_indent(first_col + 1, |p| {
            p.check_indent()?;
            parse_rest(p)
        })?;

        ParseResult::ok(Progress::Made, (first, result))
    }

    /// Parse items that must all be at the same column
    ///
    /// Used for: match arms, list elements on separate lines
    pub fn aligned_items<T>(
        &mut self,
        parse_item: impl Fn(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<Vec<T>> {
        let mut items = Vec::new();
        let expected_col = self.column();

        loop {
            let col = self.column();

            if col < self.min_indent {
                // Dedented past our context - done
                break;
            }

            if col != expected_col {
                return self.error(IndentError::Inconsistent {
                    position: self.position(),
                    expected: expected_col,
                    found: col,
                });
            }

            match parse_item(self) {
                ok @ ParseResult { result: Ok(_), .. } => {
                    items.push(ok.result.unwrap());
                }
                ParseResult { progress: Progress::None, .. } => {
                    // No progress - done with items
                    break;
                }
                err => return err.map(|_| vec![]),
            }
        }

        ParseResult::ok(Progress::Made, items)
    }

    /// Parse item, then continuation that must be indented
    ///
    /// Used for: let...in, where clauses
    pub fn with_continuation<A, B>(
        &mut self,
        parse_head: impl FnOnce(&mut Self) -> ParseResult<A>,
        parse_body: impl FnOnce(&mut Self) -> ParseResult<B>,
    ) -> ParseResult<(A, B)> {
        let head_col = self.column();
        let head = parse_head(self)?;

        // Body must be indented past head's column
        self.with_indent(head_col + 1, |p| {
            let body = parse_body(p)?;
            ParseResult::ok(Progress::Made, (head, body))
        })
    }
}
```

## Usage Examples

### Function Body

```rust
impl Parser<'_> {
    fn parse_function(&mut self) -> ParseResult<NodeId> {
        // @name (params) -> Type =
        let at_span = self.expect_span(TokenKind::At)?;
        let name = self.expect_ident()?;
        let params = self.parse_params()?;
        let return_type = self.parse_optional_return_type()?;
        self.expect(TokenKind::Equals)?;

        // Body must be indented more than @
        let body = self.indented(|p| p.parse_expr())?;

        // Build function node
        // ...
    }
}
```

Example input:
```ori
@foo (x: int) -> int =
    x + 1    // OK: indented past @

@bar (x: int) -> int =
x + 1        // ERROR: expected indent ≥5, found 1
```

### Run Expression

```rust
impl Parser<'_> {
    fn parse_run(&mut self) -> ParseResult<NodeId> {
        // run(
        let start = self.current_span();
        self.expect_keyword("run")?;
        self.expect(TokenKind::LParen)?;

        // Contents must be indented past 'run'
        let open_col = self.column();
        let bindings = self.with_indent(open_col, |p| {
            p.parse_seq_bindings()
        })?;

        self.expect(TokenKind::RParen)?;
        // ...
    }
}
```

Example:
```ori
let result = run(
    let x = get_value(),
    let y = process(x),
    x + y
)

// ERROR: contents not indented
let bad = run(
let x = 1,
x
)
```

### Match Arms

```rust
impl Parser<'_> {
    fn parse_match(&mut self) -> ParseResult<NodeId> {
        // match(scrutinee,
        self.expect_keyword("match")?;
        self.expect(TokenKind::LParen)?;
        let scrutinee = self.parse_expr()?;
        self.expect(TokenKind::Comma)?;

        // All arms must be at the same column
        let arms = self.aligned_items(|p| p.parse_match_arm())?;

        self.expect(TokenKind::RParen)?;
        // ...
    }
}
```

Example:
```ori
match(x,
    1 -> "one",
    2 -> "two",
    _ -> "other",
)

// ERROR: arms not aligned
match(x,
    1 -> "one",
  2 -> "two",      // inconsistent: expected column 5, found 3
)
```

### If Expression

```rust
impl Parser<'_> {
    fn parse_if(&mut self) -> ParseResult<NodeId> {
        let start = self.current_span();
        self.expect(TokenKind::If)?;

        // Condition on same line or indented
        let cond = self.parse_expr()?;

        self.expect(TokenKind::Then)?;

        // Then branch - indented past 'if'
        let if_col = start.start_column();
        let then_branch = self.with_indent(if_col + 1, |p| {
            p.parse_expr()
        })?;

        // Else branch (if present)
        let else_branch = if self.eat(TokenKind::Else) {
            Some(self.with_indent(if_col + 1, |p| {
                p.parse_expr()
            })?)
        } else {
            None
        };

        // ...
    }
}
```

## Indentation Errors

### Error Types

```rust
#[derive(Debug)]
pub enum IndentError {
    /// Indentation is less than required minimum
    TooShallow {
        position: Position,
        expected: u16,
        found: u16,
    },

    /// Items that should be aligned are not
    Inconsistent {
        position: Position,
        expected: u16,
        found: u16,
    },

    /// Unexpected dedent
    UnexpectedDedent {
        position: Position,
    },
}

impl IndentError {
    pub fn message(&self) -> String {
        match self {
            Self::TooShallow { expected, found, .. } => {
                format!(
                    "Insufficient indentation: expected column ≥{}, found {}",
                    expected, found
                )
            }
            Self::Inconsistent { expected, found, .. } => {
                format!(
                    "Inconsistent indentation: expected column {}, found {}",
                    expected, found
                )
            }
            Self::UnexpectedDedent { .. } => {
                "Unexpected decrease in indentation".into()
            }
        }
    }

    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::TooShallow { .. } => Some(
                "Continuation lines should be indented past their parent"
            ),
            Self::Inconsistent { .. } => Some(
                "Items in a list or match should all start at the same column"
            ),
            Self::UnexpectedDedent { .. } => Some(
                "Check for missing closing delimiter or incomplete expression"
            ),
        }
    }
}
```

### Error Rendering

```
-- INDENTATION ERROR --

  12 | @process (data: [int]) -> int =
  13 |     let sum = data.fold(
  14 |     initial: 0,
              ^
  15 |         op: (acc, x) -> acc + x
  16 |     )

Insufficient indentation: expected column ≥9, found 5

Hint: Arguments should be indented past the opening parenthesis

Try: Indent line 14 to column 9 or more
```

## Integration with Token Stream

### Token Position Tracking

```rust
#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
    // Derived from span during lexing:
    pub line: u32,
    pub column: u16,
}

impl Parser<'_> {
    fn advance(&mut self) {
        self.token_index += 1;
        if self.token_index < self.tokens.len() as u32 {
            let token = &self.tokens[self.token_index as usize];
            self.current_pos = Position {
                line: token.line,
                column: token.column,
                offset: token.span.start,
            };
        }
    }
}
```

### Whitespace-Sensitive Tokens

The lexer produces positions that account for significant whitespace:

```rust
// Lexer tracks column for each token
impl Lexer<'_> {
    fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let start = self.position;
        let kind = self.scan_token();
        let end = self.position;

        Token {
            kind,
            span: Span::new(start.offset, end.offset),
            line: start.line,
            column: start.column,
        }
    }
}
```

## Special Cases

### Multi-Line Strings

Multi-line strings should not affect indentation tracking:

```rust
let msg = "
    This is a multi-line
    string literal
"  // Column 1, but we're inside a string - don't error
```

### Comments

Comments don't affect indentation:

```rust
@foo () -> int =
    // This comment at column 5 is fine
    42  // Body at column 5
```

### Continuation Lines

Expression continuations should be indented:

```rust
let x = foo
    + bar      // OK: continuation indented
    + baz

let y = foo
+ bar          // ERROR: continuation not indented
```

## Configuration

### Indent Width

While formatting enforces 4-space indent, parsing accepts any consistent indent:

```rust
// Parser accepts any indent width
@foo () -> int =
  42        // 2 spaces - OK for parsing

@bar () -> int =
        42  // 8 spaces - OK for parsing
```

The formatter normalizes to 4 spaces.

### Tabs vs Spaces

Parser treats tabs as single characters for column counting. Formatter rejects tabs.

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_must_be_indented() {
        let source = "@foo () -> int =\nx + 1";
        let (_, errors) = parse(source);

        assert!(errors.iter().any(|e| matches!(
            e, ParseError::Indent(IndentError::TooShallow { expected: 1, found: 1, .. })
        )));
    }

    #[test]
    fn test_aligned_match_arms() {
        let source = r#"
match(x,
    1 -> "a",
  2 -> "b",
)
"#;
        let (_, errors) = parse(source);

        assert!(errors.iter().any(|e| matches!(
            e, ParseError::Indent(IndentError::Inconsistent { expected: 5, found: 3, .. })
        )));
    }

    #[test]
    fn test_nested_indent() {
        let source = r#"
@foo () -> int =
    if true then
        42
    else
        0
"#;
        let (ast, errors) = parse(source);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_indent_stack() {
        let mut parser = Parser::new(tokens, source);

        assert_eq!(parser.min_indent, 1);

        parser.push_indent(5);
        assert_eq!(parser.min_indent, 5);

        parser.push_indent(9);
        assert_eq!(parser.min_indent, 9);

        parser.pop_indent();
        assert_eq!(parser.min_indent, 5);

        parser.pop_indent();
        assert_eq!(parser.min_indent, 1);
    }
}
```

## Summary

First-class indentation handling provides:

1. **Precise error positions** - Point at the exact column
2. **Clear error messages** - "expected ≥5, found 3"
3. **Single-pass validation** - No separate indent check
4. **Composable combinators** - `indented`, `aligned_items`, `with_continuation`
5. **Stack-based context** - Handle nested scopes correctly

This follows Elm's approach where indentation errors are caught during parsing with excellent error messages.
