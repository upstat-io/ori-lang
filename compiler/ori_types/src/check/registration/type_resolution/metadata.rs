//! Registration metadata derived from parsed generic and where-clause syntax.

use ori_ir::{ExprArena, GenericParamRange, Name, ParsedType, WhereClause};
use rustc_hash::FxHashMap;

use crate::{GenericConstExpr, GenericParamMeta, Idx, ModuleChecker, WhereConstraint};

use super::resolve_type_with_method_generics_from;

pub(in super::super) fn parsed_type_contains_self(arena: &ExprArena, ty: &ParsedType) -> bool {
    match ty {
        ParsedType::SelfType => true,
        ParsedType::Primitive(_) | ParsedType::Infer | ParsedType::ConstExpr(_) => false,
        ParsedType::Named { type_args, .. } => arena
            .get_parsed_type_list(*type_args)
            .iter()
            .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id))),
        ParsedType::List(element) | ParsedType::FixedList { elem: element, .. } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*element))
        }
        ParsedType::Map { key, value } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*key))
                || parsed_type_contains_self(arena, arena.get_parsed_type(*value))
        }
        ParsedType::Tuple(elements) | ParsedType::TraitBounds(elements) => arena
            .get_parsed_type_list(*elements)
            .iter()
            .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id))),
        ParsedType::Function { params, ret } => {
            arena
                .get_parsed_type_list(*params)
                .iter()
                .any(|&id| parsed_type_contains_self(arena, arena.get_parsed_type(id)))
                || parsed_type_contains_self(arena, arena.get_parsed_type(*ret))
        }
        ParsedType::AssociatedType { base, .. } => {
            parsed_type_contains_self(arena, arena.get_parsed_type(*base))
        }
    }
}

pub(in super::super) fn convert_visibility(visibility: ori_ir::Visibility) -> crate::Visibility {
    match visibility {
        ori_ir::Visibility::Public => crate::Visibility::Public,
        ori_ir::Visibility::Private => crate::Visibility::Private,
    }
}

pub(crate) fn build_where_constraint(
    checker: &mut ModuleChecker<'_>,
    clause: &WhereClause,
    type_params: &[Name],
    scheme_overlay: &FxHashMap<Name, Idx>,
    self_type: Idx,
) -> Option<WhereConstraint> {
    let (parameter, _projection, bounds, _span) = clause.as_type_bound()?;
    let ty = if let Some(&overlay_index) = scheme_overlay.get(&parameter) {
        overlay_index
    } else if type_params.contains(&parameter) {
        checker.pool_mut().named(parameter)
    } else if parameter == checker.interner().intern("Self") {
        self_type
    } else {
        checker.pool_mut().named(parameter)
    };
    let resolved_bounds = bounds
        .iter()
        .map(|bound| checker.pool_mut().named(bound.name()))
        .collect();
    Some(WhereConstraint {
        ty,
        bounds: resolved_bounds,
    })
}

pub(crate) fn build_method_generic_metadata(
    checker: &mut ModuleChecker<'_>,
    generics: GenericParamRange,
    where_clauses: &[WhereClause],
    outer_type_params: &[Name],
    self_type: Idx,
) -> (
    Vec<u32>,
    FxHashMap<Name, Idx>,
    Vec<GenericParamMeta>,
    Vec<WhereConstraint>,
) {
    let generic_params = checker.arena().get_generic_params(generics).to_vec();
    build_method_generic_metadata_from(
        checker,
        &generic_params,
        where_clauses,
        outer_type_params,
        self_type,
        checker.arena(),
    )
}

pub(crate) fn build_method_generic_metadata_from(
    checker: &mut ModuleChecker<'_>,
    generic_params: &[ori_ir::GenericParam],
    where_clauses: &[WhereClause],
    outer_type_params: &[Name],
    self_type: Idx,
    arena: &ExprArena,
) -> (
    Vec<u32>,
    FxHashMap<Name, Idx>,
    Vec<GenericParamMeta>,
    Vec<WhereConstraint>,
) {
    let method_type_param_names: Vec<Name> = generic_params
        .iter()
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| parameter.name)
        .collect();
    let combined_scope: Vec<Name> = outer_type_params
        .iter()
        .copied()
        .chain(method_type_param_names)
        .collect();
    let mut scheme_var_ids = Vec::new();
    let mut scheme_overlay = FxHashMap::default();
    let mut param_meta = Vec::with_capacity(generic_params.len());

    for parameter in generic_params {
        let bounds = parameter
            .bounds
            .iter()
            .map(|bound| checker.pool_mut().named(bound.name()))
            .collect();
        let empty_overlay = FxHashMap::default();
        let default_type = parameter.default_type.as_ref().map(|default| {
            resolve_type_with_method_generics_from(
                checker,
                default,
                &empty_overlay,
                &combined_scope,
                self_type,
                arena,
            )
        });
        let const_type = parameter.const_type.as_ref().map(|const_type| {
            resolve_type_with_method_generics_from(
                checker,
                const_type,
                &empty_overlay,
                &combined_scope,
                self_type,
                arena,
            )
        });
        if !parameter.is_const {
            let variable = checker.pool_mut().fresh_named_var(parameter.name);
            scheme_var_ids.push(checker.pool().data(variable));
            scheme_overlay.insert(parameter.name, variable);
        }
        param_meta.push(GenericParamMeta {
            name: parameter.name,
            is_const: parameter.is_const,
            bounds,
            default_type,
            const_type,
            const_default_value: parameter
                .default_value
                .and_then(|expr| GenericConstExpr::from_arena(arena, expr).ok()),
            projection_bounds: Vec::new(),
        });
    }

    let where_metadata = where_clauses
        .iter()
        .filter_map(|clause| {
            build_where_constraint(checker, clause, &combined_scope, &scheme_overlay, self_type)
        })
        .collect();
    (scheme_var_ids, scheme_overlay, param_meta, where_metadata)
}
