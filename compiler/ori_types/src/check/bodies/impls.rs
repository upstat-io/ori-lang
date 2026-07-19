//! Impl-method body checking against registered signatures.

mod method;

use ori_ir::{ImplMethod, Module, Name, TraitDef, TraitItem};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::registration::{
    extension_method_has_self, extension_type_params, resolve_type_with_method_generics,
};
use crate::check::ModuleChecker;
use crate::Idx;

use super::method_sig::allocate_generic_binders;
use method::check_impl_method;

/// Check all impl method bodies.
///
/// For trait impls, this also checks unoverridden default methods from the trait
/// definition, registering their signatures for LLVM codegen.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.impls.len()))]
pub(in crate::check) fn check_impl_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    let mut traits: FxHashMap<Name, &TraitDef> = FxHashMap::default();
    for trait_def in &module.traits {
        traits.entry(trait_def.name).or_insert(trait_def);
    }

    for (impl_index, impl_def) in module.impls.iter().enumerate() {
        check_impl_block(checker, impl_def, &traits, impl_index);
    }
}

/// Type check extension bodies using the same signature/export spine as impl
/// methods. Synthetic owner indices follow parsed impl indices so producer
/// identity is collision-free without adding a second identity family.
#[tracing::instrument(level = "debug", skip_all, fields(count = module.extends.len()))]
pub(in crate::check) fn check_extension_bodies(checker: &mut ModuleChecker<'_>, module: &Module) {
    for (extension_index, extension) in module.extends.iter().enumerate() {
        let owner_index = module.impls.len() + extension_index;
        let type_params = extension_type_params(checker, extension);
        let preallocated = checker.impl_rigid_var_map(owner_index).cloned();
        let (mut substitutions, explicit_params, _const_params, inline_bounds) =
            allocate_generic_binders(checker, extension.generics, preallocated.as_ref());
        let generic_params = if extension.generics.is_empty() {
            substitutions = preallocated.unwrap_or_default();
            type_params
        } else {
            explicit_params
        };
        let self_type = resolve_type_with_method_generics(
            checker,
            &extension.target_ty,
            &substitutions,
            &generic_params,
            Idx::ERROR,
        );
        let context = ImplBodyContext {
            impl_index: owner_index,
            self_type,
            trait_type: None,
            type_params: &generic_params,
            substitutions: &substitutions,
            inline_bounds: &inline_bounds,
        };
        let self_kw = checker.well_known().self_kw;
        let arena = checker.arena();
        for method in &extension.methods {
            if extension_method_has_self(arena, method, self_kw) {
                check_impl_method(checker, method, &context);
            }
        }
    }
}

/// Type check methods in an impl block.
///
/// Processes explicit methods first, then unoverridden default methods from the
/// trait definition. Both register signatures via `register_impl_sig` for LLVM
/// codegen consumption (signatures are consumed positionally by `compile_impls`).
fn check_impl_block(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    traits: &FxHashMap<Name, &TraitDef>,
    impl_index: usize,
) {
    // INVARIANT: Body checking reuses registered rigid binders so Self, parameters,
    // returns, and recorded method instances share one identity.
    let impl_prealloc: Option<FxHashMap<Name, Idx>> =
        checker.impl_rigid_var_map(impl_index).cloned();
    let (impl_substitutions, impl_generic_params, _impl_const_params, impl_inline_bounds) =
        allocate_generic_binders(checker, impl_def.generics, impl_prealloc.as_ref());

    // INVARIANT: impl overlays resolve `Self` parameters to their rigid binders.
    let self_type = resolve_type_with_method_generics(
        checker,
        &impl_def.self_ty,
        &impl_substitutions,
        &impl_generic_params,
        Idx::ERROR,
    );

    let is_trait_impl = impl_def.trait_path.is_some();

    // INVARIANT: body checking resolves `Self.Item` with the registration-time bindings.
    let trait_idx = impl_def
        .trait_path
        .as_ref()
        .and_then(|path| path.last().copied())
        .map(|trait_name| checker.pool_mut().named(trait_name));
    let mut assoc_bindings: FxHashMap<Name, Idx> = FxHashMap::default();
    for impl_assoc in &impl_def.assoc_types {
        let ty = resolve_type_with_method_generics(
            checker,
            &impl_assoc.ty,
            &impl_substitutions,
            &impl_generic_params,
            self_type,
        );
        assoc_bindings.insert(impl_assoc.name, ty);
    }

    let impl_context = ImplBodyContext {
        impl_index,
        self_type,
        trait_type: trait_idx,
        type_params: &impl_generic_params,
        substitutions: &impl_substitutions,
        inline_bounds: &impl_inline_bounds,
    };

    checker.with_impl_assoc_scope(assoc_bindings, trait_idx, |checker| {
        for method in &impl_def.methods {
            check_impl_method(checker, method, &impl_context);
            if is_trait_impl {
                checker.register_trait_impl_fn_name(self_type, method.name);
            }
        }

        // Why: Codegen requires signatures for unoverridden trait defaults.
        if let Some(trait_path) = &impl_def.trait_path {
            if let Some(&trait_name) = trait_path.last() {
                let overridden: FxHashSet<Name> = impl_def.methods.iter().map(|m| m.name).collect();

                if let Some(&trait_def) = traits.get(&trait_name) {
                    for item in &trait_def.items {
                        if let TraitItem::DefaultMethod(default) = item {
                            if !overridden.contains(&default.name) {
                                let as_impl = ImplMethod::from(default);
                                check_impl_method(checker, &as_impl, &impl_context);
                                checker.register_trait_impl_fn_name(self_type, default.name);
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Impl-level inputs for method-body checking.
struct ImplBodyContext<'a> {
    impl_index: usize,
    self_type: Idx,
    trait_type: Option<Idx>,
    type_params: &'a [Name],
    substitutions: &'a FxHashMap<Name, Idx>,
    inline_bounds: &'a [(Idx, Vec<Name>)],
}
