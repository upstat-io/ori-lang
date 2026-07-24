//! Drop-type partial-move rejection — producer side of `E2048`.
//!
//! Rejects a partial move of a field whose type implements `Drop`: the
//! whole-value bind-all-fields case is the only sound destructure (a partial
//! move leaves `@drop` observing absent fields). Disjoint from `E2043`
//! (`validate_partial_move`, conditional moves on non-Drop aggregates); both
//! share the `direct_projection` / `init_receiver` / `raw_index` move
//! primitives from the `partial_move` sibling.

use ori_ir::{ExprArena, ExprId, ExprKind, MatchPattern, Name, Span};
use rustc_hash::FxHashMap;

use crate::check::validators::expr_children::child_ids;
use crate::tag::Tag;
use crate::TraitRegistry;
use crate::{ExprIndex, Idx, Pool, TypeCheckError};

use super::partial_move::{direct_projection, init_receiver, ExprIdRawIndex};

/// Validate that no `let $f = v.field` binding occurs where `v`'s type
/// implements `Drop`.
///
/// Per `drop-trait-proposal.md §Execution Timing`, the compiler walks
/// owned fields in reverse declaration order AFTER invoking the user
/// `@drop` body. A partial move would leave `@drop` observing absent
/// fields. The rule applies on every CFG path — axis disjoint from
/// `validate_partial_move`'s `E2043` (which only rejects conditional
/// partial moves on non-Drop aggregates).
///
/// The walk shape mirrors `validate_partial_move`'s direct-projection
/// detector but classifies receivers by Drop-impl status via
/// `TraitRegistry::find_impl(drop_trait_idx, receiver_type)`. The Drop
/// trait is resolved by name (`drop_trait_name`), which must be the
/// pre-interned `Drop` name from `WellKnownNames` (or any caller-supplied
/// equivalent — testing convenience).
///
/// Closure-capture sites are bindings too; their initializers route
/// through the same `ExprKind::Let` / `StmtKind::Let` paths as ordinary
/// `let` bindings, so the validator emits E2048 uniformly for both.
pub fn validate_drop_partial_move(
    pool: &Pool,
    arena: &ExprArena,
    expr_types: &FxHashMap<ExprIndex, Idx>,
    trait_registry: &TraitRegistry,
    drop_trait_name: Name,
    body_root: ExprId,
    errors: &mut Vec<TypeCheckError>,
) {
    if body_root == ExprId::INVALID {
        return;
    }
    // Resolve Drop's trait `Idx`. Missing trait = nothing to check;
    // typeck silently no-ops (the prelude has not yet wired `Drop`
    // into the registry, so this is the expected pre-deployment shape).
    let Some(drop_trait) = trait_registry.get_trait_by_name(drop_trait_name) else {
        return;
    };
    let drop_trait_idx = drop_trait.idx;
    let mut ctx = DropWalkCtx {
        pool,
        arena,
        expr_types,
        trait_registry,
        drop_trait_idx,
        errors,
    };
    walk_drop(&mut ctx, body_root);
}

/// Per-validator walk state for E2048.
struct DropWalkCtx<'a> {
    pool: &'a Pool,
    arena: &'a ExprArena,
    expr_types: &'a FxHashMap<ExprIndex, Idx>,
    trait_registry: &'a TraitRegistry,
    drop_trait_idx: Idx,
    errors: &'a mut Vec<TypeCheckError>,
}

/// Walk every expression looking for `let $f = v.field` bindings whose
/// receiver type implements `Drop`. Recurses into compound forms via
/// [`collect_child_ids`] (shared with the E2043 validator).
fn walk_drop(ctx: &mut DropWalkCtx<'_>, expr_id: ExprId) {
    if expr_id == ExprId::INVALID {
        return;
    }
    let kind = *ctx.arena.expr_kind(expr_id);
    // First check this node for direct `let` shapes; then for `match`-arm
    // partial by-value destructures; then recurse.
    check_drop_let_at(ctx, kind);
    check_drop_match_destructure_at(ctx, kind);
    let children = child_ids(ctx.arena, expr_id);
    for c in children {
        walk_drop(ctx, c);
    }
    // `collect_child_ids` does not descend into `FunctionSeq` (the shared
    // E2043 helper skips it as a post-canonicalization surface). The surface
    // `match` / `try` forms ARE `FunctionSeq`, so recurse into their child
    // expressions here to reach nested `let $f = v.field` bindings and nested
    // matches inside arm bodies / try-block statements.
    walk_drop_function_seq_children(ctx, kind);
}

/// Recurse into the child expressions of a `FunctionSeq` (`match` / `try` /
/// `for`-pattern). Shared `collect_child_ids` treats `FunctionSeq` as a leaf,
/// so this validator threads its own descent to reach arm bodies, the
/// scrutinee, and try-block statements.
fn walk_drop_function_seq_children(ctx: &mut DropWalkCtx<'_>, kind: ExprKind) {
    let ExprKind::FunctionSeq(seq_id) = kind else {
        return;
    };
    match *ctx.arena.get_function_seq(seq_id) {
        ori_ir::FunctionSeq::Match {
            scrutinee, arms, ..
        } => {
            walk_drop(ctx, scrutinee);
            let bodies: Vec<ExprId> = ctx
                .arena
                .get_arms(arms)
                .iter()
                .flat_map(|a| [a.guard.unwrap_or(ExprId::INVALID), a.body])
                .collect();
            for b in bodies {
                walk_drop(ctx, b);
            }
        }
        ori_ir::FunctionSeq::Try { stmts, result, .. } => {
            let stmt_inits: Vec<ExprId> = ctx
                .arena
                .get_stmt_range(stmts)
                .iter()
                .map(|stmt| match stmt.kind {
                    ori_ir::StmtKind::Expr(id) | ori_ir::StmtKind::Let { init: id, .. } => id,
                })
                .collect();
            for id in stmt_inits {
                walk_drop(ctx, id);
            }
            walk_drop(ctx, result);
        }
        ori_ir::FunctionSeq::ForPattern {
            over,
            map,
            ref arm,
            default,
            ..
        } => {
            let arm_guard = arm.guard.unwrap_or(ExprId::INVALID);
            let arm_body = arm.body;
            walk_drop(ctx, over);
            if let Some(m) = map {
                walk_drop(ctx, m);
            }
            walk_drop(ctx, arm_guard);
            walk_drop(ctx, arm_body);
            walk_drop(ctx, default);
        }
    }
}

/// Whole-value-consumption invariant on `match`-destructure of Drop types.
///
/// A `match` arm pattern that by-value-binds a PROPER subset of a Drop
/// type's fields (leaving the residual value live) is a double-free / leak
/// hazard: the bound fields move out (their drops now owned by the new
/// bindings) while the residual still carries the type's `@drop` glue, which
/// would re-drop the moved-out fields. Per `drop-trait-proposal.md
/// §Execution Timing`, the whole-value bind-all-fields case is the ONLY sound
/// destructure of a Drop type — it consumes the value exactly once so `@drop`
/// runs exactly once.
///
/// Detection: when the scrutinee resolves to a Drop type, an arm pattern that
/// binds fewer than all owned fields by-value fires `E2048`. The two
/// proper-subset shapes are (a) the struct rest-pattern `D { a, .. }` where
/// `rest: true` discards residual fields while binding a strict subset, and
/// (b) a struct/variant pattern binding fewer named fields than the
/// type/variant declares. The bind-all-fields case (`D { a, b }` for a 2-field
/// type; a variant pattern binding every payload field) is whole-value
/// consumption — accepted.
fn check_drop_match_destructure_at(ctx: &mut DropWalkCtx<'_>, kind: ExprKind) {
    // The surface `match scrutinee { ... }` form lowers to
    // `ExprKind::FunctionSeq(FunctionSeq::Match { .. })` (the function-sequence
    // construct), NOT `ExprKind::Match`. Resolve scrutinee + arms from the
    // FunctionSeq.
    let ExprKind::FunctionSeq(seq_id) = kind else {
        return;
    };
    let ori_ir::FunctionSeq::Match {
        scrutinee, arms, ..
    } = *ctx.arena.get_function_seq(seq_id)
    else {
        return;
    };
    let Some(&scrutinee_ty) = ctx.expr_types.get(&scrutinee.raw_index()) else {
        return;
    };
    let resolved = ctx.pool.resolve_fully(scrutinee_ty);
    // Top-level Drop name (also gates on a registered `impl T: Drop`). `None`
    // when the scrutinee is not itself a Drop type — the top-level partial-move
    // check is gated on this, but the nested check below runs regardless: a
    // Drop-typed field can be nested inside a non-Drop outer aggregate.
    let top_drop_name = drop_aggregate_type_name(ctx, scrutinee);
    // Snapshot the arms before borrowing ctx mutably for error pushes.
    let arm_patterns: Vec<(MatchPattern, Span)> = ctx
        .arena
        .get_arms(arms)
        .iter()
        .map(|a| (a.pattern.clone(), a.span))
        .collect();
    for (pattern, span) in arm_patterns {
        // Top-level partial by-value destructure of the Drop scrutinee.
        if let Some(type_name) = top_drop_name {
            if let Some(field) = partial_subset_field(ctx, &pattern, resolved) {
                ctx.errors.push(TypeCheckError::drop_partial_move(
                    span, type_name, field, type_name,
                ));
                continue;
            }
        }
        // Nested partial by-value destructure of a Drop-typed field (one or more
        // levels deep), independent of the scrutinee's own Drop status.
        if let Some((drop_name, field)) = nested_drop_partial_move(ctx, &pattern, resolved) {
            ctx.errors.push(TypeCheckError::drop_partial_move(
                span, drop_name, field, drop_name,
            ));
        }
    }
}

/// If `pattern` by-value-binds a PROPER subset of `scrutinee_ty`'s owned
/// fields, return a representative bound field `Name` for the diagnostic;
/// `None` when the pattern binds the whole value (or does not destructure by
/// value at all — `Wildcard`, `Binding`, `Literal`, `Range` all consume the
/// whole value or none of it).
fn partial_subset_field(
    ctx: &DropWalkCtx<'_>,
    pattern: &MatchPattern,
    scrutinee_ty: Idx,
) -> Option<Name> {
    let tag = ctx.pool.tag(scrutinee_ty);
    match pattern {
        // `D { a, .. }` — the rest-pattern discards the residual; binding ANY
        // strict subset is a partial move. `D { a, b }` (rest: false) binding
        // every owned field is whole-value consumption (the residual is empty),
        // so it is accepted below by the field-count comparison.
        MatchPattern::Struct { fields, rest } => {
            if tag != Tag::Struct {
                return None;
            }
            let total = ctx.pool.struct_field_count(scrutinee_ty);
            // `rest: true` always leaves ≥1 field unbound when the bound set is
            // a strict subset; `rest: false` binding fewer than `total` fields
            // is likewise a partial move. Either way, bound < total ⇒ E2048.
            if (*rest || fields.len() < total) && !fields.is_empty() {
                // The residual is non-empty only when fewer than `total`
                // fields are bound; `rest: false` with `fields.len() == total`
                // is whole-value consumption.
                if fields.len() < total {
                    return fields.first().map(|(name, _)| *name);
                }
            }
            None
        }
        // Variant pattern `V(a)` over an enum Drop type: a payload subset bind
        // is a partial move of that variant's payload.
        MatchPattern::Variant { name, inner } => {
            if tag != Tag::Enum {
                return None;
            }
            let payload_count = variant_payload_count(ctx, scrutinee_ty, *name)?;
            let bound = ctx.arena.get_match_pattern_list(*inner).len();
            if bound > 0 && bound < payload_count {
                Some(*name)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Number of payload fields the named variant of `enum_ty` declares, or `None`
/// when the variant is not found.
fn variant_payload_count(ctx: &DropWalkCtx<'_>, enum_ty: Idx, variant: Name) -> Option<usize> {
    let count = ctx.pool.enum_variant_count(enum_ty);
    for i in 0..count {
        let (vname, payload) = ctx.pool.enum_variant(enum_ty, i);
        if vname == variant {
            return Some(payload.len());
        }
    }
    None
}

/// Payload field types the named variant of `enum_ty` declares, or `None` when
/// the variant is not found. Companion to [`variant_payload_count`] for the
/// nested-destructure recursion, which needs the payload TYPES (not just the
/// count) to gate each nested sub-pattern on its own Drop status.
fn variant_payload_types(ctx: &DropWalkCtx<'_>, enum_ty: Idx, variant: Name) -> Option<Vec<Idx>> {
    let count = ctx.pool.enum_variant_count(enum_ty);
    for i in 0..count {
        let (vname, payload) = ctx.pool.enum_variant(enum_ty, i);
        if vname == variant {
            return Some(payload);
        }
    }
    None
}

/// Detect a NESTED partial by-value destructure of a Drop-typed field.
///
/// `pattern` is matched against `ty` (resolved internally). For every bound
/// sub-pattern over a field whose OWN type implements `Drop`, a nested partial
/// subset — or a deeper nested partial move — is the same double-free / leak
/// hazard as the top-level case (drop-trait-proposal.md §Execution Timing): the
/// bound nested field moves out while the residual still owns its `@drop` glue,
/// which would re-drop the moved-out field. Returns `(drop_type_name,
/// offending_field_name)` for the diagnostic, or `None`. Recursion gates each
/// level on the nested field's OWN Drop status, so a partial destructure of a
/// non-Drop nested type does not fire.
fn nested_drop_partial_move(
    ctx: &DropWalkCtx<'_>,
    pattern: &MatchPattern,
    ty: Idx,
) -> Option<(Name, Name)> {
    let resolved = ctx.pool.resolve_fully(ty);
    match pattern {
        MatchPattern::Struct { fields, .. } => {
            if ctx.pool.tag(resolved) != Tag::Struct {
                return None;
            }
            let field_tys = ctx.pool.struct_fields(resolved);
            for (fname, sub) in fields {
                let Some(sub_id) = sub else {
                    continue;
                };
                let Some(field_ty) = field_tys.iter().find(|(n, _)| n == fname).map(|(_, t)| *t)
                else {
                    continue;
                };
                if let Some(hit) = nested_field_hit(ctx, *sub_id, field_ty) {
                    return Some(hit);
                }
            }
            None
        }
        MatchPattern::Variant { name, inner } => {
            if ctx.pool.tag(resolved) != Tag::Enum {
                return None;
            }
            let payload_tys = variant_payload_types(ctx, resolved, *name)?;
            let sub_ids = ctx.arena.get_match_pattern_list(*inner);
            for (i, sub_id) in sub_ids.iter().enumerate() {
                let Some(&field_ty) = payload_tys.get(i) else {
                    continue;
                };
                if let Some(hit) = nested_field_hit(ctx, *sub_id, field_ty) {
                    return Some(hit);
                }
            }
            None
        }
        _ => None,
    }
}

/// Check one bound sub-pattern (`sub_id`) over a field of type `field_ty`.
/// Fires when the field is itself a Drop type AND its sub-pattern binds a
/// proper subset (direct nested partial move); otherwise recurses for a deeper
/// nested Drop field regardless of this field's own Drop status.
fn nested_field_hit(
    ctx: &DropWalkCtx<'_>,
    sub_id: ori_ir::MatchPatternId,
    field_ty: Idx,
) -> Option<(Name, Name)> {
    let sub_pat = ctx.arena.get_match_pattern(sub_id);
    let field_resolved = ctx.pool.resolve_fully(field_ty);
    if let Some(drop_name) = drop_type_name_for_idx(ctx, field_ty) {
        if let Some(field) = partial_subset_field(ctx, sub_pat, field_resolved) {
            return Some((drop_name, field));
        }
    }
    nested_drop_partial_move(ctx, sub_pat, field_resolved)
}

/// If `expr_id` is an `ExprKind::Let { init: Field { receiver, .. } }` OR a
/// `Block` containing `StmtKind::Let { init: Field { receiver, .. } }`,
/// and `receiver`'s type implements `Drop`, emit `E2048`.
fn check_drop_let_at(ctx: &mut DropWalkCtx<'_>, kind: ExprKind) {
    match kind {
        ExprKind::Let { init, .. } => {
            if let Some((agg, field, span)) = direct_projection(ctx.arena, init) {
                let recv = init_receiver(ctx.arena, init);
                if let Some(type_name) = drop_aggregate_type_name(ctx, recv) {
                    ctx.errors.push(TypeCheckError::drop_partial_move(
                        span, agg, field, type_name,
                    ));
                }
            }
        }
        ExprKind::Block { stmts, .. } => {
            for stmt in ctx.arena.get_stmt_range(stmts) {
                if let ori_ir::StmtKind::Let { init, .. } = stmt.kind {
                    if let Some((agg, field, span)) = direct_projection(ctx.arena, init) {
                        let recv = init_receiver(ctx.arena, init);
                        if let Some(type_name) = drop_aggregate_type_name(ctx, recv) {
                            ctx.errors.push(TypeCheckError::drop_partial_move(
                                span, agg, field, type_name,
                            ));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// When `receiver`'s inferred type implements `Drop`, return the Drop
/// type's source name for the diagnostic. Returns `None` for any of:
/// missing type, error type, primitive scalar, collection (str / [T] /
/// {K: V} / Set<T>), borrowed projection, or a Struct/Enum/Newtype with
/// no Drop impl in the registry.
fn drop_aggregate_type_name(ctx: &DropWalkCtx<'_>, expr: ExprId) -> Option<Name> {
    let &ty = ctx.expr_types.get(&expr.raw_index())?;
    drop_type_name_for_idx(ctx, ty)
}

/// Idx-based core of [`drop_aggregate_type_name`]: when `ty` (resolved or in
/// its unresolved `Named` form) is a nominal aggregate carrying a registered
/// `impl T: Drop`, return its display name; else `None`. Shared by the
/// receiver-expr path and the nested-destructure recursion so both gate Drop
/// status identically (HYG §Algorithmic DRY).
fn drop_type_name_for_idx(ctx: &DropWalkCtx<'_>, ty: Idx) -> Option<Name> {
    let resolved = ctx.pool.resolve_fully(ty);
    if resolved == Idx::ERROR {
        return None;
    }
    // Quick reject: only nominal aggregates can carry an `impl T: Drop`
    // (primitives + collections are not user-implementable per
    // `drop-trait-proposal.md §Auto-derive`).
    let tag = ctx.pool.tag(resolved);
    match tag {
        Tag::Struct | Tag::Tuple | Tag::Enum | Tag::Named | Tag::Applied => {}
        _ => return None,
    }
    // Find an `impl T: Drop` for the type. The trait registry keys
    // `impls_by_type` by the EXACT self-type Idx recorded at registration —
    // typically the unresolved `Named` form (`ty`), while the inferred type
    // often arrives as the resolved `Struct`/`Enum` form (`resolved`). Try BOTH
    // keys (mirroring the codegen-side `drop_type_name` resolve-state
    // robustness in `element_fn_gen.rs`) so the lookup is not defeated by which
    // side of the resolve boundary the impl was registered under.
    if ctx
        .trait_registry
        .find_impl(ctx.drop_trait_idx, resolved)
        .is_none()
        && ctx
            .trait_registry
            .find_impl(ctx.drop_trait_idx, ty)
            .is_none()
    {
        return None;
    }
    // Extract a display name for the diagnostic.
    drop_type_display_name(ctx.pool, resolved, tag)
}

/// Resolve a display name for the Drop type's diagnostic, by tag.
///
/// `Tag::Tuple` has no name; we synthesise a placeholder so the error
/// message remains intelligible. `Tag::Named` / `Tag::Applied` use their
/// stored name. `Tag::Struct` / `Tag::Enum` consult the pool's nominal
/// accessors.
fn drop_type_display_name(pool: &Pool, idx: Idx, tag: Tag) -> Option<Name> {
    match tag {
        Tag::Struct => Some(pool.struct_name(idx)),
        Tag::Enum => Some(pool.enum_name(idx)),
        Tag::Named => Some(pool.named_name(idx)),
        Tag::Applied => Some(pool.applied_name(idx)),
        // Tuples have no syntactic name. Every other tag is rejected
        // upstream of this helper by `drop_aggregate_type_name`'s tag
        // check, so the wildcard arm only fires for `Tag::Tuple`.
        _ => None,
    }
}

#[cfg(test)]
mod tests;
