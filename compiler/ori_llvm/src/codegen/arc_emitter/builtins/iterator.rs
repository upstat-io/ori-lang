//! Iterator adapter and consumer builtin methods.
//!
//! All iterator methods receive `ptr` (opaque iterator handle) as receiver.
//! Simple adapters (take, skip, chain, enumerate, zip) are direct runtime
//! calls. Closure adapters (map, filter) need trampolines to bridge Ori
//! closures to C-ABI function pointers.

declare_builtins! { emitter, ctx;
    // Internal iteration protocol (dead code path — __iter_next is intercepted
    // by try_emit_protocol before reaching builtin method dispatch)
    ("Iterator", "__iter_next") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iter_next(ctx.arg_vals[0], *element).map(|(tag, _, _)| tag)
        } else {
            None
        }
    },
    // Raw protocol methods — return the (Option<T>, Self) tuple
    ("Iterator", "next") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iter_next_protocol(ctx.arg_vals[0], *element, ctx.dst_ty)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "next_back") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iter_next_back_protocol(ctx.arg_vals[0], *element, ctx.dst_ty)
        } else {
            None
        }
    },
    // Simple adapters
    ("Iterator", "take") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "skip") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "chain") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "enumerate") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "zip") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Closure adapters
    ("Iterator", "map") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "filter") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Closure/simple adapters: new
    ("Iterator", "flatten") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "flat_map") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "cycle") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // DEI adapters/consumers
    ("DoubleEndedIterator", "rev") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "last") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "rfind") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("DoubleEndedIterator", "rfold") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Consumer: join (Iterator, not DEI)
    ("Iterator", "join") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    // Consumers
    ("Iterator", "collect") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "count") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "any") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "all") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "find") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "for_each") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
    ("Iterator", "fold") => {
        if let TypeInfo::Iterator { element } = ctx.type_info {
            emitter.emit_iterator_method(ctx.method, ctx.arg_vals, ctx.arc_args, ctx.arc_func, *element)
        } else {
            None
        }
    },
}

use crate::codegen::type_info::TypeInfo;
