//! Tests for string slice operations (substring, trim, split).

use crate::rc::{ori_rc_count, ori_rc_dec, ori_rc_free, ori_rc_inc};
use crate::slice_encoding::{is_slice_cap, make_slice_cap, slice_byte_offset, slice_original_data};
use crate::string::OriStr;

use super::ori_str_substring;
use super::ori_str_trim;

// ── ori_str_substring ─────────────────────────────────────────────────

#[test]
fn substring_of_heap_string_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string > 23 bytes
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // Substring that's also > 23 bytes → must produce a slice
    let sub = ori_str_substring(&source, 4, 34); // "quick brown fox jumps over the"
    assert!(
        !sub.is_sso(),
        "long substring of heap string should be heap"
    );

    let sub_heap = unsafe { &sub.heap };
    assert!(
        is_slice_cap(sub_heap.cap),
        "substring should produce a seamless slice"
    );
    assert_eq!(sub_heap.len, 30); // "quick brown fox jumps over the" = 30 bytes
    assert_eq!(slice_byte_offset(sub_heap.cap), 4); // offset from original data

    // Data pointer should point into the original buffer
    assert_eq!(sub_heap.data, unsafe { heap_data.add(4) });

    // RC should be 2 (original + slice)
    assert_eq!(ori_rc_count(heap_data), 2);

    // Content should be correct
    let sub_bytes = sub.as_bytes();
    assert_eq!(sub_bytes, b"quick brown fox jumps over the");

    // Clean up: dec slice reference, then free original
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_of_heap_short_result_uses_sso() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string > 23 bytes
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // Take a short substring (<= 23 bytes) → should use SSO, not slice
    let sub = ori_str_substring(&source, 4, 9); // "quick" = 5 bytes
    assert!(sub.is_sso(), "short substring should use SSO, not slice");
    assert_eq!(sub.as_bytes(), b"quick");

    // RC unchanged: SSO doesn't reference the original buffer
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_of_sso_string_copies() {
    // Create an SSO string (<= 23 bytes)
    let source = OriStr::from_sso(b"hello world");
    assert!(source.is_sso());

    let sub = ori_str_substring(&source, 6, 11); // "world"
    assert!(sub.is_sso(), "substring of SSO should produce SSO");
    assert_eq!(sub.as_bytes(), b"world");
}

#[test]
fn substring_empty_range_returns_empty() {
    let _g = crate::test_helpers::lock_rc();
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    // start == end → empty
    let sub = ori_str_substring(&source, 5, 5);
    assert!(sub.is_empty());
    assert_eq!(sub.len(), 0);

    // RC unchanged
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_full_range_of_heap_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    let source = OriStr::from_heap(b"The quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    // Full range → slice with offset 0
    let sub = ori_str_substring(&source, 0, 43);
    assert!(!sub.is_sso());

    let sub_heap = unsafe { &sub.heap };
    assert!(is_slice_cap(sub_heap.cap));
    assert_eq!(slice_byte_offset(sub_heap.cap), 0);
    assert_eq!(sub_heap.len, 43);
    assert_eq!(sub_heap.data, heap_data);
    assert_eq!(ori_rc_count(heap_data), 2);

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

#[test]
fn substring_clamping() {
    let source = OriStr::from_sso(b"hello");

    // Negative start clamped to 0
    let sub = ori_str_substring(&source, -5, 3);
    assert_eq!(sub.as_bytes(), b"hel");

    // End beyond length clamped to len
    let sub = ori_str_substring(&source, 2, 100);
    assert_eq!(sub.as_bytes(), b"llo");

    // Start beyond length → empty
    let sub = ori_str_substring(&source, 100, 200);
    assert!(sub.is_empty());
}

#[test]
fn substring_null_returns_empty() {
    let sub = ori_str_substring(std::ptr::null(), 0, 5);
    assert!(sub.is_empty());
}

#[test]
fn substring_of_slice_accumulates_offsets() {
    let _g = crate::test_helpers::lock_rc();
    // Create a heap string and take a slice, then slice the slice
    let source = OriStr::from_heap(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ-extra-padding-data");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // First slice: offset 4, length 38 → "EFGHIJKLMNOPQRSTUVWXYZ-extra-padding-d"
    let slice1 = ori_str_substring(&source, 4, 42);
    assert_eq!(ori_rc_count(heap_data), 2);
    let s1_heap = unsafe { &slice1.heap };
    assert!(is_slice_cap(s1_heap.cap));
    assert_eq!(slice_byte_offset(s1_heap.cap), 4);

    // Second slice of first slice: offset 2 within slice1 → total offset 6
    // Take 28 bytes: "GHIJKLMNOPQRSTUVWXYZ-extra-p"
    let slice2 = ori_str_substring(&slice1, 2, 30);
    assert_eq!(ori_rc_count(heap_data), 3);
    let s2_heap = unsafe { &slice2.heap };
    assert!(is_slice_cap(s2_heap.cap));
    assert_eq!(slice_byte_offset(s2_heap.cap), 6); // 4 + 2

    // Data should point into original buffer
    assert_eq!(s2_heap.data, unsafe { heap_data.add(6) });
    assert_eq!(slice_original_data(s2_heap.data, s2_heap.cap), heap_data);

    // Content check
    assert_eq!(slice2.as_bytes(), b"GHIJKLMNOPQRSTUVWXYZ-extra-p");

    // Clean up
    ori_rc_dec(heap_data, None); // slice1
    ori_rc_dec(heap_data, None); // slice2
    ori_rc_free(heap_data, 45, 1);
}

// ── ori_str_trim ──────────────────────────────────────────────────────

#[test]
fn trim_heap_string_produces_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with leading/trailing whitespace, trimmed result > 23 bytes
    let source = OriStr::from_heap(b"   The quick brown fox jumps over the lazy dog   ");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let trimmed = ori_str_trim(&source);
    assert!(!trimmed.is_sso());

    let t_heap = unsafe { &trimmed.heap };
    assert!(
        is_slice_cap(t_heap.cap),
        "trim of heap string should produce a slice"
    );
    assert_eq!(t_heap.len, 43); // "The quick brown fox jumps over the lazy dog"
    assert_eq!(slice_byte_offset(t_heap.cap), 3); // 3 spaces skipped

    assert_eq!(ori_rc_count(heap_data), 2);
    assert_eq!(
        trimmed.as_bytes(),
        b"The quick brown fox jumps over the lazy dog"
    );

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 49, 1);
}

#[test]
fn trim_sso_string_copies() {
    let source = OriStr::from_sso(b"  hello  ");
    assert!(source.is_sso());

    let trimmed = ori_str_trim(&source);
    assert!(trimmed.is_sso(), "trim of SSO should produce SSO");
    assert_eq!(trimmed.as_bytes(), b"hello");
}

#[test]
fn trim_all_whitespace_returns_empty() {
    let source = OriStr::from_sso(b"   \t\n  ");
    let trimmed = ori_str_trim(&source);
    assert!(trimmed.is_empty());
}

#[test]
fn trim_no_whitespace_full_slice() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with no whitespace → full slice (offset 0)
    let source = OriStr::from_heap(b"The-quick-brown-fox-jumps-over");
    let heap_data = unsafe { source.heap.data };

    let trimmed = ori_str_trim(&source);
    assert!(!trimmed.is_sso());

    let t_heap = unsafe { &trimmed.heap };
    assert!(is_slice_cap(t_heap.cap));
    assert_eq!(slice_byte_offset(t_heap.cap), 0);
    assert_eq!(t_heap.len, 30);
    assert_eq!(ori_rc_count(heap_data), 2);

    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 30, 1);
}

#[test]
fn trim_null_returns_empty() {
    let trimmed = ori_str_trim(std::ptr::null());
    assert!(trimmed.is_empty());
}

#[test]
fn trim_heap_short_result_uses_sso() {
    let _g = crate::test_helpers::lock_rc();
    // Heap string with lots of whitespace, only "hi" remains (2 bytes → SSO)
    let padded = b"                        hi                        ";
    let source = OriStr::from_heap(padded);
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let trimmed = ori_str_trim(&source);
    assert!(
        trimmed.is_sso(),
        "short trim result should use SSO, not slice"
    );
    assert_eq!(trimmed.as_bytes(), b"hi");

    // RC unchanged: SSO doesn't reference original buffer
    assert_eq!(ori_rc_count(heap_data), 1);

    ori_rc_free(heap_data, padded.len(), 1);
}

// ── ori_str_split slices ──────────────────────────────────────────────

#[test]
fn split_heap_string_long_pieces_are_slices() {
    use crate::string::ori_str_split;
    let _g = crate::test_helpers::lock_rc();

    // Create a heap string with two long pieces separated by "|"
    // "ABCDEFGHIJKLMNOPQRSTUVWX|abcdefghijklmnopqrstuvwx" = 49 bytes
    let content = b"ABCDEFGHIJKLMNOPQRSTUVWX|abcdefghijklmnopqrstuvwx";
    let source = OriStr::from_heap(content);
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let sep = b"|";
    let mut out = [0u8; 24];

    // cap >= 0 for a regular heap string (not a slice)
    let heap_cap = unsafe { source.heap.cap };
    ori_str_split(
        heap_data,
        content.len() as i64,
        heap_cap,
        sep.as_ptr(),
        sep.len() as i64,
        None,
        out.as_mut_ptr(),
    );

    // Read list result
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let list_data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    assert_eq!(len, 2);

    // Read the two OriStr elements
    let elements = list_data.cast::<OriStr>();
    let elem0 = unsafe { elements.read() };
    let elem1 = unsafe { elements.add(1).read() };

    // Both pieces are 24 bytes > 23, so they should be slices
    assert!(
        !elem0.is_sso(),
        "first piece (24 bytes) should be heap/slice"
    );
    assert!(
        !elem1.is_sso(),
        "second piece (24 bytes) should be heap/slice"
    );

    let e0_heap = unsafe { &elem0.heap };
    let e1_heap = unsafe { &elem1.heap };

    assert!(is_slice_cap(e0_heap.cap), "first piece should be a slice");
    assert!(is_slice_cap(e1_heap.cap), "second piece should be a slice");

    assert_eq!(e0_heap.len, 24);
    assert_eq!(e1_heap.len, 24);

    // Verify offsets
    assert_eq!(slice_byte_offset(e0_heap.cap), 0); // starts at beginning
    assert_eq!(slice_byte_offset(e1_heap.cap), 25); // starts after "ABCDEFGHIJKLMNOPQRSTUVWX|"

    // Verify content
    assert_eq!(elem0.as_bytes(), b"ABCDEFGHIJKLMNOPQRSTUVWX");
    assert_eq!(elem1.as_bytes(), b"abcdefghijklmnopqrstuvwx");

    // RC: original + 2 slices = 3
    assert_eq!(ori_rc_count(heap_data), 3);

    // Clean up: dec both slice references, then free original and list
    ori_rc_dec(heap_data, None);
    ori_rc_dec(heap_data, None);
    crate::rc::ori_rc_free(
        list_data,
        2 * std::mem::size_of::<OriStr>(),
        std::mem::align_of::<OriStr>(),
    );
    ori_rc_free(heap_data, content.len(), 1);
}

#[test]
fn split_heap_string_short_pieces_use_sso() {
    use crate::string::ori_str_split;
    let _g = crate::test_helpers::lock_rc();

    // Heap string where pieces are all short (< 24 bytes)
    let content = b"hello world foo bar baz qux quux";
    let source = OriStr::from_heap(content);
    let heap_data = unsafe { source.heap.data };

    let sep = b" ";
    let mut out = [0u8; 24];

    let heap_cap = unsafe { source.heap.cap };
    ori_str_split(
        heap_data,
        content.len() as i64,
        heap_cap,
        sep.as_ptr(),
        sep.len() as i64,
        None,
        out.as_mut_ptr(),
    );

    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let list_data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    assert_eq!(len, 7);

    // All pieces are short → SSO, no slice
    let elements = list_data.cast::<OriStr>();
    for i in 0..7 {
        let elem = unsafe { elements.add(i).read() };
        assert!(elem.is_sso(), "short split piece {i} should be SSO");
    }

    // Check first and last
    assert_eq!(unsafe { elements.read() }.as_bytes(), b"hello");
    assert_eq!(unsafe { elements.add(6).read() }.as_bytes(), b"quux");

    // RC unchanged: SSO pieces don't reference original
    assert_eq!(ori_rc_count(heap_data), 1);

    crate::rc::ori_rc_free(
        list_data,
        7 * std::mem::size_of::<OriStr>(),
        std::mem::align_of::<OriStr>(),
    );
    ori_rc_free(heap_data, content.len(), 1);
}

/// Semantic pin for TPR-04-006: splitting a slice-backed string must not crash.
///
/// Creates a heap string, takes a substring (seamless slice), then splits
/// the substring. Without the `str_cap` parameter fix, `ori_str_split` would
/// call `ori_rc_inc` on an interior pointer → misaligned access → crash.
#[test]
fn split_slice_backed_string_no_crash() {
    use crate::string::ori_str_split;
    let _g = crate::test_helpers::lock_rc();

    // Create heap string: 60 bytes total, well over SSO_MAX_LEN
    let content = b"AAAAAAAAAAAAAAAAAAAAAAAA|BBBBBBBBBBBBBBBBBBBBBBBB|CCCCCCCCCCCC";
    let source = OriStr::from_heap(content);
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    // Simulate a substring: take bytes [25..49] = "BBBBBBBBBBBBBBBBBBBBBBBB"
    // This creates a seamless slice with data = heap_data + 25
    let slice_offset = 25_usize;
    let slice_len = 24_i64;
    let slice_data = unsafe { heap_data.add(slice_offset) };
    let slice_cap = make_slice_cap(slice_offset);

    // Manually RC-inc the original for the slice reference
    ori_rc_inc(heap_data);
    assert_eq!(ori_rc_count(heap_data), 2);

    // Now split the slice-backed string by "|" — no "|" in the slice range
    // The source is a slice, so ori_str_split receives SLICE_FLAG in str_cap.
    // The single piece (24 bytes > 23) should become a sub-slice of the ORIGINAL buffer.
    let sep = b"|";
    let mut out = [0u8; 24];

    ori_str_split(
        slice_data,
        slice_len,
        slice_cap,
        sep.as_ptr(),
        sep.len() as i64,
        None,
        out.as_mut_ptr(),
    );

    // Read list result
    let len = unsafe { out.as_ptr().cast::<i64>().read() };
    let list_data = unsafe { out.as_ptr().add(16).cast::<*mut u8>().read() };
    assert_eq!(len, 1, "no separator found in slice → 1 piece");

    // The single piece is 24 bytes > 23, so it should be a seamless slice
    let elements = list_data.cast::<OriStr>();
    let elem0 = unsafe { elements.read() };
    assert!(
        !elem0.is_sso(),
        "24-byte piece should be heap/slice, not SSO"
    );

    let e0_heap = unsafe { &elem0.heap };
    assert!(is_slice_cap(e0_heap.cap), "piece should be a slice");
    assert_eq!(e0_heap.len, 24);

    // The sub-slice's offset should be relative to the ORIGINAL buffer:
    // base_offset (25) + part_start (0) = 25
    assert_eq!(
        slice_byte_offset(e0_heap.cap),
        25,
        "sub-slice offset should be 25 (parent offset + part offset)"
    );

    // Verify content
    assert_eq!(elem0.as_bytes(), b"BBBBBBBBBBBBBBBBBBBBBBBB");

    // RC: original (1) + slice ref (1) + sub-slice ref (1) = 3
    assert_eq!(ori_rc_count(heap_data), 3);

    // Clean up: dec sub-slice ref, dec slice ref, free list, free original
    ori_rc_dec(heap_data, None); // sub-slice
    ori_rc_dec(heap_data, None); // slice
    crate::rc::ori_rc_free(
        list_data,
        std::mem::size_of::<OriStr>(),
        std::mem::align_of::<OriStr>(),
    );
    ori_rc_free(heap_data, content.len(), 1);
}

// TPR-04-010: Slice-backed strings through string methods

/// Semantic pin: `substring(...).to_uppercase()` must not crash on a
/// slice-backed string. Without the `SLICE_FLAG` guard, `ori_rc_is_unique`
/// dereferences an interior pointer → misaligned access → crash.
#[test]
fn to_uppercase_on_slice_backed_string() {
    use super::ori_str_to_uppercase;
    let _g = crate::test_helpers::lock_rc();

    // Create heap string > 23 bytes
    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    // Take a long substring → seamless slice (> 23 bytes to avoid SSO)
    let sub = ori_str_substring(&source, 4, 43); // "quick brown fox jumps over the lazy dog"
    assert!(!sub.is_sso());
    assert!(is_slice_cap(unsafe { sub.heap.cap }));
    assert_eq!(ori_rc_count(heap_data), 2);

    // This must not crash — the old code calls ori_rc_is_unique on interior pointer
    let upper = ori_str_to_uppercase(&sub);
    assert_eq!(upper.as_bytes(), b"QUICK BROWN FOX JUMPS OVER THE LAZY DOG");

    // upper should be a new allocation (slice is never unique for its parent)
    if !upper.is_sso() {
        let upper_data = unsafe { upper.heap.data };
        if !is_slice_cap(unsafe { upper.heap.cap }) {
            ori_rc_free(upper_data, upper.len(), 1);
        }
    }
    ori_rc_dec(heap_data, None); // sub's reference
    ori_rc_free(heap_data, 43, 1); // source
}

/// `to_lowercase` on a slice-backed string — same fix as `to_uppercase`.
#[test]
fn to_lowercase_on_slice_backed_string() {
    use super::ori_str_to_lowercase;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG");
    let heap_data = unsafe { source.heap.data };

    let sub = ori_str_substring(&source, 4, 43); // "QUICK BROWN FOX JUMPS OVER THE LAZY DOG"
    assert!(!sub.is_sso());
    assert!(is_slice_cap(unsafe { sub.heap.cap }));

    let lower = ori_str_to_lowercase(&sub);
    assert_eq!(lower.as_bytes(), b"quick brown fox jumps over the lazy dog");

    if !lower.is_sso() {
        let lower_data = unsafe { lower.heap.data };
        if !is_slice_cap(unsafe { lower.heap.cap }) {
            ori_rc_free(lower_data, lower.len(), 1);
        }
    }
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

/// `replace` on a slice-backed string — same-length replacement must not crash.
#[test]
fn replace_on_slice_backed_string() {
    use super::ori_str_replace;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    let sub = ori_str_substring(&source, 4, 43); // "quick brown fox jumps over the lazy dog"
    assert!(is_slice_cap(unsafe { sub.heap.cap }));

    let from = OriStr::from_sso(b"fox");
    let to = OriStr::from_sso(b"cat");
    let result = ori_str_replace(&sub, &from, &to);
    assert_eq!(
        result.as_bytes(),
        b"quick brown cat jumps over the lazy dog"
    );

    if !result.is_sso() {
        let result_data = unsafe { result.heap.data };
        if !is_slice_cap(unsafe { result.heap.cap }) {
            ori_rc_free(result_data, result.len(), 1);
        }
    }
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

/// `push_char` on a slice-backed string must not crash.
#[test]
fn push_char_on_slice_backed_string() {
    use super::ori_str_push_char;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    let sub = ori_str_substring(&source, 4, 43); // "quick brown fox jumps over the lazy dog"
    assert!(is_slice_cap(unsafe { sub.heap.cap }));

    let result = ori_str_push_char(&sub, '!' as u32);
    assert_eq!(
        result.as_bytes(),
        b"quick brown fox jumps over the lazy dog!"
    );

    if !result.is_sso() {
        let result_data = unsafe { result.heap.data };
        if !is_slice_cap(unsafe { result.heap.cap }) {
            ori_rc_free(result_data, result.len(), 1);
        }
    }
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

/// `concat` on a slice-backed string must not crash.
#[test]
fn concat_on_slice_backed_string() {
    use crate::string::ori_str_concat;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    let sub = ori_str_substring(&source, 4, 43); // "quick brown fox jumps over the lazy dog"
    assert!(is_slice_cap(unsafe { sub.heap.cap }));

    let suffix = OriStr::from_sso(b"!");
    let result = ori_str_concat(&sub, &suffix);
    assert_eq!(
        result.as_bytes(),
        b"quick brown fox jumps over the lazy dog!"
    );

    if !result.is_sso() {
        let result_data = unsafe { result.heap.data };
        if !is_slice_cap(unsafe { result.heap.cap }) {
            ori_rc_free(result_data, result.len(), 1);
        }
    }
    ori_rc_dec(heap_data, None);
    ori_rc_free(heap_data, 43, 1);
}

// TPR-04-011: repeat(1) double-free

/// Semantic pin: `repeat(count: 1)` must return an owned clone, not `*s_ref`.
/// Without the fix, both original and repeated value share the same heap.data
/// pointer, and RC-decrementing both causes a double-free.
#[test]
fn repeat_one_returns_owned_clone() {
    use super::ori_str_repeat;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let repeated = ori_str_repeat(&source, 1);
    assert_eq!(repeated.as_bytes(), source.as_bytes());

    // The repeated value MUST be a separate allocation (or SSO copy)
    // — not sharing heap.data with source
    if !repeated.is_sso() {
        let rep_data = unsafe { repeated.heap.data };
        assert_ne!(
            rep_data, heap_data,
            "repeat(1) must return a new allocation, not alias the original"
        );
        // Source RC should still be 1 (repeated has its own allocation)
        assert_eq!(ori_rc_count(heap_data), 1);
        // Clean up repeated
        ori_rc_free(rep_data, repeated.len(), 1);
    }

    ori_rc_free(heap_data, 43, 1);
}

/// repeat(1) on a slice-backed string must also produce an owned clone.
#[test]
fn repeat_one_on_slice_backed_string() {
    use super::ori_str_repeat;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };

    let sub = ori_str_substring(&source, 4, 43); // "quick brown fox jumps over the lazy dog"
    assert!(is_slice_cap(unsafe { sub.heap.cap }));
    assert_eq!(ori_rc_count(heap_data), 2);

    let repeated = ori_str_repeat(&sub, 1);
    assert_eq!(
        repeated.as_bytes(),
        b"quick brown fox jumps over the lazy dog"
    );

    // Must be a new allocation, not sharing parent buffer
    if !repeated.is_sso() {
        let rep_data = unsafe { repeated.heap.data };
        // Should not be an interior pointer into source
        assert!(
            !is_slice_cap(unsafe { repeated.heap.cap }),
            "repeat(1) on a slice should produce a fresh heap string, not another slice"
        );
        ori_rc_free(rep_data, repeated.len(), 1);
    }

    ori_rc_dec(heap_data, None); // sub
    ori_rc_free(heap_data, 43, 1); // source
}

// TPR-04-012: concat("") double-free

/// Semantic pin: `heap_str.concat("")` must return an owned copy, not alias.
#[test]
fn concat_empty_right_returns_owned_clone() {
    use crate::string::ori_str_concat;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let empty = OriStr::EMPTY;
    let result = ori_str_concat(&source, &empty);
    assert_eq!(result.as_bytes(), source.as_bytes());

    // Result must be a separate allocation
    if !result.is_sso() {
        let result_data = unsafe { result.heap.data };
        assert_ne!(
            result_data, heap_data,
            "concat('') must return a new allocation, not alias the original"
        );
        assert_eq!(ori_rc_count(heap_data), 1);
        ori_rc_free(result_data, result.len(), 1);
    }

    ori_rc_free(heap_data, 43, 1);
}

/// `"".concat(heap_str)` — empty left operand, same fix.
#[test]
fn concat_empty_left_returns_owned_clone() {
    use crate::string::ori_str_concat;
    let _g = crate::test_helpers::lock_rc();

    let source = OriStr::from_heap(b"the quick brown fox jumps over the lazy dog");
    let heap_data = unsafe { source.heap.data };
    assert_eq!(ori_rc_count(heap_data), 1);

    let empty = OriStr::EMPTY;
    let result = ori_str_concat(&empty, &source);
    assert_eq!(result.as_bytes(), source.as_bytes());

    if !result.is_sso() {
        let result_data = unsafe { result.heap.data };
        assert_ne!(
            result_data, heap_data,
            "concat with empty left must return a new allocation"
        );
        assert_eq!(ori_rc_count(heap_data), 1);
        ori_rc_free(result_data, result.len(), 1);
    }

    ori_rc_free(heap_data, 43, 1);
}
