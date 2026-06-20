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

use ori_types::Idx;

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
        ctx.variant_stack(),
    );

    match test_kind {
        TestKind::EnumTag => tag::emit_tag_switch(lowerer, scrutinee, edges, default, ctx),
        TestKind::IntEq | TestKind::BoolEq | TestKind::CharEq => {
            int::emit_int_switch(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::ListLen => {
            // A list-length test discriminates on the list's LENGTH, not the
            // list value. Extract the length (field 0 of the {len, cap, data}
            // fat pointer) into an int, then dispatch via a comparison chain:
            // exact patterns test `len == N`, rest patterns test `len >= N`.
            // A `Switch` cannot express `>=` (an exact and a rest arm of the
            // same length collide as duplicate cases), and switching on the raw
            // list feeds a {i64, i64, ptr} aggregate to the int Switch builder.
            let len_var = lowerer
                .builder
                .emit_project(Idx::INT, scrutinee, 0, Some(ctx.span));
            chain::emit_list_len_chain(lowerer, len_var, edges, default, ctx);
        }
        TestKind::StrEq | TestKind::FloatEq => {
            chain::emit_str_chain(lowerer, scrutinee, edges, default, ctx);
        }
        TestKind::IntRange => chain::emit_range_chain(lowerer, scrutinee, edges, default, ctx),
    }
}
