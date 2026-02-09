//! AST → Canonical IR lowering.
//!
//! Transforms every `ExprKind` variant into its `CanExpr` equivalent:
//! - 39 variants mapped directly (child references remapped from `ExprId` to `CanId`)
//! - 7 sugar variants desugared into compositions of primitive `CanExpr` nodes
//! - 1 error variant mapped to `CanExpr::Error`
//!
//! See `eval_v2` Section 02 for the full lowering specification.

use ori_ir::ast::items::Module;
use ori_ir::canon::{
    CanArena, CanBindingPattern, CanBindingPatternId, CanExpr, CanField, CanFieldBinding, CanId,
    CanMapEntry, CanNamedExpr, CanNode, CanParam, CanRange, CanonResult, ConstantPool,
    DecisionTreePool,
};
use ori_ir::{ExprArena, ExprId, ExprKind, ExprRange, Name, Span, TypeId};
use ori_types::{TypeCheckResult, TypedModule};

use ori_ir::StringInterner;

/// Lower a type-checked AST to canonical form.
///
/// This is the main entry point for canonicalization. It transforms the
/// entire expression tree, desugaring syntax, attaching types, and building
/// the canonical arena.
///
/// # Arguments
///
/// - `src`: The source expression arena from parsing.
/// - `type_result`: The type check result containing type assignments and function signatures.
/// - `root`: The root expression ID to start lowering from.
/// - `interner`: Shared string interner for name resolution and creation.
///
/// # Returns
///
/// A `CanonResult` containing the canonical arena, constant pool, decision trees,
/// and the root canonical expression ID.
pub fn lower(
    src: &ExprArena,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
    root: ExprId,
    interner: &StringInterner,
) -> CanonResult {
    if !root.is_valid() {
        return CanonResult::empty();
    }

    let mut lowerer = Lowerer::new(src, &type_result.typed, pool, interner);
    let can_root = lowerer.lower_expr(root);
    let result = lowerer.finish(can_root);

    #[cfg(debug_assertions)]
    crate::validate(&result);

    result
}

/// Lower a complete module to canonical form.
///
/// Iterates all functions in the module, lowering each body into the same
/// `CanArena`. The result contains named roots mapping function names to
/// their canonical entry points.
///
/// # Arguments
///
/// - `module`: The parsed module containing function definitions.
/// - `src`: The source expression arena.
/// - `type_result`: Type check result with type assignments.
/// - `pool`: Type pool for variant/field resolution.
/// - `interner`: Shared string interner.
///
/// # Returns
///
/// A `CanonResult` with all functions lowered and named roots populated.
pub fn lower_module(
    module: &Module,
    src: &ExprArena,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
    interner: &StringInterner,
) -> CanonResult {
    let mut lowerer = Lowerer::new(src, &type_result.typed, pool, interner);
    let mut roots = Vec::with_capacity(module.functions.len() + module.tests.len());

    // Group functions by name to detect multi-clause definitions.
    let mut func_groups: rustc_hash::FxHashMap<Name, Vec<&ori_ir::Function>> =
        rustc_hash::FxHashMap::default();
    for func in &module.functions {
        func_groups.entry(func.name).or_default().push(func);
    }

    // Lower each function/group body, recording name → CanId.
    // Iteration order must match `module.functions` for compatibility with
    // `register_module_functions` which uses the same ordering.
    let mut seen_names: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();
    for func in &module.functions {
        if !seen_names.insert(func.name) {
            continue; // Already handled this group.
        }
        let group = &func_groups[&func.name];
        if group.len() == 1 {
            // Single clause — lower normally.
            if func.body.is_valid() {
                let can_id = lowerer.lower_expr(func.body);
                roots.push((func.name, can_id));
            }
        } else {
            // Multi-clause — synthesize a match body.
            let can_id = lowerer.lower_multi_clause(group);
            roots.push((func.name, can_id));
        }
    }

    // Lower each test body into the same arena.
    for test in &module.tests {
        if test.body.is_valid() {
            let can_id = lowerer.lower_expr(test.body);
            roots.push((test.name, can_id));
        }
    }

    // Lower impl/extend/def_impl method bodies for canonical user method dispatch.
    let mut method_roots = Vec::new();

    // Build trait default method map for unoverridden defaults.
    let mut trait_defaults: rustc_hash::FxHashMap<Name, Vec<&ori_ir::TraitDefaultMethod>> =
        rustc_hash::FxHashMap::default();
    for trait_def in &module.traits {
        for item in &trait_def.items {
            if let ori_ir::TraitItem::DefaultMethod(dm) = item {
                trait_defaults.entry(trait_def.name).or_default().push(dm);
            }
        }
    }

    for impl_def in &module.impls {
        let Some(&type_name) = impl_def.self_path.last() else {
            continue;
        };

        // Collect overridden method names.
        let mut overridden: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();

        for method in &impl_def.methods {
            overridden.insert(method.name);
            if method.body.is_valid() {
                let can_id = lowerer.lower_expr(method.body);
                method_roots.push((type_name, method.name, can_id));
            }
        }

        // Lower default trait methods that weren't overridden.
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                if let Some(defaults) = trait_defaults.get(&trait_name) {
                    for dm in defaults {
                        if !overridden.contains(&dm.name) && dm.body.is_valid() {
                            let can_id = lowerer.lower_expr(dm.body);
                            method_roots.push((type_name, dm.name, can_id));
                        }
                    }
                }
            }
        }
    }

    for extend_def in &module.extends {
        for method in &extend_def.methods {
            if method.body.is_valid() {
                let can_id = lowerer.lower_expr(method.body);
                method_roots.push((extend_def.target_type_name, method.name, can_id));
            }
        }
    }

    for def_impl_def in &module.def_impls {
        for method in &def_impl_def.methods {
            if method.body.is_valid() {
                let can_id = lowerer.lower_expr(method.body);
                method_roots.push((def_impl_def.trait_name, method.name, can_id));
            }
        }
    }

    // Use the first function's root as the primary root (for single-expression compat).
    let root = roots.first().map_or(CanId::INVALID, |(_, id)| *id);

    let mut result = lowerer.finish(root);
    result.roots = roots;
    result.method_roots = method_roots;

    #[cfg(debug_assertions)]
    crate::validate(&result);

    result
}

// ── Lowerer ─────────────────────────────────────────────────────────

/// State for the AST-to-CanonIR lowering pass.
///
/// Holds references to the source arena and type information, plus owns
/// the target canonical arena and auxiliary pools being built.
pub(crate) struct Lowerer<'a> {
    /// Source expression arena (read-only).
    pub(crate) src: &'a ExprArena,
    /// Type check output (read-only).
    pub(crate) typed: &'a TypedModule,
    /// Type pool for resolving variant indices and field types.
    pub(crate) pool: &'a ori_types::Pool,
    /// String interner for creating names during lowering.
    pub(crate) interner: &'a StringInterner,
    /// Target canonical arena (being built).
    pub(crate) arena: CanArena,
    /// Compile-time constant pool.
    pub(crate) constants: ConstantPool,
    /// Compiled decision trees for match expressions.
    pub(crate) decision_trees: DecisionTreePool,

    // Pre-interned method names for desugaring.
    pub(crate) name_to_str: Name,
    pub(crate) name_concat: Name,
    pub(crate) name_merge: Name,
}

impl<'a> Lowerer<'a> {
    /// Create a new lowerer, pre-allocating the target arena based on source size.
    fn new(
        src: &'a ExprArena,
        typed: &'a TypedModule,
        pool: &'a ori_types::Pool,
        interner: &'a StringInterner,
    ) -> Self {
        // Pre-allocate based on source expression count.
        // Desugaring may increase the count slightly, so add 25% headroom.
        let estimated = src.expr_count() + src.expr_count() / 4;
        let mut arena = CanArena::new();
        // Reserve capacity using a rough byte estimate (20 bytes per expression).
        if estimated > 0 {
            arena = CanArena::with_capacity(estimated * 20);
        }

        Self {
            src,
            typed,
            pool,
            interner,
            arena,
            constants: ConstantPool::new(),
            decision_trees: DecisionTreePool::new(),
            name_to_str: interner.intern("to_str"),
            name_concat: interner.intern("concat"),
            name_merge: interner.intern("merge"),
        }
    }

    /// Finish lowering and produce the final result.
    fn finish(self, root: CanId) -> CanonResult {
        CanonResult {
            arena: self.arena,
            constants: self.constants,
            decision_trees: self.decision_trees,
            root,
            roots: Vec::new(),
            method_roots: Vec::new(),
        }
    }

    /// Push a canonical node into the arena.
    pub(crate) fn push(&mut self, kind: CanExpr, span: Span, ty: TypeId) -> CanId {
        self.arena.push(CanNode::new(kind, span, ty))
    }

    /// Get the resolved type for a source expression.
    ///
    /// Converts `ori_types::Idx` (from `TypedModule.expr_types`) to `TypeId`
    /// using their identical `u32` layout. Falls back to `TypeId::ERROR` if
    /// the expression has no type assignment (error recovery).
    fn expr_type(&self, id: ExprId) -> TypeId {
        self.typed
            .expr_type(id.index())
            .map_or(TypeId::ERROR, |idx| TypeId::from_raw(idx.raw()))
    }

    /// Lower an optional expression (handles `ExprId::INVALID` sentinel).
    ///
    /// Returns `CanId::INVALID` for invalid inputs, preserving the sentinel
    /// convention used for optional children (no else branch, no guard, etc.).
    fn lower_optional(&mut self, id: ExprId) -> CanId {
        if id.is_valid() {
            self.lower_expr(id)
        } else {
            CanId::INVALID
        }
    }

    // ── Expression Lowering ─────────────────────────────────────

    /// Lower a single expression from `ExprId` to `CanId`.
    ///
    /// This is the main dispatch function. It copies the [`ExprKind`] out of
    /// the source arena (`ExprKind` is `Copy`), then matches on it to produce
    /// a `CanExpr`. This avoids borrow conflicts — we don't hold a reference
    /// to `self.src` while mutating `self.arena`.
    pub(crate) fn lower_expr(&mut self, id: ExprId) -> CanId {
        let kind = *self.src.expr_kind(id);
        let span = self.src.expr_span(id);
        let ty = self.expr_type(id);

        match kind {
            // === Leaf nodes — direct mapping ===
            ExprKind::Int(v) => self.push(CanExpr::Int(v), span, ty),
            ExprKind::Float(bits) => self.push(CanExpr::Float(bits), span, ty),
            ExprKind::Bool(v) => self.push(CanExpr::Bool(v), span, ty),
            ExprKind::String(name) => self.push(CanExpr::Str(name), span, ty),
            ExprKind::Char(c) => self.push(CanExpr::Char(c), span, ty),
            ExprKind::Duration { value, unit } => {
                self.push(CanExpr::Duration { value, unit }, span, ty)
            }
            ExprKind::Size { value, unit } => self.push(CanExpr::Size { value, unit }, span, ty),
            ExprKind::Unit => self.push(CanExpr::Unit, span, ty),
            ExprKind::None => self.push(CanExpr::None, span, ty),
            ExprKind::Ident(name) => self.push(CanExpr::Ident(name), span, ty),
            ExprKind::Const(name) => self.push(CanExpr::Const(name), span, ty),
            ExprKind::SelfRef => self.push(CanExpr::SelfRef, span, ty),
            ExprKind::FunctionRef(name) => self.push(CanExpr::FunctionRef(name), span, ty),
            ExprKind::HashLength => self.push(CanExpr::HashLength, span, ty),
            ExprKind::Error => self.push(CanExpr::Error, span, ty),

            // === Unary nodes — lower child ===
            ExprKind::Unary { op, operand } => {
                let operand = self.lower_expr(operand);
                let id = self.push(CanExpr::Unary { op, operand }, span, ty);
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::Ok(inner) => {
                let inner = self.lower_optional(inner);
                self.push(CanExpr::Ok(inner), span, ty)
            }
            ExprKind::Err(inner) => {
                let inner = self.lower_optional(inner);
                self.push(CanExpr::Err(inner), span, ty)
            }
            ExprKind::Some(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Some(inner), span, ty)
            }
            ExprKind::Break(val) => {
                let val = self.lower_optional(val);
                self.push(CanExpr::Break(val), span, ty)
            }
            ExprKind::Continue(val) => {
                let val = self.lower_optional(val);
                self.push(CanExpr::Continue(val), span, ty)
            }
            ExprKind::Await(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Await(inner), span, ty)
            }
            ExprKind::Try(inner) => {
                let inner = self.lower_expr(inner);
                self.push(CanExpr::Try(inner), span, ty)
            }
            ExprKind::Loop { body } => {
                let body = self.lower_expr(body);
                self.push(CanExpr::Loop { body }, span, ty)
            }

            // === Binary nodes — lower both children ===
            ExprKind::Binary { op, left, right } => {
                let left = self.lower_expr(left);
                let right = self.lower_expr(right);
                let id = self.push(CanExpr::Binary { op, left, right }, span, ty);
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::Cast {
                expr,
                ty: cast_ty,
                fallible,
            } => {
                let expr = self.lower_expr(expr);
                let target = self.extract_cast_target_name(cast_ty);
                self.push(
                    CanExpr::Cast {
                        expr,
                        target,
                        fallible,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Field { receiver, field } => {
                let receiver = self.lower_expr(receiver);
                self.push(CanExpr::Field { receiver, field }, span, ty)
            }
            ExprKind::Index { receiver, index } => {
                let receiver = self.lower_expr(receiver);
                let index = self.lower_expr(index);
                self.push(CanExpr::Index { receiver, index }, span, ty)
            }
            ExprKind::Assign { target, value } => {
                let target = self.lower_expr(target);
                let value = self.lower_expr(value);
                self.push(CanExpr::Assign { target, value }, span, ty)
            }

            // === Control flow ===
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(cond);
                let then_branch = self.lower_expr(then_branch);
                let else_branch = self.lower_optional(else_branch);
                let id = self.push(
                    CanExpr::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    span,
                    ty,
                );
                if let Some(folded) =
                    crate::const_fold::try_fold(&mut self.arena, &mut self.constants, id)
                {
                    folded
                } else {
                    id
                }
            }
            ExprKind::For {
                binding,
                iter,
                guard,
                body,
                is_yield,
            } => {
                let iter = self.lower_expr(iter);
                let guard = self.lower_optional(guard);
                let body = self.lower_expr(body);
                self.push(
                    CanExpr::For {
                        binding,
                        iter,
                        guard,
                        body,
                        is_yield,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::WithCapability {
                capability,
                provider,
                body,
            } => {
                let provider = self.lower_expr(provider);
                let body = self.lower_expr(body);
                self.push(
                    CanExpr::WithCapability {
                        capability,
                        provider,
                        body,
                    },
                    span,
                    ty,
                )
            }

            // === Special forms — lowered to canonical representations ===
            ExprKind::FunctionSeq(seq_id) => self.lower_function_seq(seq_id, span, ty),
            ExprKind::FunctionExp(exp_id) => self.lower_function_exp(exp_id, span, ty),

            // === Containers ===
            ExprKind::Call { func, args } => self.lower_call(func, args, span, ty),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.lower_method_call(receiver, method, args, span, ty),
            ExprKind::Block { stmts, result } => self.lower_block(stmts, result, span, ty),
            ExprKind::Let {
                pattern,
                ty: _let_ty,
                init,
                mutable,
            } => {
                let init = self.lower_expr(init);
                let pattern = self.lower_binding_pattern(pattern);
                self.push(
                    CanExpr::Let {
                        pattern,
                        init,
                        mutable,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Lambda {
                params,
                ret_ty: _,
                body,
            } => {
                let body = self.lower_expr(body);
                let params = self.lower_params(params);
                self.push(CanExpr::Lambda { params, body }, span, ty)
            }
            ExprKind::List(exprs) => self.lower_list(exprs, span, ty),
            ExprKind::Tuple(exprs) => self.lower_tuple(exprs, span, ty),
            ExprKind::Map(entries) => self.lower_map(entries, span, ty),
            ExprKind::Struct { name, fields } => self.lower_struct(name, fields, span, ty),
            ExprKind::Range {
                start,
                end,
                step,
                inclusive,
            } => {
                let start = self.lower_optional(start);
                let end = self.lower_optional(end);
                let step = self.lower_optional(step);
                self.push(
                    CanExpr::Range {
                        start,
                        end,
                        step,
                        inclusive,
                    },
                    span,
                    ty,
                )
            }
            ExprKind::Match { scrutinee, arms } => self.lower_match(scrutinee, arms, span, ty),

            // === Sugar variants — delegate to desugar.rs ===
            ExprKind::TemplateFull(name) => {
                // Trivial desugaring: template without interpolation is just a string.
                self.push(CanExpr::Str(name), span, ty)
            }
            ExprKind::TemplateLiteral { head, parts } => {
                self.desugar_template_literal(head, parts, span, ty)
            }
            ExprKind::CallNamed { func, args } => self.desugar_call_named(func, args, span, ty),
            ExprKind::MethodCallNamed {
                receiver,
                method,
                args,
            } => self.desugar_method_call_named(receiver, method, args, span, ty),
            ExprKind::ListWithSpread(elements) => self.desugar_list_with_spread(elements, span, ty),
            ExprKind::MapWithSpread(elements) => self.desugar_map_with_spread(elements, span, ty),
            ExprKind::StructWithSpread { name, fields } => {
                self.desugar_struct_with_spread(name, fields, span, ty)
            }
        }
    }

    // ── Container Lowering Helpers ──────────────────────────────

    /// Lower an `ExprRange` (expression list) to a `CanRange`.
    pub(crate) fn lower_expr_range(&mut self, range: ExprRange) -> CanRange {
        let src_ids = self.src.get_expr_list(range);
        if src_ids.is_empty() {
            return CanRange::EMPTY;
        }

        // Copy IDs out to avoid holding a borrow on src while mutating arena.
        let src_ids: Vec<ExprId> = src_ids.to_vec();
        let mut lowered = Vec::with_capacity(src_ids.len());
        for id in src_ids {
            lowered.push(self.lower_expr(id));
        }
        self.arena.push_expr_list(&lowered)
    }

    /// Lower a `StmtRange` (block statements) to a `CanRange`.
    ///
    /// Each statement is lowered to a canonical node:
    /// - `StmtKind::Expr(id)` → lower the expression
    /// - `StmtKind::Let { .. }` → emit a `CanExpr::Let` node
    fn lower_stmt_range(&mut self, range: ori_ir::StmtRange) -> CanRange {
        let stmts = self.src.get_stmt_range(range);
        if stmts.is_empty() {
            return CanRange::EMPTY;
        }

        // Copy stmts out to avoid borrow conflict.
        let stmts: Vec<ori_ir::Stmt> = stmts.to_vec();

        // Lower all statements BEFORE building the expr list. lower_expr may
        // recursively lower nested match/block expressions whose own
        // start/push/finish cycles would corrupt our range.
        let lowered: Vec<CanId> = stmts
            .iter()
            .map(|stmt| match &stmt.kind {
                ori_ir::StmtKind::Expr(expr_id) => self.lower_expr(*expr_id),
                ori_ir::StmtKind::Let {
                    pattern,
                    ty: _,
                    init,
                    mutable,
                } => {
                    let init = self.lower_expr(*init);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init,
                            mutable: *mutable,
                        },
                        stmt.span,
                        TypeId::UNIT,
                    )
                }
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }

    /// Lower a function call with positional args.
    fn lower_call(&mut self, func: ExprId, args: ExprRange, span: Span, ty: TypeId) -> CanId {
        let func = self.lower_expr(func);
        let args = self.lower_expr_range(args);
        self.push(CanExpr::Call { func, args }, span, ty)
    }

    /// Lower a method call with positional args.
    fn lower_method_call(
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
    fn lower_block(
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
    fn lower_list(&mut self, exprs: ExprRange, span: Span, ty: TypeId) -> CanId {
        let range = self.lower_expr_range(exprs);
        self.push(CanExpr::List(range), span, ty)
    }

    /// Lower a tuple literal.
    fn lower_tuple(&mut self, exprs: ExprRange, span: Span, ty: TypeId) -> CanId {
        let range = self.lower_expr_range(exprs);
        self.push(CanExpr::Tuple(range), span, ty)
    }

    /// Lower a map literal (no spread).
    fn lower_map(&mut self, entries: ori_ir::MapEntryRange, span: Span, ty: TypeId) -> CanId {
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
    fn lower_struct(
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

    /// Lower a match expression.
    ///
    /// Compiles arm patterns into a decision tree using the Maranget (2008)
    /// algorithm, then lowers arm bodies into the canonical arena.
    fn lower_match(
        &mut self,
        scrutinee: ExprId,
        arms: ori_ir::ArmRange,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let scrutinee_id = self.lower_expr(scrutinee);

        // Get scrutinee type for pattern flattening.
        let scrutinee_ty = self
            .typed
            .expr_type(scrutinee.index())
            .unwrap_or(ori_types::Idx::UNIT);

        // Extract arm data to avoid borrow conflict (patterns + guards + bodies).
        let src_arms = self.src.get_arms(arms);
        let arm_data: Vec<_> = src_arms
            .iter()
            .map(|arm| (arm.pattern.clone(), arm.guard, arm.body))
            .collect();

        // Lower guards from ExprId → CanId before compiling patterns.
        // Guards must be lowered here (where we have &mut self) rather than
        // inside compile_patterns (which only borrows &self).
        let pattern_data: Vec<_> = arm_data
            .iter()
            .map(|(pat, guard, _body)| {
                let can_guard = guard.map(|g| self.lower_expr(g));
                (pat.clone(), can_guard)
            })
            .collect();
        let tree = crate::patterns::compile_patterns(self, &pattern_data, arms.start, scrutinee_ty);
        let dt_id = self.decision_trees.push(tree);

        // Lower arm bodies BEFORE building the expr list. lower_expr may
        // recursively lower nested match expressions, which would push their
        // own arm bodies into the flat expr_lists array, corrupting our range.
        let lowered_bodies: Vec<CanId> = arm_data
            .iter()
            .map(|(_pattern, _guard, body)| self.lower_expr(*body))
            .collect();
        let start = self.arena.start_expr_list();
        for can_body in lowered_bodies {
            self.arena.push_expr_list_item(can_body);
        }
        let arms_range = self.arena.finish_expr_list(start);

        self.push(
            CanExpr::Match {
                scrutinee: scrutinee_id,
                decision_tree: dt_id,
                arms: arms_range,
            },
            span,
            ty,
        )
    }

    // ── Multi-Clause Function Lowering ────────────────────────

    /// Lower a group of same-name functions into a single body with a
    /// synthesized match expression.
    ///
    /// Transforms:
    /// ```text
    /// @fib (0: int) -> int = 0
    /// @fib (1: int) -> int = 1
    /// @fib (n: int) -> int = fib(n - 1) + fib(n - 2)
    /// ```
    /// Into a single function body equivalent to:
    /// ```text
    /// match param0 {
    ///   0 -> 0
    ///   1 -> 1
    ///   n -> fib(n - 1) + fib(n - 2)
    /// }
    /// ```
    ///
    /// For multi-parameter functions, the scrutinee is a tuple of the
    /// parameters and each arm pattern is a tuple of the param patterns.
    fn lower_multi_clause(&mut self, clauses: &[&ori_ir::Function]) -> CanId {
        debug_assert!(clauses.len() >= 2);

        // Use the first clause's span/type as the overall function span/type.
        let span = clauses[0].span;
        let ty = self.expr_type(clauses[0].body);

        // Determine parameter names from the first clause.
        let first_params = self.src.get_params(clauses[0].params);
        let param_count = first_params.len();
        let param_names: Vec<Name> = first_params.iter().map(|p| p.name).collect();

        // Build the scrutinee: Ident for single-param, Tuple for multi-param.
        // Types use ERROR because these are synthetic nodes — the evaluator
        // dispatches on values, not types. Codegen (LLVM) would need real types,
        // but multi-clause functions aren't supported there yet.
        let scrutinee_id = if param_count == 1 {
            self.push(CanExpr::Ident(param_names[0]), span, TypeId::ERROR)
        } else {
            let idents: Vec<CanId> = param_names
                .iter()
                .map(|&name| self.push(CanExpr::Ident(name), span, TypeId::ERROR))
                .collect();
            let range = self.arena.push_expr_list(&idents);
            self.push(CanExpr::Tuple(range), span, TypeId::ERROR)
        };

        // Flatten each clause's parameter patterns into a FlatPattern row.
        // Guards are lowered from ExprId → CanId before decision tree compilation.
        let mut flat_rows: Vec<Vec<ori_ir::canon::tree::FlatPattern>> =
            Vec::with_capacity(clauses.len());
        let mut guards: Vec<Option<CanId>> = Vec::with_capacity(clauses.len());

        for clause in clauses {
            let params = self.src.get_params(clause.params).to_vec();
            let row: Vec<_> = params
                .iter()
                .map(|p| crate::patterns::flatten_param_pattern(self, p))
                .collect();
            flat_rows.push(row);
            guards.push(clause.guard.map(|g| self.lower_expr(g)));
        }

        // Compile the multi-column pattern matrix into a decision tree.
        let tree = crate::patterns::compile_multi_clause_patterns(&flat_rows, &guards);
        let dt_id = self.decision_trees.push(tree);

        // Lower each clause body.
        let lowered_bodies: Vec<CanId> = clauses
            .iter()
            .map(|clause| self.lower_expr(clause.body))
            .collect();

        // Build the arms CanRange.
        let start = self.arena.start_expr_list();
        for can_body in lowered_bodies {
            self.arena.push_expr_list_item(can_body);
        }
        let arms_range = self.arena.finish_expr_list(start);

        self.push(
            CanExpr::Match {
                scrutinee: scrutinee_id,
                decision_tree: dt_id,
                arms: arms_range,
            },
            span,
            ty,
        )
    }

    // ── Binding Pattern Lowering ──────────────────────────────

    /// Lower a `BindingPatternId` (`ExprArena` reference) to `CanBindingPatternId`.
    ///
    /// Recursively lowers `BindingPattern` → `CanBindingPattern`, storing
    /// sub-patterns in the canonical arena.
    pub(crate) fn lower_binding_pattern(
        &mut self,
        bp_id: ori_ir::BindingPatternId,
    ) -> CanBindingPatternId {
        let bp = self.src.get_binding_pattern(bp_id).clone();
        self.lower_binding_pattern_value(&bp)
    }

    /// Lower a `BindingPattern` value to canonical form.
    fn lower_binding_pattern_value(&mut self, bp: &ori_ir::BindingPattern) -> CanBindingPatternId {
        let can_bp = match bp {
            ori_ir::BindingPattern::Name(name) => CanBindingPattern::Name(*name),
            ori_ir::BindingPattern::Wildcard => CanBindingPattern::Wildcard,
            ori_ir::BindingPattern::Tuple(children) => {
                let child_ids: Vec<_> = children
                    .iter()
                    .map(|c| self.lower_binding_pattern_value(c))
                    .collect();
                let range = self.arena.push_binding_pattern_list(&child_ids);
                CanBindingPattern::Tuple(range)
            }
            ori_ir::BindingPattern::Struct { fields } => {
                let field_bindings: Vec<_> = fields
                    .iter()
                    .map(|(name, sub_pat)| {
                        let sub = match sub_pat {
                            Some(p) => self.lower_binding_pattern_value(p),
                            // Field shorthand: `{ x }` → bind name directly
                            None => self
                                .arena
                                .push_binding_pattern(CanBindingPattern::Name(*name)),
                        };
                        CanFieldBinding {
                            name: *name,
                            pattern: sub,
                        }
                    })
                    .collect();
                let range = self.arena.push_field_bindings(&field_bindings);
                CanBindingPattern::Struct { fields: range }
            }
            ori_ir::BindingPattern::List { elements, rest } => {
                let child_ids: Vec<_> = elements
                    .iter()
                    .map(|c| self.lower_binding_pattern_value(c))
                    .collect();
                let range = self.arena.push_binding_pattern_list(&child_ids);
                CanBindingPattern::List {
                    elements: range,
                    rest: *rest,
                }
            }
        };
        self.arena.push_binding_pattern(can_bp)
    }

    // ── Param Lowering ────────────────────────────────────────

    /// Lower a `ParamRange` (`ExprArena` reference) to `CanParamRange`.
    fn lower_params(&mut self, param_range: ori_ir::ParamRange) -> ori_ir::canon::CanParamRange {
        let src_params = self.src.get_params(param_range);
        if src_params.is_empty() {
            return ori_ir::canon::CanParamRange::EMPTY;
        }

        // Copy out to avoid borrow conflict.
        let param_data: Vec<_> = src_params.iter().map(|p| (p.name, p.default)).collect();

        let can_params: Vec<_> = param_data
            .into_iter()
            .map(|(name, default)| CanParam {
                name,
                default: match default {
                    Some(expr_id) => self.lower_expr(expr_id),
                    None => CanId::INVALID,
                },
            })
            .collect();

        self.arena.push_params(&can_params)
    }

    // ── Cast Target Name Extraction ───────────────────────────

    /// Extract the target type name from a `ParsedTypeId` for `Cast` expressions.
    ///
    /// The evaluator dispatches cast operations by type name (e.g. "int", "float",
    /// "str"), while the LLVM backend uses the resolved `TypeId` from `CanNode.ty`.
    fn extract_cast_target_name(&self, ty_id: ori_ir::ParsedTypeId) -> Name {
        let parsed_ty = self.src.get_parsed_type(ty_id);
        match parsed_ty {
            ori_ir::ParsedType::Primitive(type_id) => {
                // Map well-known TypeIds to their interned names.
                type_id_to_name(*type_id, self.interner)
            }
            ori_ir::ParsedType::Named { name, .. } => *name,
            // For complex types, fall back to an empty name (error recovery).
            _ => Name::EMPTY,
        }
    }

    // ── FunctionExp Lowering ──────────────────────────────────

    /// Lower a `FunctionExp` (`ExprArena` side-table) to inline `CanExpr::FunctionExp`.
    fn lower_function_exp(
        &mut self,
        exp_id: ori_ir::FunctionExpId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let func_exp = self.src.get_function_exp(exp_id).clone();
        let kind = func_exp.kind;

        // Lower each named prop value.
        let src_props = self.src.get_named_exprs(func_exp.props);
        let prop_data: Vec<_> = src_props.iter().map(|p| (p.name, p.value)).collect();
        let can_props: Vec<_> = prop_data
            .into_iter()
            .map(|(name, value)| CanNamedExpr {
                name,
                value: self.lower_expr(value),
            })
            .collect();
        let props = self.arena.push_named_exprs(&can_props);

        self.push(CanExpr::FunctionExp { kind, props }, span, ty)
    }

    // ── FunctionSeq Desugaring ────────────────────────────────

    /// Lower a `FunctionSeq` (`ExprArena` side-table) into primitive `CanExpr` nodes.
    ///
    /// Each `FunctionSeq` variant is desugared:
    /// - `Run { bindings, result }` → `Block { stmts, result }`
    /// - `Try { bindings, result }` → `Block` with Try-wrapped bindings
    /// - `Match { scrutinee, arms }` → `Match` with decision tree
    /// - `ForPattern { over, map, arm, default }` → `For` with match body
    fn lower_function_seq(
        &mut self,
        seq_id: ori_ir::FunctionSeqId,
        span: Span,
        ty: TypeId,
    ) -> CanId {
        let seq = self.src.get_function_seq(seq_id).clone();
        match seq {
            ori_ir::FunctionSeq::Run {
                bindings, result, ..
            } => {
                let stmts = self.lower_seq_bindings(bindings);
                let result = self.lower_expr(result);
                self.push(CanExpr::Block { stmts, result }, span, ty)
            }
            ori_ir::FunctionSeq::Try {
                bindings, result, ..
            } => {
                let stmts = self.lower_seq_bindings_try(bindings);
                let result = self.lower_expr(result);
                self.push(CanExpr::Block { stmts, result }, span, ty)
            }
            ori_ir::FunctionSeq::Match {
                scrutinee, arms, ..
            } => self.lower_match(scrutinee, arms, span, ty),
            ori_ir::FunctionSeq::ForPattern {
                over,
                map,
                arm,
                default,
                ..
            } => {
                // Desugar ForPattern into a For expression.
                // The arm's pattern becomes a match inside the for body.
                let iter = self.lower_expr(over);

                // If there's a map transform, apply it first via a method call.
                let iter = if let Some(map_expr) = map {
                    let map_fn = self.lower_expr(map_expr);
                    let args = self.arena.push_expr_list(&[map_fn]);
                    self.push(
                        CanExpr::MethodCall {
                            receiver: iter,
                            method: self.name_concat, // placeholder — ForPattern maps are rare
                            args,
                        },
                        span,
                        ty,
                    )
                } else {
                    iter
                };

                // Lower the arm body directly.
                let body = self.lower_expr(arm.body);
                let default = self.lower_expr(default);

                // Use a simple for with the binding from the arm pattern.
                let binding = match &arm.pattern {
                    ori_ir::MatchPattern::Binding(name) => *name,
                    // Wildcard and complex patterns: simplified for now
                    _ => Name::EMPTY,
                };

                let guard = arm.guard.map_or(CanId::INVALID, |g| self.lower_expr(g));

                // If there's no default needed, just use the for.
                // Otherwise wrap with if-let or similar pattern.
                let _ = default; // ForPattern default handling deferred to runtime
                self.push(
                    CanExpr::For {
                        binding,
                        iter,
                        guard,
                        body,
                        is_yield: true,
                    },
                    span,
                    ty,
                )
            }
        }
    }

    /// Lower seq bindings (Run variant) to block statements.
    fn lower_seq_bindings(&mut self, range: ori_ir::SeqBindingRange) -> CanRange {
        let bindings = self.src.get_seq_bindings(range);
        if bindings.is_empty() {
            return CanRange::EMPTY;
        }

        let bindings: Vec<_> = bindings.to_vec();

        // Lower all bindings before building the expr list to avoid
        // interleaving with nested start/push/finish cycles.
        let lowered: Vec<CanId> = bindings
            .iter()
            .map(|binding| match binding {
                ori_ir::SeqBinding::Let {
                    pattern,
                    ty: _,
                    value,
                    mutable,
                    span,
                } => {
                    let init = self.lower_expr(*value);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init,
                            mutable: *mutable,
                        },
                        *span,
                        TypeId::UNIT,
                    )
                }
                ori_ir::SeqBinding::Stmt { expr, .. } => self.lower_expr(*expr),
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }

    /// Lower seq bindings (Try variant) — each binding wrapped in Try.
    fn lower_seq_bindings_try(&mut self, range: ori_ir::SeqBindingRange) -> CanRange {
        let bindings = self.src.get_seq_bindings(range);
        if bindings.is_empty() {
            return CanRange::EMPTY;
        }

        let bindings: Vec<_> = bindings.to_vec();

        // Lower all bindings before building the expr list.
        let lowered: Vec<CanId> = bindings
            .iter()
            .map(|binding| match binding {
                ori_ir::SeqBinding::Let {
                    pattern,
                    ty: _,
                    value,
                    mutable,
                    span,
                } => {
                    let value = self.lower_expr(*value);
                    let tried_value = self.push(CanExpr::Try(value), *span, TypeId::ERROR);
                    let pattern = self.lower_binding_pattern(*pattern);
                    self.push(
                        CanExpr::Let {
                            pattern,
                            init: tried_value,
                            mutable: *mutable,
                        },
                        *span,
                        TypeId::UNIT,
                    )
                }
                ori_ir::SeqBinding::Stmt { expr, span } => {
                    let value = self.lower_expr(*expr);
                    self.push(CanExpr::Try(value), *span, TypeId::ERROR)
                }
            })
            .collect();

        let start = self.arena.start_expr_list();
        for can_id in lowered {
            self.arena.push_expr_list_item(can_id);
        }
        self.arena.finish_expr_list(start)
    }
}

/// Map well-known `TypeId` constants to their interned names.
///
/// Used by cast target extraction to get the type name that the
/// evaluator dispatches on.
/// Map a primitive `TypeId` to its interned name.
///
/// Cast expressions need the type name for evaluator dispatch (e.g. `as int`).
/// The LLVM backend ignores this and uses the resolved `TypeId` on `CanNode.ty`.
fn type_id_to_name(type_id: TypeId, interner: &StringInterner) -> Name {
    let s = match type_id {
        TypeId::INT => "int",
        TypeId::FLOAT => "float",
        TypeId::BOOL => "bool",
        TypeId::STR => "str",
        TypeId::CHAR => "char",
        TypeId::BYTE => "byte",
        TypeId::UNIT => "void",
        _ => return Name::EMPTY,
    };
    interner.intern(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::ast::{BinaryOp, Expr};
    use ori_types::Idx;

    /// Create a minimal `TypeCheckResult` for testing.
    fn test_type_result(expr_types: Vec<Idx>) -> TypeCheckResult {
        let mut typed = TypedModule::new();
        for idx in expr_types {
            typed.expr_types.push(idx);
        }
        TypeCheckResult::ok(typed)
    }

    /// Create a shared interner for testing.
    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn lower_int_literal() {
        let mut arena = ExprArena::new();
        let root = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(0, 2)));

        let type_result = test_type_result(vec![Idx::INT]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(result.root.is_valid());
        assert_eq!(*result.arena.kind(result.root), CanExpr::Int(42));
        assert_eq!(result.arena.ty(result.root), TypeId::INT);
    }

    #[test]
    fn lower_bool_literal() {
        let mut arena = ExprArena::new();
        let root = arena.alloc_expr(Expr::new(ExprKind::Bool(true), Span::new(0, 4)));

        let type_result = test_type_result(vec![Idx::BOOL]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert_eq!(*result.arena.kind(result.root), CanExpr::Bool(true));
        assert_eq!(result.arena.ty(result.root), TypeId::BOOL);
    }

    #[test]
    fn lower_string_literal() {
        let mut arena = ExprArena::new();
        let interner = test_interner();
        let name = interner.intern("hello");
        let root = arena.alloc_expr(Expr::new(ExprKind::String(name), Span::new(0, 7)));

        let type_result = test_type_result(vec![Idx::STR]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert_eq!(*result.arena.kind(result.root), CanExpr::Str(name));
        assert_eq!(result.arena.ty(result.root), TypeId::STR);
    }

    #[test]
    fn lower_unit() {
        let mut arena = ExprArena::new();
        let root = arena.alloc_expr(Expr::new(ExprKind::Unit, Span::DUMMY));

        let type_result = test_type_result(vec![Idx::UNIT]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert_eq!(*result.arena.kind(result.root), CanExpr::Unit);
        assert_eq!(result.arena.ty(result.root), TypeId::UNIT);
    }

    #[test]
    fn lower_binary_add() {
        // 1 + 2 with two literals gets constant-folded to Constant(3).
        let mut arena = ExprArena::new();
        let left = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(0, 1)));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
            Span::new(0, 5),
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(result.root.is_valid());

        // Constant folding: 1 + 2 → Constant(3).
        match result.arena.kind(result.root) {
            CanExpr::Constant(cid) => {
                assert_eq!(
                    *result.constants.get(*cid),
                    ori_ir::canon::ConstValue::Int(3)
                );
            }
            other => panic!("expected Constant(3), got {other:?}"),
        }
    }

    #[test]
    fn lower_binary_add_runtime() {
        // x + 1 with a runtime variable stays as Binary (not folded).
        let mut arena = ExprArena::new();
        let interner = test_interner();
        let name_x = interner.intern("x");

        let left = arena.alloc_expr(Expr::new(ExprKind::Ident(name_x), Span::new(0, 1)));
        let right = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(4, 5)));
        let root = arena.alloc_expr(Expr::new(
            ExprKind::Binary {
                op: BinaryOp::Add,
                left,
                right,
            },
            Span::new(0, 5),
        ));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        assert!(result.root.is_valid());

        // Runtime operand: stays as Binary node.
        match result.arena.kind(result.root) {
            CanExpr::Binary { op, .. } => {
                assert_eq!(*op, BinaryOp::Add);
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[test]
    fn lower_ok_with_value() {
        let mut arena = ExprArena::new();
        let inner = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(3, 5)));
        let root = arena.alloc_expr(Expr::new(ExprKind::Ok(inner), Span::new(0, 6)));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Ok(inner_id) => {
                assert!(inner_id.is_valid());
                assert_eq!(*result.arena.kind(*inner_id), CanExpr::Int(42));
            }
            other => panic!("expected Ok, got {other:?}"),
        }
    }

    #[test]
    fn lower_break_no_value() {
        let mut arena = ExprArena::new();
        let root = arena.alloc_expr(Expr::new(ExprKind::Break(ExprId::INVALID), Span::DUMMY));

        let type_result = test_type_result(vec![Idx::NEVER]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Break(val) => assert!(!val.is_valid()),
            other => panic!("expected Break, got {other:?}"),
        }
    }

    #[test]
    fn lower_list_literal() {
        let mut arena = ExprArena::new();
        let e1 = arena.alloc_expr(Expr::new(ExprKind::Int(1), Span::new(1, 2)));
        let e2 = arena.alloc_expr(Expr::new(ExprKind::Int(2), Span::new(4, 5)));
        let e3 = arena.alloc_expr(Expr::new(ExprKind::Int(3), Span::new(7, 8)));
        let elems = arena.alloc_expr_list([e1, e2, e3]);
        let root = arena.alloc_expr(Expr::new(ExprKind::List(elems), Span::new(0, 9)));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT, Idx::INT]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::List(range) => {
                let items = result.arena.get_expr_list(*range);
                assert_eq!(items.len(), 3);
                assert_eq!(*result.arena.kind(items[0]), CanExpr::Int(1));
                assert_eq!(*result.arena.kind(items[1]), CanExpr::Int(2));
                assert_eq!(*result.arena.kind(items[2]), CanExpr::Int(3));
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn lower_template_full() {
        let mut arena = ExprArena::new();
        let interner = test_interner();
        let name = interner.intern("hello world");
        let root = arena.alloc_expr(Expr::new(ExprKind::TemplateFull(name), Span::new(0, 13)));

        let type_result = test_type_result(vec![Idx::STR]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        // TemplateFull desugars to Str.
        assert_eq!(*result.arena.kind(result.root), CanExpr::Str(name));
    }

    #[test]
    fn lower_invalid_root_returns_empty() {
        let arena = ExprArena::new();
        let type_result = test_type_result(vec![]);
        let interner = test_interner();

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, ExprId::INVALID, &interner);
        assert!(!result.root.is_valid());
        assert!(result.arena.is_empty());
    }

    #[test]
    fn lower_call_positional() {
        let mut arena = ExprArena::new();
        let interner = test_interner();
        let func_name = interner.intern("foo");

        let func = arena.alloc_expr(Expr::new(ExprKind::Ident(func_name), Span::new(0, 3)));
        let arg = arena.alloc_expr(Expr::new(ExprKind::Int(42), Span::new(4, 6)));
        let args = arena.alloc_expr_list([arg]);
        let root = arena.alloc_expr(Expr::new(ExprKind::Call { func, args }, Span::new(0, 7)));

        let type_result = test_type_result(vec![Idx::INT, Idx::INT, Idx::INT]);

        let pool = ori_types::Pool::new();
        let result = lower(&arena, &type_result, &pool, root, &interner);
        match result.arena.kind(result.root) {
            CanExpr::Call { func, args } => {
                assert_eq!(*result.arena.kind(*func), CanExpr::Ident(func_name));
                let arg_list = result.arena.get_expr_list(*args);
                assert_eq!(arg_list.len(), 1);
                assert_eq!(*result.arena.kind(arg_list[0]), CanExpr::Int(42));
            }
            other => panic!("expected Call, got {other:?}"),
        }
    }
}
