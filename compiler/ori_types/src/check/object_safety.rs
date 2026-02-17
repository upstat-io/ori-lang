//! Generic object safety checking for parsed type annotations.
//!
//! Two phases need to check `ParsedType` trees for non-object-safe trait usage:
//! - Signature collection (`check/signatures`) — function parameter/return types
//! - Inference (`infer/expr/type_resolution`) — let bindings, lambdas, casts
//!
//! Both walk the same tree structure with the same logic. This module provides
//! a single generic traversal parameterized by [`ObjectSafetyChecker`], which
//! `ModuleChecker` and `InferEngine` each implement.

use ori_ir::{ExprArena, Name, ParsedType, Span};

use super::well_known;
use crate::{ObjectSafetyViolation, TypeCheckError};

/// Trait for contexts that can check object safety of trait names.
///
/// Abstracts the two checking contexts (signature collection vs inference)
/// so the recursive `ParsedType` traversal is written once.
pub(crate) trait ObjectSafetyChecker {
    /// Check if `name` with the given arg count resolves to a concrete well-known
    /// type (e.g., `Iterator<T>`) rather than a trait object.
    fn is_well_known_concrete(&self, name: Name, num_args: usize) -> bool;

    /// If `name` is a non-object-safe trait, emit an E2024 error at `span`.
    fn check_and_emit(&mut self, name: Name, span: Span);
}

/// Check a parsed type annotation for non-object-safe trait usage (E2024).
///
/// Walks the `ParsedType` tree and emits errors when a trait used as a type
/// (trait object) violates object safety rules. Two patterns are checked:
///
/// - `ParsedType::Named { name }` where `name` resolves to a registered trait
/// - `ParsedType::TraitBounds(bounds)` where each bound is checked individually
pub(crate) fn check_parsed_type_object_safety<C: ObjectSafetyChecker>(
    ctx: &mut C,
    parsed: &ParsedType,
    span: Span,
    arena: &ExprArena,
) {
    match parsed {
        ParsedType::Named { name, type_args } => {
            let type_arg_ids = arena.get_parsed_type_list(*type_args);

            // Well-known concrete types (Iterator<T>, etc.) have dedicated Pool
            // constructors and are NOT trait objects. Skip object safety check.
            if !ctx.is_well_known_concrete(*name, type_arg_ids.len()) {
                ctx.check_and_emit(*name, span);
            }

            // Recurse into type arguments (e.g., `[Clone]` has Clone inside List)
            for &arg_id in type_arg_ids {
                let arg = arena.get_parsed_type(arg_id);
                check_parsed_type_object_safety(ctx, arg, span, arena);
            }
        }

        ParsedType::TraitBounds(bounds) => {
            let bound_ids = arena.get_parsed_type_list(*bounds);
            for &bound_id in bound_ids {
                let bound = arena.get_parsed_type(bound_id);
                check_parsed_type_object_safety(ctx, bound, span, arena);
            }
        }

        // Recurse into compound types that may contain trait objects
        ParsedType::List(elem_id) | ParsedType::FixedList { elem: elem_id, .. } => {
            let elem = arena.get_parsed_type(*elem_id);
            check_parsed_type_object_safety(ctx, elem, span, arena);
        }
        ParsedType::Map { key, value } => {
            let key_parsed = arena.get_parsed_type(*key);
            let value_parsed = arena.get_parsed_type(*value);
            check_parsed_type_object_safety(ctx, key_parsed, span, arena);
            check_parsed_type_object_safety(ctx, value_parsed, span, arena);
        }
        ParsedType::Tuple(elems) => {
            let elem_ids = arena.get_parsed_type_list(*elems);
            for &elem_id in elem_ids {
                let elem = arena.get_parsed_type(elem_id);
                check_parsed_type_object_safety(ctx, elem, span, arena);
            }
        }
        ParsedType::Function { params, ret } => {
            let param_ids = arena.get_parsed_type_list(*params);
            for &param_id in param_ids {
                let param = arena.get_parsed_type(param_id);
                check_parsed_type_object_safety(ctx, param, span, arena);
            }
            let ret_parsed = arena.get_parsed_type(*ret);
            check_parsed_type_object_safety(ctx, ret_parsed, span, arena);
        }
        ParsedType::AssociatedType { base, .. } => {
            let base_parsed = arena.get_parsed_type(*base);
            check_parsed_type_object_safety(ctx, base_parsed, span, arena);
        }

        // Leaf types: no trait object usage possible
        ParsedType::Primitive(_)
        | ParsedType::Infer
        | ParsedType::SelfType
        | ParsedType::ConstExpr(_) => {}
    }
}

// ============================================================================
// ModuleChecker implementation
// ============================================================================

impl ObjectSafetyChecker for super::ModuleChecker<'_> {
    fn is_well_known_concrete(&self, name: Name, num_args: usize) -> bool {
        let name_str = self.interner().lookup(name);
        well_known::is_concrete_named_type(name_str, num_args)
    }

    fn check_and_emit(&mut self, name: Name, span: Span) {
        // Borrow dance: scope the trait_registry borrow to extract violations,
        // then use self mutably to push the error.
        let violations: Option<Vec<ObjectSafetyViolation>> = {
            let trait_reg = self.trait_registry();
            trait_reg
                .get_trait_by_name(name)
                .filter(|entry| !entry.is_object_safe())
                .map(|entry| entry.object_safety_violations.clone())
        };
        if let Some(violations) = violations {
            self.push_error(TypeCheckError::not_object_safe(span, name, violations));
        }
    }
}
