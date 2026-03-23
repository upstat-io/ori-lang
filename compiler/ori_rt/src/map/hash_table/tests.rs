use super::*;

// ── Layout Tests ────────────────────────────────────────────────────────

#[test]
fn map_layout_basic() {
    let layout = HashTableLayout::for_map(4, 8, 8);
    // metadata: 4 bytes, aligned to 8 = 8
    assert_eq!(layout.metadata_bytes, 8);
    assert_eq!(layout.keys_offset, 8);
    assert_eq!(layout.vals_offset, 8 + 4 * 8); // 40
    assert_eq!(layout.total_size, 8 + 4 * 8 + 4 * 8); // 72
}

#[test]
fn map_layout_larger_cap() {
    let layout = HashTableLayout::for_map(16, 8, 16);
    // metadata: 16 bytes, already aligned to 8
    assert_eq!(layout.metadata_bytes, 16);
    assert_eq!(layout.keys_offset, 16);
    assert_eq!(layout.vals_offset, 16 + 16 * 8); // 144
    assert_eq!(layout.total_size, 16 + 16 * 8 + 16 * 16); // 400
}

#[test]
fn set_layout_basic() {
    let layout = HashTableLayout::for_set(4, 8);
    assert_eq!(layout.metadata_bytes, 8);
    assert_eq!(layout.keys_offset, 8);
    assert_eq!(layout.vals_offset, 0);
    assert_eq!(layout.total_size, 8 + 4 * 8); // 40
}

// ── Capacity Tests ──────────────────────────────────────────────────────

#[test]
fn next_hash_capacity_minimum() {
    assert_eq!(next_hash_capacity(0), 4);
    assert_eq!(next_hash_capacity(1), 4);
    assert_eq!(next_hash_capacity(2), 4);
    assert_eq!(next_hash_capacity(3), 4);
}

#[test]
fn next_hash_capacity_grows() {
    // 4 entries need cap >= ceil(4 * 4/3) = 6 → next pow2 = 8
    assert_eq!(next_hash_capacity(4), 8);
    // 6 entries need cap >= ceil(6 * 4/3) = 8 → 8
    assert_eq!(next_hash_capacity(6), 8);
    // 7 entries need cap >= ceil(7 * 4/3) = 10 → 16
    assert_eq!(next_hash_capacity(7), 16);
    // 100 entries need cap >= ceil(100 * 4/3) = 134 → 256
    assert_eq!(next_hash_capacity(100), 256);
}

#[test]
fn needs_rehash_empty() {
    assert!(needs_rehash(0, 0));
}

#[test]
fn needs_rehash_under_load() {
    // cap=4, threshold = 3 (4*3/4)
    assert!(!needs_rehash(0, 4));
    assert!(!needs_rehash(1, 4));
    assert!(!needs_rehash(2, 4));
    assert!(needs_rehash(3, 4)); // 3 >= 3
    assert!(needs_rehash(4, 4));
}

#[test]
fn needs_rehash_larger() {
    // cap=8, threshold = 6 (8*3/4)
    assert!(!needs_rehash(5, 8));
    assert!(needs_rehash(6, 8));
}

// ── Align Up Tests ──────────────────────────────────────────────────────

#[test]
fn align_up_basics() {
    assert_eq!(align_up(0, 8), 0);
    assert_eq!(align_up(1, 8), 8);
    assert_eq!(align_up(7, 8), 8);
    assert_eq!(align_up(8, 8), 8);
    assert_eq!(align_up(9, 8), 16);
    assert_eq!(align_up(16, 8), 16);
}

// ── Metadata Tests ──────────────────────────────────────────────────────

#[test]
fn metadata_read_write() {
    let _g = crate::test_helpers::lock_rc();
    let mut buf = [META_EMPTY; 8];
    unsafe {
        set_meta(buf.as_mut_ptr(), 0, META_OCCUPIED);
        set_meta(buf.as_mut_ptr(), 3, META_TOMBSTONE);
        assert_eq!(get_meta(buf.as_ptr(), 0), META_OCCUPIED);
        assert_eq!(get_meta(buf.as_ptr(), 1), META_EMPTY);
        assert_eq!(get_meta(buf.as_ptr(), 3), META_TOMBSTONE);
    }
}

// ── Probing Tests ───────────────────────────────────────────────────────

extern "C" fn i64_eq(a: *const u8, b: *const u8) -> bool {
    unsafe { *a.cast::<i64>() == *b.cast::<i64>() }
}

extern "C" fn i64_hash(a: *const u8) -> i64 {
    unsafe { *a.cast::<i64>() }
}

/// Helper to create a small map hash table on the heap for testing.
unsafe fn make_test_map_table(entries: &[(i64, i64)]) -> (*mut u8, usize) {
    let cap = next_hash_capacity(entries.len());
    let layout = HashTableLayout::for_map(cap, 8, 8);
    let data = crate::rc::ori_rc_alloc(layout.total_size, 8);
    std::ptr::write_bytes(data, META_EMPTY, layout.metadata_bytes);

    for &(key, val) in entries {
        let hash = key; // identity hash
        let bucket = probe_find_slot(data, cap, hash);
        let key_ptr = data.add(layout.keys_offset + bucket * 8);
        let val_ptr = data.add(layout.vals_offset + bucket * 8);
        key_ptr.cast::<i64>().write(key);
        val_ptr.cast::<i64>().write(val);
        set_meta(data, bucket, META_OCCUPIED);
    }

    (data, cap)
}

#[test]
fn probe_find_existing_key() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        let (data, cap) = make_test_map_table(&[(10, 100), (20, 200), (30, 300)]);
        let layout = HashTableLayout::for_map(cap, 8, 8);

        let needle: i64 = 20;
        let result = probe_find(
            data,
            cap,
            layout.keys_offset,
            std::ptr::from_ref::<i64>(&needle).cast::<u8>(),
            20,
            8,
            i64_eq,
        );
        let bucket = result.unwrap_or_else(|| panic!("key 20 should be found"));
        let val = *data.add(layout.vals_offset + bucket * 8).cast::<i64>();
        assert_eq!(val, 200);

        crate::rc::ori_rc_free(data, layout.total_size, 8);
    }
}

#[test]
fn probe_find_missing_key() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        let (data, cap) = make_test_map_table(&[(10, 100), (20, 200)]);
        let layout = HashTableLayout::for_map(cap, 8, 8);

        let needle: i64 = 99;
        let result = probe_find(
            data,
            cap,
            layout.keys_offset,
            std::ptr::from_ref::<i64>(&needle).cast::<u8>(),
            99,
            8,
            i64_eq,
        );
        assert!(result.is_none());

        crate::rc::ori_rc_free(data, layout.total_size, 8);
    }
}

#[test]
fn probe_find_slot_after_tombstone() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        // Create table, insert key, tombstone it, verify new insert goes there
        let cap = 4;
        let layout = HashTableLayout::for_map(cap, 8, 8);
        let data = crate::rc::ori_rc_alloc(layout.total_size, 8);
        std::ptr::write_bytes(data, META_EMPTY, layout.metadata_bytes);

        // Insert key=0 at bucket 0
        set_meta(data, 0, META_OCCUPIED);
        let key_ptr = data.add(layout.keys_offset).cast::<i64>();
        key_ptr.write(0);

        // Tombstone bucket 0
        set_meta(data, 0, META_TOMBSTONE);

        // probe_find_slot should return bucket 0 (tombstone is reusable)
        let slot = probe_find_slot(data, cap, 0);
        assert_eq!(slot, 0);

        crate::rc::ori_rc_free(data, layout.total_size, 8);
    }
}

#[test]
fn probe_find_skips_tombstone() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        // Insert keys 0 and 4 (both hash to bucket 0 with cap=4)
        // Tombstone key 0, verify key 4 is still found
        let cap = 4;
        let layout = HashTableLayout::for_map(cap, 8, 8);
        let data = crate::rc::ori_rc_alloc(layout.total_size, 8);
        std::ptr::write_bytes(data, META_EMPTY, layout.metadata_bytes);

        // Key 0 → bucket 0
        set_meta(data, 0, META_OCCUPIED);
        data.add(layout.keys_offset).cast::<i64>().write(0);
        data.add(layout.vals_offset).cast::<i64>().write(100);

        // Key 4 → bucket 0 is taken, linear probe → bucket 1
        set_meta(data, 1, META_OCCUPIED);
        data.add(layout.keys_offset + 8).cast::<i64>().write(4);
        data.add(layout.vals_offset + 8).cast::<i64>().write(400);

        // Tombstone key 0
        set_meta(data, 0, META_TOMBSTONE);

        // Key 4 should still be found by probing past tombstone
        let needle: i64 = 4;
        let result = probe_find(
            data,
            cap,
            layout.keys_offset,
            std::ptr::from_ref::<i64>(&needle).cast::<u8>(),
            4,
            8,
            i64_eq,
        );
        assert_eq!(result, Some(1));

        // Key 0 should NOT be found (tombstoned)
        let needle: i64 = 0;
        let result = probe_find(
            data,
            cap,
            layout.keys_offset,
            std::ptr::from_ref::<i64>(&needle).cast::<u8>(),
            0,
            8,
            i64_eq,
        );
        assert!(result.is_none());

        crate::rc::ori_rc_free(data, layout.total_size, 8);
    }
}

// ── Rehash Tests ────────────────────────────────────────────────────────

#[test]
fn rehash_map_preserves_entries() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        let entries = [(1, 10), (2, 20), (3, 30), (5, 50), (8, 80)];
        let (old_data, old_cap) = make_test_map_table(&entries);
        let new_cap = next_hash_capacity(entries.len() * 2); // force growth

        let new_data = rehash_map(old_data, old_cap, new_cap, 8, 8, i64_hash);
        assert_eq!(count_occupied(new_data, new_cap), entries.len());

        let new_layout = HashTableLayout::for_map(new_cap, 8, 8);
        // Verify all entries are findable
        for &(key, val) in &entries {
            let result = probe_find(
                new_data,
                new_cap,
                new_layout.keys_offset,
                std::ptr::from_ref::<i64>(&key).cast::<u8>(),
                key,
                8,
                i64_eq,
            );
            let bucket = result.unwrap_or_else(|| panic!("key should exist after rehash"));
            let found_val = *new_data
                .add(new_layout.vals_offset + bucket * 8)
                .cast::<i64>();
            assert_eq!(found_val, val);
        }

        let old_layout = HashTableLayout::for_map(old_cap, 8, 8);
        crate::rc::ori_rc_free(old_data, old_layout.total_size, 8);
        crate::rc::ori_rc_free(new_data, new_layout.total_size, 8);
    }
}

#[test]
fn rehash_set_preserves_entries() {
    let _g = crate::test_helpers::lock_rc();
    unsafe {
        let elems: Vec<i64> = vec![10, 20, 30, 40, 50];
        let old_cap = next_hash_capacity(elems.len());
        let old_layout = HashTableLayout::for_set(old_cap, 8);
        let old_data = crate::rc::ori_rc_alloc(old_layout.total_size, 8);
        std::ptr::write_bytes(old_data, META_EMPTY, old_layout.metadata_bytes);

        for &elem in &elems {
            let bucket = probe_find_slot(old_data, old_cap, elem);
            old_data
                .add(old_layout.keys_offset + bucket * 8)
                .cast::<i64>()
                .write(elem);
            set_meta(old_data, bucket, META_OCCUPIED);
        }

        let new_cap = next_hash_capacity(elems.len() * 2);
        let new_data = rehash_set(old_data, old_cap, new_cap, 8, i64_hash, None);
        assert_eq!(count_occupied(new_data, new_cap), elems.len());

        let new_layout = HashTableLayout::for_set(new_cap, 8);
        for &elem in &elems {
            let result = probe_find(
                new_data,
                new_cap,
                new_layout.keys_offset,
                std::ptr::from_ref::<i64>(&elem).cast::<u8>(),
                elem,
                8,
                i64_eq,
            );
            assert!(result.is_some(), "element {elem} should exist after rehash");
        }

        crate::rc::ori_rc_free(old_data, old_layout.total_size, 8);
        crate::rc::ori_rc_free(new_data, new_layout.total_size, 8);
    }
}

// ── Count Occupied Test ─────────────────────────────────────────────────

#[test]
fn count_occupied_mixed_metadata() {
    let mut buf = [META_EMPTY; 8];
    buf[0] = META_OCCUPIED;
    buf[2] = META_OCCUPIED;
    buf[4] = META_TOMBSTONE;
    buf[6] = META_OCCUPIED;
    unsafe {
        assert_eq!(count_occupied(buf.as_ptr(), 8), 3);
    }
}
