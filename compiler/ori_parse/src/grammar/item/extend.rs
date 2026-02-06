//! Extend block parsing.

use crate::{committed, ParseOutcome, Parser};
use ori_ir::{ExtendDef, GenericParamRange, ParsedType, ParsedTypeRange, TokenKind, TypeId};

impl Parser<'_> {
    /// Parse an extend block.
    ///
    /// Syntax: extend [<T>] Type { methods }
    ///
    /// Returns `EmptyErr` if no `extend` keyword is present.
    ///
    /// Examples:
    ///   extend [T] { @map... }           - extends all lists
    ///   extend<T> Option<T> { @map... }  - extends Option
    ///   extend str { @reverse... }       - extends str
    pub(crate) fn parse_extend(&mut self) -> ParseOutcome<ExtendDef> {
        if !self.check(&TokenKind::Extend) {
            return ParseOutcome::empty_err_expected(&TokenKind::Extend, self.position());
        }

        self.parse_extend_body()
    }

    fn parse_extend_body(&mut self) -> ParseOutcome<ExtendDef> {
        let start_span = self.current_span();
        committed!(self.expect(&TokenKind::Extend));

        // Optional generics: <T, U: Bound>
        let generics = if self.check(&TokenKind::Lt) {
            committed!(self.parse_generics().into_result())
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
                committed!(self.parse_type_required().into_result())
            };
            committed!(self.expect(&TokenKind::RBracket));
            // List type - method dispatch uses "list"
            let elem_id = self.arena.alloc_parsed_type(elem_ty);
            (ParsedType::List(elem_id), self.interner().intern("list"))
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
            let type_name = committed!(self.expect_ident());
            // Check for generic parameters like Option<T>
            let type_args = if self.check(&TokenKind::Lt) {
                self.advance(); // <
                let mut arg_ids = Vec::new();
                while !self.check(&TokenKind::Gt) && !self.is_at_end() {
                    let ty = committed!(self.parse_type_required().into_result());
                    let id = self.arena.alloc_parsed_type(ty);
                    arg_ids.push(id);
                    if self.check(&TokenKind::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                if self.check(&TokenKind::Gt) {
                    self.advance(); // >
                }
                self.arena.alloc_parsed_type_list(arg_ids)
            } else {
                ParsedTypeRange::EMPTY
            };
            (
                ParsedType::Named {
                    name: type_name,
                    type_args,
                },
                type_name,
            )
        };

        // Optional where clause
        let where_clauses = if self.check(&TokenKind::Where) {
            committed!(self.parse_where_clauses().into_result())
        } else {
            Vec::new()
        };

        // Extend body: { methods }
        committed!(self.expect(&TokenKind::LBrace));
        self.skip_newlines();

        let mut methods = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            let method = committed!(self.parse_impl_method());
            methods.push(method);
            self.skip_newlines();
        }

        let end_span = self.current_span();
        committed!(self.expect(&TokenKind::RBrace));

        ParseOutcome::consumed_ok(ExtendDef {
            generics,
            target_ty,
            target_type_name,
            where_clauses,
            methods,
            span: start_span.merge(end_span),
        })
    }
}
