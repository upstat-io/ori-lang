//! Canonical subtree hashing for incremental compilation.
//!
//! Produces span-free, structurally-stable hashes of canonical expression trees.
//! Two functions with identical canonical bodies produce identical hashes,
//! regardless of source formatting, comments, or arena allocation order.
//!
//! Used by:
//! - `oric` test runner: detect which function bodies changed between runs
//! - `ori_llvm` incremental codegen: replace the placeholder body hash
//!
//! # Strategy
//!
//! Recursive depth-first traversal. For each node:
//! 1. Hash `discriminant(kind)` — identifies the variant
//! 2. Hash non-child data fields (literals, names, operators, flags)
//! 3. Hash `ty.raw()` — the resolved type
//! 4. Recurse into child `CanId`s (content, not index)
//!
//! `CanId` values themselves are never hashed — they're arena indices
//! that vary across runs. Only the *content* at each index is hashed.

use std::hash::{Hash, Hasher};
use std::mem;

use rustc_hash::FxHasher;

use super::super::{
    CanArena, CanBindingPattern, CanExpr, CanFieldBindingRange, CanFieldRange, CanId,
    CanMapEntryRange, CanNamedExprRange, CanParamRange, CanRange,
};

/// Hash a canonical subtree rooted at `root`.
///
/// Returns a `u64` fingerprint that is stable across runs for identical
/// canonical trees. Different trees produce different hashes (with
/// standard hash collision probability).
///
/// # Panics
///
/// Panics in debug mode if `root` is invalid or out of bounds.
pub fn hash_canonical_subtree(arena: &CanArena, root: CanId) -> u64 {
    let mut hasher = FxHasher::default();
    hash_node(arena, root, &mut hasher);
    hasher.finish()
}

/// Hash a single node and recurse into its children.
fn hash_node(arena: &CanArena, id: CanId, state: &mut FxHasher) {
    if !id.is_valid() {
        // Sentinel for "no expression" (optional branches, no guard, etc.)
        u32::MAX.hash(state);
        return;
    }

    let kind = arena.kind(id);
    let ty = arena.ty(id);

    // Hash variant discriminant.
    mem::discriminant(kind).hash(state);

    // Hash resolved type.
    ty.raw().hash(state);

    // Hash variant-specific data and recurse into children.
    hash_expr(arena, kind, state);
}

/// Hash expression-specific data fields and recurse into children.
///
/// One exhaustive match keeps new canonical variants fail-closed: adding a
/// variant cannot compile until its stable hash semantics are defined here.
fn hash_expr(arena: &CanArena, kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        CanExpr::Int(_)
        | CanExpr::Float(_)
        | CanExpr::Bool(_)
        | CanExpr::Char(_)
        | CanExpr::Duration { .. }
        | CanExpr::Size { .. }
        | CanExpr::Unit
        | CanExpr::None
        | CanExpr::SelfRef
        | CanExpr::HashLength
        | CanExpr::Error
        | CanExpr::Constant(_)
        | CanExpr::Str(_)
        | CanExpr::Ident(_)
        | CanExpr::Const(_)
        | CanExpr::FunctionRef(_)
        | CanExpr::TypeRef(_) => hash_leaf_expr(kind, state),
        CanExpr::Binary { .. }
        | CanExpr::Unary { .. }
        | CanExpr::Cast { .. }
        | CanExpr::Field { .. }
        | CanExpr::Index { .. }
        | CanExpr::Call { .. }
        | CanExpr::MethodCall { .. }
        | CanExpr::FormatWith { .. } => hash_operator_expr(arena, kind, state),
        CanExpr::If { .. }
        | CanExpr::Match { .. }
        | CanExpr::For { .. }
        | CanExpr::Loop { .. }
        | CanExpr::Break { .. }
        | CanExpr::Continue { .. }
        | CanExpr::Try(_)
        | CanExpr::Unsafe(_)
        | CanExpr::Await(_)
        | CanExpr::Block { .. }
        | CanExpr::Let { .. }
        | CanExpr::Assign { .. }
        | CanExpr::WithCapability { .. } => hash_control_expr(arena, kind, state),
        CanExpr::Lambda { .. }
        | CanExpr::List(_)
        | CanExpr::Tuple(_)
        | CanExpr::Map(_)
        | CanExpr::Struct { .. }
        | CanExpr::Range { .. }
        | CanExpr::Ok(_)
        | CanExpr::Err(_)
        | CanExpr::Some(_)
        | CanExpr::FunctionExp { .. } => hash_construct_expr(arena, kind, state),
    }
}

fn hash_leaf_expr(kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        CanExpr::Int(value) => value.hash(state),
        CanExpr::Float(value) => value.hash(state),
        CanExpr::Bool(value) => value.hash(state),
        CanExpr::Char(value) => value.hash(state),
        CanExpr::Duration { value, unit } => {
            value.hash(state);
            mem::discriminant(&unit).hash(state);
        }
        CanExpr::Size { value, unit } => {
            value.hash(state);
            mem::discriminant(&unit).hash(state);
        }
        CanExpr::Constant(id) => id.raw().hash(state),
        CanExpr::Str(name)
        | CanExpr::Ident(name)
        | CanExpr::Const(name)
        | CanExpr::FunctionRef(name)
        | CanExpr::TypeRef(name) => name.raw().hash(state),
        CanExpr::Unit | CanExpr::None | CanExpr::SelfRef | CanExpr::HashLength | CanExpr::Error => {
        }
        _ => unreachable!("leaf expression hashing category drift"),
    }
}

fn hash_operator_expr(arena: &CanArena, kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        CanExpr::Binary { op, left, right } => {
            mem::discriminant(&op).hash(state);
            hash_node(arena, left, state);
            hash_node(arena, right, state);
        }
        CanExpr::Unary { op, operand } => {
            mem::discriminant(&op).hash(state);
            hash_node(arena, operand, state);
        }
        CanExpr::Cast {
            expr,
            target,
            fallible,
        } => {
            target.raw().hash(state);
            fallible.hash(state);
            hash_node(arena, expr, state);
        }
        CanExpr::Field { receiver, field } => {
            field.raw().hash(state);
            hash_node(arena, receiver, state);
        }
        CanExpr::Index {
            receiver,
            index,
            dispatch,
        } => {
            dispatch.hash(state);
            hash_node(arena, receiver, state);
            hash_node(arena, index, state);
        }
        CanExpr::Call { func, args } => {
            hash_node(arena, func, state);
            hash_range(arena, args, state);
        }
        CanExpr::MethodCall {
            receiver,
            method,
            args,
        } => {
            method.raw().hash(state);
            hash_node(arena, receiver, state);
            hash_range(arena, args, state);
        }
        CanExpr::FormatWith { expr, spec } => {
            spec.raw().hash(state);
            hash_node(arena, expr, state);
        }
        _ => unreachable!("operator expression hashing category drift"),
    }
}

fn hash_control_expr(arena: &CanArena, kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            hash_node(arena, cond, state);
            hash_node(arena, then_branch, state);
            hash_node(arena, else_branch, state);
        }
        CanExpr::Match {
            scrutinee,
            decision_tree,
            arms,
        } => {
            decision_tree.raw().hash(state);
            hash_node(arena, scrutinee, state);
            hash_range(arena, arms, state);
        }
        CanExpr::For {
            label,
            pattern,
            iter,
            guard,
            body,
            is_yield,
        } => {
            label.raw().hash(state);
            pattern.raw().hash(state);
            is_yield.hash(state);
            hash_node(arena, iter, state);
            hash_node(arena, guard, state);
            hash_node(arena, body, state);
        }
        CanExpr::Loop { label, body } => {
            label.raw().hash(state);
            hash_node(arena, body, state);
        }
        CanExpr::Break { label, value } | CanExpr::Continue { label, value } => {
            label.raw().hash(state);
            hash_node(arena, value, state);
        }
        CanExpr::Try(child) | CanExpr::Unsafe(child) | CanExpr::Await(child) => {
            hash_node(arena, child, state);
        }
        CanExpr::Block { stmts, result } => {
            hash_range(arena, stmts, state);
            hash_node(arena, result, state);
        }
        CanExpr::Let {
            pattern,
            init,
            mutable,
        } => {
            mutable.hash(state);
            hash_binding_pattern(arena, pattern, state);
            hash_node(arena, init, state);
        }
        CanExpr::Assign { target, value } => {
            hash_node(arena, target, state);
            hash_node(arena, value, state);
        }
        CanExpr::WithCapability {
            capability,
            provider,
            body,
        } => {
            capability.raw().hash(state);
            hash_node(arena, provider, state);
            hash_node(arena, body, state);
        }
        _ => unreachable!("control expression hashing category drift"),
    }
}

fn hash_construct_expr(arena: &CanArena, kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        CanExpr::Lambda { params, body } => {
            hash_params(arena, params, state);
            hash_node(arena, body, state);
        }
        CanExpr::List(range) | CanExpr::Tuple(range) => hash_range(arena, range, state),
        CanExpr::Map(entries) => hash_map_entries(arena, entries, state),
        CanExpr::Struct { name, fields } => {
            name.raw().hash(state);
            hash_fields(arena, fields, state);
        }
        CanExpr::Range {
            start,
            end,
            step,
            inclusive,
        } => {
            inclusive.hash(state);
            hash_node(arena, start, state);
            hash_node(arena, end, state);
            hash_node(arena, step, state);
        }
        CanExpr::Ok(child) | CanExpr::Err(child) | CanExpr::Some(child) => {
            hash_node(arena, child, state);
        }
        CanExpr::FunctionExp { kind, props } => {
            mem::discriminant(&kind).hash(state);
            hash_named_exprs(arena, props, state);
        }
        _ => unreachable!("construct expression hashing category drift"),
    }
}

/// Hash a contiguous range of expression IDs.
fn hash_range(arena: &CanArena, range: CanRange, state: &mut FxHasher) {
    let ids = arena.get_expr_list(range);
    ids.len().hash(state);
    for &id in ids {
        hash_node(arena, id, state);
    }
}

/// Hash map entries (key-value pairs).
fn hash_map_entries(arena: &CanArena, range: CanMapEntryRange, state: &mut FxHasher) {
    let entries = arena.get_map_entries(range);
    entries.len().hash(state);
    for entry in entries {
        hash_node(arena, entry.key, state);
        hash_node(arena, entry.value, state);
    }
}

/// Hash struct field initializers (name-value pairs).
fn hash_fields(arena: &CanArena, range: CanFieldRange, state: &mut FxHasher) {
    let fields = arena.get_fields(range);
    fields.len().hash(state);
    for field in fields {
        field.name.raw().hash(state);
        hash_node(arena, field.value, state);
    }
}

/// Hash canonical function parameters.
fn hash_params(arena: &CanArena, range: CanParamRange, state: &mut FxHasher) {
    let params = arena.get_params(range);
    params.len().hash(state);
    for param in params {
        param.name.raw().hash(state);
        hash_node(arena, param.default, state);
    }
}

/// Hash named expressions (for `FunctionExp` props).
fn hash_named_exprs(arena: &CanArena, range: CanNamedExprRange, state: &mut FxHasher) {
    let exprs = arena.get_named_exprs(range);
    exprs.len().hash(state);
    for expr in exprs {
        expr.name.raw().hash(state);
        hash_node(arena, expr.value, state);
    }
}

/// Hash a canonical binding pattern (for Let expressions).
fn hash_binding_pattern(
    arena: &CanArena,
    id: super::super::CanBindingPatternId,
    state: &mut FxHasher,
) {
    let pattern = arena.get_binding_pattern(id);
    mem::discriminant(pattern).hash(state);

    match *pattern {
        CanBindingPattern::Name { name, mutable } => {
            name.raw().hash(state);
            mutable.hash(state);
        }
        CanBindingPattern::Tuple(range) => hash_binding_pattern_range(arena, range, state),
        CanBindingPattern::Struct { fields } => hash_field_bindings(arena, fields, state),
        CanBindingPattern::List { elements, rest } => {
            hash_binding_pattern_range(arena, elements, state);
            rest.map(|(name, mutable)| (name.raw(), mutable))
                .hash(state);
        }
        CanBindingPattern::Wildcard => {}
    }
}

/// Hash a range of binding pattern IDs (for Tuple/List sub-patterns).
fn hash_binding_pattern_range(
    arena: &CanArena,
    range: super::super::CanBindingPatternRange,
    state: &mut FxHasher,
) {
    let ids = arena.get_binding_pattern_list(range);
    ids.len().hash(state);
    for &id in ids {
        hash_binding_pattern(arena, id, state);
    }
}

/// Hash struct field bindings.
fn hash_field_bindings(arena: &CanArena, range: CanFieldBindingRange, state: &mut FxHasher) {
    let bindings = arena.get_field_bindings(range);
    bindings.len().hash(state);
    for binding in bindings {
        binding.name.raw().hash(state);
        hash_binding_pattern(arena, binding.pattern, state);
    }
}
