//! State owned by the Hindley-Milner inference engine.

use ori_ir::{ExprId, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    check::WellKnownNames, ContextKind, FunctionSig, Idx, MonoInstanceId, PatternKey,
    PatternResolution, TraitRegistry, TypeCheckError, TypeCheckWarning, TypeRegistry, UnifyEngine,
};

use super::env::TypeEnv;
use super::scope;

/// Expression key stored as a `usize` to keep core inference indexing compact.
pub type ExprIndex = usize;

/// The type inference engine.
///
/// Orchestrates Hindley-Milner type inference:
/// - `Pool` for type storage and interning
/// - `UnifyEngine` for unification with path compression
/// - `TypeEnv` for name bindings
/// - Error accumulation for full diagnostics
///
/// # Component Structure
///
/// ```text
/// InferEngine
/// |-- UnifyEngine (unification, resolution, generalization)
/// |   `-- Pool (type storage, interning, flags)
/// |-- TypeEnv (name -> type scheme bindings)
/// |-- expr_types (expression -> inferred type)
/// |-- context_stack (error context tracking)
/// `-- errors (accumulated type errors)
/// ```
pub struct InferEngine<'pool> {
    /// The unification engine (owns mutable pool access).
    pub(super) unify: UnifyEngine<'pool>,

    /// Type environment for name bindings.
    pub(super) env: TypeEnv,

    /// Inferred types for expressions (expr index → type).
    pub(super) expr_types: FxHashMap<ExprIndex, Idx>,

    /// Context stack for error reporting.
    pub(super) context_stack: Vec<ContextKind>,

    /// Accumulated type check errors.
    pub(super) errors: Vec<TypeCheckError>,

    /// Accumulated type check warnings.
    pub(super) warnings: Vec<TypeCheckWarning>,

    /// String interner for resolving names in error messages.
    pub(super) interner: Option<&'pool StringInterner>,

    /// Pre-interned well-known type names for O(1) annotation resolution.
    pub(super) well_known: Option<&'pool WellKnownNames>,

    /// Trait registry for where-clause validation at call sites.
    pub(super) trait_registry: Option<&'pool TraitRegistry>,

    /// Function signatures for where-clause lookup.
    pub(super) signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,

    /// Module-alias namespaces (`use "path" as alias`): each alias name maps to
    /// the aliased module's public function signatures. Consulted to resolve a
    /// qualified call `alias.func(args)` (Spec: Clause 12 Module Alias) against
    /// the named function's signature.
    pub(super) module_aliases: Option<&'pool FxHashMap<Name, Vec<FunctionSig>>>,

    /// Type registry for struct/enum/newtype lookup during inference.
    pub(super) type_registry: Option<&'pool TypeRegistry>,

    /// Builtin-type extension methods (`extend str { @m }`, `extend [T] { @m }`)
    /// keyed by builtin `TypeTag`. Populated from `module.extends` at body-check
    /// entry. Consulted by `emit_unknown_method` so an `extend`-provided method on
    /// a builtin receiver is not false-rejected as unknown (TR-9 dispatch stays
    /// target-only; the evaluator resolves the actual call).
    pub(super) builtin_extensions: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>>,

    /// Current function type for `self` references (recursive calls in patterns).
    pub(super) self_type: Option<Idx>,

    /// Current impl's `Self` type (for `Self` in type annotations within impl blocks).
    pub(super) impl_self_type: Option<Idx>,

    /// Stack of enclosing-loop contexts for nested loops.
    /// Each loop form pushes a [`LoopContext`] recording the expected break
    /// value type plus whether `break value` / `continue value` are permitted
    /// (per spec: `loop` and `for...yield` carry values; `while` / `for...do`
    /// do not). `break` / `continue` consult the innermost entry.
    pub(super) loop_contexts: Vec<scope::LoopContext>,

    /// Stack of active `try {}` propagation boundaries. `Some` frames collect
    /// explicit `?` carriers; `None` frames prevent a nested lambda body from
    /// leaking propagation into an enclosing try block.
    pub(super) try_boundaries: Vec<Option<Vec<scope::TryPropagation>>>,

    /// Capabilities declared by the current function (`uses` clause).
    pub(super) current_capabilities: FxHashSet<Name>,

    /// Capabilities provided in scope (`with...in`).
    pub(super) provided_capabilities: FxHashSet<Name>,

    /// Lexically ordered value-provider bindings for capability calls.
    ///
    /// Each namespace owns a stack so nested `with` bindings shadow and then
    /// restore the outer provider exactly. Function-level entries represent
    /// hidden provider parameters and sit at the bottom of the stack.
    pub(super) capability_providers: FxHashMap<Name, Vec<crate::CapabilityProvider>>,

    /// Frozen provider selections for direct free calls in this body.
    pub(super) capability_call_sites: Vec<(ExprId, crate::CapabilityCallSite)>,

    /// Pattern resolutions accumulated during match checking.
    ///
    /// Records `Binding` patterns that were resolved to unit variants.
    /// Extracted via `take_pattern_resolutions()` after checking.
    pub(super) pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    /// Module-level constant types for `$name` reference resolution.
    pub(super) const_types: Option<&'pool FxHashMap<Name, Idx>>,

    /// Concrete module-constant values in the supported const-expression
    /// subset. A declared module const absent here is not symbolically valid;
    /// capacity validation rejects it as unevaluable.
    pub(super) const_values: Option<&'pool FxHashMap<Name, crate::ConstValue>>,

    /// Body-local const-generic parameter types for `$name` reference
    /// resolution inside functions and impl/def-impl methods. Populated at
    /// body-check entry so bodies can use `$N` in type and value positions.
    /// Owned because the const-parameter scope belongs to one body, not the
    /// module. Consulted by `const_type` after module-level lookup misses.
    pub(super) const_param_types: FxHashMap<Name, Idx>,

    /// Method-level `RigidVar` trait-bound assumptions for body-internal trait
    /// dispatch. Populated at impl/def-impl body-check
    /// entry from inline `<T: Bound>` and trailing `where T: Bound` forms so
    /// that body-internal trait queries on a method-level `RigidVar` (e.g.,
    /// `prefix.to_str()` in `@to_string<T: Printable> (prefix: T) -> str`)
    /// succeed when the trait is in the binder's declared bounds.
    ///
    /// Keyed by the `RigidVar`'s `var_id` (extractable via `pool.data(idx)`);
    /// value is the `Vec` of trait `Name`s assumed for that binder. Owned (not
    /// borrowed) because the scope is per-method, not module-level.
    /// Consulted by `rigid_var_satisfies_bound`.
    pub(super) method_rigid_bounds: FxHashMap<u32, Vec<Name>>,

    /// Monomorphization instances discovered during inference.
    ///
    /// Populated when a generic function or method is selected with concrete
    /// type arguments, including operator-selected methods that have no call
    /// expression dispatch entry. Extracted via `take_mono_instances()`.
    pub(super) mono_instances: Vec<crate::MonoInstance>,

    /// Pre-dedup `(call_expr_id, MonoInstanceId)` pairs accumulated during
    /// inference. Each [`MonoInstanceId`] is a position into `mono_instances`
    /// at insertion time (this body's local index space); the body-pass
    /// absorbs both vectors together via
    /// [`crate::check::ModuleChecker::accumulate_mono_session`] which offsets
    /// the local indices into module-wide positions, and
    /// [`crate::check::ModuleChecker::finish_with_pool`] then remaps them
    /// once more across dedup + sort before storing in
    /// [`crate::TypedModule::mono_dispatch_map`].
    ///
    /// Populated by `record_mono_with_dispatch()` for eager calls and by
    /// `check::exports::resolve_deferred_mono_calls` after a deferred call's
    /// type variables resolve to a concrete instance.
    pub(super) mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,

    /// Exact route selected for each `receiver[index]` site.
    ///
    /// Semantic producer identities are normalized into dense handles at
    /// module finalization; builtin, deferred, and error routes remain explicit.
    pub(super) index_dispatch_selections: Vec<(ExprId, crate::IndexDispatchSelection)>,

    /// Type-directed desugar plans for `ExprKind::AssignTarget` chains.
    ///
    /// Populated by `record_assign_desugar()` during `infer_assign_target`
    /// (one entry per index/field assignment). Each entry pairs the
    /// `AssignTarget` node's AST `ExprId` with its resolved per-level types.
    /// Extracted via `take_assign_desugars()` and forwarded to
    /// [`crate::TypedModule::assign_desugar_map`]. `ori_canon` consumes the
    /// map to synthesize the pure-reassignment form so AIMS never sees a
    /// `CanExpr::Assign { target: Index/Field }`. Keys are module-wide AST
    /// `ExprId`s; no body-local re-anchoring needed.
    pub(super) assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,

    /// Module-alias qualified calls resolved during this body pass: the call's
    /// AST `ExprId` paired with the qualified imported-function `Name`
    /// (`"alias.func"`). Drained per body via `take_module_alias_calls`,
    /// accumulated into `TypedModule::module_alias_call_map`; consumed by
    /// `ori_canon` to rewrite the namespace `MethodCall` to a free `Call`.
    pub(super) module_alias_calls: Vec<(ExprId, Name)>,

    /// Iterable->Iterator routed method calls resolved during this body pass.
    /// Keyed by the exact source call `ExprId`, paired with the
    /// type-directed iterator materialization/collection route.
    /// Populated by `record_iter_route()` in `resolve_receiver_and_builtin`'s
    /// Iterable fallthrough; drained per body via `take_iter_routes`, accumulated
    /// into `TypedModule::iter_route_map`; consumed by `ori_canon` to desugar
    /// `recv.method(args)` -> `recv.iter().method(args)` so the materialized
    /// iterator is a real IR node AIMS realizes (vs a backend-hidden one).
    pub(super) iter_route_desugars: Vec<(ExprId, crate::IterMethodRoute)>,

    /// Deferred monomorphization calls (generic calling generic).
    ///
    /// Populated by `record_deferred_mono_call()` when a generic function calls
    /// another generic with type variables still unresolved. Extracted via
    /// `take_deferred_mono_calls()` and resolved in `finish_with_pool()`.
    pub(super) deferred_mono_calls: Vec<crate::DeferredMonoCall>,

    /// Set to `true` at the end of body inference, immediately before the
    /// end-of-body defaulting pre-pass runs. `default_unbound_vars_in_scope`
    /// and `default_unbound_vars_from_empty_literals` debug-assert this flag
    /// so a pass-order inversion (defaulting called before body inference
    /// finished) panics in debug builds instead of silently defaulting vars
    /// that bidirectional propagation should have pinned.
    pub(super) body_inference_complete: bool,

    /// Composed `UserBurdenSpec` entries discovered during monomorphization.
    ///
    /// Populated by
    /// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
    /// when a generic builtin (Option<T>, Result<T, E>, [T], {K: V}, Set<T>)
    /// reaches its first-instantiation form with fully-resolved type args.
    /// Each entry is `(monomorphized_idx, composed_spec)`; the body-pass
    /// extractor drains them through `take_composed_burdens`, then body
    /// finalization registers them in the `TypeRegistry`.
    ///
    /// Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding
    /// the lattice-driven analysis. Composition at type-instantiation time
    /// (this accumulator) prevents Phase 5 from emitting indirect dispatch on
    /// each burden walk (proposal §Generic Burden Composition rationale).
    pub(super) composed_burdens: Vec<(Idx, crate::registry::burden::UserBurdenSpec)>,

    /// Exact locally checked body that may own deferred generic calls, plus its
    /// type-only binder roots in declaration order.
    ///
    /// Top-level roots remain signature-derived at the writer. Impl-method
    /// roots are captured here because method signatures intentionally do not
    /// expose top-level `scheme_var_ids`.
    pub(super) deferred_mono_caller: Option<(crate::DeferredMonoCaller, Vec<Idx>)>,

    /// Snapshot of name bindings that were in place at engine-creation time
    /// (typically `base_env.names()` — the module-level set of imported
    /// prelude functions, imported module bindings, and same-module function
    /// signatures from the Signatures-group pass).
    ///
    /// Used by [`InferEngine::collect_lexical_outer`] to distinguish names
    /// bound lexically within the current function body (params, local
    /// `let`s, `for`/`match` bindings) from names that are "visible but
    /// globally scoped". Required for `should_generalize`'s Value Restriction
    /// capture check to treat `let f = x -> len(xs: x)` (prelude-qualified
    /// body) as non-capturing, while still treating
    /// `let outer = 1; let f = x -> outer` (lexically-captured body) as
    /// capturing.
    ///
    /// `None` means no snapshot was provided — in that case every env name
    /// is conservatively treated as lexical (the pre-snapshot behavior).
    pub(super) module_scope_snapshot: Option<FxHashSet<Name>>,

    /// Var ids that were generalized by `generalize()` during body
    /// inference. Tracked so the end-of-body normalization pass
    /// ([`InferEngine::normalize_body_generalized_to_bound_var`]) can rewrite
    /// matching `Tag::Var` leaves in `expr_types` / `FunctionSig` positions
    /// to `Tag::BoundVar` per.
    ///
    /// Drained per-body: between bodies, the set must be empty. Accumulates
    /// via [`InferEngine::record_generalized_vars`] from the `generalize()`
    /// call path (inner let-polymorphism); top-level polymorphic function
    /// signatures' scheme var ids are passed directly to the normalization
    /// method by the body-pass caller.
    pub(super) pending_generalized_vars: Vec<u32>,
}

// The pool, interner, and registry references are shared compiler owners with
// independent debug surfaces. Keep them opaque and expose this engine's
// phase-local progress and diagnostic state.
impl std::fmt::Debug for InferEngine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InferEngine")
            .field("expr_type_count", &self.expr_types.len())
            .field("context_depth", &self.context_stack.len())
            .field("error_count", &self.errors.len())
            .field("warning_count", &self.warnings.len())
            .field("mono_instance_count", &self.mono_instances.len())
            .field("body_inference_complete", &self.body_inference_complete)
            .field("deferred_mono_caller", &self.deferred_mono_caller)
            .finish_non_exhaustive()
    }
}
