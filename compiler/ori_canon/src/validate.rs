//! Debug-mode validation of canonical IR invariants.
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
//! These checks are enabled only in debug builds (`debug_assert!`).
//! They catch bugs in the lowering pass early, before backends consume
//! invalid canonical IR.

use ori_ir::canon::{CanArena, CanExpr, CanonResult};
use ori_ir::TypeId;

/// Validate that a `CanonResult` satisfies all canonical invariants.
///
/// This function is called after lowering in debug builds. It panics
/// with a descriptive message if any invariant is violated.
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
    debug_assert!(
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
    debug_assert!(
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
            debug_assert!(
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

        // Binary nodes.
        CanExpr::Binary { left, right, .. } => {
            validate_can_id(arena, id, *left, "left");
            validate_can_id(arena, id, *right, "right");
        }
        CanExpr::Cast { expr, .. } => validate_can_id(arena, id, *expr, "expr"),
        CanExpr::Field { receiver, .. } => validate_can_id(arena, id, *receiver, "receiver"),
        CanExpr::Index { receiver, index } => {
            validate_can_id(arena, id, *receiver, "receiver");
            validate_can_id(arena, id, *index, "index");
        }
        CanExpr::Assign { target, value } => {
            validate_can_id(arena, id, *target, "target");
            validate_can_id(arena, id, *value, "value");
        }

        // Ternary+ nodes.
        CanExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            validate_can_id(arena, id, *cond, "cond");
            validate_can_id(arena, id, *then_branch, "then_branch");
            if else_branch.is_valid() {
                validate_can_id(arena, id, *else_branch, "else_branch");
            }
        }
        CanExpr::For {
            iter, guard, body, ..
        } => {
            validate_can_id(arena, id, *iter, "iter");
            if guard.is_valid() {
                validate_can_id(arena, id, *guard, "guard");
            }
            validate_can_id(arena, id, *body, "body");
        }
        CanExpr::WithCapability { provider, body, .. } => {
            validate_can_id(arena, id, *provider, "provider");
            validate_can_id(arena, id, *body, "body");
        }

        // Match — validate decision tree reference.
        CanExpr::Match {
            scrutinee,
            decision_tree,
            arms,
        } => {
            validate_can_id(arena, id, *scrutinee, "scrutinee");
            debug_assert!(
                decision_tree.index() < result.decision_trees.len(),
                "CanNode({}) references DecisionTreeId({}) but pool has {} trees",
                id.raw(),
                decision_tree.raw(),
                result.decision_trees.len(),
            );
            validate_can_range(arena, id, *arms, "arms");
        }

        // Block.
        CanExpr::Block { stmts, result: res } => {
            validate_can_range(arena, id, *stmts, "stmts");
            if res.is_valid() {
                validate_can_id(arena, id, *res, "result");
            }
        }

        // Let.
        CanExpr::Let { init, .. } => validate_can_id(arena, id, *init, "init"),

        // Lambda.
        CanExpr::Lambda { body, .. } => validate_can_id(arena, id, *body, "body"),

        // Calls.
        CanExpr::Call { func, args } => {
            validate_can_id(arena, id, *func, "func");
            validate_can_range(arena, id, *args, "args");
        }
        CanExpr::MethodCall { receiver, args, .. } => {
            validate_can_id(arena, id, *receiver, "receiver");
            validate_can_range(arena, id, *args, "args");
        }

        // Collections.
        CanExpr::List(range) | CanExpr::Tuple(range) => {
            validate_can_range(arena, id, *range, "elements");
        }
        CanExpr::Map(range) => {
            if !range.is_empty() {
                // Validate each map entry's key and value references.
                let entries = arena.get_map_entries(*range);
                for (i, entry) in entries.iter().enumerate() {
                    validate_can_id(arena, id, entry.key, &format!("map[{i}].key"));
                    validate_can_id(arena, id, entry.value, &format!("map[{i}].value"));
                }
            }
        }
        CanExpr::Struct { fields, .. } => {
            if !fields.is_empty() {
                // Validate each field's value reference.
                let field_list = arena.get_fields(*fields);
                for (i, field) in field_list.iter().enumerate() {
                    validate_can_id(arena, id, field.value, &format!("field[{i}].value"));
                }
            }
        }
        CanExpr::Range {
            start, end, step, ..
        } => {
            if start.is_valid() {
                validate_can_id(arena, id, *start, "start");
            }
            if end.is_valid() {
                validate_can_id(arena, id, *end, "end");
            }
            if step.is_valid() {
                validate_can_id(arena, id, *step, "step");
            }
        }
    }
}

/// Validate that a `CanId` is within arena bounds.
fn validate_can_id(
    arena: &CanArena,
    parent: ori_ir::canon::CanId,
    child: ori_ir::canon::CanId,
    field_name: &str,
) {
    debug_assert!(
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
        debug_assert!(
            child_id.index() < arena.len(),
            "CanNode({}).{field_name}[{i}] references CanId({}) but arena has {} nodes",
            parent.raw(),
            child_id.raw(),
            arena.len(),
        );
    }
}
