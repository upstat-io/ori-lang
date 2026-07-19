//! Trait method codegen for compound types (Option, Result, Tuple, List).
//!
//! Implements `equals`, `compare`, and `hash` for compound types by
//! structural recursion into element types. Each method dispatches to
//! the element's trait implementation via `emit_element_*` helpers.
//!
//! ## ARC enum convention
//!
//! - **Option**: `{i64 tag, T payload}` — Some=0, None=1
//! - **Result**: `{i64 tag, payload}`   — Ok=0, Err=1
//! - **Tuple**:  `{A, B, ...}`         — flat struct of resolved element types
//! - **List**:   `{i64 len, i64 cap, ptr data}` — element-wise iteration

use crate::codegen::type_info::TypeInfo;

use super::RenderStyle;

declare_builtins! { emitter, ctx;
    // list structural trait methods
    ("list", "equals") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "is_equal") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "compare") => {
        if ctx.arg_vals.len() >= 2 {
            emitter.emit_element_compare(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty)
        } else {
            None
        }
    },
    ("list", "hash") => emitter.emit_element_hash(ctx.arg_vals[0], ctx.receiver_ty),
    // Map structural trait methods
    ("map", "equals") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_map_equals(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "is_equal") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_map_equals(ctx.arg_vals[0], ctx.arg_vals[1], *key, *value)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("map", "hash") => {
        if let TypeInfo::Map { key, value } = ctx.type_info {
            emitter.emit_map_hash(ctx.arg_vals[0], *key, *value)
        } else {
            None
        }
    },
    // Set structural trait methods
    ("Set", "equals") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_set_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *element)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Set", "hash") => {
        if let TypeInfo::Set { element } = ctx.type_info {
            emitter.emit_set_hash(ctx.arg_vals[0], ctx.receiver_ty, *element)
        } else {
            None
        }
    },
    // Option structural trait methods
    ("Option", "equals") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "is_equal") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_equals(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "compare") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_option_compare(ctx.arg_vals[0], ctx.arg_vals[1], *inner)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Option", "hash") => {
        if let TypeInfo::Option { inner } = ctx.type_info {
            emitter.emit_option_hash(ctx.arg_vals[0], *inner)
        } else {
            None
        }
    },
    // Result structural trait methods
    ("Result", "equals") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "is_equal") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_equals(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "compare") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_result_compare(ctx.arg_vals[0], ctx.arg_vals[1], ctx.receiver_ty, *ok, *err)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("Result", "hash") => {
        if let TypeInfo::Result { ok, err } = ctx.type_info {
            emitter.emit_result_hash(ctx.arg_vals[0], ctx.receiver_ty, *ok, *err)
        } else {
            None
        }
    },
    // tuple structural trait methods
    ("tuple", "clone") => emitter.emit_rc_inc_clone(ctx.arg_vals[0], ctx.receiver_ty),
    ("tuple", "equals") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_equals(ctx.arg_vals[0], ctx.arg_vals[1], elements, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "compare") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            if ctx.arg_vals.len() >= 2 {
                emitter.emit_tuple_compare(ctx.arg_vals[0], ctx.arg_vals[1], elements, ctx.receiver_ty)
            } else {
                None
            }
        } else {
            None
        }
    },
    ("tuple", "hash") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            emitter.emit_tuple_hash(ctx.arg_vals[0], elements, ctx.receiver_ty)
        } else {
            None
        }
    },
    ("tuple", "debug") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            let elements = elements.clone();
            emitter.emit_tuple_debug(
                ctx.arg_vals[0],
                &elements,
                ctx.receiver_ty,
                RenderStyle::Debug,
            )
        } else {
            None
        }
    },
    ("tuple", "to_str") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            let elements = elements.clone();
            emitter.emit_tuple_debug(
                ctx.arg_vals[0],
                &elements,
                ctx.receiver_ty,
                RenderStyle::Printable,
            )
        } else {
            None
        }
    },
    // Tuple length is the compile-time arity (a fixed-shape product type).
    ("tuple", "len") => {
        if let TypeInfo::Tuple { elements } = ctx.type_info {
            Some(emitter.builder.const_i64(elements.len() as i64))
        } else {
            None
        }
    },
}
