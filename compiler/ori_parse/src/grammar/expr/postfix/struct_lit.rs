//! Struct-literal and call-argument parsing for postfix expressions.

use crate::{ParseError, Parser};
use ori_ir::{CallArg, Expr, ExprId, ExprKind, FieldInit, StructLitField, TokenKind};

impl Parser<'_> {
    /// Parse a struct literal after `{` has been consumed. `type_path` is the
    /// parsed type-path head (`type_path = identifier { "." identifier }`):
    /// `Named` for a bare name, `AssociatedType`-chain for a module-qualified
    /// path.
    pub(super) fn parse_postfix_struct_lit(
        &mut self,
        type_path: ori_ir::ParsedTypeId,
        start_span: ori_ir::Span,
    ) -> Result<ExprId, ParseError> {
        // Struct literal fields use a Vec because nested struct literals
        // share the same buffer, causing same-buffer nesting conflicts.
        let mut fields: Vec<StructLitField> = Vec::new();
        let mut has_spread = false;
        self.brace_series_direct(|p| {
            if p.cursor.check(&TokenKind::RBrace) {
                return Ok(false);
            }

            let field_span = p.cursor.current_span();

            // Check for spread syntax: ...expr
            if p.cursor.check(&TokenKind::DotDotDot) {
                p.cursor.advance();
                has_spread = true;
                let spread_expr = p.parse_expr().into_result()?;
                let end_span = p.arena.get_expr(spread_expr).span;
                fields.push(StructLitField::Spread {
                    expr: spread_expr,
                    span: field_span.merge(end_span),
                });
                return Ok(true);
            }

            // Regular field: name or name: value
            let field_name = p.cursor.expect_ident()?;
            let value = if p.cursor.check(&TokenKind::Colon) {
                p.cursor.advance();
                Some(p.parse_expr().into_result()?)
            } else {
                None
            };

            let end_span = if let Some(v) = value {
                p.arena.get_expr(v).span
            } else {
                p.cursor.previous_span()
            };

            fields.push(StructLitField::Field(FieldInit {
                name: field_name,
                value,
                span: field_span.merge(end_span),
            }));
            Ok(true)
        })?;

        let end_span = self.cursor.previous_span();

        if has_spread {
            let fields_range = self.arena.alloc_struct_lit_fields(fields);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::StructWithSpread {
                    type_path,
                    fields: fields_range,
                },
                start_span.merge(end_span),
            )))
        } else {
            let field_inits: Vec<FieldInit> = fields
                .into_iter()
                .filter_map(|f| match f {
                    StructLitField::Field(init) => Some(init),
                    StructLitField::Spread { .. } => None,
                })
                .collect();
            let fields_range = self.arena.alloc_field_inits(field_inits);
            Ok(self.arena.alloc_expr(Expr::new(
                ExprKind::Struct {
                    type_path,
                    fields: fields_range,
                },
                start_span.merge(end_span),
            )))
        }
    }

    /// Parse call arguments, supporting both positional and named args.
    pub(crate) fn parse_call_args(&mut self) -> Result<(Vec<CallArg>, bool), ParseError> {
        use crate::series::SeriesConfig;

        let args: Vec<CallArg> = self.series(&SeriesConfig::comma(TokenKind::RParen), |p| {
            if p.cursor.check(&TokenKind::RParen) {
                return Ok(None);
            }

            let arg_span = p.cursor.current_span();

            // Check for spread syntax: ...expr
            let is_spread = p.cursor.check(&TokenKind::DotDotDot);
            if is_spread {
                p.cursor.advance();
            }

            let (name, value) = if p.cursor.is_named_arg_start() {
                let arg_span = p.cursor.current_span();
                let name = p.cursor.expect_ident_or_keyword()?;
                p.cursor.expect(&TokenKind::Colon)?;

                // Argument punning: `f(x:)` desugars to `f(x: x)`
                // Spec: named_arg = identifier ":" [ expression ]
                let value =
                    if p.cursor.check(&TokenKind::Comma) || p.cursor.check(&TokenKind::RParen) {
                        // Punning — create synthetic Expr::Ident with the argument name
                        p.arena
                            .alloc_expr(Expr::new(ExprKind::Ident(name), arg_span))
                    } else {
                        p.parse_expr().into_result()?
                    };
                (Some(name), value)
            } else {
                let value = p.parse_expr().into_result()?;
                (None, value)
            };

            let end_span = p.arena.get_expr(value).span;

            Ok(Some(CallArg {
                name,
                value,
                is_spread,
                span: arg_span.merge(end_span),
            }))
        })?;

        let has_named = args.iter().any(|a| a.name.is_some());

        Ok((args, has_named))
    }
}
