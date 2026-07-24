//! Registration-boundary validation for fixed-list capacity annotations.

use ori_ir::{
    ExprArena, GenericParamRange, ImplMethod, Module, ParamRange, ParsedType, TraitItem,
    TypeDeclKind,
};

use crate::infer::validate_fixed_list_capacities;
use crate::ModuleChecker;

use super::super::signatures::resolve_const_param_type;

/// Validate fixed-list capacities in every module-level declared type after
/// module constants have value evidence and before signatures freeze.
pub fn validate_declared_fixed_list_capacities(checker: &mut ModuleChecker<'_>, module: &Module) {
    let arena = checker.arena();

    for declaration in &module.types {
        let mut types = generic_declared_types(arena, declaration.generics);
        match &declaration.kind {
            TypeDeclKind::Struct(fields) => {
                types.extend(fields.iter().map(|field| &field.ty));
            }
            TypeDeclKind::Sum(variants) => {
                types.extend(
                    variants
                        .iter()
                        .flat_map(|variant| variant.fields.iter().map(|field| &field.ty)),
                );
            }
            TypeDeclKind::Newtype(underlying) => types.push(underlying),
        }
        validate_types(checker, arena, &[declaration.generics], &types);
    }

    for function in &module.functions {
        let mut types = generic_declared_types(arena, function.generics);
        append_parameter_types(arena, function.params, &mut types);
        types.extend(function.return_ty.as_ref());
        validate_types(checker, arena, &[function.generics], &types);
    }

    for test in &module.tests {
        let mut types = Vec::new();
        append_parameter_types(arena, test.params, &mut types);
        types.extend(test.return_ty.as_ref());
        validate_types(checker, arena, &[], &types);
    }

    for trait_def in &module.traits {
        let generic_types = generic_declared_types(arena, trait_def.generics);
        validate_types(checker, arena, &[trait_def.generics], &generic_types);
        for item in &trait_def.items {
            match item {
                TraitItem::MethodSig(method) => validate_method_parts(
                    checker,
                    arena,
                    &[trait_def.generics],
                    method.generics,
                    method.params,
                    &method.return_ty,
                ),
                TraitItem::DefaultMethod(method) => validate_method_parts(
                    checker,
                    arena,
                    &[trait_def.generics],
                    method.generics,
                    method.params,
                    &method.return_ty,
                ),
                TraitItem::AssocType(assoc) => {
                    let types: Vec<_> = assoc.default_type.iter().collect();
                    validate_types(checker, arena, &[trait_def.generics], &types);
                }
            }
        }
    }

    for impl_def in &module.impls {
        let mut types = generic_declared_types(arena, impl_def.generics);
        types.push(&impl_def.self_ty);
        types.extend(
            arena
                .get_parsed_type_list(impl_def.trait_type_args)
                .iter()
                .map(|&id| arena.get_parsed_type(id)),
        );
        types.extend(impl_def.assoc_types.iter().map(|assoc| &assoc.ty));
        validate_types(checker, arena, &[impl_def.generics], &types);
        for method in &impl_def.methods {
            validate_impl_method(checker, arena, &[impl_def.generics], method);
        }
    }

    for def_impl in &module.def_impls {
        for method in &def_impl.methods {
            validate_impl_method(checker, arena, &[], method);
        }
    }

    for extension in &module.extends {
        let mut types = generic_declared_types(arena, extension.generics);
        types.push(&extension.target_ty);
        validate_types(checker, arena, &[extension.generics], &types);
        for method in &extension.methods {
            validate_impl_method(checker, arena, &[extension.generics], method);
        }
    }

    for block in &module.extern_blocks {
        for item in &block.items {
            let mut types: Vec<_> = item.params.iter().map(|param| &param.ty).collect();
            types.push(&item.return_ty);
            validate_types(checker, arena, &[], &types);
        }
    }
}

fn validate_impl_method(
    checker: &mut ModuleChecker<'_>,
    arena: &ExprArena,
    outer_generics: &[GenericParamRange],
    method: &ImplMethod,
) {
    validate_method_parts(
        checker,
        arena,
        outer_generics,
        method.generics,
        method.params,
        &method.return_ty,
    );
}

fn validate_method_parts(
    checker: &mut ModuleChecker<'_>,
    arena: &ExprArena,
    outer_generics: &[GenericParamRange],
    method_generics: GenericParamRange,
    params: ParamRange,
    return_type: &ParsedType,
) {
    let mut ranges = outer_generics.to_vec();
    ranges.push(method_generics);
    let mut types = generic_declared_types(arena, method_generics);
    append_parameter_types(arena, params, &mut types);
    types.push(return_type);
    validate_types(checker, arena, &ranges, &types);
}

fn generic_declared_types(arena: &ExprArena, range: GenericParamRange) -> Vec<&ParsedType> {
    arena
        .get_generic_params(range)
        .iter()
        .flat_map(|parameter| {
            parameter
                .default_type
                .iter()
                .chain(parameter.const_type.iter())
        })
        .collect()
}

fn append_parameter_types<'a>(
    arena: &'a ExprArena,
    range: ParamRange,
    types: &mut Vec<&'a ParsedType>,
) {
    types.extend(
        arena
            .get_params(range)
            .iter()
            .filter_map(|parameter| parameter.ty.as_ref()),
    );
}

fn validate_types(
    checker: &mut ModuleChecker<'_>,
    arena: &ExprArena,
    generic_ranges: &[GenericParamRange],
    types: &[&ParsedType],
) {
    let const_params: Vec<_> = generic_ranges
        .iter()
        .flat_map(|&range| arena.get_generic_params(range))
        .filter(|parameter| parameter.is_const)
        .map(|parameter| (parameter.name, resolve_const_param_type(checker, parameter)))
        .collect();

    let mut engine = checker.create_engine();
    for (name, ty) in const_params {
        engine.bind_const_param(name, ty);
    }
    for parsed in types {
        validate_fixed_list_capacities(&mut engine, arena, parsed);
    }
    let errors = engine.take_errors();
    drop(engine);
    for error in errors {
        checker.push_error(error);
    }
}
