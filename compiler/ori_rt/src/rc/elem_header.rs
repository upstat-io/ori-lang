//! Element header helpers for RC-managed collection buffers.
//!
//! V5 RC header layout stores `elem_dec_fn` (element destructor function pointer)
//! and `elem_count` (number of initialized elements) alongside the traditional
//! `data_size` and `strong_count` fields. These helpers read/write those slots.
//!
//! The FFI wrappers (`ori_buffer_store_elem_dec`, `ori_buffer_store_elem_count`)
//! are callable from LLVM-generated code at collection construction time.

#[cfg(not(feature = "single-threaded"))]
use std::sync::atomic::Ordering;

/// Byte offset from data pointer to the `elem_dec_fn` field in the RC header.
///
/// `data_ptr.sub(ELEM_DEC_FN_OFFSET)` reaches `elem_dec_fn` at base + 8.
pub const ELEM_DEC_FN_OFFSET: usize = 24;

/// Byte offset from data pointer to the `elem_count` field in the RC header.
///
/// `data_ptr.sub(ELEM_COUNT_OFFSET)` reaches `elem_count` at base + 16.
/// Stores the number of initialized elements for full-range cleanup when
/// a slice is the last owner.
pub const ELEM_COUNT_OFFSET: usize = 16;

/// Store an element destructor function pointer in the RC header.
///
/// Writes `f` at `data_ptr - 24` (the `elem_dec_fn` slot). If `f` is `None`,
/// writes null. Used by `ori_buffer_rc_dec` to persist the element cleanup
/// function so that any dec call reaching zero can perform element cleanup.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 24` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn store_elem_dec_fn(data: *mut u8, f: Option<extern "C" fn(*mut u8)>) {
    let slot = data
        .sub(ELEM_DEC_FN_OFFSET)
        .cast::<Option<extern "C" fn(*mut u8)>>();
    slot.write(f);
}

/// Load the element destructor function pointer from the RC header.
///
/// Reads from `data_ptr - 24` (the `elem_dec_fn` slot). Returns `None` if
/// the slot is null (zero-initialized by `ori_rc_alloc`).
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 24` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn load_elem_dec_fn(data: *mut u8) -> Option<extern "C" fn(*mut u8)> {
    let slot = data
        .sub(ELEM_DEC_FN_OFFSET)
        .cast::<Option<extern "C" fn(*mut u8)>>();
    slot.read()
}

/// Load the element destructor function pointer from a `*const u8` data pointer.
///
/// Identical to [`load_elem_dec_fn`] but accepts `*const u8` for callers that
/// only have an immutable pointer (e.g., `query.rs` functions receiving
/// `data: *const u8`). The read is logically immutable — it reads from the
/// header without modification.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
pub unsafe fn load_elem_dec_fn_const(data: *const u8) -> Option<extern "C" fn(*mut u8)> {
    let slot = data
        .sub(ELEM_DEC_FN_OFFSET)
        .cast::<Option<extern "C" fn(*mut u8)>>();
    slot.read()
}

/// Atomically store `elem_dec_fn` in the header using a write-once pattern.
///
/// If the current header value is NULL and `f` is non-NULL, stores `f`.
/// If the header already has a non-NULL value, this is a no-op (the existing
/// value is kept). Thread-safe: uses compare-exchange on the non-single-threaded
/// path, plain write on the single-threaded path.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
pub unsafe fn store_elem_dec_fn_once(data: *mut u8, f: Option<extern "C" fn(*mut u8)>) {
    let Some(func) = f else { return };
    let slot = data.sub(ELEM_DEC_FN_OFFSET);

    #[cfg(not(feature = "single-threaded"))]
    {
        use std::sync::atomic::AtomicPtr;
        let atomic_slot = slot.cast::<AtomicPtr<()>>();
        // CAS: null → func. If already non-null, the existing value wins.
        // Relaxed ordering: the function pointer is always the same value for
        // a given buffer type, so no synchronization is needed beyond atomicity.
        let _ = (*atomic_slot).compare_exchange(
            std::ptr::null_mut(),
            func as *mut (),
            Ordering::Relaxed,
            Ordering::Relaxed,
        );
    }

    #[cfg(feature = "single-threaded")]
    {
        let current = slot.cast::<*const ()>().read();
        if current.is_null() {
            slot.cast::<*const ()>().write(func as *const ());
        }
    }
}

/// Store the initialized element count in the RC header.
///
/// Writes `count` at `data_ptr - 16` (the `elem_count` slot). Non-slice callers
/// of `ori_buffer_rc_dec` store their `len` here so that if a slice is the last
/// owner, `slice_buffer_rc_dec` can read the full element count for cleanup.
///
/// Uses a "last write wins" strategy: each non-slice dec overwrites with the
/// current `len`. Slices never write to this field. At RC → 0, the stored value
/// reflects the element count from the most recent non-slice owner's dec call.
///
/// Thread-safe: uses an atomic store on the multi-threaded path. `Relaxed`
/// ordering is sufficient because the refcount's `Release`/`Acquire` pair
/// (in `rc_dec_to_zero`) already establishes the happens-before relationship
/// between this store and the cleanup thread's load.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 16` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn store_elem_count(data: *mut u8, count: i64) {
    #[cfg(not(feature = "single-threaded"))]
    {
        use std::sync::atomic::AtomicI64;
        let atomic_slot = data.sub(ELEM_COUNT_OFFSET).cast::<AtomicI64>();
        (*atomic_slot).store(count, Ordering::Relaxed);
    }

    #[cfg(feature = "single-threaded")]
    {
        let slot = data.sub(ELEM_COUNT_OFFSET).cast::<i64>();
        slot.write(count);
    }
}

/// Load the initialized element count from the RC header.
///
/// Reads from `data_ptr - 16` (the `elem_count` slot). Returns 0 if never
/// stored (zero-initialized by `ori_rc_alloc`).
///
/// Used by `slice_buffer_rc_dec` to determine how many elements to clean up
/// when a slice is the last owner of a buffer.
///
/// Thread-safe: uses an atomic load on the multi-threaded path. `Relaxed`
/// ordering pairs with the `Relaxed` store in [`store_elem_count`] —
/// synchronization is provided by the refcount's `Release`/`Acquire` pair.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc` (so `data - 16` is valid
/// and 8-byte aligned within the RC header).
pub unsafe fn load_elem_count(data: *mut u8) -> i64 {
    #[cfg(not(feature = "single-threaded"))]
    {
        use std::sync::atomic::AtomicI64;
        let atomic_slot = data.sub(ELEM_COUNT_OFFSET).cast::<AtomicI64>();
        (*atomic_slot).load(Ordering::Relaxed)
    }

    #[cfg(feature = "single-threaded")]
    {
        let slot = data.sub(ELEM_COUNT_OFFSET).cast::<i64>();
        slot.read()
    }
}

/// Load the initialized element count from a `*const u8` data pointer.
///
/// Identical to [`load_elem_count`] but accepts `*const u8` for callers that
/// only have an immutable pointer. The read is logically immutable.
///
/// Thread-safe: uses an atomic load on the multi-threaded path.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
pub unsafe fn load_elem_count_const(data: *const u8) -> i64 {
    #[cfg(not(feature = "single-threaded"))]
    {
        use std::sync::atomic::AtomicI64;
        let atomic_slot = data.sub(ELEM_COUNT_OFFSET).cast::<AtomicI64>();
        (*atomic_slot).load(Ordering::Relaxed)
    }

    #[cfg(feature = "single-threaded")]
    {
        let slot = data.sub(ELEM_COUNT_OFFSET).cast::<i64>();
        slot.read()
    }
}

// FFI wrappers callable from LLVM-generated code

/// Store `elem_dec_fn` in a buffer's RC header at collection construction time.
///
/// Called by LLVM codegen after constructing a list/set literal to populate
/// the header's element destructor slot. Wraps [`store_elem_dec_fn`].
///
/// `elem_dec_fn` is NULL for scalar element types (int, float, bool) — the
/// header is already zero-initialized by `ori_rc_alloc`, so the call can be
/// skipped by codegen for scalar elements.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
#[no_mangle]
pub extern "C" fn ori_buffer_store_elem_dec(
    data: *mut u8,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
) {
    if data.is_null() {
        return;
    }
    // SAFETY: caller guarantees data was returned by ori_rc_alloc.
    unsafe { store_elem_dec_fn(data, elem_dec_fn) }
}

/// Store `elem_count` in a buffer's RC header at collection construction time.
///
/// Called by LLVM codegen after constructing a list/set literal to record
/// the number of initialized elements. Required so that `slice_buffer_rc_dec`
/// can clean up all elements when a slice outlives its source buffer.
///
/// # Safety
///
/// `data` must have been returned by `ori_rc_alloc`.
#[no_mangle]
pub extern "C" fn ori_buffer_store_elem_count(data: *mut u8, count: i64) {
    if data.is_null() {
        return;
    }
    // SAFETY: caller guarantees data was returned by ori_rc_alloc.
    unsafe { store_elem_count(data, count) }
}
