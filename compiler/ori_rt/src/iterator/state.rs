//! Iterator state machine and type definitions.
//!
//! `IterState` is the internal enum that drives all iterator operations.
//! Never exposed to LLVM — all interaction goes through pointer-sized handles.

/// Maximum element size for stack scratch buffers in `next()`.
///
/// Covers all current Ori types. Asserted at adapter creation time.
pub(crate) const MAX_ELEM_SIZE: usize = 256;

/// Trampoline signature for map: `(env, in_ptr, out_ptr) -> void`
pub(crate) type TransformFn = extern "C" fn(*mut u8, *const u8, *mut u8);

/// Trampoline signature for filter/any/all/find: `(env, elem_ptr) -> bool`
pub(crate) type PredicateFn = extern "C" fn(*mut u8, *const u8) -> bool;

/// Trampoline signature for `for_each`: `(env, elem_ptr) -> void`
pub(crate) type ForEachFn = extern "C" fn(*mut u8, *const u8);

/// Trampoline signature for fold: `(env, acc_ptr, elem_ptr, out_ptr) -> void`
pub(crate) type FoldFn = extern "C" fn(*mut u8, *const u8, *const u8, *mut u8);

/// Iterator state machine. Each variant corresponds to an iterator source
/// or adapter from the evaluator's `IteratorValue` enum.
pub(crate) enum IterState {
    /// Iterates over a contiguous array of elements (list data buffer).
    ///
    /// When `cap != 0`, the iterator owns a reference to the RC-managed data
    /// buffer. `Drop` calls `ori_buffer_rc_dec` to release it. This handles
    /// both regular lists (`cap > 0`) and seamless slices (`cap < 0`, where
    /// the `SLICE_FLAG` is set). When `cap == 0` (e.g., Rust unit tests with
    /// stack data), no cleanup is performed.
    List {
        data: *mut u8,
        len: i64,
        pos: i64,
        cap: i64,
        elem_size: i64,
        elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    },

    /// Iterates over an integer range with step.
    Range {
        current: i64,
        end: i64,
        step: i64,
        inclusive: bool,
    },

    /// Transforms each element via a trampoline function.
    Mapped {
        source: Box<IterState>,
        transform_fn: TransformFn,
        transform_env: *mut u8,
        in_size: i64,
    },

    /// Filters elements via a predicate trampoline.
    Filtered {
        source: Box<IterState>,
        predicate_fn: PredicateFn,
        predicate_env: *mut u8,
        elem_size: i64,
    },

    /// Takes at most N elements from source.
    TakeN {
        source: Box<IterState>,
        remaining: i64,
    },

    /// Skips N elements then delegates to source.
    SkipN {
        source: Box<IterState>,
        remaining: i64,
    },

    /// Wraps each element with its index: (index, element).
    Enumerated { source: Box<IterState>, index: i64 },

    /// Zips two iterators, yielding `(left_elem, right_elem)` tuples.
    Zipped {
        left: Box<IterState>,
        right: Box<IterState>,
        left_elem_size: i64,
    },

    /// Chains two iterators — yields all of first, then all of second.
    Chained {
        first: Box<IterState>,
        second: Box<IterState>,
        first_done: bool,
    },

    /// Iterates over a UTF-8 string, yielding Unicode codepoints (i32/char).
    ///
    /// When `owns_data` is true, the iterator holds an RC reference to the
    /// string data and `Drop` calls `ori_buffer_rc_dec` to release it (dec
    /// refcount + free when rc reaches 0). When false (e.g., Rust unit
    /// tests), no cleanup is performed.
    Str {
        data: *mut u8,
        len: i64,
        byte_offset: i64,
        owns_data: bool,
    },

    /// Iterates over a map's key-value pairs, yielding `(key, value)` tuples.
    ///
    /// Data layout (hash table): `[metadata | keys | values]`. The iterator
    /// scans metadata for OCCUPIED buckets. `pos` is the current bucket index.
    /// When `owns_data` is true, the iterator holds an RC reference to the
    /// combined data buffer. `Drop` calls `ori_map_buffer_rc_dec` to clean up.
    Map {
        data: *mut u8,
        cap: i64,
        len: i64,
        pos: i64,
        key_size: i64,
        val_size: i64,
        owns_data: bool,
        key_dec_fn: Option<extern "C" fn(*mut u8)>,
        val_dec_fn: Option<extern "C" fn(*mut u8)>,
    },
}

impl Drop for IterState {
    fn drop(&mut self) {
        // Release RC references to data owned by source-level iterators.
        //
        // The ARC pipeline transfers ownership of one RC reference to the
        // iterator when `.iter()` is called (Owned semantics). We release
        // it here so the data is freed when the last iterator reference
        // goes away.
        //
        // For adapter variants (Mapped, Filtered, etc.), Rust automatically
        // drops the inner `Box<IterState>` AFTER this `drop()` returns,
        // cascading the cleanup to the source iterator.
        match self {
            IterState::List {
                data,
                len,
                cap,
                elem_size,
                elem_dec_fn,
                ..
            } => {
                // cap != 0 indicates RC-managed data (from the compiler):
                //   cap > 0 → regular list (cap is capacity)
                //   cap < 0 → seamless slice (SLICE_FLAG set, ori_buffer_rc_dec handles it)
                // cap == 0 indicates test data (stack-allocated, no cleanup).
                if !data.is_null() && *cap != 0 {
                    crate::ori_buffer_rc_dec(*data, *len, *cap, *elem_size, *elem_dec_fn);
                }
            }
            IterState::Str {
                data,
                len,
                owns_data,
                ..
            } => {
                // String data is allocated via ori_rc_alloc (in ori_str_from_raw),
                // so we must use ori_buffer_rc_dec to both dec the refcount AND
                // free the memory when rc reaches 0. ori_rc_dec alone only decs
                // the refcount without freeing.
                // len=0 (no inner RC elements), cap=string byte length, elem_size=1.
                if *owns_data && !data.is_null() {
                    crate::ori_buffer_rc_dec(*data, 0, *len, 1, None);
                }
            }
            IterState::Map {
                data,
                cap,
                len,
                key_size,
                val_size,
                owns_data,
                key_dec_fn,
                val_dec_fn,
                ..
            } => {
                // Map data buffer uses hash table layout [metadata|keys|values].
                // ori_map_buffer_rc_dec decs the refcount, scans metadata for
                // OCCUPIED buckets to clean up key/value children, and frees
                // the buffer when rc reaches 0.
                if *owns_data && !data.is_null() {
                    crate::ori_map_buffer_rc_dec(
                        *data,
                        *cap,
                        *len,
                        *key_size,
                        *val_size,
                        *key_dec_fn,
                        *val_dec_fn,
                    );
                }
            }
            _ => {}
        }
    }
}

/// Create an empty range iterator (yields nothing). Used as placeholder
/// in chain adapters when one side is null.
pub(super) fn empty_range() -> IterState {
    IterState::Range {
        current: 0,
        end: 0,
        step: 1,
        inclusive: false,
    }
}

// Ensure IterState is Send (function pointers and raw pointers are Send).
// Required for the unsafe `Box::from_raw` / `Box::into_raw` dance.
unsafe impl Send for IterState {}
