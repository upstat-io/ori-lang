//! Iterator adapter constructors — create adapter iterators that wrap sources.
//!
//! These are `extern "C"` functions called from LLVM-generated code to create
//! map, filter, take, skip, enumerate, zip, chain, and cycle adapters.

use std::ptr;

use super::state::{
    assert_elem_size, empty_range, CallbackEnv, CycleSource, ElemDecFn, ElemIncFn, IterState,
    PredicateFn, TransformFn, YieldGuard,
};
use super::take_iter;

/// Create a mapped iterator adapter.
///
/// `transform_fn` is a trampoline: `(env, in_ptr, out_ptr) -> void`.
/// `transform_env` is the closure environment pointer (may be null).
/// The paired environment hooks retain and release a generated closure pair;
/// both are null for a stable borrowed embedding environment.
/// `in_size` is the byte size of input elements (for scratch buffer sizing).
/// `output_dec_fn` releases one fresh mapped result when an adapter consumes
/// or discards it internally (null for results without RC children).
#[no_mangle]
pub extern "C" fn ori_iter_map(
    iter: *mut u8,
    transform_fn: TransformFn,
    transform_env: *mut u8,
    transform_env_inc_fn: Option<ElemIncFn>,
    transform_env_dec_fn: Option<ElemDecFn>,
    in_size: i64,
    output_dec_fn: Option<ElemDecFn>,
) -> *mut u8 {
    assert_elem_size(in_size, "ori_iter_map");
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    // SAFETY: Generated hooks describe the `{fn_ptr, env_ptr}` pair at
    // `transform_env`; hook-free embedding callers retain their stable pointer.
    let transform_env =
        unsafe { CallbackEnv::new(transform_env, transform_env_inc_fn, transform_env_dec_fn) };
    let state = IterState::Mapped {
        source,
        transform_fn,
        transform_env,
        in_size,
        output_dec_fn,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a filtered iterator adapter.
///
/// `predicate_fn` is a trampoline: `(env, elem_ptr) -> bool`.
/// `predicate_env` is the closure environment pointer (may be null).
/// Its paired hooks follow the same ownership contract as `ori_iter_map`.
#[no_mangle]
pub extern "C" fn ori_iter_filter(
    iter: *mut u8,
    predicate_fn: PredicateFn,
    predicate_env: *mut u8,
    predicate_env_inc_fn: Option<ElemIncFn>,
    predicate_env_dec_fn: Option<ElemDecFn>,
    elem_size: i64,
) -> *mut u8 {
    assert_elem_size(elem_size, "ori_iter_filter");
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    // SAFETY: Same callback-environment contract as `ori_iter_map`.
    let predicate_env =
        unsafe { CallbackEnv::new(predicate_env, predicate_env_inc_fn, predicate_env_dec_fn) };
    let state = IterState::Filtered {
        source,
        predicate_fn,
        predicate_env,
        elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a take(n) adapter — yields at most `n` elements from source.
#[no_mangle]
pub extern "C" fn ori_iter_take(iter: *mut u8, n: i64) -> *mut u8 {
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    let state = IterState::TakeN {
        source,
        remaining: n.max(0),
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a skip(n) adapter — skips `n` elements then yields the rest.
#[no_mangle]
pub extern "C" fn ori_iter_skip(iter: *mut u8, n: i64) -> *mut u8 {
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    let state = IterState::SkipN {
        source,
        remaining: n.max(0),
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create an enumerate adapter — wraps each element with its 0-based index.
///
/// Output element layout: `{ i64 index, T element }`.
#[no_mangle]
pub extern "C" fn ori_iter_enumerate(iter: *mut u8) -> *mut u8 {
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    let state = IterState::Enumerated { source, index: 0 };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a zip adapter — pairs elements from two iterators.
///
/// Output element layout: `[left_elem | right_elem]` (concatenated bytes).
/// Stops when either iterator is exhausted.
#[no_mangle]
pub extern "C" fn ori_iter_zip(left: *mut u8, right: *mut u8, left_elem_size: i64) -> *mut u8 {
    assert_elem_size(left_elem_size, "ori_iter_zip");
    let Some(left_state) = take_iter(left) else {
        drop(take_iter(right));
        return ptr::null_mut();
    };
    let Some(right_state) = take_iter(right) else {
        drop(left_state);
        return ptr::null_mut();
    };
    let state = IterState::Zipped {
        left: left_state,
        right: right_state,
        left_elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a chain adapter — yields all elements from first, then all from second.
#[no_mangle]
pub extern "C" fn ori_iter_chain(first: *mut u8, second: *mut u8) -> *mut u8 {
    if first.is_null() && second.is_null() {
        return ptr::null_mut();
    }
    let first_state = match take_iter(first) {
        Some(state) => state,
        None => Box::new(empty_range()),
    };
    let second_state = match take_iter(second) {
        Some(state) => state,
        None => Box::new(empty_range()),
    };
    let state = IterState::Chained {
        first: first_state,
        second: second_state,
        first_done: false,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a flatten adapter — flattens iterator of iterators into a single stream.
///
/// The source iterator must yield iterator pointers (ptr-sized = 8 bytes).
/// `inner_elem_size` is the byte size of elements in the inner iterators.
#[no_mangle]
pub extern "C" fn ori_iter_flatten(iter: *mut u8, inner_elem_size: i64) -> *mut u8 {
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    assert_elem_size(inner_elem_size, "ori_iter_flatten");
    let state = IterState::Flattened {
        source,
        inner: None,
        inner_elem_size,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a cycle adapter — repeats the source iterator infinitely.
///
/// Buffers elements on the first pass, then replays from the buffer.
/// Empty sources produce an empty cycle (no infinite loop).
#[no_mangle]
pub extern "C" fn ori_iter_cycle(
    iter: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<ElemIncFn>,
    elem_dec_fn: Option<ElemDecFn>,
) -> *mut u8 {
    let Some(source) = take_iter(iter) else {
        return ptr::null_mut();
    };
    assert_elem_size(elem_size, "ori_iter_cycle");
    let state = IterState::Cycled {
        source: CycleSource::Reading(source),
        buffer: Vec::new(),
        buf_pos: 0,
        elem_size,
        elem_inc_fn,
        elem_dec_fn,
    };
    Box::into_raw(Box::new(state)).cast()
}

/// Create a reversed iterator by collecting all elements and iterating backward.
///
/// This is an eager operation — all elements are collected into memory.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_rev(
    iter: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<ElemIncFn>,
    elem_dec_fn: Option<ElemDecFn>,
) -> *mut u8 {
    use super::state::ElemBuf;

    let Some(mut state) = take_iter(iter) else {
        return ptr::null_mut();
    };
    assert_elem_size(elem_size, "ori_iter_rev");
    let es = elem_size.max(1) as usize;

    // Build the owning state before eager iteration. If a source callback
    // unwinds, `IterState::drop` releases every fully retained prefix element.
    let mut reversed = IterState::Reversed {
        elements: Vec::new(),
        pos: 0,
        front: 0,
        elem_size,
        elem_dec_fn,
    };
    let mut elem_buf = ElemBuf::new();
    // SAFETY:
    // - `elem_buf` is writable for every validated `elem_size`.
    // - `state` owns every source allocation read by `next`.
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let _source_yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        let IterState::Reversed { elements, pos, .. } = &mut reversed else {
            unreachable!("ori_iter_rev buffer state changed variant")
        };
        elements.extend_from_slice(&elem_buf[..es]);
        if let Some(inc) = elem_inc_fn {
            // SAFETY: The appended element occupies `[len - es, len)` in `elements`.
            let stored = unsafe { elements.as_mut_ptr().add(elements.len() - es) };
            inc(stored);
        }
        *pos += 1;
    }

    // INVARIANT: Source teardown releases originals; retained buffer owners remain live.
    drop(state);
    Box::into_raw(Box::new(reversed)).cast()
}
