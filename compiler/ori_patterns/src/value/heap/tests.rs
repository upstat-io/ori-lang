#![expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
#![expect(clippy::expect_used, reason = "Tests use expect for clarity")]

use super::*;

#[test]
fn test_heap_deref() {
    let h = Heap::new(42i64);
    assert_eq!(*h, 42);
}

#[test]
fn test_heap_clone() {
    let h1 = Heap::new(vec![1, 2, 3]);
    let h2 = h1.clone();
    assert_eq!(*h1, *h2);
    // They share the same allocation
    assert!(Arc::ptr_eq(&h1.0, &h2.0));
}

#[test]
fn test_heap_eq() {
    let h1 = Heap::new("hello".to_string());
    let h2 = Heap::new("hello".to_string());
    let h3 = Heap::new("world".to_string());
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn try_into_inner_unique_ref_succeeds() {
    let h = Heap::new(vec![1, 2, 3]);
    let owned = h.try_into_inner().expect("refcount should be 1");
    assert_eq!(owned, vec![1, 2, 3]);
}

#[test]
fn try_into_inner_shared_ref_fails() {
    let h1 = Heap::new(vec![1, 2, 3]);
    let _h2 = h1.clone(); // bump refcount to 2
    let result = h1.try_into_inner();
    assert!(result.is_err());
    // The Err variant returns the original Heap unchanged
    let recovered = result.unwrap_err();
    assert_eq!(*recovered, vec![1, 2, 3]);
}

#[test]
fn try_into_inner_after_drop_succeeds() {
    let h1 = Heap::new(42i64);
    let h2 = h1.clone();
    drop(h2); // refcount back to 1
    assert_eq!(h1.try_into_inner().unwrap(), 42);
}

// COW (copy-on-write) tests for Heap::make_mut

#[test]
fn make_mut_unique_ref_mutates_in_place() {
    let mut h = Heap::new(vec![1, 2, 3]);
    let arc_ptr_before = Arc::as_ptr(&h.0);
    assert_eq!(Arc::strong_count(&h.0), 1);
    h.make_mut().push(4);
    // Same Arc allocation — no clone happened (Vec buffer may reallocate, that's fine)
    assert_eq!(Arc::as_ptr(&h.0), arc_ptr_before);
    assert_eq!(*h, vec![1, 2, 3, 4]);
}

#[test]
fn make_mut_shared_ref_clones_data() {
    let mut h1 = Heap::new(vec![1, 2, 3]);
    let h2 = h1.clone();
    assert_eq!(Arc::strong_count(&h1.0), 2);
    // make_mut on shared ref must clone the Vec
    h1.make_mut().push(4);
    // h1 now has its own allocation
    assert_eq!(*h1, vec![1, 2, 3, 4]);
    // h2 is unchanged — still points to original data
    assert_eq!(*h2, vec![1, 2, 3]);
    // No longer sharing the same Arc
    assert!(!Arc::ptr_eq(&h1.0, &h2.0));
}

#[test]
fn make_mut_chain_on_unique_ref_never_clones() {
    let mut h = Heap::new(Vec::with_capacity(200));
    let ptr_original = h.0.as_ptr();
    // 100 pushes on a unique reference — all should be in-place
    for i in 0..100 {
        assert_eq!(Arc::strong_count(&h.0), 1);
        h.make_mut().push(i);
    }
    // Same underlying Vec buffer (capacity was pre-allocated)
    assert_eq!(h.0.as_ptr(), ptr_original);
    assert_eq!(h.len(), 100);
    assert_eq!(h[0], 0);
    assert_eq!(h[99], 99);
}

#[test]
fn make_mut_after_share_dropped_is_in_place() {
    let mut h = Heap::new(vec![1, 2, 3]);
    let shared = h.clone();
    assert_eq!(Arc::strong_count(&h.0), 2);
    drop(shared); // RC back to 1
    assert_eq!(Arc::strong_count(&h.0), 1);
    let arc_ptr_before = Arc::as_ptr(&h.0);
    h.make_mut().push(4);
    // In-place — no clone needed since RC is back to 1 (Vec buffer may reallocate, that's fine)
    assert_eq!(Arc::as_ptr(&h.0), arc_ptr_before);
    assert_eq!(*h, vec![1, 2, 3, 4]);
}
