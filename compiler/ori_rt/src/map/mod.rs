//! Map operations for AOT-compiled Ori programs.
//!
//! Ori maps use a single-buffer layout: `[key0..keyN | val0..valN]` where
//! keys start at `data + 0` and values at `data + cap * key_size`. This
//! single RC header enables one uniqueness check for COW operations.
//!
//! All key lookups use a `key_eq` callback for type-agnostic comparison.
//! The codegen generates type-specific comparators (e.g., inline `i64 ==`
//! for int keys, `ori_str_eq` for string keys).
//!
//! # Submodules
//!
//! - `cow` — COW mutation functions (`ori_map_insert_cow`, etc.) with consuming
//!   semantics: fast path mutates in place when RC==1, slow path copies.

pub mod cow;

use crate::list::write_array_to_list;

/// Ori map representation: { i64 len, i64 cap, *mut u8 data }
///
/// Single RC'd buffer with keys then values: `[key0..keyN | val0..valN]`.
/// Keys start at `data + 0`, values at `data + cap * key_size`.
/// Single RC header enables one uniqueness check for COW.
#[repr(C)]
pub struct OriMap {
    pub len: i64,
    pub cap: i64,
    pub data: *mut u8,
}

impl OriMap {
    /// Pointer to key at `index` within the data buffer.
    ///
    /// # Safety
    /// `index` must be < `self.len` and `self.data` must be valid.
    #[inline]
    pub unsafe fn key_ptr(&self, index: usize, key_size: usize) -> *const u8 {
        self.data.add(index * key_size)
    }

    /// Mutable pointer to key at `index`.
    ///
    /// # Safety
    /// `index` must be < `self.cap` and `self.data` must be valid.
    #[inline]
    pub unsafe fn key_ptr_mut(&self, index: usize, key_size: usize) -> *mut u8 {
        self.data.add(index * key_size)
    }

    /// Pointer to value at `index` within the data buffer.
    ///
    /// # Safety
    /// `index` must be < `self.len` and `self.data` must be valid.
    #[inline]
    pub unsafe fn value_ptr(&self, index: usize, key_size: usize, val_size: usize) -> *const u8 {
        self.data
            .add((self.cap as usize) * key_size + index * val_size)
    }

    /// Mutable pointer to value at `index`.
    ///
    /// # Safety
    /// `index` must be < `self.cap` and `self.data` must be valid.
    #[inline]
    pub unsafe fn value_ptr_mut(&self, index: usize, key_size: usize, val_size: usize) -> *mut u8 {
        self.data
            .add((self.cap as usize) * key_size + index * val_size)
    }

    /// Total byte size of the data buffer for given capacity.
    #[inline]
    pub fn buffer_size(cap: usize, key_size: usize, val_size: usize) -> usize {
        cap * key_size + cap * val_size
    }

    /// Allocate a new data buffer for `cap` entries and return its pointer.
    pub(crate) fn alloc_buffer(cap: usize, key_size: usize, val_size: usize) -> *mut u8 {
        let total = Self::buffer_size(cap, key_size, val_size);
        if total == 0 {
            return std::ptr::null_mut();
        }
        crate::rc::ori_rc_alloc(total, 8)
    }
}

/// Search for a key in the map's key region. Returns the index if found.
///
/// Used by `ori_map_get`, `ori_map_contains_key`, and COW mutation functions.
pub(crate) fn find_key(
    data: *const u8,
    len: usize,
    key_size: usize,
    needle: *const u8,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> Option<usize> {
    if data.is_null() || len == 0 {
        return None;
    }
    for i in 0..len {
        let k = unsafe { data.add(i * key_size) };
        if key_eq(k, needle) {
            return Some(i);
        }
    }
    None
}

/// Return an empty map sentinel (no allocation).
#[no_mangle]
pub extern "C" fn ori_map_empty() -> OriMap {
    OriMap {
        len: 0,
        cap: 0,
        data: std::ptr::null_mut(),
    }
}

/// Check if a map contains a key using a type-specific equality callback.
///
/// Map data buffer layout: `[key0..keyN | val0..valN]`.
/// Keys start at `data + 0`. Linear scan with `key_eq` comparison.
/// Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_map_contains_key(
    data: *const u8,
    len: i64,
    needle: *const u8,
    key_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let ks = key_size.max(1) as usize;
    i64::from(find_key(data, len as usize, ks, needle, key_eq).is_some())
}

/// Extract map keys as a new list.
///
/// Keys start at `data + 0`. Allocates a new buffer and copies all keys.
/// Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_map_keys_to_list(data: *const u8, len: i64, key_size: i64, out_ptr: *mut u8) {
    // Keys are at offset 0 in the data buffer, so data is the keys array.
    write_array_to_list(data, len, key_size, out_ptr);
}

/// Extract map values as a new list.
///
/// Values start at `data + cap * key_size`. Allocates a new buffer and copies
/// all values. Writes `{len, len, data_ptr}` to `out_ptr` (sret pattern).
#[no_mangle]
pub extern "C" fn ori_map_values_to_list(
    data: *const u8,
    cap: i64,
    len: i64,
    key_size: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    if data.is_null() || len <= 0 {
        write_array_to_list(std::ptr::null(), 0, val_size, out_ptr);
        return;
    }
    let vals_start = unsafe { data.add(cap as usize * key_size as usize) };
    write_array_to_list(vals_start, len, val_size, out_ptr);
}

/// Look up a key in a map and return `Option<V>` via sret.
///
/// Scans the keys region using `key_eq` callback. Writes `{tag: i64, value}`
/// to `out_ptr`. Tag 0 = Some (value copied), tag 1 = None.
///
/// Map data layout: `[key0..keyN | val0..valN]`, values at `data + cap * key_size`.
#[no_mangle]
pub extern "C" fn ori_map_get(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const u8,
    key_size: i64,
    val_size: i64,
    key_eq: extern "C" fn(*const u8, *const u8) -> bool,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;

    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }

    let n = len as usize;
    let c = cap as usize;

    if let Some(i) = find_key(data, n, ks, needle, key_eq) {
        // Some: tag = 0, copy value
        let vals_start = unsafe { data.add(c * ks) };
        unsafe {
            out_ptr.cast::<i64>().write(0);
            std::ptr::copy_nonoverlapping(vals_start.add(i * vs), out_ptr.add(8), vs);
        }
    } else {
        // None: tag = 1
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
    }
}

/// Write a map struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
pub(crate) fn write_map_struct(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}
