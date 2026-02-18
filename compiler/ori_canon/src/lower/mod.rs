//! AST → Canonical IR lowering.
//!
//! Transforms every `ExprKind` variant into its `CanExpr` equivalent:
//! - 39 variants mapped directly (child references remapped from `ExprId` to `CanId`)
//! - 7 sugar variants desugared into compositions of primitive `CanExpr` nodes
//! - 1 error variant mapped to `CanExpr::Error`
//!
//! See `eval_v2` Section 02 for the full lowering specification.

mod collections;
mod expr;
mod misc;
mod patterns;
mod sequences;

use ori_ir::ast::items::Module;
use ori_ir::canon::{
    CanArena, CanExpr, CanId, CanNode, CanonResult, ConstantPool, DecisionTreePool, MethodRoot,
};
use ori_ir::{ExprArena, ExprId, Name, Span, TypeId};
use ori_types::{TypeCheckResult, TypedModule};

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
#[expect(
    clippy::too_many_lines,
    reason = "module-level canonicalization pipeline"
)]
pub fn lower_module(
    module: &Module,
    src: &ExprArena,
    type_result: &TypeCheckResult,
    pool: &ori_types::Pool,
    interner: &ori_ir::StringInterner,
) -> CanonResult {
    let mut lowerer = Lowerer::new(src, &type_result.typed, pool, interner);
    let mut roots = Vec::with_capacity(module.functions.len() + module.tests.len());

    // Group functions by name to detect multi-clause definitions.
    let mut func_groups: rustc_hash::FxHashMap<Name, Vec<&ori_ir::Function>> =
        rustc_hash::FxHashMap::default();
    for func in &module.functions {
        func_groups.entry(func.name).or_default().push(func);
    }

    // Lower each function/group body, recording name → CanonRoot.
    // Iteration order must match `module.functions` for compatibility with
    // `register_module_functions` which uses the same ordering.
    let mut seen_names: rustc_hash::FxHashSet<Name> = rustc_hash::FxHashSet::default();
    for func in &module.functions {
        if !seen_names.insert(func.name) {
            continue; // Already handled this group.
        }
        let group = &func_groups[&func.name];
        if group.len() == 1 {
            // Single clause — lower body and parameter defaults.
            if func.body.is_valid() {
                let can_id = lowerer.lower_expr(func.body);
                let defaults = lowerer.lower_param_defaults(func.params);
                roots.push(ori_ir::canon::CanonRoot {
                    name: func.name,
                    body: can_id,
                    defaults,
                });
            }
        } else {
            // Multi-clause — synthesize a match body. Use first clause's defaults.
            let can_id = lowerer.lower_multi_clause(group);
            let defaults = lowerer.lower_param_defaults(group[0].params);
            roots.push(ori_ir::canon::CanonRoot {
                name: func.name,
                body: can_id,
                defaults,
            });
        }
    }

    // Lower each test body into the same arena (tests have no defaults).
    for test in &module.tests {
        if test.body.is_valid() {
            let can_id = lowerer.lower_expr(test.body);
            roots.push(ori_ir::canon::CanonRoot {
                name: test.name,
                body: can_id,
                defaults: Vec::new(),
            });
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
                method_roots.push(MethodRoot {
                    type_name,
                    method_name: method.name,
                    body: can_id,
                });
            }
        }

        // Lower default trait methods that weren't overridden.
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                if let Some(defaults) = trait_defaults.get(&trait_name) {
                    for dm in defaults {
                        if !overridden.contains(&dm.name) && dm.body.is_valid() {
                            let can_id = lowerer.lower_expr(dm.body);
                            method_roots.push(MethodRoot {
                                type_name,
                                method_name: dm.name,
                                body: can_id,
                            });
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
                method_roots.push(MethodRoot {
                    type_name: extend_def.target_type_name,
                    method_name: method.name,
                    body: can_id,
                });
            }
        }
    }

    for def_impl_def in &module.def_impls {
        for method in &def_impl_def.methods {
            if method.body.is_valid() {
                let can_id = lowerer.lower_expr(method.body);
                method_roots.push(MethodRoot {
                    type_name: def_impl_def.trait_name,
                    method_name: method.name,
                    body: can_id,
                });
            }
        }
    }

    // Use the first function's root as the primary root (for single-expression compat).
    let root = roots.first().map_or(CanId::INVALID, |r| r.body);

    let mut result = lowerer.finish(root);
    result.roots = roots;
    result.method_roots = method_roots;

    #[cfg(debug_assertions)]
    crate::validate(&result);

    result
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

    // Pre-interned names for check desugaring.
    pub(super) name_msg: Name,
    pub(super) name_check_result: Name,
    pub(super) name_pre_check_failed: Name,
    pub(super) name_post_check_failed: Name,
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
            name_to_str: interner.intern("to_str"),
            name_concat: interner.intern("concat"),
            name_merge: interner.intern("merge"),
            name_duration: interner.intern("Duration"),
            name_size: interner.intern("Size"),
            name_collect: interner.intern("collect"),
            name_collect_set: interner
                .intern(ori_ir::builtin_constants::iterator::COLLECT_SET_METHOD),
            name_msg: interner.intern("msg"),
            name_check_result: interner.intern("__check_result"),
            name_pre_check_failed: interner.intern("pre_check failed"),
            name_post_check_failed: interner.intern("post_check failed"),
        }
    }

    /// Finish lowering and produce the final result.
    pub(super) fn finish(self, root: CanId) -> CanonResult {
        CanonResult {
            arena: self.arena,
            constants: self.constants,
            decision_trees: self.decision_trees,
            root,
            roots: Vec::new(),
            method_roots: Vec::new(),
            problems: self.problems,
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
    pub(super) fn is_type_reference(&self, name: Name) -> bool {
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
