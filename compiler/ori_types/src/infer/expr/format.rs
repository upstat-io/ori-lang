//! Format-spec and interpolation validation for template literals.
//!
//! Home for the template-literal inference entry point plus its two
//! validators, all previously inlined in the `infer/expr/mod.rs` dispatch
//! module:
//!
//! - `infer_template_literal` — iterates template parts, recursively infers
//!   each interpolant, and routes to the validator that matches the part's
//!   format-spec presence. Returns `Idx::STR`.
//! - `check_interpolation_printable` — validates `{expr}` interpolation
//!   arguments implement `Printable` (`E2038`).
//! - `validate_format_spec` — parses and validates `{expr:spec}` format
//!   specifiers against the expression's inferred type (`E2034` / `E2035`).
//!
//! Relocated here to keep `infer/expr/mod.rs` a routing-only dispatch per
//!.

use ori_ir::{ExprArena, Name, Span, TemplatePartRange};

use super::super::InferEngine;
use super::infer_expr;
use crate::{Idx, Tag, TypeCheckError};

/// Infer the type of a template literal expression.
///
/// Iterates template parts, recursively infers each interpolated expression,
/// and routes to the appropriate validator based on whether a format spec is
/// present (`{expr}` → [`check_interpolation_printable`], `{expr:spec}` →
/// [`validate_format_spec`]). Returns `Idx::STR` — template literals always
/// have type `str`.
pub(crate) fn infer_template_literal(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    parts: TemplatePartRange,
    span: Span,
) -> Idx {
    for part in arena.get_template_parts(parts) {
        let part_ty = infer_expr(engine, arena, part.expr);

        if part.format_spec == Name::EMPTY {
            // {expr} — requires Printable for to_str() conversion (E2038)
            check_interpolation_printable(engine, part_ty, span);
        } else {
            // {expr:spec} — validate format spec (E2034/E2035)
            // Formattable requirement is implied; Printable not needed
            validate_format_spec(engine, part.format_spec, part_ty, span);
        }
    }
    Idx::STR
}

/// Validate that an interpolated expression's type implements `Printable` (E2038).
///
/// Follows the `check_map_key_hashable` pattern: resolve type, skip variables/errors,
/// check primitives + compound types via `WellKnownNames`, then check user types
/// via `TraitRegistry`.
pub(crate) fn check_interpolation_printable(
    engine: &mut InferEngine<'_>,
    expr_type: Idx,
    span: Span,
) {
    let resolved = engine.resolve(expr_type);
    let tag = engine.pool().tag(resolved);

    // Skip unresolved variables, error sentinels, and Never (coerces to anything)
    if matches!(tag, Tag::Var | Tag::Infer | Tag::Never) || resolved == Idx::ERROR {
        return;
    }

    // Phase B-Residual-2 (c): a method-level RigidVar with `T: Printable`
    // declared inline (or via where-clause once threaded) satisfies Printable
    // by assumption — body-internal trait dispatch on the binder treats it
    // as Printable without requiring a registry impl. The check runs before
    // WellKnownNames / TraitRegistry queries because RigidVars never satisfy
    // either of those paths.
    if let Some(p_name) = engine.well_known().map(|wk| wk.printable) {
        if engine.rigid_var_satisfies_bound(resolved, p_name) {
            return;
        }
    }

    // Check via WellKnownNames (primitives + compound types)
    let satisfies_via_wellknown = {
        engine
            .well_known()
            .is_some_and(|wk| wk.type_satisfies_trait(resolved, wk.printable, engine.pool()))
    };
    if satisfies_via_wellknown {
        return;
    }

    // User-defined types: check trait registry for Printable impl
    let has_impl = {
        let printable_name = engine.well_known().map(|wk| wk.printable);
        if let Some(p_name) = printable_name {
            let printable_idx = engine.pool_mut().named(p_name);
            engine
                .trait_registry()
                .is_some_and(|reg| reg.has_impl(printable_idx, resolved))
        } else {
            // No well-known cache — skip check (isolated test context)
            return;
        }
    };
    if !has_impl {
        engine.push_error(TypeCheckError::missing_printable(span, resolved));
    }
}

/// Validate a format specification against the expression's inferred type.
///
/// Checks:
/// 1. The format spec parses correctly (E2034 if not)
/// 2. The format type is compatible with the expression type (E2035 if not):
///    - `b`, `o`, `x`, `X` require `int`
///    - `e`, `E`, `f`, `%` require `float`
pub(crate) fn validate_format_spec(
    engine: &mut InferEngine<'_>,
    format_spec: Name,
    expr_type: Idx,
    span: Span,
) {
    use ori_ir::format_spec::parse_format_spec;

    let Some(spec_str) = engine.lookup_name(format_spec) else {
        return;
    };

    if spec_str.is_empty() {
        return;
    }

    let parsed = match parse_format_spec(spec_str) {
        Ok(p) => p,
        Err(e) => {
            engine.push_error(TypeCheckError::invalid_format_spec(
                span,
                spec_str.to_owned(),
                e.to_string(),
            ));
            return;
        }
    };

    // Validate format type against expression type
    let Some(fmt_type) = parsed.format_type else {
        return;
    };

    let resolved = engine.resolve(expr_type);
    let tag = engine.pool().tag(resolved);

    if fmt_type.is_integer_only() && !matches!(tag, Tag::Int) {
        engine.push_error(TypeCheckError::format_type_mismatch(
            span,
            resolved,
            fmt_type.name().to_owned(),
            "int",
        ));
    } else if fmt_type.is_float_only() && !matches!(tag, Tag::Float) {
        engine.push_error(TypeCheckError::format_type_mismatch(
            span,
            resolved,
            fmt_type.name().to_owned(),
            "float",
        ));
    }
}
