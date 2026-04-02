//! Core string operations for AOT-compiled Ori programs.
//!
//! Contains `chars`, `split`, `concat`, `eq`/`ne`/`compare`, `hash`,
//! `len`, and `data`. Transformation methods (contains, trim, replace, etc.)
//! are in the sibling `methods` module.

use crate::list::write_array_to_list;
use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_inc, ori_rc_is_unique};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset};

use super::{OriStr, OriStrHeap, OriStrSSO, SSO_FLAG, SSO_MAX_LEN};

/// Convert a string into a list of chars (i32 Unicode scalar values).
///
/// Writes `{len, cap, data_ptr}` to `out_ptr` (sret pattern). Each element
/// is an `i32` representing a Unicode code point.
#[no_mangle]
pub extern "C" fn ori_str_chars(str_ptr: *const u8, str_len: i64, out_ptr: *mut u8) {
    if out_ptr.is_null() {
        return;
    }
    if str_ptr.is_null() || str_len <= 0 {
        write_array_to_list(std::ptr::null(), 0, 4, None, out_ptr);
        return;
    }
    // SAFETY: str_ptr validated non-null above; caller guarantees valid data for str_len bytes.
    let bytes = unsafe { std::slice::from_raw_parts(str_ptr, str_len as usize) };
    // SAFETY: Ori strings are valid UTF-8 by construction.
    let s = unsafe { std::str::from_utf8_unchecked(bytes) };

    let chars: Vec<i32> = s.chars().map(|c| c as i32).collect();
    let n = chars.len() as i64;
    write_array_to_list(chars.as_ptr().cast(), n, 4, None, out_ptr);
}

/// Split a string by a separator, returning a list of `OriStr` values.
///
/// Writes `{len, cap, data_ptr}` to `out_ptr` (sret pattern). Each element
/// is an `OriStr` (24 bytes).
///
/// When the source is a heap string, split pieces > 23 bytes are returned
/// as seamless slices (sharing the original buffer). Pieces <= 23 bytes
/// use SSO (no heap allocation, no RC). This hybrid approach minimizes
/// allocations while avoiding RC overhead for small pieces.
///
/// `str_cap` is the cap field of the source `OriStr` — needed to detect
/// slice-backed strings (from `substring`, `trim`, or prior `split`).
/// When the source is a seamless slice, sub-slices reference the original
/// allocation with adjusted byte offsets.
#[no_mangle]
pub extern "C" fn ori_str_split(
    str_ptr: *const u8,
    str_len: i64,
    str_cap: i64,
    sep_ptr: *const u8,
    sep_len: i64,
    elem_dec_fn: Option<extern "C" fn(*mut u8)>,
    out_ptr: *mut u8,
) {
    let elem_size = std::mem::size_of::<OriStr>();
    if out_ptr.is_null() {
        return;
    }
    if str_ptr.is_null() || str_len < 0 {
        write_array_to_list(std::ptr::null(), 0, elem_size as i64, None, out_ptr);
        return;
    }

    // SAFETY: str_ptr validated non-null above; caller guarantees valid data for str_len bytes.
    let bytes = unsafe { std::slice::from_raw_parts(str_ptr, str_len as usize) };
    // SAFETY: Ori strings are valid UTF-8 by construction.
    let s = unsafe { std::str::from_utf8_unchecked(bytes) };

    let sep_bytes = if sep_ptr.is_null() || sep_len <= 0 {
        ""
    } else {
        // SAFETY: sep_ptr validated non-null; caller guarantees valid UTF-8 data for sep_len bytes.
        unsafe {
            std::str::from_utf8_unchecked(std::slice::from_raw_parts(sep_ptr, sep_len as usize))
        }
    };

    // Collect parts with byte offsets for slice creation
    let parts: Vec<(usize, usize)> = {
        let mut result = Vec::new();
        let mut start = 0;
        if sep_bytes.is_empty() {
            // Empty separator: split into individual characters
            for (i, ch) in s.char_indices() {
                result.push((i, i + ch.len_utf8()));
            }
            if result.is_empty() {
                result.push((0, 0)); // empty string splits to [""]
            }
        } else {
            for (i, _) in s.match_indices(sep_bytes) {
                result.push((start, i));
                start = i + sep_bytes.len();
            }
            result.push((start, s.len()));
        }
        result
    };
    let n = parts.len();

    // Determine whether the source is backed by a heap allocation we can slice.
    //
    // Three cases:
    // - SSO (str_len <= 23): all pieces fit in SSO, no slicing needed
    // - Heap (cap >= 0, str_len > 23): can slice, str_ptr is allocation data
    // - Seamless slice (cap < 0 / SLICE_FLAG): can slice, str_ptr is interior
    //   pointer — sub-slices must reference the original allocation with
    //   adjusted byte offsets
    let (can_slice, base_offset) = if is_slice_cap(str_cap) {
        // Source is a seamless slice — sub-slices reference the original buffer
        let parent_offset = slice_byte_offset(str_cap);
        (true, parent_offset)
    } else if str_len as usize > SSO_MAX_LEN {
        // Source is a regular heap string — sub-slices reference str_ptr
        (true, 0_usize)
    } else {
        // Source is SSO — no slicing possible (all pieces fit in SSO anyway)
        (false, 0_usize)
    };

    let total = n * elem_size;
    let new_data = crate::rc::ori_rc_alloc(total, std::mem::align_of::<OriStr>());
    let elements = new_data.cast::<OriStr>();

    for (i, &(part_start, part_end)) in parts.iter().enumerate() {
        let part_len = part_end - part_start;
        let element = if can_slice && part_len > SSO_MAX_LEN {
            // Heap/slice source, long piece: create seamless slice.
            // Use slice-aware RC inc — handles both heap and slice sources
            // by computing the original allocation base when needed.
            crate::rc::ori_str_rc_inc(str_ptr as *mut u8, str_cap);
            OriStr {
                heap: OriStrHeap {
                    len: part_len as i64,
                    cap: make_slice_cap(base_offset + part_start),
                    // SAFETY: str_ptr is valid; part_start is within the source string bounds.
                    data: unsafe { (str_ptr as *mut u8).add(part_start) },
                },
            }
        } else {
            // SSO source or short piece: copy bytes
            OriStr::from_bytes(&bytes[part_start..part_end])
        };
        // SAFETY: elements points to freshly allocated memory for n OriStr values; i < n.
        unsafe { elements.add(i).write(element) };
    }

    // SAFETY: new_data was returned by ori_rc_alloc — header offsets are valid.
    unsafe {
        crate::rc::store_elem_dec_fn(new_data, elem_dec_fn);
        crate::rc::store_elem_count(new_data, n as i64);
        out_ptr.cast::<i64>().write(n as i64);
        out_ptr.cast::<i64>().add(1).write(n as i64);
        out_ptr.add(16).cast::<*mut u8>().write(new_data);
    }
}

/// Concatenate two strings with COW (Copy-on-Write) optimization.
///
/// Four cases, from fastest to slowest:
/// 1. Both SSO and combined <= 23 bytes -> SSO result (no allocation)
/// 2. `a` is heap, unique, has capacity -> append `b` in place (O(m))
/// 3. `a` is heap, unique, no capacity -> realloc + append (O(n+m) amortized)
/// 4. `a` is shared or SSO (promotion needed) -> allocate new (O(n+m))
///
/// The caller is responsible for RC management of the result.
#[no_mangle]
pub extern "C" fn ori_str_concat(a: *const OriStr, b: *const OriStr) -> OriStr {
    let a_ref = if a.is_null() {
        &OriStr::EMPTY
    } else {
        // SAFETY: a validated non-null; pointer from LLVM codegen is valid per runtime protocol.
        unsafe { &*a }
    };
    let b_ref = if b.is_null() {
        &OriStr::EMPTY
    } else {
        // SAFETY: b validated non-null; pointer from LLVM codegen is valid per runtime protocol.
        unsafe { &*b }
    };

    let a_bytes = a_ref.as_bytes();
    let b_bytes = b_ref.as_bytes();
    let a_len = a_bytes.len();
    let b_len = b_bytes.len();

    // Short-circuit: empty operands — must return owned copy, not alias
    if b_len == 0 {
        return OriStr::from_bytes(a_bytes);
    }
    if a_len == 0 {
        return OriStr::from_bytes(b_bytes);
    }

    let combined = a_len + b_len;

    // Case 1: Both fit in SSO
    if combined <= SSO_MAX_LEN {
        let mut sso = OriStrSSO {
            bytes: [0u8; 23],
            flags: SSO_FLAG | combined as u8,
        };
        sso.bytes[..a_len].copy_from_slice(a_bytes);
        sso.bytes[a_len..combined].copy_from_slice(b_bytes);
        return OriStr { sso };
    }

    // Cases 2-3: `a` is heap and uniquely owned -- in-place append.
    //
    // The caller borrows `a` by pointer and will RC-dec it after the call.
    // Case 2 (in-place append): We reuse `a`'s data pointer in the result,
    //   so both old and new values share it. RC-inc to keep the allocation
    //   alive after the caller's dec.
    // Case 3 (realloc): `realloc` may free the old allocation, making the
    //   caller's old pointer dangling. We CANNOT use realloc here -- instead
    //   fall through to Case 4 (fresh allocation) so the caller's dec
    //   cleanly frees the old allocation.
    if !a_ref.is_sso() {
        // SAFETY: is_sso() returned false, so the heap variant is active.
        let heap = unsafe { &a_ref.heap };
        if !heap.data.is_null()
            && !is_slice_cap(heap.cap)
            && ori_rc_is_unique(heap.data)
            && (heap.cap as usize) >= combined
        {
            // Case 2: has capacity -- append in place, inc RC for caller's dec
            // SAFETY: heap.data is uniquely owned with cap >= combined; writing
            // b_len bytes at offset a_len is within allocated capacity.
            unsafe {
                std::ptr::copy_nonoverlapping(b_bytes.as_ptr(), heap.data.add(a_len), b_len);
            }
            ori_rc_inc(heap.data);
            return OriStr {
                heap: OriStrHeap {
                    len: combined as i64,
                    cap: heap.cap,
                    data: heap.data,
                },
            };
        }
    }

    // Case 4: allocate new (a is shared or SSO requiring promotion)
    // Use combined length (not a's capacity) as the growth base. Using a_cap
    // caused exponential blowup in loops like `s = s + "-" + "x"` where each
    // iteration doubled a_cap twice (once per concat), reaching terabyte
    // capacities after ~40 iterations and crashing allocators with limited
    // virtual memory (e.g., CI runners).
    let new_cap = next_capacity(combined, combined);
    let new_data = ori_rc_alloc(new_cap, 1);
    if new_data.is_null() {
        crate::ori_panic_cstr(c"out of memory in string concatenation".as_ptr());
    }
    // SAFETY: new_data is freshly allocated with new_cap >= combined bytes and validated
    // non-null above; a_len + b_len == combined, so both writes are within bounds.
    unsafe {
        std::ptr::copy_nonoverlapping(a_bytes.as_ptr(), new_data, a_len);
        std::ptr::copy_nonoverlapping(b_bytes.as_ptr(), new_data.add(a_len), b_len);
    }
    OriStr {
        heap: OriStrHeap {
            len: combined as i64,
            cap: new_cap as i64,
            data: new_data,
        },
    }
}

/// Compare two strings for equality.
#[no_mangle]
pub extern "C" fn ori_str_eq(a: *const OriStr, b: *const OriStr) -> bool {
    let a_str = if a.is_null() {
        ""
    } else {
        // SAFETY: a validated non-null; OriStr bytes are valid UTF-8 by construction.
        unsafe { (*a).as_str() }
    };
    let b_str = if b.is_null() {
        ""
    } else {
        // SAFETY: b validated non-null; OriStr bytes are valid UTF-8 by construction.
        unsafe { (*b).as_str() }
    };

    a_str == b_str
}

/// Compare two strings for inequality.
#[no_mangle]
pub extern "C" fn ori_str_ne(a: *const OriStr, b: *const OriStr) -> bool {
    !ori_str_eq(a, b)
}

/// Compare two strings lexicographically.
///
/// Returns Ordering tag: 0 (Less), 1 (Equal), 2 (Greater).
#[no_mangle]
pub extern "C" fn ori_str_compare(a: *const OriStr, b: *const OriStr) -> i8 {
    let a_str = if a.is_null() {
        ""
    } else {
        // SAFETY: a validated non-null; OriStr bytes are valid UTF-8 by construction.
        unsafe { (*a).as_str() }
    };
    let b_str = if b.is_null() {
        ""
    } else {
        // SAFETY: b validated non-null; OriStr bytes are valid UTF-8 by construction.
        unsafe { (*b).as_str() }
    };

    match a_str.cmp(b_str) {
        core::cmp::Ordering::Less => 0,
        core::cmp::Ordering::Equal => 1,
        core::cmp::Ordering::Greater => 2,
    }
}

/// Hash a string using FNV-1a (64-bit).
///
/// Returns a deterministic i64 hash of the string's bytes. Uses the same
/// FNV-1a constants as the LLVM derive codegen for struct hashing, so
/// hash values compose correctly via `hash_combine`.
#[no_mangle]
pub extern "C" fn ori_str_hash(s: *const OriStr) -> i64 {
    const FNV_OFFSET_BASIS: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;

    let bytes: &[u8] = if s.is_null() {
        &[]
    } else {
        // SAFETY: s validated non-null; pointer from LLVM codegen is valid per runtime protocol.
        unsafe { (*s).as_bytes() }
    };

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash as i64
}

/// Return the byte length of a string (SSO-safe).
///
/// For SSO strings, the length is encoded in the flags byte (byte 23, bits 0-6).
/// For heap strings, the length is the first i64 field. Codegen must call this
/// instead of extracting field 0 directly, since the union layout differs.
#[no_mangle]
pub extern "C" fn ori_str_len(s: *const OriStr) -> i64 {
    if s.is_null() {
        return 0;
    }
    // SAFETY: s validated non-null above; pointer from LLVM codegen is valid per runtime protocol.
    unsafe { &*s }.len() as i64
}

/// Return a pointer to the string's raw byte data (SSO-safe).
///
/// For SSO strings, returns a pointer to the inline bytes (at the start of the
/// struct). For heap strings, returns the data pointer (field 2). The returned
/// pointer is valid as long as the `OriStr` it came from is alive and unmodified.
///
/// **Lifetime warning**: For SSO strings, the pointer points into the `OriStr`
/// struct itself. If the `OriStr` is on the stack (an alloca), the pointer is only
/// valid while that stack frame is live. Do NOT store this pointer in a long-lived
/// structure -- use `ori_iter_from_str` for iterators instead.
#[no_mangle]
pub extern "C" fn ori_str_data(s: *const OriStr) -> *const u8 {
    if s.is_null() {
        return std::ptr::null();
    }
    // SAFETY: s validated non-null above; pointer from LLVM codegen is valid per runtime protocol.
    let s = unsafe { &*s };
    if s.is_sso() {
        // SAFETY: is_sso() returned true, so the sso variant is active.
        unsafe { s.sso.bytes.as_ptr() }
    } else {
        // SAFETY: is_sso() returned false, so the heap variant is active.
        unsafe { s.heap.data }
    }
}
