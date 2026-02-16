use std::hash::{Hash, Hasher};

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

// ── Adapter variant tests ───────────────────────────────────────────

fn make_list_iter(vals: &[i64]) -> IteratorValue {
    let items = Heap::new(vals.iter().map(|&v| Value::int(v)).collect());
    IteratorValue::from_list(items)
}

fn hash_of(val: &IteratorValue) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    val.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn mapped_debug_format() {
    let source = make_list_iter(&[1, 2, 3]);
    let mapped = IteratorValue::Mapped {
        source: Box::new(source),
        transform: Box::new(Value::Bool(true)), // dummy
    };
    let debug = format!("{mapped:?}");
    assert!(debug.starts_with("MappedIterator("));
}

#[test]
fn filtered_debug_format() {
    let source = make_list_iter(&[1, 2]);
    let filtered = IteratorValue::Filtered {
        source: Box::new(source),
        predicate: Box::new(Value::Bool(true)), // dummy
    };
    let debug = format!("{filtered:?}");
    assert!(debug.starts_with("FilteredIterator("));
}

#[test]
fn take_debug_format() {
    let source = make_list_iter(&[1, 2, 3]);
    let take = IteratorValue::TakeN {
        source: Box::new(source),
        remaining: 2,
    };
    let debug = format!("{take:?}");
    assert!(debug.starts_with("TakeIterator(remaining=2"));
}

#[test]
fn skip_debug_format() {
    let source = make_list_iter(&[1, 2, 3]);
    let skip = IteratorValue::SkipN {
        source: Box::new(source),
        remaining: 1,
    };
    let debug = format!("{skip:?}");
    assert!(debug.starts_with("SkipIterator(remaining=1"));
}

#[test]
fn mapped_equality() {
    let s1 = make_list_iter(&[1, 2]);
    let s2 = make_list_iter(&[1, 2]);
    let transform = Box::new(Value::Bool(true));

    let a = IteratorValue::Mapped {
        source: Box::new(s1),
        transform: transform.clone(),
    };
    let b = IteratorValue::Mapped {
        source: Box::new(s2),
        transform,
    };
    assert_eq!(a, b);
}

#[test]
fn mapped_inequality_different_transform() {
    let s1 = make_list_iter(&[1, 2]);
    let s2 = make_list_iter(&[1, 2]);

    let a = IteratorValue::Mapped {
        source: Box::new(s1),
        transform: Box::new(Value::Bool(true)),
    };
    let b = IteratorValue::Mapped {
        source: Box::new(s2),
        transform: Box::new(Value::Bool(false)),
    };
    assert_ne!(a, b);
}

#[test]
fn take_equality() {
    let s1 = make_list_iter(&[1, 2, 3]);
    let s2 = make_list_iter(&[1, 2, 3]);

    let a = IteratorValue::TakeN {
        source: Box::new(s1),
        remaining: 2,
    };
    let b = IteratorValue::TakeN {
        source: Box::new(s2),
        remaining: 2,
    };
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn take_inequality_different_remaining() {
    let s1 = make_list_iter(&[1, 2, 3]);
    let s2 = make_list_iter(&[1, 2, 3]);

    let a = IteratorValue::TakeN {
        source: Box::new(s1),
        remaining: 2,
    };
    let b = IteratorValue::TakeN {
        source: Box::new(s2),
        remaining: 3,
    };
    assert_ne!(a, b);
}

#[test]
fn adapter_not_equal_to_source() {
    let source = make_list_iter(&[1, 2, 3]);
    let take = IteratorValue::TakeN {
        source: Box::new(make_list_iter(&[1, 2, 3])),
        remaining: 3,
    };
    assert_ne!(source, take);
}
