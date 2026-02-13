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

use crate::Name;

use super::{
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
/// Organized by variant category for clarity. Each arm hashes non-child
/// data first, then recurses into child `CanId`s in a deterministic order.
fn hash_expr(arena: &CanArena, kind: &CanExpr, state: &mut FxHasher) {
    match *kind {
        // Literals — hash value directly
        CanExpr::Int(v) => v.hash(state),
        CanExpr::Float(v) => v.hash(state),
        CanExpr::Bool(v) => v.hash(state),
        CanExpr::Char(c) => c.hash(state),
        CanExpr::Duration { value, unit } => {
            value.hash(state);
            mem::discriminant(&unit).hash(state);
        }
        CanExpr::Size { value, unit } => {
            value.hash(state);
            mem::discriminant(&unit).hash(state);
        }
        CanExpr::Unit | CanExpr::None | CanExpr::SelfRef | CanExpr::HashLength | CanExpr::Error => {
            // No additional data — discriminant + type already hashed.
        }

        // References
        CanExpr::Constant(id) => id.raw().hash(state),
        CanExpr::Str(name)
        | CanExpr::Ident(name)
        | CanExpr::Const(name)
        | CanExpr::FunctionRef(name)
        | CanExpr::TypeRef(name) => name.raw().hash(state),

        // Operators
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

        // Calls
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

        // Access
        CanExpr::Field { receiver, field } => {
            field.raw().hash(state);
            hash_node(arena, receiver, state);
        }
        CanExpr::Index { receiver, index } => {
            hash_node(arena, receiver, state);
            hash_node(arena, index, state);
        }

        // Control flow
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
            binding,
            iter,
            guard,
            body,
            is_yield,
        } => {
            binding.raw().hash(state);
            is_yield.hash(state);
            hash_node(arena, iter, state);
            hash_node(arena, guard, state);
            hash_node(arena, body, state);
        }
        CanExpr::Loop { body } => hash_node(arena, body, state),
        CanExpr::Break(child)
        | CanExpr::Continue(child)
        | CanExpr::Try(child)
        | CanExpr::Await(child) => hash_node(arena, child, state),

        // Bindings
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

        // Functions
        CanExpr::Lambda { params, body } => {
            hash_params(arena, params, state);
            hash_node(arena, body, state);
        }

        // Collections
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

        // Algebraic
        CanExpr::Ok(child) | CanExpr::Err(child) | CanExpr::Some(child) => {
            hash_node(arena, child, state);
        }

        // Capabilities
        CanExpr::WithCapability {
            capability,
            provider,
            body,
        } => {
            capability.raw().hash(state);
            hash_node(arena, provider, state);
            hash_node(arena, body, state);
        }

        // Special forms
        CanExpr::FunctionExp { kind, props } => {
            mem::discriminant(&kind).hash(state);
            hash_named_exprs(arena, props, state);
        }
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
fn hash_binding_pattern(arena: &CanArena, id: super::CanBindingPatternId, state: &mut FxHasher) {
    let pattern = arena.get_binding_pattern(id);
    mem::discriminant(pattern).hash(state);

    match *pattern {
        CanBindingPattern::Name(name) => name.raw().hash(state),
        CanBindingPattern::Tuple(range) => hash_binding_pattern_range(arena, range, state),
        CanBindingPattern::Struct { fields } => hash_field_bindings(arena, fields, state),
        CanBindingPattern::List { elements, rest } => {
            hash_binding_pattern_range(arena, elements, state);
            rest.map(Name::raw).hash(state);
        }
        CanBindingPattern::Wildcard => {}
    }
}

/// Hash a range of binding pattern IDs (for Tuple/List sub-patterns).
fn hash_binding_pattern_range(
    arena: &CanArena,
    range: super::CanBindingPatternRange,
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

#[cfg(test)]
mod tests {
    use crate::canon::{CanField, CanMapEntry, CanNode, CanParam};
    use crate::{Name, Span, TypeId};

    use super::*;

    /// Build a simple arena with a single Int node.
    fn arena_with_int(value: i64) -> (CanArena, CanId) {
        let mut arena = CanArena::new();
        let id = arena.push(CanNode::new(CanExpr::Int(value), Span::DUMMY, TypeId::INT));
        (arena, id)
    }

    #[test]
    fn same_body_same_hash() {
        let (a1, r1) = arena_with_int(42);
        let (a2, r2) = arena_with_int(42);
        assert_eq!(
            hash_canonical_subtree(&a1, r1),
            hash_canonical_subtree(&a2, r2),
        );
    }

    #[test]
    fn different_value_different_hash() {
        let (a1, r1) = arena_with_int(42);
        let (a2, r2) = arena_with_int(43);
        assert_ne!(
            hash_canonical_subtree(&a1, r1),
            hash_canonical_subtree(&a2, r2),
        );
    }

    #[test]
    fn different_type_different_hash() {
        let mut a1 = CanArena::new();
        let r1 = a1.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));

        let mut a2 = CanArena::new();
        let r2 = a2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::FLOAT));

        assert_ne!(
            hash_canonical_subtree(&a1, r1),
            hash_canonical_subtree(&a2, r2),
        );
    }

    #[test]
    fn span_does_not_affect_hash() {
        let mut a1 = CanArena::new();
        let r1 = a1.push(CanNode::new(CanExpr::Int(42), Span::new(0, 5), TypeId::INT));

        let mut a2 = CanArena::new();
        let r2 = a2.push(CanNode::new(
            CanExpr::Int(42),
            Span::new(100, 200),
            TypeId::INT,
        ));

        assert_eq!(
            hash_canonical_subtree(&a1, r1),
            hash_canonical_subtree(&a2, r2),
            "span differences should not affect the hash",
        );
    }

    #[test]
    fn binary_expr_hash() {
        let mut arena = CanArena::new();
        let left = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let right = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let add = arena.push(CanNode::new(
            CanExpr::Binary {
                op: crate::BinaryOp::Add,
                left,
                right,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        let mut arena2 = CanArena::new();
        let left2 = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let right2 = arena2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let add2 = arena2.push(CanNode::new(
            CanExpr::Binary {
                op: crate::BinaryOp::Add,
                left: left2,
                right: right2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_eq!(
            hash_canonical_subtree(&arena, add),
            hash_canonical_subtree(&arena2, add2),
        );
    }

    #[test]
    fn different_operator_different_hash() {
        let mut a1 = CanArena::new();
        let l1 = a1.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let r1 = a1.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let add = a1.push(CanNode::new(
            CanExpr::Binary {
                op: crate::BinaryOp::Add,
                left: l1,
                right: r1,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        let mut a2 = CanArena::new();
        let l2 = a2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let r2 = a2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let sub = a2.push(CanNode::new(
            CanExpr::Binary {
                op: crate::BinaryOp::Sub,
                left: l2,
                right: r2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_ne!(
            hash_canonical_subtree(&a1, add),
            hash_canonical_subtree(&a2, sub),
        );
    }

    #[test]
    fn block_with_stmts_hash() {
        let mut arena = CanArena::new();
        let s1 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let s2 = arena.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let result = arena.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));
        let stmts = arena.push_expr_list(&[s1, s2]);
        let block = arena.push(CanNode::new(
            CanExpr::Block { stmts, result },
            Span::DUMMY,
            TypeId::INT,
        ));

        // Same block, different arena
        let mut arena2 = CanArena::new();
        let s1b = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let s2b = arena2.push(CanNode::new(CanExpr::Int(2), Span::DUMMY, TypeId::INT));
        let result2 = arena2.push(CanNode::new(CanExpr::Int(3), Span::DUMMY, TypeId::INT));
        let stmts2 = arena2.push_expr_list(&[s1b, s2b]);
        let block2 = arena2.push(CanNode::new(
            CanExpr::Block {
                stmts: stmts2,
                result: result2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_eq!(
            hash_canonical_subtree(&arena, block),
            hash_canonical_subtree(&arena2, block2),
        );
    }

    #[test]
    fn invalid_root_produces_consistent_hash() {
        let arena = CanArena::new();
        let h1 = hash_canonical_subtree(&arena, CanId::INVALID);
        let h2 = hash_canonical_subtree(&arena, CanId::INVALID);
        assert_eq!(h1, h2);
    }

    #[test]
    fn call_expr_hash() {
        let mut arena = CanArena::new();
        let func = arena.push(CanNode::new(
            CanExpr::Ident(Name::from_raw(10)),
            Span::DUMMY,
            TypeId::INT,
        ));
        let arg = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
        let args = arena.push_expr_list(&[arg]);
        let call = arena.push(CanNode::new(
            CanExpr::Call { func, args },
            Span::DUMMY,
            TypeId::INT,
        ));

        // Same call in a different arena
        let mut arena2 = CanArena::new();
        let func2 = arena2.push(CanNode::new(
            CanExpr::Ident(Name::from_raw(10)),
            Span::DUMMY,
            TypeId::INT,
        ));
        let arg2 = arena2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
        let args2 = arena2.push_expr_list(&[arg2]);
        let call2 = arena2.push(CanNode::new(
            CanExpr::Call {
                func: func2,
                args: args2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_eq!(
            hash_canonical_subtree(&arena, call),
            hash_canonical_subtree(&arena2, call2),
        );
    }

    #[test]
    fn struct_expr_hash() {
        let mut arena = CanArena::new();
        let v1 = arena.push(CanNode::new(CanExpr::Int(0), Span::DUMMY, TypeId::INT));
        let v2 = arena.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let fields = arena.push_fields(&[
            CanField {
                name: Name::from_raw(1),
                value: v1,
            },
            CanField {
                name: Name::from_raw(2),
                value: v2,
            },
        ]);
        let root = arena.push(CanNode::new(
            CanExpr::Struct {
                name: Name::from_raw(10),
                fields,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        // Different field values should produce different hash
        let mut arena2 = CanArena::new();
        let v1b = arena2.push(CanNode::new(CanExpr::Int(99), Span::DUMMY, TypeId::INT));
        let v2b = arena2.push(CanNode::new(CanExpr::Int(1), Span::DUMMY, TypeId::INT));
        let fields2 = arena2.push_fields(&[
            CanField {
                name: Name::from_raw(1),
                value: v1b,
            },
            CanField {
                name: Name::from_raw(2),
                value: v2b,
            },
        ]);
        let root2 = arena2.push(CanNode::new(
            CanExpr::Struct {
                name: Name::from_raw(10),
                fields: fields2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_ne!(
            hash_canonical_subtree(&arena, root),
            hash_canonical_subtree(&arena2, root2),
        );
    }

    #[test]
    fn lambda_hash() {
        let mut arena = CanArena::new();
        let body = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
        let params = arena.push_params(&[CanParam {
            name: Name::from_raw(1),
            default: CanId::INVALID,
        }]);
        let lambda = arena.push(CanNode::new(
            CanExpr::Lambda { params, body },
            Span::DUMMY,
            TypeId::INT,
        ));

        // Same lambda, different param name → different hash
        let mut arena2 = CanArena::new();
        let body2 = arena2.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
        let params2 = arena2.push_params(&[CanParam {
            name: Name::from_raw(2),
            default: CanId::INVALID,
        }]);
        let lambda2 = arena2.push(CanNode::new(
            CanExpr::Lambda {
                params: params2,
                body: body2,
            },
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_ne!(
            hash_canonical_subtree(&arena, lambda),
            hash_canonical_subtree(&arena2, lambda2),
        );
    }

    #[test]
    fn map_expr_hash() {
        let mut arena = CanArena::new();
        let k = arena.push(CanNode::new(
            CanExpr::Str(Name::from_raw(1)),
            Span::DUMMY,
            TypeId::STR,
        ));
        let v = arena.push(CanNode::new(CanExpr::Int(42), Span::DUMMY, TypeId::INT));
        let entries = arena.push_map_entries(&[CanMapEntry { key: k, value: v }]);
        let root = arena.push(CanNode::new(
            CanExpr::Map(entries),
            Span::DUMMY,
            TypeId::INT,
        ));

        // Different value in map → different hash
        let mut arena2 = CanArena::new();
        let k2 = arena2.push(CanNode::new(
            CanExpr::Str(Name::from_raw(1)),
            Span::DUMMY,
            TypeId::STR,
        ));
        let v2 = arena2.push(CanNode::new(CanExpr::Int(99), Span::DUMMY, TypeId::INT));
        let entries2 = arena2.push_map_entries(&[CanMapEntry { key: k2, value: v2 }]);
        let root2 = arena2.push(CanNode::new(
            CanExpr::Map(entries2),
            Span::DUMMY,
            TypeId::INT,
        ));

        assert_ne!(
            hash_canonical_subtree(&arena, root),
            hash_canonical_subtree(&arena2, root2),
        );
    }
}
