// Type expression parsing for Sigil
// Handles parsing of type annotations and type expressions

use super::Parser;
use crate::ast::TypeExpr;
use crate::lexer::Token;

impl Parser {
    pub(super) fn parse_type(&mut self) -> Result<TypeExpr, String> {
        // Optional type: ?T
        if matches!(self.current(), Some(Token::Question)) {
            self.advance();
            let inner = self.parse_type()?;
            return Ok(TypeExpr::Optional(Box::new(inner)));
        }

        // Dynamic trait object: dyn Trait
        if matches!(self.current(), Some(Token::Dyn)) {
            self.advance();
            let trait_name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected trait name after 'dyn'".to_string()),
            };
            return Ok(TypeExpr::DynTrait(trait_name));
        }

        // Async type: async T
        if matches!(self.current(), Some(Token::Async)) {
            self.advance();
            let inner = self.parse_type()?;
            return Ok(TypeExpr::Async(Box::new(inner)));
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
            if let Some(Token::Ident(_field_name)) = self.current().cloned() {
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
                    types
                        .into_iter()
                        .next()
                        .unwrap_or_else(|| TypeExpr::Named("void".to_string()))
                } else {
                    TypeExpr::Tuple(types)
                };
                return Ok(TypeExpr::Function(Box::new(param_type), Box::new(ret)));
            }

            // Single-element parens are grouping, not tuple
            if types.len() == 1 {
                return Ok(types
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| TypeExpr::Named("void".to_string())));
            }

            return Ok(TypeExpr::Tuple(types));
        }

        // Named type - track if it can have generic args
        let (name, can_be_generic) = match self.current() {
            Some(Token::IntType) => {
                self.advance();
                ("int".to_string(), false)
            }
            Some(Token::FloatType) => {
                self.advance();
                ("float".to_string(), false)
            }
            Some(Token::StrType) => {
                self.advance();
                ("str".to_string(), false)
            }
            Some(Token::BoolType) => {
                self.advance();
                ("bool".to_string(), false)
            }
            Some(Token::VoidType) => {
                self.advance();
                ("void".to_string(), false)
            }
            Some(Token::ResultType) => {
                self.advance();
                ("Result".to_string(), true) // Result can have generic args
            }
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                (n, true) // User-defined types can have generic args
            }
            _ => return Err("Expected type".to_string()),
        };

        // Check for generic type arguments with angle bracket syntax: Type<T, U>
        if can_be_generic && matches!(self.current(), Some(Token::Lt)) {
            self.advance(); // consume '<'
            let mut args = Vec::new();
            while !matches!(self.current(), Some(Token::Gt)) {
                args.push(self.parse_type()?);
                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
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
}
