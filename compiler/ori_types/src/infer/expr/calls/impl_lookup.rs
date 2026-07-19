//! Impl method lookup and signature resolution via `TraitRegistry`.

use rustc_hash::FxHashMap;

use ori_ir::Name;

use super::super::super::InferEngine;
use crate::pool::substitute::{substitute_named_in_pool, substitute_self_in_pool};
use crate::{
    BoundChainLookup, GenericParamMeta, Idx, MethodLookupResult, Pool, Tag, WhereConstraint,
};

/// A trait-impl method match: method def, impl-binder substitution, trait
/// name (ambiguity reporting), impl type-params in declaration order.
type TraitMatch<'a> = (
    &'a crate::ImplMethodDef,
    FxHashMap<Name, Idx>,
    Name,
    Vec<Name>,
    crate::MethodProducer,
);

/// An inherent-impl method match: method def, impl-binder substitution, impl
/// type-params in declaration order.
type InherentMatch<'a> = (
    &'a crate::ImplMethodDef,
    FxHashMap<Name, Idx>,
    Vec<Name>,
    crate::MethodProducer,
);

/// An extension-method match. The final `Name` is a diagnostic label for
/// ambiguity; producer identity remains the exact `ImplMethodId`.
type ExtensionMatch<'a> = (
    &'a crate::ImplMethodDef,
    FxHashMap<Name, Idx>,
    Vec<Name>,
    crate::MethodProducer,
    Name,
);

/// Result of looking up a method in the `TraitRegistry`.
pub(super) enum LookupOutcome {
    Found {
        /// Exact checker-selected executable producer.
        producer: Option<crate::MethodProducer>,
        sig: Idx,
        has_self: bool,
        /// Method-level where-clause constraints, deep-copied owned form.
        /// Empty when the method has no `where` clause.
        where_clause_metadata: Vec<WhereConstraint>,
        /// Method-level generic parameter metadata (one entry per declared
        /// param, type AND const). Carries the inline `<T: Bound>` info that
        /// `check_method_inline_bounds` enforces at the call site.
        generic_param_metadata: Vec<GenericParamMeta>,
        /// Fixed-list capacity expressions depending on method const binders.
        fixed_list_capacity_constraints: Vec<crate::GenericConstExpr>,
        /// Pool `var_id`s for the method's quantified type variables in
        /// declaration order (parallel to non-const params in
        /// `generic_param_metadata`). Used to map each binder name to the
        /// fresh Var Idx allocated during call-site `instantiate_with_subst`.
        scheme_var_ids: Vec<u32>,
        /// Impl-level binder substitution from receiver-vs-`entry.self_type`
        /// structural match. `Name → Idx` keyed on the impl's `type_params`
        /// (e.g. `{U: int}` for `impl<U> Box<U>` matched against `Box<int>`).
        /// Empty for non-generic impls (exact-Idx primary lookup); populated
        /// only by the base-name fallback path. `resolve_impl_signature`
        /// applies this via `substitute_named_in_pool` BEFORE method-level
        /// `Tag::Scheme` instantiation.
        impl_subst: FxHashMap<Name, Idx>,
        /// Impl-level binder names in declaration order for structurally
        /// matched generic providers. Empty for exact-`Idx` primary lookups and
        /// bound-chain dispatch.
        /// Non-empty iff the resolved method is eligible for receiver-bearing
        /// monomorphization recording (it pins both "inherent" and the order
        /// for `impl_args`).
        impl_type_params: Vec<Name>,
        /// Count of non-`self` params WITH a default value.
        /// Drives the relaxed call-site arity check; `0` = strict equality.
        optional_param_count: usize,
    },
    Ambiguous(Vec<ori_ir::Name>),
    NotFound,
}

/// Receiver-side monomorphization carrier for a method resolved on a generic
/// provider (`impl<T> Box<T>` or `extend [T]`). `impl_type_args` lists the provider-level
/// binders in declaration order paired with their concrete substitution
/// (`[(T, int)]` for `impl<T> Box<T>` matched against `Box<int>`).
///
/// Builtin dispatch, bound-chain dispatch, and non-generic providers leave the
/// enclosing `Option` `None`, so the method-mono emission hook stays inert.
pub(super) struct MethodMonoData {
    /// Impl-level binders in declaration order, each paired with its concrete
    /// substitution (`Name → Idx`). Drives the emitted instance's `impl_args`
    /// + the impl-binder entries of its `body_type_map`.
    pub(super) impl_type_args: Vec<(Name, Idx)>,
}

/// Successfully resolved impl method signature.
pub(super) struct ImplMethodSig {
    /// Exact checker-selected producer when dispatch reached a concrete impl.
    pub(super) producer: Option<crate::MethodProducer>,
    /// Method parameters (excluding `self`).
    pub(super) params: Vec<Idx>,
    /// Return type.
    pub(super) ret: Idx,
    /// Receiver-side monomorphization carrier — `Some` only for an inherent
    /// method on a generic impl instantiated at a concrete receiver. Consumed
    /// by the method-mono emission hook to mint a `MonoInstance::new_method`.
    pub(super) method_mono: Option<MethodMonoData>,
    /// Method-level where-clause constraints, forwarded from `LookupOutcome::Found`
    /// for call-site bound enforcement (`check_method_where_clauses`).
    pub(super) where_clause_metadata: Vec<WhereConstraint>,
    /// Method-level generic parameter metadata (inline `<T: Bound>` form).
    /// Forwarded for call-site enforcement via `check_method_inline_bounds`.
    pub(super) generic_param_metadata: Vec<GenericParamMeta>,
    /// Fixed-list capacity expressions depending on method const binders.
    pub(super) fixed_list_capacity_constraints: Vec<crate::GenericConstExpr>,
    /// Pool `var_id`s for the method's quantified type variables. Parallel to
    /// the non-const entries of `generic_param_metadata` in declaration order.
    /// Used by `check_method_inline_bounds` to zip param → `var_id` → subst.
    pub(super) scheme_var_ids: Vec<u32>,
    /// Substitution map produced by `instantiate_with_subst` at call site:
    /// `scheme_var_id → fresh_var_idx`. Empty when the method had no
    /// `Tag::Scheme` wrap (no method-level type generics). Lets the bound
    /// checker resolve each method-level binder to its post-instantiation
    /// concrete type via `engine.resolve(subst[scheme_var_id])`.
    pub(super) instantiation_subst: FxHashMap<u32, Idx>,
}

/// Extract the base type name from a receiver / pattern type.
///
/// `Tag::Applied(Name, [args])` and `Tag::Named(Name)` carry a nominal head
/// name extractable in O(1). Structural receivers have no nominal base; they
/// bypass this fast prefilter and rely on the exact structural matcher below.
pub(crate) fn pool_base_name(pool: &Pool, ty: Idx) -> Option<Name> {
    match pool.tag(ty) {
        Tag::Applied => Some(pool.applied_name(ty)),
        Tag::Named => Some(pool.named_name(ty)),
        _ => None,
    }
}

/// Structurally match a registered provider's `entry.self_type` (the pattern,
/// containing `Tag::Named(binder)` references for provider-level type params)
/// against a concrete receiver type (the target).
///
/// Returns `Some(Name → Idx)` mapping each impl-level binder to the
/// corresponding concrete sub-tree of the target when the structures align.
/// Returns `None` when the structures diverge.
///
/// `type_params` lists the impl's binder names — only `Tag::Named(name)`
/// references whose name is in this set are treated as binders; other
/// `Tag::Named` references are nominal type lookups requiring exact-Idx match.
///
/// Engine half of the dispatch path: the engine owns inference state (EN-2)
/// while the registry stays a frozen-after-registration data store (RG-2).
pub(crate) fn match_self_type(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
) -> Option<FxHashMap<Name, Idx>> {
    let mut subst: FxHashMap<Name, Idx> = FxHashMap::default();
    if match_self_type_inner(pool, pattern, target, type_params, &mut subst) {
        Some(subst)
    } else {
        None
    }
}

/// `(Tag::Named, _)` arm: `pattern` is either an impl-level binder (bind or
/// re-check against `subst`) or a nominal type ref requiring exact-Idx match
/// (already ruled out by the caller's `pattern == target` fast path).
fn match_named_binder(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    let name = pool.named_name(pattern);
    if !type_params.contains(&name) {
        // Nominal type ref, not a binder — require exact-Idx match.
        return false;
    }
    if let Some(&existing) = subst.get(&name) {
        existing == target
    } else {
        subst.insert(name, target);
        true
    }
}

/// `(Tag::Applied, Tag::Applied)` arm: same nominal head + structurally
/// matching generic args, pairwise.
fn match_applied(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    if pool.applied_name(pattern) != pool.applied_name(target) {
        return false;
    }
    let pargs = pool.applied_args(pattern);
    let targs = pool.applied_args(target);
    if pargs.len() != targs.len() {
        return false;
    }
    pargs
        .iter()
        .zip(targs.iter())
        .all(|(&p, &t)| match_self_type_inner(pool, p, t, type_params, subst))
}

/// `(Tag::Map, Tag::Map)` arm: key and value both structurally match.
fn match_map(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    match_self_type_inner(
        pool,
        pool.map_key(pattern),
        pool.map_key(target),
        type_params,
        subst,
    ) && match_self_type_inner(
        pool,
        pool.map_value(pattern),
        pool.map_value(target),
        type_params,
        subst,
    )
}

/// `(Tag::Result, Tag::Result)` arm: Ok and Err payloads both structurally match.
fn match_result(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    match_self_type_inner(
        pool,
        pool.result_ok(pattern),
        pool.result_ok(target),
        type_params,
        subst,
    ) && match_self_type_inner(
        pool,
        pool.result_err(pattern),
        pool.result_err(target),
        type_params,
        subst,
    )
}

/// `(Tag::Tuple, Tag::Tuple)` arm: same arity + every element structurally matches.
fn match_tuple(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    let pattern_elems = pool.tuple_elems(pattern);
    let target_elems = pool.tuple_elems(target);
    pattern_elems.len() == target_elems.len()
        && pattern_elems
            .iter()
            .zip(&target_elems)
            .all(|(&pattern, &target)| {
                match_self_type_inner(pool, pattern, target, type_params, subst)
            })
}

/// `(Tag::Function, Tag::Function)` arm: same arity + every param plus the
/// return type structurally match.
fn match_function(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    let pattern_params = pool.function_params(pattern);
    let target_params = pool.function_params(target);
    pattern_params.len() == target_params.len()
        && pattern_params
            .iter()
            .zip(&target_params)
            .all(|(&pattern, &target)| {
                match_self_type_inner(pool, pattern, target, type_params, subst)
            })
        && match_self_type_inner(
            pool,
            pool.function_return(pattern),
            pool.function_return(target),
            type_params,
            subst,
        )
}

fn match_self_type_inner(
    pool: &Pool,
    pattern: Idx,
    target: Idx,
    type_params: &[Name],
    subst: &mut FxHashMap<Name, Idx>,
) -> bool {
    if pattern == target {
        return true;
    }
    match (pool.tag(pattern), pool.tag(target)) {
        (Tag::Named, _) => match_named_binder(pool, pattern, target, type_params, subst),
        (Tag::Applied, Tag::Applied) => match_applied(pool, pattern, target, type_params, subst),
        (
            Tag::List
            | Tag::Option
            | Tag::Set
            | Tag::Channel
            | Tag::Range
            | Tag::Iterator
            | Tag::DoubleEndedIterator,
            target_tag,
        ) if pool.tag(pattern) == target_tag => match_self_type_inner(
            pool,
            Idx::from_raw(pool.data(pattern)),
            Idx::from_raw(pool.data(target)),
            type_params,
            subst,
        ),
        (Tag::Map, Tag::Map) => match_map(pool, pattern, target, type_params, subst),
        (Tag::Result, Tag::Result) => match_result(pool, pattern, target, type_params, subst),
        (Tag::Tuple, Tag::Tuple) => match_tuple(pool, pattern, target, type_params, subst),
        (Tag::Function, Tag::Function) => match_function(pool, pattern, target, type_params, subst),
        _ => false,
    }
}

/// Result of the base-name fallback search — single match, ambiguous, or none.
enum FallbackResult {
    Single {
        producer: crate::MethodProducer,
        sig: Idx,
        has_self: bool,
        where_clause_metadata: Vec<WhereConstraint>,
        generic_param_metadata: Vec<GenericParamMeta>,
        fixed_list_capacity_constraints: Vec<crate::GenericConstExpr>,
        scheme_var_ids: Vec<u32>,
        impl_subst: FxHashMap<Name, Idx>,
        /// Impl-level binder names in declaration order — populated for the
        /// selected structurally matched provider.
        impl_type_params: Vec<Name>,
        optional_param_count: usize,
    },
    Ambiguous(Vec<Name>),
    None,
}

/// Structural fallback: iterate registered impls, structurally match
/// `entry.self_type` against the receiver, and return the resolved candidate.
///
/// Provider tiers are inherent, trait, then extension. Within trait and
/// extension tiers, conflicts return `Ambiguous` so the caller emits `E2023`.
/// Build the `FallbackResult::Single` variant shared verbatim by the
/// inherent, trait, and extension resolution tiers of
/// [`lookup_method_by_base_match`] — every tier resolves to the same shape,
/// differing only in which match list produced the `method_def`.
fn build_fallback_single(
    method_def: &crate::ImplMethodDef,
    impl_subst: FxHashMap<Name, Idx>,
    impl_type_params: Vec<Name>,
    producer: crate::MethodProducer,
) -> FallbackResult {
    FallbackResult::Single {
        producer,
        sig: method_def.signature,
        has_self: method_def.has_self,
        where_clause_metadata: method_def.where_clause_metadata.clone(),
        generic_param_metadata: method_def.generic_param_metadata.clone(),
        fixed_list_capacity_constraints: method_def.fixed_list_capacity_constraints.clone(),
        scheme_var_ids: method_def.scheme_var_ids.clone(),
        impl_subst,
        impl_type_params,
        optional_param_count: method_def.optional_param_count,
    }
}

/// Classify every registered impl providing `method` into inherent, trait, or
/// extension tiers by structurally matching `entry.self_type` against
/// `receiver_ty`. Feeds the tiered resolution in
/// [`lookup_method_by_base_match`].
fn classify_impl_matches<'a>(
    pool: &Pool,
    reg: &'a crate::TraitRegistry,
    receiver_ty: Idx,
    receiver_base: Option<Name>,
    method: Name,
) -> (
    Vec<InherentMatch<'a>>,
    Vec<TraitMatch<'a>>,
    Vec<ExtensionMatch<'a>>,
) {
    // Both inherent and trait matches carry the impl's `type_params` (declaration
    // order) so the method-mono emission hook can build `impl_args` in a canonical
    // order — a trait method on a generic impl (`impl<T> Box<T>: Container`) needs
    // its receiver-instantiated mono recorded for LLVM codegen exactly like an
    // inherent one. Trait matches additionally carry the trait `Name` for
    // ambiguity reporting.
    let mut inherent_matches: Vec<InherentMatch<'a>> = Vec::new();
    let mut trait_matches: Vec<TraitMatch<'a>> = Vec::new();
    let mut extension_matches: Vec<ExtensionMatch<'a>> = Vec::new();

    for (impl_idx, entry) in reg.impls_iter() {
        if let (Some(receiver_base), Some(entry_base)) =
            (receiver_base, pool_base_name(pool, entry.self_type))
        {
            let root_is_binder = pool.tag(entry.self_type) == Tag::Named
                && entry
                    .type_params
                    .contains(&pool.named_name(entry.self_type));
            if !root_is_binder && entry_base != receiver_base {
                continue;
            }
        }
        let Some(method_def) = entry.methods.get(&method) else {
            continue;
        };
        let Some(impl_subst) =
            match_self_type(pool, entry.self_type, receiver_ty, &entry.type_params)
        else {
            continue;
        };
        let Some(producer) = reg.method_producer(impl_idx, method_def) else {
            continue;
        };
        if reg.is_extension_impl(impl_idx) {
            extension_matches.push((
                method_def,
                impl_subst,
                entry.type_params.clone(),
                producer,
                method,
            ));
            continue;
        }
        match entry.trait_idx {
            None => {
                inherent_matches.push((
                    method_def,
                    impl_subst,
                    entry.type_params.clone(),
                    producer,
                ));
            }
            Some(trait_idx) => {
                let trait_name = reg.get_trait_by_idx(trait_idx).map_or(method, |t| t.name);
                trait_matches.push((
                    method_def,
                    impl_subst,
                    trait_name,
                    entry.type_params.clone(),
                    producer,
                ));
            }
        }
    }

    (inherent_matches, trait_matches, extension_matches)
}

fn lookup_method_by_base_match(
    engine: &InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> FallbackResult {
    let pool = engine.pool();
    let receiver_base = pool_base_name(pool, receiver_ty);
    let Some(reg) = engine.trait_registry() else {
        return FallbackResult::None;
    };

    let (inherent_matches, trait_matches, extension_matches) =
        classify_impl_matches(pool, reg, receiver_ty, receiver_base, method);

    // Inherent wins; ambiguity within inherent is a registration error caught
    // earlier (coherence check `TR-5`), so first hit suffices.
    if let Some((method_def, impl_subst, impl_type_params, producer)) =
        inherent_matches.into_iter().next()
    {
        return build_fallback_single(method_def, impl_subst, impl_type_params, producer);
    }

    if trait_matches.len() > 1 {
        let trait_names: Vec<Name> = trait_matches.iter().map(|(_, _, n, _, _)| *n).collect();
        return FallbackResult::Ambiguous(trait_names);
    }
    if let Some((method_def, impl_subst, _, impl_type_params, producer)) =
        trait_matches.into_iter().next()
    {
        return build_fallback_single(method_def, impl_subst, impl_type_params, producer);
    }

    if extension_matches.len() > 1 {
        return FallbackResult::Ambiguous(
            extension_matches
                .iter()
                .map(|(_, _, _, _, label)| *label)
                .collect(),
        );
    }
    if let Some((method_def, impl_subst, impl_type_params, producer, _)) =
        extension_matches.into_iter().next()
    {
        return build_fallback_single(method_def, impl_subst, impl_type_params, producer);
    }

    FallbackResult::None
}

/// Perform the borrow-dance lookup for impl methods via `TraitRegistry`.
///
/// Scopes the immutable `trait_registry` borrow to extract data, so the
/// caller can use `engine` mutably afterwards.
///
/// Two-phase lookup:
/// 1. Exact-`Idx` primary lookup via `lookup_method_checked` — fast path,
///    matches concrete impls (`impl Box<int>`) and impls registered against
///    the receiver's exact pool index.
/// 2. Base-name fallback via `lookup_method_by_base_match` — fires only on
///    primary-lookup miss. Iterates registered impls, structurally matches
///    each `entry.self_type` against the receiver, and returns the resolved
///    candidate with its impl-level substitution map. This is what makes
///    `b: Box<int>` dispatch to `impl<U> Box<U> { @m<T> ... }` work despite
///    `Applied(Box, [Named(U)]) ≠ Applied(Box, [Int])`.
pub(super) fn lookup_impl_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let primary = {
        let Some(reg) = engine.trait_registry() else {
            return LookupOutcome::NotFound;
        };
        reg.lookup_method_checked(receiver_ty, method)
    };
    match primary {
        MethodLookupResult::Found(lookup) => {
            let m = lookup.method();
            let producer = engine
                .trait_registry()
                .and_then(|registry| registry.method_producer(lookup.impl_idx(), m));
            return LookupOutcome::Found {
                producer,
                sig: m.signature,
                has_self: m.has_self,
                where_clause_metadata: m.where_clause_metadata.clone(),
                generic_param_metadata: m.generic_param_metadata.clone(),
                fixed_list_capacity_constraints: m.fixed_list_capacity_constraints.clone(),
                scheme_var_ids: m.scheme_var_ids.clone(),
                impl_subst: FxHashMap::default(),
                // Exact-`Idx` primary lookup matches non-generic impls; no
                // receiver-side binders to record.
                impl_type_params: Vec::new(),
                optional_param_count: m.optional_param_count,
            };
        }
        MethodLookupResult::Ambiguous { candidates } => {
            return LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect());
        }
        MethodLookupResult::NotFound => {
            // Fall through to base-name fallback below.
        }
    }

    match lookup_method_by_base_match(engine, receiver_ty, method) {
        FallbackResult::Single {
            producer,
            sig,
            has_self,
            where_clause_metadata,
            generic_param_metadata,
            fixed_list_capacity_constraints,
            scheme_var_ids,
            impl_subst,
            impl_type_params,
            optional_param_count,
        } => {
            return LookupOutcome::Found {
                producer: Some(producer),
                sig,
                has_self,
                where_clause_metadata,
                generic_param_metadata,
                fixed_list_capacity_constraints,
                scheme_var_ids,
                impl_subst,
                impl_type_params,
                optional_param_count,
            };
        }
        FallbackResult::Ambiguous(trait_names) => return LookupOutcome::Ambiguous(trait_names),
        FallbackResult::None => {}
    }

    lookup_bound_method(engine, receiver_ty, method)
}

/// Resolve a method through the declared bounds of a rigid generic receiver.
fn lookup_bound_method(
    engine: &mut InferEngine<'_>,
    receiver_ty: Idx,
    method: Name,
) -> LookupOutcome {
    let receiver_tag = engine.pool().tag(receiver_ty);
    if matches!(receiver_tag, Tag::RigidVar | Tag::Var) {
        let Some(bounds) = engine.rigid_var_bounds(receiver_ty).map(<[Name]>::to_vec) else {
            return LookupOutcome::NotFound;
        };
        let lookup = {
            let Some(reg) = engine.trait_registry() else {
                return LookupOutcome::NotFound;
            };
            reg.find_trait_method_via_bound_chain(method, &bounds)
        };
        match lookup {
            BoundChainLookup::Found {
                trait_idx: _,
                method: tm,
            } => {
                // Capture method metadata + raw signature before releasing
                // the trait_registry borrow (which was held inside the
                // closure that returned `lookup`). Then substitute
                // `Tag::SelfType` -> `receiver_ty` in the signature so
                // chained calls like `val.clone().to_str()` see the
                // receiver's type for the second-call dispatch instead of
                // a fresh unification var.
                let raw_sig = tm.signature;
                let method_has_self = tm.has_self;
                let where_clause_metadata = tm.where_clause_metadata.clone();
                let generic_param_metadata = tm.generic_param_metadata.clone();
                let fixed_list_capacity_constraints = tm.fixed_list_capacity_constraints.clone();
                let scheme_var_ids = tm.scheme_var_ids.clone();
                // Trait method sigs are registered with `Self` resolved to
                // `Tag::Named("Self")` per `check/registration/type_resolution.rs`,
                // so `substitute_named_in_pool` with a `{Self -> receiver_ty}`
                // mapping pins the method's `Self` references to the actual
                // receiver. The companion `substitute_self_in_pool` walks
                // `Tag::SelfType` placeholders for paths that may use the
                // tag-level form.
                let sig = if let Some(self_name) = engine.intern_name("Self") {
                    let mut self_subst = FxHashMap::default();
                    self_subst.insert(self_name, receiver_ty);
                    let named_substituted =
                        substitute_named_in_pool(engine.pool_mut(), raw_sig, &self_subst);
                    substitute_self_in_pool(engine.pool_mut(), named_substituted, receiver_ty)
                } else {
                    substitute_self_in_pool(engine.pool_mut(), raw_sig, receiver_ty)
                };
                LookupOutcome::Found {
                    producer: None,
                    sig,
                    // Use the trait method's actual self-ness: an instance method
                    // (`hello(self)`) consumes the receiver as `self`; a no-`self`
                    // capability/associated method (`get(url: str)`) does not, so
                    // the arity check (`impl_signature.rs` `skip = has_self`) must
                    // count every declared param as an explicit argument.
                    has_self: method_has_self,
                    where_clause_metadata,
                    generic_param_metadata,
                    fixed_list_capacity_constraints,
                    scheme_var_ids,
                    impl_subst: FxHashMap::default(),
                    // RigidVar / Var receiver — not a concrete instantiation;
                    // no receiver-side binders to record.
                    impl_type_params: Vec::new(),
                    // Bound-chain dispatch: strict arity. Trait-method default
                    // carry-through is not threaded on this path.
                    optional_param_count: 0,
                }
            }
            BoundChainLookup::Ambiguous { candidates } => {
                LookupOutcome::Ambiguous(candidates.iter().map(|&(_, n)| n).collect())
            }
            BoundChainLookup::NotFound => LookupOutcome::NotFound,
        }
    } else {
        LookupOutcome::NotFound
    }
}
