//! Extend block parsing.

use ori_ir::{ExtendDef, GenericParamRange, ParsedType, TokenKind, TypeId};
use crate::{ParseError, Parser};

impl Parser<'_> {
    /// Parse an extend block.
    /// Syntax: extend [<T>] Type { methods }
    ///
    /// Examples:
    ///   extend [T] { @map... }           - extends all lists
    ///   extend<T> Option<T> { @map... }  - extends Option
    ///   extend str { @reverse... }       - extends str
    pub(crate) fn parse_extend(&mut self) -> Result<ExtendDef, ParseError> {
        let start_span = self.current_span();
        self.expect(&TokenKind::Extend)?;

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            self.parse_generics()?
        } else {
            GenericParamRange::EMPTY
        };

        // Parse the target type
        // Handle [T] for list types
        let (target_ty, target_type_name) = if self.check(&TokenKind::LBracket) {
            self.advance(); // [
            // Parse element type (optional, default to infer)
            let elem_ty = if self.check(&TokenKind::RBracket) {
                ParsedType::Infer
            } else {
                self.parse_type_required()?
            };
            self.expect(&TokenKind::RBracket)?;
            // List type - method dispatch uses "list"
            (ParsedType::List(Box::new(elem_ty)), self.interner().intern("list"))
        } else if self.check_type_keyword() {
            // Primitive type keywords: str, int, float, bool, etc.
            let (ty, type_name_str) = match self.current_kind() {
                TokenKind::StrType => (ParsedType::Primitive(TypeId::STR), "str"),
                TokenKind::IntType => (ParsedType::Primitive(TypeId::INT), "int"),
                TokenKind::FloatType => (ParsedType::Primitive(TypeId::FLOAT), "float"),
                TokenKind::BoolType => (ParsedType::Primitive(TypeId::BOOL), "bool"),
                TokenKind::CharType => (ParsedType::Primitive(TypeId::CHAR), "char"),
                TokenKind::ByteType => (ParsedType::Primitive(TypeId::BYTE), "byte"),
                _ => (ParsedType::Infer, "unknown"),
            };
            self.advance();
            (ty, self.interner().intern(type_name_str))
        } else {
            // Named type like Option<T>, MyType, etc.
            let type_name = self.expect_ident()?;
            // Check for generic parameters like Option<T>
            let type_args = if self.check(&TokenKind::Lt) {
                self.advance(); // <
                let mut args = Vec::new();
                while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                    args.push(self.parse_type_required()?);
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(&TokenKind::Gt) {
                    self.advance(); // >
                }
                args
            } else {
                Vec::new()
            };
            (ParsedType::Named { name: type_name, type_args }, type_name)
        };

        // Optional where clause
        let where_clauses = if self.check(&TokenKind::Where) {
            self.parse_where_clauses()?
        } else {
            Vec::new()
        };

        // Extend body: { methods }
        self.expect(&TokenKind::LBrace)?;
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            match self.parse_impl_method() {
                Ok(method) => methods.push(method),
                Err(e) => {
                    return Err(e);
                }
            }
            self.skip_newlines();
        }

        let end_span = self.current_span();
        self.expect(&TokenKind::RBrace)?;

        Ok(ExtendDef {
            generics,
            target_ty,
            target_type_name,
            where_clauses,
            methods,
            span: start_span.merge(end_span),
        })
    }
}
