# Parser v2: Error Recovery

## Overview

This document describes the multi-layered error recovery system, combining techniques from Rust (snapshots), Roc (progress tracking), Go (sync sets), and Elm (precise error types).

## Design Goals

1. **Collect all errors** - Don't stop at the first error
2. **Precise positions** - Errors point at the problem, not past it
3. **Contextual messages** - Each error type has specific, helpful text
4. **Fast recovery** - Skip to next valid construct quickly
5. **No cascading** - Prevent one error from causing many spurious ones

## Progress Tracking

### The Progress Enum

Inspired by Roc, we track whether parsing consumed any input:

```rust
/// Did the parser consume any tokens?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Parser consumed one or more tokens
    Made,
    /// Parser failed without consuming any tokens
    None,
}
```

### Why Progress Matters

Progress determines whether to:
1. **Backtrack** (Progress::None) - Try an alternative
2. **Commit** (Progress::Made) - This was the right path, report the error

```rust
// Example: one_of combinator
macro_rules! one_of {
    ($self:expr, $($parser:expr),+ $(,)?) => {{
        $(
            match $parser {
                // Success - return immediately
                ok @ ParseResult { result: Ok(_), .. } => return ok,
                // Made progress but failed - this was the right path
                err @ ParseResult { progress: Progress::Made, .. } => return err,
                // No progress - try next alternative
                ParseResult { progress: Progress::None, .. } => {}
            }
        )+
        // All alternatives failed without progress
        ParseResult::err(Progress::None, $self.make_error())
    }};
}
```

### ParseResult Type

```rust
/// Result of a parsing operation
pub struct ParseResult<T, E = ParseError> {
    pub progress: Progress,
    pub result: Result<T, E>,
}

impl<T, E> ParseResult<T, E> {
    pub fn ok(progress: Progress, value: T) -> Self {
        Self { progress, result: Ok(value) }
    }

    pub fn err(progress: Progress, error: E) -> Self {
        Self { progress, result: Err(error) }
    }

    /// Chain parsing operations
    pub fn and_then<U>(self, f: impl FnOnce(T) -> ParseResult<U, E>) -> ParseResult<U, E> {
        match self.result {
            Ok(value) => {
                let next = f(value);
                ParseResult {
                    progress: self.progress.combine(next.progress),
                    result: next.result,
                }
            }
            Err(e) => ParseResult::err(self.progress, e),
        }
    }
}

impl Progress {
    /// Combine progress from sequential parsing
    pub fn combine(self, other: Progress) -> Progress {
        match (self, other) {
            (Progress::Made, _) | (_, Progress::Made) => Progress::Made,
            _ => Progress::None,
        }
    }
}
```

## Snapshot-Based Speculation

### Snapshot Structure

```rust
/// Parser state snapshot for backtracking
#[derive(Clone)]
pub struct Snapshot {
    // Token position
    token_index: u32,

    // Storage state
    node_count: u32,
    extra_count: u32,

    // Error state
    error_count: u32,
    expected_tokens: ExpectedTokens,

    // Context state
    context: ParseContext,
    min_indent: u16,
}
```

### Speculation Pattern

```rust
impl Parser<'_> {
    /// Save current parser state
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            token_index: self.cursor.index(),
            node_count: self.storage.len() as u32,
            extra_count: self.storage.extra_len() as u32,
            error_count: self.errors.len() as u32,
            expected_tokens: self.expected.clone(),
            context: self.context,
            min_indent: self.min_indent,
        }
    }

    /// Restore parser to snapshot state
    pub fn restore(&mut self, snapshot: Snapshot) {
        self.cursor.set_index(snapshot.token_index);
        self.storage.truncate(snapshot.node_count as usize);
        self.storage.truncate_extra(snapshot.extra_count as usize);
        self.errors.truncate(snapshot.error_count as usize);
        self.expected = snapshot.expected_tokens;
        self.context = snapshot.context;
        self.min_indent = snapshot.min_indent;
    }

    /// Try parsing speculatively, rolling back on failure
    pub fn speculate<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let snapshot = self.snapshot();
        let result = f(self);

        if result.result.is_err() {
            self.restore(snapshot);
        }

        result
    }

    /// Try parsing, always rolling back (for lookahead)
    pub fn lookahead<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> ParseResult<T>,
    ) -> ParseResult<T> {
        let snapshot = self.snapshot();
        let result = f(self);
        self.restore(snapshot);
        result
    }
}
```

### Usage Example

```rust
impl Parser<'_> {
    /// Parse parenthesized expression or lambda
    fn parse_paren_expr(&mut self) -> ParseResult<NodeId> {
        // Speculate: is this a lambda?
        let snapshot = self.snapshot();

        if let Ok(params) = self.parse_lambda_params() {
            if self.eat(TokenKind::RParen) && self.eat(TokenKind::Arrow) {
                // Definitely a lambda - continue parsing body
                return self.parse_lambda_body(params);
            }
        }

        // Not a lambda - restore and parse as expression
        self.restore(snapshot);
        self.parse_grouped_or_tuple()
    }
}
```

## Synchronization Sets

### Sync Set Definition

```rust
/// Synchronization points for error recovery
pub struct SyncSets;

impl SyncSets {
    /// Tokens that can start a top-level item
    pub const ITEM_START: u64 = {
        (1 << TokenKind::At as u64) |      // @function
        (1 << TokenKind::Pub as u64) |     // pub @function
        (1 << TokenKind::Use as u64) |     // use ...
        (1 << TokenKind::Type as u64) |    // type ...
        (1 << TokenKind::Trait as u64) |   // trait ...
        (1 << TokenKind::Impl as u64) |    // impl ...
        (1 << TokenKind::Extend as u64) |  // extend ...
        (1 << TokenKind::Dollar as u64) |  // $config
        (1 << TokenKind::Hash as u64) |    // #[attr]
        (1 << TokenKind::Extension as u64) // extension ...
    };

    /// Tokens that can end an expression
    pub const EXPR_END: u64 = {
        (1 << TokenKind::RParen as u64) |
        (1 << TokenKind::RBracket as u64) |
        (1 << TokenKind::RBrace as u64) |
        (1 << TokenKind::Comma as u64) |
        (1 << TokenKind::Newline as u64) |
        (1 << TokenKind::Eof as u64)
    };

    /// Tokens that can start a statement in a block
    pub const STMT_START: u64 = {
        (1 << TokenKind::Let as u64) |
        (1 << TokenKind::If as u64) |
        (1 << TokenKind::For as u64) |
        (1 << TokenKind::Loop as u64) |
        (1 << TokenKind::Match as u64) |
        (1 << TokenKind::Return as u64) |
        (1 << TokenKind::Break as u64) |
        (1 << TokenKind::Continue as u64) |
        Self::EXPR_START
    };

    /// Tokens that can start an expression
    pub const EXPR_START: u64 = {
        (1 << TokenKind::Int as u64) |
        (1 << TokenKind::Float as u64) |
        (1 << TokenKind::String as u64) |
        (1 << TokenKind::Char as u64) |
        (1 << TokenKind::True as u64) |
        (1 << TokenKind::False as u64) |
        (1 << TokenKind::Ident as u64) |
        (1 << TokenKind::UpperIdent as u64) |
        (1 << TokenKind::LParen as u64) |
        (1 << TokenKind::LBracket as u64) |
        (1 << TokenKind::LBrace as u64) |
        (1 << TokenKind::Minus as u64) |
        (1 << TokenKind::Bang as u64) |
        (1 << TokenKind::Tilde as u64)
    };

    /// Check if token is in sync set
    #[inline]
    pub fn contains(set: u64, token: TokenKind) -> bool {
        let bit = token as u64;
        if bit < 64 {
            set & (1 << bit) != 0
        } else {
            false
        }
    }
}
```

### Synchronization Implementation

```rust
impl Parser<'_> {
    /// Skip tokens until reaching a synchronization point
    pub fn synchronize(&mut self, sync_set: u64) {
        // Track progress to prevent infinite loops
        let mut iterations = 0;
        let max_iterations = 1000;

        while !self.at_end() && iterations < max_iterations {
            if SyncSets::contains(sync_set, self.current_kind()) {
                return;
            }

            // Also stop at matching delimiter
            if self.at(TokenKind::RBrace) || self.at(TokenKind::RParen) {
                return;
            }

            self.advance();
            iterations += 1;
        }
    }

    /// Skip to next item (for module-level recovery)
    pub fn synchronize_to_item(&mut self) {
        self.synchronize(SyncSets::ITEM_START);
    }

    /// Skip to next statement (for block-level recovery)
    pub fn synchronize_to_stmt(&mut self) {
        self.synchronize(SyncSets::STMT_START | SyncSets::EXPR_END);
    }
}
```

## Expected Token Tracking

### ExpectedTokens Structure

```rust
/// Bitset of expected tokens (up to 64 kinds)
#[derive(Clone, Copy, Default)]
pub struct ExpectedTokens(u64);

impl ExpectedTokens {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn add(&mut self, kind: TokenKind) {
        let bit = kind as u64;
        if bit < 64 {
            self.0 |= 1 << bit;
        }
    }

    pub fn clear(&mut self) {
        self.0 = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = TokenKind> + '_ {
        (0..64).filter_map(move |i| {
            if self.0 & (1 << i) != 0 {
                TokenKind::try_from(i as u8).ok()
            } else {
                None
            }
        })
    }
}
```

### Integration with Parser

```rust
impl Parser<'_> {
    /// Expect a specific token, recording it if not found
    pub fn expect(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            self.expected.clear();
            true
        } else {
            self.expected.add(kind);
            false
        }
    }

    /// Create error with expected tokens
    fn make_unexpected_error(&self) -> ParseError {
        ParseError::UnexpectedToken {
            found: self.current_kind(),
            expected: self.expected.iter().collect(),
            position: self.position(),
            hint: self.contextual_hint(),
        }
    }
}
```

## Error Type Hierarchy

### Top-Level Error Enum

```rust
/// All parse errors
#[derive(Debug)]
pub enum ParseError {
    /// Unexpected token with context
    UnexpectedToken {
        found: TokenKind,
        expected: Vec<TokenKind>,
        position: Position,
        hint: Option<String>,
    },

    /// Expression-specific errors
    Expr(ExprError),

    /// Pattern-specific errors
    Pattern(PatternError),

    /// Type-specific errors
    Type(TypeError),

    /// Item-specific errors
    Item(ItemError),

    /// Module-specific errors
    Module(ModuleError),

    /// Indentation error
    Indent(IndentError),

    /// Lexical error (from tokenizer)
    Lexical(LexicalError),
}
```

### Domain-Specific Errors

```rust
/// Expression errors (30+ variants)
#[derive(Debug)]
pub enum ExprError {
    /// Expression expected but not found
    Start(Position),

    // === Let bindings ===
    /// Expected identifier after 'let'
    LetName(Position),
    /// Expected '=' after let binding name
    LetEquals(Position),
    /// Expected expression after '='
    LetValue(Position),

    // === Conditionals ===
    /// Expected condition after 'if'
    IfCondition(Position),
    /// Expected 'then' after if condition
    IfThen(Position),
    /// Expected expression after 'then'
    IfBody(Position),
    /// Expected expression after 'else'
    IfElse(Position),

    // === Match ===
    /// Expected scrutinee after 'match('
    MatchScrutinee(Position),
    /// Expected ',' after scrutinee
    MatchComma(Position),
    /// Expected pattern in match arm
    MatchPattern(Position),
    /// Expected '->' after pattern
    MatchArrow(Position),
    /// Expected expression after '->'
    MatchBody(Position),

    // === Lambdas ===
    /// Expected parameter in lambda
    LambdaParam(Position),
    /// Expected '->' in lambda
    LambdaArrow(Position),
    /// Expected body after '->'
    LambdaBody(Position),

    // === Calls ===
    /// Expected '(' for function call
    CallOpen(Position),
    /// Expected argument name
    CallArgName(Position),
    /// Expected ':' after argument name
    CallArgColon(Position),
    /// Expected expression for argument value
    CallArgValue(Position),
    /// Expected ')' to close call
    CallClose(Position),

    // === Run/Try patterns ===
    /// Expected ',' in run/try sequence
    SequenceComma(Position),
    /// Expected expression in sequence
    SequenceExpr(Position),

    // === Field access ===
    /// Expected field name after '.'
    FieldName(Position),

    // === Indexing ===
    /// Expected ']' after index
    IndexClose(Position),

    // === Miscellaneous ===
    /// Integer literal overflow
    IntOverflow(Position),
    /// Invalid escape sequence
    InvalidEscape(Position, char),
}

/// Pattern errors
#[derive(Debug)]
pub enum PatternError {
    /// Pattern expected
    Start(Position),
    /// Expected ')' in tuple pattern
    TupleClose(Position),
    /// Expected ']' in list pattern
    ListClose(Position),
    /// Expected '}' in struct pattern
    StructClose(Position),
    /// Expected ':' in struct field pattern
    StructFieldColon(Position),
    /// Expected pattern after '@'
    AtPattern(Position),
    /// Expected ')' after guard
    GuardClose(Position),
    /// Invalid pattern in this context
    InvalidInContext(Position, PatternContext),
}

/// Indentation errors
#[derive(Debug)]
pub enum IndentError {
    /// Insufficient indentation
    TooShallow {
        position: Position,
        expected: u16,
        found: u16,
    },
    /// Inconsistent indentation
    Inconsistent {
        position: Position,
        expected: u16,
        found: u16,
    },
}
```

### Error Hints

```rust
impl ExprError {
    /// Get contextual hint for this error
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::LambdaArrow(_) => Some(
                "Lambda syntax: `x -> expr` or `(x, y) -> expr`"
            ),
            Self::MatchArrow(_) => Some(
                "Match arms use `->`, not `=>`"
            ),
            Self::CallArgColon(_) => Some(
                "Function calls require named arguments: `f(name: value)`"
            ),
            Self::LetEquals(_) => Some(
                "Let bindings use `=`, not `:=`"
            ),
            _ => None,
        }
    }

    /// Get suggested fix if applicable
    pub fn suggestion(&self, source: &str, pos: Position) -> Option<String> {
        match self {
            Self::MatchArrow(p) => {
                // Check if user wrote =>
                if source[p.offset as usize..].starts_with("=>") {
                    Some("Replace `=>` with `->`".into())
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}
```

## Recovery Patterns

### Module-Level Recovery

```rust
impl Parser<'_> {
    /// Parse module with recovery
    pub fn parse_module(&mut self) -> Module {
        let mut items = Vec::new();

        while !self.at_end() {
            match self.parse_item() {
                ParseResult { result: Ok(item), .. } => {
                    items.push(item);
                }
                ParseResult { result: Err(e), progress } => {
                    self.errors.push(e);

                    if progress == Progress::Made {
                        // Made progress - synchronize to next item
                        self.synchronize_to_item();
                    } else {
                        // No progress - skip one token to avoid infinite loop
                        self.advance();
                    }
                }
            }
        }

        Module { items }
    }
}
```

### List-Level Recovery

```rust
impl Parser<'_> {
    /// Parse comma-separated list with recovery
    fn parse_list<T>(
        &mut self,
        parse_element: impl Fn(&mut Self) -> ParseResult<T>,
        end_token: TokenKind,
    ) -> ParseResult<Vec<T>> {
        let mut elements = Vec::new();
        let mut progress = Progress::None;

        while !self.at(end_token) && !self.at_end() {
            match parse_element(self) {
                ParseResult { result: Ok(elem), progress: p } => {
                    elements.push(elem);
                    progress = progress.combine(p);

                    // Handle comma
                    if !self.eat(TokenKind::Comma) {
                        if !self.at(end_token) {
                            // Missing comma - report but continue
                            self.errors.push(ParseError::MissingComma(self.position()));
                        }
                    }
                }
                ParseResult { result: Err(e), progress: p } => {
                    self.errors.push(e);
                    progress = progress.combine(p);

                    // Synchronize to comma, end token, or expr start
                    self.synchronize(
                        (1 << end_token as u64) |
                        (1 << TokenKind::Comma as u64) |
                        SyncSets::EXPR_START
                    );

                    // Skip comma if present
                    self.eat(TokenKind::Comma);
                }
            }
        }

        ParseResult::ok(progress, elements)
    }
}
```

### Expression-Level Recovery

```rust
impl Parser<'_> {
    /// Parse expression with recovery placeholder
    fn parse_expr_or_error(&mut self) -> NodeId {
        match self.parse_expr() {
            ParseResult { result: Ok(node), .. } => node,
            ParseResult { result: Err(e), .. } => {
                self.errors.push(e);

                // Create error placeholder node
                self.storage.alloc(
                    NodeTag::Error,
                    self.current_span(),
                    NodeData { none: () },
                )
            }
        }
    }
}
```

## Error Rendering

### Renderer Implementation

```rust
pub struct ErrorRenderer<'a> {
    source: &'a str,
    lines: Vec<&'a str>,
}

impl<'a> ErrorRenderer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            lines: source.lines().collect(),
        }
    }

    pub fn render(&self, error: &ParseError) -> String {
        let mut output = String::new();

        // Title
        writeln!(output, "-- {} --", error.title()).unwrap();
        writeln!(output).unwrap();

        // Source context
        let pos = error.position();
        self.render_source_context(&mut output, pos);

        // Error message
        writeln!(output).unwrap();
        writeln!(output, "{}", error.message()).unwrap();

        // Hint
        if let Some(hint) = error.hint() {
            writeln!(output).unwrap();
            writeln!(output, "Hint: {}", hint).unwrap();
        }

        // Suggestion
        if let Some(suggestion) = error.suggestion(self.source) {
            writeln!(output).unwrap();
            writeln!(output, "Try: {}", suggestion).unwrap();
        }

        output
    }

    fn render_source_context(&self, output: &mut String, pos: Position) {
        let line_idx = pos.line as usize - 1;

        // Show context lines before
        if line_idx > 0 {
            if let Some(line) = self.lines.get(line_idx - 1) {
                writeln!(output, "{:>4} | {}", pos.line - 1, line).unwrap();
            }
        }

        // Show error line
        if let Some(line) = self.lines.get(line_idx) {
            writeln!(output, "{:>4} | {}", pos.line, line).unwrap();

            // Underline
            let col = pos.column as usize;
            writeln!(output, "     | {}^", " ".repeat(col - 1)).unwrap();
        }

        // Show context line after
        if let Some(line) = self.lines.get(line_idx + 1) {
            writeln!(output, "{:>4} | {}", pos.line + 1, line).unwrap();
        }
    }
}
```

### Example Output

```
-- PARSE ERROR --

   3 | let x = if true {
   4 |     1 + 2
   5 |   } else 3
            ^

Expected 'then' after if condition.

Hint: If expressions use 'if cond then expr else expr' syntax.

Try: Replace '{' with 'then'
```

## Testing Error Recovery

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovers_from_missing_comma() {
        let source = "[1 2 3]"; // Missing commas
        let (ast, errors) = parse(source);

        // Should still parse as list
        assert_eq!(ast.elements.len(), 3);

        // Should have errors for missing commas
        assert!(errors.len() >= 2);
    }

    #[test]
    fn test_recovers_to_next_item() {
        let source = r#"
            @foo () -> void = broken syntax here
            @bar () -> void = 42
        "#;

        let (module, errors) = parse_module(source);

        // Should find @bar even after @foo fails
        assert!(module.items.len() >= 1);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_progress_prevents_infinite_loop() {
        let source = "(((("; // All opens, no closes
        let (_, errors) = parse_expr(source);

        // Should not hang, should have errors
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_expected_tokens_collected() {
        let source = "let x = "; // Missing expression
        let (_, errors) = parse(source);

        if let ParseError::UnexpectedToken { expected, .. } = &errors[0] {
            assert!(expected.contains(&TokenKind::Int));
            assert!(expected.contains(&TokenKind::Ident));
        }
    }

    #[test]
    fn test_speculation_restores_state() {
        let mut parser = Parser::new(tokens, source);

        let before = parser.cursor.index();
        parser.speculate(|p| {
            p.advance();
            p.advance();
            ParseResult::err(Progress::Made, make_error())
        });
        let after = parser.cursor.index();

        assert_eq!(before, after); // State restored
    }
}
```

## Summary

The error recovery system provides:

1. **Progress tracking** - Know when to backtrack vs. commit
2. **Snapshot speculation** - Try alternatives safely
3. **Sync sets** - Fast recovery to known good points
4. **Expected token tracking** - Know what was expected
5. **Hierarchical error types** - Precise, contextual messages
6. **Error hints** - Helpful suggestions for common mistakes
7. **Recovery patterns** - Module, list, and expression level

This combination enables the parser to collect all errors while providing Elm-quality error messages.
