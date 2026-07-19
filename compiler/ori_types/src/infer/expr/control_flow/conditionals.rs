//! If-expression inference.

use ori_ir::{ExprArena, ExprId, Span};

use crate::{ContextKind, Expected, ExpectedOrigin, Idx, SequenceKind, Tag};

use super::super::super::InferEngine;
use super::super::infer_expr;

/// Infer the type of an if expression.
pub(crate) fn infer_if(
    engine: &mut InferEngine<'_>,
    arena: &ExprArena,
    cond: ExprId,
    then_branch: ExprId,
    else_branch: ExprId,
    _span: Span,
) -> Idx {
    // Condition must be bool
    let cond_ty = infer_expr(engine, arena, cond);
    engine.push_context(ContextKind::IfCondition);
    let expected = Expected {
        ty: Idx::BOOL,
        origin: ExpectedOrigin::NoExpectation,
    };
    let _ = engine.check_type(cond_ty, &expected, arena.get_expr(cond).span);
    engine.pop_context();

    // Infer then branch
    engine.push_context(ContextKind::IfThenBranch);
    let then_ty = infer_expr(engine, arena, then_branch);
    engine.pop_context();

    if else_branch.is_present() {
        // If the then-branch diverges (`Never` — e.g. `break` / `continue` /
        // `panic()` / `return`), the if's value is always the else-branch value.
        // `Never` coerces TO any type, not FROM one (UN-3), so constraining the
        // else branch against `Never` would force it to `Never` and yield a
        // `Never`-typed if even when the else branch is concrete (e.g.
        // `if c then break else cond` should be `bool`, not `Never`).
        let then_resolved = engine.resolve(then_ty);
        if engine.pool().tag(then_resolved) == Tag::Never {
            engine.push_context(ContextKind::IfElseBranch { branch_index: 0 });
            let else_ty = infer_expr(engine, arena, else_branch);
            engine.pop_context();
            return engine.resolve(else_ty);
        }

        // Else branch must match then branch
        engine.push_context(ContextKind::IfElseBranch { branch_index: 0 });
        let then_span = arena.get_expr(then_branch).span;
        let expected = Expected {
            ty: then_ty,
            origin: ExpectedOrigin::PreviousInSequence {
                previous_span: then_span,
                current_index: 1,
                sequence_kind: SequenceKind::IfBranches,
            },
        };
        let else_ty = infer_expr(engine, arena, else_branch);
        let _ = engine.check_type(else_ty, &expected, arena.get_expr(else_branch).span);
        engine.pop_context();

        engine.resolve(then_ty)
    } else {
        // No else: then-branch must be void or Never (Spec: Clause 16, §16.1).
        // "the then-branch shall have type void or Never"
        let expected = Expected {
            ty: Idx::UNIT,
            origin: ExpectedOrigin::NoExpectation,
        };
        let _ = engine.check_type(then_ty, &expected, arena.get_expr(then_branch).span);
        Idx::UNIT
    }
}
