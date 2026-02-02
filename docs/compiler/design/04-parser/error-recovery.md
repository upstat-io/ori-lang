---
title: "Parser Error Recovery"
description: "Ori Compiler Design — Parser Error Recovery"
order: 401
section: "Parser"
---

# Parser Error Recovery

The Ori parser uses error recovery to parse as much as possible despite syntax errors. This enables reporting multiple errors in one pass.

## Goals

1. **Report multiple errors** - Don't stop at first error
2. **Continue parsing** - Produce partial AST
3. **Avoid cascading errors** - One error shouldn't cause many false errors
4. **Preserve spans** - Track where errors occurred

## Error Types

Parse errors use structured `ErrorCode` + message (not an enum):

```rust
pub struct ParseError {
    /// Error code for searchability (e.g., E1001)
    pub code: ori_diagnostic::ErrorCode,
    /// Human-readable message
    pub message: String,
    /// Location of the error
    pub span: Span,
    /// Optional context for suggestions
    pub context: Option<String>,
}

impl ParseError {
    pub fn new(code: ErrorCode, message: impl Into<String>, span: Span) -> Self;
    pub fn with_context(self, context: impl Into<String>) -> Self;
    pub fn to_diagnostic(&self) -> Diagnostic;
}
```

Error codes follow the E1xxx range (see Appendix C):
- `E1001` - Unexpected token
- `E1002` - Expected expression
- `E1003` - Unclosed delimiter
- `E1004` - Expected identifier

## Recovery Strategies

### 1. Synchronization

Skip tokens until a "synchronization point" is found:

```rust
fn synchronize(&mut self) {
    self.advance();  // Skip error token

    while !self.at_end() {
        // Statement/item boundaries are sync points
        if self.previous_was(TokenKind::Semicolon) {
            return;
        }

        match self.current() {
            // Keywords that start statements/items
            TokenKind::Let |
            TokenKind::If |
            TokenKind::For |
            TokenKind::At |
            TokenKind::Type |
            TokenKind::Use => return,
            _ => { self.advance(); }
        }
    }
}
```

### 2. Insertion

Insert a missing token and continue:

```rust
fn expect(&mut self, kind: &TokenKind) -> Result<(), ParseError> {
    if self.cursor.check(kind) {
        self.cursor.advance();
        Ok(())
    } else {
        let span = self.cursor.current_span();
        Err(ParseError::new(E1001, format!("expected {:?}", kind), span))
    }
}

fn expect_recover(&mut self, kind: &TokenKind) {
    if !self.cursor.check(kind) {
        self.error_expected(kind);
        // Don't advance - continue as if token was present
    } else {
        self.cursor.advance();
    }
}
```

### 3. Deletion

Skip unexpected tokens:

```rust
fn parse_list(&mut self) -> Vec<ExprId> {
    let mut items = Vec::new();
    self.expect(TokenKind::LBracket);

    while !self.check(&TokenKind::RBracket) && !self.at_end() {
        // Skip unexpected tokens
        if !self.can_start_expr() {
            self.error_unexpected();
            self.advance();
            continue;
        }

        items.push(self.parse_expr());

        if !self.check(&TokenKind::RBracket) {
            self.expect_recover(TokenKind::Comma);
        }
    }

    self.expect_recover(TokenKind::RBracket);
    items
}
```

### 4. Dummy Values

Return placeholder nodes for invalid input:

```rust
fn parse_expr_or_error(&mut self) -> ExprId {
    if self.can_start_expr() {
        self.parse_expr()
    } else {
        let span = self.cursor.current_span();
        self.push_error(ParseError::new(E1002, "expected expression", span));
        // Return error placeholder
        self.alloc(ExprKind::Error)
    }
}
```

## Recovery Points

### Statement Level

```rust
fn parse_statements(&mut self) -> Vec<Stmt> {
    let mut stmts = Vec::new();

    while !self.at_block_end() {
        match self.try_parse_statement() {
            Ok(stmt) => stmts.push(stmt),
            Err(_) => {
                self.synchronize();
                // Continue with next statement
            }
        }
    }

    stmts
}
```

### Expression Level

```rust
fn parse_primary(&mut self) -> ExprId {
    match self.current() {
        TokenKind::Int(n) => {
            self.advance();
            self.alloc(ExprKind::Literal(Literal::Int(*n)))
        }
        TokenKind::LParen => {
            self.advance();
            let expr = self.parse_expr();
            self.expect_recover(TokenKind::RParen);
            expr
        }
        _ => {
            self.error_expected_expression();
            self.alloc(ExprKind::Error)
        }
    }
}
```

### Item Level

```rust
fn parse_module(&mut self) -> Module {
    let mut functions = Vec::new();

    while !self.at_end() {
        match self.try_parse_item() {
            Ok(item) => {
                match item {
                    Item::Function(f) => functions.push(f),
                    // ...
                }
            }
            Err(_) => {
                // Skip to next item
                self.synchronize_to_item();
            }
        }
    }

    Module { functions, ... }
}
```

## Preventing Cascading Errors

### Error Limit

```rust
### Progress-Based Recovery

Instead of traditional panic mode, Ori uses progress tracking for error recovery:

```rust
pub enum Progress {
    Made,  // Tokens were consumed
    None,  // No tokens consumed
}

pub struct ParseResult<T> {
    pub progress: Progress,
    pub result: Result<T, ParseError>,
}
```

Recovery decisions are based on progress:
- `Progress::None` + error → try alternative productions
- `Progress::Made` + error → commit to path and report error

```rust
fn parse_item_with_progress(&mut self) -> ParseResult<Item> {
    let start_pos = self.cursor.position();
    let result = self.try_parse_item();
    let progress = if self.cursor.position() > start_pos {
        Progress::Made
    } else {
        Progress::None
    };
    ParseResult { progress, result }
}
```

### Context Tracking

The parser uses bitflag-based context for disambiguation:

```rust
pub struct ParseContext(u8);

impl ParseContext {
    const NO_STRUCT_LIT: Self = Self(0b0001);  // In if/while conditions
    const IN_PATTERN: Self = Self(0b0010);     // Parsing match patterns
    const IN_TYPE: Self = Self(0b0100);        // Parsing type annotations
    const IN_LOOP: Self = Self(0b1000);        // Inside loop body
}

fn with_context<T>(&mut self, add: ParseContext, f: impl FnOnce(&mut Self) -> T) -> T {
    let old = self.context;
    self.context = self.context.with(add);
    let result = f(self);
    self.context = old;
    result
}
```

Used to prevent struct literals in conditions:

```rust
fn parse_if_condition(&mut self) -> ExprId {
    self.with_context(ParseContext::NO_STRUCT_LIT, |p| p.parse_expr())
}
```

## Error Messages

### Context-Aware Messages

```rust
fn error_expected(&mut self, kind: &TokenKind) {
    let span = self.cursor.current_span();
    let message = format!("expected {:?}", kind);

    // Add context based on current parsing state
    let context = if self.context.has(ParseContext::NO_STRUCT_LIT) {
        Some("in if condition".to_string())
    } else if self.context.has(ParseContext::IN_LOOP) {
        Some("in loop body".to_string())
    } else {
        None
    };

    let error = ParseError::new(E1001, message, span);
    self.push_error(if let Some(ctx) = context {
        error.with_context(ctx)
    } else {
        error
    });
}
```

### Suggestions

```rust
fn error_unexpected_token(&mut self) {
    let span = self.cursor.current_span();
    let found = self.cursor.current_kind();

    // Suggest common fixes
    let (message, context) = match found {
        TokenKind::Eq =>
            ("unexpected '='", Some("did you mean '=='?")),
        TokenKind::Semicolon =>
            ("unexpected ';'", Some("Ori uses expressions, not statements")),
        _ =>
            ("unexpected token", None),
    };

    let error = ParseError::new(E1001, message, span);
    self.push_error(if let Some(ctx) = context {
        error.with_context(ctx)
    } else {
        error
    });
}
```

## Result

After parsing with errors:

```rust
ParseResult {
    module: Module { ... },  // Partial, with Error nodes
    arena: ExprArena { ... },
    errors: vec![
        ParseError { kind: UnexpectedToken { ... }, span: ... },
        ParseError { kind: MissingExpression, span: ... },
        // Multiple errors reported
    ],
}
```
