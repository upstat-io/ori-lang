//! Validation of canonical IR invariants.
//!
//! Walks the canonical arena and asserts that all invariants hold:
//! - No sugar variants present in `CanExpr`
//! - All `CanId` references resolve to valid nodes
//! - All `CanRange` references are within bounds
//! - All `CanMapEntryRange` and `CanFieldRange` references are within bounds
//! - Every `CanNode` has a valid (non-INFER) type
//! - All `DecisionTreeId` references resolve to valid trees
//! - All `ConstantId` references resolve to valid constants
//!
//! These release-active checks catch bugs in the lowering pass before backends
//! consume invalid canonical IR.

use ori_ir::canon::{CanArena, CanExpr, CanonResult};
use ori_ir::TypeId;

/// Validate that a `CanonResult` satisfies all canonical invariants.
///
/// This function is called after lowering. It panics with a descriptive message
/// if any invariant is violated.
///
/// # What's Checked
///
/// 1. All `CanId` references point to valid arena nodes
/// 2. All range references are within their storage bounds
/// 3. No `CanExpr` variant is a sugar variant (type-level guarantee
///    already prevents this, but we verify ranges and IDs)
/// 4. Every node has a resolved type (not INFER)
/// 5. The root expression is valid
pub fn validate(result: &CanonResult) {
    if !result.root.is_valid() {
        // Empty/error result — nothing to validate.
        return;
    }

    let arena = &result.arena;

    // Validate root is within bounds.
    assert!(
        result.root.index() < arena.len(),
        "root CanId({}) out of bounds (arena has {} nodes)",
        result.root.raw(),
        arena.len(),
    );

    // Walk all nodes and validate references.
    for i in 0..arena.len() {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "arena indices always fit u32"
        )]
        let id = ori_ir::canon::CanId::new(i as u32);
        let kind = arena.kind(id);
        let ty = arena.ty(id);

        validate_type(id, ty);
        validate_expr(arena, result, id, kind);
    }
}

/// Validate that a node's type is resolved (not INFER).
fn validate_type(id: ori_ir::canon::CanId, ty: TypeId) {
    assert!(
        ty != TypeId::INFER,
        "CanNode({}) has unresolved type INFER",
        id.raw(),
    );
}

/// Validate all child references in a `CanExpr`.
fn validate_expr(arena: &CanArena, result: &CanonResult, id: ori_ir::canon::CanId, kind: &CanExpr) {
    match kind {
        // Leaf nodes — no child references to validate.
        CanExpr::Int(_)
        | CanExpr::Float(_)
        | CanExpr::Bool(_)
        | CanExpr::Str(_)
        | CanExpr::Char(_)
        | CanExpr::Duration { .. }
        | CanExpr::Size { .. }
        | CanExpr::Unit
        | CanExpr::Ident(_)
        | CanExpr::Const(_)
        | CanExpr::SelfRef
        | CanExpr::FunctionRef(_)
        | CanExpr::TypeRef(_)
        | CanExpr::HashLength
        | CanExpr::None
        | CanExpr::FunctionExp { .. }
        | CanExpr::Error => {}

        // Constant — validate pool reference.
        CanExpr::Constant(const_id) => {
            assert!(
                const_id.index() < result.constants.len(),
                "CanNode({}) references ConstantId({}) but pool has {} entries",
                id.raw(),
                const_id.raw(),
                result.constants.len(),
            );
        }

        // Unary nodes — validate single child.
        CanExpr::Unary { operand, .. } => validate_can_id(arena, id, *operand, "operand"),
        CanExpr::Try(child)
        | CanExpr::Await(child)
        | CanExpr::Unsafe(child)
        | CanExpr::Some(child)
        | CanExpr::Ok(child)
        | CanExpr::Err(child)
        | CanExpr::Loop { body: child, .. }
        | CanExpr::Break { value: child, .. }
        | CanExpr::Continue { value: child, .. } => {
            // INVALID is allowed for Ok(()), Err(()), Break, Continue with no value.
            if child.is_valid() {
                validate_can_id(arena, id, *child, "child");
            }
        }

        expression @ (CanExpr::Binary { .. }
        | CanExpr::Cast { .. }
        | CanExpr::FormatWith { .. }
        | CanExpr::Field { .. }
        | CanExpr::Index { .. }
        | CanExpr::Assign { .. }) => validate_operator_expr(arena, id, expression),
        expression @ (CanExpr::If { .. }
        | CanExpr::For { .. }
        | CanExpr::WithCapability { .. }
        | CanExpr::Match { .. }) => validate_control_expr(arena, result, id, expression),
        expression @ (CanExpr::Block { .. }
        | CanExpr::Let { .. }
        | CanExpr::Lambda { .. }
        | CanExpr::Call { .. }
        | CanExpr::MethodCall { .. }
        | CanExpr::List(_)
        | CanExpr::Tuple(_)
        | CanExpr::Map(_)
        | CanExpr::Struct { .. }
        | CanExpr::Range { .. }) => validate_aggregate_expr(arena, id, expression),
    }
}

fn validate_operator_expr(arena: &CanArena, id: ori_ir::canon::CanId, kind: &CanExpr) {
    match kind {
        CanExpr::Binary { left, right, .. } => {
            validate_can_id(arena, id, *left, "left");
            validate_can_id(arena, id, *right, "right");
        }
        CanExpr::Cast { expr, .. } | CanExpr::FormatWith { expr, .. } => {
            validate_can_id(arena, id, *expr, "expr");
        }
        CanExpr::Field { receiver, .. } => validate_can_id(arena, id, *receiver, "receiver"),
        CanExpr::Index { receiver, index } => {
            validate_can_id(arena, id, *receiver, "receiver");
            validate_can_id(arena, id, *index, "index");
        }
        CanExpr::Assign { target, value } => {
            validate_can_id(arena, id, *target, "target");
            validate_can_id(arena, id, *value, "value");
        }
        _ => unreachable!("non-operator expression in operator validation"),
    }
}

fn validate_control_expr(
    arena: &CanArena,
    result: &CanonResult,
    id: ori_ir::canon::CanId,
    kind: &CanExpr,
) {
    match kind {
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            validate_can_id(arena, id, *cond, "cond");
            validate_can_id(arena, id, *then_branch, "then_branch");
            validate_optional_can_id(arena, id, *else_branch, "else_branch");
        }
        CanExpr::For {
            iter, guard, body, ..
        } => {
            validate_can_id(arena, id, *iter, "iter");
            validate_optional_can_id(arena, id, *guard, "guard");
            validate_can_id(arena, id, *body, "body");
        }
        CanExpr::WithCapability { provider, body, .. } => {
            validate_can_id(arena, id, *provider, "provider");
            validate_can_id(arena, id, *body, "body");
        }
        CanExpr::Match {
            scrutinee,
            decision_tree,
            arms,
        } => {
            validate_can_id(arena, id, *scrutinee, "scrutinee");
            assert!(
                decision_tree.index() < result.decision_trees.len(),
                "CanNode({}) references DecisionTreeId({}) but pool has {} trees",
                id.raw(),
                decision_tree.raw(),
                result.decision_trees.len(),
            );
            validate_can_range(arena, id, *arms, "arms");
        }
        _ => unreachable!("non-control expression in control validation"),
    }
}

fn validate_aggregate_expr(arena: &CanArena, id: ori_ir::canon::CanId, kind: &CanExpr) {
    match kind {
        CanExpr::Block { stmts, result } => {
            validate_can_range(arena, id, *stmts, "stmts");
            validate_optional_can_id(arena, id, *result, "result");
        }
        CanExpr::Let { init, .. } => validate_can_id(arena, id, *init, "init"),
        CanExpr::Lambda { body, .. } => validate_can_id(arena, id, *body, "body"),
        CanExpr::Call { func, args } => {
            validate_can_id(arena, id, *func, "func");
            validate_can_range(arena, id, *args, "args");
        }
        CanExpr::MethodCall { receiver, args, .. } => {
            validate_can_id(arena, id, *receiver, "receiver");
            validate_can_range(arena, id, *args, "args");
        }
        CanExpr::List(range) | CanExpr::Tuple(range) => {
            validate_can_range(arena, id, *range, "elements");
        }
        CanExpr::Map(range) => validate_map_entries(arena, id, *range),
        CanExpr::Struct { fields, .. } => validate_struct_fields(arena, id, *fields),
        CanExpr::Range {
            start, end, step, ..
        } => {
            validate_optional_can_id(arena, id, *start, "start");
            validate_optional_can_id(arena, id, *end, "end");
            validate_optional_can_id(arena, id, *step, "step");
        }
        _ => unreachable!("non-aggregate expression in aggregate validation"),
    }
}

fn validate_optional_can_id(
    arena: &CanArena,
    parent: ori_ir::canon::CanId,
    child: ori_ir::canon::CanId,
    field_name: &str,
) {
    if child.is_valid() {
        validate_can_id(arena, parent, child, field_name);
    }
}

fn validate_map_entries(
    arena: &CanArena,
    id: ori_ir::canon::CanId,
    range: ori_ir::canon::CanMapEntryRange,
) {
    for (index, entry) in arena.get_map_entries(range).iter().enumerate() {
        validate_can_id(arena, id, entry.key, &format!("map[{index}].key"));
        validate_can_id(arena, id, entry.value, &format!("map[{index}].value"));
    }
}

fn validate_struct_fields(
    arena: &CanArena,
    id: ori_ir::canon::CanId,
    fields: ori_ir::canon::CanFieldRange,
) {
    for (index, field) in arena.get_fields(fields).iter().enumerate() {
        validate_can_id(arena, id, field.value, &format!("field[{index}].value"));
    }
}

/// Validate that a `CanId` is within arena bounds.
fn validate_can_id(
    arena: &CanArena,
    parent: ori_ir::canon::CanId,
    child: ori_ir::canon::CanId,
    field_name: &str,
) {
    assert!(
        child.index() < arena.len(),
        "CanNode({}).{field_name} references CanId({}) but arena has {} nodes",
        parent.raw(),
        child.raw(),
        arena.len(),
    );
}

/// Validate that a `CanRange` is within the `expr_lists` bounds.
fn validate_can_range(
    arena: &CanArena,
    parent: ori_ir::canon::CanId,
    range: ori_ir::canon::CanRange,
    field_name: &str,
) {
    if range.is_empty() {
        return;
    }
    // Verify each ID in the range is valid.
    let ids = arena.get_expr_list(range);
    for (i, &child_id) in ids.iter().enumerate() {
        assert!(
            child_id.index() < arena.len(),
            "CanNode({}).{field_name}[{i}] references CanId({}) but arena has {} nodes",
            parent.raw(),
            child_id.raw(),
            arena.len(),
        );
    }
}

#[cfg(test)]
mod tests;
