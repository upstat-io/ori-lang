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
//! | `TemplateLiteral` | `Str` + `.to_str()` + `.concat()` chain |
//! | `ListWithSpread` | `List` + `.concat()` chains |
//! | `MapWithSpread` | `Map` + `.merge()` chains |
//! | `StructWithSpread` | `Struct` with all fields resolved via `Field` access |
//!
//! See `eval_v2` Section 02.3 for the full desugaring specification.

use ori_ir::canon::{CanExpr, CanField, CanId, CanMapEntry};
use ori_ir::{
    CallArgRange, ExprId, ListElementRange, MapElementRange, Name, Span, StructLitFieldRange,
    TemplatePartRange, TypeId,
};

use crate::lower::Lowerer;

impl Lowerer<'_> {
    // CallNamed → Call

    /// Desugar `CallNamed { func, args: CallArgRange }` to `Call { func, args: CanRange }`.
    ///
    /// Named arguments are reordered to match the function signature's parameter
    /// order. If the function signature is unavailable (error recovery, lambdas),
    /// arguments are kept in source order.
    pub(crate) fn desugar_call_named(
        &mut self,
        func: ExprId,
        args: CallArgRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func_kind = *self.src.expr_kind(func);
        let lowered_func = self.lower_expr(func);

        // Get source call arguments (copy out to avoid borrow conflict).
        let src_args = self.src.get_call_args(args);
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_args.iter().map(|a| (a.name, a.value)).collect();

        // Try to resolve the function signature for reordering and default filling.
        let params = self.resolve_func_params(func_kind);

        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args_range = self.arena.push_expr_list(&lowered_args);

        self.push(
            CanExpr::Call {
                func: lowered_func,
                args: args_range,
            },
            span,
            ty,
        )
    }

    // MethodCallNamed → MethodCall

    /// Desugar `MethodCallNamed { receiver, method, args }` to `MethodCall`.
    ///
    /// Same reordering logic as `CallNamed` but looks up the method signature
    /// from `impl_sigs`.
    pub(crate) fn desugar_method_call_named(
        &mut self,
        receiver: ExprId,
        method: Name,
        args: CallArgRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let lowered_receiver = self.lower_expr(receiver);

        // Get source call arguments.
        let src_args = self.src.get_call_args(args);
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_args.iter().map(|a| (a.name, a.value)).collect();

        // Try to resolve the method signature for reordering and default filling.
        let params = self.resolve_method_params(method);

        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args_range = self.arena.push_expr_list(&lowered_args);

        self.push(
            CanExpr::MethodCall {
                receiver: lowered_receiver,
                method,
                args: args_range,
            },
            span,
            ty,
        )
    }

    /// Reorder named arguments to match parameter order, filling omitted
    /// parameters with their default expressions.
    ///
    /// If `params` is available, arguments with names are placed in the
    /// corresponding parameter position. Unnamed/positional arguments fill
    /// remaining slots left-to-right. Empty slots are filled by lowering the
    /// parameter's default expression. If `params` is `None`, arguments stay
    /// in source order (fallback for lambdas and error recovery).
    fn reorder_and_lower_args(
        &mut self,
        src_args: &[(Option<Name>, ExprId)],
        params: Option<&[(Name, Option<ExprId>)]>,
    ) -> Vec<CanId> {
        match params {
            Some(params) if !params.is_empty() => {
                // Build positional slots matching parameter count.
                let mut slots: Vec<Option<CanId>> = vec![None; params.len()];
                let mut unnamed = Vec::new();

                for &(name, value) in src_args {
                    let lowered = self.lower_expr(value);
                    if let Some(arg_name) = name {
                        // Find the parameter position by name.
                        if let Some(pos) = params.iter().position(|(p, _)| *p == arg_name) {
                            slots[pos] = Some(lowered);
                        } else {
                            // Unknown param name — append as-is (error recovery).
                            unnamed.push(lowered);
                        }
                    } else {
                        unnamed.push(lowered);
                    }
                }

                // Fill empty slots: first try unnamed positional args, then defaults.
                let mut unnamed_iter = unnamed.into_iter();
                for (i, slot) in slots.iter_mut().enumerate() {
                    if slot.is_none() {
                        if let Some(val) = unnamed_iter.next() {
                            *slot = Some(val);
                        } else if let Some(default_expr) = params[i].1 {
                            // Lower the default expression from the function signature.
                            *slot = Some(self.lower_expr(default_expr));
                        }
                    }
                }

                // Collect: all slots (filled by named args, positional args, or defaults),
                // then any remaining unnamed args (error recovery — more args than params).
                let mut result: Vec<CanId> = slots.into_iter().flatten().collect();
                result.extend(unnamed_iter);
                result
            }
            _ => {
                // No signature available — keep source order.
                src_args
                    .iter()
                    .map(|&(_, value)| self.lower_expr(value))
                    .collect()
            }
        }
    }

    /// Try to resolve parameter info (names + defaults) from a function expression.
    fn resolve_func_params(
        &self,
        func_kind: ori_ir::ExprKind,
    ) -> Option<Vec<(Name, Option<ExprId>)>> {
        let (ori_ir::ExprKind::Ident(name) | ori_ir::ExprKind::FunctionRef(name)) = func_kind
        else {
            return None;
        };
        self.typed.function(name).map(|sig| {
            sig.param_names
                .iter()
                .zip(
                    sig.param_defaults
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(None)),
                )
                .map(|(&name, default)| (name, default))
                .collect()
        })
    }

    /// Try to resolve parameter info (names + defaults) from a method signature.
    fn resolve_method_params(&self, method: Name) -> Option<Vec<(Name, Option<ExprId>)>> {
        self.typed
            .impl_sigs
            .iter()
            .find(|(name, _)| *name == method)
            .map(|(_, sig)| {
                sig.param_names
                    .iter()
                    .zip(
                        sig.param_defaults
                            .iter()
                            .copied()
                            .chain(std::iter::repeat(None)),
                    )
                    .map(|(&name, default)| (name, default))
                    .collect()
            })
    }

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
    ) -> CanId {
        // Start with the head text segment.
        let mut result = self.push(CanExpr::Str(head), span, TypeId::STR);

        // Get template parts (copy out for borrow safety).
        let src_parts = self.src.get_template_parts(parts);
        let src_parts: Vec<(ExprId, Name, Name)> = src_parts
            .iter()
            .map(|p| (p.expr, p.format_spec, p.text_after))
            .collect();

        for (expr_id, _format_spec, text_after) in src_parts {
            // Lower the interpolated expression.
            let expr = self.lower_expr(expr_id);
            let expr_ty = self.arena.ty(expr);

            // If the expression isn't already a string, wrap in .to_str().
            let str_expr = if expr_ty == TypeId::STR {
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

    // ListWithSpread → List + .concat()

    /// Desugar `[a, b, ...c, d, ...e]` into:
    ///
    /// ```text
    /// [a, b].concat(c).concat([d]).concat(e)
    /// ```
    ///
    /// Groups consecutive non-spread elements into `List` literals, then
    /// chains all segments left-to-right via `.concat()`.
    pub(crate) fn desugar_list_with_spread(
        &mut self,
        elements: ListElementRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let src_elements = self.src.get_list_elements(elements);

        // Copy out element data (is_spread, expr_id) to avoid borrow conflict.
        let mut element_data: Vec<(bool, ExprId)> = Vec::with_capacity(src_elements.len());
        for elem in src_elements {
            element_data.push(match elem {
                ori_ir::ListElement::Expr { expr, .. } => (false, *expr),
                ori_ir::ListElement::Spread { expr, .. } => (true, *expr),
            });
        }

        // Group consecutive non-spread elements into list segments.
        let mut segments: Vec<CanId> = Vec::new();
        let mut current_group: Vec<CanId> = Vec::new();

        for (is_spread, expr_id) in element_data {
            if is_spread {
                // Flush current non-spread group as a List.
                if !current_group.is_empty() {
                    let range = self.arena.push_expr_list(&current_group);
                    segments.push(self.push(CanExpr::List(range), span, ty));
                    current_group.clear();
                }
                // The spread expression itself is a segment.
                segments.push(self.lower_expr(expr_id));
            } else {
                current_group.push(self.lower_expr(expr_id));
            }
        }

        // Flush trailing non-spread group.
        if !current_group.is_empty() {
            let range = self.arena.push_expr_list(&current_group);
            segments.push(self.push(CanExpr::List(range), span, ty));
        }

        // Chain all segments via .concat().
        self.chain_method_calls(segments, self.name_concat, span, ty)
    }

    // MapWithSpread → Map + .merge()

    /// Desugar `{k1: v1, ...base, k2: v2}` into:
    ///
    /// ```text
    /// {k1: v1}.merge(base).merge({k2: v2})
    /// ```
    pub(crate) fn desugar_map_with_spread(
        &mut self,
        elements: MapElementRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        enum MapSegment {
            Entry(ExprId, ExprId),
            Spread(ExprId),
        }

        let src_elements = self.src.get_map_elements(elements);

        // Copy out element data to avoid borrow conflict.
        let mut element_data: Vec<MapSegment> = Vec::with_capacity(src_elements.len());
        for elem in src_elements {
            element_data.push(match elem {
                ori_ir::MapElement::Entry(entry) => MapSegment::Entry(entry.key, entry.value),
                ori_ir::MapElement::Spread { expr, .. } => MapSegment::Spread(*expr),
            });
        }

        // Group consecutive entries into map segments.
        let mut segments: Vec<CanId> = Vec::new();
        let mut current_entries: Vec<CanMapEntry> = Vec::new();

        for elem in element_data {
            match elem {
                MapSegment::Entry(key, value) => {
                    let key = self.lower_expr(key);
                    let value = self.lower_expr(value);
                    current_entries.push(CanMapEntry { key, value });
                }
                MapSegment::Spread(expr_id) => {
                    // Flush current entry group as a Map.
                    if !current_entries.is_empty() {
                        let range = self.arena.push_map_entries(&current_entries);
                        segments.push(self.push(CanExpr::Map(range), span, ty));
                        current_entries.clear();
                    }
                    segments.push(self.lower_expr(expr_id));
                }
            }
        }

        // Flush trailing entry group.
        if !current_entries.is_empty() {
            let range = self.arena.push_map_entries(&current_entries);
            segments.push(self.push(CanExpr::Map(range), span, ty));
        }

        // Chain all segments via .merge().
        self.chain_method_calls(segments, self.name_merge, span, ty)
    }

    // StructWithSpread → Struct

    /// Desugar `Point { ...base, x: 10 }` into a flat `Struct` with all fields
    /// resolved by extracting individual fields from the spread expression.
    ///
    /// Strategy:
    /// 1. Look up the struct definition to get all field names in order.
    /// 2. Walk the source fields left-to-right:
    ///    - `Field(init)` → sets that field's value
    ///    - `Spread { expr }` → for ALL fields, set value to `expr.field_name`
    /// 3. "Later wins" — explicit fields after a spread override the spread.
    /// 4. Emit a flat `CanExpr::Struct` with all fields.
    pub(crate) fn desugar_struct_with_spread(
        &mut self,
        name: Name,
        fields: StructLitFieldRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        enum FieldData {
            Init {
                name: Name,
                value: Option<ExprId>,
                span: Span,
            },
            Spread {
                expr: ExprId,
                span: Span,
            },
        }

        let src_fields = self.src.get_struct_lit_fields(fields);

        // Copy out field data to avoid borrow conflict.
        let mut field_data: Vec<FieldData> = Vec::with_capacity(src_fields.len());
        for f in src_fields {
            field_data.push(match f {
                ori_ir::StructLitField::Field(init) => FieldData::Init {
                    name: init.name,
                    value: init.value,
                    span: init.span,
                },
                ori_ir::StructLitField::Spread { expr, span } => FieldData::Spread {
                    expr: *expr,
                    span: *span,
                },
            });
        }

        // Look up the struct definition for field ordering.
        let struct_field_names = self.resolve_struct_fields(name);

        if let Some(field_names) = struct_field_names {
            // We know the struct layout — build a fully resolved field list.
            let mut field_values: Vec<Option<CanId>> = vec![None; field_names.len()];

            for field in &field_data {
                match field {
                    FieldData::Init {
                        name: field_name,
                        value,
                        span: field_span,
                    } => {
                        let field_name = *field_name;
                        let field_span = *field_span;
                        if let Some(pos) = field_names.iter().position(|n| *n == field_name) {
                            let val = match value {
                                Some(expr_id) => self.lower_expr(*expr_id),
                                None => {
                                    self.push(CanExpr::Ident(field_name), field_span, TypeId::ERROR)
                                }
                            };
                            field_values[pos] = Some(val);
                        }
                    }
                    FieldData::Spread {
                        expr: spread_expr,
                        span: spread_span,
                    } => {
                        let spread = self.lower_expr(*spread_expr);
                        for (i, field_name) in field_names.iter().enumerate() {
                            let field_access = self.push(
                                CanExpr::Field {
                                    receiver: spread,
                                    field: *field_name,
                                },
                                *spread_span,
                                TypeId::ERROR,
                            );
                            field_values[i] = Some(field_access);
                        }
                    }
                }
            }

            // Build the canonical fields.
            let can_fields: Vec<CanField> = field_names
                .iter()
                .zip(field_values)
                .map(|(fname, value)| {
                    let value = value.unwrap_or_else(|| {
                        // Missing field — emit Error (type checker should catch this).
                        self.push(CanExpr::Error, span, TypeId::ERROR)
                    });
                    CanField {
                        name: *fname,
                        value,
                    }
                })
                .collect();

            let fields_range = self.arena.push_fields(&can_fields);
            self.push(
                CanExpr::Struct {
                    name,
                    fields: fields_range,
                },
                span,
                ty,
            )
        } else {
            // Struct definition not found — fall back to lowering fields in order.
            // This handles error recovery gracefully.
            let mut can_fields = Vec::new();
            for field in &field_data {
                match field {
                    FieldData::Init {
                        name: field_name,
                        value,
                        span: field_span,
                    } => {
                        let field_name = *field_name;
                        let field_span = *field_span;
                        let val = match value {
                            Some(expr_id) => self.lower_expr(*expr_id),
                            None => {
                                self.push(CanExpr::Ident(field_name), field_span, TypeId::ERROR)
                            }
                        };
                        can_fields.push(CanField {
                            name: field_name,
                            value: val,
                        });
                    }
                    FieldData::Spread { .. } => {
                        // Struct definition not found — skip lowering the spread
                        // expression to avoid allocating orphaned nodes in the arena.
                    }
                }
            }
            let fields_range = self.arena.push_fields(&can_fields);
            self.push(
                CanExpr::Struct {
                    name,
                    fields: fields_range,
                },
                span,
                ty,
            )
        }
    }

    // Shared Helpers

    /// Chain a list of segments via left-to-right method calls.
    ///
    /// `[a, b, c]` with method `concat` becomes: `a.concat(b).concat(c)`
    ///
    /// Returns the first segment directly if there's only one (no chaining needed).
    fn chain_method_calls(
        &mut self,
        segments: Vec<CanId>,
        method: Name,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let mut iter = segments.into_iter();
        let Some(first) = iter.next() else {
            // Empty — return an empty collection. Callers should handle
            // this case, but emit Error for safety.
            return self.push(CanExpr::Error, span, ty);
        };

        iter.fold(first, |acc, segment| {
            let args = self.arena.push_expr_list(&[segment]);
            self.push(
                CanExpr::MethodCall {
                    receiver: acc,
                    method,
                    args,
                },
                span,
                ty,
            )
        })
    }

    /// Look up struct field names in order from the type registry.
    fn resolve_struct_fields(&self, name: Name) -> Option<Vec<Name>> {
        let type_entry = self.typed.type_def(name)?;
        match &type_entry.kind {
            ori_types::TypeKind::Struct(def) => Some(def.fields.iter().map(|f| f.name).collect()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lower::lower;
    use ori_ir::ast::{CallArg, Expr, TemplatePart};
    use ori_ir::{ExprArena, ExprKind, SharedInterner, Span};
    use ori_types::{Idx, TypeCheckResult, TypedModule};

    fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
        let mut typed = TypedModule::new();
        for idx in expr_types {
            typed.expr_types.push(idx);
        }
        TypeCheckResult::ok(typed)
    }

    fn test_interner() -> SharedInterner {
        SharedInterner::new()
    }

    #[test]
    fn desugar_call_named_source_order_fallback() {
        // When no function signature is available, args stay in source order.
        let mut arena = ExprArena::new();
        let interner = test_interner();

        let func_name = interner.intern("unknown_fn");
        let arg_a_name = interner.intern("a");
        let arg_b_name = interner.intern("b");

        let func = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 3)));
        let val1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));
        let val2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(8, 9)));

        let args = arena.alloc_call_args([
            CallArg {
                name: Some(arg_b_name),
                value: val2,
                is_spread: false,
                span: Span::new(7, 10),
            },
            CallArg {
                name: Some(arg_a_name),
                value: val1,
                is_spread: false,
                span: Span::new(4, 6),
            },
        ]);

        let root = arena.alloc_expr(Expr::new(
            ExprKind::CallNamed { func, args },
            Span::new(0, 11),
        ));

        // No function sig → source order (b=2, a=1).
        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Call { args, .. } => {
                let arg_list = result.arena.get_expr_list(*args);
                assert_eq!(arg_list.len(), 2);
                // Source order preserved: b=2 first, a=1 second.
                assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(2));
                assert_eq!(*result.arena.kind(arg_list[1]), CanExpr::Int(1));
            }
            other => panic!("expected Call, got {other:?}"),
        }
    }

    #[test]
    fn desugar_template_literal_simple() {
        // `hello {name}!` → "hello".concat(name.to_str()).concat("!")
        let mut arena = ExprArena::new();
        let interner = test_interner();

        let head = interner.intern("hello ");
        let tail = interner.intern("!");
        let var_name = interner.intern("name");

        let expr = arena.alloc_expr(Expr::new(ExprKind::Ident(var_name), Span::new(8, 12)));

        let parts = arena.alloc_template_parts([TemplatePart {
            expr,
            format_spec: Name::EMPTY,
            text_after: tail,
        }]);

        let root = arena.alloc_expr(Expr::new(
            ExprKind::TemplateLiteral { head, parts },
            Span::new(0, 14),
        ));

        // expr_types: [0]=Ident(name):str, [1]=TemplateLiteral:str
        let type_result = test_type_result(vec![Idx::STR, Idx::STR]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(result.root.is_valid());

        // The root should be a concat chain. Since `name` is already str,
        // no to_str wrapping needed. Result is:
        // "hello ".concat(name).concat("!")
        // The final concat("!") is the root.
        match result.arena.kind(result.root) {
            CanExpr::MethodCall { method, .. } => {
                let concat = interner.intern("concat");
                assert_eq!(*method, concat);
            }
            other => panic!("expected MethodCall(concat), got {other:?}"),
        }
    }

    #[test]
    fn desugar_list_with_spread_simple() {
        // [1, ...xs, 2] → [1].concat(xs).concat([2])
        let mut arena = ExprArena::new();
        let interner = test_interner();

        let xs_name = interner.intern("xs");

        let e1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
        let xs = arena.alloc_expr(Expr::new(ExprKind::Ident(xs_name), Span::new(7, 9)));
        let e2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(11, 12)));

        let elements = arena.alloc_list_elements([
            ori_ir::ListElement::Expr {
                expr: e1,
                span: Span::new(1, 2),
            },
            ori_ir::ListElement::Spread {
                expr: xs,
                span: Span::new(4, 9),
            },
            ori_ir::ListElement::Expr {
                expr: e2,
                span: Span::new(11, 12),
            },
        ]);

        let root = arena.alloc_expr(Expr::new(
            ExprKind::ListWithSpread(elements),
            Span::new(0, 13),
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(result.root.is_valid());

        // Root should be: [1].concat(xs).concat([2])
        // That's a chain of MethodCall(concat).
        match result.arena.kind(result.root) {
            CanExpr::MethodCall { method, .. } => {
                let concat = interner.intern("concat");
                assert_eq!(*method, concat);
            }
            other => panic!("expected MethodCall(concat), got {other:?}"),
        }
    }
}
