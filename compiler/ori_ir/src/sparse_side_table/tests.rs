use super::SparseSideTable;

#[test]
fn insert_keeps_entries_sorted_for_binary_search() {
    let mut table: SparseSideTable<u32, &str> = SparseSideTable::new();
    table.insert(30, "c");
    table.insert(10, "a");
    table.insert(20, "b");

    assert_eq!(table.get(10), Some(&"a"));
    assert_eq!(table.get(20), Some(&"b"));
    assert_eq!(table.get(30), Some(&"c"));
    let keys: Vec<u32> = table.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, vec![10, 20, 30]);
}

#[test]
fn get_returns_none_on_missing_key() {
    let mut table: SparseSideTable<u32, i32> = SparseSideTable::new();
    table.insert(5, 50);
    assert_eq!(table.get(6), None);
    assert!(!table.contains_key(6));
    assert!(table.contains_key(5));
}

#[test]
fn insert_overwrites_on_duplicate_key() {
    let mut table: SparseSideTable<u32, i32> = SparseSideTable::new();
    table.insert(7, 1);
    table.insert(7, 2);
    assert_eq!(table.get(7), Some(&2));
    assert_eq!(table.len(), 1);
}

#[test]
fn from_unsorted_sorts_once_for_lookup() {
    let table = SparseSideTable::from_unsorted(vec![(9u32, "i"), (3, "c"), (6, "f")]);
    assert_eq!(table.get(3), Some(&"c"));
    assert_eq!(table.get(6), Some(&"f"));
    assert_eq!(table.get(9), Some(&"i"));
    let keys: Vec<u32> = table.iter().map(|(k, _)| *k).collect();
    assert_eq!(keys, vec![3, 6, 9]);
}

#[test]
fn empty_table_reports_empty_and_misses() {
    let table: SparseSideTable<u32, i32> = SparseSideTable::new();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
    assert_eq!(table.get(0), None);
}

#[test]
fn equal_tables_compare_equal_regardless_of_insert_order() {
    let mut a: SparseSideTable<u32, i32> = SparseSideTable::new();
    a.insert(2, 20);
    a.insert(1, 10);
    let b = SparseSideTable::from_unsorted(vec![(1u32, 10), (2, 20)]);
    assert_eq!(a, b);
}
