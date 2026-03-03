//! Sugar elimination during lowering.
//!
//! Called by `lower.rs` to desugar the 7 sugar `ExprKind` variants into
//! compositions of primitive `CanExpr` nodes:
//!
//! | Sugar | Desugared to |
//! |-------|-------------|
//! | `CallNamed` | `Call` (args reordered to positional) |
//! | `MethodCallNamed` | `MethodCall` (args reordered) |
//! | `TemplateFull` | `Str` (handled inline in lower.rs) |
//! | `TemplateLiteral` | `Str` + `.to_str()` / `FormatWith` + `.concat()` chain |
//! | `ListWithSpread` | `List` + `.concat()` chains |
//! | `MapWithSpread` | `Map` + `.merge()` chains |
//! | `StructWithSpread` | `Struct` with all fields resolved via `Field` access |
//!
//! See `eval_v2` Section 02.3 for the full desugaring specification.

mod calls;
mod spread;

use ori_ir::canon::CanExpr;
use ori_ir::{Name, Span, TemplatePartRange, TypeId};

use crate::lower::Lowerer;

impl Lowerer<'_> {
    // TemplateLiteral → .concat() chain

    /// Desugar `` `head {expr1} mid {expr2} tail` `` into a chain of
    /// `.concat()` calls:
    ///
    /// ```text
    /// "head".concat(expr1.to_str()).concat("mid").concat(expr2.to_str()).concat("tail")
    /// ```
    pub(crate) fn desugar_template_literal(
        &mut self,
        head: Name,
        parts: TemplatePartRange,
        span: Span,
        _ty: TypeId,
    ) -> ori_ir::canon::CanId {
        // Start with the head text segment.
        let mut result = self.push(CanExpr::Str(head), span, TypeId::STR);

        // Get template parts (copy out for borrow safety).
        let src_parts = self.src.get_template_parts(parts);
        let src_parts: Vec<(ori_ir::ExprId, Name, Name)> = src_parts
            .iter()
            .map(|p| (p.expr, p.format_spec, p.text_after))
            .collect();

        for (expr_id, format_spec, text_after) in src_parts {
            // Lower the interpolated expression.
            let expr = self.lower_expr(expr_id);
            let expr_ty = self.arena.ty(expr);

            // If a format spec is present, use FormatWith (even for strings —
            // they may need width/alignment/precision).
            // Otherwise, wrap non-string expressions in .to_str().
            let str_expr = if format_spec != Name::EMPTY {
                self.push(
                    CanExpr::FormatWith {
                        expr,
                        spec: format_spec,
                    },
                    span,
                    TypeId::STR,
                )
            } else if expr_ty == TypeId::STR {
                expr
            } else {
                let empty_args = self.arena.push_expr_list(&[]);
                self.push(
                    CanExpr::MethodCall {
                        receiver: expr,
                        method: self.name_to_str,
                        args: empty_args,
                    },
                    span,
                    TypeId::STR,
                )
            };

            // Chain: result = result.concat(str_expr)
            let concat_args = self.arena.push_expr_list(&[str_expr]);
            result = self.push(
                CanExpr::MethodCall {
                    receiver: result,
                    method: self.name_concat,
                    args: concat_args,
                },
                span,
                TypeId::STR,
            );

            // If there's text after this interpolation, concat it too.
            if text_after != Name::EMPTY {
                let text_node = self.push(CanExpr::Str(text_after), span, TypeId::STR);
                let text_args = self.arena.push_expr_list(&[text_node]);
                result = self.push(
                    CanExpr::MethodCall {
                        receiver: result,
                        method: self.name_concat,
                        args: text_args,
                    },
                    span,
                    TypeId::STR,
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests;
