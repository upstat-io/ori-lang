//! String transformation and predicate methods.
//!
//! Contains `substring`, `contains`, `starts_with`, `ends_with`, `trim`,
//! `to_uppercase`, `to_lowercase`, `replace`, `repeat`, `push_char`,
//! and `next_char`.

use crate::next_capacity;
use crate::rc::{ori_rc_alloc, ori_rc_inc, ori_rc_is_unique, ori_rc_realloc};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset};

use super::{deref_str, OriCharResult, OriStr, OriStrHeap, OriStrSSO, SSO_FLAG, SSO_MAX_LEN};

/// Create a substring as a seamless slice (zero-copy for heap strings).
///
/// For **SSO strings**: copies the bytes into a new SSO or heap string
/// (SSO strings have no heap buffer to share — they're inline).
///
/// For **heap strings**: creates a seamless slice view. The slice shares
/// the original buffer's data via RC increment. No bytes are copied.
///
/// # Parameters
/// - `s`: pointer to the source string
/// - `start`: byte offset of the first byte (inclusive)
/// - `end`: byte offset past the last byte (exclusive)
///
/// # Preconditions
/// - `start` and `end` should be valid UTF-8 boundaries
/// - `0 <= start <= end <= len(s)`
#[no_mangle]
pub extern "C" fn ori_str_substring(s: *const OriStr, start: i64, end: i64) -> OriStr {
    if s.is_null() {
        return OriStr::EMPTY;
    }
    // SAFETY: s validated non-null above; pointer from LLVM codegen is valid per runtime protocol.
    let s_ref = unsafe { &*s };
    let s_len = s_ref.len() as i64;

    // Clamp and validate range
    let start = start.max(0).min(s_len);
    let end = end.max(start).min(s_len);
    let result_len = (end - start) as usize;

    if result_len == 0 {
        return OriStr::EMPTY;
    }

    // SSO path: can't slice inline storage — copy bytes
    if s_ref.is_sso() {
        let bytes = s_ref.as_bytes();
        return OriStr::from_bytes(&bytes[start as usize..end as usize]);
    }

    // Heap path: create a seamless slice
    // SAFETY: is_sso() returned false, so the heap variant is active.
    let heap = unsafe { &s_ref.heap };
    if heap.data.is_null() {
        return OriStr::EMPTY;
    }

    // If result fits in SSO, copy is cheaper than RC management
    if result_len <= SSO_MAX_LEN {
        // SAFETY: heap.data is non-null (checked above); start..start+result_len is within
        // the allocated buffer (clamped to s_len).
        let bytes =
            unsafe { std::slice::from_raw_parts(heap.data.add(start as usize), result_len) };
        return OriStr::from_sso(bytes);
    }

    // Compute the original allocation's data pointer and total byte offset.
    let (original_data, total_byte_offset) = if is_slice_cap(heap.cap) {
        let existing_offset = slice_byte_offset(heap.cap);
        // SAFETY: heap.data is an interior pointer into the original allocation;
        // subtracting the slice byte offset recovers the original base pointer.
        let orig = unsafe { heap.data.sub(existing_offset) };
        (orig, existing_offset + start as usize)
    } else {
        (heap.data, start as usize)
    };

    // Increment RC on the original buffer (new slice reference)
    ori_rc_inc(original_data);

    // SAFETY: total_byte_offset is within the original allocation's bounds
    // (clamped start + any existing slice offset).
    let slice_data = unsafe { original_data.add(total_byte_offset) };

    OriStr {
        heap: OriStrHeap {
            len: result_len as i64,
            cap: make_slice_cap(total_byte_offset),
            data: slice_data,
        },
    }
}

/// Check if a string contains a substring.
#[no_mangle]
pub extern "C" fn ori_str_contains(s: *const OriStr, needle: *const OriStr) -> bool {
    // SAFETY: deref_str handles null (returns ""); non-null pointers valid per C-ABI protocol.
    let (s, needle) = unsafe { (deref_str(s), deref_str(needle)) };
    s.contains(needle)
}

/// Check if a string starts with a prefix.
#[no_mangle]
pub extern "C" fn ori_str_starts_with(s: *const OriStr, prefix: *const OriStr) -> bool {
    // SAFETY: deref_str handles null (returns ""); non-null pointers valid per C-ABI protocol.
    let (s, prefix) = unsafe { (deref_str(s), deref_str(prefix)) };
    s.starts_with(prefix)
}

/// Check if a string ends with a suffix.
#[no_mangle]
pub extern "C" fn ori_str_ends_with(s: *const OriStr, suffix: *const OriStr) -> bool {
    // SAFETY: deref_str handles null (returns ""); non-null pointers valid per C-ABI protocol.
    let (s, suffix) = unsafe { (deref_str(s), deref_str(suffix)) };
    s.ends_with(suffix)
}

/// Trim whitespace from both ends of a string.
///
/// For heap strings, returns a seamless slice (zero-copy) of the trimmed
/// region. For SSO strings, returns a new SSO copy of the trimmed bytes.
/// Uses `ori_str_substring` for the actual slice creation.
#[no_mangle]
pub extern "C" fn ori_str_trim(s: *const OriStr) -> OriStr {
    if s.is_null() {
        return OriStr::EMPTY;
    }
    // SAFETY: s validated non-null above; pointer from LLVM codegen is valid per runtime protocol.
    let s_ref = unsafe { &*s };
    let bytes = s_ref.as_bytes();
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .map_or(start, |i| i + 1);
    ori_str_substring(s, start as i64, end as i64)
}

/// Convert a string to uppercase.
///
/// COW optimization: if the string is ASCII-only and uniquely owned on the
/// heap, the transformation is done in place (ASCII case change preserves
/// byte length). For SSO strings, the bytes are copied and transformed
/// inline. Non-ASCII strings fall through to Rust's `to_uppercase()`.
#[no_mangle]
pub extern "C" fn ori_str_to_uppercase(s: *const OriStr) -> OriStr {
    let s_ref = if s.is_null() {
        return OriStr::EMPTY;
    } else {
        // SAFETY: s validated non-null by the if-branch above.
        unsafe { &*s }
    };
    let bytes = s_ref.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return OriStr::EMPTY;
    }

    // Non-ASCII: fall through to Rust (length may change, e.g. ss -> SS)
    if !bytes.is_ascii() {
        // SAFETY: OriStr bytes are valid UTF-8 by construction.
        return OriStr::from_owned(&unsafe { s_ref.as_str() }.to_uppercase());
    }

    // ASCII: byte length is preserved -- can do in-place or SSO
    if s_ref.is_sso() {
        // SAFETY: is_sso() returned true, so the sso variant is active.
        let mut sso = unsafe { s_ref.sso };
        for b in &mut sso.bytes[..len] {
            b.make_ascii_uppercase();
        }
        return OriStr { sso };
    }

    // Heap: try in-place if unique (not a slice — slices share parent buffer)
    // SAFETY: is_sso() returned false, so the heap variant is active.
    let heap = unsafe { &s_ref.heap };
    if !heap.data.is_null() && !is_slice_cap(heap.cap) && ori_rc_is_unique(heap.data) {
        // SAFETY: heap.data is non-null, uniquely owned, and allocated with at least `len` bytes.
        let data = unsafe { std::slice::from_raw_parts_mut(heap.data, len) };
        data.make_ascii_uppercase();
        return *s_ref;
    }

    // Shared heap or slice: allocate new
    let mut result = Vec::with_capacity(len);
    result.extend(bytes.iter().map(u8::to_ascii_uppercase));
    OriStr::from_bytes(&result)
}

/// Convert a string to lowercase.
///
/// Same COW strategy as `ori_str_to_uppercase`.
#[no_mangle]
pub extern "C" fn ori_str_to_lowercase(s: *const OriStr) -> OriStr {
    let s_ref = if s.is_null() {
        return OriStr::EMPTY;
    } else {
        // SAFETY: s validated non-null by the if-branch above.
        unsafe { &*s }
    };
    let bytes = s_ref.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return OriStr::EMPTY;
    }

    if !bytes.is_ascii() {
        // SAFETY: OriStr bytes are valid UTF-8 by construction.
        return OriStr::from_owned(&unsafe { s_ref.as_str() }.to_lowercase());
    }

    if s_ref.is_sso() {
        // SAFETY: is_sso() returned true, so the sso variant is active.
        let mut sso = unsafe { s_ref.sso };
        for b in &mut sso.bytes[..len] {
            b.make_ascii_lowercase();
        }
        return OriStr { sso };
    }

    // SAFETY: is_sso() returned false, so the heap variant is active.
    let heap = unsafe { &s_ref.heap };
    if !heap.data.is_null() && !is_slice_cap(heap.cap) && ori_rc_is_unique(heap.data) {
        // SAFETY: heap.data is non-null, uniquely owned, and allocated with at least `len` bytes.
        let data = unsafe { std::slice::from_raw_parts_mut(heap.data, len) };
        data.make_ascii_lowercase();
        return *s_ref;
    }

    let mut result = Vec::with_capacity(len);
    result.extend(bytes.iter().map(u8::to_ascii_lowercase));
    OriStr::from_bytes(&result)
}

/// Replace all occurrences of `from` with `to` in a string.
///
/// COW optimization: if `from` and `to` have the same byte length and the
/// source is a uniquely-owned heap string, replacements are done in place.
#[no_mangle]
pub extern "C" fn ori_str_replace(
    s: *const OriStr,
    from: *const OriStr,
    to: *const OriStr,
) -> OriStr {
    let s_ref = if s.is_null() {
        return OriStr::EMPTY;
    } else {
        // SAFETY: s validated non-null by the if-branch above.
        unsafe { &*s }
    };
    // SAFETY: deref_str handles null (returns ""); non-null pointers valid per C-ABI protocol.
    let from_str = unsafe { deref_str(from) };
    // SAFETY: deref_str handles null (returns ""); non-null pointers valid per C-ABI protocol.
    let to_str = unsafe { deref_str(to) };

    // Same-length replacement on unique heap -> in-place (not slices)
    if from_str.len() == to_str.len() && !from_str.is_empty() && !s_ref.is_sso() {
        // SAFETY: is_sso() returned false, so the heap variant is active.
        let heap = unsafe { &s_ref.heap };
        if !heap.data.is_null() && !is_slice_cap(heap.cap) && ori_rc_is_unique(heap.data) {
            let len = s_ref.len();
            // SAFETY: heap.data is non-null, uniquely owned, and allocated with at least `len` bytes.
            let data = unsafe { std::slice::from_raw_parts_mut(heap.data, len) };
            let from_bytes = from_str.as_bytes();
            let to_bytes = to_str.as_bytes();
            let pat_len = from_bytes.len();
            let mut i = 0;
            while i + pat_len <= len {
                if &data[i..i + pat_len] == from_bytes {
                    data[i..i + pat_len].copy_from_slice(to_bytes);
                    i += pat_len;
                } else {
                    i += 1;
                }
            }
            return *s_ref;
        }
    }

    // General case: use Rust's replace and wrap result
    // SAFETY: OriStr bytes are valid UTF-8 by construction.
    let s_str = unsafe { s_ref.as_str() };
    OriStr::from_bytes(s_str.replace(from_str, to_str).as_bytes())
}

/// Repeat a string `count` times.
///
/// Always allocates a new buffer with exact capacity (no COW -- the result
/// is always a new string).
#[no_mangle]
pub extern "C" fn ori_str_repeat(s: *const OriStr, count: i64) -> OriStr {
    let s_ref = if s.is_null() {
        return OriStr::EMPTY;
    } else {
        // SAFETY: s validated non-null by the if-branch above.
        unsafe { &*s }
    };
    let bytes = s_ref.as_bytes();
    let len = bytes.len();
    let n = count.max(0) as usize;

    if n == 0 || len == 0 {
        return OriStr::EMPTY;
    }
    if n == 1 {
        return OriStr::from_bytes(bytes);
    }

    let total = len.saturating_mul(n);

    // Fits in SSO
    if total <= SSO_MAX_LEN {
        let mut sso = OriStrSSO {
            bytes: [0u8; 23],
            flags: SSO_FLAG | total as u8,
        };
        for i in 0..n {
            sso.bytes[i * len..(i + 1) * len].copy_from_slice(bytes);
        }
        return OriStr { sso };
    }

    // Heap: allocate exact capacity
    let data = ori_rc_alloc(total, 1);
    for i in 0..n {
        // SAFETY: data is freshly allocated with `total` bytes; each write at
        // offset i*len is within bounds since i*len + len <= total.
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), data.add(i * len), len);
        }
    }
    OriStr {
        heap: OriStrHeap {
            len: total as i64,
            cap: total as i64,
            data,
        },
    }
}

/// Append a single character to a string.
///
/// Follows the same COW protocol as `ori_str_concat`:
/// 1. If result fits in SSO -> SSO (no allocation)
/// 2. If heap, unique, has capacity -> append in place
/// 3. If heap, unique, no capacity -> realloc
/// 4. Otherwise -> allocate new
#[no_mangle]
pub extern "C" fn ori_str_push_char(s: *const OriStr, ch: u32) -> OriStr {
    let s_ref = if s.is_null() {
        &OriStr::EMPTY
    } else {
        // SAFETY: s validated non-null by the if-branch above.
        unsafe { &*s }
    };

    // Encode the char to UTF-8
    let c = char::from_u32(ch).unwrap_or(char::REPLACEMENT_CHARACTER);
    let mut buf = [0u8; 4];
    let encoded = c.encode_utf8(&mut buf);
    let ch_bytes = encoded.as_bytes();
    let ch_len = ch_bytes.len();

    let s_bytes = s_ref.as_bytes();
    let s_len = s_bytes.len();
    let combined = s_len + ch_len;

    // Case 1: fits in SSO
    if combined <= SSO_MAX_LEN {
        let mut sso = OriStrSSO {
            bytes: [0u8; 23],
            flags: SSO_FLAG | combined as u8,
        };
        sso.bytes[..s_len].copy_from_slice(s_bytes);
        sso.bytes[s_len..combined].copy_from_slice(ch_bytes);
        return OriStr { sso };
    }

    // Cases 2-3: heap and uniquely owned (not slices — can't mutate parent)
    if !s_ref.is_sso() {
        // SAFETY: is_sso() returned false, so the heap variant is active.
        let heap = unsafe { &s_ref.heap };
        if !heap.data.is_null() && !is_slice_cap(heap.cap) && ori_rc_is_unique(heap.data) {
            if (heap.cap as usize) >= combined {
                // Case 2: has capacity
                // SAFETY: heap.data is uniquely owned with cap >= combined; writing
                // ch_len bytes at offset s_len is within the allocated capacity.
                unsafe {
                    std::ptr::copy_nonoverlapping(ch_bytes.as_ptr(), heap.data.add(s_len), ch_len);
                }
                return OriStr {
                    heap: OriStrHeap {
                        len: combined as i64,
                        cap: heap.cap,
                        data: heap.data,
                    },
                };
            }
            // Case 3: realloc
            let new_cap = next_capacity(heap.cap as usize, combined);
            let new_data = ori_rc_realloc(heap.data, s_len, new_cap, 1);
            // SAFETY: new_data is freshly reallocated with new_cap >= combined bytes;
            // writing ch_len bytes at offset s_len is within capacity.
            unsafe {
                std::ptr::copy_nonoverlapping(ch_bytes.as_ptr(), new_data.add(s_len), ch_len);
            }
            return OriStr {
                heap: OriStrHeap {
                    len: combined as i64,
                    cap: new_cap as i64,
                    data: new_data,
                },
            };
        }
    }

    // Case 4: allocate new
    let new_cap = next_capacity(0, combined);
    let new_data = ori_rc_alloc(new_cap, 1);
    // SAFETY: new_data is freshly allocated with new_cap >= combined bytes;
    // s_len + ch_len == combined, so both writes are within bounds.
    unsafe {
        std::ptr::copy_nonoverlapping(s_bytes.as_ptr(), new_data, s_len);
        std::ptr::copy_nonoverlapping(ch_bytes.as_ptr(), new_data.add(s_len), ch_len);
    }
    OriStr {
        heap: OriStrHeap {
            len: combined as i64,
            cap: new_cap as i64,
            data: new_data,
        },
    }
}

/// Decode the next UTF-8 character from a string at the given byte offset.
///
/// Returns the Unicode codepoint and the byte offset of the next character.
/// If `byte_offset >= len` or the byte sequence is invalid, returns
/// codepoint = -1 and `next_offset = len` (termination sentinel).
///
/// # Parameters
/// - `data`: Pointer to the string's UTF-8 byte data
/// - `len`: Total byte length of the string
/// - `byte_offset`: Current byte position to decode from
#[no_mangle]
pub extern "C" fn ori_str_next_char(data: *const u8, len: i64, byte_offset: i64) -> OriCharResult {
    if data.is_null() || byte_offset < 0 || byte_offset >= len {
        return OriCharResult {
            codepoint: -1,
            next_offset: len,
        };
    }

    let offset = byte_offset as usize;
    let length = len as usize;
    let remaining = length - offset;

    // SAFETY: data is valid for `len` bytes, offset < len
    let lead = unsafe { *data.add(offset) };

    // Decode UTF-8 leading byte to determine character width
    let (codepoint, width) = if lead < 0x80 {
        // 1-byte: 0xxxxxxx (ASCII)
        (i32::from(lead), 1)
    } else if lead < 0xC0 {
        // Continuation byte in leading position -- invalid
        return OriCharResult {
            codepoint: -1,
            next_offset: byte_offset + 1,
        };
    } else if lead < 0xE0 && remaining >= 2 {
        // 2-byte: 110xxxxx 10xxxxxx
        // SAFETY: remaining >= 2 guarantees offset+1 < len; data valid for len bytes.
        let b1 = unsafe { *data.add(offset + 1) };
        let cp = (i32::from(lead & 0x1F) << 6) | i32::from(b1 & 0x3F);
        (cp, 2)
    } else if lead < 0xF0 && remaining >= 3 {
        // 3-byte: 1110xxxx 10xxxxxx 10xxxxxx
        // SAFETY: remaining >= 3 guarantees offset+1..offset+2 < len; data valid for len bytes.
        let b1 = unsafe { *data.add(offset + 1) };
        let b2 = unsafe { *data.add(offset + 2) };
        let cp =
            (i32::from(lead & 0x0F) << 12) | (i32::from(b1 & 0x3F) << 6) | i32::from(b2 & 0x3F);
        (cp, 3)
    } else if lead < 0xF8 && remaining >= 4 {
        // 4-byte: 11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
        // SAFETY: remaining >= 4 guarantees offset+1..offset+3 < len; data valid for len bytes.
        let b1 = unsafe { *data.add(offset + 1) };
        let b2 = unsafe { *data.add(offset + 2) };
        let b3 = unsafe { *data.add(offset + 3) };
        let cp = (i32::from(lead & 0x07) << 18)
            | (i32::from(b1 & 0x3F) << 12)
            | (i32::from(b2 & 0x3F) << 6)
            | i32::from(b3 & 0x3F);
        (cp, 4)
    } else {
        // Incomplete multi-byte sequence or invalid lead byte
        return OriCharResult {
            codepoint: -1,
            next_offset: byte_offset + 1,
        };
    };

    OriCharResult {
        codepoint,
        next_offset: byte_offset + width,
    }
}

#[cfg(test)]
mod tests;
