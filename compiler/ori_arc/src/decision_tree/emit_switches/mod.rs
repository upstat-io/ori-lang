//! Switch emitters for decision tree pattern matching.
//!
//! Handles tag switch (enum dispatch), int/bool/char switch, string/float
//! equality chains, range check chains, and the select chain optimization
//! for branchless trivial matches.
//!
//! ## Submodules
//!
//! - [`tag`] — enum tag switch via `Switch` terminator
//! - [`int`] — int/bool/char/list-length switch via `Switch` terminator
//! - [`chain`] — string/float equality + range check via `Branch` chains
//! - [`select`] — branchless select chain optimization

mod chain;
mod int;
mod select;
mod tag;

use super::emit::EmitContext;
use super::{ScrutineePath, TestKind};

/// Dispatch to the appropriate switch emitter based on the test kind.
pub(super) fn emit_switch(
    lowerer: &mut crate::lower::ArcLowerer<'_>,
    path: &ScrutineePath,
    test_kind: TestKind,
    edges: &[(super::TestValue, super::DecisionTree)],
    default: Option<&super::DecisionTree>,
    ctx: &mut EmitContext,
) {
    let scrutinee = super::emit::resolve_path(
        lowerer,
        ctx.root_scrutinee,
        ctx.root_scrutinee_ty,
        path,
        ctx.span,
        &ctx.variant_stack,
    );

    match test_kind {
        TestKind::EnumTag => tag::emit_tag_switch(lowerer, scrutinee, edges, default, ctx),
        TestKind::IntEq | TestKind::BoolEq | TestKind::CharEq | TestKind::ListLen => {
            int::emit_int_switch(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::StrEq | TestKind::FloatEq => {
            chain::emit_str_chain(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::IntRange => chain::emit_range_chain(lowerer, scrutinee, edges, default, ctx),
    }
}
