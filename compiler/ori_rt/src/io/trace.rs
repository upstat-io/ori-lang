//! `Error` struct + `TraceEntry` runtime support for `?`-hop trace injection.
//!
//! Backs the `Traceable` trait: `_ori_inject_trace_entry` COW-pushes a trace
//! entry on `?`-propagation, `_ori_format_error_trace` renders the trace as a
//! string, `_ori_error_with_trace` returns an `Error` with one more entry
//! appended.

/// Traceable `TraceEntry` struct layout representation for FFI.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OriTraceEntry {
    pub function: crate::OriStr,
    pub file: crate::OriStr,
    pub line: i64,
    pub column: i64,
}

extern "C" fn trace_entry_inc_fn(ptr: *mut u8) {
    // SAFETY: `ptr` is the `elem_dec_fn`/inc-callback contract's element
    // pointer — the RC machinery guarantees it points to a live, initialized
    // `OriTraceEntry` for the duration of this call.
    let entry = unsafe { &*(ptr.cast::<OriTraceEntry>()) };
    // SAFETY: `function`/`file` are heap-variant `OriStr`s written by
    // `_ori_inject_trace_entry`; their `data`/`cap` fields are valid RC
    // buffer handles for `ori_str_rc_inc`.
    unsafe {
        let func_data = entry.function.heap.data;
        let func_cap = entry.function.heap.cap;
        crate::rc::ori_str_rc_inc(func_data, func_cap);

        let file_data = entry.file.heap.data;
        let file_cap = entry.file.heap.cap;
        crate::rc::ori_str_rc_inc(file_data, file_cap);
    }
}

extern "C" fn trace_entry_dec_fn(ptr: *mut u8) {
    // SAFETY: `ptr` is the `elem_dec_fn` callback contract's element pointer
    // — the RC machinery guarantees it points to a live, initialized
    // `OriTraceEntry` for the duration of this call.
    let entry = unsafe { &*(ptr.cast::<OriTraceEntry>()) };
    // SAFETY: `function`/`file` are heap-variant `OriStr`s; their
    // `data`/`cap` fields are valid RC buffer handles for `ori_str_rc_dec`.
    unsafe {
        let func_data = entry.function.heap.data;
        let func_cap = entry.function.heap.cap;
        crate::rc::ori_str_rc_dec(func_data, func_cap, Some(crate::rc::ori_str_drop_buffer));

        let file_data = entry.file.heap.data;
        let file_cap = entry.file.heap.cap;
        crate::rc::ori_str_rc_dec(file_data, file_cap, Some(crate::rc::ori_str_drop_buffer));
    }
}

/// Error struct layout representation for FFI.
#[repr(C)]
pub struct OriError {
    pub message: crate::OriStr,
    pub trace: crate::OriList,
}

/// Inject a trace entry into an Error struct.
///
/// `function` and `file` are passed by pointer, not by value: `OriStr` is a
/// 24-byte struct, and the `SysV` AMD64 C ABI classifies a struct larger than
/// 16 bytes as MEMORY, passed indirectly. Passing it as a raw `{i64,i64,ptr}`
/// value across the LLVM->Rust FFI boundary mis-classifies the eightbytes and
/// shifts every following argument. Mirrors `_ori_error_with_trace`, which
/// passes its struct operands by pointer for the same reason.
#[no_mangle]
pub extern "C" fn _ori_inject_trace_entry(
    error_ptr: *mut OriError,
    function: *const crate::OriStr,
    file: *const crate::OriStr,
    line: i64,
    column: i64,
) {
    if error_ptr.is_null() || function.is_null() || file.is_null() {
        return;
    }
    // SAFETY: error_ptr is verified non-null. Caller guarantees it points to a valid OriError.
    let error = unsafe { &mut *error_ptr };
    // SAFETY: function/file are verified non-null; the caller spills each OriStr
    // to an alloca and passes its pointer per the by-pointer ABI above.
    let entry = OriTraceEntry {
        function: unsafe { *function },
        file: unsafe { *file },
        line,
        column,
    };

    let mut out_list = crate::OriList {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    };
    let out_list_ptr = std::ptr::from_mut::<crate::OriList>(&mut out_list).cast::<u8>();
    let entry_bytes = std::ptr::from_ref::<OriTraceEntry>(&entry).cast::<u8>();
    let elem_size = std::mem::size_of::<OriTraceEntry>() as i64;

    // Mutation is COW-aware and consumes the trace reference owned by `error`.
    crate::ori_list_push_cow(
        error.trace.data,
        error.trace.len,
        error.trace.cap,
        entry_bytes,
        elem_size,
        std::mem::align_of::<OriTraceEntry>() as i64,
        Some(trace_entry_inc_fn),
        0, // cow_mode = dynamic (0)
        out_list_ptr,
    );

    if !out_list.data.is_null() {
        unsafe {
            crate::rc::store_elem_dec_fn(out_list.data, Some(trace_entry_dec_fn));
        }
    }
    error.trace = out_list;
}

/// Format the trace of an Error struct as a string.
#[no_mangle]
pub extern "C" fn _ori_format_error_trace(error_ptr: *const OriError) -> crate::OriStr {
    if error_ptr.is_null() {
        return crate::OriStr::from_owned("");
    }
    // SAFETY: error_ptr is verified non-null. Caller guarantees it points to a valid OriError.
    let error = unsafe { &*error_ptr };
    let trace_len = error.trace.len;
    if trace_len <= 0 || error.trace.data.is_null() {
        return crate::OriStr::from_owned("");
    }

    let mut parts = Vec::new();
    let entries = unsafe {
        std::slice::from_raw_parts(error.trace.data.cast::<OriTraceEntry>(), trace_len as usize)
    };
    for entry in entries {
        // SAFETY: entry is a valid OriTraceEntry with properly initialized string fields.
        let func = unsafe { entry.function.as_str() };
        let file = unsafe { entry.file.as_str() };
        let formatted = format!("{} at {}:{}:{}", func, file, entry.line, entry.column);
        parts.push(formatted);
    }
    let joined = parts.join("\n");
    crate::OriStr::from_owned(&joined)
}

/// Create a new Error struct with an additional trace entry appended.
#[no_mangle]
pub extern "C" fn _ori_error_with_trace(
    out_ptr: *mut OriError,
    error_ptr: *const OriError,
    entry_ptr: *const OriTraceEntry,
) {
    if out_ptr.is_null() {
        return;
    }
    if error_ptr.is_null() || entry_ptr.is_null() {
        unsafe {
            std::ptr::write(
                out_ptr,
                OriError {
                    message: crate::OriStr::from_owned(""),
                    trace: crate::OriList {
                        len: 0,
                        cap: 0,
                        data: std::ptr::null_mut(),
                    },
                },
            );
            return;
        }
    }
    // SAFETY: error_ptr and entry_ptr are verified non-null.
    let error = unsafe { &*error_ptr };
    let entry = unsafe { &*entry_ptr };

    let message = unsafe {
        let cap = error.message.heap.cap;
        let data = error.message.heap.data;
        crate::rc::ori_str_rc_inc(data, cap);
        error.message
    };

    // Increment trace's RC to transfer ownership of one reference to `ori_list_push_cow`.
    crate::rc::ori_list_rc_inc(error.trace.data, error.trace.cap);

    let entry_copy = *entry;
    unsafe {
        let func_data = entry_copy.function.heap.data;
        let func_cap = entry_copy.function.heap.cap;
        crate::rc::ori_str_rc_inc(func_data, func_cap);

        let file_data = entry_copy.file.heap.data;
        let file_cap = entry_copy.file.heap.cap;
        crate::rc::ori_str_rc_inc(file_data, file_cap);
    }

    let mut trace = crate::OriList {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    };
    let trace_out_ptr = std::ptr::from_mut::<crate::OriList>(&mut trace).cast::<u8>();
    let entry_bytes = std::ptr::from_ref::<OriTraceEntry>(&entry_copy).cast::<u8>();
    let elem_size = std::mem::size_of::<OriTraceEntry>() as i64;

    crate::ori_list_push_cow(
        error.trace.data,
        error.trace.len,
        error.trace.cap,
        entry_bytes,
        elem_size,
        std::mem::align_of::<OriTraceEntry>() as i64,
        Some(trace_entry_inc_fn),
        0, // cow_mode = dynamic (0)
        trace_out_ptr,
    );

    if !trace.data.is_null() {
        unsafe {
            crate::rc::store_elem_dec_fn(trace.data, Some(trace_entry_dec_fn));
        }
    }

    unsafe {
        std::ptr::write(out_ptr, OriError { message, trace });
    }
}
