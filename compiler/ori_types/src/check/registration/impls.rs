//! Implementation block registration (Pass 0c, part 2).
//!
//! Registers both inherent impls (`impl Type { ... }`) and trait impls
//! (`impl Type: Trait { ... }`). Handles default method inheritance,
//! super-trait transitive defaults, associated types, where clauses,
//! coherence checks, and specificity computation.

use ori_ir::{ExprId, Name, Span};
use rustc_hash::{FxHashMap, FxHashSet};

use super::type_resolution::{
    build_method_generic_metadata, build_where_constraint, collect_generic_param_bounds,
    collect_generic_params, resolve_parsed_type_simple, resolve_type_with_method_generics,
    resolve_type_with_self,
};
use crate::check::bodies::allocate_rigid_var_map;
use crate::registry::burden::UserBurdenSpec;
use crate::registry::burden_compose::scc::mint_compiled_drop_fn_sym;
use crate::{
    Idx, ImplEntry, ImplMethodDef, ImplSpecificity, ModuleChecker, TypeCheckError, WhereConstraint,
};

/// Register implementation blocks.
///
/// For trait impls, also registers unoverridden default methods so they're
/// visible during method resolution in function body checking (Pass 2).
pub fn register_impls(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    for impl_def in &module.impls {
        // Allocate this impl block's `RigidVar` substitution map NOW (Pass 0c),
        // before any body pass, and store it keyed by `module.impls` position.
        // `check_impl_block` (Pass 4) reuses it via `prealloc`. Allocating here —
        // rather than at Pass 4 — is what lets a method mono recorded at a Pass-3
        // call site see the impl binder in `var_states`; the constructor
        // composite (`Pair<RigidVar(B), RigidVar(A)>`) then registers correctly.
        let impl_rigid_var_map = allocate_rigid_var_map(checker, impl_def.generics);
        checker.push_impl_rigid_var_map(impl_rigid_var_map);
        register_impl(checker, impl_def, &module.traits);
    }
}

/// Register a single implementation.
///
/// Converts an `ori_ir::ImplDef` to an `ImplEntry` and registers it in the
/// `TraitRegistry`. Handles both inherent impls (`impl Type { ... }`) and
/// trait impls (`impl Type: Trait { ... }`).
fn register_impl(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
) {
    // 1. Collect generic parameters
    let arena = checker.arena();
    let type_params = collect_generic_params(arena, impl_def.generics);
    let type_param_bounds = collect_generic_param_bounds(arena, impl_def.generics);
    debug_assert_eq!(
        type_params.len(),
        type_param_bounds.len(),
        "type_param_bounds must be index-aligned with type_params"
    );

    // 2. Resolve self type
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, arena);

    // 3. Resolve trait reference (if trait impl): trait name + type arguments.
    let (trait_idx, trait_type_args) = resolve_impl_trait_ref(checker, impl_def);

    // 4. Process associated type definitions FIRST.
    //
    // Registration-ordering: a method whose declared param/return type carries
    // `Self.Item` must resolve that projection against this impl's own
    // `type Item = …` bindings — but the `ImplEntry` (and so the trait
    // registry) is not registered until step 9 below. Build the bindings here,
    // before the method loop, and install them on the checker as the
    // `current_impl_assoc` scope so `resolve_type_with_overlay_inner`'s
    // `ParsedType::AssociatedType` arm projects `Self.Item` from the in-scope
    // map The binding RHS (`type Item = int`) is concrete, so its
    // own resolution never recurses into an unresolvable projection.
    let mut assoc_types = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_self(checker, &impl_assoc.ty, &type_params, self_type);
        assoc_types.insert(impl_assoc.name, ty);
    }

    // 5. Process explicitly defined methods + inherited defaults, under the
    //    associated-type projection scope so `Self.Item` resolves.
    //
    // Explicit user-written impl methods reference impl-level binders directly
    // (e.g. `op: (T, X) -> T` where `X` is the impl's own type-param) — no
    // trait→impl substitution is needed. Pass an empty `trait_substitutions`
    // overlay. The substitution path matters only for inherited defaults at
    // step 5b below (see `inherit_default_methods`).
    let (methods, explicit_methods) =
        checker.with_impl_assoc_scope(assoc_types.clone(), trait_idx, |checker| {
            let empty_trait_subst: FxHashMap<Name, Idx> = FxHashMap::default();
            let mut methods = FxHashMap::default();
            for impl_method in &impl_def.methods {
                let method_def = build_impl_method(
                    checker,
                    impl_method,
                    &type_params,
                    self_type,
                    &empty_trait_subst,
                );
                methods.insert(impl_method.name, method_def);
            }

            // 5b. Inherit unoverridden default methods (direct + transitive).
            //
            // `ImplBuildContext` bundles the three co-varying impl-instance
            // fields (`type_params`, `self_type`, `trait_type_args`).
            // `trait_type_args` is required so direct-default inheritance can
            // build the trait→impl binder substitution map (e.g. `F → X` for
            // `impl<X> Reducer<X>` over `trait Reducer<F>`) — without it, the
            // inherited default's `op: (T, F) -> T` body would carry a dangling
            // `Tag::Named("F")` that fails to unify at call sites.
            let impl_ctx = ImplBuildContext {
                type_params: &type_params,
                self_type,
                trait_type_args: &trait_type_args,
            };
            let explicit_methods = inherit_default_methods(
                checker,
                impl_def,
                traits,
                trait_idx,
                &impl_ctx,
                &mut methods,
            );
            (methods, explicit_methods)
        });

    // 6. Process where clauses (const bounds filtered out — not yet evaluated)
    // Empty scheme_overlay: impl-level where-clauses don't reference method-level
    // binders. Method-level where-clauses go through `build_method_generic_metadata`
    // with a populated overlay.
    let empty_overlay: FxHashMap<Name, Idx> = FxHashMap::default();
    let where_clause: Vec<WhereConstraint> = impl_def
        .where_clauses
        .iter()
        .filter_map(|wc| {
            build_where_constraint(checker, wc, &type_params, &empty_overlay, self_type)
        })
        .collect();

    // 7. Validate associated types, check conflicting defaults, check coherence.
    // Why: an unregistered `trait_idx` (no `TraitEntry` — missing prelude or a typo'd
    // trait name) would ICE the `validate_assoc_types` / `check_conflicting_defaults`
    // debug_asserts; the guard emits a clean E2003 and skips validation, while the
    // impl still registers structurally below so other diagnostics flow.
    if let Some(t_idx) = trait_idx {
        if checker.trait_registry().get_trait_by_idx(t_idx).is_none() {
            let trait_name = impl_def
                .trait_path
                .as_ref()
                .and_then(|path| path.last().copied())
                .unwrap_or_else(|| checker.interner().intern("<trait>"));
            checker.push_error(TypeCheckError::unresolved_trait(impl_def.span, trait_name));
        } else {
            validate_assoc_types(checker, impl_def, t_idx, &assoc_types);
            check_conflicting_defaults(checker, impl_def, t_idx, &explicit_methods);
            if has_coherence_violation(checker, impl_def, t_idx, self_type, &trait_type_args) {
                return;
            }
        }
    }

    // 8. Compute specificity. A non-empty inline bound (`impl<T: Eq>`) makes the
    //    impl Constrained exactly as a trailing `where` clause does.
    let has_inline_bound = type_param_bounds.iter().any(|b| !b.is_empty());
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if !where_clause.is_empty() || has_inline_bound {
        ImplSpecificity::Constrained
    } else {
        ImplSpecificity::Generic
    };

    // 9. Register in TraitRegistry
    let entry = ImplEntry {
        trait_idx,
        trait_type_args,
        self_type,
        type_params,
        type_param_bounds,
        methods,
        assoc_types,
        where_clause,
        specificity,
        span: impl_def.span,
    };

    checker.trait_registry_mut().register_impl(entry);

    // 10. Drop trait wiring: when this impl is `impl T: Drop`,
    //     populate `UserBurdenSpec.user_drop = Some(FnSym)` AND mint
    //     `compiled_drop = Some(FnSym)` so codegen's refcount-zero path
    //     materializes the AUGMENT body (user @drop FIRST, then field
    //     walk in reverse declaration order). The decision
    //     rule: `compiled_drop = Some(_) iff (in non-singleton SCC) OR
    //     (self-loop) OR (user_drop = Some(_))`. The third clause fires
    //     here for every Drop type, including non-recursive ones —
    //     they need an entry point invoked by `ori_rc_dec` at rc==0.
    //
    //     Drop is explicit-impl-only per `drop-trait-proposal.md
    //     §Auto-derive`; population happens at this `register_impl`
    //     site, NOT `register_derived_impl`.
    populate_drop_burden_if_applicable(checker, impl_def, self_type, trait_idx);
}

/// Resolve a trait impl's trait reference: the trait's pool `Idx` (None for
/// inherent impls) plus its resolved type arguments (e.g., `<int, str>` in
/// `impl T: Index<int, str>`).
///
/// Parser invariant: a `trait_path: Some(_)` impl block carries at least one
/// path segment (`impl T: { }` does not parse). An empty `path` indicates a
/// parser-level invariant breach upstream; debug builds surface the violation
/// while release builds fall back to a synthetic "<unknown>" name so the rest
/// of registration continues producing diagnostics rather than panicking.
fn resolve_impl_trait_ref(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
) -> (Option<Idx>, Vec<Idx>) {
    let arena = checker.arena();
    let trait_idx = impl_def.trait_path.as_ref().map(|path| {
        let trait_name = path.last().copied().unwrap_or_else(|| {
            debug_assert!(
                false,
                "register_impl received Some(trait_path) with empty path segments — \
                 parser invariant violated for impl_def.span={:?}",
                impl_def.span
            );
            checker.interner().intern("<unknown>")
        });
        checker.pool_mut().named(trait_name)
    });

    let trait_type_args: Vec<Idx> = {
        let arg_ids = arena.get_parsed_type_list(impl_def.trait_type_args);
        arg_ids
            .iter()
            .map(|&arg_id| {
                let parsed = arena.get_parsed_type(arg_id);
                resolve_parsed_type_simple(checker, parsed, arena)
            })
            .collect()
    };

    (trait_idx, trait_type_args)
}

/// When `impl_def` is a `Drop` impl on `self_type`, populate
/// `UserBurdenSpec.user_drop` and `compiled_drop` on the type's burden
/// entry.
///
/// Resolves `Drop`'s trait `Idx` via the interner; gracefully no-ops if
/// the trait is not yet registered (pre-deployment shape).
///
/// `compiled_drop` is minted via the shared SCC `FnSym` helper so the
/// codegen-side `_ori_drop$<idx_raw>` mangling stays consistent across
/// the three populator sites (recursive, closure, and Drop
/// populators).
fn populate_drop_burden_if_applicable(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    self_type: Idx,
    trait_idx: Option<Idx>,
) {
    let Some(t_idx) = trait_idx else {
        return;
    };

    // Resolve Drop's Idx via interner -> trait_registry.
    let drop_name = checker.interner().intern("Drop");
    let Some(drop_trait_entry) = checker.trait_registry().get_trait_by_name(drop_name) else {
        return;
    };
    if drop_trait_entry.idx != t_idx {
        return;
    }

    // Value/Drop conflict detection (E2049).
    //
    // When `impl T: Drop` is being registered AND T's trait set already
    // carries `Value` (recorded at the type-decl registration site via
    // `TypeRegistry::record_value_marker`), the two markers mutually
    // exclude per Annex E §AIMS: `Value`
    // declares inline storage with no ARC, so the refcount-zero cleanup
    // path that `@drop` hooks into never fires.
    //
    // The span points at the `impl T: Drop` block (the second
    // registration to land); the diagnostic still permits Phase 5 to
    // proceed with whichever burden spec was already populated for T
    // (the codegen gate at the driver level — `PC-4` — suppresses
    // emission when any typeck error remains).
    if checker.type_registry().carries_value_marker(self_type) {
        // Resolve the type's name from the registry for the diagnostic.
        // Fall back to the placeholder `<unknown>` when the type is not
        // registered (shouldn't happen for well-formed input, but the
        // graceful fallback matches the existing pattern in
        // `register_impl`).
        let type_name = checker.type_registry().get_by_idx(self_type).map_or_else(
            || checker.interner().intern("<unknown>"),
            |entry| entry.name,
        );
        checker.push_error(TypeCheckError::value_drop_conflict(
            impl_def.span,
            type_name,
        ));
        // Do NOT short-circuit Drop wiring: emitting the diagnostic
        // suppresses codegen at the driver level, but Phase 5 may still
        // touch the type's burden. Wiring `user_drop` / `compiled_drop`
        // below keeps the spec internally consistent (matches the
        // shape of well-formed Drop types) until the user resolves the
        // conflict by removing one of the two markers.
    }

    let user_drop_fn_sym = mint_compiled_drop_fn_sym(self_type);

    // Look up existing burden + merge: preserve any spec already
    // computed by `burden_compute` at type-registration time;
    // overlay the user_drop / compiled_drop fields.
    let existing = checker
        .type_registry()
        .burden(self_type)
        .cloned()
        .unwrap_or_default();
    let merged = UserBurdenSpec {
        user_drop: Some(user_drop_fn_sym),
        compiled_drop: Some(mint_compiled_drop_fn_sym(self_type)),
        ..existing
    };
    checker
        .type_registry_mut()
        .register_user_burden(self_type, merged);
}

/// Per-impl resolver context.
///
/// Bundles the three impl-instance descriptors that travel together for any
/// per-impl operation: the impl-level type-generic param names, the resolved
/// `Self` type, and the resolved trait type arguments. The three fields
/// co-vary at every site (`inherit_default_methods` and `build_impl_method`
/// both need all three); bundling into a domain newtype keeps the flat
/// signature under clippy's `too_many_arguments` threshold.
///
/// # Note
///
/// `build_impl_method` does not consume `trait_type_args` (only param + return
/// type resolution, which uses `trait_substitutions` directly), so
/// `ImplBuildContext` is consumed only by `inherit_default_methods` today.
struct ImplBuildContext<'a> {
    /// Impl-level type-generic param names (e.g. `["X"]` for
    /// `impl<X> Reducer<X> for Container<X>`).
    type_params: &'a [Name],
    /// Resolved `Self` type for the impl block (e.g. `Container<X>`).
    self_type: Idx,
    /// Resolved trait type arguments (e.g. `[pool.named("X")]` for
    /// `Reducer<X>`). Used by Step 1 to build the trait→impl substitution.
    trait_type_args: &'a [Idx],
}

/// Inherit unoverridden default methods from trait definitions.
///
/// For trait impls, collects default methods from both the direct trait (AST)
/// and transitive super-traits (registry). Returns the set of "explicit"
/// method names (user-defined + direct defaults, excluding transitive) for
/// use in conflicting default detection.
fn inherit_default_methods(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &[ori_ir::TraitDef],
    trait_idx: Option<Idx>,
    impl_ctx: &ImplBuildContext<'_>,
    methods: &mut FxHashMap<Name, ImplMethodDef>,
) -> FxHashSet<Name> {
    let Some(trait_path) = &impl_def.trait_path else {
        // Non-trait impls: all methods are explicit
        return methods.keys().copied().collect();
    };

    // Step 1: Direct defaults from the AST trait definition.
    //
    // Inherited default-method binder remapping: `From<&TraitDefaultMethod>
    // for ImplMethod` at `compiler/ori_ir/src/ast/items/traits.rs` shares
    // the parsed param/return types verbatim, so the trait's view of
    // `(T, F) -> T` survives unchanged into the impl-side `ImplMethodDef`.
    // Without a trait→impl substitution, resolving `F` finds neither a
    // method-level overlay entry NOR an impl-level type_param, so it falls
    // through to a dangling `Tag::Named("F")` that fails to unify at call
    // sites (display-equal `int ≠ int` error).
    //
    // Build `trait_subst: trait_param_name → impl_trait_arg_Idx` from the trait
    // declaration's generics zipped with the impl's resolved trait_type_args
    // (e.g. `F → pool.named("X")` for `impl<X> Reducer<X>` over
    // `trait Reducer<F>`). Pass it through `build_impl_method` so the resolver
    // overlay sees both method-level binders (fresh Vars) AND trait-level
    // binders (impl trait args).
    if let Some(&trait_name) = trait_path.last() {
        if let Some(trait_def) = traits.iter().find(|t| t.name == trait_name) {
            // Collect trait-level type-generic param names (skip const).
            let trait_param_names: Vec<Name> = checker
                .arena()
                .get_generic_params(trait_def.generics)
                .iter()
                .filter(|p| !p.is_const)
                .map(|p| p.name)
                .collect();
            // Build the trait→impl substitution by zipping the trait's
            // declared type-params with the impl's resolved trait_type_args.
            // Length mismatch (e.g. arity-error impl) is silently truncated by
            // zip — the type-checker's earlier coherence/arity checks emit the
            // user-facing diagnostic; the inherited body's resolver simply
            // sees no substitution for the unmapped trait params, which
            // degrades to today's behavior (dangling `Tag::Named`).
            let trait_subst: FxHashMap<Name, Idx> = trait_param_names
                .iter()
                .zip(impl_ctx.trait_type_args.iter())
                .map(|(&name, &idx)| (name, idx))
                .collect();
            for item in &trait_def.items {
                if let ori_ir::TraitItem::DefaultMethod(default) = item {
                    methods.entry(default.name).or_insert_with(|| {
                        let as_impl = ori_ir::ImplMethod::from(default);
                        build_impl_method(
                            checker,
                            &as_impl,
                            impl_ctx.type_params,
                            impl_ctx.self_type,
                            &trait_subst,
                        )
                    });
                }
            }
        }
    }

    // Snapshot explicit methods BEFORE transitive defaults are added.
    let explicit_methods: FxHashSet<Name> = methods.keys().copied().collect();

    // Step 2: Transitive defaults from super-trait hierarchy via the registry.
    // Borrow dance: scope the immutable trait_registry borrow to extract the
    // data we need, then use checker mutably for build_impl_method.
    if let Some(t_idx) = trait_idx {
        let transitive_defaults: Vec<(Name, Idx, ExprId, Span)> = {
            let reg = checker.trait_registry();
            reg.collected_methods(t_idx)
                .into_iter()
                .filter_map(|(name, _owner, def)| {
                    let body = def.default_body?;
                    if !def.has_default {
                        return None;
                    }
                    Some((name, def.signature, body, def.span))
                })
                .collect()
        };

        for (name, signature, body, span) in transitive_defaults {
            methods.entry(name).or_insert(ImplMethodDef {
                name,
                signature,
                has_self: true,
                body,
                scheme_var_ids: Vec::new(),
                generic_param_metadata: Vec::new(),
                where_clause_metadata: Vec::new(),
                // Inherited trait-default method copy: strict arity (trait
                // default-param carry-through is a follow-up per R3-F3).
                optional_param_count: 0,
                span,
            });
        }
    }

    explicit_methods
}

/// Validate that all required associated types are defined in the impl.
fn validate_assoc_types(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    assoc_types: &FxHashMap<Name, Idx>,
) {
    // Registration discipline: trait_idx is queried after the Registration group has
    // populated TraitRegistry (CK-1 pass 0c). A None here indicates the caller passed
    // an Idx that bypassed registration — a missing-registration bug, not "trait has
    // no associated types". Surface in debug builds; release continues without an
    // associated-type check (downstream impl-checking already emits diagnostics for
    // the structural problems an unregistered trait would otherwise produce).
    let Some(trait_entry) = checker.trait_registry().get_trait_by_idx(trait_idx) else {
        debug_assert!(
            false,
            "validate_assoc_types called with unregistered trait_idx={trait_idx:?} — \
             Registration group pass 0c must precede this query (CK-1)"
        );
        return;
    };
    let trait_name = trait_entry.name;
    let required: Vec<Name> = trait_entry
        .assoc_types
        .iter()
        .filter(|(_, def)| def.default.is_none())
        .map(|(&name, _)| name)
        .collect();

    for name in required {
        if !assoc_types.contains_key(&name) {
            checker.push_error(TypeCheckError::missing_assoc_type(
                impl_def.span,
                name,
                trait_name,
            ));
        }
    }
}

/// Check for conflicting default methods inherited from multiple super-traits.
///
/// Only reports conflicts for methods not explicitly overridden in the impl.
fn check_conflicting_defaults(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    explicit_methods: &FxHashSet<Name>,
) {
    // Borrow dance: scope the registry borrow to extract conflict data.
    //
    // Registration discipline: provider_idxs comes from find_conflicting_defaults,
    // which already guarded its own super-trait lookup against unregistered ids
    // (CK-1 pass 0c precedence). Every provider Idx returned here MUST resolve via
    // get_trait_by_idx; a None branch indicates registry inconsistency between
    // find_conflicting_defaults's view and the registry's get_trait_by_idx surface.
    // Surface in debug, fall back to filter_map in release so diagnostic emission
    // continues with the providers we can name.
    let conflicts: Vec<(Name, Vec<Name>)> = {
        let reg = checker.trait_registry();
        reg.find_conflicting_defaults(trait_idx)
            .into_iter()
            .map(|(method_name, provider_idxs)| {
                let names: Vec<Name> = provider_idxs
                    .iter()
                    .filter_map(|&idx| {
                        let entry = reg.get_trait_by_idx(idx);
                        debug_assert!(
                            entry.is_some(),
                            "check_conflicting_defaults: super-trait Idx {idx:?} \
                             returned by find_conflicting_defaults is missing from \
                             registry (CK-1 pass 0c invariant breach)"
                        );
                        entry.map(|e| e.name)
                    })
                    .collect();
                (method_name, names)
            })
            .collect()
    };

    for (method_name, provider_names) in conflicts {
        if !explicit_methods.contains(&method_name) && provider_names.len() >= 2 {
            checker.push_error(TypeCheckError::conflicting_defaults(
                impl_def.span,
                method_name,
                provider_names[0],
                provider_names[1],
            ));
        }
    }
}

/// Check for coherence violations (duplicate impls of the same trait for the same type).
///
/// Returns `true` if a violation was found (caller should skip registration).
fn has_coherence_violation(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    trait_idx: Idx,
    self_type: Idx,
    trait_type_args: &[Idx],
) -> bool {
    // Borrow dance: extract existing impl span and trait name, then push error.
    // Uses type-argument-aware matching so that `impl T: Index<int, str>`
    // and `impl T: Index<str, str>` are correctly treated as distinct.
    let existing: Option<(Span, Name)> = {
        let reg = checker.trait_registry();
        reg.find_impl_with_args(trait_idx, self_type, trait_type_args)
            .and_then(|(_, entry)| {
                let trait_name = reg.get_trait_by_idx(trait_idx).map(|t| t.name)?;
                Some((entry.span, trait_name))
            })
    };
    if let Some((first_span, trait_name)) = existing {
        checker.push_error(TypeCheckError::duplicate_impl(
            impl_def.span,
            first_span,
            trait_name,
        ));
        return true;
    }
    false
}

/// Build an `ImplMethodDef` from an impl method.
///
/// When the method declares method-level type generics
/// (e.g. `@map<U> (self, f: T -> U) -> Box<U>` in `impl<T> Box<T>`),
/// the registered `signature` is wrapped in a `Tag::Scheme(scheme_var_ids,
/// fn_type)`. The scheme's body carries fresh `Tag::Var` Idx values (one per
/// method-level type-generic) whose `var_id`s match `scheme_var_ids`. At
/// call-site, `resolve_impl_signature` invokes `engine.instantiate(...)` —
/// the standard generalization-instantiation pattern — so each
/// call gets fresh unification vars that DO unify with concrete types.
/// Without this wrapping, method-level binders in the registered sig
/// would either be bare `Tag::Var`s (which lower-rank generalization would
/// incorrectly capture) or unresolved `Tag::Named` (which fails to unify
/// against function-typed arguments).
///
/// Body-checking (`check/bodies/impls.rs::check_impl_method`) allocates a
/// SEPARATE set of `Tag::RigidVar`s per method-level binder via
/// `pool.rigid_var(name)` — those `RigidVars` enforce body-internal
/// parametricity (the negative pin `shadow_negative_binder_identity.ori`).
/// Registration's fresh-Var-in-Scheme and body-check's `RigidVars` are distinct
/// pool entries serving distinct purposes; no sharing is required.
fn build_impl_method(
    checker: &mut ModuleChecker<'_>,
    method: &ori_ir::ImplMethod,
    type_params: &[Name],
    self_type: Idx,
    trait_substitutions: &FxHashMap<Name, Idx>,
) -> ImplMethodDef {
    // Deep-copy method-level generics + where-clauses into arena-independent
    // owned form for downstream bound enforcement. Also collect the
    // `Name → Idx` overlay for fresh-Var substitution of method-level type
    // names in param/return resolution.
    let (scheme_var_ids, scheme_overlay, generic_param_metadata, where_clause_metadata) =
        build_method_generic_metadata(
            checker,
            method.generics,
            &method.where_clauses,
            type_params,
            self_type,
        );

    // allocate the method body's `RigidVar`s NOW (Pass 0c) and store
    // them keyed by body `ExprId`, so `check_impl_method` (Pass 4) REUSES them
    // via `prealloc`. Symmetric with the impl-level binder lifecycle: the rigid
    // vars must exist in `var_states` before any pass-3 call-site records a
    // method mono, so the recording's name-scan (`build_impl_rigid_var_subst`)
    // resolves `[Rigid(method_T)] -> concrete` in `body_type_map`. Without early
    // allocation the body's `RigidVar`s are born at Pass 4 (after the recording)
    // and the rigid leaf survives to codegen. The registration-time scheme vars
    // (`scheme_var_ids`, fresh `Tag::Var` for call-site instantiation) and these
    // body rigid vars are distinct pool entries serving distinct purposes.
    let method_rigid_var_map = allocate_rigid_var_map(checker, method.generics);
    checker.set_method_rigid_var_map(method.body, method_rigid_var_map);

    // Merge `trait_substitutions` into the resolver overlay used for
    // param/return type resolution. The trait→impl
    // substitution carries impl-level `Idx` values for the trait's declared
    // type-params (e.g. `F → pool.named("X")` for `impl<X> Reducer<X>` over
    // `trait Reducer<F>`); the method-level `scheme_overlay` carries fresh
    // `Tag::Var` Idx values for method-level binders (e.g. `T → V_T_var`).
    // The two are at different scopes by construction (trait scope vs method
    // scope) — collisions are vanishingly rare, but if they happen the
    // method-level binder shadows per HM scoping, so use `or_insert` to
    // preserve `scheme_overlay`'s entries on collision. The combined overlay
    // is used ONLY for param/return resolution; `scheme_var_ids` and
    // `generic_param_metadata` describe method-level binders alone and are
    // not affected. (Where-clause resolution inside
    // `build_method_generic_metadata` still runs against `scheme_overlay`
    // only — inherited defaults whose where-clauses reference trait-level
    // binders are a known follow-up.)
    let mut combined_overlay = scheme_overlay.clone();
    for (&tname, &tidx) in trait_substitutions {
        combined_overlay.entry(tname).or_insert(tidx);
    }

    // Combined scope for type resolution: impl-level (outer) + method-level
    // (inner) names. `resolve_type_with_method_generics` checks the overlay
    // first (method-level → fresh `Tag::Var`, OR trait-level → impl arg Idx
    // for inherited defaults), then falls through to `type_params.contains`
    // (impl-level → `Tag::Named`).
    let method_param_names: Vec<Name> = scheme_overlay.keys().copied().collect();
    let combined_type_params: Vec<Name> = type_params
        .iter()
        .copied()
        .chain(method_param_names.iter().copied())
        .collect();

    // Resolve parameter types, substituting Self with the actual type and
    // method-level binders with their fresh-Var Idx via the overlay.
    let params: Vec<_> = checker.arena().get_params(method.params).to_vec();
    let param_types: Vec<Idx> = params
        .iter()
        .map(|p| {
            let is_self = p.name == checker.well_known().self_kw;
            match p.ty.as_ref() {
                Some(ty) => resolve_type_with_method_generics(
                    checker,
                    ty,
                    &combined_overlay,
                    &combined_type_params,
                    self_type,
                ),
                None if is_self => self_type,
                None => Idx::ERROR,
            }
        })
        .collect();

    // Resolve return type with the same combined overlay.
    let return_ty = resolve_type_with_method_generics(
        checker,
        &method.return_ty,
        &combined_overlay,
        &combined_type_params,
        self_type,
    );

    // Detect whether the first parameter is `self` (instance method vs associated function)
    let has_self = params
        .first()
        .is_some_and(|p| p.name == checker.well_known().self_kw);

    // Create the function-type body. When the method has method-level type
    // generics (scheme_var_ids non-empty), wrap in Tag::Scheme so call-site
    // resolution can instantiate per `GN-2`. When empty, store the bare
    // function type (zero behavioral change for non-method-generic methods).
    let fn_type = checker.pool_mut().function(&param_types, return_ty);
    let signature = if scheme_var_ids.is_empty() {
        fn_type
    } else {
        checker.pool_mut().scheme(&scheme_var_ids, fn_type)
    };

    // Count of non-`self` params WITH a default value. The relaxed call-site
    // arity check (resolve_impl_signature) permits omitting that many trailing
    // params; canon fills them.
    let optional_param_count = params
        .iter()
        .skip(usize::from(has_self))
        .filter(|p| p.default.is_some())
        .count();

    ImplMethodDef {
        name: method.name,
        signature,
        has_self,
        body: method.body,
        scheme_var_ids,
        generic_param_metadata,
        where_clause_metadata,
        optional_param_count,
        span: method.span,
    }
}

/// Build the builtin-type extension-method index from `module.extends` and
/// install it on the checker. Only extensions whose target resolves to a builtin
/// `TypeTag` are indexed; user-type extensions defer through normal dispatch
/// (a user `Named`/`Applied` receiver is never reject-eligible). Consulted by
/// `emit_unknown_method` so an `extend <builtin> { @m }` method is not
/// false-rejected as unknown (TR-9 dispatch stays target-only — the evaluator
/// owns the actual call).
pub fn register_builtin_extensions(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    let arena = checker.arena();
    let mut index: FxHashMap<ori_registry::TypeTag, FxHashSet<Name>> = FxHashMap::default();
    for ext in &module.extends {
        let target_idx = resolve_parsed_type_simple(checker, &ext.target_ty, arena);
        let tag = checker.pool().tag(target_idx);
        let Some(type_tag) = crate::infer::tag_to_type_tag(tag) else {
            continue;
        };
        let methods = index.entry(type_tag).or_default();
        for m in &ext.methods {
            methods.insert(m.name);
        }
    }
    checker.set_builtin_extensions(index);
}
