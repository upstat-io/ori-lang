//! Canonical lowering for calls and composite collection expressions.

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
        // INVARIANT: Positional calls receive the same default filling as named calls.
        let src_ids: Vec<ExprId> = self.src.get_expr_list(args).to_vec();
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_ids.into_iter().map(|id| (None, id)).collect();
        let params = self.resolve_func_params(func_kind);
        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let lowered_args = self.append_capability_args(call_expr_id, lowered_args, span);
        let args = self.arena.push_expr_list(&lowered_args);
        let can_id = self.push(CanExpr::Call { func, args }, span, ty);
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// Materialize a type-checked Iterable-to-Iterator route as `recv.iter()`.
    ///
    /// The explicit canonical node preserves ownership and cleanup obligations;
    /// `None` means type checking selected no iterator route.
    pub(crate) fn lower_iter_routed_receiver(
        &mut self,
        call_expr_id: ExprId,
        receiver: ExprId,
        span: Span,
    ) -> Option<(CanId, Option<ori_types::Idx>, Option<ori_types::Idx>)> {
        let route = self.typed.resolve_iter_route(call_expr_id)?;
        let lowered_receiver = self.lower_expr(receiver);
        let Some(iter_ty) = route.iter_ty else {
            return Some((
                lowered_receiver,
                self.typed.expr_type(receiver.index()),
                route.adapter_ty,
            ));
        };
        let iter_type_id = TypeId::from_raw(iter_ty.raw());
        let iter_name = self.interner.intern("iter");
        let empty_args = self.arena.push_expr_list(&[]);
        let iter_call = self.push(
            CanExpr::MethodCall {
                receiver: lowered_receiver,
                method: iter_name,
                args: empty_args,
            },
            span,
            iter_type_id,
        );
        // INVARIANT: Default lookup consumes pool indices, not canonical `TypeId`s.
        Some((iter_call, Some(iter_ty), route.adapter_ty))
    }

    /// Lower a method receiver and preserve its type-checked iterator route.
    pub(crate) fn lower_method_receiver(
        &mut self,
        call_expr_id: ExprId,
        receiver: ExprId,
        span: Span,
    ) -> (CanId, Option<ori_types::Idx>, Option<ori_types::Idx>) {
        match self.lower_iter_routed_receiver(call_expr_id, receiver, span) {
            Some(routed) => routed,
            None => (
                self.lower_expr(receiver),
                self.typed.expr_type(receiver.index()),
                None,
            ),
        }
    }

    /// Finish an eager iterator-adapter route with a concrete list collector.
    ///
    /// The adapter type is supplied by type checking and doubles as the route
    /// discriminator: terminal routes such as `Range.fold` have no adapter and
    /// return unchanged. Canonicalization does not inspect the receiver or
    /// method spelling to rediscover this semantic choice.
    pub(crate) fn finish_eager_iter_adapter(
        &mut self,
        adapter_call: CanId,
        adapter_ty: Option<ori_types::Idx>,
        span: Span,
        result_ty: TypeId,
    ) -> CanId {
        if adapter_ty.is_none() {
            return adapter_call;
        }

        let empty_args = self.arena.push_expr_list(&[]);
        self.push(
            CanExpr::MethodCall {
                receiver: adapter_call,
                method: self.name_collect,
                args: empty_args,
            },
            span,
            result_ty,
        )
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
        // Lower a module-alias-qualified call as a free call; its namespace
        // receiver is not a runtime `self` value.
        if let Some(qualified) = self.typed.resolve_module_alias_call(call_expr_id) {
            let src_args: Vec<(Option<Name>, ExprId)> = self
                .src
                .get_expr_list(args)
                .iter()
                .map(|&id| (None, id))
                .collect();
            return self.lower_module_alias_call(call_expr_id, qualified, &src_args, span, ty);
        }
        // Preserve the source receiver type before lowering so same-named impl
        // methods fill defaults from the correct signature.
        let (receiver, receiver_ty, adapter_ty) =
            self.lower_method_receiver(call_expr_id, receiver, span);
        // Same positional/zero-arg default-fill as lower_call: a method
        // called with a defaulted parameter omitted positionally must fill the
        // default so the AOT/LLVM call arity matches the method signature.
        let src_ids: Vec<ExprId> = self.src.get_expr_list(args).to_vec();
        let src_args: Vec<(Option<Name>, ExprId)> =
            src_ids.into_iter().map(|id| (None, id)).collect();
        let params = self.resolve_method_params(method, receiver_ty);
        let lowered_args = self.reorder_and_lower_args(&src_args, params.as_deref());
        let args = self.arena.push_expr_list(&lowered_args);
        let method = self.specialize_collect(call_expr_id, method);
        let method_ty = adapter_ty.map_or(ty, |idx| TypeId::from_raw(idx.raw()));
        let adapter_call = self.push(
            CanExpr::MethodCall {
                receiver,
                method,
                args,
            },
            span,
            method_ty,
        );
        let can_id = self.finish_eager_iter_adapter(adapter_call, adapter_ty, span, ty);
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
        let lowered_args = self.append_capability_args(call_expr_id, lowered_args, span);
        let args = self.arena.push_expr_list(&lowered_args);
        let can_id = self.push(CanExpr::Call { func, args }, span, ty);
        self.record_mono_dispatch_if_present(call_expr_id, can_id);
        can_id
    }

    /// If the typeck-side `mono_dispatch_map` carries an entry for this AST
    /// `ExprId`, append the `(CanId, MonoInstanceId)` translation to the
    /// lowerer's accumulator. `Lowerer::finish` sorts the accumulator for
    /// binary-search lookup (sub-steps 1d/1e/1f).
    pub(crate) fn record_mono_dispatch_if_present(&mut self, call_expr_id: ExprId, can_id: CanId) {
        if let Some(&mono_id) = self.typed.mono_dispatch_map.get(call_expr_id) {
            self.mono_dispatch_map_can.push((can_id, mono_id));
        }
    }

    /// Append the type-checker-selected implicit provider values in source
    /// `uses` order. The capability namespace is already bound to the exact
    /// lexical provider by either `with ... in` or the callee's own hidden
    /// parameter, so Canon materializes a normal identifier operand while the
    /// sidecar supplies its concrete type.
    pub(crate) fn append_capability_args(
        &mut self,
        call_expr_id: ExprId,
        mut args: Vec<CanId>,
        span: Span,
    ) -> Vec<CanId> {
        let providers = self
            .typed
            .resolve_capability_call(call_expr_id)
            .map(|site| site.providers.clone())
            .unwrap_or_default();
        args.reserve(providers.len());
        for provider in providers {
            args.push(self.push(
                CanExpr::Ident(provider.capability),
                span,
                TypeId::from_raw(provider.provider_type.raw()),
            ));
        }
        args
    }

    /// Apply the exact `Collect` target selected by type checking.
    ///
    /// Absence of an exact call-site route preserves the selected method
    /// identity, even when an unrelated user method returns `Set<T>`.
    fn specialize_collect(&self, call_expr_id: ExprId, method: Name) -> Name {
        let Some(collect_ty) = self
            .typed
            .resolve_iter_route(call_expr_id)
            .and_then(|route| route.collect_ty)
        else {
            return method;
        };
        let resolved = self.pool.resolve_fully(collect_ty);
        match self.pool.tag(resolved) {
            ori_types::Tag::Set => self.name_collect_set,
            _ => method,
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
