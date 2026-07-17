//! Collection builtin dispatch declarations.

use crate::codegen::type_info::TypeInfo;

declare_builtins! { emitter, ctx;
    // Set
    ("Set", "debug") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_debug(ctx.arg_vals[0], *element, true)
        } else {
            None
        }
    },
    ("Set", "to_str") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_debug(ctx.arg_vals[0], *element, false)
        } else {
            None
        }
    },
    ("Set", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("Set", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.len"),
    ("Set", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.len"),
    ("Set", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "set.is_empty"),
    ("Set", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                emitter.emit_set_contains(ctx.arg_vals[0], ctx.arg_vals[1], *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "insert") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_insert(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_remove(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "union") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_union(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "intersection") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_intersection(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "difference") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Set { element } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_set_difference(ctx.arg_vals[0], ctx.arg_vals[1], *element, cm);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "to_list") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_to_list(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("Set", "into") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_to_list(ctx.arg_vals[0], *element)
        } else {
            None
        }
    },
    ("Set", "iter") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            // Same credit gate as list/map: a borrowed-rooted receiver is
            // non-owning unless the final callee contract demanded an independent
            // whole-value credit for this invocation.
            let receiver_owned = !emitter.is_var_borrowed_rooted(ctx.arc_args[0])
                || emitter.iter_receiver_owns_via_contract(ctx.arc_args[0]);
            emitter.emit_set_iter(ctx.arg_vals[0], *element, receiver_owned)
        } else {
            None
        }
    },
}
