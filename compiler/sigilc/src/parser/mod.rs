// Parser for Sigil
// Converts tokens into AST

use crate::ast::*;
use crate::lexer::{SpannedToken, Token};

pub struct Parser {
    tokens: Vec<SpannedToken>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<SpannedToken>) -> Self {
        Parser { tokens, pos: 0 }
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.value)
    }

    fn peek(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset).map(|t| &t.value)
    }

    fn advance(&mut self) -> Option<&SpannedToken> {
        if self.pos < self.tokens.len() {
            let token = &self.tokens[self.pos];
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: Token) -> Result<&SpannedToken, String> {
        match self.current() {
            Some(t) if *t == expected => {
                Ok(self.advance().unwrap())
            }
            Some(t) => Err(format!("Expected {:?}, found {:?}", expected, t)),
            None => Err(format!("Expected {:?}, found end of input", expected)),
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current(), Some(Token::Newline)) {
            self.advance();
        }
    }

    /// Try to get an identifier from the current token.
    /// This treats certain keywords as valid identifiers for context-sensitive parsing.
    /// Used for function names after @ where keywords like type names should be valid.
    fn try_get_ident(&self) -> Option<String> {
        match self.current()? {
            Token::Ident(n) => Some(n.clone()),
            // Type keywords - valid as function names
            Token::IntType => Some("int".to_string()),
            Token::FloatType => Some("float".to_string()),
            Token::StrType => Some("str".to_string()),
            Token::BoolType => Some("bool".to_string()),
            // Other keywords that could be function names
            Token::Assert => Some("assert".to_string()),
            Token::AssertErr => Some("assert_err".to_string()),
            Token::Tests => Some("tests".to_string()),
            _ => None,
        }
    }

    pub fn parse_module(&mut self, name: &str) -> Result<Module, String> {
        let mut items = Vec::new();

        self.skip_newlines();

        while self.current().is_some() {
            self.skip_newlines();
            if self.current().is_none() {
                break;
            }

            let item = self.parse_item()?;
            items.push(item);

            self.skip_newlines();
        }

        Ok(Module {
            name: name.to_string(),
            items,
        })
    }

    fn parse_item(&mut self) -> Result<Item, String> {
        match self.current() {
            Some(Token::Dollar) => self.parse_config().map(Item::Config),
            Some(Token::At) => self.parse_function_or_test(false),
            Some(Token::Pub) => {
                self.advance();
                match self.current() {
                    Some(Token::At) => self.parse_function_or_test(true),
                    Some(Token::Type) => self.parse_type_def(true).map(Item::TypeDef),
                    Some(Token::Use) => self.parse_use().map(Item::Use),
                    _ => Err("Expected @, type, or use after pub".to_string()),
                }
            }
            Some(Token::Type) => self.parse_type_def(false).map(Item::TypeDef),
            Some(Token::Use) => self.parse_use().map(Item::Use),
            Some(t) => Err(format!("Unexpected token at top level: {:?}", t)),
            None => Err("Unexpected end of input".to_string()),
        }
    }

    fn parse_function_or_test(&mut self, public: bool) -> Result<Item, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::At)?;

        let name = match self.try_get_ident() {
            Some(n) => {
                self.advance();
                n
            }
            None => return Err("Expected name after @".to_string()),
        };

        // Check if this is a test definition
        if matches!(self.current(), Some(Token::Tests)) {
            self.advance(); // consume 'tests'
            return self.parse_test(name, start);
        }

        // Otherwise, parse as function
        self.parse_function_rest(public, name, start).map(Item::Function)
    }

    fn parse_test(&mut self, name: String, start: usize) -> Result<Item, String> {
        // Expect @target
        self.expect(Token::At)?;
        let target = match self.try_get_ident() {
            Some(n) => {
                self.advance();
                n
            }
            None => return Err("Expected target function name after 'tests @'".to_string()),
        };

        // Parameters (typically empty for tests)
        self.expect(Token::LParen)?;
        self.parse_params()?; // ignore params for tests
        self.expect(Token::RParen)?;

        // Return type (should be void)
        self.expect(Token::Arrow)?;
        self.parse_type()?; // ignore, should be void

        // Body
        self.expect(Token::Eq)?;
        let body = self.parse_expr()?;

        let end = self.tokens.get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(Item::Test(TestDef {
            name,
            target,
            body,
            span: start..end,
        }))
    }

    fn parse_config(&mut self) -> Result<ConfigDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Dollar)?;

        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected identifier after $".to_string()),
        };

        // Optional type annotation
        let ty = if matches!(self.current(), Some(Token::Colon)) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(Token::Eq)?;

        let value = self.parse_expr()?;
        let end = self.tokens.get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(ConfigDef {
            name,
            ty,
            value,
            span: start..end,
        })
    }

    fn parse_function_rest(&mut self, public: bool, name: String, start: usize) -> Result<FunctionDef, String> {
        // Optional type parameters
        let type_params = if matches!(self.current(), Some(Token::Ident(_))) {
            // Check if next is also an identifier (type param) or LParen (params)
            let mut params = Vec::new();
            while let Some(Token::Ident(p)) = self.current() {
                if matches!(self.peek(1), Some(Token::Colon) | Some(Token::LParen)) {
                    break;
                }
                params.push(p.clone());
                self.advance();
            }
            params
        } else {
            Vec::new()
        };

        // Parameters
        self.expect(Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(Token::RParen)?;

        // Return type
        self.expect(Token::Arrow)?;
        let return_type = self.parse_type()?;

        // Body (skip newlines to allow multi-line expressions)
        self.expect(Token::Eq)?;
        self.skip_newlines();
        let body = self.parse_expr()?;

        let end = self.tokens.get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(FunctionDef {
            public,
            name,
            type_params,
            params,
            return_type,
            body,
            span: start..end,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();

        while !matches!(self.current(), Some(Token::RParen)) {
            let name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected parameter name".to_string()),
            };

            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;

            params.push(Param { name, ty });

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(params)
    }

    fn parse_type(&mut self) -> Result<TypeExpr, String> {
        // Optional type: ?T
        if matches!(self.current(), Some(Token::Question)) {
            self.advance();
            let inner = self.parse_type()?;
            return Ok(TypeExpr::Optional(Box::new(inner)));
        }

        // List type: [T]
        if matches!(self.current(), Some(Token::LBracket)) {
            self.advance();
            let inner = self.parse_type()?;
            self.expect(Token::RBracket)?;
            return Ok(TypeExpr::List(Box::new(inner)));
        }

        // Record type: { field: type, ... } or Map type: { K: V }
        if matches!(self.current(), Some(Token::LBrace)) {
            self.advance();
            self.skip_newlines();

            // Empty braces
            if matches!(self.current(), Some(Token::RBrace)) {
                self.advance();
                return Ok(TypeExpr::Record(vec![]));
            }

            // Check if first element looks like "ident: type" (record) or "type: type" (map)
            // If we see an identifier followed by colon, it's a record
            if let Some(Token::Ident(field_name)) = self.current().cloned() {
                if matches!(self.peek(1), Some(Token::Colon)) {
                    // Record type: { field1: type1, field2: type2, ... }
                    let mut fields = Vec::new();
                    loop {
                        self.skip_newlines();
                        if matches!(self.current(), Some(Token::RBrace)) {
                            break;
                        }
                        // Parse field name
                        let fname = match self.current() {
                            Some(Token::Ident(n)) => {
                                let n = n.clone();
                                self.advance();
                                n
                            }
                            _ => return Err("Expected field name in record type".to_string()),
                        };
                        self.expect(Token::Colon)?;
                        let ftype = self.parse_type()?;
                        fields.push((fname, ftype));

                        self.skip_newlines();
                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    self.skip_newlines();
                    self.expect(Token::RBrace)?;
                    return Ok(TypeExpr::Record(fields));
                }
            }

            // Map type: { K: V }
            let key = self.parse_type()?;
            self.expect(Token::Colon)?;
            let value = self.parse_type()?;
            self.expect(Token::RBrace)?;
            return Ok(TypeExpr::Map(Box::new(key), Box::new(value)));
        }

        // Parenthesized type or tuple type: (T) or (T, U)
        if matches!(self.current(), Some(Token::LParen)) {
            self.advance();
            let mut types = Vec::new();
            while !matches!(self.current(), Some(Token::RParen)) {
                types.push(self.parse_type()?);
                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::RParen)?;

            // Check if this is a function type with tuple params: (T, U) -> V
            if matches!(self.current(), Some(Token::Arrow)) {
                self.advance();
                let ret = self.parse_type()?;
                let param_type = if types.len() == 1 {
                    types.into_iter().next().unwrap()
                } else {
                    TypeExpr::Tuple(types)
                };
                return Ok(TypeExpr::Function(Box::new(param_type), Box::new(ret)));
            }

            // Single-element parens are grouping, not tuple
            if types.len() == 1 {
                return Ok(types.into_iter().next().unwrap());
            }

            return Ok(TypeExpr::Tuple(types));
        }

        // Named type
        let name = match self.current() {
            Some(Token::IntType) => { self.advance(); "int".to_string() }
            Some(Token::FloatType) => { self.advance(); "float".to_string() }
            Some(Token::StrType) => { self.advance(); "str".to_string() }
            Some(Token::BoolType) => { self.advance(); "bool".to_string() }
            Some(Token::VoidType) => { self.advance(); "void".to_string() }
            Some(Token::ResultType) => { self.advance(); "Result".to_string() }
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected type".to_string()),
        };

        // Check for generic type arguments
        if matches!(self.current(), Some(Token::Ident(_)) | Some(Token::IntType) |
                    Some(Token::StrType) | Some(Token::BoolType) | Some(Token::Question) |
                    Some(Token::LBracket)) {
            // Could be generic arguments like `Result T E`
            let mut args = Vec::new();
            while matches!(self.current(), Some(Token::Ident(_)) | Some(Token::IntType) |
                          Some(Token::StrType) | Some(Token::BoolType) | Some(Token::Question) |
                          Some(Token::LBracket)) {
                // Stop if we hit something that looks like a new declaration
                if matches!(self.peek(1), Some(Token::Colon) | Some(Token::Arrow) | Some(Token::Eq)) {
                    break;
                }
                args.push(self.parse_type()?);
            }
            if !args.is_empty() {
                return Ok(TypeExpr::Generic(name, args));
            }
        }

        // Function type: T -> U
        if matches!(self.current(), Some(Token::Arrow)) {
            self.advance();
            let ret = self.parse_type()?;
            return Ok(TypeExpr::Function(
                Box::new(TypeExpr::Named(name)),
                Box::new(ret),
            ));
        }

        Ok(TypeExpr::Named(name))
    }

    fn parse_type_def(&mut self, public: bool) -> Result<TypeDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Type)?;

        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected type name".to_string()),
        };

        // Optional type parameters
        let mut params = Vec::new();
        while let Some(Token::Ident(p)) = self.current() {
            if matches!(self.peek(1), Some(Token::Eq) | Some(Token::LBrace) | Some(Token::Pipe)) {
                break;
            }
            params.push(p.clone());
            self.advance();
        }

        let kind = if matches!(self.current(), Some(Token::Eq)) {
            self.advance();
            // Could be alias or enum
            if matches!(self.current(), Some(Token::Pipe)) {
                // Enum
                self.parse_enum_variants()?
            } else {
                // Alias
                TypeDefKind::Alias(self.parse_type()?)
            }
        } else if matches!(self.current(), Some(Token::LBrace)) {
            // Struct
            self.parse_struct_fields()?
        } else if matches!(self.current(), Some(Token::Pipe)) {
            // Enum without =
            self.parse_enum_variants()?
        } else {
            return Err("Expected =, {, or | after type name".to_string());
        };

        let end = self.tokens.get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(TypeDef {
            public,
            name,
            params,
            kind,
            span: start..end,
        })
    }

    fn parse_struct_fields(&mut self) -> Result<TypeDefKind, String> {
        self.expect(Token::LBrace)?;
        let mut fields = Vec::new();

        self.skip_newlines();
        while !matches!(self.current(), Some(Token::RBrace)) {
            let name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected field name".to_string()),
            };

            self.expect(Token::Colon)?;
            let ty = self.parse_type()?;

            fields.push(Field { name, ty });

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            }
        }

        self.expect(Token::RBrace)?;
        Ok(TypeDefKind::Struct(fields))
    }

    fn parse_enum_variants(&mut self) -> Result<TypeDefKind, String> {
        let mut variants = Vec::new();

        while matches!(self.current(), Some(Token::Pipe)) {
            self.advance();
            self.skip_newlines();

            let name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected variant name".to_string()),
            };

            let fields = if matches!(self.current(), Some(Token::LBrace)) {
                self.advance();
                let mut fields = Vec::new();
                while !matches!(self.current(), Some(Token::RBrace)) {
                    let fname = match self.current() {
                        Some(Token::Ident(n)) => {
                            let n = n.clone();
                            self.advance();
                            n
                        }
                        _ => return Err("Expected field name in variant".to_string()),
                    };
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    fields.push(Field { name: fname, ty });

                    if matches!(self.current(), Some(Token::Comma)) {
                        self.advance();
                    }
                }
                self.expect(Token::RBrace)?;
                fields
            } else {
                Vec::new()
            };

            variants.push(Variant { name, fields });
            self.skip_newlines();
        }

        Ok(TypeDefKind::Enum(variants))
    }

    fn parse_use(&mut self) -> Result<UseDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Use)?;

        // Parse path: types.user
        let mut path = Vec::new();
        loop {
            let segment = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected identifier in use path".to_string()),
            };
            path.push(segment);

            if matches!(self.current(), Some(Token::Dot)) {
                self.advance();
            } else {
                break;
            }
        }

        // Parse items: { Item1, Item2 as Alias }
        self.expect(Token::LBrace)?;
        let mut items = Vec::new();

        while !matches!(self.current(), Some(Token::RBrace)) {
            let name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                Some(Token::Star) => {
                    self.advance();
                    "*".to_string()
                }
                _ => return Err("Expected identifier in use items".to_string()),
            };

            let alias = if let Some(Token::Ident(s)) = self.current() {
                if s == "as" {
                    self.advance();
                    match self.current() {
                        Some(Token::Ident(a)) => {
                            let a = a.clone();
                            self.advance();
                            Some(a)
                        }
                        _ => return Err("Expected alias after 'as'".to_string()),
                    }
                } else {
                    None
                }
            } else {
                None
            };

            items.push(UseItem { name, alias });

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(Token::RBrace)?;

        let end = self.tokens.get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(UseDef {
            path,
            items,
            span: start..end,
        })
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or_expr()
    }

    fn parse_or_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and_expr()?;

        while matches!(self.current(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and_expr()?;
            left = Expr::Binary {
                op: BinaryOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_and_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_range_expr()?;

        while matches!(self.current(), Some(Token::And)) {
            self.advance();
            let right = self.parse_range_expr()?;
            left = Expr::Binary {
                op: BinaryOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_range_expr(&mut self) -> Result<Expr, String> {
        let left = self.parse_equality_expr()?;

        if matches!(self.current(), Some(Token::DotDot)) {
            self.advance();
            let right = self.parse_equality_expr()?;
            return Ok(Expr::Range {
                start: Box::new(left),
                end: Box::new(right),
            });
        }

        Ok(left)
    }

    fn parse_equality_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison_expr()?;

        while matches!(self.current(), Some(Token::EqEq) | Some(Token::NotEq)) {
            let op = match self.current() {
                Some(Token::EqEq) => BinaryOp::Eq,
                Some(Token::NotEq) => BinaryOp::NotEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_comparison_expr()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_comparison_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive_expr()?;

        while matches!(self.current(), Some(Token::Lt) | Some(Token::LtEq) |
                       Some(Token::Gt) | Some(Token::GtEq)) {
            let op = match self.current() {
                Some(Token::Lt) => BinaryOp::Lt,
                Some(Token::LtEq) => BinaryOp::LtEq,
                Some(Token::Gt) => BinaryOp::Gt,
                Some(Token::GtEq) => BinaryOp::GtEq,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_additive_expr()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_additive_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative_expr()?;

        while matches!(self.current(), Some(Token::Plus) | Some(Token::Minus)) {
            let op = match self.current() {
                Some(Token::Plus) => BinaryOp::Add,
                Some(Token::Minus) => BinaryOp::Sub,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_multiplicative_expr()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary_expr()?;

        while matches!(self.current(), Some(Token::Star) | Some(Token::Slash) |
                       Some(Token::Percent) | Some(Token::Div)) {
            let op = match self.current() {
                Some(Token::Star) => BinaryOp::Mul,
                Some(Token::Slash) => BinaryOp::Div,
                Some(Token::Percent) => BinaryOp::Mod,
                Some(Token::Div) => BinaryOp::IntDiv,
                _ => unreachable!(),
            };
            self.advance();
            let right = self.parse_unary_expr()?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, String> {
        if matches!(self.current(), Some(Token::Bang)) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                operand: Box::new(operand),
            });
        }

        if matches!(self.current(), Some(Token::Minus)) {
            self.advance();
            let operand = self.parse_unary_expr()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
            });
        }

        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.current() {
                Some(Token::Dot) => {
                    self.advance();
                    let name = match self.current() {
                        Some(Token::Ident(n)) => {
                            let n = n.clone();
                            self.advance();
                            n
                        }
                        _ => return Err("Expected identifier after .".to_string()),
                    };

                    // Check for method call
                    if matches!(self.current(), Some(Token::LParen)) {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect(Token::RParen)?;
                        expr = Expr::MethodCall {
                            receiver: Box::new(expr),
                            method: name,
                            args,
                        };
                    } else {
                        expr = Expr::Field(Box::new(expr), name);
                    }
                }
                Some(Token::LBracket) => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(Token::RBracket)?;
                    expr = Expr::Index(Box::new(expr), Box::new(index));
                }
                Some(Token::LParen) => {
                    self.advance();
                    let args = self.parse_args()?;
                    self.expect(Token::RParen)?;
                    expr = Expr::Call {
                        func: Box::new(expr),
                        args,
                    };
                }
                Some(Token::DoubleQuestion) => {
                    self.advance();
                    let default = self.parse_unary_expr()?;
                    expr = Expr::Coalesce {
                        value: Box::new(expr),
                        default: Box::new(default),
                    };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary_expr(&mut self) -> Result<Expr, String> {
        match self.current() {
            Some(Token::Int(n)) => {
                let n = *n;
                self.advance();
                Ok(Expr::Int(n))
            }
            Some(Token::Float(f)) => {
                let f = *f;
                self.advance();
                Ok(Expr::Float(f))
            }
            Some(Token::String(s)) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::String(s))
            }
            Some(Token::True) => {
                self.advance();
                Ok(Expr::Bool(true))
            }
            Some(Token::False) => {
                self.advance();
                Ok(Expr::Bool(false))
            }
            Some(Token::Nil) => {
                self.advance();
                Ok(Expr::Nil)
            }
            Some(Token::Ok_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Ok(Box::new(value)))
            }
            Some(Token::Err_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Err(Box::new(value)))
            }
            Some(Token::Some_) => {
                self.advance();
                self.expect(Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Some(Box::new(value)))
            }
            Some(Token::None_) => {
                self.advance();
                Ok(Expr::None_)
            }
            Some(Token::Assert) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("assert".to_string())),
                    args,
                })
            }
            Some(Token::AssertErr) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("assert_err".to_string())),
                    args,
                })
            }
            // Type keywords used as conversion functions: str(), int(), etc.
            Some(Token::StrType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("str".to_string())),
                    args,
                })
            }
            Some(Token::IntType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("int".to_string())),
                    args,
                })
            }
            Some(Token::FloatType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("float".to_string())),
                    args,
                })
            }
            Some(Token::BoolType) => {
                self.advance();
                self.expect(Token::LParen)?;
                let args = self.parse_args()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Call {
                    func: Box::new(Expr::Ident("bool".to_string())),
                    args,
                })
            }
            Some(Token::Dollar) => {
                self.advance();
                match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        Ok(Expr::Config(n))
                    }
                    _ => Err("Expected identifier after $".to_string()),
                }
            }
            Some(Token::Hash) => {
                // # is length placeholder (arr[# - 1] means arr[length - 1])
                self.advance();
                Ok(Expr::LengthPlaceholder)
            }
            Some(Token::Match) => {
                self.advance();
                self.parse_match_expr()
            }
            Some(Token::LParen) => {
                self.advance();
                if matches!(self.current(), Some(Token::RParen)) {
                    self.advance();
                    // Check for lambda with no params: () -> expr
                    if matches!(self.current(), Some(Token::Arrow)) {
                        self.advance();
                        let body = self.parse_expr()?;
                        return Ok(Expr::Lambda {
                            params: Vec::new(),
                            body: Box::new(body),
                        });
                    }
                    return Ok(Expr::Tuple(Vec::new()));
                }
                let expr = self.parse_expr()?;
                if matches!(self.current(), Some(Token::Comma)) {
                    // Could be tuple or multi-param lambda
                    let mut exprs = vec![expr];
                    while matches!(self.current(), Some(Token::Comma)) {
                        self.advance();
                        if matches!(self.current(), Some(Token::RParen)) {
                            break;
                        }
                        exprs.push(self.parse_expr()?);
                    }
                    self.expect(Token::RParen)?;
                    // Check for multi-param lambda: (a, b) -> expr
                    if matches!(self.current(), Some(Token::Arrow)) {
                        self.advance();
                        // Convert exprs to param names
                        let params: Result<Vec<String>, String> = exprs.into_iter().map(|e| {
                            match e {
                                Expr::Ident(n) => Ok(n),
                                _ => Err("Lambda parameters must be identifiers".to_string()),
                            }
                        }).collect();
                        let body = self.parse_expr()?;
                        return Ok(Expr::Lambda {
                            params: params?,
                            body: Box::new(body),
                        });
                    }
                    Ok(Expr::Tuple(exprs))
                } else {
                    self.expect(Token::RParen)?;
                    // Check for single-param lambda with parens: (x) -> expr
                    if matches!(self.current(), Some(Token::Arrow)) {
                        self.advance();
                        let param = match expr {
                            Expr::Ident(n) => n,
                            _ => return Err("Lambda parameter must be an identifier".to_string()),
                        };
                        let body = self.parse_expr()?;
                        return Ok(Expr::Lambda {
                            params: vec![param],
                            body: Box::new(body),
                        });
                    }
                    Ok(expr)
                }
            }
            Some(Token::LBracket) => {
                self.advance();
                let mut exprs = Vec::new();
                while !matches!(self.current(), Some(Token::RBracket)) {
                    exprs.push(self.parse_expr()?);
                    if matches!(self.current(), Some(Token::Comma)) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBracket)?;
                Ok(Expr::List(exprs))
            }
            Some(Token::If) => {
                self.advance();
                let condition = self.parse_or_expr()?;
                self.expect(Token::ColonThen)?;
                self.skip_newlines();
                // Use parse_comparison_expr for then branch - allows binary ops but stops before :else
                let then_expr = self.parse_comparison_expr()?;
                self.skip_newlines();
                let else_expr = if matches!(self.current(), Some(Token::ColonElse)) {
                    self.advance();
                    self.skip_newlines();
                    let e = self.parse_expr()?;
                    Some(Box::new(e))
                } else {
                    None
                };
                Ok(Expr::If {
                    condition: Box::new(condition),
                    then_branch: Box::new(then_expr),
                    else_branch: else_expr,
                })
            }
            Some(Token::For) => {
                self.advance();
                let binding = match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        n
                    }
                    _ => return Err("Expected identifier in for loop".to_string()),
                };
                self.expect(Token::In)?;
                let iterator = self.parse_expr()?;
                self.expect(Token::LBrace)?;
                self.skip_newlines();
                let body = self.parse_expr()?;
                self.skip_newlines();
                self.expect(Token::RBrace)?;
                Ok(Expr::For {
                    binding,
                    iterator: Box::new(iterator),
                    body: Box::new(body),
                })
            }
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();

                // Check for pattern keywords (context-sensitive)
                // These are only patterns when followed by ( and have the right arg count
                if matches!(self.current(), Some(Token::LParen)) {
                    match n.as_str() {
                        "run" => {
                            self.advance(); // consume '('
                            let exprs = self.parse_args()?;
                            self.expect(Token::RParen)?;
                            return Ok(Expr::Block(exprs));
                        }
                        "fold" | "map" | "filter" | "collect" | "recurse" | "parallel" => {
                            return self.parse_pattern_or_call_from_ident(&n);
                        }
                        _ => {} // Fall through to normal handling
                    }
                }

                // Check for assignment
                if matches!(self.current(), Some(Token::ColonEq)) {
                    self.advance();
                    let value = self.parse_expr()?;
                    return Ok(Expr::Assign {
                        target: n,
                        value: Box::new(value),
                    });
                }

                // Check for struct literal
                if matches!(self.current(), Some(Token::LBrace)) {
                    self.advance();
                    let mut fields = Vec::new();
                    self.skip_newlines();
                    while !matches!(self.current(), Some(Token::RBrace)) {
                        let fname = match self.current() {
                            Some(Token::Ident(f)) => {
                                let f = f.clone();
                                self.advance();
                                f
                            }
                            _ => return Err("Expected field name".to_string()),
                        };
                        self.expect(Token::Colon)?;
                        let value = self.parse_expr()?;
                        fields.push((fname, value));

                        self.skip_newlines();
                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                            self.skip_newlines();
                        } else {
                            break;
                        }
                    }
                    self.expect(Token::RBrace)?;
                    return Ok(Expr::Struct { name: n, fields });
                }

                // Check for lambda: x -> expr
                if matches!(self.current(), Some(Token::Arrow)) {
                    self.advance();
                    let body = self.parse_expr()?;
                    return Ok(Expr::Lambda {
                        params: vec![n],
                        body: Box::new(body),
                    });
                }

                Ok(Expr::Ident(n))
            }
            // Standalone operators as values (for fold, etc.)
            Some(Token::Plus) => {
                self.advance();
                Ok(Expr::Ident("+".to_string()))
            }
            Some(Token::Star) => {
                self.advance();
                Ok(Expr::Ident("*".to_string()))
            }
            Some(Token::Minus) => {
                // Could be unary minus or standalone operator
                // Check if followed by a number/expr
                self.advance();
                if matches!(self.current(), Some(Token::Int(_)) | Some(Token::Float(_)) | Some(Token::Ident(_)) | Some(Token::LParen)) {
                    let operand = self.parse_primary_expr()?;
                    Ok(Expr::Unary {
                        op: UnaryOp::Neg,
                        operand: Box::new(operand),
                    })
                } else {
                    Ok(Expr::Ident("-".to_string()))
                }
            }
            _ => Err(format!("Unexpected token in expression: {:?}", self.current())),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();

        self.skip_newlines();
        while !matches!(self.current(), Some(Token::RParen)) {
            args.push(self.parse_expr()?);
            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Parse named arguments in pattern syntax: .property: value
    /// Returns a list of (property_name, value) pairs
    fn parse_named_args(&mut self) -> Result<Vec<(String, Expr)>, String> {
        let mut args = Vec::new();

        self.skip_newlines();
        while !matches!(self.current(), Some(Token::RParen)) {
            // Expect .property: value syntax
            self.expect(Token::Dot)?;

            let prop_name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected property name after '.'".to_string()),
            };

            self.expect(Token::Colon)?;

            // Parse the value expression
            let value = self.parse_expr()?;

            args.push((prop_name, value));

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        Ok(args)
    }

    /// Check if the current position has named arg syntax (.property:)
    fn is_named_arg_start(&self) -> bool {
        matches!(self.current(), Some(Token::Dot)) &&
        matches!(self.peek(1), Some(Token::Ident(_)))
    }

    fn parse_match_expr(&mut self) -> Result<Expr, String> {
        self.expect(Token::LParen)?;
        self.skip_newlines();

        // Parse first expression - could be scrutinee or first condition
        let first_expr = self.parse_expr()?;

        // Check what follows: Comma means scrutinee, Colon means cond-style
        let is_cond_style = matches!(self.current(), Some(Token::Colon));
        let (scrutinee, mut arms): (Expr, Vec<MatchArm>) = if is_cond_style {
            // Cond-style match: match(cond: body, cond: body, default)
            // No scrutinee, just condition checks. Use Bool(true) as dummy scrutinee.
            self.advance(); // consume :
            let body = self.parse_expr()?;
            let first_arm = MatchArm {
                pattern: Pattern::Condition(first_expr),
                body,
            };
            (Expr::Bool(true), vec![first_arm])
        } else {
            // Standard match: match(scrutinee, pattern: body, ...)
            self.expect(Token::Comma)?;
            (first_expr, Vec::new())
        };

        self.skip_newlines();
        if matches!(self.current(), Some(Token::Comma)) {
            self.advance();
            self.skip_newlines();
        }

        while !matches!(self.current(), Some(Token::RParen)) {
            if is_cond_style {
                // For cond-style, parse full expressions as conditions
                let cond_expr = self.parse_expr()?;

                // Check for colon - if not present, this is the default case
                if matches!(self.current(), Some(Token::Colon)) {
                    self.advance();
                    let body = self.parse_expr()?;
                    arms.push(MatchArm {
                        pattern: Pattern::Condition(cond_expr),
                        body,
                    });
                } else {
                    // Default case: expr is the body, use wildcard pattern
                    arms.push(MatchArm {
                        pattern: Pattern::Wildcard,
                        body: cond_expr,
                    });
                }
            } else {
                // Standard match with patterns
                let pattern = self.parse_pattern()?;
                self.expect(Token::Colon)?;
                let body = self.parse_expr()?;
                arms.push(MatchArm { pattern, body });
            }

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            }
        }

        self.expect(Token::RParen)?;

        Ok(Expr::Match(Box::new(MatchExpr { scrutinee, arms })))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, String> {
        match self.current() {
            Some(Token::Ident(n)) if n == "_" => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            Some(Token::Ok_) | Some(Token::Err_) | Some(Token::Some_) | Some(Token::None_) => {
                let name = match self.current() {
                    Some(Token::Ok_) => "Ok",
                    Some(Token::Err_) => "Err",
                    Some(Token::Some_) => "Some",
                    Some(Token::None_) => { self.advance(); return Ok(Pattern::Variant { name: "None".to_string(), fields: Vec::new() }); }
                    _ => unreachable!(),
                }.to_string();
                self.advance();

                let fields = if matches!(self.current(), Some(Token::LBrace)) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !matches!(self.current(), Some(Token::RBrace)) {
                        let fname = match self.current() {
                            Some(Token::Ident(n)) => {
                                let n = n.clone();
                                self.advance();
                                n
                            }
                            _ => return Err("Expected field name in pattern".to_string()),
                        };
                        fields.push((fname.clone(), Pattern::Binding(fname)));

                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RBrace)?;
                    fields
                } else {
                    Vec::new()
                };

                Ok(Pattern::Variant { name, fields })
            }
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();

                // Check if it's a variant pattern with fields
                if matches!(self.current(), Some(Token::LBrace)) {
                    self.advance();
                    let mut fields = Vec::new();
                    while !matches!(self.current(), Some(Token::RBrace)) {
                        let fname = match self.current() {
                            Some(Token::Ident(f)) => {
                                let f = f.clone();
                                self.advance();
                                f
                            }
                            _ => return Err("Expected field name".to_string()),
                        };
                        fields.push((fname.clone(), Pattern::Binding(fname)));

                        if matches!(self.current(), Some(Token::Comma)) {
                            self.advance();
                        }
                    }
                    self.expect(Token::RBrace)?;
                    return Ok(Pattern::Variant { name: n, fields });
                }

                Ok(Pattern::Binding(n))
            }
            Some(Token::Int(_)) | Some(Token::String(_)) | Some(Token::True) | Some(Token::False) => {
                let expr = self.parse_primary_expr()?;
                Ok(Pattern::Literal(expr))
            }
            _ => {
                // Try parsing as a condition
                let expr = self.parse_expr()?;
                Ok(Pattern::Condition(expr))
            }
        }
    }

    /// Disambiguate between pattern keyword (fold, map, filter, etc.) and function call.
    /// Called after we've already consumed the identifier and know it's followed by '('.
    /// Supports both positional args and named property syntax (.property: value).
    fn parse_pattern_or_call_from_ident(&mut self, keyword: &str) -> Result<Expr, String> {
        // Parse args (we know current token is '(')
        self.advance(); // consume '('
        self.skip_newlines();

        // Check if we have named property syntax
        if self.is_named_arg_start() {
            return self.parse_pattern_with_named_args(keyword);
        }

        // Positional args
        let args = self.parse_args()?;
        self.expect(Token::RParen)?;

        // Check if arg count matches pattern signature
        let is_pattern = match keyword {
            "fold" => args.len() == 3,      // fold(collection, init, op)
            "map" => args.len() == 2,       // map(collection, transform)
            "filter" => args.len() == 2,    // filter(collection, predicate)
            "collect" => args.len() == 2,   // collect(range, transform)
            "recurse" => args.len() == 3 || args.len() == 4,   // recurse(cond, base, step) or recurse(cond, base, step, memo)
            _ => false,
        };

        if is_pattern {
            match keyword {
                "fold" => Ok(Expr::Pattern(PatternExpr::Fold {
                    collection: Box::new(args[0].clone()),
                    init: Box::new(args[1].clone()),
                    op: Box::new(args[2].clone()),
                })),
                "map" => Ok(Expr::Pattern(PatternExpr::Map {
                    collection: Box::new(args[0].clone()),
                    transform: Box::new(args[1].clone()),
                })),
                "filter" => Ok(Expr::Pattern(PatternExpr::Filter {
                    collection: Box::new(args[0].clone()),
                    predicate: Box::new(args[1].clone()),
                })),
                "collect" => Ok(Expr::Pattern(PatternExpr::Collect {
                    range: Box::new(args[0].clone()),
                    transform: Box::new(args[1].clone()),
                })),
                "recurse" => {
                    // Check if fourth arg is `true` for memoization
                    let memo = if args.len() >= 4 {
                        matches!(args[3], Expr::Bool(true))
                    } else {
                        false
                    };
                    Ok(Expr::Pattern(PatternExpr::Recurse {
                        condition: Box::new(args[0].clone()),
                        base_value: Box::new(args[1].clone()),
                        step: Box::new(args[2].clone()),
                        memo,
                        parallel_threshold: 0,  // positional syntax doesn't support parallel yet
                    }))
                }
                _ => Ok(Expr::Call {
                    func: Box::new(Expr::Ident(keyword.to_string())),
                    args,
                }),
            }
        } else {
            // Not a pattern - treat as function call
            Ok(Expr::Call {
                func: Box::new(Expr::Ident(keyword.to_string())),
                args,
            })
        }
    }

    /// Parse a pattern expression with named property syntax
    /// e.g., recurse(.cond: n <= 1, .base: 1, .step: n * self(n-1), .memo: true)
    fn parse_pattern_with_named_args(&mut self, keyword: &str) -> Result<Expr, String> {
        let named_args = self.parse_named_args()?;
        self.expect(Token::RParen)?;

        // Convert named args to a hashmap for easy lookup
        let mut props: std::collections::HashMap<String, Expr> = named_args.into_iter().collect();

        match keyword {
            "recurse" => {
                // Required: cond, base, step
                // Optional: memo (default false), parallel (default false)
                let condition = props.remove("cond")
                    .ok_or_else(|| "recurse pattern requires .cond: property".to_string())?;
                let base_value = props.remove("base")
                    .ok_or_else(|| "recurse pattern requires .base: property".to_string())?;
                let step = props.remove("step")
                    .ok_or_else(|| "recurse pattern requires .step: property".to_string())?;
                let memo = props.remove("memo")
                    .map(|e| matches!(e, Expr::Bool(true)))
                    .unwrap_or(false);
                // .parallel: N means parallelize when n > N
                // .parallel: 0 means always parallelize
                // absent means no parallelization
                let parallel_threshold = props.remove("parallel")
                    .map(|e| match e {
                        Expr::Int(n) => n,
                        _ => i64::MAX,  // invalid value = no parallelization
                    })
                    .unwrap_or(i64::MAX);

                Ok(Expr::Pattern(PatternExpr::Recurse {
                    condition: Box::new(condition),
                    base_value: Box::new(base_value),
                    step: Box::new(step),
                    memo,
                    parallel_threshold,
                }))
            }
            "fold" => {
                // Required: over, init, op
                let collection = props.remove("over")
                    .ok_or_else(|| "fold pattern requires .over: property".to_string())?;
                let init = props.remove("init")
                    .ok_or_else(|| "fold pattern requires .init: property".to_string())?;
                let op = props.remove("op")
                    .ok_or_else(|| "fold pattern requires .op: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Fold {
                    collection: Box::new(collection),
                    init: Box::new(init),
                    op: Box::new(op),
                }))
            }
            "map" => {
                // Required: over, transform
                let collection = props.remove("over")
                    .ok_or_else(|| "map pattern requires .over: property".to_string())?;
                let transform = props.remove("transform")
                    .ok_or_else(|| "map pattern requires .transform: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Map {
                    collection: Box::new(collection),
                    transform: Box::new(transform),
                }))
            }
            "filter" => {
                // Required: over, predicate
                let collection = props.remove("over")
                    .ok_or_else(|| "filter pattern requires .over: property".to_string())?;
                let predicate = props.remove("predicate")
                    .ok_or_else(|| "filter pattern requires .predicate: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Filter {
                    collection: Box::new(collection),
                    predicate: Box::new(predicate),
                }))
            }
            "collect" => {
                // Required: range, transform
                let range = props.remove("range")
                    .ok_or_else(|| "collect pattern requires .range: property".to_string())?;
                let transform = props.remove("transform")
                    .ok_or_else(|| "collect pattern requires .transform: property".to_string())?;

                Ok(Expr::Pattern(PatternExpr::Collect {
                    range: Box::new(range),
                    transform: Box::new(transform),
                }))
            }
            "parallel" => {
                // Optional: timeout, on_error
                // All other properties become branches
                let timeout = props.remove("timeout").map(|e| Box::new(e));
                let on_error = props.remove("on_error")
                    .map(|e| {
                        if let Expr::Ident(s) = &e {
                            if s == "collect_all" {
                                OnError::CollectAll
                            } else {
                                OnError::FailFast
                            }
                        } else {
                            OnError::FailFast
                        }
                    })
                    .unwrap_or(OnError::FailFast);

                // Remaining props are the branches
                let branches: Vec<(String, Expr)> = props.into_iter().collect();

                if branches.is_empty() {
                    return Err("parallel pattern requires at least one branch".to_string());
                }

                Ok(Expr::Pattern(PatternExpr::Parallel {
                    branches,
                    timeout,
                    on_error,
                }))
            }
            _ => Err(format!("Unknown pattern keyword with named args: {}", keyword)),
        }
    }
}


pub fn parse(tokens: Vec<SpannedToken>, filename: &str) -> Result<Module, String> {
    let mut parser = Parser::new(tokens);
    parser.parse_module(filename)
}
