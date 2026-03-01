//! Tests for seamless slice encoding primitives.

use super::*;

#[test]
fn slice_flag_is_sign_bit() {
    assert_eq!(SLICE_FLAG, i64::MIN);
    assert_eq!(SLICE_FLAG, -0x8000_0000_0000_0000_i64);
    const { assert!(SLICE_FLAG < 0) };
}

#[test]
fn is_slice_cap_regular() {
    // Regular capacities (non-negative) are not slices
    assert!(!is_slice_cap(0));
    assert!(!is_slice_cap(1));
    assert!(!is_slice_cap(100));
    assert!(!is_slice_cap(i64::MAX));
}

#[test]
fn is_slice_cap_slice() {
    // Slice caps (negative) are slices
    assert!(is_slice_cap(SLICE_FLAG));
    assert!(is_slice_cap(SLICE_FLAG | 1));
    assert!(is_slice_cap(SLICE_FLAG | 0x7FFF_FFFF_FFFF_FFFF));
    assert!(is_slice_cap(-1)); // All bits set
    assert!(is_slice_cap(-100));
}

#[test]
fn make_slice_cap_roundtrip() {
    // Zero offset
    let cap = make_slice_cap(0);
    assert!(is_slice_cap(cap));
    assert_eq!(slice_byte_offset(cap), 0);

    // Small offset
    let cap = make_slice_cap(8);
    assert!(is_slice_cap(cap));
    assert_eq!(slice_byte_offset(cap), 8);

    // Larger offset (e.g., 1000 elements * 8 bytes each)
    let cap = make_slice_cap(8000);
    assert!(is_slice_cap(cap));
    assert_eq!(slice_byte_offset(cap), 8000);

    // Maximum 63-bit offset
    let max_offset = i64::MAX as usize; // 2^63 - 1
    let cap = make_slice_cap(max_offset);
    assert!(is_slice_cap(cap));
    assert_eq!(slice_byte_offset(cap), max_offset);
}

#[test]
fn make_slice_cap_always_negative() {
    // Every make_slice_cap result must be negative (satisfies is_slice_cap)
    for offset in [0, 1, 7, 8, 16, 100, 1024, 1_000_000] {
        let cap = make_slice_cap(offset);
        assert!(
            cap < 0,
            "make_slice_cap({offset}) = {cap}, expected negative"
        );
    }
}

#[test]
fn slice_original_data_computes_correct_base() {
    // Simulate: original buffer at address `base`, slice starts at `base + offset`
    let mut buffer = vec![0u8; 1024];
    let base = buffer.as_mut_ptr();

    // Slice starting at offset 40 (e.g., 5 elements * 8 bytes)
    let offset = 40usize;
    let slice_data = unsafe { base.add(offset) };
    let cap = make_slice_cap(offset);

    let recovered = slice_original_data(slice_data, cap);
    assert_eq!(recovered, base);
}

#[test]
fn slice_original_data_zero_offset() {
    // Slice at offset 0 (full view) — original == slice data
    let mut buffer = vec![0u8; 64];
    let base = buffer.as_mut_ptr();
    let cap = make_slice_cap(0);

    let recovered = slice_original_data(base, cap);
    assert_eq!(recovered, base);
}

#[test]
fn slice_of_slice_offset_accumulation() {
    // Simulate: original at `base`, first slice at offset 16, second slice at offset 8
    // from the first slice. Total offset from base = 16 + 8 = 24.
    let mut buffer = vec![0u8; 1024];
    let base = buffer.as_mut_ptr();

    // First slice: offset 16 from base
    let first_offset = 16usize;
    let first_data = unsafe { base.add(first_offset) };
    let first_cap = make_slice_cap(first_offset);
    assert_eq!(slice_original_data(first_data, first_cap), base);

    // Second slice: additional offset 8 from first slice
    // Total offset from base = 16 + 8 = 24
    let additional_offset = 8usize;
    let total_offset = first_offset + additional_offset;
    let second_data = unsafe { first_data.add(additional_offset) };
    let second_cap = make_slice_cap(total_offset);

    let recovered = slice_original_data(second_data, second_cap);
    assert_eq!(recovered, base);
}

#[test]
fn slice_cap_does_not_interfere_with_regular_cap() {
    // Regular caps should never be confused with slice caps
    for regular_cap in [0i64, 1, 4, 8, 16, 100, 1024, i64::MAX] {
        assert!(
            !is_slice_cap(regular_cap),
            "regular cap {regular_cap} incorrectly identified as slice"
        );
    }
}
