//! Collection builtin dispatch declarations.

use ori_arc::ir::ArgOwnership;

use crate::codegen::type_info::TypeInfo;

use super::super::RenderStyle;

declare_builtins! { emitter, ctx;
    // list
    ("list", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("list", "count") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.len"),
    ("list", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "list.is_empty"),
    ("list", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "add") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_concat_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "push") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                // Same discriminator as the element-escape keep-alive:
                // a RETURNED receiver's result buffer carries the elem header (its
                // elem_dec_fn is the keep-alive's balancing release in the caller).
                let receiver_returned = ori_arc::push_receiver_lineage_returned(
                    ctx.arc_func,
                    ctx.arc_args[0],
                );
                let r = emitter.emit_list_push_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty, receiver_returned);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "prepend") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_prepend_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "first") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_first(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "last") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_last(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "flatten") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_flatten(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "pop") => {
        if let TypeInfo::List { element } = ctx.type_info {
            // Why: pop() is typed Option<T> but the removing dual-return (element +
            // modified list) needs ARC-pipeline cooperation that is not wired, so the
            // emitted form is the non-mutating last-element read.
            emitter.emit_list_last(ctx.arg_vals[0], *element, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "reverse") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_reverse_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "sort_stable") => {
        if let TypeInfo::List { element } = ctx.type_info {
            let cm = emitter.cow_mode_const(ctx.arc_func);
            let r = emitter.emit_list_sort_stable_cow(ctx.arg_vals[0], *element, cm, ctx.receiver_ty);
            if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
            r
        } else {
            None
        }
    },
    ("list", "set") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_set_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "updated") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_updated_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "insert") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_insert_cow(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_list_remove_cow(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "slice") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_slice(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "take") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_take_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "drop") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::List { element } = ctx.type_info {
                emitter.emit_list_drop_slice(ctx.arg_vals[0], ctx.arg_vals[1], *element, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("list", "debug") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_debug(ctx.arg_vals[0], *element, RenderStyle::Debug)
        } else {
            None
        }
    },
    ("list", "to_str") => {
        if let TypeInfo::List { element } = ctx.type_info {
            emitter.emit_list_debug(ctx.arg_vals[0], *element, RenderStyle::Printable)
        } else {
            None
        }
    },
    // Spec: Clause 8.2.2 gives fixed and dynamic lists the same runtime layout.
    ("list", "to_dynamic") => Some(ctx.arg_vals[0]),
    ("list", "to_fixed") => Some(ctx.arg_vals[0]),
    ("list", "iter") => {
        if let TypeInfo::List { element } = ctx.type_info {
            // INVARIANT: retained closure arguments give iterators an independent credit.
            let ownership = if !emitter.is_var_borrowed_rooted(ctx.arc_args[0])
                || emitter.iter_receiver_owns_via_contract(ctx.arc_args[0])
            {
                ArgOwnership::Owned
            } else {
                ArgOwnership::Borrowed
            };
            tracing::trace!(
                receiver = ctx.arc_args[0].index(),
                ?ownership,
                borrowed_rooted = emitter.is_var_borrowed_rooted(ctx.arc_args[0]),
                owns_via_contract = emitter.iter_receiver_owns_via_contract(ctx.arc_args[0]),
                "list iter owns_data decision"
            );
            emitter.emit_list_iter(ctx.arg_vals[0], ctx.receiver_ty, *element, ownership)
        } else {
            None
        }
    },
}
