//! Type-annotation rendering for typed-IR expression dumps.
//!
//! Computes the per-node decorations the `ir_dump` expression walker appends:
//! the ` : type` suffix, the method-dispatch `[builtin: ...]` hint, and the
//! `(unresolved)` unification-status marker.

use ori_ir::ExprId;
use ori_types::{Idx, Pool, Tag};

use super::{fmt_ty, DumpCtx};

/// Format the type annotation suffix for an expression.
///
/// Returns ` : type_name` if the expression has a resolved type, or
/// ` : ?` if the expression is untyped (error nodes, out of range).
pub(super) fn type_of(id: ExprId, ctx: &DumpCtx) -> String {
    let DumpCtx {
        typed,
        pool,
        interner,
        with_idx,
        ..
    } = *ctx;
    match typed.expr_type(id.index()) {
        Some(idx) => {
            let ts = fmt_ty(pool, idx, interner, with_idx);
            let hint = unification_hint(idx, pool);
            format!(" : {ts}{hint}")
        }
        None => " : ?".to_string(),
    }
}

/// Classify a method call's dispatch target based on the receiver's type.
///
/// Returns a dispatch hint string like `"  [builtin: list]"` or `""`.
/// Uses the receiver type's tag to determine if the method is a builtin
/// (primitive or collection method) or a user-defined method (inherent/trait).
pub(super) fn dispatch_hint(receiver_id: ExprId, ctx: &DumpCtx) -> String {
    let DumpCtx { typed, pool, .. } = *ctx;
    let Some(receiver_idx) = typed.expr_type(receiver_id.index()) else {
        return String::new();
    };
    let tag = pool.tag(receiver_idx);

    // Builtin types have compiler-provided methods
    if tag.is_primitive() || tag.is_container() {
        let category = tag_to_method_category(tag);
        format!("  [builtin: {category}]")
    } else {
        String::new()
    }
}

/// Map a Pool Tag to the type category string used in method dispatch.
///
/// These names match the type names used in `ori_registry::BUILTIN_TYPES`.
fn tag_to_method_category(tag: Tag) -> &'static str {
    match tag {
        Tag::Int => "int",
        Tag::Float => "float",
        Tag::Bool => "bool",
        Tag::Str => "str",
        Tag::Char => "char",
        Tag::Byte => "byte",
        Tag::Unit => "unit",
        Tag::Never => "Never",
        Tag::Duration => "Duration",
        Tag::Size => "Size",
        Tag::Ordering => "Ordering",
        Tag::Error => "error",
        Tag::List => "list",
        Tag::Map => "map",
        Tag::Set => "set",
        Tag::Option => "Option",
        Tag::Result => "Result",
        Tag::Range => "range",
        Tag::Channel => "Channel",
        Tag::Tuple => "tuple",
        Tag::Iterator => "Iterator",
        Tag::DoubleEndedIterator => "DoubleEndedIterator",
        _ => "builtin",
    }
}

/// Classify a type's unification status for display.
///
/// Returns ` (unresolved)` for type variables that weren't fully unified
/// to a concrete type. Variables that were linked (unified) are NOT flagged —
/// `format_type_resolved` already follows the link chain to show the concrete type.
fn unification_hint(idx: Idx, pool: &Pool) -> &'static str {
    let tag = pool.tag(idx);
    if !tag.is_type_variable() {
        return "";
    }
    // Check if this variable was unified (linked) to a concrete type.
    // VarState::Link means it was resolved; Unbound means it wasn't.
    let var_id = pool.data(idx);
    match pool.var_state(var_id) {
        ori_types::VarState::Link { .. } => "",
        _ => " (unresolved)",
    }
}
