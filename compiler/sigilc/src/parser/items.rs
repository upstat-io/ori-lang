// Item parsing for Sigil
// Handles functions, tests, configs, type definitions, and use statements

use super::Parser;
use crate::ast::{
    ConfigDef, Field, FunctionDef, Item, Param, TestDef, TypeDef, TypeDefKind, UseDef, UseItem,
    Variant,
};
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_function_or_test(&mut self, public: bool) -> Result<Item, String> {
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
        self.parse_function_rest(public, name, start)
            .map(Item::Function)
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

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(Item::Test(TestDef {
            name,
            target,
            body,
            span: start..end,
        }))
    }

    pub(super) fn parse_config(&mut self) -> Result<ConfigDef, String> {
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
        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(ConfigDef {
            name,
            ty,
            value,
            span: start..end,
        })
    }

    fn parse_function_rest(
        &mut self,
        public: bool,
        name: String,
        start: usize,
    ) -> Result<FunctionDef, String> {
        // Optional type parameters
        // Type params are identifiers before the opening paren.
        // We stop when we see an identifier followed by colon (that's a param name: type)
        let type_params = if matches!(self.current(), Some(Token::Ident(_))) {
            let mut params = Vec::new();
            while let Some(Token::Ident(p)) = self.current() {
                // If next token is colon, this ident is a param name, not a type param
                if matches!(self.peek(1), Some(Token::Colon)) {
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

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
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

    pub(super) fn parse_params(&mut self) -> Result<Vec<Param>, String> {
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

    pub(super) fn parse_type_def(&mut self, public: bool) -> Result<TypeDef, String> {
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

        // Optional type parameters for generic types
        let params = if matches!(self.current(), Some(Token::Ident(_))) {
            let mut p = Vec::new();
            while let Some(Token::Ident(param)) = self.current() {
                p.push(param.clone());
                self.advance();
            }
            p
        } else {
            Vec::new()
        };

        let kind = if matches!(self.current(), Some(Token::LBrace)) {
            self.parse_struct_fields()?
        } else if matches!(self.current(), Some(Token::Eq)) {
            // Type alias or enum
            self.advance();
            if matches!(self.current(), Some(Token::Pipe)) {
                // Enum: type Foo = | A | B | C
                self.parse_enum_variants()?
            } else {
                // Type alias: type Foo = OtherType
                let aliased = self.parse_type()?;
                TypeDefKind::Alias(aliased)
            }
        } else {
            return Err("Expected { or = after type name".to_string());
        };

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
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
        self.skip_newlines();

        let mut fields = Vec::new();
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
            } else {
                break;
            }
        }

        self.expect(Token::RBrace)?;

        Ok(TypeDefKind::Struct(fields))
    }

    fn parse_enum_variants(&mut self) -> Result<TypeDefKind, String> {
        let mut variants = Vec::new();

        while matches!(self.current(), Some(Token::Pipe)) {
            self.advance(); // consume |
            self.skip_newlines();

            let name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                Some(Token::Ok_) => {
                    self.advance();
                    "Ok".to_string()
                }
                Some(Token::Err_) => {
                    self.advance();
                    "Err".to_string()
                }
                Some(Token::Some_) => {
                    self.advance();
                    "Some".to_string()
                }
                Some(Token::None_) => {
                    self.advance();
                    "None".to_string()
                }
                _ => return Err("Expected variant name".to_string()),
            };

            // Optional associated data
            let fields = if matches!(self.current(), Some(Token::LBrace)) {
                self.advance();
                let mut variant_fields = Vec::new();
                self.skip_newlines();
                while !matches!(self.current(), Some(Token::RBrace)) {
                    let field_name = match self.current() {
                        Some(Token::Ident(n)) => {
                            let n = n.clone();
                            self.advance();
                            n
                        }
                        _ => return Err("Expected field name in variant".to_string()),
                    };
                    self.expect(Token::Colon)?;
                    let ty = self.parse_type()?;
                    variant_fields.push(Field {
                        name: field_name,
                        ty,
                    });

                    self.skip_newlines();
                    if matches!(self.current(), Some(Token::Comma)) {
                        self.advance();
                        self.skip_newlines();
                    } else {
                        break;
                    }
                }
                self.expect(Token::RBrace)?;
                variant_fields
            } else {
                Vec::new()
            };

            variants.push(Variant { name, fields });
            self.skip_newlines();
        }

        Ok(TypeDefKind::Enum(variants))
    }

    pub(super) fn parse_use(&mut self) -> Result<UseDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Use)?;

        // Parse module path (e.g., std.math or just math)
        let mut path = Vec::new();
        loop {
            match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    path.push(n);
                }
                _ => return Err("Expected module name in use statement".to_string()),
            }

            if matches!(self.current(), Some(Token::Dot)) {
                self.advance();
            } else {
                break;
            }
        }

        // Parse imported items { ... }
        self.expect(Token::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();
        while !matches!(self.current(), Some(Token::RBrace)) {
            match self.current() {
                Some(Token::Ident(n)) => {
                    let name = n.clone();
                    self.advance();

                    // Check for alias: `name as alias`
                    let alias = if matches!(self.current(), Some(Token::Ident(s)) if s == "as") {
                        self.advance();
                        match self.current() {
                            Some(Token::Ident(a)) => {
                                let a = a.clone();
                                self.advance();
                                Some(a)
                            }
                            _ => return Err("Expected identifier after 'as'".to_string()),
                        }
                    } else {
                        None
                    };

                    items.push(UseItem { name, alias });
                }
                Some(Token::Star) => {
                    self.advance();
                    items.push(UseItem {
                        name: "*".to_string(),
                        alias: None,
                    });
                }
                _ => return Err("Expected identifier or * in use statement".to_string()),
            }

            self.skip_newlines();
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
                self.skip_newlines();
            } else {
                break;
            }
        }

        self.expect(Token::RBrace)?;

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(UseDef {
            path,
            items,
            span: start..end,
        })
    }
}
