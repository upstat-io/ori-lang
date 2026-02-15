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
