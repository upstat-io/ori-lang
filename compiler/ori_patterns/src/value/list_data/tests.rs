//! Unit tests for `ListData` zero-copy slicing.

// Arc is the intentional test subject — we verify Arc::ptr_eq for sharing.
#![expect(clippy::disallowed_types, reason = "testing Arc sharing behavior")]

use std::sync::Arc;

use super::*;

/// Helper: create a `ListData` with [0, 1, 2, 3, 4].
fn sample_list() -> ListData {
    ListData::owned(vec![
        Value::int(0),
        Value::int(1),
        Value::int(2),
        Value::int(3),
        Value::int(4),
    ])
}

#[test]
fn owned_list_basic_properties() {
    let list = sample_list();
    assert_eq!(list.len(), 5);
    assert!(!list.is_slice());
    assert_eq!(list.offset(), 0);
    assert!(!list.is_empty());
}

#[test]
fn deref_gives_correct_slice() {
    let list = sample_list();
    let slice: &[Value] = &list;
    assert_eq!(slice.len(), 5);
    assert_eq!(slice[0], Value::int(0));
    assert_eq!(slice[4], Value::int(4));
}

#[test]
fn slice_shares_backing() {
    let list = sample_list();
    let sliced = list.slice(1, 4);

    // Same Arc pointer means shared backing
    assert!(Arc::ptr_eq(
        list.backing().inner(),
        sliced.backing().inner()
    ));
    assert!(sliced.is_slice());
    assert_eq!(sliced.len(), 3);
    assert_eq!(sliced[0], Value::int(1));
    assert_eq!(sliced[1], Value::int(2));
    assert_eq!(sliced[2], Value::int(3));
}

#[test]
fn slice_of_slice_accumulates_offset() {
    let list = sample_list();
    let first = list.slice(1, 4); // [1, 2, 3]
    let second = first.slice(1, 2); // [2]

    // All three share the same backing
    assert!(Arc::ptr_eq(
        list.backing().inner(),
        second.backing().inner()
    ));
    assert_eq!(second.len(), 1);
    assert_eq!(second.offset(), 2);
    assert_eq!(second[0], Value::int(2));
}

#[test]
fn slice_mutation_materializes() {
    let list = sample_list();
    let mut sliced = list.slice(1, 4); // [1, 2, 3]

    // Push on slice creates independent copy
    sliced.push(Value::int(99));
    assert_eq!(sliced.len(), 4);
    assert_eq!(sliced[3], Value::int(99));

    // Original unchanged
    assert_eq!(list.len(), 5);
    assert_eq!(list[3], Value::int(3));

    // Sliced is no longer a slice after materialization
    assert!(!sliced.is_slice());
}

#[test]
fn owned_mutation_is_cow_inplace() {
    // When there's only one reference, mutation is in-place (no clone)
    let mut list = sample_list();
    list.push(Value::int(5));
    assert_eq!(list.len(), 6);
    assert_eq!(list[5], Value::int(5));
    assert!(!list.is_slice());
}

#[test]
fn shared_mutation_clones() {
    let list = sample_list();
    let mut clone = list.clone();

    // Shared backing (2 references)
    assert!(Arc::ptr_eq(list.backing().inner(), clone.backing().inner()));

    // Push on clone triggers COW
    clone.push(Value::int(99));
    assert_eq!(clone.len(), 6);
    assert_eq!(clone[5], Value::int(99));

    // Original unchanged
    assert_eq!(list.len(), 5);

    // No longer sharing backing
    assert!(!Arc::ptr_eq(
        list.backing().inner(),
        clone.backing().inner()
    ));
}

#[test]
fn take_and_skip_zero_copy() {
    let list = sample_list();
    let taken = list.take(3); // [0, 1, 2]
    let skipped = list.skip(2); // [2, 3, 4]

    // Both share backing with original
    assert!(Arc::ptr_eq(list.backing().inner(), taken.backing().inner()));
    assert!(Arc::ptr_eq(
        list.backing().inner(),
        skipped.backing().inner()
    ));

    assert_eq!(taken.len(), 3);
    assert_eq!(taken[0], Value::int(0));
    assert_eq!(taken[2], Value::int(2));

    assert_eq!(skipped.len(), 3);
    assert_eq!(skipped[0], Value::int(2));
    assert_eq!(skipped[2], Value::int(4));
}

#[test]
fn len_reflects_slice_not_backing() {
    let list = sample_list();
    let sliced = list.slice(1, 3);
    assert_eq!(sliced.len(), 2);
    assert_eq!(list.backing().len(), 5); // backing is still 5
}

#[test]
fn slice_clamping() {
    let list = sample_list();

    // End beyond length is clamped
    let s1 = list.slice(0, 100);
    assert_eq!(s1.len(), 5);

    // Start beyond end gives empty
    let s2 = list.slice(10, 3);
    assert_eq!(s2.len(), 0);
    assert!(s2.is_empty());

    // take(100) on 5-element list gives all 5
    let s3 = list.take(100);
    assert_eq!(s3.len(), 5);

    // skip(100) on 5-element list gives empty
    let s4 = list.skip(100);
    assert_eq!(s4.len(), 0);
}

#[test]
fn empty_list_operations() {
    let list = ListData::owned(vec![]);
    assert_eq!(list.len(), 0);
    assert!(list.is_empty());

    let sliced = list.slice(0, 0);
    assert_eq!(sliced.len(), 0);
    assert!(sliced.is_empty());
}

#[test]
fn set_on_slice_materializes() {
    let list = sample_list();
    let mut sliced = list.slice(1, 4); // [1, 2, 3]
    sliced.set(0, Value::int(99));

    assert_eq!(sliced[0], Value::int(99));
    // Original unchanged
    assert_eq!(list[1], Value::int(1));
}

#[test]
fn insert_on_slice() {
    let list = sample_list();
    let mut sliced = list.slice(1, 3); // [1, 2]
    sliced.insert(1, Value::int(99)); // [1, 99, 2]

    assert_eq!(sliced.len(), 3);
    assert_eq!(sliced[0], Value::int(1));
    assert_eq!(sliced[1], Value::int(99));
    assert_eq!(sliced[2], Value::int(2));
}

#[test]
fn remove_on_slice() {
    let list = sample_list();
    let mut sliced = list.slice(1, 4); // [1, 2, 3]
    sliced.remove(1); // [1, 3]

    assert_eq!(sliced.len(), 2);
    assert_eq!(sliced[0], Value::int(1));
    assert_eq!(sliced[1], Value::int(3));
}

#[test]
fn reverse_on_slice() {
    let list = sample_list();
    let mut sliced = list.slice(1, 4); // [1, 2, 3]
    sliced.reverse(); // [3, 2, 1]

    assert_eq!(sliced[0], Value::int(3));
    assert_eq!(sliced[1], Value::int(2));
    assert_eq!(sliced[2], Value::int(1));
    // Original unchanged
    assert_eq!(list[1], Value::int(1));
}

#[test]
fn sort_by_on_slice() {
    let mut sliced = sample_list().slice(0, 5);
    // Reverse sort
    sliced.sort_by(|a, b| {
        let Value::Int(ai) = a else { unreachable!() };
        let Value::Int(bi) = b else { unreachable!() };
        bi.raw().cmp(&ai.raw())
    });

    assert_eq!(sliced[0], Value::int(4));
    assert_eq!(sliced[4], Value::int(0));
}

#[test]
fn extend_from_slice_on_slice() {
    let list = sample_list();
    let mut sliced = list.slice(0, 2); // [0, 1]
    sliced.extend_from_slice(&[Value::int(10), Value::int(11)]);

    assert_eq!(sliced.len(), 4);
    assert_eq!(sliced[2], Value::int(10));
    assert_eq!(sliced[3], Value::int(11));
}

#[test]
fn into_vec_owned() {
    let list = sample_list();
    let vec = list.into_vec();
    assert_eq!(vec.len(), 5);
    assert_eq!(vec[0], Value::int(0));
}

#[test]
fn into_vec_slice() {
    let list = sample_list();
    let sliced = list.slice(1, 3); // [1, 2]
    let vec = sliced.into_vec();
    assert_eq!(vec.len(), 2);
    assert_eq!(vec[0], Value::int(1));
    assert_eq!(vec[1], Value::int(2));
}

#[test]
fn equality_compares_visible_window() {
    let a = sample_list();
    let b = sample_list();
    assert_eq!(a, b);

    let slice_a = a.slice(1, 3);
    let slice_b = b.slice(1, 3);
    assert_eq!(slice_a, slice_b);

    // Different windows are not equal
    let slice_c = a.slice(0, 3);
    assert_ne!(slice_a, slice_c);
}

#[test]
fn hash_reflects_visible_window() {
    use std::collections::hash_map::DefaultHasher;

    let a = sample_list();
    let b = sample_list();

    let hash = |list: &ListData| -> u64 {
        let mut hasher = DefaultHasher::new();
        list.hash(&mut hasher);
        hasher.finish()
    };

    // Same content, same hash
    assert_eq!(hash(&a), hash(&b));

    // Slice with same elements, same hash
    let slice_a = a.slice(1, 3);
    let slice_b = b.slice(1, 3);
    assert_eq!(hash(&slice_a), hash(&slice_b));
}
