//! Collection consumers.

use std::ptr;

use super::super::state::{assert_elem_size, IterState, YieldGuard};
use super::super::ElemBuf;
use super::take_iter;

/// An iterator-owned list buffer detached from its `IterState`.
///
/// Each element remains owned here until `mark_moved` transfers it into a
/// destination collection. On rollback, only elements that were not moved are
/// torn down; moved elements are owned by the destination's unwind guard.
struct OwnedListBuffer {
    data: *mut u8,
    len: usize,
    cap: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    moved: Vec<bool>,
}

impl OwnedListBuffer {
    /// Detach the direct list source owned by `state`.
    ///
    /// `require_unique` is needed when element storage will move into a
    /// differently-laid-out collection. List-to-list collection can transfer
    /// the iterator's buffer reference even when another list shares it.
    fn take(
        state: &mut IterState,
        expected_elem_size: usize,
        require_unique: bool,
        fallback_dec_fn: Option<extern "C" fn(*mut u8)>,
    ) -> Option<Self> {
        let IterState::List {
            data,
            len,
            pos,
            cap,
            elem_size,
            owns_data,
        } = state
        else {
            return None;
        };

        if !*owns_data
            || data.is_null()
            || *pos != 0
            || *cap <= 0
            || (*elem_size).max(1) as usize != expected_elem_size
            || (require_unique && !crate::ori_rc_is_unique(*data))
        {
            return None;
        }

        let data_value = *data;
        let len_value = (*len).max(0) as usize;
        let cap_value = *cap as usize;
        // Allocate bookkeeping before detaching the state so allocation panic
        // leaves the original iterator as the sole cleanup authority.
        let moved = vec![false; len_value];
        let elem_dec_fn = unsafe { crate::rc::load_elem_dec_fn(data_value) }.or(fallback_dec_fn);

        *data = ptr::null_mut();
        *len = 0;
        *pos = 0;
        *cap = 0;
        *owns_data = false;

        Some(Self {
            data: data_value,
            len: len_value,
            cap: cap_value,
            elem_size: expected_elem_size,
            elem_dec_fn,
            moved,
        })
    }

    fn element(&self, index: usize) -> *mut u8 {
        debug_assert!(index < self.len);
        unsafe { self.data.add(index * self.elem_size) }
    }

    fn mark_moved(&mut self, index: usize) {
        debug_assert!(index < self.len);
        self.moved[index] = true;
    }

    /// Transfer the entire list allocation to a list result.
    fn release_to_list(mut self) -> (*mut u8, usize, usize, Option<extern "C" fn(*mut u8)>) {
        let result = (self.data, self.len, self.cap, self.elem_dec_fn);
        self.data = ptr::null_mut();
        result
    }
}

impl Drop for OwnedListBuffer {
    fn drop(&mut self) {
        if self.data.is_null() {
            return;
        }
        if let Some(dec) = self.elem_dec_fn {
            for index in 0..self.len {
                if !self.moved[index] {
                    dec(self.element(index));
                }
            }
        }
        crate::ori_rc_free(self.data, self.cap * self.elem_size, 8);
    }
}

/// Unwind guard for the raw RC buffer built by `collect`.
///
/// The output list does not own the allocation until its fields have been
/// written to `out_ptr`. Until then this guard records exactly how many copied
/// elements were initialized and releases both their child RCs and the buffer
/// if advancing the source invokes a panicking user closure.
struct CollectBuffer {
    data: *mut u8,
    capacity: usize,
    initialized: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
}

impl CollectBuffer {
    fn new(capacity: usize, elem_size: usize, elem_dec_fn: Option<extern "C" fn(*mut u8)>) -> Self {
        Self {
            data: crate::ori_rc_alloc(capacity * elem_size, 8),
            capacity,
            initialized: 0,
            elem_size,
            elem_dec_fn,
        }
    }

    fn release_to_output(mut self) -> *mut u8 {
        if !self.data.is_null() {
            unsafe {
                crate::rc::store_elem_dec_fn(self.data, self.elem_dec_fn);
                crate::rc::store_elem_count(self.data, self.initialized as i64);
            }
        }
        let data = self.data;
        self.data = ptr::null_mut();
        data
    }
}

impl Drop for CollectBuffer {
    fn drop(&mut self) {
        if self.data.is_null() {
            return;
        }
        crate::ori_buffer_rc_dec(
            self.data,
            self.initialized as i64,
            self.capacity as i64,
            self.elem_size as i64,
            self.elem_dec_fn,
        );
    }
}

/// Unwind guard for the hash-table buffer built by `collect_set`.
struct CollectSetBuffer {
    data: *mut u8,
    capacity: usize,
    initialized: usize,
    elem_size: usize,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
}

impl CollectSetBuffer {
    fn new(capacity: usize, elem_size: usize, elem_dec_fn: Option<extern "C" fn(*mut u8)>) -> Self {
        Self {
            data: crate::set::alloc_set_hash_buffer(capacity, elem_size, elem_dec_fn),
            capacity,
            initialized: 0,
            elem_size,
            elem_dec_fn,
        }
    }

    fn release_to_output(mut self) -> *mut u8 {
        let data = self.data;
        self.data = ptr::null_mut();
        data
    }
}

impl Drop for CollectSetBuffer {
    fn drop(&mut self) {
        if self.data.is_null() {
            return;
        }
        crate::ori_set_buffer_drop_unique(
            self.data,
            self.capacity as i64,
            self.initialized as i64,
            self.elem_size as i64,
            self.elem_dec_fn,
        );
    }
}

/// Move a uniquely-owned direct list source into a new set hash table.
///
/// Source slots stay owned by `source` until their bytes have been installed
/// in an occupied destination bucket. The two guards therefore partition
/// ownership at every step: unwinding drops each logical element exactly once,
/// while successful deduplication leaves duplicates source-owned.
fn collect_unique_list_into_set(
    mut source: OwnedListBuffer,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    out_ptr: *mut u8,
) {
    use crate::map::hash_table::{
        needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_set, set_meta,
        HashTableLayout, META_OCCUPIED,
    };
    use crate::set::write_set_struct;

    let es = source.elem_size;
    let initial_cap = next_hash_capacity(0);
    let mut buffer = CollectSetBuffer::new(initial_cap, es, source.elem_dec_fn);

    for index in 0..source.len {
        let element = source.element(index);
        let layout = HashTableLayout::for_set(buffer.capacity, es);
        let hash = elem_hash(element.cast_const());

        if unsafe {
            probe_find(
                buffer.data,
                buffer.capacity,
                layout.keys_offset,
                element.cast_const(),
                hash,
                es,
                elem_eq,
            )
        }
        .is_some()
        {
            continue;
        }

        if needs_rehash(buffer.initialized + 1, buffer.capacity) {
            let new_cap = buffer.capacity * 2;
            let old_dec = unsafe { crate::rc::load_elem_dec_fn(buffer.data) };
            let new_data = unsafe {
                rehash_set(
                    buffer.data,
                    buffer.capacity,
                    new_cap,
                    es,
                    elem_hash,
                    old_dec,
                )
            };
            crate::ori_rc_free(buffer.data, layout.total_size, 8);
            buffer.data = new_data;
            buffer.capacity = new_cap;
        }

        let bucket = unsafe { probe_find_slot(buffer.data, buffer.capacity, hash) };
        let layout = HashTableLayout::for_set(buffer.capacity, es);
        unsafe {
            let dst = buffer.data.add(layout.keys_offset + bucket * es);
            ptr::copy_nonoverlapping(element.cast_const(), dst, es);
            set_meta(buffer.data, bucket, META_OCCUPIED);
        }
        source.mark_moved(index);
        buffer.initialized += 1;
    }

    let len = buffer.initialized;
    let cap = buffer.capacity;
    // Discarded duplicates run their teardown while the destination guard still
    // owns every moved element. Only after that succeeds does the result escape.
    drop(source);
    let data = buffer.release_to_output();
    write_set_struct(out_ptr, len as i64, cap as i64, data);
}

// Collect

/// Collect all remaining elements into a new list.
///
/// Returns an `OriList { len: i64, cap: i64, data: *mut u8 }` by writing
/// to the caller-provided `out_ptr` (sret pattern to avoid >16 byte return).
///
/// `elem_size` is the byte size of each element.
/// For sources that cannot transfer ownership, `elem_inc_fn` increments child
/// RCs of each copied element before iterator teardown. A direct list source
/// transfers its existing buffer reference instead and performs no copy.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_collect(
    iter: *mut u8,
    elem_size: i64,
    elem_inc_fn: Option<extern "C" fn(*mut u8)>,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    assert_elem_size(elem_size, "ori_iter_collect");
    if iter.is_null() || out_ptr.is_null() {
        if out_ptr.is_null() {
            drop(take_iter(iter));
        }
        // Write empty list
        if !out_ptr.is_null() {
            unsafe {
                out_ptr.cast::<i64>().write(0); // len
                out_ptr.cast::<i64>().add(1).write(0); // cap
                out_ptr.add(16).cast::<*mut u8>().write(ptr::null_mut()); // data
            }
        }
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };
    let es = elem_size.max(1) as usize;

    // A direct list iterator already owns one reference to a correctly laid
    // out list buffer. Transfer that reference to the result instead of
    // materializing a second set of logical elements.
    if let Some(source) = OwnedListBuffer::take(&mut state, es, false, elem_dec_fn) {
        let (data, len, cap, stored_dec_fn) = source.release_to_list();
        unsafe {
            crate::rc::store_elem_count(data, len as i64);
            crate::rc::store_elem_dec_fn(data, stored_dec_fn);
            out_ptr.cast::<i64>().write(len as i64);
            out_ptr.cast::<i64>().add(1).write(cap as i64);
            out_ptr.add(16).cast::<*mut u8>().write(data);
        }
        return;
    }

    // Start with capacity 8, grow by doubling
    let mut buffer = CollectBuffer::new(8, es, elem_dec_fn);

    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        if buffer.initialized >= buffer.capacity {
            let new_cap = buffer.capacity * 2;
            let new_data =
                crate::ori_rc_realloc(buffer.data, buffer.capacity * es, new_cap * es, 8);
            if new_data.is_null() {
                break;
            }
            buffer.data = new_data;
            buffer.capacity = new_cap;
        }
        unsafe {
            let dst = buffer.data.add(buffer.initialized * es);
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), dst, es);
            // Increment child RCs so collected element survives iterator Drop
            if let Some(inc) = elem_inc_fn {
                inc(dst);
            }
        }
        buffer.initialized += 1;
    }

    let len = buffer.initialized;
    let cap = buffer.capacity;
    let data = buffer.release_to_output();

    // Write OriList { len, cap, data } to out_ptr
    unsafe {
        out_ptr.cast::<i64>().write(len as i64);
        out_ptr.cast::<i64>().add(1).write(cap as i64);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

// Collect Set

/// Collect all remaining elements into a new hash table set.
///
/// Uses `elem_hash`/`elem_eq` for hash-based deduplication during collection.
/// Returns a set `{ len: i64, cap: i64, data: *mut u8 }` by writing
/// to the caller-provided `out_ptr` (sret pattern). Duplicates are
/// skipped — only the first occurrence is kept.
///
/// For sources that cannot transfer ownership, `elem_inc_fn` increments child
/// RCs of each inserted element before iterator teardown. A uniquely-owned
/// direct list source moves its elements under paired source/destination guards.
#[no_mangle]
pub extern "C-unwind" fn ori_iter_collect_set(
    iter: *mut u8,
    elem_size: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    elem_hash: extern "C" fn(*const u8) -> i64,
    elem_inc_fn: Option<extern "C" fn(*mut u8)>,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    use crate::map::hash_table::{
        needs_rehash, next_hash_capacity, probe_find, probe_find_slot, rehash_set, set_meta,
        HashTableLayout, META_OCCUPIED,
    };
    use crate::set::write_set_struct;
    assert_elem_size(elem_size, "ori_iter_collect_set");

    if iter.is_null() || out_ptr.is_null() {
        if out_ptr.is_null() {
            drop(take_iter(iter));
        }
        if !out_ptr.is_null() {
            write_set_struct(out_ptr, 0, 0, ptr::null_mut());
        }
        return;
    }

    let Some(mut state) = take_iter(iter) else {
        return;
    };
    let es = elem_size.max(1) as usize;

    if let Some(source) = OwnedListBuffer::take(&mut state, es, true, elem_dec_fn) {
        collect_unique_list_into_set(source, elem_eq, elem_hash, out_ptr);
        return;
    }

    let initial_cap = next_hash_capacity(0);
    let mut buffer = CollectSetBuffer::new(initial_cap, es, elem_dec_fn);

    let mut elem_buf = ElemBuf::new();
    while unsafe { state.next(elem_buf.as_mut_ptr(), elem_size) } {
        let _yield = YieldGuard::new(&mut state, elem_buf.as_mut_ptr());
        let layout = HashTableLayout::for_set(buffer.capacity, es);
        let hash = elem_hash(elem_buf.as_ptr());

        // Deduplicate via hash probe
        if unsafe {
            probe_find(
                buffer.data,
                buffer.capacity,
                layout.keys_offset,
                elem_buf.as_ptr(),
                hash,
                es,
                elem_eq,
            )
        }
        .is_some()
        {
            continue;
        }

        // Rehash if load factor exceeded
        if needs_rehash(buffer.initialized + 1, buffer.capacity) {
            let new_cap = buffer.capacity * 2;
            let old_dec = unsafe { crate::rc::load_elem_dec_fn(buffer.data) };
            let new_data = unsafe {
                rehash_set(
                    buffer.data,
                    buffer.capacity,
                    new_cap,
                    es,
                    elem_hash,
                    old_dec,
                )
            };
            crate::ori_rc_free(buffer.data, layout.total_size, 8);
            buffer.data = new_data;
            buffer.capacity = new_cap;
        }

        // Find empty slot and insert
        let bucket = unsafe { probe_find_slot(buffer.data, buffer.capacity, hash) };
        let layout = HashTableLayout::for_set(buffer.capacity, es);
        unsafe {
            let dst = buffer.data.add(layout.keys_offset + bucket * es);
            ptr::copy_nonoverlapping(elem_buf.as_ptr(), dst, es);
            set_meta(buffer.data, bucket, META_OCCUPIED);
            // Increment child RCs so collected element survives iterator Drop
            if let Some(inc) = elem_inc_fn {
                inc(dst);
            }
        }
        buffer.initialized += 1;
    }

    let len = buffer.initialized;
    let cap = buffer.capacity;
    let data = buffer.release_to_output();
    write_set_struct(out_ptr, len as i64, cap as i64, data);
}
