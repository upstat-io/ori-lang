//! Extension-method registration.

use ori_ir::{ExprArena, Name, ParsedType};
use rustc_hash::FxHashMap;

use super::super::type_resolution::{
    build_where_constraint, collect_generic_param_bounds, collect_generic_params,
    resolve_type_with_self,
};
use super::methods::build_impl_method;
use crate::check::bodies::allocate_rigid_var_map_for_names;
use crate::{ImplEntry, ImplSpecificity, ModuleChecker, WhereConstraint};

/// Register every source extension as a distinct, lowest-precedence method
/// provider. The owner index is disjoint from parsed impl blocks and remains
/// stable through `ImplMethodId` into monomorphization and realization.
pub fn register_extensions(checker: &mut ModuleChecker<'_>, module: &ori_ir::Module) {
    // Extension dispatch now uses producer-qualified registry entries. Keep the
    // legacy diagnostic-only mask empty so a missed or invalid extension cannot
    // suppress `UnknownMethod`.
    checker.set_builtin_extensions(FxHashMap::default());
    for (extension_index, extension) in module.extends.iter().enumerate() {
        let owner_index = module.impls.len() + extension_index;
        register_extension(checker, extension, owner_index);
    }
}

fn register_extension(
    checker: &mut ModuleChecker<'_>,
    extension: &ori_ir::ExtendDef,
    owner_index: usize,
) {
    let type_params = extension_type_params(checker, extension);
    let rigid_map = allocate_rigid_var_map_for_names(checker, &type_params);
    checker.push_impl_rigid_var_map(rigid_map);

    let arena = checker.arena();
    let self_type = resolve_type_with_self(
        checker,
        &extension.target_ty,
        &type_params,
        crate::Idx::ERROR,
    );
    let empty_substitutions = FxHashMap::default();
    let self_kw = checker.well_known().self_kw;
    let methods = extension
        .methods
        .iter()
        .filter(|method| extension_method_has_self(arena, method, self_kw))
        .map(|method| {
            (
                method.name,
                build_impl_method(
                    checker,
                    method,
                    &type_params,
                    self_type,
                    &empty_substitutions,
                ),
            )
        })
        .collect();

    let empty_overlay = FxHashMap::default();
    let where_clause: Vec<WhereConstraint> = extension
        .where_clauses
        .iter()
        .filter_map(|constraint| {
            build_where_constraint(checker, constraint, &type_params, &empty_overlay, self_type)
        })
        .collect();
    let type_param_bounds = if extension.generics.is_empty() {
        vec![Vec::new(); type_params.len()]
    } else {
        collect_generic_param_bounds(arena, extension.generics)
    };
    let has_inline_bound = type_param_bounds.iter().any(|bounds| !bounds.is_empty());
    let specificity = if type_params.is_empty() {
        ImplSpecificity::Concrete
    } else if has_inline_bound || !where_clause.is_empty() {
        ImplSpecificity::Constrained
    } else {
        ImplSpecificity::Generic
    };

    checker.trait_registry_mut().register_impl_with_origin(
        ImplEntry {
            trait_idx: None,
            trait_type_args: Vec::new(),
            self_type,
            type_params,
            type_param_bounds,
            methods,
            assoc_types: FxHashMap::default(),
            where_clause,
            specificity,
            span: extension.span,
        },
        Some(crate::registry::RegisteredImplOrigin::Extension {
            owner_index,
            target_name: extension.target_type_name,
        }),
    );
}

pub(crate) fn extension_method_has_self(
    arena: &ExprArena,
    method: &ori_ir::ImplMethod,
    self_kw: Name,
) -> bool {
    arena
        .get_params(method.params)
        .first()
        .is_some_and(|param| param.name == self_kw)
}

/// Collect extension-level type binders. The where-clause form in the spec
/// introduces list element binders through the target (`extend [T] where ...`),
/// while angle-bracket declarations use the ordinary generic range.
pub(crate) fn extension_type_params(
    checker: &ModuleChecker<'_>,
    extension: &ori_ir::ExtendDef,
) -> Vec<Name> {
    if !extension.generics.is_empty() {
        return collect_generic_params(checker.arena(), extension.generics);
    }

    let mut names = Vec::new();
    collect_implicit_target_params(
        checker,
        checker.arena(),
        &extension.target_ty,
        true,
        &mut names,
    );
    names
}

fn collect_implicit_target_params(
    checker: &ModuleChecker<'_>,
    arena: &ExprArena,
    parsed: &ParsedType,
    is_target_root: bool,
    names: &mut Vec<Name>,
) {
    match parsed {
        ParsedType::Named { name, type_args } => {
            if !is_target_root
                && type_args.is_empty()
                && checker.type_registry().get_by_name(*name).is_none()
                && checker.resolve_registration_primitive(*name).is_none()
                && !names.contains(name)
            {
                names.push(*name);
            }
            for &argument in arena.get_parsed_type_list(*type_args) {
                collect_implicit_target_params(
                    checker,
                    arena,
                    arena.get_parsed_type(argument),
                    false,
                    names,
                );
            }
        }
        ParsedType::List(element) | ParsedType::FixedList { elem: element, .. } => {
            collect_implicit_target_params(
                checker,
                arena,
                arena.get_parsed_type(*element),
                false,
                names,
            );
        }
        ParsedType::Map { key, value } => {
            for id in [*key, *value] {
                collect_implicit_target_params(
                    checker,
                    arena,
                    arena.get_parsed_type(id),
                    false,
                    names,
                );
            }
        }
        ParsedType::Tuple(elements) | ParsedType::TraitBounds(elements) => {
            for &element in arena.get_parsed_type_list(*elements) {
                collect_implicit_target_params(
                    checker,
                    arena,
                    arena.get_parsed_type(element),
                    false,
                    names,
                );
            }
        }
        ParsedType::Function { params, ret } => {
            for &parameter in arena.get_parsed_type_list(*params) {
                collect_implicit_target_params(
                    checker,
                    arena,
                    arena.get_parsed_type(parameter),
                    false,
                    names,
                );
            }
            collect_implicit_target_params(
                checker,
                arena,
                arena.get_parsed_type(*ret),
                false,
                names,
            );
        }
        ParsedType::AssociatedType { base, .. } => collect_implicit_target_params(
            checker,
            arena,
            arena.get_parsed_type(*base),
            false,
            names,
        ),
        ParsedType::Primitive(_)
        | ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::ConstExpr(_) => {}
    }
}
