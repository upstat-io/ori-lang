//! Backward advancement for double-ended iterator states.
//!
//! `List`, `Range`, and `Str` shrink their back boundary; `Reversed` advances
//! its front boundary. `Mapped` and `Filtered` delegate to double-ended
//! sources. Other variants return `false` because type checking excludes them.
//! Copy-out does not retain the yielded bytes; iterator state keeps ownership
//! until its normal drop path runs.

use std::ptr;

use super::state::{ElemBuf, IterState, PredicateFn, TransformFn, YieldGuard};

impl IterState {
    /// Advance the iterator from the back, writing the element to `out_ptr`.
    ///
    /// Returns `true` if an element was produced, `false` if exhausted or the
    /// variant is not double-ended.
    ///
    /// # Safety
    ///
    /// `out_ptr` must be writable for the variant's output layout and must not
    /// overlap any source allocation. `elem_size` must match that layout.
    // SAFETY: The caller supplies the output region; each variant preserves its live window.
    pub(crate) unsafe fn next_back(&mut self, out_ptr: *mut u8, _elem_size: i64) -> bool {
        match self {
            Self::List {
                data,
                len,
                pos,
                elem_size: es,
                ..
            } => Self::next_back_list(*data, len, *pos, *es, out_ptr),
            Self::Range {
                current,
                end,
                step,
                inclusive,
            } => Self::next_back_range(*current, end, *step, inclusive, out_ptr),
            Self::Mapped {
                source,
                transform_fn,
                transform_env,
                in_size,
                ..
            } => Self::next_back_mapped(source, *transform_fn, *transform_env, *in_size, out_ptr),
            Self::Filtered {
                source,
                predicate_fn,
                predicate_env,
                elem_size: es,
            } => Self::next_back_filtered(source, *predicate_fn, *predicate_env, *es, out_ptr),
            Self::Reversed {
                elements,
                pos,
                front,
                elem_size: es,
                ..
            } => Self::next_back_reversed(elements, front, *pos, *es, out_ptr),
            Self::Str {
                data,
                len,
                byte_offset,
                ..
            } => Self::next_back_str(*data, len, *byte_offset, out_ptr),
            _ => false,
        }
    }

    /// List: pop from the high end of the `[pos, len)` window.
    ///
    /// Decrements `len` (the back boundary) and copies `data[len]`. `pos` (the
    /// front boundary) is untouched, so forward and backward iteration share
    /// one contiguous window. The owned buffer is unchanged; `Drop` releases
    /// `[0, len)` per the V5 header.
    // SAFETY: `data` owns the live `[pos, len)` window; the output is disjoint and writable.
    unsafe fn next_back_list(
        data: *mut u8,
        len: &mut i64,
        pos: i64,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if pos >= *len {
            return false;
        }
        *len -= 1;
        let offset = *len * es;
        ptr::copy_nonoverlapping(data.add(offset as usize), out_ptr, es as usize);
        true
    }

    /// Range: yield the last aligned value, then shrink the upper bound.
    ///
    /// Mirrors `IteratorValue::next_back` (`ori_patterns`): compute the count of
    /// remaining values, derive the last aligned value, set the bound exclusive
    /// at that value so the next backward step yields its predecessor.
    // SAFETY: The range bounds are live scalars; `out_ptr` is a disjoint writable `i64`.
    unsafe fn next_back_range(
        current: i64,
        end: &mut i64,
        step: i64,
        inclusive: &mut bool,
        out_ptr: *mut u8,
    ) -> bool {
        // Why: An unbounded descending range has no final value to yield.
        if step == 0 || (*end == i64::MAX && step < 0) {
            return false;
        }
        let n = range_len(current, *end, step, *inclusive);
        if n == 0 {
            return false;
        }
        // INVARIANT: `n` counts reachable values, so the last aligned value remains in range.
        let last = current + (n - 1) * step;
        *end = last;
        *inclusive = false;
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i64>(&last).cast::<u8>(),
            out_ptr,
            size_of::<i64>(),
        );
        true
    }

    // SAFETY: `in_size` fits `ElemBuf`; the source, trampoline, and output layouts remain live.
    unsafe fn next_back_mapped(
        source: &mut IterState,
        transform_fn: TransformFn,
        transform_env: *mut u8,
        in_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let mut scratch = ElemBuf::new();
        if !source.next_back(scratch.as_mut_ptr(), in_size) {
            return false;
        }
        let _source_yield = YieldGuard::new(source, scratch.as_mut_ptr());
        (transform_fn)(transform_env, scratch.as_ptr(), out_ptr);
        true
    }

    // SAFETY: The source and predicate remain live; `out_ptr` is writable for `es` bytes.
    unsafe fn next_back_filtered(
        source: &mut IterState,
        predicate_fn: PredicateFn,
        predicate_env: *mut u8,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        loop {
            if !source.next_back(out_ptr, es) {
                return false;
            }
            let mut source_yield = YieldGuard::new(source, out_ptr);
            if (predicate_fn)(predicate_env, out_ptr) {
                source_yield.disarm();
                return true;
            }
        }
    }

    /// Pop the low end of `[front, pos)` without retaining the copied owner.
    /// `Drop` remains responsible for every stored master.
    // SAFETY: `[front, pos)` indexes whole elements; `out_ptr` is disjoint and writable.
    unsafe fn next_back_reversed(
        elements: &[u8],
        front: &mut i64,
        pos: i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if *front >= pos {
            return false;
        }
        let es = elem_size.max(1) as usize;
        let offset = (*front as usize) * es;
        ptr::copy_nonoverlapping(elements[offset..].as_ptr(), out_ptr, es);
        *front += 1;
        true
    }

    // SAFETY: `data` contains the active byte window; `out_ptr` is a disjoint `i32` slot.
    unsafe fn next_back_str(
        data: *mut u8,
        len: &mut i64,
        byte_offset: i64,
        out_ptr: *mut u8,
    ) -> bool {
        if data.is_null() || byte_offset >= *len {
            return false;
        }
        // INVARIANT: A codepoint starts at the first non-continuation byte found in reverse.
        let mut start = *len - 1;
        while start > byte_offset {
            let b = *data.add(start as usize);
            if (b & 0xC0) != 0x80 {
                break;
            }
            start -= 1;
        }
        let result = crate::ori_str_next_char(data, *len, start);
        if result.codepoint < 0 {
            return false;
        }
        let cp = result.codepoint;
        ptr::copy_nonoverlapping(
            std::ptr::from_ref::<i32>(&cp).cast::<u8>(),
            out_ptr,
            size_of::<i32>(),
        );
        *len = start;
        true
    }
}

/// Count values yielded by `[current, end)` or `[current, end]` under `step`.
fn range_len(current: i64, end: i64, step: i64, inclusive: bool) -> i64 {
    if step == 0 {
        return 0;
    }
    let span = if inclusive {
        if step > 0 {
            end - current + 1
        } else {
            current - end + 1
        }
    } else if step > 0 {
        end - current
    } else {
        current - end
    };
    if span <= 0 {
        return 0;
    }
    let abs_step = step.abs();
    (span + abs_step - 1) / abs_step
}
