# Parser Error Recovery

The Ori parser uses error recovery to parse as much as possible despite syntax errors. This enables reporting multiple errors in one pass.

## Goals

1. **Report multiple errors** - Don't stop at first error
2. **Continue parsing** - Produce partial AST
3. **Avoid cascading errors** - One error shouldn't cause many false errors
4. **Preserve spans** - Track where errors occurred

## Error Types

```rust
pub enum ParseErrorKind {
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
    },
    UnexpectedEof,
    InvalidLiteral(String),
    MissingExpression,
    MissingType,
    InvalidPattern,
    // ...
}

pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
}
```

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
fn expect(&mut self, kind: TokenKind) -> Result<(), ParseError> {
    if self.check(&kind) {
        self.advance();
        Ok(())
    } else {
        // Record error but continue as if token was there
        self.error(ParseErrorKind::UnexpectedToken {
            expected: vec![kind],
            found: self.current().clone(),
        });
        Err(ParseError { ... })
    }
}

fn expect_recover(&mut self, kind: TokenKind) {
    if !self.check(&kind) {
        self.error_expected(kind);
        // Don't advance - continue as if token was present
    } else {
        self.advance();
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
        self.error(ParseErrorKind::MissingExpression);
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
const MAX_ERRORS: usize = 100;

fn should_continue(&self) -> bool {
    self.errors.len() < MAX_ERRORS
}
```

### Panic Mode

Track if we're in error recovery to suppress duplicates:

```rust
struct Parser {
    in_panic_mode: bool,
    // ...
}

fn error(&mut self, kind: ParseErrorKind) {
    if !self.in_panic_mode {
        self.errors.push(ParseError { kind, ... });
        self.in_panic_mode = true;
    }
}

fn synchronize(&mut self) {
    // ... sync logic ...
    self.in_panic_mode = false;  // Exit panic mode
}
```

### Context Tracking

Track context for better errors:

```rust
enum ParseContext {
    Function,
    IfCondition,
    ForLoop,
    MatchArm,
}

fn parse_with_context<T>(&mut self, ctx: ParseContext, f: impl FnOnce(&mut Self) -> T) -> T {
    self.context_stack.push(ctx);
    let result = f(self);
    self.context_stack.pop();
    result
}
```

## Error Messages

### Context-Aware Messages

```rust
fn error_expected(&mut self, kind: TokenKind) {
    let context_hint = match self.current_context() {
        Some(ParseContext::IfCondition) =>
            "in if condition",
        Some(ParseContext::ForLoop) =>
            "in for loop",
        _ => "",
    };

    self.error(ParseErrorKind::UnexpectedToken {
        expected: vec![kind],
        found: self.current().clone(),
        context: context_hint.to_string(),
    });
}
```

### Suggestions

```rust
fn error_unexpected_token(&mut self) {
    let found = self.current().clone();

    // Suggest common fixes
    let suggestion = match &found {
        TokenKind::Eq if self.expected_double_eq() =>
            Some("did you mean '=='?"),
        TokenKind::Semicolon =>
            Some("unexpected ';' - Ori uses expressions, not statements"),
        _ => None,
    };

    self.error(ParseErrorKind::UnexpectedToken {
        expected: self.expected_tokens(),
        found,
        suggestion,
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
