# Parser v2: Pratt Parser for Expressions

## Overview

This document describes the Pratt parsing (precedence climbing) algorithm for Ori's expression parser. Pratt parsing elegantly handles operator precedence and associativity without the deep call stack of traditional recursive descent.

## Background

### The Problem with Recursive Descent for Operators

Traditional recursive descent creates one function per precedence level:

```rust
// Traditional approach: 14+ functions for 14 precedence levels
fn parse_or(&mut self) -> Expr { ... parse_and() ... }
fn parse_and(&mut self) -> Expr { ... parse_bitor() ... }
fn parse_bitor(&mut self) -> Expr { ... parse_bitxor() ... }
// ... 11 more levels
fn parse_unary(&mut self) -> Expr { ... }
```

**Problems:**
1. Deep call stack (14 levels for simple `1 + 2`)
2. Repetitive code
3. Hard to modify precedence
4. Difficult to add new operators

### Pratt Parsing Solution

Pratt parsing uses a single loop with precedence comparison:

```rust
fn parse_expr(&mut self, min_prec: u8) -> Expr {
    let mut left = self.parse_prefix();

    while let Some((op, prec)) = self.peek_infix_op() {
        if prec < min_prec { break; }
        self.advance();
        let right = self.parse_expr(prec + 1);  // Left-associative
        left = Expr::Binary(op, left, right);
    }

    left
}
```

**Benefits:**
1. Single loop, constant stack depth
2. Precedence is data, not code structure
3. Easy to add operators
4. Clear associativity handling

## Precedence Table

```rust
/// Operator precedence levels (higher = binds tighter)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Precedence {
    None = 0,       // Not an operator
    Coalesce = 1,   // ??
    Or = 2,         // ||
    And = 3,        // &&
    BitOr = 4,      // |
    BitXor = 5,     // ^
    BitAnd = 6,     // &
    Equality = 7,   // == !=
    Comparison = 8, // < > <= >=
    Range = 9,      // .. ..=
    Shift = 10,     // << >>
    Additive = 11,  // + -
    Multiplicative = 12, // * / % div
    Unary = 13,     // - ! ~ (prefix)
    Postfix = 14,   // . [] () ? (postfix)
    Primary = 15,   // Literals, identifiers
}

impl Precedence {
    /// Get precedence for next iteration (handles associativity)
    pub fn next(self) -> Self {
        // All binary operators are left-associative
        // So we use prec + 1 for right operand
        Self::from_u8(self as u8 + 1).unwrap_or(Self::Primary)
    }

    fn from_u8(n: u8) -> Option<Self> {
        if n <= 15 { Some(unsafe { std::mem::transmute(n) }) } else { None }
    }
}
```

## Operator Classification

```rust
/// Binary operator info
#[derive(Clone, Copy, Debug)]
pub struct BinaryOp {
    pub tag: NodeTag,
    pub precedence: Precedence,
}

impl BinaryOp {
    pub const ADD: Self = Self { tag: NodeTag::BinAdd, precedence: Precedence::Additive };
    pub const SUB: Self = Self { tag: NodeTag::BinSub, precedence: Precedence::Additive };
    pub const MUL: Self = Self { tag: NodeTag::BinMul, precedence: Precedence::Multiplicative };
    pub const DIV: Self = Self { tag: NodeTag::BinDiv, precedence: Precedence::Multiplicative };
    pub const MOD: Self = Self { tag: NodeTag::BinMod, precedence: Precedence::Multiplicative };
    pub const FLOORDIV: Self = Self { tag: NodeTag::BinFloorDiv, precedence: Precedence::Multiplicative };

    pub const SHL: Self = Self { tag: NodeTag::BinShl, precedence: Precedence::Shift };
    pub const SHR: Self = Self { tag: NodeTag::BinShr, precedence: Precedence::Shift };

    pub const RANGE: Self = Self { tag: NodeTag::BinRange, precedence: Precedence::Range };
    pub const RANGE_INC: Self = Self { tag: NodeTag::BinRangeInc, precedence: Precedence::Range };

    pub const LT: Self = Self { tag: NodeTag::BinLt, precedence: Precedence::Comparison };
    pub const LE: Self = Self { tag: NodeTag::BinLe, precedence: Precedence::Comparison };
    pub const GT: Self = Self { tag: NodeTag::BinGt, precedence: Precedence::Comparison };
    pub const GE: Self = Self { tag: NodeTag::BinGe, precedence: Precedence::Comparison };

    pub const EQ: Self = Self { tag: NodeTag::BinEq, precedence: Precedence::Equality };
    pub const NE: Self = Self { tag: NodeTag::BinNe, precedence: Precedence::Equality };

    pub const BITAND: Self = Self { tag: NodeTag::BinBitAnd, precedence: Precedence::BitAnd };
    pub const BITXOR: Self = Self { tag: NodeTag::BinBitXor, precedence: Precedence::BitXor };
    pub const BITOR: Self = Self { tag: NodeTag::BinBitOr, precedence: Precedence::BitOr };

    pub const AND: Self = Self { tag: NodeTag::BinAnd, precedence: Precedence::And };
    pub const OR: Self = Self { tag: NodeTag::BinOr, precedence: Precedence::Or };

    pub const COALESCE: Self = Self { tag: NodeTag::BinCoalesce, precedence: Precedence::Coalesce };
}

/// Map token to binary operator
fn token_to_binary_op(token: TokenKind) -> Option<BinaryOp> {
    Some(match token {
        TokenKind::Plus => BinaryOp::ADD,
        TokenKind::Minus => BinaryOp::SUB,
        TokenKind::Star => BinaryOp::MUL,
        TokenKind::Slash => BinaryOp::DIV,
        TokenKind::Percent => BinaryOp::MOD,
        TokenKind::Div => BinaryOp::FLOORDIV,

        TokenKind::LtLt => BinaryOp::SHL,
        TokenKind::GtGt => BinaryOp::SHR,

        TokenKind::DotDot => BinaryOp::RANGE,
        TokenKind::DotDotEq => BinaryOp::RANGE_INC,

        TokenKind::Lt => BinaryOp::LT,
        TokenKind::Le => BinaryOp::LE,
        TokenKind::Gt => BinaryOp::GT,
        TokenKind::Ge => BinaryOp::GE,

        TokenKind::EqEq => BinaryOp::EQ,
        TokenKind::BangEq => BinaryOp::NE,

        TokenKind::Amp => BinaryOp::BITAND,
        TokenKind::Caret => BinaryOp::BITXOR,
        TokenKind::Pipe => BinaryOp::BITOR,

        TokenKind::AmpAmp => BinaryOp::AND,
        TokenKind::PipePipe => BinaryOp::OR,

        TokenKind::QuestionQuestion => BinaryOp::COALESCE,

        _ => return None,
    })
}

/// Unary operators
#[derive(Clone, Copy, Debug)]
pub struct UnaryOp {
    pub tag: NodeTag,
}

impl UnaryOp {
    pub const NEG: Self = Self { tag: NodeTag::UnaryNeg };
    pub const NOT: Self = Self { tag: NodeTag::UnaryNot };
    pub const BITNOT: Self = Self { tag: NodeTag::UnaryBitNot };
}

fn token_to_unary_op(token: TokenKind) -> Option<UnaryOp> {
    Some(match token {
        TokenKind::Minus => UnaryOp::NEG,
        TokenKind::Bang => UnaryOp::NOT,
        TokenKind::Tilde => UnaryOp::BITNOT,
        _ => return None,
    })
}
```

## Core Pratt Parser Implementation

```rust
impl Parser<'_> {
    /// Parse an expression with minimum precedence
    ///
    /// This is the heart of the Pratt parser.
    pub fn parse_expr(&mut self) -> ParseResult<NodeId> {
        self.parse_expr_with_precedence(Precedence::None)
    }

    /// Parse expression with precedence threshold
    fn parse_expr_with_precedence(&mut self, min_prec: Precedence) -> ParseResult<NodeId> {
        // Parse prefix (unary operators + primary expressions)
        let mut left = self.parse_prefix()?;

        // Parse infix operators while their precedence is high enough
        while let Some(op) = self.peek_binary_op() {
            if op.precedence < min_prec {
                break;
            }

            let op_span = self.current_span();
            self.advance(); // Consume operator

            // Parse right operand with higher precedence (left-associative)
            let right = self.parse_expr_with_precedence(op.precedence.next())?;

            // Build binary node
            let span = self.storage.span(left).merge(self.storage.span(right));
            left = self.storage.alloc_binary(op.tag, span, left, right);
        }

        ParseResult::ok(Progress::Made, left)
    }

    /// Peek at current token as binary operator
    fn peek_binary_op(&self) -> Option<BinaryOp> {
        // Check context restrictions
        if self.context.contains(ParseContext::NO_STRUCT_LIT) {
            // In `if` condition, don't parse `{` as start of struct
            // (handled elsewhere, but restrict operators that could confuse)
        }

        token_to_binary_op(self.current_kind())
    }
}
```

## Prefix Parsing

```rust
impl Parser<'_> {
    /// Parse prefix expression (unary + postfix + primary)
    fn parse_prefix(&mut self) -> ParseResult<NodeId> {
        // Check for unary operators
        if let Some(op) = token_to_unary_op(self.current_kind()) {
            let start = self.current_span();
            self.advance();

            // Parse operand (recursively handles more unary ops)
            let operand = self.parse_prefix()?;

            let span = start.merge(self.storage.span(operand));
            let node = self.storage.alloc(op.tag, span, NodeData { node: operand });
            return ParseResult::ok(Progress::Made, node);
        }

        // Parse postfix expression (primary + postfix ops)
        self.parse_postfix()
    }
}
```

## Postfix Parsing

```rust
impl Parser<'_> {
    /// Parse postfix expression (primary followed by postfix operators)
    fn parse_postfix(&mut self) -> ParseResult<NodeId> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.current_kind() {
                // Field access: expr.field
                TokenKind::Dot => {
                    self.advance();

                    if self.at(TokenKind::Ident) {
                        let field_token = self.cursor.index();
                        let field_span = self.current_span();
                        self.advance();

                        // Check for method call: expr.method(args)
                        if self.at(TokenKind::LParen) {
                            expr = self.parse_method_call(expr, field_token)?;
                        } else {
                            // Field access
                            let span = self.storage.span(expr).merge(field_span);
                            expr = self.storage.alloc(
                                NodeTag::FieldAccess,
                                span,
                                NodeData { node_token: (expr, TokenIdx(field_token)) },
                            );
                        }
                    } else {
                        return self.error(ExprError::FieldName(self.position()));
                    }
                }

                // Indexing: expr[index]
                TokenKind::LBracket => {
                    let start = self.storage.span(expr);
                    self.advance();

                    let index = self.parse_expr()?;

                    if !self.expect(TokenKind::RBracket) {
                        return self.error(ExprError::IndexClose(self.position()));
                    }

                    let span = start.merge(self.current_span());
                    expr = self.storage.alloc(
                        NodeTag::Index,
                        span,
                        NodeData { node_pair: [expr, index] },
                    );
                }

                // Function call: expr(args)
                TokenKind::LParen => {
                    expr = self.parse_call(expr)?;
                }

                // Try propagation: expr?
                TokenKind::Question => {
                    let span = self.storage.span(expr).merge(self.current_span());
                    self.advance();
                    expr = self.storage.alloc(
                        NodeTag::TryPropagate,
                        span,
                        NodeData { node: expr },
                    );
                }

                _ => break,
            }
        }

        ParseResult::ok(Progress::Made, expr)
    }

    /// Parse function call arguments
    fn parse_call(&mut self, callee: NodeId) -> ParseResult<NodeId> {
        debug_assert!(self.at(TokenKind::LParen));
        self.advance();

        let mut args = Vec::new();

        if !self.at(TokenKind::RParen) {
            loop {
                // Parse argument: name: expr
                if self.at(TokenKind::Ident) && self.peek_next() == TokenKind::Colon {
                    let _name_token = self.cursor.index();
                    self.advance(); // name
                    self.advance(); // :
                    let value = self.parse_expr()?;
                    args.push(value);
                } else {
                    // Positional argument (for lambdas, type conversions)
                    let value = self.parse_expr()?;
                    args.push(value);
                }

                if !self.eat(TokenKind::Comma) {
                    break;
                }
            }
        }

        if !self.expect(TokenKind::RParen) {
            return self.error(ExprError::CallCloseParen(self.position()));
        }

        let span = self.storage.span(callee).merge(self.prev_span());
        let node = self.storage.alloc_call(span, callee, &args);
        ParseResult::ok(Progress::Made, node)
    }

    /// Parse method call: expr.method(args)
    fn parse_method_call(&mut self, receiver: NodeId, method_token: u32) -> ParseResult<NodeId> {
        debug_assert!(self.at(TokenKind::LParen));

        // Parse arguments
        let call = self.parse_call(receiver)?; // Reuse call parsing

        // Convert to method call
        let span = self.storage.span(call);
        let data = self.storage.data(call);

        // Rewrap as method call
        let node = self.storage.alloc(
            NodeTag::MethodCall,
            span,
            NodeData {
                // Store method token + call info
                // (implementation detail: method name + receiver + args)
                node_extra: (receiver, unsafe { data.extra_range }),
            },
        );

        ParseResult::ok(Progress::Made, node)
    }
}
```

## Primary Expressions

```rust
impl Parser<'_> {
    /// Parse primary expression (literals, identifiers, grouped expressions)
    fn parse_primary(&mut self) -> ParseResult<NodeId> {
        let span = self.current_span();

        match self.current_kind() {
            // Literals
            TokenKind::Int => {
                let index = self.cursor.index();
                self.advance();
                let node = self.storage.alloc(NodeTag::IntLit, span, NodeData {
                    index: index,
                });
                ParseResult::ok(Progress::Made, node)
            }

            TokenKind::Float => {
                let index = self.cursor.index();
                self.advance();
                let node = self.storage.alloc(NodeTag::FloatLit, span, NodeData {
                    index: index,
                });
                ParseResult::ok(Progress::Made, node)
            }

            TokenKind::String => {
                let index = self.cursor.index();
                self.advance();
                let node = self.storage.alloc(NodeTag::StringLit, span, NodeData {
                    index: index,
                });
                ParseResult::ok(Progress::Made, node)
            }

            TokenKind::True => {
                self.advance();
                let node = self.storage.alloc(NodeTag::BoolTrue, span, NodeData { none: () });
                ParseResult::ok(Progress::Made, node)
            }

            TokenKind::False => {
                self.advance();
                let node = self.storage.alloc(NodeTag::BoolFalse, span, NodeData { none: () });
                ParseResult::ok(Progress::Made, node)
            }

            // Identifiers
            TokenKind::Ident => {
                self.parse_ident_or_pattern_expr()
            }

            // Uppercase = variant constructor or type
            TokenKind::UpperIdent => {
                self.parse_variant_or_struct_literal()
            }

            // Grouped expression, tuple, unit, or lambda
            TokenKind::LParen => {
                self.parse_paren_expr()
            }

            // List literal
            TokenKind::LBracket => {
                self.parse_list_literal()
            }

            // Map literal
            TokenKind::LBrace => {
                self.parse_map_literal()
            }

            // Control flow
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Loop => self.parse_loop_expr(),
            TokenKind::For => self.parse_for_expr(),
            TokenKind::Let => self.parse_let_expr(),

            // Pattern expressions (soft keywords)
            _ if self.is_pattern_keyword() => {
                self.parse_pattern_expr()
            }

            _ => {
                self.expected.add(TokenKind::Int);
                self.expected.add(TokenKind::Ident);
                self.expected.add(TokenKind::LParen);
                ParseResult::err(Progress::None, ParseError::Expr(
                    ExprError::Start(self.position())
                ))
            }
        }
    }

    /// Check if current token is a pattern keyword (run, try, match, etc.)
    fn is_pattern_keyword(&self) -> bool {
        match self.current_kind() {
            TokenKind::Ident => {
                let text = self.current_text();
                matches!(text, "run" | "try" | "match" | "recurse" |
                              "parallel" | "spawn" | "timeout" | "cache" |
                              "with" | "catch")
                    && self.peek_next() == TokenKind::LParen
            }
            _ => false,
        }
    }
}
```

## Parenthesized Expression Disambiguation

One of the trickiest parts: `(x)` could be:
- Grouped expression: `(1 + 2)`
- Unit: `()`
- Tuple: `(1, 2)`
- Lambda: `(x) -> x + 1`
- Lambda params: `(x, y) -> x + y`

```rust
impl Parser<'_> {
    /// Parse parenthesized expression, handling all cases
    fn parse_paren_expr(&mut self) -> ParseResult<NodeId> {
        debug_assert!(self.at(TokenKind::LParen));
        let start = self.current_span();
        self.advance();

        // Unit: ()
        if self.at(TokenKind::RParen) {
            self.advance();
            let span = start.merge(self.prev_span());

            // Check for arrow -> this is lambda with no params
            if self.at(TokenKind::Arrow) {
                return self.parse_lambda_body(span, Vec::new());
            }

            let node = self.storage.alloc(NodeTag::Unit, span, NodeData { none: () });
            return ParseResult::ok(Progress::Made, node);
        }

        // Use tristate lookahead for disambiguation
        match self.is_lambda_params() {
            Tristate::True => {
                // Definitely lambda params
                let params = self.parse_lambda_params()?;
                self.expect(TokenKind::RParen);
                self.expect(TokenKind::Arrow);
                self.parse_lambda_body(start, params)
            }

            Tristate::False => {
                // Definitely expression (possibly tuple)
                self.parse_grouped_or_tuple(start)
            }

            Tristate::Unknown => {
                // Could be either - use speculation
                let snapshot = self.snapshot();

                // Try parsing as lambda params
                match self.parse_lambda_params() {
                    Ok(params) if self.at(TokenKind::RParen) => {
                        self.advance(); // )
                        if self.at(TokenKind::Arrow) {
                            self.advance(); // ->
                            return self.parse_lambda_body(start, params);
                        }
                    }
                    _ => {}
                }

                // Not a lambda, restore and parse as expression
                self.restore(snapshot);
                self.parse_grouped_or_tuple(start)
            }
        }
    }

    /// Tristate lookahead for lambda detection
    fn is_lambda_params(&self) -> Tristate {
        // Quick checks for definite cases
        match self.current_kind() {
            // (: or () -> definitely not a lambda body
            TokenKind::Colon | TokenKind::RParen => {
                // Need more context
            }

            // (ident: type -> definitely lambda with typed param
            TokenKind::Ident => {
                if self.peek_next() == TokenKind::Colon {
                    return Tristate::True;
                }
                // (ident) -> ... could be lambda
                // (ident, ...) -> ... could be lambda
                // (ident + ...) is expression
                if self.peek_next() == TokenKind::RParen {
                    // Check what follows )
                    return Tristate::Unknown;
                }
                if self.peek_next() == TokenKind::Comma {
                    return Tristate::Unknown;
                }
                // Has operator after ident -> expression
                return Tristate::False;
            }

            _ => return Tristate::False,
        }

        Tristate::Unknown
    }

    /// Parse grouped expression or tuple
    fn parse_grouped_or_tuple(&mut self, start: Span) -> ParseResult<NodeId> {
        let first = self.parse_expr()?;

        if self.at(TokenKind::Comma) {
            // Tuple
            let mut elements = vec![first];
            while self.eat(TokenKind::Comma) {
                if self.at(TokenKind::RParen) {
                    break; // Trailing comma
                }
                elements.push(self.parse_expr()?);
            }

            self.expect(TokenKind::RParen);
            let span = start.merge(self.prev_span());
            let node = self.alloc_tuple(span, &elements);
            return ParseResult::ok(Progress::Made, node);
        }

        // Simple grouped expression
        self.expect(TokenKind::RParen);

        // Check for arrow (lambda with single param parsed as expression)
        if self.at(TokenKind::Arrow) {
            // Convert expression to parameter
            if let Some(param) = self.expr_to_param(first) {
                self.advance(); // ->
                return self.parse_lambda_body(start, vec![param]);
            }
        }

        // Just a grouped expression - return inner expression directly
        ParseResult::ok(Progress::Made, first)
    }
}
```

## Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> NodeId {
        let tokens = tokenize(source);
        let mut parser = Parser::new(&tokens, source);
        parser.parse_expr().result.unwrap()
    }

    fn check_binary(storage: &NodeStorage, id: NodeId, expected_tag: NodeTag) {
        assert_eq!(storage.tag(id), expected_tag);
    }

    #[test]
    fn test_precedence_additive() {
        // 1 + 2 + 3 should be ((1 + 2) + 3) - left associative
        let storage = parse_to_storage("1 + 2 + 3");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinAdd);

        let (left, right) = storage.binary_children(root);
        assert_eq!(storage.tag(left), NodeTag::BinAdd); // (1 + 2)
        assert_eq!(storage.tag(right), NodeTag::IntLit); // 3
    }

    #[test]
    fn test_precedence_mul_over_add() {
        // 1 + 2 * 3 should be (1 + (2 * 3))
        let storage = parse_to_storage("1 + 2 * 3");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinAdd);

        let (left, right) = storage.binary_children(root);
        assert_eq!(storage.tag(left), NodeTag::IntLit); // 1
        assert_eq!(storage.tag(right), NodeTag::BinMul); // 2 * 3
    }

    #[test]
    fn test_unary_precedence() {
        // -1 + 2 should be ((-1) + 2)
        let storage = parse_to_storage("-1 + 2");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinAdd);

        let (left, _) = storage.binary_children(root);
        assert_eq!(storage.tag(left), NodeTag::UnaryNeg);
    }

    #[test]
    fn test_postfix_precedence() {
        // a.b.c should be ((a.b).c)
        let storage = parse_to_storage("a.b.c");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::FieldAccess);

        let inner = unsafe { storage.data(root).node_token.0 };
        assert_eq!(storage.tag(inner), NodeTag::FieldAccess);
    }

    #[test]
    fn test_call_chain() {
        // f()()(1) - curried calls
        let storage = parse_to_storage("f()()(x: 1)");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::FunctionCall);
    }

    #[test]
    fn test_comparison_chain() {
        // a < b < c should be (a < b) < c (left-associative)
        let storage = parse_to_storage("a < b < c");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinLt);

        let (left, _) = storage.binary_children(root);
        assert_eq!(storage.tag(left), NodeTag::BinLt);
    }

    #[test]
    fn test_range() {
        // 1..10 should parse as Range
        let storage = parse_to_storage("1..10");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinRange);
    }

    #[test]
    fn test_coalesce() {
        // a ?? b ?? c - lowest precedence binary
        let storage = parse_to_storage("a ?? b ?? c");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinCoalesce);

        let (left, _) = storage.binary_children(root);
        assert_eq!(storage.tag(left), NodeTag::BinCoalesce);
    }

    #[test]
    fn test_mixed_precedence() {
        // a || b && c | d ^ e & f == g < h + i * j
        // Should parse with correct precedence tree
        let storage = parse_to_storage("a || b && c | d ^ e & f == g < h + i * j");
        let root = storage.root();
        assert_eq!(storage.tag(root), NodeTag::BinOr); // || is lowest
    }
}
```

## Integration with Error Recovery

```rust
impl Parser<'_> {
    /// Parse expression with error recovery
    fn parse_expr_recovering(&mut self) -> ParseResult<NodeId> {
        match self.parse_expr() {
            ok @ ParseResult { result: Ok(_), .. } => ok,
            err @ ParseResult { progress: Progress::Made, .. } => {
                // Made progress but failed - record error and create placeholder
                self.record_error(err.result.unwrap_err());
                let node = self.storage.alloc(
                    NodeTag::Error,
                    self.current_span(),
                    NodeData { none: () },
                );
                ParseResult::ok(Progress::Made, node)
            }
            err => err, // No progress - propagate
        }
    }
}
```

## Summary

The Pratt parser provides:

1. **Correct precedence handling** with a simple loop
2. **Left associativity** via `prec.next()` for right operand
3. **Easy modification** - add operators by updating the table
4. **Constant stack depth** for binary operators
5. **Clean postfix handling** in a separate loop
6. **Tristate disambiguation** for lambda detection
7. **Error recovery** with placeholder nodes

This is the same approach used by Rust, V8, and many other production parsers.
