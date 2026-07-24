//! `IterState::next()` dispatch and per-variant advancement logic.

use std::ptr;

use super::state::{
    assert_elem_size, CallbackEnv, CycleSource, ElemBuf, IterState, PredicateFn, TransformFn,
    YieldGuard,
};

impl IterState {
    /// Advance the iterator, writing the next element to `out_ptr`.
    ///
    /// Returns `true` if an element was produced, `false` if exhausted.
    ///
    /// # Safety
    ///
    /// `out_ptr` must be writable for the variant's output layout and must not
    /// overlap any source allocation. `elem_size` must match that layout.
    // SAFETY: The caller supplies the output region; constructors preserve every source layout.
    pub(crate) unsafe fn next(&mut self, out_ptr: *mut u8, elem_size: i64) -> bool {
        match self {
            Self::List {
                data,
                len,
                pos,
                elem_size: es,
                ..
            } => Self::next_list(*data, *len, pos, *es, out_ptr),
            Self::Range {
                current,
                end,
                step,
                inclusive,
            } => Self::next_range(current, *end, *step, *inclusive, out_ptr),
            Self::Mapped {
                source,
                transform_fn,
                transform_env,
                in_size,
                ..
            } => Self::next_mapped(source, *transform_fn, transform_env, *in_size, out_ptr),
            Self::Filtered {
                source,
                predicate_fn,
                predicate_env,
                elem_size: es,
            } => Self::next_filtered(source, *predicate_fn, predicate_env, *es, out_ptr),
            Self::TakeN { source, remaining } => {
                Self::next_take(source, remaining, elem_size, out_ptr)
            }
            Self::SkipN { source, remaining } => {
                Self::next_skip(source, remaining, elem_size, out_ptr)
            }
            Self::Enumerated { source, index } => {
                Self::next_enumerated(source, index, elem_size, out_ptr)
            }
            Self::Zipped {
                left,
                right,
                left_elem_size,
            } => Self::next_zipped(left, right, *left_elem_size, elem_size, out_ptr),
            Self::Chained {
                first,
                second,
                first_done,
            } => Self::next_chained(first, second, first_done, elem_size, out_ptr),
            Self::Flattened {
                source,
                inner,
                inner_elem_size,
            } => Self::next_flattened(source, inner, *inner_elem_size, out_ptr),
            Self::Cycled {
                source,
                buffer,
                buf_pos,
                elem_size: es,
                elem_inc_fn,
                ..
            } => Self::next_cycled(source, buffer, buf_pos, *es, *elem_inc_fn, out_ptr),
            Self::Reversed {
                elements,
                pos,
                front,
                elem_size: es,
                ..
            } => Self::next_reversed(elements, pos, *front, *es, out_ptr),
            Self::Str {
                data,
                len,
                byte_offset,
                ..
            } => Self::next_str(*data, *len, byte_offset, out_ptr),
            Self::Map {
                data,
                cap,
                len,
                pos,
                key_size,
                val_size,
                ..
            } => Self::next_map(*data, *cap, *len, pos, *key_size, *val_size, out_ptr),
            Self::Repeat {
                value, elem_size, ..
            } => Self::next_repeat(value, *elem_size, out_ptr),
        }
    }

    // SAFETY: `value` supplies `elem_size` bytes and `out_ptr` is disjoint and writable.
    unsafe fn next_repeat(value: &[u8], elem_size: i64, out_ptr: *mut u8) -> bool {
        let es = elem_size.max(0) as usize;
        ptr::copy_nonoverlapping(value.as_ptr(), out_ptr, es);
        true
    }

    // SAFETY: `data` owns `len * es` bytes; `pos` and the disjoint output are in bounds.
    unsafe fn next_list(data: *mut u8, len: i64, pos: &mut i64, es: i64, out_ptr: *mut u8) -> bool {
        if *pos >= len {
            return false;
        }
        let offset = *pos * es;
        ptr::copy_nonoverlapping(data.add(offset as usize), out_ptr, es as usize);
        *pos += 1;
        true
    }

    // SAFETY: `current` is a live `i64`; `out_ptr` is a disjoint writable `i64` slot.
    unsafe fn next_range(
        current: &mut i64,
        end: i64,
        step: i64,
        inclusive: bool,
        out_ptr: *mut u8,
    ) -> bool {
        // Why: `i64::MAX` encodes an unbounded end that descending comparison cannot satisfy.
        let in_bounds = if end == i64::MAX && step < 0 {
            true
        } else if inclusive {
            if step > 0 {
                *current <= end
            } else {
                *current >= end
            }
        } else if step > 0 {
            *current < end
        } else {
            *current > end
        };
        if !in_bounds {
            return false;
        }
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i64>(current).cast::<u8>(),
            out_ptr,
            size_of::<i64>(),
        );
        *current += step;
        true
    }

    // SAFETY: `in_size` fits `ElemBuf`; the source, trampoline, and output layouts remain live.
    unsafe fn next_mapped(
        source: &mut IterState,
        transform_fn: TransformFn,
        transform_env: &mut CallbackEnv,
        in_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let mut scratch = ElemBuf::new();
        if !source.next(scratch.as_mut_ptr(), in_size) {
            return false;
        }
        let _source_yield = YieldGuard::new(source, scratch.as_mut_ptr());
        (transform_fn)(transform_env.as_mut_ptr(), scratch.as_ptr(), out_ptr);
        true
    }

    // SAFETY: The source and predicate remain live; `out_ptr` is writable for `es` bytes.
    unsafe fn next_filtered(
        source: &mut IterState,
        predicate_fn: PredicateFn,
        predicate_env: &mut CallbackEnv,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        loop {
            if !source.next(out_ptr, es) {
                return false;
            }
            let mut source_yield = YieldGuard::new(source, out_ptr);
            if (predicate_fn)(predicate_env.as_mut_ptr(), out_ptr) {
                source_yield.disarm();
                return true;
            }
        }
    }

    // SAFETY: `source` owns every delegated read; `out_ptr` is writable for `elem_size` bytes.
    unsafe fn next_take(
        source: &mut IterState,
        remaining: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if *remaining <= 0 {
            return false;
        }
        if !source.next(out_ptr, elem_size) {
            *remaining = 0;
            return false;
        }
        *remaining -= 1;
        true
    }

    // SAFETY: `elem_size` fits `ElemBuf`; the source and output remain live during skipped yields.
    unsafe fn next_skip(
        source: &mut IterState,
        remaining: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        while *remaining > 0 {
            let mut discard = ElemBuf::new();
            if !source.next(discard.as_mut_ptr(), elem_size) {
                *remaining = 0;
                return false;
            }
            let _discarded_yield = YieldGuard::new(source, discard.as_mut_ptr());
            *remaining -= 1;
        }
        source.next(out_ptr, elem_size)
    }

    // SAFETY: The checked output holds one aligned `i64` followed by the source element.
    unsafe fn next_enumerated(
        source: &mut IterState,
        index: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        // INVARIANT: The output stores one `i64` index followed by the source element.
        let inner_size = elem_size - size_of::<i64>() as i64;
        if inner_size < 0 {
            return false;
        }
        let elem_ptr = out_ptr.add(size_of::<i64>());
        if !source.next(elem_ptr, inner_size) {
            return false;
        }
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i64>(index).cast::<u8>(),
            out_ptr,
            size_of::<i64>(),
        );
        *index += 1;
        true
    }

    // SAFETY: Both sources are live; `out_ptr` contains disjoint left and right regions.
    unsafe fn next_zipped(
        left: &mut IterState,
        right: &mut IterState,
        left_elem_size: i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let right_elem_size = elem_size - left_elem_size;
        assert_elem_size(right_elem_size, "next_zipped(right)");
        if !left.next(out_ptr, left_elem_size) {
            return false;
        }
        let mut left_yield = YieldGuard::new(left, out_ptr);
        let right_ptr = out_ptr.add(left_elem_size as usize);
        if !right.next(right_ptr, right_elem_size) {
            return false;
        }
        left_yield.disarm();
        true
    }

    // SAFETY: Both sources preserve the delegated layout; `out_ptr` is writable.
    unsafe fn next_chained(
        first: &mut IterState,
        second: &mut IterState,
        first_done: &mut bool,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if !*first_done {
            if first.next(out_ptr, elem_size) {
                return true;
            }
            *first_done = true;
        }
        second.next(out_ptr, elem_size)
    }

    // SAFETY: `data` contains `len` live bytes; `out_ptr` is a disjoint writable `i32` slot.
    unsafe fn next_str(data: *mut u8, len: i64, byte_offset: &mut i64, out_ptr: *mut u8) -> bool {
        if data.is_null() || *byte_offset >= len {
            return false;
        }
        let result = crate::ori_str_next_char(data, len, *byte_offset);
        if result.codepoint < 0 {
            *byte_offset = len;
            return false;
        }
        let cp = result.codepoint;
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i32>(&cp).cast::<u8>(),
            out_ptr,
            size_of::<i32>(),
        );
        *byte_offset = result.next_offset;
        true
    }

    // SAFETY: The source yields raw boxes recovered once; the output matches the inner layout.
    unsafe fn next_flattened(
        source: &mut IterState,
        inner: &mut Option<Box<IterState>>,
        inner_elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        loop {
            if let Some(ref mut inner_state) = inner {
                if inner_state.next(out_ptr, inner_elem_size) {
                    return true;
                }
                *inner = None;
            }

            let mut iter_ptr_buf = [0u8; 8];
            if !source.next(iter_ptr_buf.as_mut_ptr(), 8) {
                return false;
            }
            let iter_ptr = ptr::read(iter_ptr_buf.as_ptr().cast::<*mut u8>());
            if iter_ptr.is_null() {
                continue;
            }
            *inner = Some(Box::from_raw(iter_ptr.cast::<IterState>()));
        }
    }

    // SAFETY: The buffer stores whole owners; the increment callback and output stay in bounds.
    unsafe fn next_cycled(
        source: &mut CycleSource,
        buffer: &mut Vec<u8>,
        buf_pos: &mut usize,
        elem_size: i64,
        elem_inc_fn: Option<extern "C" fn(*mut u8)>,
        out_ptr: *mut u8,
    ) -> bool {
        let es = elem_size.max(1) as usize;

        if let CycleSource::Reading(src) = source {
            let mut elem_buf = ElemBuf::new();
            if src.next(elem_buf.as_mut_ptr(), elem_size) {
                let mut source_yield = YieldGuard::new(src, elem_buf.as_mut_ptr());
                // INVARIANT: Each buffered owner is retained once and released once by `Drop`.
                buffer.extend_from_slice(&elem_buf[..es]);
                if let Some(inc) = elem_inc_fn {
                    let stored = buffer.as_mut_ptr().add(buffer.len() - es);
                    inc(stored);
                }
                ptr::copy_nonoverlapping(elem_buf.as_ptr(), out_ptr, es);
                // The cycle forwards the current source obligation. Its parent
                // consumer releases it through `Cycled::release_last_yield`.
                source_yield.disarm();
                return true;
            }
            *source = CycleSource::Replaying;
            *buf_pos = 0;
        }

        if buffer.is_empty() {
            return false;
        }
        let idx = *buf_pos * es;
        if idx >= buffer.len() {
            *buf_pos = 0;
        }
        let idx = *buf_pos * es;
        ptr::copy_nonoverlapping(buffer[idx..].as_ptr(), out_ptr, es);
        *buf_pos += 1;
        true
    }

    /// Reversed: pop the high end of the `[front, pos)` window, yielding the
    /// pre-collected elements in reverse source order. `front` is the back
    /// boundary `next_back` advances; forward iteration stops when the window
    /// is empty (`pos <= front`), not at a fixed `0`.
    // SAFETY: `[front, pos)` indexes whole elements; `out_ptr` is disjoint and writable.
    unsafe fn next_reversed(
        elements: &[u8],
        pos: &mut i64,
        front: i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if *pos <= front {
            return false;
        }
        *pos -= 1;
        let es = elem_size.max(1) as usize;
        let offset = (*pos as usize) * es;
        ptr::copy_nonoverlapping(elements[offset..].as_ptr(), out_ptr, es);
        true
    }

    // SAFETY: `data` owns the described table; `out_ptr` is disjoint and fits one pair.
    unsafe fn next_map(
        data: *mut u8,
        cap: i64,
        _len: i64,
        pos: &mut i64,
        key_size: i64,
        val_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let c = cap as usize;
        let ks = key_size as usize;
        let vs = val_size as usize;
        let layout = crate::map::hash_table::HashTableLayout::for_map(c, ks, vs);

        while (*pos as usize) < c {
            let bucket = *pos as usize;
            *pos += 1;
            if crate::map::hash_table::get_meta(data, bucket)
                == crate::map::hash_table::META_OCCUPIED
            {
                let key_ptr = data.add(layout.keys_offset + bucket * ks);
                let val_ptr = data.add(layout.vals_offset + bucket * vs);
                ptr::copy_nonoverlapping(key_ptr, out_ptr, ks);
                ptr::copy_nonoverlapping(val_ptr, out_ptr.add(ks), vs);
                return true;
            }
        }
        false
    }
}
