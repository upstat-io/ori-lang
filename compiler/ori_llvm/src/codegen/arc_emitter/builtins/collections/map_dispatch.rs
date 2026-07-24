//! Collection builtin dispatch declarations.

use ori_arc::ir::ArgOwnership;

use crate::codegen::type_info::TypeInfo;

use super::super::RenderStyle;

declare_builtins! { emitter, ctx;
    // map
    ("map", "debug") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_debug(
                ctx.arg_vals[0],
                ctx.receiver_ty,
                *key,
                *value,
                RenderStyle::Debug,
            )
        } else {
            None
        }
    },
    ("map", "to_str") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_debug(
                ctx.arg_vals[0],
                ctx.receiver_ty,
                *key,
                *value,
                RenderStyle::Printable,
            )
        } else {
            None
        }
    },
    ("map", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("map", "length") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.len"),
    ("map", "len") => emitter.emit_collection_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.len"),
    ("map", "is_empty") => emitter.emit_collection_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0], "map.is_empty"),
    ("map", "contains_key") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, .. } = ctx.type_info {
                emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1], *key, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    // `map.contains(key)` is the `Contains`-trait spelling of `contains_key`.
    ("map", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, .. } = ctx.type_info {
                emitter.emit_map_contains_key(ctx.arg_vals[0], ctx.arg_vals[1], *key, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "keys") => {
        if let TypeInfo::Map { key, .. } = ctx.type_info {
            emitter.emit_map_keys(ctx.arg_vals[0], *key, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("map", "values") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_values(ctx.arg_vals[0], *key, *value, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("map", "get") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                emitter.emit_map_get(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, Some(ctx.receiver_ty))
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "insert") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_insert(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "merge") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_merge(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "updated") => {
        if ctx.arg_vals.len() >= 3 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_updated(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "iter") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            // Same credit gate as the list path. Borrowed-rooted receivers stay
            // non-owning unless the final callee contract demanded a whole-value
            // credit (for example, one retained by a closure adapter).
            let ownership = if !emitter.is_var_borrowed_rooted(ctx.arc_args[0])
                || emitter.iter_receiver_owns_via_contract(ctx.arc_args[0])
            {
                ArgOwnership::Owned
            } else {
                ArgOwnership::Borrowed
            };
            emitter.emit_map_iter(ctx.arg_vals[0], *key, *value, ctx.receiver_ty, ownership)
        } else {
            None
        }
    },
    ("map", "remove") => {
        if ctx.arg_vals.len() >= 2 {
            if let TypeInfo::Map { key, value } = ctx.type_info {
                let cm = emitter.cow_mode_const(ctx.arc_func);
                let r = emitter.emit_map_remove(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value, cm, ctx.receiver_ty);
                if r.is_some() { emitter.mark_cow_data_noalias_if_unique(ctx.arc_func); }
                r
            } else {
                None
            }
        } else {
            None
        }
    },
}
