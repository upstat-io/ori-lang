//! Collection and container lowering — calls, blocks, lists, tuples, maps, structs.
//!
//! Handles lowering of composite expression types that contain child
//! expressions or ranges: function calls, method calls, blocks, and
//! literal collection constructors.

use ori_ir::canon::{CanExpr, CanField, CanId, CanMapEntry};
use ori_ir::{ExprId, ExprRange, Name, Span, TypeId};

use super::Lowerer;

impl Lowerer<'_> {
    /// Lower a function call with positional args.
    pub(super) fn lower_call(
        &mut self,
        func: ExprId,
        args: ExprRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func = self.lower_expr(func);
        let args = self.lower_expr_range(args);
        self.push(CanExpr::Call { func, args }, span, ty)
    }

    /// Lower a method call with positional args.
    pub(super) fn lower_method_call(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let receiver = self.lower_expr(receiver);
        let args = self.lower_expr_range(args);
        self.push(
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            },
            span,
            ty,
        )
    }

    /// Lower a block expression.
    pub(super) fn lower_block(
        &mut self,
        stmts: ori_ir::StmtRange,
        result: ExprId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let stmts = self.lower_stmt_range(stmts);
        let result = self.lower_optional(result);
        self.push(CanExpr::Block { stmts, result }, span, ty)
    }

    /// Lower a list literal.
    pub(super) fn lower_list(&mut self, exprs: ExprRange, span: Span, ty: TypeId) -> CanId {
        let range = self.lower_expr_range(exprs);
        self.push(CanExpr::List(range), span, ty)
    }

    /// Lower a tuple literal.
    pub(super) fn lower_tuple(&mut self, exprs: ExprRange, span: Span, ty: TypeId) -> CanId {
        let range = self.lower_expr_range(exprs);
        self.push(CanExpr::Tuple(range), span, ty)
    }

    /// Lower a map literal (no spread).
    pub(super) fn lower_map(
        &mut self,
        entries: ori_ir::MapEntryRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let range = self.lower_map_entries(entries);
        self.push(CanExpr::Map(range), span, ty)
    }

    /// Lower map entries from the source arena to canonical map entries.
    pub(crate) fn lower_map_entries(
        &mut self,
        range: ori_ir::MapEntryRange,
    ) -> ori_ir::canon::CanMapEntryRange {
        let src_entries = self.src.get_map_entries(range);
        if src_entries.is_empty() {
            return ori_ir::canon::CanMapEntryRange::EMPTY;
        }

        // Copy out to avoid borrow conflict.
        let src_entries: Vec<(ExprId, ExprId)> =
            src_entries.iter().map(|e| (e.key, e.value)).collect();
        let mut can_entries = Vec::with_capacity(src_entries.len());

        for (key, value) in src_entries {
            let key = self.lower_expr(key);
            let value = self.lower_expr(value);
            can_entries.push(CanMapEntry { key, value });
        }

        self.arena.push_map_entries(&can_entries)
    }

    /// Lower a struct literal (no spread).
    pub(super) fn lower_struct(
        &mut self,
        name: Name,
        fields: ori_ir::FieldInitRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let range = self.lower_field_inits(fields);
        self.push(
            CanExpr::Struct {
                name,
                fields: range,
            },
            span,
            ty,
        )
    }

    /// Lower struct field initializers from the source arena.
    ///
    /// Handles the shorthand syntax: `FieldInit { name, value: None }` is
    /// desugared to `CanExpr::Ident(name)` (implicit variable reference).
    pub(crate) fn lower_field_inits(
        &mut self,
        range: ori_ir::FieldInitRange,
    ) -> ori_ir::canon::CanFieldRange {
        let src_fields = self.src.get_field_inits(range);
        if src_fields.is_empty() {
            return ori_ir::canon::CanFieldRange::EMPTY;
        }

        // Copy out to avoid borrow conflict.
        let src_fields: Vec<(Name, Option<ExprId>, Span)> = src_fields
            .iter()
            .map(|f| (f.name, f.value, f.span))
            .collect();
        let mut can_fields = Vec::with_capacity(src_fields.len());

        for (name, value, field_span) in src_fields {
            let value = match value {
                Some(expr_id) => self.lower_expr(expr_id),
                // Shorthand: `Point { x }` → synthesize `Ident(x)`.
                None => self.push(
                    CanExpr::Ident(name),
                    field_span,
                    Self::resolve_field_type(name),
                ),
            };
            can_fields.push(CanField { name, value });
        }

        self.arena.push_fields(&can_fields)
    }

    /// Resolve the type for a field shorthand reference.
    ///
    /// For `Point { x }`, we need the type of the variable `x`.
    /// This is best-effort — falls back to ERROR if we can't determine it.
    fn resolve_field_type(_name: Name) -> TypeId {
        // The synthesized Ident will get its type from context.
        // For now, use ERROR as the type — it will be refined once
        // the evaluator/codegen looks up the variable binding.
        TypeId::ERROR
    }
}
