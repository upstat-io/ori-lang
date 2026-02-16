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
    let iter = IteratorValue::from_range(0, Some(3), 1, false);

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
    let iter = IteratorValue::from_range(1, Some(3), 1, true);

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
    let iter = IteratorValue::from_range(3, Some(1), -1, false);

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
    let iter = IteratorValue::from_range(5, Some(3), 1, false);
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

// size_hint tests

#[test]
fn size_hint_list() {
    let iter = make_list_iter(&[1, 2, 3]);
    assert_eq!(iter.size_hint(), (3, Some(3)));

    let (_, iter) = iter.next();
    assert_eq!(iter.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_list_empty() {
    let iter = make_list_iter(&[]);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

#[test]
fn size_hint_range_exclusive() {
    let iter = IteratorValue::from_range(0, Some(5), 1, false);
    assert_eq!(iter.size_hint(), (5, Some(5)));
}

#[test]
fn size_hint_range_inclusive() {
    let iter = IteratorValue::from_range(0, Some(5), 1, true);
    assert_eq!(iter.size_hint(), (6, Some(6)));
}

#[test]
fn size_hint_range_step() {
    // 0, 2, 4 → 3 items
    let iter = IteratorValue::from_range(0, Some(5), 2, false);
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_range_negative_step() {
    // 3, 2 → 2 items (exclusive, stops before 1)
    let iter = IteratorValue::from_range(3, Some(1), -1, false);
    assert_eq!(iter.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_range_empty() {
    let iter = IteratorValue::from_range(5, Some(3), 1, false);
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

#[test]
fn size_hint_str() {
    // "abc" = 3 bytes, 3 chars → lower=1 (ceil(3/4)), upper=3
    let data = Heap::new(Cow::Borrowed("abc"));
    let iter = IteratorValue::from_string(data);
    let (lower, upper) = iter.size_hint();
    assert!(lower <= 3);
    assert_eq!(upper, Some(3));
}

#[test]
fn size_hint_mapped() {
    // Mapped preserves source bounds
    let source = make_list_iter(&[1, 2, 3]);
    let mapped = IteratorValue::Mapped {
        source: Box::new(source),
        transform: Box::new(Value::Bool(true)),
    };
    assert_eq!(mapped.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_filtered() {
    // Filtered: lower=0, upper from source
    let source = make_list_iter(&[1, 2, 3]);
    let filtered = IteratorValue::Filtered {
        source: Box::new(source),
        predicate: Box::new(Value::Bool(true)),
    };
    assert_eq!(filtered.size_hint(), (0, Some(3)));
}

#[test]
fn size_hint_take() {
    // Take 2 from 5-element source
    let source = make_list_iter(&[1, 2, 3, 4, 5]);
    let take = IteratorValue::TakeN {
        source: Box::new(source),
        remaining: 2,
    };
    assert_eq!(take.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_take_exceeds_source() {
    // Take 10 from 3-element source — capped at source size
    let source = make_list_iter(&[1, 2, 3]);
    let take = IteratorValue::TakeN {
        source: Box::new(source),
        remaining: 10,
    };
    assert_eq!(take.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_skip() {
    // Skip 2 from 5-element source → 3 remaining
    let source = make_list_iter(&[1, 2, 3, 4, 5]);
    let skip = IteratorValue::SkipN {
        source: Box::new(source),
        remaining: 2,
    };
    assert_eq!(skip.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_skip_exceeds_source() {
    // Skip 10 from 3-element source → 0 remaining
    let source = make_list_iter(&[1, 2, 3]);
    let skip = IteratorValue::SkipN {
        source: Box::new(source),
        remaining: 10,
    };
    assert_eq!(skip.size_hint(), (0, Some(0)));
}

// ── New adapter variant tests (Phase 2C/2D) ─────────────────────────

// Debug format tests

#[test]
fn enumerated_debug_format() {
    let source = make_list_iter(&[1, 2]);
    let iter = IteratorValue::Enumerated {
        source: Box::new(source),
        index: 0,
    };
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("EnumeratedIterator(index=0"));
}

#[test]
fn zipped_debug_format() {
    let left = make_list_iter(&[1, 2]);
    let right = make_list_iter(&[3, 4]);
    let iter = IteratorValue::Zipped {
        left: Box::new(left),
        right: Box::new(right),
    };
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("ZippedIterator("));
}

#[test]
fn chained_debug_format() {
    let first = make_list_iter(&[1]);
    let second = make_list_iter(&[2]);
    let iter = IteratorValue::Chained {
        first: Box::new(first),
        second: Box::new(second),
        first_done: false,
    };
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("ChainedIterator(first_done=false"));
}

#[test]
fn flattened_debug_format() {
    let source = make_list_iter(&[1]);
    let iter = IteratorValue::Flattened {
        source: Box::new(source),
        inner: None,
    };
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("FlattenedIterator(inner=false"));
}

#[test]
fn cycled_debug_format() {
    let source = make_list_iter(&[1, 2]);
    let iter = IteratorValue::Cycled {
        source: Some(Box::new(source)),
        buffer: Vec::new(),
        buf_pos: 0,
    };
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("CycledIterator(buffered=0"));
}

// size_hint tests

#[test]
fn size_hint_enumerated() {
    // 1:1 with source
    let source = make_list_iter(&[1, 2, 3]);
    let iter = IteratorValue::Enumerated {
        source: Box::new(source),
        index: 0,
    };
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_zipped_equal() {
    let left = make_list_iter(&[1, 2, 3]);
    let right = make_list_iter(&[4, 5, 6]);
    let iter = IteratorValue::Zipped {
        left: Box::new(left),
        right: Box::new(right),
    };
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn size_hint_zipped_unequal() {
    // Shorter side limits
    let left = make_list_iter(&[1, 2]);
    let right = make_list_iter(&[4, 5, 6, 7]);
    let iter = IteratorValue::Zipped {
        left: Box::new(left),
        right: Box::new(right),
    };
    assert_eq!(iter.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_chained() {
    let first = make_list_iter(&[1, 2]);
    let second = make_list_iter(&[3, 4, 5]);
    let iter = IteratorValue::Chained {
        first: Box::new(first),
        second: Box::new(second),
        first_done: false,
    };
    assert_eq!(iter.size_hint(), (5, Some(5)));
}

#[test]
fn size_hint_chained_first_done() {
    let first = make_list_iter(&[]);
    let second = make_list_iter(&[3, 4]);
    let iter = IteratorValue::Chained {
        first: Box::new(first),
        second: Box::new(second),
        first_done: true,
    };
    assert_eq!(iter.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_flattened() {
    // Unknowable
    let source = make_list_iter(&[1, 2]);
    let iter = IteratorValue::Flattened {
        source: Box::new(source),
        inner: None,
    };
    assert_eq!(iter.size_hint(), (0, None));
}

#[test]
fn size_hint_cycled_with_source() {
    // Still consuming — unknown upper
    let source = make_list_iter(&[1, 2, 3]);
    let iter = IteratorValue::Cycled {
        source: Some(Box::new(source)),
        buffer: Vec::new(),
        buf_pos: 0,
    };
    let (lower, upper) = iter.size_hint();
    assert_eq!(lower, 3);
    assert_eq!(upper, None);
}

#[test]
fn size_hint_cycled_replaying() {
    // Non-empty buffer → infinite
    let iter = IteratorValue::Cycled {
        source: None,
        buffer: vec![Value::int(1), Value::int(2)],
        buf_pos: 0,
    };
    assert_eq!(iter.size_hint(), (usize::MAX, None));
}

#[test]
fn size_hint_cycled_empty() {
    // Empty buffer, no source → exhausted
    let iter = IteratorValue::Cycled {
        source: None,
        buffer: Vec::new(),
        buf_pos: 0,
    };
    assert_eq!(iter.size_hint(), (0, Some(0)));
}

// Equality tests

#[test]
fn enumerated_equality() {
    let s1 = make_list_iter(&[1, 2]);
    let s2 = make_list_iter(&[1, 2]);
    let a = IteratorValue::Enumerated {
        source: Box::new(s1),
        index: 0,
    };
    let b = IteratorValue::Enumerated {
        source: Box::new(s2),
        index: 0,
    };
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn zipped_equality() {
    let a = IteratorValue::Zipped {
        left: Box::new(make_list_iter(&[1])),
        right: Box::new(make_list_iter(&[2])),
    };
    let b = IteratorValue::Zipped {
        left: Box::new(make_list_iter(&[1])),
        right: Box::new(make_list_iter(&[2])),
    };
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn chained_equality() {
    let a = IteratorValue::Chained {
        first: Box::new(make_list_iter(&[1])),
        second: Box::new(make_list_iter(&[2])),
        first_done: false,
    };
    let b = IteratorValue::Chained {
        first: Box::new(make_list_iter(&[1])),
        second: Box::new(make_list_iter(&[2])),
        first_done: false,
    };
    assert_eq!(a, b);
}

#[test]
fn chained_inequality_different_done() {
    let a = IteratorValue::Chained {
        first: Box::new(make_list_iter(&[1])),
        second: Box::new(make_list_iter(&[2])),
        first_done: false,
    };
    let b = IteratorValue::Chained {
        first: Box::new(make_list_iter(&[1])),
        second: Box::new(make_list_iter(&[2])),
        first_done: true,
    };
    assert_ne!(a, b);
}

// from_value tests

#[test]
fn from_value_list() {
    let val = Value::list(vec![Value::int(1), Value::int(2)]);
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("list should be iterable");
    };
    let (item, _) = iter.next();
    assert_eq!(item, Some(Value::int(1)));
}

#[test]
fn from_value_range() {
    let val = Value::Range(super::super::RangeValue {
        start: 0,
        end: Some(3),
        step: 1,
        inclusive: false,
    });
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("range should be iterable");
    };
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

#[test]
fn from_value_str() {
    let val = Value::string("hi");
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("str should be iterable");
    };
    let (item, _) = iter.next();
    assert_eq!(item, Some(Value::Char('h')));
}

#[test]
fn from_value_iterator() {
    let inner = IteratorValue::from_range(0, Some(5), 1, false);
    let val = Value::iterator(inner.clone());
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("iterator should be iterable");
    };
    assert_eq!(iter.size_hint(), (5, Some(5)));
}

#[test]
fn from_value_option_some() {
    let val = Value::some(Value::int(99));
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("Some should be iterable");
    };
    let (item, iter) = iter.next();
    assert_eq!(item, Some(Value::int(99)));
    let (item, _) = iter.next();
    assert_eq!(item, None);
}

#[test]
fn from_value_option_none() {
    let Some(iter) = IteratorValue::from_value(&Value::None) else {
        panic!("None should be iterable");
    };
    let (item, _) = iter.next();
    assert_eq!(item, None);
}

#[test]
fn from_value_set() {
    let mut set = BTreeMap::new();
    set.insert("int:1".to_string(), Value::int(1));
    set.insert("int:2".to_string(), Value::int(2));
    set.insert("int:3".to_string(), Value::int(3));
    let val = Value::set(set);
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("set should be iterable");
    };
    // BTreeMap order: keys are sorted, so values come out in key order
    let (item, iter) = iter.next();
    assert_eq!(item, Some(Value::int(1)));
    let (item, iter) = iter.next();
    assert_eq!(item, Some(Value::int(2)));
    let (item, iter) = iter.next();
    assert_eq!(item, Some(Value::int(3)));
    let (item, _) = iter.next();
    assert_eq!(item, None);
}

#[test]
fn from_value_set_empty() {
    let val = Value::set(BTreeMap::new());
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("empty set should be iterable");
    };
    let (item, _) = iter.next();
    assert_eq!(item, None);
}

#[test]
fn from_value_non_iterable() {
    assert!(IteratorValue::from_value(&Value::int(42)).is_none());
    assert!(IteratorValue::from_value(&Value::Bool(true)).is_none());
    assert!(IteratorValue::from_value(&Value::Void).is_none());
}

// ── DoubleEndedIterator (next_back) tests ────────────────────────────

#[test]
fn list_next_back_basic() {
    let items = Heap::new(vec![Value::int(1), Value::int(2), Value::int(3)]);
    let iter = IteratorValue::from_list(items);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(2)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(1)));

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn list_next_back_empty() {
    let iter = IteratorValue::from_list(Heap::new(vec![]));
    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn list_next_back_fused() {
    let items = Heap::new(vec![Value::int(1)]);
    let iter = IteratorValue::from_list(items);

    let (_, iter) = iter.next_back(); // yields 1
    let (val, iter) = iter.next_back(); // yields None
    assert_eq!(val, None);

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn list_interleaved_next_and_next_back() {
    let items = Heap::new(vec![
        Value::int(1),
        Value::int(2),
        Value::int(3),
        Value::int(4),
        Value::int(5),
    ]);
    let iter = IteratorValue::from_list(items);

    // next() from front
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));

    // next_back() from back
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(5)));

    // next() again
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));

    // next_back() again
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(4)));

    // One element left (3)
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));

    // Exhausted
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_next_back_exclusive() {
    // 0..5 by 1 → values: 0, 1, 2, 3, 4
    let iter = IteratorValue::from_range(0, Some(5), 1, false);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(4)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(2)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(1)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(0)));

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn range_next_back_inclusive() {
    // 1..=3 by 1 → values: 1, 2, 3
    let iter = IteratorValue::from_range(1, Some(3), 1, true);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(2)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(1)));

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn range_next_back_step() {
    // 0..10 by 3 → values: 0, 3, 6, 9
    let iter = IteratorValue::from_range(0, Some(10), 3, false);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(9)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(6)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(0)));

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn range_next_back_negative_step() {
    // 5..1 by -1 → values: 5, 4, 3, 2
    let iter = IteratorValue::from_range(5, Some(1), -1, false);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(2)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(4)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(5)));

    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn range_next_back_empty() {
    let iter = IteratorValue::from_range(5, Some(3), 1, false);
    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn range_interleaved_next_and_next_back() {
    // 0..5 by 1 → values: 0, 1, 2, 3, 4
    let iter = IteratorValue::from_range(0, Some(5), 1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(4)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));

    // Exhausted
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn range_interleaved_step() {
    // 0..10 by 3 → values: 0, 3, 6, 9
    let iter = IteratorValue::from_range(0, Some(10), 3, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(9)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::int(6)));

    // Exhausted
    let (val, _) = iter.next();
    assert_eq!(val, None);
}

#[test]
fn str_next_back_ascii() {
    let data = Heap::new(Cow::Borrowed("abc"));
    let iter = IteratorValue::from_string(data);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('c')));
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('b')));
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('a')));
    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn str_next_back_unicode() {
    let data = Heap::new(Cow::Borrowed("café"));
    let iter = IteratorValue::from_string(data);

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('é')));
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('f')));
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('a')));
    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('c')));
    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn str_next_back_empty() {
    let data = Heap::new(Cow::Borrowed(""));
    let iter = IteratorValue::from_string(data);
    let (val, _) = iter.next_back();
    assert_eq!(val, None);
}

#[test]
fn str_interleaved_next_and_next_back() {
    let data = Heap::new(Cow::Borrowed("abcde"));
    let iter = IteratorValue::from_string(data);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('a')));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('e')));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('b')));

    let (val, iter) = iter.next_back();
    assert_eq!(val, Some(Value::Char('d')));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::Char('c')));

    let (val, _) = iter.next();
    assert_eq!(val, None);
}

// is_double_ended tests

#[test]
fn is_double_ended_source_variants() {
    assert!(make_list_iter(&[1, 2]).is_double_ended());
    assert!(IteratorValue::from_range(0, Some(5), 1, false).is_double_ended());

    let data = Heap::new(Cow::Borrowed("abc"));
    assert!(IteratorValue::from_string(data).is_double_ended());

    // Map and Set are NOT double-ended
    let mut map = BTreeMap::new();
    map.insert("a".to_string(), Value::int(1));
    assert!(!IteratorValue::from_map(&map).is_double_ended());

    let set_items = Heap::new(vec![Value::int(1)]);
    assert!(!IteratorValue::from_set(set_items).is_double_ended());
}

#[test]
fn is_double_ended_adapters() {
    let de_source = make_list_iter(&[1, 2, 3]);
    let non_de_source = IteratorValue::from_set(Heap::new(vec![Value::int(1)]));

    // Mapped propagates
    let mapped_de = IteratorValue::Mapped {
        source: Box::new(de_source.clone()),
        transform: Box::new(Value::Bool(true)),
    };
    assert!(mapped_de.is_double_ended());

    let mapped_non_de = IteratorValue::Mapped {
        source: Box::new(non_de_source.clone()),
        transform: Box::new(Value::Bool(true)),
    };
    assert!(!mapped_non_de.is_double_ended());

    // Filtered propagates
    let filtered_de = IteratorValue::Filtered {
        source: Box::new(de_source),
        predicate: Box::new(Value::Bool(true)),
    };
    assert!(filtered_de.is_double_ended());

    // Other adapters are NOT double-ended
    assert!(!IteratorValue::TakeN {
        source: Box::new(make_list_iter(&[1])),
        remaining: 1,
    }
    .is_double_ended());

    assert!(!IteratorValue::SkipN {
        source: Box::new(make_list_iter(&[1])),
        remaining: 1,
    }
    .is_double_ended());

    assert!(!IteratorValue::Enumerated {
        source: Box::new(make_list_iter(&[1])),
        index: 0,
    }
    .is_double_ended());

    assert!(!IteratorValue::Zipped {
        left: Box::new(make_list_iter(&[1])),
        right: Box::new(make_list_iter(&[1])),
    }
    .is_double_ended());

    assert!(!IteratorValue::Chained {
        first: Box::new(make_list_iter(&[1])),
        second: Box::new(make_list_iter(&[1])),
        first_done: false,
    }
    .is_double_ended());

    assert!(!IteratorValue::Flattened {
        source: Box::new(make_list_iter(&[1])),
        inner: None,
    }
    .is_double_ended());

    assert!(!IteratorValue::Cycled {
        source: Some(Box::new(make_list_iter(&[1]))),
        buffer: Vec::new(),
        buf_pos: 0,
    }
    .is_double_ended());
}

// size_hint after next_back

#[test]
fn size_hint_after_next_back() {
    let iter = make_list_iter(&[1, 2, 3, 4, 5]);
    assert_eq!(iter.size_hint(), (5, Some(5)));

    let (_, iter) = iter.next_back();
    assert_eq!(iter.size_hint(), (4, Some(4)));

    let (_, iter) = iter.next();
    assert_eq!(iter.size_hint(), (3, Some(3)));

    let (_, iter) = iter.next_back();
    assert_eq!(iter.size_hint(), (2, Some(2)));
}

#[test]
fn size_hint_range_after_next_back() {
    let iter = IteratorValue::from_range(0, Some(10), 3, false); // 0, 3, 6, 9
    assert_eq!(iter.size_hint(), (4, Some(4)));

    let (_, iter) = iter.next_back(); // removes 9
    assert_eq!(iter.size_hint(), (3, Some(3)));
}

// ── Reversed variant ─────────────────────────────────────────────────

#[test]
fn reversed_is_double_ended() {
    let iter = make_list_iter(&[1, 2, 3]);
    let reversed = IteratorValue::Reversed {
        source: Box::new(iter),
    };
    assert!(reversed.is_double_ended());
}

#[test]
fn reversed_size_hint_delegates_to_source() {
    let iter = make_list_iter(&[1, 2, 3, 4, 5]);
    let reversed = IteratorValue::Reversed {
        source: Box::new(iter),
    };
    assert_eq!(reversed.size_hint(), (5, Some(5)));
}

#[test]
fn reversed_size_hint_range() {
    let iter = IteratorValue::from_range(0, Some(10), 2, false); // 0, 2, 4, 6, 8
    let reversed = IteratorValue::Reversed {
        source: Box::new(iter),
    };
    assert_eq!(reversed.size_hint(), (5, Some(5)));
}

#[test]
fn reversed_debug_format() {
    let iter = make_list_iter(&[1]);
    let reversed = IteratorValue::Reversed {
        source: Box::new(iter),
    };
    let debug = format!("{reversed:?}");
    assert!(debug.starts_with("ReversedIterator("));
}

#[test]
fn reversed_equality() {
    let iter_a = make_list_iter(&[1, 2, 3]);
    let iter_b = make_list_iter(&[1, 2, 3]);
    let rev_a = IteratorValue::Reversed {
        source: Box::new(iter_a),
    };
    let rev_b = IteratorValue::Reversed {
        source: Box::new(iter_b),
    };
    assert_eq!(rev_a, rev_b);
}

#[test]
fn reversed_inequality_different_source() {
    let rev_a = IteratorValue::Reversed {
        source: Box::new(make_list_iter(&[1, 2])),
    };
    let rev_b = IteratorValue::Reversed {
        source: Box::new(make_list_iter(&[3, 4])),
    };
    assert_ne!(rev_a, rev_b);
}

#[test]
fn reversed_hash_consistency() {
    let iter_a = make_list_iter(&[1, 2, 3]);
    let iter_b = make_list_iter(&[1, 2, 3]);
    let rev_a = IteratorValue::Reversed {
        source: Box::new(iter_a),
    };
    let rev_b = IteratorValue::Reversed {
        source: Box::new(iter_b),
    };
    let hash_a = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        rev_a.hash(&mut h);
        h.finish()
    };
    let hash_b = {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        rev_b.hash(&mut h);
        h.finish()
    };
    assert_eq!(hash_a, hash_b);
}

// ── Repeat variant tests ─────────────────────────────────────────────

#[test]
fn repeat_basic() {
    let iter = IteratorValue::from_repeat(Value::int(42));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(42)));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(42)));

    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::int(42)));
}

#[test]
fn repeat_string() {
    let iter = IteratorValue::from_repeat(Value::string("hello"));

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::string("hello")));

    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::string("hello")));
}

#[test]
fn repeat_never_exhausts() {
    let mut iter = IteratorValue::from_repeat(Value::int(7));
    for _ in 0..100 {
        let (val, next) = iter.next();
        assert_eq!(val, Some(Value::int(7)));
        iter = next;
    }
}

#[test]
fn repeat_not_double_ended() {
    let iter = IteratorValue::from_repeat(Value::int(1));
    assert!(!iter.is_double_ended());
}

#[test]
fn repeat_size_hint_infinite() {
    let iter = IteratorValue::from_repeat(Value::int(1));
    assert_eq!(iter.size_hint(), (usize::MAX, None));
}

#[test]
fn repeat_debug_format() {
    let iter = IteratorValue::from_repeat(Value::int(99));
    let debug = format!("{iter:?}");
    assert!(debug.starts_with("RepeatIterator("));
}

#[test]
fn repeat_equality() {
    let a = IteratorValue::from_repeat(Value::int(5));
    let b = IteratorValue::from_repeat(Value::int(5));
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn repeat_inequality_different_value() {
    let a = IteratorValue::from_repeat(Value::int(1));
    let b = IteratorValue::from_repeat(Value::int(2));
    assert_ne!(a, b);
}

#[test]
fn repeat_not_equal_to_other_variant() {
    let repeat = IteratorValue::from_repeat(Value::int(1));
    let list = make_list_iter(&[1]);
    assert_ne!(repeat, list);
}

// ── Unbounded range iterator tests ──────────────────────────────────

#[test]
fn unbounded_range_next_always_yields() {
    let iter = IteratorValue::from_range(0, None, 1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(1)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(2)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));
    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::int(4)));
}

#[test]
fn unbounded_range_never_exhausts() {
    let mut iter = IteratorValue::from_range(0, None, 1, false);
    for i in 0..100 {
        let (val, next) = iter.next();
        assert_eq!(val, Some(Value::int(i)));
        iter = next;
    }
}

#[test]
fn unbounded_range_with_step() {
    let iter = IteratorValue::from_range(0, None, 3, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(3)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(6)));
    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::int(9)));
}

#[test]
fn unbounded_range_negative_step() {
    let iter = IteratorValue::from_range(0, None, -1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(0)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(-1)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(-2)));
    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::int(-3)));
}

#[test]
fn unbounded_range_from_nonzero() {
    let iter = IteratorValue::from_range(100, None, 1, false);

    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(100)));
    let (val, iter) = iter.next();
    assert_eq!(val, Some(Value::int(101)));
    let (val, _) = iter.next();
    assert_eq!(val, Some(Value::int(102)));
}

#[test]
fn unbounded_range_not_double_ended() {
    let iter = IteratorValue::from_range(0, None, 1, false);
    assert!(!iter.is_double_ended());
}

#[test]
fn unbounded_range_size_hint() {
    let iter = IteratorValue::from_range(0, None, 1, false);
    assert_eq!(iter.size_hint(), (usize::MAX, None));
}

#[test]
fn unbounded_range_size_hint_with_step() {
    let iter = IteratorValue::from_range(0, None, 5, false);
    assert_eq!(iter.size_hint(), (usize::MAX, None));
}

#[test]
fn unbounded_range_debug_format() {
    let iter = IteratorValue::from_range(0, None, 1, false);
    let debug = format!("{iter:?}");
    assert!(debug.contains("RangeIterator(0..)"));
}

#[test]
fn unbounded_range_equality() {
    let a = IteratorValue::from_range(0, None, 1, false);
    let b = IteratorValue::from_range(0, None, 1, false);
    assert_eq!(a, b);
    assert_eq!(hash_of(&a), hash_of(&b));
}

#[test]
fn unbounded_range_inequality_different_start() {
    let a = IteratorValue::from_range(0, None, 1, false);
    let b = IteratorValue::from_range(5, None, 1, false);
    assert_ne!(a, b);
}

#[test]
fn unbounded_range_inequality_different_step() {
    let a = IteratorValue::from_range(0, None, 1, false);
    let b = IteratorValue::from_range(0, None, 2, false);
    assert_ne!(a, b);
}

#[test]
fn unbounded_vs_bounded_range_inequality() {
    let unbounded = IteratorValue::from_range(0, None, 1, false);
    let bounded = IteratorValue::from_range(0, Some(100), 1, false);
    assert_ne!(unbounded, bounded);
}

#[test]
fn from_value_unbounded_range() {
    let val = Value::Range(super::super::RangeValue {
        start: 0,
        end: None,
        step: 1,
        inclusive: false,
    });
    let Some(iter) = IteratorValue::from_value(&val) else {
        panic!("unbounded range should be iterable");
    };
    assert_eq!(iter.size_hint(), (usize::MAX, None));
    let (item, _) = iter.next();
    assert_eq!(item, Some(Value::int(0)));
}
