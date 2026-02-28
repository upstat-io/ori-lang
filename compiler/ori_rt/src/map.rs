//! Map operations for AOT-compiled Ori programs.
//!
//! Ori maps use a single-buffer layout: `[key0..keyN | val0..valN]` where
//! keys start at `data + 0` and values at `data + cap * key_size`. This
//! single RC header enables one uniqueness check for COW operations.
//!
//! All map operations currently return new maps via sret pattern — in-place
//! COW mutation planned for §04.2–§04.3.

use crate::list::write_array_to_list;
use crate::string::deref_str;
use crate::OriStr;

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
    fn alloc_buffer(cap: usize, key_size: usize, val_size: usize) -> *mut u8 {
        let total = Self::buffer_size(cap, key_size, val_size);
        if total == 0 {
            return std::ptr::null_mut();
        }
        crate::rc::ori_rc_alloc(total, 8)
    }
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

/// Check if a map contains a string key.
///
/// Map data buffer layout: `[key0..keyN | val0..valN]`.
/// Keys start at `data + 0`. Linear scan with string comparison.
/// Returns 1 if found, 0 otherwise.
#[no_mangle]
pub extern "C" fn ori_map_contains_key(data: *const u8, len: i64, needle: *const OriStr) -> i64 {
    if data.is_null() || len <= 0 || needle.is_null() {
        return 0;
    }
    let needle_str = unsafe { deref_str(needle) };
    let key_size = std::mem::size_of::<OriStr>();
    for i in 0..len as usize {
        let key_ptr = unsafe { data.add(i * key_size).cast::<OriStr>() };
        let key_str = unsafe { (*key_ptr).as_str() };
        if key_str == needle_str {
            return 1;
        }
    }
    0
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
/// Scans the keys region for a matching string key. Writes `{tag: i64, value}`
/// to `out_ptr`. Tag 0 = Some (value copied), tag 1 = None.
///
/// Map data layout: `[key0..keyN | val0..valN]`, values at `data + cap * key_size`.
#[no_mangle]
pub extern "C" fn ori_map_get(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const OriStr,
    key_size: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let needle_str = unsafe { deref_str(needle) };

    if data.is_null() || len <= 0 {
        unsafe {
            out_ptr.cast::<i64>().write(1);
        }
        return;
    }

    let n = len as usize;
    let c = cap as usize;
    for i in 0..n {
        let kp = unsafe { data.add(i * ks).cast::<OriStr>() };
        let key_str = unsafe { (*kp).as_str() };
        if key_str == needle_str {
            // Some: tag = 0, copy value
            let vals_start = unsafe { data.add(c * ks) };
            unsafe {
                out_ptr.cast::<i64>().write(0);
                std::ptr::copy_nonoverlapping(vals_start.add(i * vs), out_ptr.add(8), vs);
            }
            return;
        }
    }

    unsafe {
        out_ptr.cast::<i64>().write(1);
    }
}

/// Insert a key-value pair into a map, returning a new map via sret.
///
/// If the key already exists, its value is updated. Otherwise a new entry
/// is appended. Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
///
/// Data layout: `[key0..keyN | val0..valN]`, values at `data + cap * key_size`.
#[no_mangle]
pub extern "C" fn ori_map_insert(
    data: *const u8,
    cap: i64,
    len: i64,
    key: *const OriStr,
    val_ptr: *const u8,
    key_size: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() || key.is_null() || val_ptr.is_null() {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;
    let needle_str = unsafe { deref_str(key) };

    // Check if key already exists
    let mut found_idx: Option<usize> = None;
    if !data.is_null() && n > 0 {
        for i in 0..n {
            let k = unsafe { data.add(i * ks).cast::<OriStr>() };
            if unsafe { (*k).as_str() } == needle_str {
                found_idx = Some(i);
                break;
            }
        }
    }

    if let Some(idx) = found_idx {
        // Key exists — allocate new buffer, copy, overwrite value at idx
        let new_data = OriMap::alloc_buffer(n, ks, vs);
        if !data.is_null() {
            unsafe {
                // Copy keys
                std::ptr::copy_nonoverlapping(data, new_data, n * ks);
                // Copy values
                let old_vals = data.add(c * ks);
                let new_vals = new_data.add(n * ks);
                std::ptr::copy_nonoverlapping(old_vals, new_vals, n * vs);
                // Overwrite the value at the found index
                std::ptr::copy_nonoverlapping(val_ptr, new_vals.add(idx * vs), vs);
            }
        }
        write_map_struct(out_ptr, n as i64, n as i64, new_data);
    } else {
        // Key doesn't exist — allocate larger buffer, copy, append
        let new_len = n + 1;
        let new_data = OriMap::alloc_buffer(new_len, ks, vs);
        if !data.is_null() && n > 0 {
            unsafe {
                // Copy existing keys
                std::ptr::copy_nonoverlapping(data, new_data, n * ks);
                // Copy existing values
                let old_vals = data.add(c * ks);
                let new_vals = new_data.add(new_len * ks);
                std::ptr::copy_nonoverlapping(old_vals, new_vals, n * vs);
            }
        }
        // Append new key and value
        unsafe {
            std::ptr::copy_nonoverlapping(key.cast::<u8>(), new_data.add(n * ks), ks);
            let new_vals = new_data.add(new_len * ks);
            std::ptr::copy_nonoverlapping(val_ptr, new_vals.add(n * vs), vs);
        }
        write_map_struct(out_ptr, new_len as i64, new_len as i64, new_data);
    }
}

/// Remove a key from a map, returning a new map via sret.
///
/// If the key doesn't exist, the result is a copy of the original map.
/// Writes `{i64 len, i64 cap, ptr data}` to `out_ptr`.
///
/// Data layout: `[key0..keyN | val0..valN]`, values at `data + cap * key_size`.
#[no_mangle]
pub extern "C" fn ori_map_remove(
    data: *const u8,
    cap: i64,
    len: i64,
    needle: *const OriStr,
    key_size: i64,
    val_size: i64,
    out_ptr: *mut u8,
) {
    if out_ptr.is_null() {
        return;
    }
    let ks = key_size.max(1) as usize;
    let vs = val_size.max(1) as usize;
    let n = len.max(0) as usize;
    let c = cap.max(0) as usize;
    let needle_str = unsafe { deref_str(needle) };

    if data.is_null() || n == 0 {
        write_map_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    // Find the key to remove
    let mut remove_idx: Option<usize> = None;
    for i in 0..n {
        let k = unsafe { data.add(i * ks).cast::<OriStr>() };
        if unsafe { (*k).as_str() } == needle_str {
            remove_idx = Some(i);
            break;
        }
    }

    let old_vals = unsafe { data.add(c * ks) };

    let Some(idx) = remove_idx else {
        // Key not found — return a copy of the original map
        let new_data = OriMap::alloc_buffer(n, ks, vs);
        unsafe {
            std::ptr::copy_nonoverlapping(data, new_data, n * ks);
            let new_vals = new_data.add(n * ks);
            std::ptr::copy_nonoverlapping(old_vals, new_vals, n * vs);
        }
        write_map_struct(out_ptr, n as i64, n as i64, new_data);
        return;
    };

    let new_len = n - 1;
    if new_len == 0 {
        write_map_struct(out_ptr, 0, 0, std::ptr::null_mut());
        return;
    }

    let new_data = OriMap::alloc_buffer(new_len, ks, vs);
    let new_vals = unsafe { new_data.add(new_len * ks) };

    // Copy keys before and after the removed index
    if idx > 0 {
        unsafe { std::ptr::copy_nonoverlapping(data, new_data, idx * ks) };
    }
    let after = n - idx - 1;
    if after > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                data.add((idx + 1) * ks),
                new_data.add(idx * ks),
                after * ks,
            );
        }
    }

    // Copy values before and after the removed index
    if idx > 0 {
        unsafe { std::ptr::copy_nonoverlapping(old_vals, new_vals, idx * vs) };
    }
    if after > 0 {
        unsafe {
            std::ptr::copy_nonoverlapping(
                old_vals.add((idx + 1) * vs),
                new_vals.add(idx * vs),
                after * vs,
            );
        }
    }

    write_map_struct(out_ptr, new_len as i64, new_len as i64, new_data);
}

/// Write a map struct `{i64 len, i64 cap, ptr data}` to `out_ptr`.
fn write_map_struct(out_ptr: *mut u8, len: i64, cap: i64, data: *mut u8) {
    unsafe {
        out_ptr.cast::<i64>().write(len);
        out_ptr.cast::<i64>().add(1).write(cap);
        out_ptr.add(16).cast::<*mut u8>().write(data);
    }
}
