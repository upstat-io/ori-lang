//! Collection builtin dispatch declarations.

use super::super::super::StringRuntimeReturnAbi;

declare_builtins! { emitter, ctx;
    // str
    ("str", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    // `str.into() : Error` constructs the user-facing Error struct.
    ("str", "into") => emitter.emit_str_into_error(ctx.arg_vals[0], ctx.receiver_ty, ctx.dst_ty),
    ("str", "length") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "len") => emitter.emit_str_length_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "is_empty") => emitter.emit_str_is_empty_forwarded(ctx.arg_vals[0], ctx.arc_args[0]),
    ("str", "concat") => {
        if ctx.arg_vals.len() >= 2 {
            Some(emitter.emit_str_runtime_call(
                "ori_str_concat",
                ctx.arg_vals[0],
                ctx.arg_vals[1],
                StringRuntimeReturnAbi::StringSret,
            ))
        } else {
            None
        }
    },
    ("str", "to_str") => Some(ctx.arg_vals[0]),
    ("str", "debug") => emitter.emit_element_debug(ctx.arg_vals[0], ctx.receiver_ty),
    ("str", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_contains", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "starts_with") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_starts_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "ends_with") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_bool_call("ori_str_ends_with", ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "trim") => emitter.emit_str_unary_call("ori_str_trim", ctx.arg_vals[0]),
    ("str", "substring") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_substring(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "slice") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_substring(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "to_uppercase") => emitter.emit_str_unary_call("ori_str_to_uppercase", ctx.arg_vals[0]),
    ("str", "to_lowercase") => emitter.emit_str_unary_call("ori_str_to_lowercase", ctx.arg_vals[0]),
    ("str", "escape") => emitter.emit_str_unary_call("ori_str_escape_control", ctx.arg_vals[0]),
    ("str", "replace") => {
        if ctx.arg_vals.len() >= 3 {
            emitter.emit_str_replace(ctx.arg_vals[0], ctx.arg_vals[1], ctx.arg_vals[2])
        } else {
            None
        }
    },
    ("str", "repeat") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_repeat(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
    ("str", "chars") => emitter.emit_str_chars(ctx.arg_vals[0]),
    ("str", "split") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_str_split(ctx.arg_vals[0], ctx.arg_vals[1], ori_types::Idx::STR)
        } else {
            None
        }
    },
    ("str", "iter") => emitter.emit_str_iter(ctx.arg_vals[0]),
}
