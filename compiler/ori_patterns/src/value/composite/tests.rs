use super::*;
use ori_ir::ExprArena;

fn dummy_arena() -> SharedArena {
    SharedArena::new(ExprArena::new())
}

#[test]
fn test_range_exclusive() {
    let range = RangeValue::exclusive(0, 5);
    let values: Vec<_> = range.iter().collect();
    assert_eq!(values, vec![0, 1, 2, 3, 4]);
    assert_eq!(range.len(), 5);
    assert!(range.contains(0));
    assert!(range.contains(4));
    assert!(!range.contains(5));
}

#[test]
fn test_range_inclusive() {
    let range = RangeValue::inclusive(0, 5);
    let values: Vec<_> = range.iter().collect();
    assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
    assert_eq!(range.len(), 6);
    assert!(range.contains(5));
}

#[test]
fn test_function_value_new() {
    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    assert!(func.params.is_empty());
    assert!(!func.has_captures());
}

#[test]
fn test_function_value_with_captures() {
    let mut captures = FxHashMap::default();
    captures.insert(Name::new(0, 1), Value::int(42));
    let func = FunctionValue::new(vec![], captures, dummy_arena());
    assert!(func.has_captures());
    assert_eq!(func.get_capture(Name::new(0, 1)), Some(&Value::int(42)));
}

// Edge case tests for None cases

#[test]
fn test_struct_layout_get_index_missing_field() {
    let field_names = vec![Name::new(0, 1), Name::new(0, 2)];
    let layout = StructLayout::new(&field_names);
    // Query a field that doesn't exist
    let missing_field = Name::new(0, 999);
    assert_eq!(layout.get_index(missing_field), None);
}

#[test]
fn test_struct_layout_get_index_existing_field() {
    let field_a = Name::new(0, 1);
    let field_b = Name::new(0, 2);
    let layout = StructLayout::new(&[field_a, field_b]);
    assert!(layout.get_index(field_a).is_some());
    assert!(layout.get_index(field_b).is_some());
}

#[test]
fn test_struct_value_get_field_missing() {
    let type_name = Name::new(0, 100);
    let field_a = Name::new(0, 1);
    let mut fields = FxHashMap::default();
    fields.insert(field_a, Value::int(42));
    let sv = StructValue::new(type_name, fields);

    // Query a field that doesn't exist
    let missing_field = Name::new(0, 999);
    assert_eq!(sv.get_field(missing_field), None);
}

#[test]
fn test_struct_value_get_field_existing() {
    let type_name = Name::new(0, 100);
    let field_a = Name::new(0, 1);
    let mut fields = FxHashMap::default();
    fields.insert(field_a, Value::int(42));
    let sv = StructValue::new(type_name, fields);

    assert_eq!(sv.get_field(field_a), Some(&Value::int(42)));
}

#[test]
fn test_function_value_get_capture_missing() {
    let mut captures = FxHashMap::default();
    captures.insert(Name::new(0, 1), Value::int(42));
    let func = FunctionValue::new(vec![], captures, dummy_arena());

    // Query a capture that doesn't exist
    let missing_name = Name::new(0, 999);
    assert_eq!(func.get_capture(missing_name), None);
}

#[test]
fn test_memoized_function_get_cached_uncached() {
    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    let memoized = MemoizedFunctionValue::new(func);

    // Query with args that haven't been cached
    let args = vec![Value::int(1), Value::int(2)];
    assert_eq!(memoized.get_cached(&args), None);
}

#[test]
fn test_memoized_function_cache_and_retrieve() {
    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    let memoized = MemoizedFunctionValue::new(func);

    // Cache a result
    let args = vec![Value::int(1), Value::int(2)];
    let result = Value::int(3);
    memoized.cache_result(&args, result.clone());

    // Retrieve it
    assert_eq!(memoized.get_cached(&args), Some(result));
    assert_eq!(memoized.cache_size(), 1);
}

#[test]
fn test_memoized_function_different_args_not_cached() {
    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    let memoized = MemoizedFunctionValue::new(func);

    // Cache with one set of args
    let args1 = vec![Value::int(1)];
    memoized.cache_result(&args1, Value::int(10));

    // Query with different args
    let args2 = vec![Value::int(2)];
    assert_eq!(memoized.get_cached(&args2), None);
}

#[test]
fn test_memoized_function_cache_eviction() {
    use super::MAX_MEMO_CACHE_SIZE;

    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    let memoized = MemoizedFunctionValue::new(func);

    // Fill the cache to capacity
    for i in 0..MAX_MEMO_CACHE_SIZE {
        let args = vec![Value::int(i as i64)];
        memoized.cache_result(&args, Value::int(i as i64 * 10));
    }
    assert_eq!(memoized.cache_size(), MAX_MEMO_CACHE_SIZE);

    // Verify first entry is still present
    assert_eq!(memoized.get_cached(&[Value::int(0)]), Some(Value::int(0)));

    // Add one more entry - should evict the oldest (key 0)
    let new_args = vec![Value::int(MAX_MEMO_CACHE_SIZE as i64)];
    memoized.cache_result(&new_args, Value::int(999));

    // Size should still be at capacity
    assert_eq!(memoized.cache_size(), MAX_MEMO_CACHE_SIZE);

    // First entry should be evicted
    assert_eq!(memoized.get_cached(&[Value::int(0)]), None);

    // New entry should be present
    assert_eq!(
        memoized.get_cached(&[Value::int(MAX_MEMO_CACHE_SIZE as i64)]),
        Some(Value::int(999))
    );

    // Entry 1 (second oldest) should still be present
    assert_eq!(memoized.get_cached(&[Value::int(1)]), Some(Value::int(10)));
}

#[test]
fn test_memoized_function_cache_update_no_eviction() {
    let func = FunctionValue::new(vec![], FxHashMap::default(), dummy_arena());
    let memoized = MemoizedFunctionValue::new(func);

    // Cache initial value
    let args = vec![Value::int(42)];
    memoized.cache_result(&args, Value::int(100));
    assert_eq!(memoized.cache_size(), 1);

    // Update same key - should not increase size or cause eviction
    memoized.cache_result(&args, Value::int(200));
    assert_eq!(memoized.cache_size(), 1);
    assert_eq!(memoized.get_cached(&args), Some(Value::int(200)));
}

// Unbounded range tests

#[test]
fn test_range_unbounded() {
    let range = RangeValue::unbounded(0);
    assert!(range.is_unbounded());
    assert_eq!(range.start, 0);
    assert_eq!(range.end, None);
    assert_eq!(range.step, 1);
    assert!(!range.inclusive);
}

#[test]
fn test_range_unbounded_with_step() {
    let range = RangeValue::unbounded_with_step(10, 3);
    assert!(range.is_unbounded());
    assert_eq!(range.start, 10);
    assert_eq!(range.end, None);
    assert_eq!(range.step, 3);
}

#[test]
fn test_range_unbounded_negative_step() {
    let range = RangeValue::unbounded_with_step(0, -1);
    assert!(range.is_unbounded());
    assert_eq!(range.step, -1);
}

#[test]
fn test_range_unbounded_len() {
    let range = RangeValue::unbounded(0);
    assert_eq!(range.len(), usize::MAX);
}

#[test]
fn test_range_unbounded_is_empty() {
    // Unbounded ranges are never empty (step != 0)
    let range = RangeValue::unbounded(0);
    assert!(!range.is_empty());

    let range = RangeValue::unbounded_with_step(100, -2);
    assert!(!range.is_empty());
}

#[test]
fn test_range_unbounded_contains_positive_step() {
    let range = RangeValue::unbounded(0);
    assert!(range.contains(0));
    assert!(range.contains(1));
    assert!(range.contains(100));
    assert!(!range.contains(-1)); // before start
}

#[test]
fn test_range_unbounded_contains_step_alignment() {
    let range = RangeValue::unbounded_with_step(0, 3);
    assert!(range.contains(0));
    assert!(range.contains(3));
    assert!(range.contains(6));
    assert!(!range.contains(1)); // not aligned to step
    assert!(!range.contains(2));
    assert!(!range.contains(-3)); // before start
}

#[test]
fn test_range_unbounded_contains_negative_step() {
    let range = RangeValue::unbounded_with_step(10, -1);
    assert!(range.contains(10));
    assert!(range.contains(9));
    assert!(range.contains(0));
    assert!(range.contains(-100));
    assert!(!range.contains(11)); // above start with negative step
}

#[test]
fn test_range_unbounded_contains_from_nonzero() {
    let range = RangeValue::unbounded(5);
    assert!(range.contains(5));
    assert!(range.contains(6));
    assert!(range.contains(1000));
    assert!(!range.contains(4)); // before start
}

#[test]
fn test_range_unbounded_iter() {
    let range = RangeValue::unbounded(0);
    let first_5: Vec<_> = range.iter().take(5).collect();
    assert_eq!(first_5, vec![0, 1, 2, 3, 4]);
}

#[test]
fn test_range_unbounded_iter_with_step() {
    let range = RangeValue::unbounded_with_step(0, 2);
    let first_5: Vec<_> = range.iter().take(5).collect();
    assert_eq!(first_5, vec![0, 2, 4, 6, 8]);
}

#[test]
fn test_range_unbounded_iter_negative_step() {
    let range = RangeValue::unbounded_with_step(0, -1);
    let first_5: Vec<_> = range.iter().take(5).collect();
    assert_eq!(first_5, vec![0, -1, -2, -3, -4]);
}

#[test]
fn test_range_unbounded_iter_from_nonzero() {
    let range = RangeValue::unbounded(100);
    let first_3: Vec<_> = range.iter().take(3).collect();
    assert_eq!(first_3, vec![100, 101, 102]);
}

#[test]
fn test_range_bounded_is_not_unbounded() {
    let range = RangeValue::exclusive(0, 5);
    assert!(!range.is_unbounded());
    let range = RangeValue::inclusive(0, 5);
    assert!(!range.is_unbounded());
}
