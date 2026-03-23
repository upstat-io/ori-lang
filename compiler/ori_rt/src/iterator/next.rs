//! `IterState::next()` dispatch and per-variant advancement logic.

use std::ptr;

use super::state::{assert_elem_size, IterState, PredicateFn, TransformFn, MAX_ELEM_SIZE};

impl IterState {
    /// Advance the iterator, writing the next element to `out_ptr`.
    ///
    /// Returns `true` if an element was produced, `false` if exhausted.
    ///
    /// # Safety
    ///
    /// `out_ptr` must be valid for `elem_size` bytes (varies by variant).
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
            } => Self::next_mapped(source, *transform_fn, *transform_env, *in_size, out_ptr),
            Self::Filtered {
                source,
                predicate_fn,
                predicate_env,
                elem_size: es,
            } => Self::next_filtered(source, *predicate_fn, *predicate_env, *es, out_ptr),
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
        }
    }

    unsafe fn next_list(data: *mut u8, len: i64, pos: &mut i64, es: i64, out_ptr: *mut u8) -> bool {
        if *pos >= len {
            return false;
        }
        let offset = *pos * es;
        ptr::copy_nonoverlapping(data.add(offset as usize), out_ptr, es as usize);
        *pos += 1;
        true
    }

    unsafe fn next_range(
        current: &mut i64,
        end: i64,
        step: i64,
        inclusive: bool,
        out_ptr: *mut u8,
    ) -> bool {
        let in_bounds = if inclusive {
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

    unsafe fn next_mapped(
        source: &mut IterState,
        transform_fn: TransformFn,
        transform_env: *mut u8,
        in_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let mut scratch = [0u8; MAX_ELEM_SIZE];
        if !source.next(scratch.as_mut_ptr(), in_size) {
            return false;
        }
        (transform_fn)(transform_env, scratch.as_ptr(), out_ptr);
        true
    }

    unsafe fn next_filtered(
        source: &mut IterState,
        predicate_fn: PredicateFn,
        predicate_env: *mut u8,
        es: i64,
        out_ptr: *mut u8,
    ) -> bool {
        loop {
            if !source.next(out_ptr, es) {
                return false;
            }
            if (predicate_fn)(predicate_env, out_ptr) {
                return true;
            }
        }
    }

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

    unsafe fn next_skip(
        source: &mut IterState,
        remaining: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        while *remaining > 0 {
            let mut discard = [0u8; MAX_ELEM_SIZE];
            if !source.next(discard.as_mut_ptr(), elem_size) {
                *remaining = 0;
                return false;
            }
            *remaining -= 1;
        }
        source.next(out_ptr, elem_size)
    }

    unsafe fn next_enumerated(
        source: &mut IterState,
        index: &mut i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        // Layout: first 8 bytes = index, then elem_size - 8 bytes = element
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

    /// Zip: advance both iterators, copy left then right to output.
    ///
    /// Output layout: `[left_elem_bytes | right_elem_bytes]`.
    /// Total output size is `elem_size` (= `left_elem_size` + `right_elem_size`).
    unsafe fn next_zipped(
        left: &mut IterState,
        right: &mut IterState,
        left_elem_size: i64,
        elem_size: i64,
        out_ptr: *mut u8,
    ) -> bool {
        let right_elem_size = elem_size - left_elem_size;
        // Validate right element size (left validated at adapter creation).
        assert_elem_size(right_elem_size, "next_zipped(right)");
        // Advance left into front of output buffer
        if !left.next(out_ptr, left_elem_size) {
            return false;
        }
        // Advance right into back of output buffer
        let right_ptr = out_ptr.add(left_elem_size as usize);
        if !right.next(right_ptr, right_elem_size) {
            return false;
        }
        true
    }

    /// Chain: yield all of first iterator, then all of second.
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

    /// Str: decode the next UTF-8 codepoint and write it as i32.
    ///
    /// Output is a single `i32` (4 bytes) — the Unicode scalar value.
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

    /// Map: yield the next `(key, value)` pair.
    ///
    /// Data layout (hash table): `[metadata | keys | values]`.
    /// Scans metadata starting at bucket `pos` for the next OCCUPIED entry.
    /// Output layout: `[key_bytes | value_bytes]` (concatenated).
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
