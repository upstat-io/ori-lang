//! Binary-operator support rules — operator-to-trait mapping, cross-type
//! arithmetic, and trait dispatch.

use ori_ir::{BinaryOp, ExprArena, ExprId, Name, Span};

use super::super::super::InferEngine;
use super::super::registry_bridge::is_binary_op_supported;
use crate::{
    ContextKind, Expected, ExpectedOrigin, Idx, MethodLookup, MethodLookupResult, Pool, Tag,
};

/// Map a binary operator to its trait method name.
///
/// Delegates to `BinaryOp::trait_method_name()` — the single source of truth in `ori_ir`.
fn binary_op_to_method_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_method_name()
}

/// Map a binary operator to its trait name (for error messages).
///
/// Delegates to `BinaryOp::trait_name()` — the single source of truth in `ori_ir`.
pub(super) fn binary_op_to_trait_name(op: BinaryOp) -> Option<&'static str> {
    op.trait_name()
}

/// Map a comparison operator to the trait name for error messages.
pub(super) fn comparison_trait_name(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Eq | BinaryOp::NotEq => "Eq",
        BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => "Comparable",
        _ => unreachable!("comparison_trait_name called with non-comparison op"),
    }
}

/// Check for cross-type arithmetic special cases.
///
/// Handles mixed-type arithmetic that the per-type registry can't express:
/// - `Duration * int`, `int * Duration`, `Duration / int`, `Duration div int`
/// - `Size * int`, `int * Size`, `Size / int`, `Size div int`
///
/// These are cross-type rules where the result type differs from at least one
/// operand type. The registry validates whether each individual type supports
/// the operator; this function handles the cross-type pairing.
///
/// Returns `Some(result_idx)` if a cross-type rule matched, `None` otherwise.
pub(super) fn check_cross_type_arithmetic(
    left_tag: Tag,
    right_tag: Tag,
    op: BinaryOp,
) -> Option<Idx> {
    // Validate that the operator is supported for the non-int side via the
    // registry. This prevents accepting e.g. Duration @ int (MatMul).
    let (unit_tag, unit_idx) = match (left_tag, right_tag) {
        (Tag::Duration, Tag::Int) | (Tag::Int, Tag::Duration) => (Tag::Duration, Idx::DURATION),
        (Tag::Size, Tag::Int) | (Tag::Int, Tag::Size) => (Tag::Size, Idx::SIZE),
        _ => return None,
    };

    // Only specific operators are valid for cross-type arithmetic.
    // Duration/Size: mul (both directions), div and floor_div (unit / int only).
    let unit_is_left = left_tag == unit_tag;
    match op {
        BinaryOp::Mul => Some(unit_idx),
        BinaryOp::Div | BinaryOp::FloorDiv if unit_is_left => {
            // Duration/Size supports div: check registry.
            is_binary_op_supported(unit_tag, op)
                .filter(|&supported| supported)
                .map(|_| unit_idx)
        }
        _ => None,
    }
}

/// Extract the nominal base name of a receiver type.
///
/// `Tag::Applied(Name, [args])` and `Tag::Named(Name)` carry a nominal head
/// name. Other tags (primitives, function types, vars) have no nominal base.
fn base_name(pool: &Pool, ty: Idx) -> Option<Name> {
    match pool.tag(ty) {
        Tag::Applied => Some(pool.applied_name(ty)),
        Tag::Named => Some(pool.named_name(ty)),
        _ => None,
    }
}

/// Check if a user nominal type implements a specific trait, identified by the
/// trait's name and its canonical method name.
///
/// Two-tier lookup, matching `infer/expr/calls/impl_lookup.rs` dispatch:
/// 1. Exact-`Idx` `lookup_method_checked` — handles non-generic `Named` types
///    and inherent-shadows-trait correctly.
/// 2. Base-name impl scan — covers generic impls (`impl<T> Pair<T>: Trait`)
///    whose `self_type = Applied(Name, [Named(T)])` does not exact-match a
///    concrete receiver `Applied(Name, [int])`.
///
/// Verifying the trait by name prevents an unrelated `trait Weird { @compare }`
/// (or `@eq`) from bypassing the operator gate.
fn type_implements_named_trait(
    engine: &InferEngine<'_>,
    ty: Idx,
    method_name_str: &str,
    trait_name_str: &str,
) -> bool {
    let Some(method_name) = engine.intern_name(method_name_str) else {
        return false;
    };
    let Some(trait_name) = engine.intern_name(trait_name_str) else {
        return false;
    };
    let Some(trait_registry) = engine.trait_registry() else {
        return false;
    };
    let pool = engine.pool();

    let is_named_trait = |trait_idx: Idx| -> bool {
        pool.tag(trait_idx) == Tag::Named && pool.named_name(trait_idx) == trait_name
    };

    // Tier 1: exact-Idx checked lookup (non-generic Named + inherent shadowing).
    if let MethodLookupResult::Found(MethodLookup::Trait { trait_idx, .. }) =
        trait_registry.lookup_method_checked(ty, method_name)
    {
        if is_named_trait(trait_idx) {
            return true;
        }
    }

    // Tier 2: base-name impl scan for generic instantiations.
    let Some(base) = base_name(pool, ty) else {
        return false;
    };
    trait_registry.impls_iter().any(|(_, entry)| {
        entry.trait_idx.is_some_and(is_named_trait)
            && base_name(pool, entry.self_type) == Some(base)
            && entry.methods.contains_key(&method_name)
    })
}

/// Check if a user-defined type implements the Comparable trait specifically.
/// Recognizes non-generic and generic (`impl<T> T: Comparable`) impls.
pub(crate) fn has_comparable_trait(engine: &InferEngine<'_>, ty: Idx) -> bool {
    type_implements_named_trait(engine, ty, "compare", "Comparable")
}

/// Check if a user-defined type implements the Eq trait specifically.
///
/// Recognizes `#derive(Eq)`, manual `impl T: Eq`, and generic
/// (`impl<T: Eq> Pair<T>: Eq`) impls. Verifying the trait by name prevents an
/// unrelated `eq` method from bypassing the equality-operator gate.
/// Spec: 14-expressions.md "Equality" — operands shall implement `Eq`.
pub(crate) fn has_eq_trait(engine: &InferEngine<'_>, ty: Idx) -> bool {
    type_implements_named_trait(engine, ty, "eq", "Eq")
}

/// Try to resolve a binary operator via trait dispatch.
///
/// Looks up the operator's method name in the `TraitRegistry` for the left
/// operand's type. If found, checks the right operand against the method's
/// parameter type and returns the method's return type.
pub(super) fn resolve_binary_op_via_trait(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    receiver_ty: Idx,
    right_ty: Idx,
    right: ExprId,
    op: BinaryOp,
    span: Span,
) -> Option<Idx> {
    let method_name = binary_op_to_method_name(op)?;
    let op_str = op.as_symbol();
    let name = engine.intern_name(method_name)?;

    // Scoped borrow: extract signature and self-ness, then release the registry borrow.
    let (sig_ty, has_self) = {
        let trait_registry = engine.trait_registry()?;
        let lookup = trait_registry.lookup_method(receiver_ty, name)?;
        (lookup.method().signature, lookup.method().has_self)
    };

    let resolved_sig = engine.resolve(sig_ty);
    if engine.pool().tag(resolved_sig) != Tag::Function {
        return Some(Idx::ERROR);
    }

    let params = engine.pool().function_params(resolved_sig);
    let ret = engine.pool().function_return(resolved_sig);

    // Skip `self` parameter for instance methods
    let skip = usize::from(has_self);
    let method_params = &params[skip..];

    // Binary operators expect exactly one non-self parameter
    if method_params.len() != 1 {
        return Some(Idx::ERROR);
    }

    // Check right operand against the method's parameter type
    let expected = Expected {
        ty: method_params[0],
        origin: ExpectedOrigin::Context {
            span,
            kind: ContextKind::BinaryOpRight { op: op_str },
        },
    };
    let _ = engine.check_type(right_ty, &expected, arena.get_expr(right).span);

    Some(ret)
}
