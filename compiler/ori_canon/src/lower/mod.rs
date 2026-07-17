//! AST → Canonical IR lowering.
//!
//! Transforms every `ExprKind` variant into its `CanExpr` equivalent:
//! - 39 variants mapped directly (child references remapped from `ExprId` to `CanId`)
//! - 7 sugar variants desugared into compositions of primitive `CanExpr` nodes
//! - 1 error variant mapped to `CanExpr::Error`

mod cast_target;
mod collections;
mod expr;
mod format_names;
mod patterns;
mod sequences;

use format_names::FormatDesugarNames;

use ori_ir::ast::items::Module;
use ori_ir::canon::{
    CanArena, CanExpr, CanId, CanNode, CanonResult, CanonRoot, ConstantPool, DecisionTreePool,
    MethodRoot, MonoInstanceId,
};
use ori_ir::{ExprArena, ExprId, Name, Span, TypeId};
use ori_types::{Idx, Tag, TypeCheckResult, TypedModule};
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::debug;

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
    interner: &ori_ir::StringInterner,
) -> CanonResult {
    debug!(source_exprs = src.expr_count(), "canon lower started");

    if !root.is_valid() {
        debug!("canon lower: invalid root, returning empty");
        return CanonResult::empty();
    }

    let mut lowerer = Lowerer::new(src, &type_result.typed, pool, interner);
    let can_root = lowerer.lower_expr(root);
    let result = lowerer.finish(can_root);

    debug!(
        canon_nodes = result.arena.len(),
        constants = result.constants.len(),
        decision_trees = result.decision_trees.len(),
        "canon lower complete"
    );

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
    interner: &ori_ir::StringInterner,
) -> CanonResult {
    debug!(
        functions = module.functions.len(),
        tests = module.tests.len(),
        impls = module.impls.len(),
        source_exprs = src.expr_count(),
        "canon lower_module started"
    );

    let mut lowerer = Lowerer::new(src, &type_result.typed, pool, interner);
    let mut roots = lower_function_roots(&mut lowerer, module);
    lower_test_roots(&mut lowerer, module, &mut roots);

    let trait_defaults = trait_default_methods(module);
    let mut method_roots = lower_impl_method_roots(&mut lowerer, module, interner, &trait_defaults);
    lower_named_method_roots(
        &mut lowerer,
        module
            .extends
            .iter()
            .map(|extension| (extension.target_type_name, extension.methods.as_slice())),
        &mut method_roots,
    );
    lower_named_method_roots(
        &mut lowerer,
        module
            .def_impls
            .iter()
            .map(|impl_def| (impl_def.trait_name, impl_def.methods.as_slice())),
        &mut method_roots,
    );

    let root = roots.first().map_or(CanId::INVALID, |entry| entry.body);
    let mut result = lowerer.finish(root);
    result.roots = roots;
    result.method_roots = method_roots;

    debug!(
        canon_nodes = result.arena.len(),
        roots = result.roots.len(),
        method_roots = result.method_roots.len(),
        constants = result.constants.len(),
        decision_trees = result.decision_trees.len(),
        "canon lower_module complete"
    );
    #[cfg(debug_assertions)]
    crate::validate(&result);
    result
}

fn lower_function_roots(lowerer: &mut Lowerer<'_>, module: &Module) -> Vec<CanonRoot> {
    let mut groups: FxHashMap<Name, Vec<&ori_ir::Function>> = FxHashMap::default();
    for function in &module.functions {
        groups.entry(function.name).or_default().push(function);
    }

    let mut roots = Vec::with_capacity(module.functions.len() + module.tests.len());
    let mut seen = FxHashSet::default();
    for function in &module.functions {
        if !seen.insert(function.name) {
            continue;
        }
        if let Some(root) = lower_function_group(lowerer, function, &groups[&function.name]) {
            roots.push(root);
        }
    }
    roots
}

fn lower_function_group(
    lowerer: &mut Lowerer<'_>,
    function: &ori_ir::Function,
    group: &[&ori_ir::Function],
) -> Option<CanonRoot> {
    if group.len() == 1 {
        if !function.body.is_valid() {
            return None;
        }
        return Some(CanonRoot {
            name: function.name,
            body: lowerer.lower_expr(function.body),
            defaults: lowerer.lower_param_defaults(function.params),
            param_names: Vec::new(),
        });
    }

    let body = lowerer.lower_multi_clause(group);
    let defaults = lowerer.lower_param_defaults(group[0].params);
    let param_names = lowerer
        .typed
        .function(function.name)
        .map(|signature| signature.param_names.clone())
        .unwrap_or_default();
    Some(CanonRoot {
        name: function.name,
        body,
        defaults,
        param_names,
    })
}

fn lower_test_roots(lowerer: &mut Lowerer<'_>, module: &Module, roots: &mut Vec<CanonRoot>) {
    for test in &module.tests {
        if !test.body.is_valid() {
            continue;
        }
        roots.push(CanonRoot {
            name: test.name,
            body: lowerer.lower_expr(test.body),
            defaults: Vec::new(),
            param_names: Vec::new(),
        });
    }
}

fn trait_default_methods(module: &Module) -> FxHashMap<Name, Vec<&ori_ir::TraitDefaultMethod>> {
    let mut defaults = FxHashMap::default();
    for trait_def in &module.traits {
        for item in &trait_def.items {
            if let ori_ir::TraitItem::DefaultMethod(method) = item {
                defaults
                    .entry(trait_def.name)
                    .or_insert_with(Vec::new)
                    .push(method);
            }
        }
    }
    defaults
}

fn lower_impl_method_roots(
    lowerer: &mut Lowerer<'_>,
    module: &Module,
    interner: &ori_ir::StringInterner,
    trait_defaults: &FxHashMap<Name, Vec<&ori_ir::TraitDefaultMethod>>,
) -> Vec<MethodRoot> {
    let mut roots = Vec::new();
    for impl_def in &module.impls {
        let Some(type_name) = impl_def.semantic_type_name(interner) else {
            continue;
        };
        let mut overridden = FxHashSet::default();
        for method in &impl_def.methods {
            overridden.insert(method.name);
            if let Some(root) = lower_method_root(lowerer, type_name, method) {
                roots.push(root);
            }
        }

        let Some(trait_name) = impl_def
            .trait_path
            .as_ref()
            .and_then(|path| path.last())
            .copied()
        else {
            continue;
        };
        let Some(defaults) = trait_defaults.get(&trait_name) else {
            continue;
        };
        for method in defaults {
            if !overridden.contains(&method.name) && method.body.is_valid() {
                roots.push(MethodRoot {
                    type_name,
                    method_name: method.name,
                    source_body: method.body,
                    body: lowerer.lower_expr(method.body),
                });
            }
        }
    }
    roots
}

fn lower_method_root(
    lowerer: &mut Lowerer<'_>,
    type_name: Name,
    method: &ori_ir::ImplMethod,
) -> Option<MethodRoot> {
    method.body.is_valid().then(|| MethodRoot {
        type_name,
        method_name: method.name,
        source_body: method.body,
        body: lowerer.lower_expr(method.body),
    })
}

fn lower_named_method_roots<'a>(
    lowerer: &mut Lowerer<'_>,
    groups: impl IntoIterator<Item = (Name, &'a [ori_ir::ImplMethod])>,
    roots: &mut Vec<MethodRoot>,
) {
    for (type_name, methods) in groups {
        for method in methods {
            if let Some(root) = lower_method_root(lowerer, type_name, method) {
                roots.push(root);
            }
        }
    }
}

// Lowerer

/// State for the AST-to-CanonIR lowering pass.
///
/// Holds references to the source arena and type information, plus owns
/// the target canonical arena and auxiliary pools being built.
pub(crate) struct Lowerer<'a> {
    /// Source expression arena (read-only).
    /// Accessed by: lower, desugar, patterns
    pub(crate) src: &'a ExprArena,
    /// Type check output (read-only).
    /// Accessed by: lower, desugar, patterns
    pub(crate) typed: &'a TypedModule,
    /// Type pool for resolving variant indices and field types.
    /// Accessed by: lower, patterns
    pub(crate) pool: &'a ori_types::Pool,
    /// String interner for creating names during lowering.
    /// Accessed by: lower, patterns
    pub(crate) interner: &'a ori_ir::StringInterner,
    /// Target canonical arena (being built).
    /// Accessed by: lower, desugar
    pub(crate) arena: CanArena,
    /// Compile-time constant pool.
    pub(super) constants: ConstantPool,
    /// Compiled decision trees for match expressions.
    pub(super) decision_trees: DecisionTreePool,
    /// Pattern problems accumulated during exhaustiveness checking.
    pub(crate) problems: Vec<ori_ir::canon::PatternProblem>,
    /// Pre-sort `(CanId, MonoInstanceId)` pairs accumulated during lowering.
    /// Each entry corresponds to a generic call site whose AST `ExprId` was
    /// resolved to a specific `MonoInstanceId` by the type checker (carried
    /// in `TypedModule.mono_dispatch_map`). The lowerer translates each
    /// resolved `ExprId` to its newly allocated `CanId` and appends the pair;
    /// `finish` sorts by `CanId.raw()` for binary-search lookup.
    pub(crate) mono_dispatch_map_can: Vec<(CanId, MonoInstanceId)>,

    /// Active index-temp overrides for an in-flight index/field-assignment
    /// desugar. Maps a source index `ExprId` to the `CanId` of the synthetic
    /// `let $__assign_idx_N` temporary that hoisted it. While non-empty,
    /// `lower_expr` returns the mapped temp instead of re-lowering the index —
    /// guaranteeing a side-effecting index (`arr[f()] += 1`) evaluates exactly
    /// once across the read-copy and write-copy the parser's compound-assign
    /// desugar shares. Cleared after the assignment's value + chain are built.
    pub(crate) index_temp_overrides: rustc_hash::FxHashMap<ExprId, CanId>,

    // Pre-interned method names for desugaring.
    // Accessed by: lower, desugar
    pub(crate) name_to_str: Name,
    pub(crate) name_concat: Name,
    pub(crate) name_merge: Name,

    // Pre-interned builtin type names for TypeRef detection.
    pub(super) name_duration: Name,
    pub(super) name_size: Name,

    // Pre-interned names for collection specialization.
    pub(crate) name_collect: Name,
    pub(crate) name_collect_set: Name,

    // Pre-interned method name for the index/field-assignment desugar
    // (`x[i] = v` → `x = x.updated(key: i, value: v)`).
    pub(crate) name_updated: Name,

    // Pre-interned names for non-primitive `{expr:spec}` desugaring.
    // `Formattable.format` MethodCall + synthesized `FormatSpec` struct.
    pub(crate) fmt: FormatDesugarNames,
}

impl<'a> Lowerer<'a> {
    /// Create a new lowerer, pre-allocating the target arena based on source size.
    pub(super) fn new(
        src: &'a ExprArena,
        typed: &'a TypedModule,
        pool: &'a ori_types::Pool,
        interner: &'a ori_ir::StringInterner,
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
            problems: Vec::new(),
            mono_dispatch_map_can: Vec::new(),
            index_temp_overrides: rustc_hash::FxHashMap::default(),
            name_to_str: interner.intern("to_str"),
            name_concat: interner.intern("concat"),
            name_merge: interner.intern("merge"),
            name_duration: interner.intern("Duration"),
            name_size: interner.intern("Size"),
            name_collect: interner.intern("collect"),
            name_collect_set: interner
                .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet.name()),
            name_updated: interner.intern("updated"),
            fmt: FormatDesugarNames::new(interner),
        }
    }

    /// Finish lowering and produce the final result.
    pub(super) fn finish(self, root: CanId) -> CanonResult {
        let mut mono_dispatch_map_can = self.mono_dispatch_map_can;
        mono_dispatch_map_can.sort_by_key(|(cid, _)| cid.raw());
        let mono_const_bindings = self
            .typed
            .mono_instances
            .iter()
            .map(|instance| instance.const_bindings.clone())
            .collect();
        CanonResult {
            arena: self.arena,
            constants: self.constants,
            decision_trees: self.decision_trees,
            root,
            roots: Vec::new(),
            method_roots: Vec::new(),
            problems: self.problems,
            mono_dispatch_map_can,
            mono_const_bindings,
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
    pub(super) fn expr_type(&self, id: ExprId) -> TypeId {
        self.typed
            .expr_type(id.index())
            .map_or(TypeId::ERROR, |idx| TypeId::from_raw(idx.raw()))
    }

    /// Check if a name refers to a type with associated functions.
    ///
    /// Returns `true` for:
    /// - User-defined types with type definitions (structs, enums, newtypes)
    /// - Builtin types with associated functions (Duration, Size)
    ///
    /// This enables the canonicalizer to emit `CanExpr::TypeRef` instead of
    /// `CanExpr::Ident`, so the evaluator can skip the `UserMethodRegistry`
    /// read lock on the hot path.
    ///
    /// The evaluator still checks the environment first for variable shadowing,
    /// so this classification is safe even if a variable shadows a type name.
    pub(super) fn is_type_reference(&self, name: Name, ty: Idx) -> bool {
        // A module variant can share a name with a universe type (notably
        // `Error`). Type checking has already selected the variant, so its
        // function/result type is authoritative over a name-only type lookup.
        let resolved = self.pool.resolve_fully(ty);
        let result_type = if self.pool.tag(resolved) == Tag::Function {
            self.pool.resolve_fully(self.pool.function_return(resolved))
        } else {
            resolved
        };
        if self.pool.tag(result_type) == Tag::Enum
            && self
                .pool
                .enum_variants(result_type)
                .iter()
                .any(|(variant, _)| *variant == name)
        {
            return false;
        }

        // Builtin types with associated functions (pre-interned Name comparison).
        if name == self.name_duration || name == self.name_size {
            return true;
        }
        // User-defined types known to the type checker.
        self.typed.type_def(name).is_some()
    }

    /// Lower an optional expression (handles `ExprId::INVALID` sentinel).
    ///
    /// Returns `CanId::INVALID` for invalid inputs, preserving the sentinel
    /// convention used for optional children (no else branch, no guard, etc.).
    pub(super) fn lower_optional(&mut self, id: ExprId) -> CanId {
        if id.is_valid() {
            self.lower_expr(id)
        } else {
            CanId::INVALID
        }
    }
}

#[cfg(test)]
mod tests;
