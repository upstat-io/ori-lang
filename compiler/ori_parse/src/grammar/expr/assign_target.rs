//! Assignment-target classification.
//!
//! Given an already-parsed left-hand side at assignment position, classifies it
//! into either a bare identifier (kept as-is) or an `AssignTarget` chain:
//! a root `Ident` followed by one or more `[index]` / `.field` access steps.
//! Invalid targets (non-`Ident` root, or a non-index/non-field node in the
//! chain) are rejected with `E1020`.

use crate::{ParseError, Parser};
use ori_ir::{AccessStep, AccessStepRange, Expr, ExprId, ExprKind};

/// Outcome of classifying an assignment LHS.
pub(super) enum AssignLhs {
    /// Bare identifier: keep `Assign { target: <Ident> }` unchanged (DD-5 zero-step guard).
    BareIdent,
    /// Root `Ident` plus a root-to-leaf access-step chain (>=1 step).
    Chain {
        root: ExprId,
        steps: AccessStepRange,
    },
}

impl Parser<'_> {
    /// Classify an assignment LHS into a bare identifier or an access-step chain.
    ///
    /// Walks `left` outermost-to-root, collecting `Index` / `Field` steps; the
    /// root must be a bare `Ident`. Returns `E1020` on an invalid target.
    pub(super) fn classify_assign_target(&mut self, left: ExprId) -> Result<AssignLhs, ParseError> {
        // Collect steps outermost-first while descending into receivers.
        let mut steps_outer_first: Vec<AccessStep> = Vec::new();
        let mut current = left;

        loop {
            match self.arena.get_expr(current).kind {
                ExprKind::Index { receiver, index } => {
                    steps_outer_first.push(AccessStep::Index(index));
                    current = receiver;
                }
                ExprKind::Field { receiver, field } => {
                    steps_outer_first.push(AccessStep::Field(field));
                    current = receiver;
                }
                ExprKind::Ident(_) => break,
                _ => {
                    return Err(self.invalid_assign_target(left));
                }
            }
        }

        // `current` is the root Ident; `left` was a bare Ident iff no steps collected.
        if steps_outer_first.is_empty() {
            return Ok(AssignLhs::BareIdent);
        }

        // Steps were collected outermost-first; reverse to root-to-leaf order.
        // The step `ExprId`s are reused from the parsed chain (shared identity).
        let start = self.arena.start_access_steps();
        for step in steps_outer_first.into_iter().rev() {
            self.arena.push_access_step(step);
        }
        let steps = self.arena.finish_access_steps(start);

        Ok(AssignLhs::Chain {
            root: current,
            steps,
        })
    }

    /// Resolve an assignment LHS to the `target` `ExprId` for `ExprKind::Assign`.
    ///
    /// Bare identifier (zero steps) yields `left` unchanged; a chain of >=1
    /// `[index]` / `.field` step yields a freshly-allocated `AssignTarget`
    /// spanning `left`. Invalid targets return `E1020`.
    pub(super) fn assign_target_node(&mut self, left: ExprId) -> Result<ExprId, ParseError> {
        match self.classify_assign_target(left)? {
            AssignLhs::BareIdent => Ok(left),
            AssignLhs::Chain { root, steps } => {
                let span = self.arena.get_expr(left).span;
                Ok(self
                    .arena
                    .alloc_expr(Expr::new(ExprKind::AssignTarget { root, steps }, span)))
            }
        }
    }

    /// Build the `E1020` invalid-assignment-target error spanning `left`.
    fn invalid_assign_target(&self, left: ExprId) -> ParseError {
        let span = self.arena.get_expr(left).span;
        ParseError::new(
            ori_diagnostic::ErrorCode::E1020,
            "invalid assignment target: left-hand side must be an identifier optionally \
             followed by `[index]` or `.field` accessors"
                .to_string(),
            span,
        )
    }
}
