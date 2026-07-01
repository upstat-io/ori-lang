use super::*;

#[test]
fn test_intern_and_lookup() {
    let interner = StringInterner::new();

    let hello = interner.intern("hello");
    let world = interner.intern("world");
    let hello2 = interner.intern("hello");

    assert_eq!(hello, hello2);
    assert_ne!(hello, world);

    assert_eq!(interner.lookup(hello), "hello");
    assert_eq!(interner.lookup(world), "world");
}

#[test]
fn test_empty_string() {
    let interner = StringInterner::new();
    let empty = interner.intern("");
    assert_eq!(empty, Name::EMPTY);
    assert_eq!(interner.lookup(Name::EMPTY), "");
}

#[test]
fn test_keywords_pre_interned() {
    let interner = StringInterner::new();

    let if_name = interner.intern("if");
    let else_name = interner.intern("else");

    assert_eq!(interner.lookup(if_name), "if");
    assert_eq!(interner.lookup(else_name), "else");
}

#[test]
fn test_shared_interner() {
    let interner = SharedInterner::new();
    let interner2 = interner.clone();

    let name1 = interner.intern("shared");
    let name2 = interner2.intern("shared");

    assert_eq!(name1, name2);
}

#[test]
fn test_intern_owned() {
    let interner = StringInterner::new();

    // Intern an owned string
    let owned = String::from("owned_string");
    let name1 = interner.intern_owned(owned);

    // Should return same Name for equivalent string
    let name2 = interner.intern("owned_string");
    assert_eq!(name1, name2);

    assert_eq!(interner.lookup(name1), "owned_string");
}

#[test]
fn test_intern_owned_already_interned() {
    let interner = StringInterner::new();

    // First intern via reference
    let name1 = interner.intern("test_string");

    // Then intern owned - should return same Name
    let owned = String::from("test_string");
    let name2 = interner.intern_owned(owned);

    assert_eq!(name1, name2);
}

#[test]
fn test_lookup_static_after_chunk_growth() {
    let interner = StringInterner::new();
    let mut names = Vec::new();

    for i in 0..128 {
        let value = format!("long_identifier_{i}_{}", "x".repeat(128));
        let name = interner.intern_owned(value.clone());
        names.push((name, value));
    }

    for (name, value) in names {
        assert_eq!(interner.lookup(name), value);
        assert_eq!(interner.lookup_static(name), value);
    }
}

#[test]
fn test_lookup_static_does_not_depend_on_map_probe() {
    let interner = StringInterner::new();
    let name = interner.intern("span_indexed_lookup");

    {
        let mut shard = interner.shards[name.shard()].write();
        assert!(shard.remove_map_entry_for_test("span_indexed_lookup"));
    }

    assert_eq!(interner.lookup(name), "span_indexed_lookup");
    assert_eq!(interner.lookup_static(name), "span_indexed_lookup");
}

#[test]
fn test_lookup_does_not_hold_shard_lock() {
    let interner = StringInterner::new();
    let name = interner.intern("chunk_pool_lookup");

    let value = interner.lookup(name);
    let guard = interner.shards[name.shard()].try_write();

    assert!(guard.is_some(), "lookup must not retain a shard read lock");
    assert_eq!(value, "chunk_pool_lookup");
}

#[test]
fn test_try_lookup_does_not_hold_shard_lock() {
    let interner = StringInterner::new();
    let name = interner.intern("try_chunk_pool_lookup");

    let value = interner.try_lookup(name);
    let guard = interner.shards[name.shard()].try_write();

    assert!(
        guard.is_some(),
        "try_lookup must not retain a shard read lock"
    );
    assert_eq!(value, Some("try_chunk_pool_lookup"));
}

#[test]
fn test_try_lookup_unknown_local_returns_none() {
    let interner = StringInterner::new();
    assert!(interner.try_lookup(Name::new(0, Name::MAX_LOCAL)).is_none());
}
