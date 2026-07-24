//! Format-spec and interpolation validation for template literals.
//!
//! - `infer_template_literal` — iterates template parts, recursively infers
//!   each interpolant, and routes to the validator that matches the part's
//!   format-spec presence. Returns `Idx::STR`.
//! - `check_interpolation_printable` — validates a `{expr}` interpolation
//!   argument implements `Printable` (`E2038`).
//! - `check_interpolation_formattable` — validates a `{expr:spec}`
//!   interpolation argument is `Formattable` (`E2038`).
//! - `validate_format_spec` — parses and validates a `{expr:spec}` format
//!   specifier against the expression's inferred type (`E2034` / `E2035`).

use ori_ir::{ExprArena, Name, Span, TemplatePartRange};

use crate::{Idx, Tag, TypeCheckError};

use super::super::InferEngine;
use super::infer_expr;

/// Infer the type of a template literal expression.
///
/// Iterates template parts, recursively infers each interpolated expression,
/// and routes to the validators that match whether a format spec is present
/// (`{expr}` → [`check_interpolation_printable`]; `{expr:spec}` →
/// [`check_interpolation_formattable`] + [`validate_format_spec`]). Returns
/// `Idx::STR` — template literals always have type `str`.
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
            // {expr:spec} — requires Formattable (E2038), AND validates the spec
            // string (E2034/E2035). The capability check mirrors the desugar union
            // in ori_canon (primitive | explicit Formattable impl | Printable via
            // the blanket `impl<T: Printable> T: Formattable`): a type that is
            // neither primitive, Formattable, nor Printable must be rejected, just
            // as the no-spec form rejects a non-Printable type.
            // Spec: Clause 14 (string interpolation), Clause 9 (Printable/Formattable).
            check_interpolation_formattable(engine, part_ty, span);
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

    // A method-level RigidVar bound by `T: Printable` satisfies Printable by
    // assumption — body-internal trait dispatch on the binder treats it as
    // Printable without requiring a registry impl. The check runs before
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

/// Validate that an interpolated expression's type can be formatted with a
/// format spec (E2038) — the `{expr:spec}` capability requirement.
///
/// Mirrors `check_interpolation_printable`, but accepts the full union the
/// `ori_canon` desugar handles: primitive, an explicit `impl T: Formattable`, OR
/// `Printable` (which becomes `Formattable` via the blanket
/// `impl<T: Printable> T: Formattable`). A type that is none of these is
/// rejected with E2038, just as the no-spec form rejects a non-`Printable` type.
pub(crate) fn check_interpolation_formattable(
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

    // A method-level RigidVar bound by `Printable` or `Formattable` satisfies the
    // requirement by assumption (parallels the `check_interpolation_printable`
    // RigidVar path; runs before WellKnownNames / TraitRegistry queries).
    if let Some((p_name, f_name)) = engine.well_known().map(|wk| (wk.printable, wk.formattable)) {
        if engine.rigid_var_satisfies_bound(resolved, p_name)
            || engine.rigid_var_satisfies_bound(resolved, f_name)
        {
            return;
        }
    }

    // WellKnownNames (primitives + compound types): primitives are `Printable`,
    // hence `Formattable` via the blanket impl.
    let satisfies_via_wellknown = engine
        .well_known()
        .is_some_and(|wk| wk.type_satisfies_trait(resolved, wk.printable, engine.pool()));
    if satisfies_via_wellknown {
        return;
    }

    // User-defined types: accept a `Printable` impl OR an explicit `Formattable`
    // impl (the desugar's branch-2 path).
    let has_impl = {
        let names = engine.well_known().map(|wk| (wk.printable, wk.formattable));
        if let Some((printable_name, formattable_name)) = names {
            let printable_idx = engine.pool_mut().named(printable_name);
            let formattable_idx = engine.pool_mut().named(formattable_name);
            engine.trait_registry().is_some_and(|reg| {
                reg.has_impl(printable_idx, resolved) || reg.has_impl(formattable_idx, resolved)
            })
        } else {
            // No well-known cache — skip check (isolated test context).
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
    use ori_format::parse_format_spec;

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
