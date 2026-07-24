//! Registration-time parsed-type resolution without method overlays.

use ori_ir::{ExprArena, Name, ParsedType, TypeId};

use crate::{Idx, ModuleChecker};

pub(in super::super) fn collect_generic_params(
    arena: &ExprArena,
    generics: ori_ir::GenericParamRange,
) -> Vec<Name> {
    arena
        .get_generic_params(generics)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| parameter.name)
        .collect()
}

pub(in super::super) fn collect_generic_param_bounds(
    arena: &ExprArena,
    generics: ori_ir::GenericParamRange,
) -> Vec<Vec<Name>> {
    arena
        .get_generic_params(generics)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| {
            parameter
                .bounds
                .iter()
                .map(ori_ir::TraitBound::name)
                .collect()
        })
        .collect()
}

pub(in super::super) fn resolve_field_type(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
) -> Idx {
    let arena = checker.arena();
    resolve_parsed_type_simple(checker, parsed, arena)
}

pub(crate) fn resolve_parsed_type_simple(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Primitive(type_id) => {
            let raw = type_id.raw();
            if raw < TypeId::PRIMITIVE_COUNT {
                Idx::from_raw(raw)
            } else {
                Idx::ERROR
            }
        }
        ParsedType::List(element_id) => {
            let element_type =
                resolve_parsed_type_simple(checker, arena.get_parsed_type(*element_id), arena);
            checker.pool_mut().list(element_type)
        }
        ParsedType::Map { key, value } => {
            let key_type = resolve_parsed_type_simple(checker, arena.get_parsed_type(*key), arena);
            let value_type =
                resolve_parsed_type_simple(checker, arena.get_parsed_type(*value), arena);
            checker.pool_mut().map(key_type, value_type)
        }
        ParsedType::Tuple(elements) => {
            let element_types: Vec<Idx> = arena
                .get_parsed_type_list(*elements)
                .iter()
                .map(|&element_id| {
                    resolve_parsed_type_simple(checker, arena.get_parsed_type(element_id), arena)
                })
                .collect();
            checker.pool_mut().tuple(&element_types)
        }
        ParsedType::Function { params, ret } => {
            let parameter_types: Vec<Idx> = arena
                .get_parsed_type_list(*params)
                .iter()
                .map(|&parameter_id| {
                    resolve_parsed_type_simple(checker, arena.get_parsed_type(parameter_id), arena)
                })
                .collect();
            let return_type =
                resolve_parsed_type_simple(checker, arena.get_parsed_type(*ret), arena);
            checker.pool_mut().function(&parameter_types, return_type)
        }
        ParsedType::Named { name, type_args } => {
            let resolved_args: Vec<Idx> = arena
                .get_parsed_type_list(*type_args)
                .iter()
                .map(|&argument_id| {
                    resolve_parsed_type_simple(checker, arena.get_parsed_type(argument_id), arena)
                })
                .collect();
            if !resolved_args.is_empty() {
                if let Some(index) =
                    checker.resolve_well_known_generic_cached(*name, &resolved_args)
                {
                    return index;
                }
                return checker.pool_mut().applied(*name, &resolved_args);
            }
            if let Some(index) = checker.resolve_registration_primitive(*name) {
                return index;
            }
            let named_index = checker.pool_mut().named(*name);
            if let (Some(concrete), Some(kind)) = (
                checker.resolve_ffi_concrete(*name),
                checker.resolve_ffi_cabi_kind(*name),
            ) {
                checker
                    .pool_mut()
                    .attach_ffi_carrier(named_index, concrete, kind);
            }
            named_index
        }
        ParsedType::FixedList { elem, capacity: _ } => {
            let element_type =
                resolve_parsed_type_simple(checker, arena.get_parsed_type(*elem), arena);
            checker.pool_mut().list(element_type)
        }
        ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::AssociatedType { .. }
        | ParsedType::ConstExpr(_) => Idx::ERROR,
        ParsedType::TraitBounds(bounds) => arena
            .get_parsed_type_list(*bounds)
            .first()
            .map_or(Idx::ERROR, |&first_id| {
                resolve_parsed_type_simple(checker, arena.get_parsed_type(first_id), arena)
            }),
    }
}

pub(in super::super) fn resolve_type_with_params(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_params: &[Name],
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Named { name, .. } if type_params.contains(name) => {
            checker.pool_mut().named(*name)
        }
        ParsedType::SelfType => {
            let self_name = checker.interner().intern("Self");
            checker.pool_mut().named(self_name)
        }
        ParsedType::List(element_id) => {
            let element_type = resolve_type_with_params(
                checker,
                arena.get_parsed_type(*element_id),
                type_params,
                arena,
            );
            checker.pool_mut().list(element_type)
        }
        ParsedType::Map { key, value } => {
            let key_type =
                resolve_type_with_params(checker, arena.get_parsed_type(*key), type_params, arena);
            let value_type = resolve_type_with_params(
                checker,
                arena.get_parsed_type(*value),
                type_params,
                arena,
            );
            checker.pool_mut().map(key_type, value_type)
        }
        ParsedType::Tuple(elements) => {
            let element_types: Vec<Idx> = arena
                .get_parsed_type_list(*elements)
                .iter()
                .map(|&element_id| {
                    resolve_type_with_params(
                        checker,
                        arena.get_parsed_type(element_id),
                        type_params,
                        arena,
                    )
                })
                .collect();
            checker.pool_mut().tuple(&element_types)
        }
        ParsedType::Function { params, ret } => {
            let parameter_types: Vec<Idx> = arena
                .get_parsed_type_list(*params)
                .iter()
                .map(|&parameter_id| {
                    resolve_type_with_params(
                        checker,
                        arena.get_parsed_type(parameter_id),
                        type_params,
                        arena,
                    )
                })
                .collect();
            let return_type =
                resolve_type_with_params(checker, arena.get_parsed_type(*ret), type_params, arena);
            checker.pool_mut().function(&parameter_types, return_type)
        }
        ParsedType::TraitBounds(bounds) => {
            arena
                .get_parsed_type_list(*bounds)
                .first()
                .map_or(Idx::ERROR, |&first_id| {
                    resolve_type_with_params(
                        checker,
                        arena.get_parsed_type(first_id),
                        type_params,
                        arena,
                    )
                })
        }
        _ => resolve_parsed_type_simple(checker, parsed, arena),
    }
}
