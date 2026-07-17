//! Collection builtin dispatch declarations.

declare_builtins! { emitter, ctx;
    // range
    ("range", "iter") => emitter.emit_range_iter(ctx.arg_vals[0]),
    ("range", "len") => emitter.emit_range_len(ctx.arg_vals[0]),
    ("range", "count") => emitter.emit_range_len(ctx.arg_vals[0]),
    ("range", "contains") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_range_contains(ctx.arg_vals[0], ctx.arg_vals[1])
        } else {
            None
        }
    },
}
