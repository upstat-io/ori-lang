//! COW (Copy-on-Write) set mutation functions.
//!
//! All functions in this module follow consuming semantics: they take ownership
//! of the caller's reference to the data buffer and produce a new `{len, cap, data}`
//! triple via the `out_ptr` sret pattern.
//!
//! Sets use the same contiguous layout as lists, so COW mechanics are simpler
//! than maps (no split-buffer relocation needed).

use crate::list::inc_copied_elements;
use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_dec, ori_rc_free, ori_rc_is_unique, ori_rc_realloc};

/// Write `{i64 len, i64 cap, ptr data}` to `out_ptr`.
fn write_set_output(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}

/// Linear scan for an element using the provided equality function.
fn find_elem(
    data: *const u8,
    len: usize,
    elem_size: usize,
    needle: *const u8,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> Option<usize> {
    if data.is_null() || len == 0 {
        return None;
    }
    for i in 0..len {
        let existing = unsafe { data.add(i * elem_size) };
        if elem_eq(existing, needle) {
            return Some(i);
        }
    }
    None
}

/// Check if a data buffer contains an element (using the provided equality function).
fn contains_elem(
    data: *const u8,
    len: usize,
    elem_size: usize,
    needle: *const u8,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> bool {
    find_elem(data, len, elem_size, needle, elem_eq).is_some()
}

/// COW-aware set insert with consuming semantics.
///
/// Inserts an element into a set. The data buffer must be RC-allocated
/// (via `ori_rc_alloc`) or null (empty sentinel). This function **takes
/// ownership** of the caller's reference to `data`:
///
/// - **No-op** (element exists): Returns input unchanged regardless of sharing.
/// - **Fast path** (unique, has capacity): Appends in place.
/// - **Fast path** (unique, needs growth): Realloc + append.
/// - **Slow path** (shared or empty): New buffer allocated, old elements
///   byte-copied, new element appended. Old buffer's RC decremented.
///
/// # Element RC
///
/// On the slow path, byte-copied elements get their RC incremented via
/// `inc_fn`. Pass null for scalar element types.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_insert_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem: *const u8,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Check if element already exists — no-op if so
    if find_elem(data, n, es, elem, elem_eq).is_some() {
        write_set_output(out_ptr, len, cap, data);
        return;
    }

    let new_len = n + 1;

    // FAST PATH: unique owner — can mutate in place
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));
    if is_unique {
        if c >= new_len {
            // Has capacity — write element in place
            unsafe {
                std::ptr::copy_nonoverlapping(elem, data.add(n * es), es);
            }
            write_set_output(out_ptr, new_len as i64, cap, data);
            return;
        }

        // Needs growth — realloc
        let new_cap = next_capacity(c, new_len);
        let new_data = ori_rc_realloc(data, c * es, new_cap * es, ea);
        if new_data.is_null() {
            // Realloc failed — return original unchanged
            write_set_output(out_ptr, len, cap, data);
            return;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(elem, new_data.add(n * es), es);
        }
        write_set_output(out_ptr, new_len as i64, new_cap as i64, new_data);
        return;
    }

    // SLOW PATH: shared or empty — allocate new buffer.
    // Use tight-fit capacity (= new_len) instead of doubling the old capacity.
    // Doubling only benefits the fast path (unique owner, same buffer reused).
    // On the slow path, over-allocating causes exponential capacity growth in
    // shared-insert loops.
    let new_cap = if data.is_null() {
        next_capacity(0, new_len)
    } else {
        new_len
    };
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // Copy old elements and increment their RC
    if !data.is_null() && n > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, n * es);
        }
        inc_copied_elements(new_data, n, es, inc_fn);
    }

    // Write new element
    unsafe {
        std::ptr::copy_nonoverlapping(elem, new_data.add(n * es), es);
    }

    // Release old buffer reference
    ori_rc_dec(data, None);

    write_set_output(out_ptr, new_len as i64, new_cap as i64, new_data);
}

/// COW-aware set remove with consuming semantics.
///
/// Removes an element from a set. The data buffer must be RC-allocated
/// or null (empty sentinel). This function **takes ownership** of the
/// caller's reference to `data`:
///
/// - **No-op** (element not found): Returns input unchanged.
/// - **Fast path** (unique, found): Shifts remaining elements left in place.
/// - **Fast path** (unique, last element): Frees the buffer, returns empty
///   sentinel.
/// - **Slow path** (shared, found): Allocates new buffer, copies all elements
///   except the removed one. Old buffer's RC decremented.
///
/// # Element RC
///
/// On the slow path, byte-copied elements get their RC incremented via
/// `inc_fn`. The removed element's RC is NOT decremented here — the caller
/// (codegen) is responsible.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_remove_cow(
    data: *mut u8,
    len: i64,
    cap: i64,
    elem: *const u8,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || elem.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;

    // Find element
    let Some(idx) = find_elem(data, n, es, elem, elem_eq) else {
        // Not found — return input unchanged
        write_set_output(out_ptr, len, cap, data);
        return;
    };

    let new_len = n - 1;
    let tail = n - idx - 1; // elements after the removed one

    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique = !data.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(data)));

    // Special case: removing last element → empty sentinel
    if new_len == 0 {
        if !data.is_null() {
            if is_unique {
                ori_rc_free(data, c * es, ea);
            } else {
                ori_rc_dec(data, None);
            }
        }
        write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique owner — shift left in place
    if is_unique {
        if tail > 0 {
            unsafe {
                std::ptr::copy(data.add((idx + 1) * es), data.add(idx * es), tail * es);
            }
        }
        write_set_output(out_ptr, new_len as i64, cap, data);
        return;
    }

    // SLOW PATH: shared — allocate new buffer, copy all except removed
    let new_data = ori_rc_alloc(new_len * es, ea);

    unsafe {
        if idx > 0 {
            std::ptr::copy_nonoverlapping(data, new_data, idx * es);
        }
        if tail > 0 {
            std::ptr::copy_nonoverlapping(
                data.add((idx + 1) * es),
                new_data.add(idx * es),
                tail * es,
            );
        }
    }

    // Inc RC for all copied elements (contiguous in new buffer)
    inc_copied_elements(new_data, new_len, es, inc_fn);

    // Release old buffer reference
    ori_rc_dec(data, None);

    write_set_output(out_ptr, new_len as i64, new_len as i64, new_data);
}

/// COW-aware set union with consuming semantics.
///
/// Computes `set1 ∪ set2`. The data buffer for set1 must be RC-allocated
/// or null (empty sentinel). This function **takes ownership** of the
/// caller's reference to `d1` (consuming semantics). `d2` is borrowed
/// (read-only, not consumed).
///
/// - **Fast path** (set1 unique): Extends set1 in place with elements from
///   set2 that aren't already present. Reallocs if capacity is insufficient.
/// - **Slow path** (set1 shared or empty): Allocates a new buffer containing
///   all elements from set1 plus unique elements from set2. Decrements set1's RC.
///
/// # Element RC
///
/// Elements copied from either set get their RC incremented via `inc_fn`
/// on the slow path. On the fast path, elements appended from set2 are
/// byte-copied (no inc needed — they're new additions to set1's buffer).
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_union_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;

    // set2 empty → return set1 unchanged (identity: A∪∅ = A)
    if n2 == 0 || d2.is_null() {
        write_set_output(out_ptr, l1, c1, d1);
        return;
    }

    // set1 empty → copy set2 into a fresh buffer, release set1
    if n1 == 0 || d1.is_null() {
        let new_cap = next_capacity(0, n2);
        let new_data = ori_rc_alloc(new_cap * es, ea);
        unsafe { std::ptr::copy_nonoverlapping(d2, new_data, n2 * es) };
        inc_copied_elements(new_data, n2, es, inc_fn);
        ori_rc_dec(d1, None); // dec null is safe (no-op)
        write_set_output(out_ptr, n2 as i64, new_cap as i64, new_data);
        return;
    }

    // Count how many elements from set2 are new (not in set1)
    let mut new_count = 0usize;
    for i in 0..n2 {
        let elem = unsafe { d2.add(i * es) };
        if !contains_elem(d1, n1, es, elem, elem_eq) {
            new_count += 1;
        }
    }

    // No new elements → return set1 unchanged (A∪A = A, or A⊇B)
    if new_count == 0 {
        write_set_output(out_ptr, l1, c1, d1);
        return;
    }

    let result_len = n1 + new_count;

    // FAST PATH: unique owner — extend in place
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique = cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1));
    if is_unique {
        let (buf, out_cap) = if cap1 >= result_len {
            (d1, cap1)
        } else {
            let new_cap = next_capacity(cap1, result_len);
            let grown = ori_rc_realloc(d1, cap1 * es, new_cap * es, ea);
            if grown.is_null() {
                write_set_output(out_ptr, l1, c1, d1);
                return;
            }
            (grown, new_cap)
        };

        // Append elements from set2 not already in set1
        let mut write_pos = n1;
        for i in 0..n2 {
            let elem = unsafe { d2.add(i * es) };
            if !contains_elem(buf, n1, es, elem, elem_eq) {
                unsafe {
                    std::ptr::copy_nonoverlapping(elem, buf.add(write_pos * es), es);
                }
                write_pos += 1;
            }
        }
        write_set_output(out_ptr, result_len as i64, out_cap as i64, buf);
        return;
    }

    // SLOW PATH: shared — allocate new buffer with combined elements
    let new_cap = next_capacity(0, result_len);
    let new_data = ori_rc_alloc(new_cap * es, ea);

    // Copy all of set1
    unsafe { std::ptr::copy_nonoverlapping(d1, new_data, n1 * es) };

    // Append unique elements from set2
    let mut write_pos = n1;
    for i in 0..n2 {
        let elem = unsafe { d2.add(i * es) };
        if !contains_elem(d1, n1, es, elem, elem_eq) {
            unsafe {
                std::ptr::copy_nonoverlapping(elem, new_data.add(write_pos * es), es);
            }
            write_pos += 1;
        }
    }

    // Inc RC for all copied elements (set1 elements + appended set2 elements)
    inc_copied_elements(new_data, result_len, es, inc_fn);

    // Release old buffer reference
    ori_rc_dec(d1, None);

    write_set_output(out_ptr, result_len as i64, new_cap as i64, new_data);
}

/// COW-aware set intersection with consuming semantics.
///
/// Computes `set1 ∩ set2`. The data buffer for set1 must be RC-allocated
/// or null (empty sentinel). This function **takes ownership** of the
/// caller's reference to `d1`. `d2` is borrowed (not consumed).
///
/// - **Fast path** (set1 unique): Compacts set1 in place, keeping only
///   elements that also appear in set2. Uses a write cursor to avoid
///   shifting after each removal.
/// - **Slow path** (set1 shared or either empty): Allocates a new buffer
///   containing only elements present in both sets. Decrements set1's RC.
///
/// # Element RC
///
/// - Fast path: Elements removed in place are NOT RC-decremented here.
///   The caller (codegen) handles element cleanup.
/// - Slow path: Elements copied to the new buffer get their RC incremented
///   via `inc_fn`.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_intersection_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;

    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    // Note: d1 null check must come first; cow_mode=1 with null is invalid
    let d1_is_unique = !d1.is_null() && (cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1)));

    // Either set empty → result is empty (identity: A∩∅ = ∅)
    if n1 == 0 || d1.is_null() || n2 == 0 || d2.is_null() {
        if !d1.is_null() {
            if d1_is_unique {
                ori_rc_free(d1, cap1 * es, ea);
            } else {
                ori_rc_dec(d1, None);
            }
        }
        write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // FAST PATH: unique owner — compact in place
    if d1_is_unique {
        let mut write = 0usize;
        for read in 0..n1 {
            let elem = unsafe { d1.add(read * es) };
            if contains_elem(d2, n2, es, elem, elem_eq) {
                if write < read {
                    unsafe {
                        std::ptr::copy(d1.add(read * es), d1.add(write * es), es);
                    }
                }
                write += 1;
            }
        }

        if write == 0 {
            // All elements removed → free buffer, return empty
            ori_rc_free(d1, cap1 * es, ea);
            write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        } else {
            write_set_output(out_ptr, write as i64, c1, d1);
        }
        return;
    }

    // SLOW PATH: shared — allocate new buffer with intersection
    let max_result = n1.min(n2);
    let new_data = ori_rc_alloc(max_result * es, ea);
    let mut result_len = 0usize;

    for i in 0..n1 {
        let elem = unsafe { d1.add(i * es) };
        if contains_elem(d2, n2, es, elem, elem_eq) {
            unsafe {
                std::ptr::copy_nonoverlapping(elem, new_data.add(result_len * es), es);
            }
            result_len += 1;
        }
    }

    if result_len == 0 {
        // No common elements — free the new buffer, return empty
        ori_rc_free(new_data, max_result * es, ea);
        ori_rc_dec(d1, None);
        write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    inc_copied_elements(new_data, result_len, es, inc_fn);
    ori_rc_dec(d1, None);

    write_set_output(out_ptr, result_len as i64, result_len as i64, new_data);
}

/// COW-aware set difference with consuming semantics.
///
/// Computes `set1 \ set2` (elements in set1 not in set2). The data buffer
/// for set1 must be RC-allocated or null (empty sentinel). This function
/// **takes ownership** of the caller's reference to `d1`. `d2` is borrowed.
///
/// - **Fast path** (set1 unique): Compacts set1 in place, removing elements
///   that appear in set2. Same write-cursor compaction as intersection.
/// - **Slow path** (set1 shared or set1 empty): Allocates a new buffer
///   containing only elements from set1 not present in set2.
///
/// # Element RC
///
/// - Fast path: Elements removed in place are NOT RC-decremented here.
/// - Slow path: Elements copied to the new buffer get their RC incremented
///   via `inc_fn`.
///
/// # Output
///
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
#[no_mangle]
pub extern "C" fn ori_set_difference_cow(
    d1: *mut u8,
    l1: i64,
    c1: i64,
    d2: *const u8,
    l2: i64,
    elem_size: i64,
    elem_align: i64,
    elem_eq: extern "C" fn(*const u8, *const u8) -> bool,
    inc_fn: Option<extern "C" fn(*mut u8)>,
    cow_mode: i32,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }

    let es = elem_size.max(1) as usize;
    let ea = elem_align.max(1) as usize;
    let n1 = l1.max(0) as usize;
    let n2 = l2.max(0) as usize;
    let cap1 = c1.max(0) as usize;

    // set1 empty → result is empty (∅\B = ∅)
    if n1 == 0 || d1.is_null() {
        ori_rc_dec(d1, None);
        write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // set2 empty → result is set1 unchanged (identity: A\∅ = A)
    if n2 == 0 || d2.is_null() {
        write_set_output(out_ptr, l1, c1, d1);
        return;
    }

    // FAST PATH: unique owner — compact in place
    // cow_mode: 0=dynamic, 1=static unique, 2=static shared
    let is_unique = cow_mode == 1 || (cow_mode != 2 && ori_rc_is_unique(d1));
    if is_unique {
        let mut write = 0usize;
        for read in 0..n1 {
            let elem = unsafe { d1.add(read * es) };
            if !contains_elem(d2, n2, es, elem, elem_eq) {
                if write < read {
                    unsafe {
                        std::ptr::copy(d1.add(read * es), d1.add(write * es), es);
                    }
                }
                write += 1;
            }
        }

        if write == 0 {
            // All elements removed → free buffer, return empty
            ori_rc_free(d1, cap1 * es, ea);
            write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        } else {
            write_set_output(out_ptr, write as i64, c1, d1);
        }
        return;
    }

    // SLOW PATH: shared — allocate new buffer with difference
    let new_data = ori_rc_alloc(n1 * es, ea);
    let mut result_len = 0usize;

    for i in 0..n1 {
        let elem = unsafe { d1.add(i * es) };
        if !contains_elem(d2, n2, es, elem, elem_eq) {
            unsafe {
                std::ptr::copy_nonoverlapping(elem, new_data.add(result_len * es), es);
            }
            result_len += 1;
        }
    }

    if result_len == 0 {
        // All elements removed — free new buffer, return empty (A\A = ∅)
        ori_rc_free(new_data, n1 * es, ea);
        ori_rc_dec(d1, None);
        write_set_output(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    inc_copied_elements(new_data, result_len, es, inc_fn);
    ori_rc_dec(d1, None);

    write_set_output(out_ptr, result_len as i64, result_len as i64, new_data);
}
