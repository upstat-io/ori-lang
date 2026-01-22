//! Parser for Sigil that produces a flattened AST.
//!
//! This is a recursive descent parser that:
//! - Allocates all expressions in an arena
//! - Uses indices instead of Box for children
//! - Handles error recovery to continue parsing after errors

use crate::intern::{Name, StringInterner};
use crate::errors::Diagnostic;
use super::{
    Token, TokenKind, TokenList, Span,
    ExprArena, Expr, ExprKind, ExprId, ExprRange, PatternKind,
    BinaryOp, UnaryOp,
    items::{Item, Import, Visibility},
    expr::{
        BindingPattern, MapEntry, Param, PatternArgs, PatternArg,
        TypeExpr, TypeExprKind,
    },
};

/// Parser state.
pub struct Parser<'src, 'i> {
    /// Token list from lexer.
    tokens: &'src TokenList,
    /// String interner.
    interner: &'i StringInterner,
    /// Expression arena.
    arena: ExprArena,
    /// Current token index.
    pos: usize,
    /// Collected diagnostics.
    diagnostics: Vec<Diagnostic>,
    /// Collected imports.
    imports: Vec<Import>,
    /// Collected items.
    items: Vec<Item>,
}

impl<'src, 'i> Parser<'src, 'i> {
    /// Create a new parser.
    pub fn new(tokens: &'src TokenList, interner: &'i StringInterner) -> Self {
        Parser {
            tokens,
            interner,
            arena: ExprArena::new(),
            pos: 0,
            diagnostics: Vec::new(),
            imports: Vec::new(),
            items: Vec::new(),
        }
    }

    /// Parse a complete module.
    pub fn parse_module(mut self) -> ParseResult {
        self.skip_newlines();

        while !self.at_end() {
            match self.parse_item() {
                Ok(item) => self.items.push(item),
                Err(diag) => {
                    self.diagnostics.push(diag);
                    self.recover_to_next_item();
                }
            }
            self.skip_newlines();
        }

        ParseResult {
            items: self.items,
            imports: self.imports,
            arena: self.arena,
            diagnostics: self.diagnostics,
        }
    }

    /// Parse a single expression (for REPL/testing).
    pub fn parse_expression(mut self) -> (ExprId, ExprArena, Vec<Diagnostic>) {
        self.skip_newlines();
        let expr = match self.expression() {
            Ok(id) => id,
            Err(diag) => {
                self.diagnostics.push(diag);
                self.arena.alloc(Expr::new(ExprKind::Error, self.current_span()))
            }
        };
        (expr, self.arena, self.diagnostics)
    }

    // ===== Token access =====

    fn current(&self) -> &Token {
        &self.tokens.tokens[self.pos.min(self.tokens.tokens.len() - 1)]
    }

    fn current_kind(&self) -> &TokenKind {
        &self.current().kind
    }

    fn current_span(&self) -> Span {
        self.current().span
    }

    fn at_end(&self) -> bool {
        matches!(self.current_kind(), TokenKind::Eof)
    }

    #[allow(dead_code)]
    fn peek(&self, offset: usize) -> &TokenKind {
        let idx = (self.pos + offset).min(self.tokens.tokens.len() - 1);
        &self.tokens.tokens[idx].kind
    }

    fn advance(&mut self) -> &Token {
        let _token = self.current();
        if !self.at_end() {
            self.pos += 1;
        }
        // Return reference from original position
        &self.tokens.tokens[self.pos - 1]
    }

    fn check(&self, kind: &TokenKind) -> bool {
        std::mem::discriminant(self.current_kind()) == std::mem::discriminant(kind)
    }

    fn consume(&mut self, kind: &TokenKind, msg: &str) -> Result<&Token, Diagnostic> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(self.error(msg))
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current_kind(), TokenKind::Newline) {
            self.advance();
        }
    }

    fn error(&self, msg: &str) -> Diagnostic {
        Diagnostic::error(msg.to_string(), self.current_span())
    }

    fn recover_to_next_item(&mut self) {
        while !self.at_end() {
            match self.current_kind() {
                TokenKind::At | TokenKind::Dollar | TokenKind::Type |
                TokenKind::Pub | TokenKind::Use | TokenKind::Trait |
                TokenKind::Impl | TokenKind::Extend => break,
                TokenKind::Newline => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    // ===== Item parsing =====

    fn parse_item(&mut self) -> Result<Item, Diagnostic> {
        let visibility = if self.check(&TokenKind::Pub) {
            self.advance();
            Visibility::Public
        } else {
            Visibility::Private
        };

        match self.current_kind() {
            TokenKind::At => self.parse_function(visibility),
            TokenKind::Dollar => self.parse_config(visibility),
            TokenKind::Type => self.parse_type_def(visibility),
            TokenKind::Use => self.parse_import(visibility),
            TokenKind::Trait => self.parse_trait(visibility),
            TokenKind::Impl => self.parse_impl(),
            _ => Err(self.error("expected item declaration")),
        }
    }

    fn parse_function(&mut self, visibility: Visibility) -> Result<Item, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::At, "expected '@'")?;

        let name = self.parse_name()?;
        let type_params = self.parse_type_params()?;

        self.consume(&TokenKind::LParen, "expected '('")?;
        let params = self.parse_params()?;
        self.consume(&TokenKind::RParen, "expected ')'")?;

        let return_type = if self.check(&TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        let capabilities = if self.check(&TokenKind::Uses) {
            self.advance();
            self.parse_capability_list()?
        } else {
            Vec::new()
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let body = self.expression()?;
        let span = start.merge(self.arena.get(body).span);

        Ok(Item {
            kind: super::items::ItemKind::Function(super::items::Function {
                name,
                visibility,
                type_params,
                params: self.arena.alloc_params(params),
                return_type: return_type.map(|t| self.arena.alloc_type_expr(t)),
                capabilities,
                body,
                is_async: false,
                sig_span: start,
            }),
            span,
        })
    }

    fn parse_config(&mut self, visibility: Visibility) -> Result<Item, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Dollar, "expected '$'")?;

        let name = self.parse_name()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let value = self.expression()?;
        let span = start.merge(self.arena.get(value).span);

        Ok(Item {
            kind: super::items::ItemKind::Config(super::items::Config {
                name,
                visibility,
                ty: ty.map(|t| self.arena.alloc_type_expr(t)),
                value,
            }),
            span,
        })
    }

    fn parse_type_def(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        // Placeholder - full type definition parsing
        Err(self.error("type definitions not yet implemented"))
    }

    fn parse_import(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        // Placeholder - full import parsing
        Err(self.error("imports not yet implemented"))
    }

    fn parse_trait(&mut self, _visibility: Visibility) -> Result<Item, Diagnostic> {
        // Placeholder - full trait parsing
        Err(self.error("traits not yet implemented"))
    }

    fn parse_impl(&mut self) -> Result<Item, Diagnostic> {
        // Placeholder - full impl parsing
        Err(self.error("impls not yet implemented"))
    }

    // ===== Expression parsing =====

    fn expression(&mut self) -> Result<ExprId, Diagnostic> {
        self.parse_precedence(14) // Lowest precedence (assignment level)
    }

    fn parse_precedence(&mut self, max_prec: u8) -> Result<ExprId, Diagnostic> {
        let mut left = self.unary()?;

        while let Some((op, prec)) = self.binary_op() {
            if prec > max_prec {
                break;
            }

            self.advance();
            self.skip_newlines();

            let right = if op.is_left_assoc() {
                self.parse_precedence(prec - 1)?
            } else {
                self.parse_precedence(prec)?
            };

            let span = self.arena.get(left).span.merge(self.arena.get(right).span);
            left = self.arena.alloc(Expr::new(
                ExprKind::Binary { op, left, right },
                span,
            ));
        }

        Ok(left)
    }

    fn binary_op(&self) -> Option<(BinaryOp, u8)> {
        let op = match self.current_kind() {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Mod,
            TokenKind::Div => BinaryOp::FloorDiv,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::NotEq => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::LtEq => BinaryOp::Le,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::GtEq => BinaryOp::Ge,
            TokenKind::AmpAmp => BinaryOp::And,
            TokenKind::PipePipe => BinaryOp::Or,
            TokenKind::Amp => BinaryOp::BitAnd,
            TokenKind::Pipe => BinaryOp::BitOr,
            TokenKind::Caret => BinaryOp::BitXor,
            TokenKind::DotDot => BinaryOp::Range,
            TokenKind::DotDotEq => BinaryOp::RangeInc,
            TokenKind::DoubleQuestion => BinaryOp::Coalesce,
            _ => return None,
        };
        Some((op, op.precedence()))
    }

    fn unary(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();

        let op = match self.current_kind() {
            TokenKind::Bang => Some(UnaryOp::Not),
            TokenKind::Minus => Some(UnaryOp::Neg),
            TokenKind::Tilde => Some(UnaryOp::BitNot),
            _ => None,
        };

        if let Some(op) = op {
            self.advance();
            let operand = self.unary()?;
            let span = start.merge(self.arena.get(operand).span);
            Ok(self.arena.alloc(Expr::new(
                ExprKind::Unary { op, operand },
                span,
            )))
        } else {
            self.postfix()
        }
    }

    fn postfix(&mut self) -> Result<ExprId, Diagnostic> {
        let mut expr = self.primary()?;

        loop {
            match self.current_kind() {
                TokenKind::Dot => {
                    self.advance();
                    let field = self.parse_name()?;

                    // Check for method call
                    if self.check(&TokenKind::LParen) {
                        self.advance();
                        let args = self.parse_call_args()?;
                        self.consume(&TokenKind::RParen, "expected ')'")?;
                        let span = self.arena.get(expr).span.merge(self.current_span());
                        expr = self.arena.alloc(Expr::new(
                            ExprKind::MethodCall {
                                receiver: expr,
                                method: field,
                                args,
                            },
                            span,
                        ));
                    } else {
                        let span = self.arena.get(expr).span.merge(self.current_span());
                        expr = self.arena.alloc(Expr::new(
                            ExprKind::Field { receiver: expr, field },
                            span,
                        ));
                    }
                }
                TokenKind::LBracket => {
                    self.advance();
                    let index = self.expression()?;
                    self.consume(&TokenKind::RBracket, "expected ']'")?;
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(
                        ExprKind::Index { receiver: expr, index },
                        span,
                    ));
                }
                TokenKind::LParen => {
                    self.advance();
                    let args = self.parse_call_args()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(
                        ExprKind::Call { func: expr, args },
                        span,
                    ));
                }
                TokenKind::Question => {
                    self.advance();
                    let span = self.arena.get(expr).span.merge(self.current_span());
                    expr = self.arena.alloc(Expr::new(ExprKind::Try(expr), span));
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn primary(&mut self) -> Result<ExprId, Diagnostic> {
        let span = self.current_span();

        match self.current_kind().clone() {
            // Literals
            TokenKind::Int(n) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Int(n), span)))
            }
            TokenKind::Float(bits) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Float(f64::from_bits(bits)), span)))
            }
            TokenKind::String(name) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::String(name), span)))
            }
            TokenKind::Char(c) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Char(c), span)))
            }
            TokenKind::True => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Bool(true), span)))
            }
            TokenKind::False => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Bool(false), span)))
            }
            TokenKind::Duration(value, unit) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Duration { value, unit },
                    span,
                )))
            }

            // Identifiers and references
            TokenKind::Ident(name) => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::Ident(name), span)))
            }
            TokenKind::At => {
                self.advance();
                let name = self.parse_name()?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::FunctionRef(name),
                    span.merge(end_span),
                )))
            }
            TokenKind::Dollar => {
                self.advance();
                let name = self.parse_name()?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Config(name),
                    span.merge(end_span),
                )))
            }
            TokenKind::SelfLower => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::SelfRef, span)))
            }
            TokenKind::Hash => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::HashLength, span)))
            }

            // Constructors
            TokenKind::Ok => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.expression()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Some(expr)
                } else {
                    None
                };
                Ok(self.arena.alloc(Expr::new(ExprKind::Ok(inner), span)))
            }
            TokenKind::Err => {
                self.advance();
                let inner = if self.check(&TokenKind::LParen) {
                    self.advance();
                    let expr = self.expression()?;
                    self.consume(&TokenKind::RParen, "expected ')'")?;
                    Some(expr)
                } else {
                    None
                };
                Ok(self.arena.alloc(Expr::new(ExprKind::Err(inner), span)))
            }
            TokenKind::Some => {
                self.advance();
                self.consume(&TokenKind::LParen, "expected '('")?;
                let inner = self.expression()?;
                self.consume(&TokenKind::RParen, "expected ')'")?;
                let end_span = self.current_span();
                Ok(self.arena.alloc(Expr::new(
                    ExprKind::Some(inner),
                    span.merge(end_span),
                )))
            }
            TokenKind::None => {
                self.advance();
                Ok(self.arena.alloc(Expr::new(ExprKind::None, span)))
            }

            // Control flow
            TokenKind::If => self.parse_if(),
            TokenKind::For => self.parse_for(),
            TokenKind::Loop => self.parse_loop(),
            TokenKind::Let => self.parse_let(),

            // Pattern expressions
            TokenKind::Run => self.parse_pattern(PatternKind::Run),
            TokenKind::Try => self.parse_pattern(PatternKind::Try),
            TokenKind::Match => self.parse_pattern(PatternKind::Match),
            TokenKind::Map => self.parse_pattern(PatternKind::Map),
            TokenKind::Filter => self.parse_pattern(PatternKind::Filter),
            TokenKind::Fold => self.parse_pattern(PatternKind::Fold),
            TokenKind::Find => self.parse_pattern(PatternKind::Find),
            TokenKind::Collect => self.parse_pattern(PatternKind::Collect),
            TokenKind::Recurse => self.parse_pattern(PatternKind::Recurse),
            TokenKind::Parallel => self.parse_pattern(PatternKind::Parallel),
            TokenKind::Timeout => self.parse_pattern(PatternKind::Timeout),
            TokenKind::Retry => self.parse_pattern(PatternKind::Retry),
            TokenKind::Cache => self.parse_pattern(PatternKind::Cache),
            TokenKind::Validate => self.parse_pattern(PatternKind::Validate),

            // Collections
            TokenKind::LBracket => self.parse_list(),
            TokenKind::LBrace => self.parse_map_or_struct(),
            TokenKind::LParen => self.parse_paren_or_tuple(),

            _ => Err(self.error("expected expression")),
        }
    }

    fn parse_if(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::If, "expected 'if'")?;

        let cond = self.expression()?;
        self.consume(&TokenKind::Then, "expected 'then'")?;
        self.skip_newlines();

        let then_branch = self.expression()?;

        let else_branch = if self.check(&TokenKind::Else) {
            self.advance();
            self.skip_newlines();
            Some(self.expression()?)
        } else {
            None
        };

        let end_span = else_branch
            .map(|e| self.arena.get(e).span)
            .unwrap_or(self.arena.get(then_branch).span);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::If { cond, then_branch, else_branch },
            start.merge(end_span),
        )))
    }

    fn parse_for(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::For, "expected 'for'")?;

        let binding = self.parse_name()?;
        self.consume(&TokenKind::In, "expected 'in'")?;
        let iter = self.expression()?;

        // Optional guard: if cond
        let guard = if self.check(&TokenKind::If) {
            self.advance();
            Some(self.expression()?)
        } else {
            None
        };

        // do or yield
        let is_yield = if self.check(&TokenKind::Do) {
            self.advance();
            false
        } else if self.check(&TokenKind::Yield) {
            self.advance();
            true
        } else {
            return Err(self.error("expected 'do' or 'yield'"));
        };

        self.skip_newlines();
        let body = self.expression()?;
        let end_span = self.arena.get(body).span;

        Ok(self.arena.alloc(Expr::new(
            ExprKind::For { binding, iter, guard, body, is_yield },
            start.merge(end_span),
        )))
    }

    fn parse_loop(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Loop, "expected 'loop'")?;
        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        let body = self.expression()?;

        self.skip_newlines();
        self.consume(&TokenKind::RParen, "expected ')'")?;

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Loop { body },
            start.merge(self.current_span()),
        )))
    }

    fn parse_let(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::Let, "expected 'let'")?;

        let mutable = if self.check(&TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };

        let pattern = self.parse_binding_pattern()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type_expr()?)
        } else {
            None
        };

        self.consume(&TokenKind::Eq, "expected '='")?;
        self.skip_newlines();

        let init = self.expression()?;
        let end_span = self.arena.get(init).span;

        let ty_id = ty.map(|t| self.arena.alloc_type_expr(t));
        Ok(self.arena.alloc(Expr::new(
            ExprKind::Let {
                pattern,
                ty: ty_id,
                init,
                mutable,
            },
            start.merge(end_span),
        )))
    }

    fn parse_pattern(&mut self, kind: PatternKind) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.advance(); // Consume pattern keyword

        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        let args = self.parse_pattern_args(kind)?;

        self.skip_newlines();
        self.consume(&TokenKind::RParen, "expected ')'")?;

        let args_id = self.arena.alloc_pattern_args(args);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Pattern { kind, args: args_id },
            start.merge(self.current_span()),
        )))
    }

    fn parse_pattern_args(&mut self, kind: PatternKind) -> Result<PatternArgs, Diagnostic> {
        let start = self.current_span();
        let mut named = Vec::new();
        let mut positional = Vec::new();

        // For 'run' pattern, arguments are positional
        let is_positional = matches!(kind, PatternKind::Run);

        loop {
            if self.check(&TokenKind::RParen) {
                break;
            }

            if is_positional || !self.check(&TokenKind::Dot) {
                // Positional argument
                let expr = self.expression()?;
                positional.push(expr);
            } else {
                // Named argument: .name: value
                self.consume(&TokenKind::Dot, "expected '.'")?;
                let name = self.parse_name()?;
                self.consume(&TokenKind::Colon, "expected ':'")?;
                self.skip_newlines();
                let value = self.expression()?;
                let arg_span = start.merge(self.arena.get(value).span);
                named.push(PatternArg { name, value, span: arg_span });
            }

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        let positional_range = self.arena.alloc_expr_list(positional);

        Ok(PatternArgs {
            named,
            positional: positional_range,
            span: start.merge(self.current_span()),
        })
    }

    fn parse_list(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LBracket, "expected '['")?;
        self.skip_newlines();

        let mut elements = Vec::new();
        while !self.check(&TokenKind::RBracket) && !self.at_end() {
            elements.push(self.expression()?);

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        self.consume(&TokenKind::RBracket, "expected ']'")?;
        let range = self.arena.alloc_expr_list(elements);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::List(range),
            start.merge(self.current_span()),
        )))
    }

    fn parse_map_or_struct(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LBrace, "expected '{'")?;
        self.skip_newlines();

        if self.check(&TokenKind::RBrace) {
            self.advance();
            return Ok(self.arena.alloc(Expr::new(
                ExprKind::Map(super::MapEntryRange::EMPTY),
                start.merge(self.current_span()),
            )));
        }

        // Look ahead to determine if this is a map or struct field init
        // Map: { key: value, ... }
        // Struct: { field, field: value, ... }

        let mut entries = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.at_end() {
            let key = self.expression()?;

            if self.check(&TokenKind::Colon) {
                self.advance();
                self.skip_newlines();
                let value = self.expression()?;
                let entry_span = self.arena.get(key).span.merge(self.arena.get(value).span);
                entries.push(MapEntry { key, value, span: entry_span });
            } else {
                // Shorthand: { x } means { x: x }
                let span = self.arena.get(key).span;
                entries.push(MapEntry { key, value: key, span });
            }

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        self.consume(&TokenKind::RBrace, "expected '}'")?;
        let range = self.arena.alloc_map_entries(entries);

        Ok(self.arena.alloc(Expr::new(
            ExprKind::Map(range),
            start.merge(self.current_span()),
        )))
    }

    fn parse_paren_or_tuple(&mut self) -> Result<ExprId, Diagnostic> {
        let start = self.current_span();
        self.consume(&TokenKind::LParen, "expected '('")?;
        self.skip_newlines();

        // Empty tuple: ()
        if self.check(&TokenKind::RParen) {
            self.advance();
            return Ok(self.arena.alloc(Expr::new(
                ExprKind::Unit,
                start.merge(self.current_span()),
            )));
        }

        let first = self.expression()?;

        // Check for lambda: (params) -> body
        // Check for tuple: (a, b, c)
        // Otherwise: grouping (a)

        if self.check(&TokenKind::Comma) {
            // Tuple
            let mut elements = vec![first];
            while self.check(&TokenKind::Comma) {
                self.advance();
                self.skip_newlines();
                if self.check(&TokenKind::RParen) {
                    break;
                }
                elements.push(self.expression()?);
            }
            self.consume(&TokenKind::RParen, "expected ')'")?;
            let range = self.arena.alloc_expr_list(elements);
            Ok(self.arena.alloc(Expr::new(
                ExprKind::Tuple(range),
                start.merge(self.current_span()),
            )))
        } else {
            // Grouping
            self.consume(&TokenKind::RParen, "expected ')'")?;
            Ok(first)
        }
    }

    fn parse_call_args(&mut self) -> Result<ExprRange, Diagnostic> {
        self.skip_newlines();
        let mut args = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.at_end() {
            args.push(self.expression()?);

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        Ok(self.arena.alloc_expr_list(args))
    }

    // ===== Helper parsers =====

    fn parse_name(&mut self) -> Result<Name, Diagnostic> {
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            // Allow pattern keywords as identifiers in non-pattern context
            TokenKind::Map | TokenKind::Filter | TokenKind::Fold |
            TokenKind::Run | TokenKind::Try | TokenKind::Find |
            TokenKind::Collect | TokenKind::Recurse | TokenKind::Parallel |
            TokenKind::Timeout | TokenKind::Retry | TokenKind::Cache |
            TokenKind::Validate => {
                let name = self.interner.intern(self.current_kind().display_name());
                self.advance();
                Ok(name)
            }
            _ => Err(self.error("expected identifier")),
        }
    }

    fn parse_type_params(&mut self) -> Result<Vec<super::items::TypeParam>, Diagnostic> {
        if !self.check(&TokenKind::Lt) {
            return Ok(Vec::new());
        }

        self.advance();
        let mut params = Vec::new();

        loop {
            let name = self.parse_name()?;
            let start = self.current_span();

            let bounds = if self.check(&TokenKind::Colon) {
                self.advance();
                self.parse_type_bounds()?
            } else {
                Vec::new()
            };

            params.push(super::items::TypeParam {
                name,
                bounds,
                default: None,
                span: start,
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        self.consume(&TokenKind::Gt, "expected '>'")?;
        Ok(params)
    }

    fn parse_type_bounds(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        let mut bounds = vec![self.parse_type_expr()?];

        while self.check(&TokenKind::Plus) {
            self.advance();
            bounds.push(self.parse_type_expr()?);
        }

        Ok(bounds)
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, Diagnostic> {
        let mut params = Vec::new();

        while !self.check(&TokenKind::RParen) && !self.at_end() {
            let start = self.current_span();
            let name = self.parse_name()?;

            let ty = if self.check(&TokenKind::Colon) {
                self.advance();
                let type_expr = self.parse_type_expr()?;
                Some(self.arena.alloc_type_expr(type_expr))
            } else {
                None
            };

            let default = if self.check(&TokenKind::Eq) {
                self.advance();
                Some(self.expression()?)
            } else {
                None
            };

            params.push(Param {
                name,
                ty,
                default,
                span: start.merge(self.current_span()),
            });

            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
            self.skip_newlines();
        }

        Ok(params)
    }

    fn parse_capability_list(&mut self) -> Result<Vec<Name>, Diagnostic> {
        let mut caps = vec![self.parse_name()?];

        while self.check(&TokenKind::Comma) {
            self.advance();
            caps.push(self.parse_name()?);
        }

        Ok(caps)
    }

    fn parse_type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        let span = self.current_span();

        match self.current_kind().clone() {
            TokenKind::IntType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("int"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::FloatType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("float"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::BoolType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("bool"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::StrType => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("str"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::Void => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Named {
                        name: self.interner.intern("void"),
                        type_args: Vec::new(),
                    },
                    span,
                })
            }
            TokenKind::Ident(name) => {
                self.advance();
                let type_args = if self.check(&TokenKind::Lt) {
                    self.parse_type_args()?
                } else {
                    Vec::new()
                };
                Ok(TypeExpr {
                    kind: TypeExprKind::Named { name, type_args },
                    span: span.merge(self.current_span()),
                })
            }
            TokenKind::LBracket => {
                self.advance();
                let inner = self.parse_type_expr()?;
                self.consume(&TokenKind::RBracket, "expected ']'")?;
                Ok(TypeExpr {
                    kind: TypeExprKind::List(Box::new(inner)),
                    span: span.merge(self.current_span()),
                })
            }
            TokenKind::LParen => {
                self.advance();
                let mut types = Vec::new();

                while !self.check(&TokenKind::RParen) && !self.at_end() {
                    types.push(self.parse_type_expr()?);
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RParen, "expected ')'")?;

                // Check for function type
                if self.check(&TokenKind::Arrow) {
                    self.advance();
                    let ret = self.parse_type_expr()?;
                    Ok(TypeExpr {
                        kind: TypeExprKind::Function {
                            params: types,
                            ret: Box::new(ret),
                        },
                        span: span.merge(self.current_span()),
                    })
                } else {
                    Ok(TypeExpr {
                        kind: TypeExprKind::Tuple(types),
                        span: span.merge(self.current_span()),
                    })
                }
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(TypeExpr {
                    kind: TypeExprKind::Infer,
                    span,
                })
            }
            _ => Err(self.error("expected type")),
        }
    }

    fn parse_type_args(&mut self) -> Result<Vec<TypeExpr>, Diagnostic> {
        self.consume(&TokenKind::Lt, "expected '<'")?;
        let mut args = Vec::new();

        loop {
            args.push(self.parse_type_expr()?);
            if !self.check(&TokenKind::Comma) {
                break;
            }
            self.advance();
        }

        self.consume(&TokenKind::Gt, "expected '>'")?;
        Ok(args)
    }

    fn parse_binding_pattern(&mut self) -> Result<BindingPattern, Diagnostic> {
        match self.current_kind().clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(BindingPattern::Name(name))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(BindingPattern::Wildcard)
            }
            TokenKind::LParen => {
                self.advance();
                let mut patterns = Vec::new();

                while !self.check(&TokenKind::RParen) && !self.at_end() {
                    patterns.push(self.parse_binding_pattern()?);
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RParen, "expected ')'")?;
                Ok(BindingPattern::Tuple(patterns))
            }
            TokenKind::LBrace => {
                self.advance();
                let mut fields = Vec::new();

                while !self.check(&TokenKind::RBrace) && !self.at_end() {
                    let name = self.parse_name()?;
                    let pattern = if self.check(&TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_binding_pattern()?)
                    } else {
                        None
                    };
                    fields.push((name, pattern));

                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }

                self.consume(&TokenKind::RBrace, "expected '}'")?;
                Ok(BindingPattern::Struct { fields })
            }
            _ => Err(self.error("expected binding pattern")),
        }
    }
}

/// Result of parsing a module.
pub struct ParseResult {
    /// Top-level items.
    pub items: Vec<Item>,
    /// Import declarations.
    pub imports: Vec<Import>,
    /// Expression arena.
    pub arena: ExprArena,
    /// Parse diagnostics.
    pub diagnostics: Vec<Diagnostic>,
}
