//! Type inference engine.
//!
//! This module provides the main orchestrator for Hindley-Milner type inference,
//! connecting the Pool, `UnifyEngine`, and error system into a unified inference API.
//!
//! # Architecture
//!
//! `InferEngine` wraps `UnifyEngine` and adds:
//! - Expression type storage (`expr_types`)
//! - Type environment management (`TypeEnv`)
//! - Context-aware error reporting
//! - Bidirectional type checking (`infer` vs `check`)
//!
//! # Usage
//!
//! ```rust
//! use ori_types::{Idx, InferEngine, Pool};
//!
//! let mut pool = Pool::new();
//! let mut engine = InferEngine::new(&mut pool);
//!
//! let inferred = engine.fresh_var();
//! engine
//!     .unify()
//!     .unify(inferred, Idx::INT)
//!     .expect("an unconstrained inferred type should accept int");
//! assert_eq!(engine.unify().resolve(inferred), Idx::INT);
//! ```
//!
//! # Design Notes
//!
//! The engine uses:
//! - `Idx` as the canonical type handle (not `Type` or `TypeId`)
//! - `UnifyEngine` for O(α(n)) unification
//! - `Pool` for O(1) type equality
//! - Rich error context for helpful diagnostic messages

mod body_finalize;
mod context;
mod engine_api;
mod env;
mod expr;
pub(crate) use expr::match_self_type;
pub(crate) use expr::register_concrete_applied_resolutions;
pub(crate) use expr::type_satisfies_named_trait;
pub(crate) use expr::{NestedPathStep, RefutableReason};
mod scope;
pub(crate) use scope::{LoopContext, LoopForm};
mod accessors;
mod state;
mod type_builders;

pub use env::TypeEnv;
pub use expr::{
    check_expr, compose_burden_for_idx, infer_expr, register_resolved_collection_burdens,
    resolve_parsed_type,
};
pub(crate) use expr::{tag_to_type_tag, OP_TRAIT_MAP};

use ori_ir::{ExprId, Name, StringInterner};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    check::WellKnownNames, ContextKind, FunctionSig, Idx, MonoInstanceId, PatternKey,
    PatternResolution, TraitRegistry, TypeCheckError, TypeCheckWarning, TypeRegistry, UnifyEngine,
};

/// Expression ID type (mirrors `ori_ir::ExprId`).
///
/// Using a simple usize to avoid dependency on `ori_ir` for the core types module.
/// Maps to `ori_ir::ExprId` when integrating with the module checker.
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
    unify: UnifyEngine<'pool>,

    /// Type environment for name bindings.
    env: TypeEnv,

    /// Inferred types for expressions (expr index → type).
    expr_types: FxHashMap<ExprIndex, Idx>,

    /// Context stack for error reporting.
    context_stack: Vec<ContextKind>,

    /// Accumulated type check errors.
    errors: Vec<TypeCheckError>,

    /// Accumulated type check warnings.
    warnings: Vec<TypeCheckWarning>,

    /// String interner for resolving names in error messages.
    interner: Option<&'pool StringInterner>,

    /// Pre-interned well-known type names for O(1) annotation resolution.
    well_known: Option<&'pool WellKnownNames>,

    /// Trait registry for where-clause validation at call sites.
    trait_registry: Option<&'pool TraitRegistry>,

    /// Function signatures for where-clause lookup.
    signatures: Option<&'pool FxHashMap<Name, FunctionSig>>,

    /// Module-alias namespaces (`use "path" as alias`): each alias name maps to
    /// the aliased module's public function signatures. Consulted to resolve a
    /// qualified call `alias.func(args)` (Spec: Clause 12 Module Alias) against
    /// the named function's signature.
    module_aliases: Option<&'pool FxHashMap<Name, Vec<FunctionSig>>>,

    /// Type registry for struct/enum/newtype lookup during inference.
    type_registry: Option<&'pool TypeRegistry>,

    /// Builtin-type extension methods (`extend str { @m }`, `extend [T] { @m }`)
    /// keyed by builtin `TypeTag`. Populated from `module.extends` at body-check
    /// entry. Consulted by `emit_unknown_method` so an `extend`-provided method on
    /// a builtin receiver is not false-rejected as unknown (TR-9 dispatch stays
    /// target-only; the evaluator resolves the actual call).
    builtin_extensions: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>>,

    /// Current function type for `self` references (recursive calls in patterns).
    self_type: Option<Idx>,

    /// Current impl's `Self` type (for `Self` in type annotations within impl blocks).
    impl_self_type: Option<Idx>,

    /// Stack of enclosing-loop contexts for nested loops.
    /// Each loop form pushes a [`LoopContext`] recording the expected break
    /// value type plus whether `break value` / `continue value` are permitted
    /// (per spec: `loop` and `for...yield` carry values; `while` / `for...do`
    /// do not). `break` / `continue` consult the innermost entry.
    loop_contexts: Vec<scope::LoopContext>,

    /// Stack of active `try {}` propagation boundaries. `Some` frames collect
    /// explicit `?` carriers; `None` frames prevent a nested lambda body from
    /// leaking propagation into an enclosing try block.
    try_boundaries: Vec<Option<Vec<scope::TryPropagation>>>,

    /// Capabilities declared by the current function (`uses` clause).
    current_capabilities: FxHashSet<Name>,

    /// Capabilities provided in scope (`with...in`).
    provided_capabilities: FxHashSet<Name>,

    /// Pattern resolutions accumulated during match checking.
    ///
    /// Records `Binding` patterns that were resolved to unit variants.
    /// Extracted via `take_pattern_resolutions()` after checking.
    pattern_resolutions: Vec<(PatternKey, PatternResolution)>,

    /// Module-level constant types for `$name` reference resolution.
    const_types: Option<&'pool FxHashMap<Name, Idx>>,

    /// Method-level const-generic parameter types for `$name` reference
    /// resolution inside an impl/def-impl method body. Populated at body-check
    /// entry so that bodies can use `$N` in type-arg
    /// positions (e.g., `to_fixed<$N>()`). Owned (not borrowed) because the
    /// scope is per-method, not module-level. Consulted by `const_type` AFTER
    /// the module-level lookup misses.
    method_const_types: FxHashMap<Name, Idx>,

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
    method_rigid_bounds: FxHashMap<u32, Vec<Name>>,

    /// Monomorphization instances discovered during inference.
    ///
    /// Populated when a generic function or method is selected with concrete
    /// type arguments, including operator-selected methods that have no call
    /// expression dispatch entry. Extracted via `take_mono_instances()`.
    mono_instances: Vec<crate::MonoInstance>,

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
    /// Populated by `record_mono_with_dispatch()` from the eager call-site
    /// path (`infer::expr::calls::monomorphization::maybe_record_mono_instance`).
    /// The deferred-resolution path (`check::exports::resolve_deferred_mono_calls`)
    /// is the sub-step 1b-deferred resolution and does NOT populate this map;
    /// the dispatch map
    /// will be silent for transitive generic-calling-generic call sites until
    /// that sub-step lands.
    mono_dispatch_pre_dedup: Vec<(ExprId, MonoInstanceId)>,

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
    assign_desugars: Vec<(ExprId, crate::AssignDesugar)>,

    /// Module-alias qualified calls resolved during this body pass: the call's
    /// AST `ExprId` paired with the qualified imported-function `Name`
    /// (`"alias.func"`). Drained per body via `take_module_alias_calls`,
    /// accumulated into `TypedModule::module_alias_call_map`; consumed by
    /// `ori_canon` to rewrite the namespace `MethodCall` to a free `Call`.
    module_alias_calls: Vec<(ExprId, Name)>,

    /// Iterable->Iterator routed method calls resolved during this body pass.
    /// Keyed by the exact source call `ExprId`, paired with the
    /// type-directed iterator materialization/collection route.
    /// Populated by `record_iter_route()` in `resolve_receiver_and_builtin`'s
    /// Iterable fallthrough; drained per body via `take_iter_routes`, accumulated
    /// into `TypedModule::iter_route_map`; consumed by `ori_canon` to desugar
    /// `recv.method(args)` -> `recv.iter().method(args)` so the materialized
    /// iterator is a real IR node AIMS realizes (vs a backend-hidden one).
    iter_route_desugars: Vec<(ExprId, crate::IterMethodRoute)>,

    /// Deferred monomorphization calls (generic calling generic).
    ///
    /// Populated by `record_deferred_mono_call()` when a generic function calls
    /// another generic with type variables still unresolved. Extracted via
    /// `take_deferred_mono_calls()` and resolved in `finish_with_pool()`.
    deferred_mono_calls: Vec<crate::DeferredMonoCall>,

    /// Set to `true` at the end of body inference, immediately before the
    /// end-of-body defaulting pre-pass runs. `default_unbound_vars_in_scope`
    /// and `default_unbound_vars_from_empty_literals` debug-assert this flag
    /// so a pass-order inversion (defaulting called before body inference
    /// finished) panics in debug builds instead of silently defaulting vars
    /// that bidirectional propagation should have pinned.
    body_inference_complete: bool,

    /// Composed `UserBurdenSpec` entries discovered during monomorphization.
    ///
    /// Populated by
    /// `infer::expr::calls::monomorphization::maybe_record_mono_instance`
    /// when a generic builtin (Option<T>, Result<T, E>, [T], {K: V}, Set<T>)
    /// reaches its first-instantiation form with fully-resolved type args.
    /// Each entry is `(monomorphized_idx, composed_spec)`; consumed downstream
    /// by the body-pass extractor (`take_composed_burdens`) and flushed into
    /// `TypeRegistry::burden` once the mutable-registry surface lands.
    ///
    /// Spec: Annex E §AIMS — burden specs are typed pre-pass input feeding
    /// the lattice-driven analysis. Composition at type-instantiation time
    /// (this accumulator) prevents Phase 5 from emitting indirect dispatch on
    /// each burden walk (proposal §Generic Burden Composition rationale).
    composed_burdens: Vec<(Idx, crate::registry::burden::UserBurdenSpec)>,

    /// Name of the function currently being type-checked.
    ///
    /// Used by `maybe_record_mono_instance()` to identify the caller when
    /// recording deferred mono calls.
    current_function: Option<Name>,

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
    module_scope_snapshot: Option<FxHashSet<Name>>,

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
    pending_generalized_vars: Vec<u32>,
}

#[cfg(test)]
mod tests;
