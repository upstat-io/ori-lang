//! Imported implementation template registration.

use ori_ir::{ExprId, Name};
use rustc_hash::FxHashMap;

use super::super::type_resolution::{
    build_where_constraint, collect_generic_param_bounds, collect_generic_params,
    resolve_parsed_type_simple, resolve_type_with_method_generics_from,
};
use super::defaults::has_coherence_violation;
use super::methods::build_impl_method_from;
use crate::{
    FunctionSig, Idx, ImplEntry, ImplMethodDef, ImplSpecificity, ImportedImplSig, ModuleChecker,
    Tag,
};

/// Register impl templates owned by one imported module.
///
/// Foreign `ExprId`s never escape as executable identities. Each method is
/// registered with a stable producer symbol + source-signature fingerprint,
/// while every type coordinate is rebuilt in the importing checker's pool.
pub(crate) fn register_imported_impls(
    checker: &mut ModuleChecker<'_>,
    module: &ori_ir::Module,
    foreign_arena: &ori_ir::ExprArena,
    module_identity: &str,
) {
    for (impl_index, impl_def) in module.impls.iter().enumerate() {
        register_imported_impl(
            checker,
            impl_def,
            foreign_arena,
            module_identity,
            impl_index,
        );
    }
}

fn register_imported_impl(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    foreign_arena: &ori_ir::ExprArena,
    module_identity: &str,
    impl_index: usize,
) {
    let type_params = collect_generic_params(foreign_arena, impl_def.generics);
    let type_param_bounds = collect_generic_param_bounds(foreign_arena, impl_def.generics);
    let self_type = resolve_parsed_type_simple(checker, &impl_def.self_ty, foreign_arena);

    let trait_idx = impl_def.trait_path.as_ref().map(|path| {
        let trait_name = path
            .last()
            .copied()
            .unwrap_or_else(|| checker.interner().intern("<unknown>"));
        checker.pool_mut().named(trait_name)
    });
    let trait_type_args: Vec<Idx> = foreign_arena
        .get_parsed_type_list(impl_def.trait_type_args)
        .iter()
        .map(|&id| {
            resolve_parsed_type_simple(checker, foreign_arena.get_parsed_type(id), foreign_arena)
        })
        .collect();

    // A provider has already validated its own impl. Coherence still has to be
    // enforced in the consumer's combined import graph: two provider modules
    // may export equally-specific impls for the same trait/receiver.
    if let Some(trait_idx) = trait_idx {
        if checker
            .trait_registry()
            .get_trait_by_idx(trait_idx)
            .is_none()
        {
            return;
        }
        if has_coherence_violation(checker, impl_def, trait_idx, self_type, &trait_type_args) {
            return;
        }
    }

    let mut assoc_types = FxHashMap::default();
    for assoc in &impl_def.assoc_types {
        let resolved = resolve_type_with_method_generics_from(
            checker,
            &assoc.ty,
            &FxHashMap::default(),
            &type_params,
            self_type,
            foreign_arena,
        );
        assoc_types.insert(assoc.name, resolved);
    }

    let method_context = ImportedImplMethodContext {
        foreign_arena,
        module_identity,
        impl_index,
        self_type,
        trait_type: trait_idx,
        type_params: &type_params,
        type_param_bounds: &type_param_bounds,
    };
    let (methods, origins) = build_imported_methods(checker, impl_def, &method_context);

    let where_clause = impl_def
        .where_clauses
        .iter()
        .filter_map(|clause| {
            build_where_constraint(
                checker,
                clause,
                &type_params,
                &FxHashMap::default(),
                self_type,
            )
        })
        .collect();
    let has_inline_bound = type_param_bounds.iter().any(|bounds| !bounds.is_empty());
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if !impl_def.where_clauses.is_empty() || has_inline_bound {
        ImplSpecificity::Constrained
    } else {
        ImplSpecificity::Generic
    };
    checker.trait_registry_mut().register_impl_with_origin(
        ImplEntry {
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
        },
        Some(crate::registry::RegisteredImplOrigin::Imported(origins)),
    );
}

struct ImportedImplMethodContext<'a> {
    foreign_arena: &'a ori_ir::ExprArena,
    module_identity: &'a str,
    impl_index: usize,
    self_type: Idx,
    trait_type: Option<Idx>,
    type_params: &'a [Name],
    type_param_bounds: &'a [Vec<Name>],
}

fn build_imported_methods(
    checker: &mut ModuleChecker<'_>,
    impl_def: &ori_ir::ImplDef,
    context: &ImportedImplMethodContext<'_>,
) -> (
    FxHashMap<Name, ImplMethodDef>,
    FxHashMap<ExprId, crate::registry::ImportedMethodOrigin>,
) {
    let mut methods = FxHashMap::default();
    let mut origins = FxHashMap::default();
    for (method_index, method) in impl_def.methods.iter().enumerate() {
        let method_def = build_impl_method_from(
            checker,
            method,
            context.type_params,
            context.self_type,
            &FxHashMap::default(),
            context.foreign_arena,
            false,
        );
        let producer = crate::imported_method_producer(
            context.module_identity,
            context.impl_index,
            method_index,
            method,
            context.foreign_arena,
            checker.interner(),
        );
        let crate::MethodProducer::Imported {
            symbol,
            signature_hash,
        } = &producer
        else {
            unreachable!("imported_method_producer must return Imported")
        };
        origins.insert(
            method.body,
            crate::registry::ImportedMethodOrigin {
                symbol: symbol.clone(),
                signature_hash: *signature_hash,
            },
        );
        checker.imported_impl_sigs.push(build_imported_impl_sig(
            checker,
            ImportedSignatureInput {
                method,
                method_def: &method_def,
                producer,
                receiver: context.self_type,
                trait_type: context.trait_type,
                impl_type_params: context.type_params,
                impl_type_param_bounds: context.type_param_bounds,
                foreign_arena: context.foreign_arena,
            },
        ));
        methods.insert(method.name, method_def);
    }
    (methods, origins)
}

struct ImportedSignatureInput<'a> {
    method: &'a ori_ir::ImplMethod,
    method_def: &'a ImplMethodDef,
    producer: crate::MethodProducer,
    receiver: Idx,
    trait_type: Option<Idx>,
    impl_type_params: &'a [Name],
    impl_type_param_bounds: &'a [Vec<Name>],
    foreign_arena: &'a ori_ir::ExprArena,
}

fn build_imported_impl_sig(
    checker: &ModuleChecker<'_>,
    input: ImportedSignatureInput<'_>,
) -> ImportedImplSig {
    let ImportedSignatureInput {
        method,
        method_def,
        producer,
        receiver,
        trait_type,
        impl_type_params,
        impl_type_param_bounds,
        foreign_arena,
    } = input;
    let signature = if checker.pool().tag(method_def.signature) == Tag::Scheme {
        checker.pool().scheme_body(method_def.signature)
    } else {
        method_def.signature
    };
    let param_types = checker.pool().function_params(signature);
    let return_type = checker.pool().function_return(signature);
    let params = foreign_arena.get_params(method.params);
    let method_type_params: Vec<Name> = foreign_arena
        .get_generic_params(method.generics)
        .iter()
        .filter(|generic| !generic.is_const)
        .map(|generic| generic.name)
        .collect();
    let method_bounds = foreign_arena
        .get_generic_params(method.generics)
        .iter()
        .filter(|generic| !generic.is_const)
        .map(|generic| {
            generic
                .bounds
                .iter()
                .map(ori_ir::TraitBound::name)
                .collect()
        });
    let type_params: Vec<Name> = impl_type_params
        .iter()
        .copied()
        .chain(method_type_params)
        .collect();
    let type_param_bounds = impl_type_param_bounds
        .iter()
        .cloned()
        .chain(method_bounds)
        .collect();
    let param_defaults: Vec<_> = params.iter().map(|param| param.default).collect();
    let required_params = param_defaults
        .iter()
        .filter(|default| default.is_none())
        .count();
    let sig = FunctionSig {
        name: method.name,
        type_params,
        const_params: Vec::new(),
        param_names: params.iter().map(|param| param.name).collect(),
        param_hashes: param_types
            .iter()
            .map(|&parameter| checker.pool().hash(parameter))
            .collect(),
        param_types,
        return_type,
        return_hash: checker.pool().hash(return_type),
        capabilities: method.capabilities.iter().map(|cap| cap.name).collect(),
        capability_params: Vec::new(),
        is_public: false,
        is_test: false,
        is_main: false,
        is_fbip: false,
        type_param_bounds,
        where_clauses: Vec::new(),
        generic_param_mapping: Vec::new(),
        scheme_var_ids: method_def.scheme_var_ids.clone(),
        required_params,
        param_defaults,
        return_projection: None,
    };
    ImportedImplSig {
        producer,
        receiver,
        trait_type,
        name: method.name,
        has_self: method_def.has_self,
        sig,
    }
}
