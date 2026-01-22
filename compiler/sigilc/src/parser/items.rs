// Item parsing for Sigil
// Handles functions, tests, configs, type definitions, use statements, traits, and impls

use super::Parser;
use crate::ast::{
    AssociatedType, AssociatedTypeImpl, ConfigDef, ExtendBlock, ExtensionImport, ExtensionItem,
    Field, FunctionDef, ImplBlock, Item, Param, TestDef, TraitDef, TraitMethodDef, TypeDef,
    TypeDefKind, TypeExpr, TypeParam, UseDef, UseItem, Variant, WhereBound,
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
        // Optional type parameters with bounds: @func<T, U: Comparable>(...)
        let type_param_bounds = if matches!(self.current(), Some(Token::Lt)) {
            self.parse_bounded_type_params()?
        } else {
            Vec::new()
        };

        // Extract just the names for backward compatibility
        let type_params: Vec<String> = type_param_bounds.iter().map(|p| p.name.clone()).collect();

        // Parameters
        self.expect(Token::LParen)?;
        let params = self.parse_params()?;
        self.expect(Token::RParen)?;

        // Return type
        self.expect(Token::Arrow)?;
        let return_type = self.parse_type()?;

        // Optional where clause: where T: Bound, U: Bound
        let where_clause = if matches!(self.current(), Some(Token::Where)) {
            self.advance();
            self.parse_where_clause()?
        } else {
            Vec::new()
        };

        // Optional uses clause: uses Http, FileSystem
        let uses_clause = if matches!(self.current(), Some(Token::Uses)) {
            self.advance();
            self.parse_uses_clause()?
        } else {
            Vec::new()
        };

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
            type_param_bounds,
            where_clause,
            uses_clause,
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

        // Optional type parameters for generic types with angle bracket syntax: type Box<T>
        let params = if matches!(self.current(), Some(Token::Lt)) {
            self.advance(); // consume '<'
            let mut p = Vec::new();
            while !matches!(self.current(), Some(Token::Gt)) {
                match self.current() {
                    Some(Token::Ident(param)) => {
                        p.push(param.clone());
                        self.advance();
                    }
                    _ => return Err("Expected type parameter name".to_string()),
                }
                if matches!(self.current(), Some(Token::Comma)) {
                    self.advance();
                } else {
                    break;
                }
            }
            self.expect(Token::Gt)?;
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

        // Parse module path - either a string literal for relative paths
        // or dot-separated identifiers for module paths
        let mut path = Vec::new();

        // Check for string literal (relative path like '../math' or "./math")
        if let Some(Token::String(s)) = self.current() {
            let s = s.clone();
            self.advance();
            path.push(s);
        } else {
            // Parse dot-separated module path (e.g., std.math or just math)
            loop {
                match self.current() {
                    Some(Token::Ident(n)) => {
                        let n = n.clone();
                        self.advance();
                        path.push(n);
                    }
                    _ => return Err("Expected module name or path string in use statement".to_string()),
                }

                if matches!(self.current(), Some(Token::Dot)) {
                    self.advance();
                } else {
                    break;
                }
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

    /// Parse a trait definition: trait Name<T>: Supertrait { ... }
    pub(super) fn parse_trait(&mut self, public: bool) -> Result<TraitDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Trait)?;

        // Trait name
        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected trait name".to_string()),
        };

        // Optional type parameters: trait Comparable<T>
        let type_params = if matches!(self.current(), Some(Token::Lt)) {
            self.parse_type_params()?
        } else {
            Vec::new()
        };

        // Optional supertraits: trait Ord: Eq + PartialOrd
        let supertraits = if matches!(self.current(), Some(Token::Colon)) {
            self.advance();
            self.parse_trait_bounds()?
        } else {
            Vec::new()
        };

        // Trait body
        self.expect(Token::LBrace)?;
        self.skip_newlines();

        let mut associated_types = Vec::new();
        let mut methods = Vec::new();

        while !matches!(self.current(), Some(Token::RBrace)) {
            self.skip_newlines();
            if matches!(self.current(), Some(Token::RBrace)) {
                break;
            }

            // Associated type: type Item: Bound = Default
            if matches!(self.current(), Some(Token::Type)) {
                associated_types.push(self.parse_associated_type()?);
            }
            // Method: @name(...) -> Type or @name(...) -> Type = default_body
            else if matches!(self.current(), Some(Token::At)) {
                methods.push(self.parse_trait_method()?);
            } else {
                return Err("Expected 'type' or '@' in trait body".to_string());
            }

            self.skip_newlines();
        }

        self.expect(Token::RBrace)?;

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(TraitDef {
            public,
            name,
            type_params,
            supertraits,
            associated_types,
            methods,
            span: start..end,
        })
    }

    /// Parse an associated type: type Item: Bound = Default
    fn parse_associated_type(&mut self) -> Result<AssociatedType, String> {
        self.expect(Token::Type)?;

        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected associated type name".to_string()),
        };

        // Optional bounds: type Item: Comparable
        let bounds = if matches!(self.current(), Some(Token::Colon)) {
            self.advance();
            self.parse_trait_bounds()?
        } else {
            Vec::new()
        };

        // Optional default: type Item = int
        let default = if matches!(self.current(), Some(Token::Eq)) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.skip_newlines();

        Ok(AssociatedType {
            name,
            bounds,
            default,
        })
    }

    /// Parse a trait method: @name<T>(...) -> Type [= default_body]
    fn parse_trait_method(&mut self) -> Result<TraitMethodDef, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::At)?;

        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected method name after @".to_string()),
        };

        // Optional type parameters
        let type_params = if matches!(self.current(), Some(Token::Lt)) {
            self.parse_type_params()?
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

        // Optional default body
        let default_body = if matches!(self.current(), Some(Token::Eq)) {
            self.advance();
            self.skip_newlines();
            Some(self.parse_expr()?)
        } else {
            None
        };

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(TraitMethodDef {
            name,
            type_params,
            params,
            return_type,
            default_body,
            span: start..end,
        })
    }

    /// Parse an impl block: impl<T> Trait for Type where T: Bound { ... }
    pub(super) fn parse_impl(&mut self) -> Result<ImplBlock, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Impl)?;

        // Optional type parameters: impl<T>
        let type_params = if matches!(self.current(), Some(Token::Lt)) {
            self.parse_type_params()?
        } else {
            Vec::new()
        };

        // First type - could be trait name or the type itself (inherent impl)
        let first_type = self.parse_type()?;

        // Check for "for Type" to determine if this is a trait impl
        let (trait_name, for_type) = if matches!(self.current(), Some(Token::For)) {
            self.advance();
            let for_ty = self.parse_type()?;
            // Extract trait name from first_type
            let trait_name = match first_type {
                TypeExpr::Named(n) => Some(n),
                TypeExpr::Generic(n, _) => Some(n),
                _ => return Err("Expected trait name before 'for'".to_string()),
            };
            (trait_name, for_ty)
        } else {
            // Inherent impl: impl Type { ... }
            (None, first_type)
        };

        // Optional where clause
        let where_clause = if matches!(self.current(), Some(Token::Where)) {
            self.advance();
            self.parse_where_clause()?
        } else {
            Vec::new()
        };

        // Impl body
        self.expect(Token::LBrace)?;
        self.skip_newlines();

        let mut associated_types = Vec::new();
        let mut methods = Vec::new();

        while !matches!(self.current(), Some(Token::RBrace)) {
            self.skip_newlines();
            if matches!(self.current(), Some(Token::RBrace)) {
                break;
            }

            // Associated type impl: type Item = ConcreteType
            if matches!(self.current(), Some(Token::Type)) {
                associated_types.push(self.parse_associated_type_impl()?);
            }
            // Method: @name(...) -> Type = body
            else if matches!(self.current(), Some(Token::At)) {
                let public = false; // Impl methods inherit visibility from impl block
                let func = self.parse_function_or_test(public)?;
                match func {
                    Item::Function(f) => methods.push(f),
                    _ => return Err("Expected function in impl block".to_string()),
                }
            } else {
                return Err("Expected 'type' or '@' in impl body".to_string());
            }

            self.skip_newlines();
        }

        self.expect(Token::RBrace)?;

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(ImplBlock {
            type_params,
            trait_name,
            for_type,
            where_clause,
            associated_types,
            methods,
            span: start..end,
        })
    }

    /// Parse an associated type implementation: type Item = ConcreteType
    fn parse_associated_type_impl(&mut self) -> Result<AssociatedTypeImpl, String> {
        self.expect(Token::Type)?;

        let name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected associated type name".to_string()),
        };

        self.expect(Token::Eq)?;
        let ty = self.parse_type()?;

        self.skip_newlines();

        Ok(AssociatedTypeImpl { name, ty })
    }

    /// Parse type parameters: <T, U, V>
    /// For backward compatibility, returns just the names (no bounds)
    fn parse_type_params(&mut self) -> Result<Vec<String>, String> {
        let bounded = self.parse_bounded_type_params()?;
        Ok(bounded.into_iter().map(|p| p.name).collect())
    }

    /// Parse type parameters with optional bounds: <T, U: Comparable, V: Eq + Clone>
    fn parse_bounded_type_params(&mut self) -> Result<Vec<TypeParam>, String> {
        self.expect(Token::Lt)?;
        let mut params = Vec::new();

        while !matches!(self.current(), Some(Token::Gt)) {
            let name = match self.current() {
                Some(Token::Ident(p)) => {
                    let p = p.clone();
                    self.advance();
                    p
                }
                _ => return Err("Expected type parameter name".to_string()),
            };

            // Check for optional bounds: T: Bound1 + Bound2
            let bounds = if matches!(self.current(), Some(Token::Colon)) {
                self.advance();
                self.parse_trait_bounds()?
            } else {
                Vec::new()
            };

            params.push(TypeParam { name, bounds });

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        self.expect(Token::Gt)?;
        Ok(params)
    }

    /// Parse trait bounds: Trait1 + Trait2 + Trait3
    fn parse_trait_bounds(&mut self) -> Result<Vec<String>, String> {
        let mut bounds = Vec::new();

        loop {
            match self.current() {
                Some(Token::Ident(n)) => {
                    bounds.push(n.clone());
                    self.advance();
                }
                _ => {
                    if bounds.is_empty() {
                        return Err("Expected trait name in bounds".to_string());
                    }
                    break;
                }
            }

            if matches!(self.current(), Some(Token::Plus)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(bounds)
    }

    /// Parse where clause: T: Trait1, U: Trait2 + Trait3
    fn parse_where_clause(&mut self) -> Result<Vec<WhereBound>, String> {
        let mut bounds = Vec::new();

        loop {
            let type_param = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => break,
            };

            self.expect(Token::Colon)?;
            let trait_bounds = self.parse_trait_bounds()?;

            bounds.push(WhereBound {
                type_param,
                bounds: trait_bounds,
            });

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(bounds)
    }

    /// Parse uses clause: Capability1, Capability2, ...
    fn parse_uses_clause(&mut self) -> Result<Vec<String>, String> {
        let mut capabilities = Vec::new();

        loop {
            match self.current() {
                Some(Token::Ident(name)) => {
                    capabilities.push(name.clone());
                    self.advance();
                }
                _ => {
                    if capabilities.is_empty() {
                        return Err("Expected capability name after 'uses'".to_string());
                    }
                    break;
                }
            }

            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            } else {
                break;
            }
        }

        Ok(capabilities)
    }

    /// Parse extend block: extend Trait where ... { methods }
    pub(super) fn parse_extend(&mut self) -> Result<ExtendBlock, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Extend)?;

        // Trait name
        let trait_name = match self.current() {
            Some(Token::Ident(n)) => {
                let n = n.clone();
                self.advance();
                n
            }
            _ => return Err("Expected trait name after 'extend'".to_string()),
        };

        // Optional where clause
        let where_clause = if matches!(self.current(), Some(Token::Where)) {
            self.advance();
            self.parse_where_clause()?
        } else {
            Vec::new()
        };

        // Extend body
        self.expect(Token::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();

        while !matches!(self.current(), Some(Token::RBrace)) {
            self.skip_newlines();
            if matches!(self.current(), Some(Token::RBrace)) {
                break;
            }

            // Method: @name(...) -> Type = body
            if matches!(self.current(), Some(Token::At)) {
                let public = false; // Extension methods are not public individually
                let func = self.parse_function_or_test(public)?;
                match func {
                    Item::Function(f) => methods.push(f),
                    _ => return Err("Expected function in extend block".to_string()),
                }
            } else {
                return Err("Expected '@' for method in extend body".to_string());
            }

            self.skip_newlines();
        }

        self.expect(Token::RBrace)?;

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(ExtendBlock {
            trait_name,
            where_clause,
            methods,
            span: start..end,
        })
    }

    /// Parse extension import: extension path { Trait.method, ... }
    pub(super) fn parse_extension(&mut self) -> Result<ExtensionImport, String> {
        let start = self.tokens[self.pos].span.start;
        self.expect(Token::Extension)?;

        // Path - either 'path' (string literal) or dotted identifier
        let path = if let Some(Token::String(s)) = self.current() {
            let s = s.clone();
            self.advance();
            // Split string path into components
            s.split('/')
                .filter(|p| !p.is_empty() && *p != ".")
                .map(|s| s.to_string())
                .collect()
        } else {
            // Dotted identifier path: std.iter.extensions
            let mut path = Vec::new();
            loop {
                match self.current() {
                    Some(Token::Ident(n)) => {
                        path.push(n.clone());
                        self.advance();
                    }
                    _ => {
                        if path.is_empty() {
                            return Err("Expected module path after 'extension'".to_string());
                        }
                        break;
                    }
                }
                if matches!(self.current(), Some(Token::Dot)) {
                    self.advance();
                } else {
                    break;
                }
            }
            path
        };

        // Items: { Trait.method, ... }
        self.expect(Token::LBrace)?;
        self.skip_newlines();

        let mut items = Vec::new();

        while !matches!(self.current(), Some(Token::RBrace)) {
            self.skip_newlines();
            if matches!(self.current(), Some(Token::RBrace)) {
                break;
            }

            // Parse Trait.method
            let trait_name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected trait name in extension import".to_string()),
            };

            self.expect(Token::Dot)?;

            let method_name = match self.current() {
                Some(Token::Ident(n)) => {
                    let n = n.clone();
                    self.advance();
                    n
                }
                _ => return Err("Expected method name after '.' in extension import".to_string()),
            };

            items.push(ExtensionItem {
                trait_name,
                method_name,
            });

            // Optional comma
            if matches!(self.current(), Some(Token::Comma)) {
                self.advance();
            }

            self.skip_newlines();
        }

        self.expect(Token::RBrace)?;

        let end = self
            .tokens
            .get(self.pos.saturating_sub(1))
            .map(|t| t.span.end)
            .unwrap_or(start);

        Ok(ExtensionImport {
            path,
            items,
            span: start..end,
        })
    }
}
