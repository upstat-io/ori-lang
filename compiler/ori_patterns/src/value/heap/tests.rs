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
