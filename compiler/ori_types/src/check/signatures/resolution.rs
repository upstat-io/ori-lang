//! Generic-aware signature type resolution and object-safety checks.

use ori_ir::{ExprArena, Name, ParsedType, Span};
use rustc_hash::FxHashMap;

use super::ModuleChecker;
use crate::Idx;

/// Resolve a const parameter's declared type to a pool index.
pub(crate) fn resolve_const_param_type(
    checker: &ModuleChecker<'_>,
    param: &ori_ir::GenericParam,
) -> Idx {
    match &param.const_type {
        Some(ParsedType::Primitive(type_id)) => match type_id.raw() {
            0 => Idx::INT,
            2 => Idx::BOOL,
            _ => Idx::ERROR,
        },
        Some(ParsedType::Named { name, .. }) => {
            let well_known = checker.well_known();
            if *name == well_known.int {
                Idx::INT
            } else if *name == well_known.bool {
                Idx::BOOL
            } else {
                Idx::ERROR
            }
        }
        _ => Idx::ERROR,
    }
}

/// Resolve a parsed type while checking trait-object safety in one walk.
pub(super) fn resolve_and_check_type_with_vars(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_param_vars: &FxHashMap<Name, Idx>,
    span: Span,
    arena: &ExprArena,
) -> Idx {
    match parsed {
        ParsedType::Primitive(type_id) => {
            let raw = type_id.raw();
            if raw < ori_ir::TypeId::PRIMITIVE_COUNT {
                Idx::from_raw(raw)
            } else {
                Idx::ERROR
            }
        }
        ParsedType::List(element_id) => {
            let element = arena.get_parsed_type(*element_id);
            let element_type =
                resolve_and_check_type_with_vars(checker, element, type_param_vars, span, arena);
            checker.pool_mut().list(element_type)
        }
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            let key_type =
                resolve_and_check_type_with_vars(checker, key_parsed, type_param_vars, span, arena);
            let value_type = resolve_and_check_type_with_vars(
                checker,
                value_parsed,
                type_param_vars,
                span,
                arena,
            );
            checker.pool_mut().map(key_type, value_type)
        }
        ParsedType::Tuple(elements) => {
            let element_ids = arena.get_parsed_type_list(*elements);
            let element_types: Vec<Idx> = element_ids
                .iter()
                .map(|&element_id| {
                    resolve_and_check_type_with_vars(
                        checker,
                        arena.get_parsed_type(element_id),
                        type_param_vars,
                        span,
                        arena,
                    )
                })
                .collect();
            checker.pool_mut().tuple(&element_types)
        }
        ParsedType::Function { .. } => {
            resolve_function_type(checker, parsed, type_param_vars, span, arena)
        }
        ParsedType::Named { .. } => {
            resolve_named_type(checker, parsed, type_param_vars, span, arena)
        }
        ParsedType::FixedList { elem, capacity: _ } => {
            let element_type = resolve_and_check_type_with_vars(
                checker,
                arena.get_parsed_type(*elem),
                type_param_vars,
                span,
                arena,
            );
            checker.pool_mut().list(element_type)
        }
        ParsedType::Infer => checker.pool_mut().fresh_var(),
        ParsedType::SelfType => Idx::ERROR,
        ParsedType::AssociatedType { base, .. } => {
            resolve_and_check_type_with_vars(
                checker,
                arena.get_parsed_type(*base),
                type_param_vars,
                span,
                arena,
            );
            Idx::ERROR
        }
        ParsedType::ConstExpr(_) => Idx::INT,
        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            let mut primary = Idx::ERROR;
            for (index, &bound_id) in bound_ids.iter().enumerate() {
                let resolved = resolve_and_check_type_with_vars(
                    checker,
                    arena.get_parsed_type(bound_id),
                    type_param_vars,
                    span,
                    arena,
                );
                if index == 0 {
                    primary = resolved;
                }
            }
            primary
        }
    }
}

fn resolve_function_type(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_param_vars: &FxHashMap<Name, Idx>,
    span: Span,
    arena: &ExprArena,
) -> Idx {
    let ParsedType::Function { params, ret } = parsed else {
        unreachable!("function resolver called for non-function type");
    };
    let parameter_types: Vec<Idx> = arena
        .get_parsed_type_list(*params)
        .iter()
        .map(|&parameter_id| {
            resolve_and_check_type_with_vars(
                checker,
                arena.get_parsed_type(parameter_id),
                type_param_vars,
                span,
                arena,
            )
        })
        .collect();
    let return_type = resolve_and_check_type_with_vars(
        checker,
        arena.get_parsed_type(*ret),
        type_param_vars,
        span,
        arena,
    );
    checker.pool_mut().function(&parameter_types, return_type)
}

fn resolve_named_type(
    checker: &mut ModuleChecker<'_>,
    parsed: &ParsedType,
    type_param_vars: &FxHashMap<Name, Idx>,
    span: Span,
    arena: &ExprArena,
) -> Idx {
    let ParsedType::Named { name, type_args } = parsed else {
        unreachable!("named resolver called for non-named type");
    };
    if let Some(&variable) = type_param_vars.get(name) {
        return variable;
    }
    let type_arg_ids = arena.get_parsed_type_list(*type_args);
    let resolved_args: Vec<Idx> = type_arg_ids
        .iter()
        .map(|&arg_id| {
            resolve_and_check_type_with_vars(
                checker,
                arena.get_parsed_type(arg_id),
                type_param_vars,
                span,
                arena,
            )
        })
        .collect();
    if !checker.is_well_known_concrete_cached(*name, type_arg_ids.len()) {
        emit_object_safety_error(checker, *name, span);
    }
    if !resolved_args.is_empty() {
        if let Some(index) = checker.resolve_well_known_generic_cached(*name, &resolved_args) {
            return index;
        }
        return checker.pool_mut().applied(*name, &resolved_args);
    }
    checker
        .resolve_primitive_name(*name)
        .unwrap_or_else(|| checker.pool_mut().named(*name))
}

fn emit_object_safety_error(checker: &mut ModuleChecker<'_>, name: Name, span: Span) {
    use crate::{ObjectSafetyViolation, TypeCheckError};

    let violations: Option<Vec<ObjectSafetyViolation>> = checker
        .trait_registry()
        .get_trait_by_name(name)
        .filter(|entry| !entry.is_object_safe())
        .map(|entry| entry.object_safety_violations.clone());
    if let Some(violations) = violations {
        checker.push_error(TypeCheckError::not_object_safe(span, name, violations));
    }
}
