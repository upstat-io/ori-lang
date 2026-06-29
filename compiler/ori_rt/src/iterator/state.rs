//! Iterator state machine and type definitions.
//!
//! `IterState` is the internal enum that drives all iterator operations.
//! Never exposed to LLVM — all interaction goes through pointer-sized handles.

/// Maximum element size for stack scratch buffers in `next()`.
///
/// Covers all current Ori types (str=24B, list=24B, practical structs <200B).
/// Asserted at source/adapter creation time via `assert_elem_size`.
pub(crate) const MAX_ELEM_SIZE: usize = 256;

/// Stack scratch buffer for one iterator element, 16-byte aligned.
///
/// A bare `[u8; MAX_ELEM_SIZE]` array has alignment 1; an element written into
/// it (e.g. a 24-byte `OriStr` fat pointer, 8-byte aligned) is then read back by
/// a consumer/predicate as a typed value, and creating a Rust reference to a
/// misaligned address is UB. `align(16)` covers every Ori value type's
/// alignment. `Deref`/`DerefMut` to the inner array keep every existing
/// `as_ptr` / `as_mut_ptr` / slice / `&mut` use site unchanged.
#[repr(C, align(16))]
pub(crate) struct ElemBuf([u8; MAX_ELEM_SIZE]);

// INVARIANT: the 16-byte alignment is load-bearing for UB prevention — drop it and
// element reads through the scratch buffer become misaligned. Pin it so removing
// `align(16)` is a compile error, not a silent regression.
const _: () = assert!(core::mem::align_of::<ElemBuf>() == 16);

impl ElemBuf {
    #[inline]
    pub(crate) const fn new() -> Self {
        Self([0u8; MAX_ELEM_SIZE])
    }
}

impl core::ops::Deref for ElemBuf {
    type Target = [u8; MAX_ELEM_SIZE];
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for ElemBuf {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Assert that an element size fits in the stack scratch buffer.
///
/// Called at iterator source/adapter creation time to catch oversized elements
/// before any `[0u8; MAX_ELEM_SIZE]` buffer is used.
///
/// Uses `assert!` (not `debug_assert!`) because the scratch buffers are
/// fixed-size `[0u8; MAX_ELEM_SIZE]` in both debug and release builds —
/// an oversized element causes a stack buffer overflow in release if unchecked.
#[inline]
pub(crate) fn assert_elem_size(elem_size: i64, context: &str) {
    assert!(
        elem_size >= 0 && (elem_size as usize) <= MAX_ELEM_SIZE,
        "{context}: element size {elem_size} exceeds MAX_ELEM_SIZE ({MAX_ELEM_SIZE})"
    );
}

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
    ///
    /// Element cleanup is entirely header-based: `ori_buffer_rc_dec` reads
    /// `elem_dec_fn` from the V5 RC header at cleanup time.
    ///
    /// `owns_data` records the ARC `@iter` arg-ownership decision: `true` when
    /// the iterator received its own RC reference (moved/inc'd source), `false`
    /// when the source is borrowed-co-owned (the flatten inner `sub.iter()`
    /// runs inside an opaque map trampoline so the ARC pipeline cannot inc — the
    /// outer container retains the single RC and frees the buffer once). `Drop`
    /// decs only when `owns_data`; a borrowed iterator drops without dec.
    List {
        data: *mut u8,
        len: i64,
        pos: i64,
        cap: i64,
        elem_size: i64,
        owns_data: bool,
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

    /// Flattens a nested iterator (iterator of iterators) into a single stream.
    ///
    /// Tracks the outer source iterator and the current inner iterator.
    /// When the inner is exhausted, advances the outer to get the next inner.
    Flattened {
        source: Box<IterState>,
        inner: Option<Box<IterState>>,
        inner_elem_size: i64,
    },

    /// Cycles through elements by buffering the first pass and replaying.
    ///
    /// On the first pass, elements are collected into `buffer`. Once the source
    /// is exhausted, subsequent iterations replay from the buffer.
    ///
    /// `buffer` OWNS its element copies: each element is inc'd via `elem_inc_fn`
    /// when stored (so the buffered fat-pointer aliases a live allocation
    /// independent of the source-free that happens on exhaustion AND independent
    /// of consumer behavior), and `Drop` decs every stored master via
    /// `elem_dec_fn`. Null for scalar elements (no RC). The consumer's per-yield
    /// inc (e.g. `ori_iter_collect`'s `elem_inc_fn`) is a SEPARATE ownership
    /// domain covering the yielded aliases; the buffer never yields ownership.
    Cycled {
        source: Option<Box<IterState>>,
        buffer: Vec<u8>,
        buf_pos: usize,
        elem_size: i64,
        source_exhausted: bool,
        elem_inc_fn: Option<extern "C" fn(*mut u8)>,
        elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    },

    /// Reverses iteration by collecting all elements then iterating backward.
    ///
    /// `elements` OWNS its copies: `ori_iter_rev` incs each at collect time (the
    /// source is freed immediately after, so the collected fat-pointers must
    /// hold their own ref), and `Drop` decs every stored master via
    /// `elem_dec_fn`. Null for scalar elements. The inc happens once at collect
    /// (single-pass), so no stored `elem_inc_fn` is needed on this variant.
    ///
    /// Double-ended: the un-yielded window is `[front, pos)`. `next` pops the
    /// high end (`pos -= 1`, yielding `elements` in reverse), `next_back` pops
    /// the low end (`front += 1`, yielding in source order). Drop decs every
    /// stored master regardless of the window (copies were handed out, masters
    /// stay until teardown).
    Reversed {
        elements: Vec<u8>,
        pos: i64,
        front: i64,
        elem_size: i64,
        elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    },

    /// Iterates over a UTF-8 string, yielding Unicode codepoints (i32/char).
    ///
    /// When `owns_data` is true, the iterator holds an RC reference to the
    /// string data and `Drop` calls `ori_str_rc_dec` to release it. The `cap`
    /// field carries the string's capacity (with possible `SLICE_FLAG`) so that
    /// slice strings from `str.split()` are cleaned up correctly.
    Str {
        data: *mut u8,
        len: i64,
        cap: i64,
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

    /// Infinitely yields copies of a single owned master value.
    ///
    /// Mirrors the interpreter's `IteratorValue::Repeat`. `value` holds the
    /// elem_size-byte master; `ori_iter_repeat`'s caller (codegen) inc'd the
    /// value's RC once before construction so the master owns a reference
    /// independent of the (borrowed) source binding. Each `next()` yields a
    /// bitwise copy of the master WITHOUT incrementing — the consumer's
    /// per-yield inc (e.g. `ori_iter_collect`'s `elem_inc_fn`) covers the
    /// yielded aliases, identical to the `Cycled` ownership protocol. `Drop`
    /// decs the master exactly once via `elem_dec_fn` (null for scalars).
    Repeat {
        value: Vec<u8>,
        elem_size: i64,
        elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    },
}

/// Dec every owned element master in a flat `elem_size`-byte buffer exactly
/// once via `dec`, regardless of cursor position. Shared by the `Cycled` and
/// `Reversed` Drop arms — each OWNS its stored copies (`dec` is the V5-header
/// `elem_dec_fn`), so the release count is `buffer.len() / elem_size`.
fn dec_owned_buffer(buffer: &mut [u8], elem_size: i64, dec: extern "C" fn(*mut u8)) {
    let es = elem_size.max(1) as usize;
    let n = buffer.len() / es;
    for i in 0..n {
        // SAFETY: buffer holds n contiguous es-byte masters.
        let elem = unsafe { buffer.as_mut_ptr().add(i * es) };
        dec(elem);
    }
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
                owns_data,
                ..
            } => {
                // owns_data gates the dec on the ARC arg-ownership: a borrowed
                // iterator (the flatten inner sub.iter() co-owned by an outer
                // list) drops without dec so the outer elem_dec_fn frees once.
                // cap != 0 indicates RC-managed data (from the compiler):
                //   cap > 0 → regular list (cap is capacity)
                //   cap < 0 → seamless slice (SLICE_FLAG set, ori_buffer_rc_dec handles it)
                // cap == 0 indicates test data (stack-allocated, no cleanup).
                // elem_dec_fn is read from the V5 RC header by ori_buffer_rc_dec.
                if *owns_data && !data.is_null() && *cap != 0 {
                    crate::ori_buffer_rc_dec(*data, *len, *cap, *elem_size, None);
                }
            }
            IterState::Str {
                data,
                cap,
                owns_data,
                ..
            } => {
                if *owns_data && !data.is_null() {
                    if crate::slice_encoding::is_slice_cap(*cap) {
                        // Seamless slice from str.split(): data is an interior
                        // pointer. Compute original buffer and dec its RC.
                        // ori_buffer_rc_dec handles free on rc=0 using
                        // stored data_size.
                        let original = crate::slice_encoding::slice_original_data(*data, *cap);
                        // len=0 (no inner RC elements), cap=data_size (from
                        // header), elem_size=1.
                        let data_size = crate::ori_rc_data_size(original.cast_const());
                        crate::ori_buffer_rc_dec(original, 0, data_size, 1, None);
                    } else {
                        // Regular heap string or SSO copy: data is the start
                        // of an RC allocation. cap is the byte capacity.
                        // len=0 (no inner RC elements), elem_size=1.
                        crate::ori_buffer_rc_dec(*data, 0, *cap, 1, None);
                    }
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
            IterState::Cycled {
                buffer,
                elem_size,
                elem_dec_fn: Some(dec),
                ..
            } => {
                // The buffer OWNS its element copies (inc'd on store in
                // next_cycled). The source Option<Box> auto-drops after this
                // body (gated by its own owns_data).
                dec_owned_buffer(buffer, *elem_size, *dec);
            }
            IterState::Reversed {
                elements,
                elem_size,
                elem_dec_fn: Some(dec),
                ..
            } => {
                // The elements buffer OWNS its copies (inc'd at collect time in
                // ori_iter_rev).
                dec_owned_buffer(elements, *elem_size, *dec);
            }
            IterState::Repeat {
                value,
                elem_size,
                elem_dec_fn: Some(dec),
            } => {
                // The master OWNS one copy (the caller inc'd the value's RC
                // before construction). Release it exactly once; yielded
                // aliases are owned by the consumer's per-yield inc.
                dec_owned_buffer(value, *elem_size, *dec);
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
