//! Iterator state machine and type definitions.
//!
//! `IterState` is the internal enum that drives all iterator operations.
//! Never exposed to LLVM — all interaction goes through pointer-sized handles.

#[path = "state_lifecycle.rs"]
mod lifecycle;

use core::{mem::size_of, ptr};

pub(super) use lifecycle::empty_range;
pub(crate) use lifecycle::YieldGuard;

/// Stack scratch buffer for one iterator element, 16-byte aligned.
///
/// A bare byte array has alignment 1; an element written into
/// it (e.g. a 24-byte `OriStr` fat pointer, 8-byte aligned) is then read back by
/// a consumer/predicate as a typed value, and creating a Rust reference to a
/// misaligned address is UB. `align(16)` covers every Ori value type's
/// alignment. `Deref`/`DerefMut` to the inner array keep every existing
/// `as_ptr` / `as_mut_ptr` / slice / `&mut` use site unchanged.
#[repr(C, align(16))]
#[derive(Debug)]
pub(crate) struct ElemBuf([u8; ElemBuf::MAX_SIZE]);

// INVARIANT: the 16-byte alignment is load-bearing for UB prevention — drop it and
// element reads through the scratch buffer become misaligned. Pin it so removing
// `align(16)` is a compile error, not a silent regression.
const _: () = assert!(core::mem::align_of::<ElemBuf>() == 16);

impl ElemBuf {
    /// Maximum element size admitted by iterator scratch storage.
    pub(crate) const MAX_SIZE: usize = 256;

    #[inline]
    pub(crate) const fn new() -> Self {
        Self([0u8; Self::MAX_SIZE])
    }
}

impl core::ops::Deref for ElemBuf {
    type Target = [u8; ElemBuf::MAX_SIZE];
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
/// before any fixed-size [`ElemBuf`] is used.
///
/// Uses `assert!` (not `debug_assert!`) because the scratch buffers are
/// fixed-size [`ElemBuf`] values in both debug and release builds —
/// an oversized element causes a stack buffer overflow in release if unchecked.
#[inline]
pub(crate) fn assert_elem_size(elem_size: i64, context: &str) {
    assert!(
        elem_size >= 0 && (elem_size as usize) <= ElemBuf::MAX_SIZE,
        "{context}: element size {elem_size} exceeds MAX_ELEM_SIZE ({})",
        ElemBuf::MAX_SIZE
    );
}

/// Trampoline signature for map: `(env, in_ptr, out_ptr) -> void`.
///
/// User closures may panic. The unwind-capable ABI is therefore part of the
/// callback contract, not an implementation detail of any one consumer.
pub(crate) type TransformFn = extern "C-unwind" fn(*mut u8, *const u8, *mut u8);

/// Trampoline signature for filter/any/all/find: `(env, elem_ptr) -> bool`
pub(crate) type PredicateFn = extern "C-unwind" fn(*mut u8, *const u8) -> bool;

/// Trampoline signature for `for_each`: `(env, elem_ptr) -> void`
pub(crate) type ForEachFn = extern "C-unwind" fn(*mut u8, *const u8);

/// Trampoline signature for fold: `(env, acc_ptr, elem_ptr, out_ptr) -> void`
pub(crate) type FoldFn = extern "C-unwind" fn(*mut u8, *const u8, *const u8, *mut u8);

/// Releases the RC children owned by one byte-copied value.
///
/// Null at the ABI boundary means the value has no RC children.
pub(crate) type ElemDecFn = extern "C" fn(*mut u8);

/// Retains the RC children of one byte-copied value.
///
/// Selection consumers use this before discharging a dynamic source yield;
/// lazy adapters use it to own their copied callback closure pair.
pub(crate) type ElemIncFn = extern "C" fn(*mut u8);

/// Closure trampoline environment retained for a lazy adapter's lifetime.
#[derive(Debug)]
pub(crate) enum CallbackEnv {
    /// Test and embedding callers may provide a stable opaque environment with
    /// no ownership hooks.
    Borrowed(*mut u8),
    /// Generated code passes a pointer to a `{fn_ptr, env_ptr}` closure pair.
    /// The copied pair owns one retained reference to the captured environment.
    Owned {
        words: [*mut u8; 2],
        dec_fn: ElemDecFn,
    },
}

impl CallbackEnv {
    /// Preserve an opaque environment, or copy and retain a generated closure pair.
    ///
    /// # Safety
    ///
    /// When ownership hooks are present, `source` must point to two initialized
    /// pointer-sized closure fields accepted by both hooks.
    pub(crate) unsafe fn new(
        source: *mut u8,
        inc_fn: Option<ElemIncFn>,
        dec_fn: Option<ElemDecFn>,
    ) -> Self {
        match (inc_fn, dec_fn) {
            (None, None) => Self::Borrowed(source),
            (Some(inc), Some(dec)) => {
                assert!(
                    !source.is_null(),
                    "owned callback environment must be non-null"
                );
                let mut words: [*mut u8; 2] = [ptr::null_mut(); 2];
                // SAFETY: upheld by this function's caller contract; both
                // regions hold exactly one generated closure pair and do not
                // overlap.
                unsafe {
                    ptr::copy_nonoverlapping(
                        source.cast_const(),
                        words.as_mut_ptr().cast::<u8>(),
                        size_of::<[*mut u8; 2]>(),
                    );
                }
                inc(words.as_mut_ptr().cast());
                Self::Owned { words, dec_fn: dec }
            }
            _ => panic!("callback environment retain and release hooks must be paired"),
        }
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        match self {
            Self::Borrowed(ptr) => *ptr,
            Self::Owned { words, .. } => words.as_mut_ptr().cast(),
        }
    }
}

impl Drop for CallbackEnv {
    fn drop(&mut self) {
        if let Self::Owned { words, dec_fn } = self {
            dec_fn(words.as_mut_ptr().cast());
        }
    }
}

/// Source phase for a cycling iterator.
///
/// The source exists only while the first pass is being buffered. Once that
/// pass ends, the iterator permanently replays its owned buffer.
#[derive(Debug)]
pub(crate) enum CycleSource {
    Reading(Box<IterState>),
    Replaying,
}

/// Iterator state machine. Each variant corresponds to an iterator source
/// or adapter from the evaluator's `IteratorValue` enum.
#[derive(Debug)]
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
        transform_env: CallbackEnv,
        in_size: i64,
        /// Releases a yielded map result when another adapter consumes or
        /// discards it instead of forwarding it to the terminal consumer.
        output_dec_fn: Option<ElemDecFn>,
    },

    /// Filters elements via a predicate trampoline.
    Filtered {
        source: Box<IterState>,
        predicate_fn: PredicateFn,
        predicate_env: CallbackEnv,
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
        source: CycleSource,
        buffer: Vec<u8>,
        buf_pos: usize,
        elem_size: i64,
        elem_inc_fn: Option<ElemIncFn>,
        elem_dec_fn: Option<ElemDecFn>,
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
        elem_dec_fn: Option<ElemDecFn>,
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
        elem_dec_fn: Option<ElemDecFn>,
    },
}

impl IterState {
    /// Release the ownership obligation attached to the most recent successful
    /// yield from this iterator.
    ///
    /// Source iterators and replay buffers yield borrowed aliases, so their
    /// obligation is a no-op. A mapped iterator yields a fresh value and owns a
    /// type-matched release thunk. Identity adapters delegate to the exact
    /// source that produced the value.
    ///
    /// Callers must release, transfer, or forward a successful yield before
    /// advancing this iterator again; adapter branch state identifies only the
    /// most recent yield.
    ///
    /// # Safety
    ///
    /// `elem_ptr` must point to the complete element produced by this
    /// iterator's most recent successful `next` or `next_back` call.
    pub(crate) unsafe fn release_last_yield(&mut self, elem_ptr: *mut u8) {
        match self {
            Self::Mapped {
                output_dec_fn: Some(dec),
                ..
            } => dec(elem_ptr),
            Self::Filtered { source, .. }
            | Self::TakeN { source, .. }
            | Self::SkipN { source, .. }
            | Self::Cycled {
                source: CycleSource::Reading(source),
                ..
            } => source.release_last_yield(elem_ptr),
            Self::Enumerated { source, .. } => {
                source.release_last_yield(elem_ptr.add(size_of::<i64>()));
            }
            Self::Zipped {
                left,
                right,
                left_elem_size,
            } => {
                left.release_last_yield(elem_ptr);
                right.release_last_yield(elem_ptr.add(*left_elem_size as usize));
            }
            Self::Chained {
                first,
                second,
                first_done,
            } => {
                if *first_done {
                    second.release_last_yield(elem_ptr);
                } else {
                    first.release_last_yield(elem_ptr);
                }
            }
            Self::Flattened {
                inner: Some(inner), ..
            } => {
                inner.release_last_yield(elem_ptr);
            }
            Self::List { .. }
            | Self::Range { .. }
            | Self::Mapped {
                output_dec_fn: None,
                ..
            }
            | Self::Flattened { inner: None, .. }
            | Self::Cycled { .. }
            | Self::Reversed { .. }
            | Self::Str { .. }
            | Self::Map { .. }
            | Self::Repeat { .. } => {}
        }
    }
}
