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
    ///
    /// `call_expr_id` is the AST `ExprId` of the call expression itself —
    /// used to look up the typeck-side `mono_dispatch_map` and translate
    /// any `(ExprId, MonoInstanceId)` entry to a `(CanId, MonoInstanceId)`
    /// entry on `CanonResult.mono_dispatch_map_can`.
    pub(super) fn lower_call(
        &mut self,
        call_expr_id: ExprId,
        func: ExprId,
        args: ExprRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func_kind = *self.src.expr_kind(func);
        let func = self.lower_expr(func);
        // Positional / zero-arg calls carry no argument names — route them through
        // the SAME reorder+default-fill primitive the named-call path uses so an
        // omitted trailing default is FILLED before ARC/LLVM lowering. Without this
        // the AOT/LLVM call is emitted under-arity. Over-arity args are preserved
        // verbatim (E2004 already fired in typeck); variadic extras pass through
        // unchanged.
        let src_ids: Vec<ExprId> = self.src.get_expr_list(args).to_vec();
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_ids.into_iter().map(|id| (None, id)).collect();
        let params = self.resolve_func_params(func_kind);
        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args = self.arena.push_expr_list(&lowered_args);
        let can_id = self.push(CanExpr::Call { func, args }, span, ty);
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// Iterable->Iterator desugar. When the type checker routed a
    /// method call through the iterator surface (a Set/Iterable pipeline method
    /// the receiver does not have natively), synthesize `recv.iter()` and return
    /// it as the new receiver paired with the materialized iterator type, so both
    /// method-call lowering paths thread it identically. Making the iterator a
    /// real `MethodCall` IR node lets AIMS freeze its ownership and cleanup
    /// obligations correctly (vs a backend-hidden materialization that makes a
    /// counter projection double-free element-extracting consumers like
    /// `find`). Returns `None` when the call was not routed.
    pub(crate) fn lower_iter_routed_receiver(
        &mut self,
        receiver: ExprId,
        span: Span,
    ) -> Option<(CanId, Option<ori_types::Idx>)> {
        let iter_idx = self.typed.resolve_iter_route(receiver)?;
        let iter_ty = TypeId::from_raw(iter_idx.raw());
        let lowered_receiver = self.lower_expr(receiver);
        let iter_name = self.interner.intern("iter");
        let empty_args = self.arena.push_expr_list(&[]);
        let iter_call = self.push(
            CanExpr::MethodCall {
                receiver: lowered_receiver,
                method: iter_name,
                args: empty_args,
            },
            span,
            iter_ty,
        );
        // The `receiver_ty` the callers thread into `resolve_method_params` is the
        // pool `Idx` form (matching `typed.expr_type`), NOT the node `TypeId`.
        Some((iter_call, Some(iter_idx)))
    }

    /// Lower a method-call receiver, threading the Iterable->Iterator route
    /// when the type checker recorded one, else lowering the plain
    /// receiver. Returns the lowered receiver node paired with the `Idx`-form
    /// receiver type both method-call lowering paths feed into
    /// `resolve_method_params`. Single home for the route-or-fallback the
    /// positional (`lower_method_call`) and named (`lower_method_call_named`)
    /// paths share.
    pub(crate) fn lower_method_receiver(
        &mut self,
        receiver: ExprId,
        span: Span,
    ) -> (CanId, Option<ori_types::Idx>) {
        match self.lower_iter_routed_receiver(receiver, span) {
            Some(routed) => routed,
            None => (
                self.lower_expr(receiver),
                self.typed.expr_type(receiver.index()),
            ),
        }
    }

    /// Lower a method call with positional args.
    ///
    /// Performs type-directed specialization for `collect()`: when the type
    /// checker resolved `collect()` to `Set<T>` (via bidirectional inference),
    /// rewrites the method name to `__collect_set` so the evaluator dispatches
    /// to `eval_iter_collect_set` instead of the default list collector.
    ///
    /// `call_expr_id` is the AST `ExprId` of the method-call expression —
    /// used to look up the typeck-side `mono_dispatch_map`.
    pub(super) fn lower_method_call(
        &mut self,
        call_expr_id: ExprId,
        receiver: ExprId,
        method: Name,
        args: ExprRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        // Module-alias qualified call (`alias.func(args)`): the type checker
        // recorded this call's rewrite target. Lower it as a free `Call` to the
        // qualified imported function — the namespace receiver is NOT threaded
        // as a `self` argument (it is not a value). Both backends then see one
        // shape (a plain `Call`), matching the eval `Value::ModuleNamespace`
        // result on the AOT/LLVM path.
        if let Some(qualified) = self.typed.resolve_module_alias_call(call_expr_id) {
            let src_args: Vec<(Option<Name>, ExprId)> = self
                .src
                .get_expr_list(args)
                .iter()
                .map(|&id| (None, id))
                .collect();
            return self.lower_module_alias_call(call_expr_id, qualified, &src_args, span, ty);
        }
        // Capture the SOURCE receiver's resolved type before `receiver` is shadowed
        // by its lowered CanId — used to disambiguate same-named methods across impls
        // so each receiver fills its own defaults. A routed call threads
        // `recv.iter()` as the receiver (typed as the iterator).
        let (receiver, receiver_ty) = self.lower_method_receiver(receiver, span);
        // Same positional/zero-arg default-fill as lower_call: a method
        // called with a defaulted parameter omitted positionally must fill the
        // default so the AOT/LLVM call arity matches the method signature.
        let src_ids: Vec<ExprId> = self.src.get_expr_list(args).to_vec();
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_ids.into_iter().map(|id| (None, id)).collect();
        let params = self.resolve_method_params(method, receiver_ty);
        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args = self.arena.push_expr_list(&lowered_args);
        let method = self.specialize_collect(method, ty);
        let can_id = self.push(
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            },
            span,
            ty,
        );
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// Lower a module-alias qualified call (`alias.func(args)`) — recorded by
    /// the type checker in `TypedModule::module_alias_call_map` — to a free
    /// `CanExpr::Call { func: FunctionRef(qualified), args }`. The namespace
    /// receiver is dropped (it is not a value); args are reordered + default-
    /// filled against the qualified imported function's params, exactly as a
    /// plain free call. Shared by `lower_method_call` (positional) and
    /// `desugar_method_call_named` (named).
    pub(crate) fn lower_module_alias_call(
        &mut self,
        call_expr_id: ExprId,
        qualified: Name,
        src_args: &[(Option<Name>, ExprId)],
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func = self.push(CanExpr::FunctionRef(qualified), span, ty);
        let params = self.resolve_func_params(ori_ir::ExprKind::FunctionRef(qualified));
        let lowered_args = self.reorder_and_lower_args(src_args, params.as_deref());
        let args = self.arena.push_expr_list(&lowered_args);
        let can_id = self.push(CanExpr::Call { func, args }, span, ty);
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// If the typeck-side `mono_dispatch_map` carries an entry for this AST
    /// `ExprId`, append the `(CanId, MonoInstanceId)` translation to the
    /// lowerer's accumulator. The accumulator is sorted in `Lowerer::finish`
    /// for binary-search lookup downstream (sub-steps 1d/1e/1f).
    pub(crate) fn record_mono_dispatch_if_present(&mut self, call_expr_id: ExprId, can_id: CanId) {
        if let Some(&mono_id) = self.typed.mono_dispatch_map.get(call_expr_id) {
            self.mono_dispatch_map_can.push((can_id, mono_id));
        }
    }

    /// Rewrite `collect` → `__collect_set` when the resolved type is `Set<T>`.
    ///
    /// The type checker stores the resolved return type for each expression.
    /// When bidirectional checking determined that `collect()` should produce
    /// a `Set<T>`, the `ty` will have tag `Set`. We rewrite the method name
    /// so the evaluator can dispatch to the correct collector.
    fn specialize_collect(&self, method: Name, ty: TypeId) -> Name {
        if method != self.name_collect {
            return method;
        }

        let idx = ori_types::Idx::from_raw(ty.raw());
        let tag = self.pool.tag(idx);
        if tag == ori_types::Tag::Set {
            self.name_collect_set
        } else {
            method
        }
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
                // Shorthand: `Point { x }` → synthesize `Ident(x)`. The
                // synthesized Ident's type is refined when the evaluator/codegen
                // looks up the variable binding, so it carries ERROR here.
                None => self.push(CanExpr::Ident(name), field_span, TypeId::ERROR),
            };
            can_fields.push(CanField { name, value });
        }

        self.arena.push_fields(&can_fields)
    }
}
