use super::*;

#[test]
fn list_iterator_basic() {
    let items = Heap::new(vec![Value::int(1), Value::int(2), Value::int(3)]);
    let iter = IteratorValue::from_list(items);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));

    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn list_iterator_empty() {
    let iter = IteratorValue::from_list(Heap::new(vec![]));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn list_iterator_fused() {
    let items = Heap::new(vec![Value::int(1)]);
    let iter = IteratorValue::from_list(items);

    let (_, iter) = iter.next(); // yields 1
    let (val, iter) = iter.next(); // yields None
    assert_eq!(val, None);

    // Fused: calling next on exhausted iterator still returns None
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_iterator_exclusive() {
    let iter = IteratorValue::from_range(0, 3, 1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_iterator_inclusive() {
    let iter = IteratorValue::from_range(1, 3, 1, true);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_iterator_negative_step() {
    let iter = IteratorValue::from_range(3, 1, -1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_iterator_empty() {
    // Empty range: start >= end with positive step
    let iter = IteratorValue::from_range(5, 3, 1, false);
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn str_iterator_ascii() {
    let data = Heap::new(Cow::Borrowed("abc"));
    let iter = IteratorValue::from_string(data);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('a')));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('b')));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('c')));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn str_iterator_unicode() {
    let data = Heap::new(Cow::Borrowed("café"));
    let iter = IteratorValue::from_string(data);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('c')));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('a')));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('f')));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('é')));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn str_iterator_empty() {
    let data = Heap::new(Cow::Borrowed(""));
    let iter = IteratorValue::from_string(data);
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn map_iterator_basic() {
    let mut map = BTreeMap::new();
    map.insert("a".to_string(), Value::int(1));
    map.insert("b".to_string(), Value::int(2));
    let iter = IteratorValue::from_map(&map);

    // BTreeMap iterates in sorted key order
    let (val, iter) = iter.next();
    assert_eq!(
        val,
        Some(Value::tuple(vec![Value::string("a"), Value::int(1)]))
    );
    let (val, iter) = iter.next();
    assert_eq!(
        val,
        Some(Value::tuple(vec![Value::string("b"), Value::int(2)]))
    );
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn set_iterator_basic() {
    let items = Heap::new(vec![Value::int(10), Value::int(20)]);
    let iter = IteratorValue::from_set(items);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(10)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(20)));
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn iterator_equality() {
    let items = Heap::new(vec![Value::int(1), Value::int(2)]);
    let a = IteratorValue::from_list(items.clone());
    let b = IteratorValue::from_list(items);

    assert_eq!(a, b);

    // After advancing one, they're no longer equal
    let (_, a2) = a.next();
    assert_ne!(a2, b);
}
