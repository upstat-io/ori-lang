//! Format-spec and interpolation validation for template literals.
//!
//! Home for the two checks previously inlined in the `infer/expr/mod.rs`
//! dispatch module:
//!
//! - `check_interpolation_printable` — validates `{expr}` interpolation
//!   arguments implement `Printable` (`E2038`).
//! - `validate_format_spec` — parses and validates `{expr:spec}` format
//!   specifiers against the expression's inferred type (`E2034` / `E2035`).
//!
//! Relocated here to keep `infer/expr/mod.rs` a routing-only dispatch per
//! `impl-hygiene.md §Side-Logic Rule`.

use ori_ir::{Name, Span};

use super::super::InferEngine;
use crate::{Idx, Tag, TypeCheckError};

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
