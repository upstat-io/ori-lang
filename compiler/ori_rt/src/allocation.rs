//! General-purpose runtime allocation and collection-capacity primitives.

/// Compute the next capacity for a collection that must hold `required` elements.
#[inline]
pub(crate) fn next_capacity(current: usize, required: usize) -> usize {
    const MINIMUM: usize = 4;

    current.saturating_mul(2).max(required).max(MINIMUM)
}

/// Allocate uninitialized memory with the given size and alignment.
#[no_mangle]
pub extern "C" fn ori_alloc(size: usize, align: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }

    let align = align.max(8);
    let layout = match std::alloc::Layout::from_size_align(size, align) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: `layout` has nonzero size and a valid alignment.
    unsafe { std::alloc::alloc(layout) }
}

/// Free memory previously allocated with [`ori_alloc`].
///
/// # Safety
///
/// `ptr` must identify a live allocation created with the same size and alignment.
#[no_mangle]
pub extern "C" fn ori_free(ptr: *mut u8, size: usize, align: usize) {
    if ptr.is_null() || size == 0 {
        return;
    }

    let align = align.max(8);
    let layout = match std::alloc::Layout::from_size_align(size, align) {
        Ok(layout) => layout,
        Err(_) => return,
    };

    // SAFETY: The caller supplies the allocation's original layout.
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// Reallocate memory while preserving the overlapping initialized prefix.
#[no_mangle]
pub extern "C" fn ori_realloc(
    ptr: *mut u8,
    old_size: usize,
    new_size: usize,
    align: usize,
) -> *mut u8 {
    if ptr.is_null() {
        return ori_alloc(new_size, align);
    }
    if new_size == 0 {
        ori_free(ptr, old_size, align);
        return std::ptr::null_mut();
    }

    let align = align.max(8);
    let old_layout = match std::alloc::Layout::from_size_align(old_size, align) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };

    // SAFETY: The caller supplies the allocation's original layout.
    unsafe { std::alloc::realloc(ptr, old_layout, new_size) }
}
