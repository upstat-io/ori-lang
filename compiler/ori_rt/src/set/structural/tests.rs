use super::{ori_set_eq, ori_set_hash};
use crate::map::hash_table::{probe_find_slot, set_meta, HashTableLayout, META_OCCUPIED};
use crate::set::OriSet;

extern "C" fn i64_eq(left: *const u8, right: *const u8) -> bool {
    // SAFETY: the fixtures pass pointers to initialized i64 element slots.
    unsafe { left.cast::<i64>().read_unaligned() == right.cast::<i64>().read_unaligned() }
}

extern "C" fn collision_hash(_value: *const u8) -> i64 {
    0
}

extern "C" fn identity_hash(value: *const u8) -> i64 {
    // SAFETY: the fixtures pass pointers to initialized i64 element slots.
    unsafe { value.cast::<i64>().read_unaligned() }
}

fn i64_set(values: &[i64], hash: extern "C" fn(*const u8) -> i64) -> (Vec<u8>, OriSet) {
    let capacity = 8;
    let elem_size = size_of::<i64>();
    let layout = HashTableLayout::for_set(capacity, elem_size);
    let mut storage = vec![0_u8; layout.total_size];
    let data = storage.as_mut_ptr();

    for value in values {
        let value_ptr = std::ptr::from_ref(value).cast::<u8>();
        let bucket = unsafe { probe_find_slot(data, capacity, hash(value_ptr)) };
        // SAFETY: `bucket` is within the allocated table and the destination is
        // an initialized i64-sized slot in the fixture's element region.
        unsafe {
            data.add(layout.keys_offset + bucket * elem_size)
                .cast::<i64>()
                .write_unaligned(*value);
            set_meta(data, bucket, META_OCCUPIED);
        }
    }

    let set = OriSet {
        len: i64::try_from(values.len()).unwrap_or_else(|_| panic!("fixture length fits i64")),
        cap: i64::try_from(capacity).unwrap_or_else(|_| panic!("fixture capacity fits i64")),
        data,
    };
    (storage, set)
}

#[test]
fn equal_members_with_different_collision_order_compare_equal() {
    let (_left_storage, left) = i64_set(&[1, 2, 3], collision_hash);
    let (_right_storage, right) = i64_set(&[3, 1, 2], collision_hash);

    assert!(ori_set_eq(
        &left,
        &right,
        size_of::<i64>() as i64,
        i64_eq,
        collision_hash,
    ));
}

#[test]
fn distinct_members_with_equal_length_compare_unequal() {
    let (_left_storage, left) = i64_set(&[1, 2, 3], collision_hash);
    let (_right_storage, right) = i64_set(&[1, 2, 4], collision_hash);

    assert!(!ori_set_eq(
        &left,
        &right,
        size_of::<i64>() as i64,
        i64_eq,
        collision_hash,
    ));
}

#[test]
fn set_hash_xors_member_hashes_independently_of_bucket_order() {
    let (_left_storage, left) = i64_set(&[1, 2, 4], collision_hash);
    let (_right_storage, right) = i64_set(&[4, 1, 2], collision_hash);
    let expected = 1_i64 ^ 2_i64 ^ 4_i64;

    assert_eq!(
        ori_set_hash(&left, size_of::<i64>() as i64, identity_hash),
        expected,
    );
    assert_eq!(
        ori_set_hash(&right, size_of::<i64>() as i64, identity_hash),
        expected,
    );
}
