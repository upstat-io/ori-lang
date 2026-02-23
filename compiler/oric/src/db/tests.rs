use super::*;

#[test]
fn test_db_creation() {
    let _db = CompilerDb::new();
    // If this compiles and runs, Salsa is working
}

#[test]
fn test_db_clone() {
    let db1 = CompilerDb::new();
    let _db2 = db1.clone();
    // Clone must work for Salsa
}

#[test]
fn test_db_default() {
    let _db = CompilerDb::default();
}

#[test]
fn test_is_stdlib_path() {
    // Native-separator paths (these work on the current platform)
    assert!(is_stdlib_path(Path::new(
        "/home/user/ori/library/std/prelude.ori"
    )));
    assert!(is_stdlib_path(Path::new(
        "/home/user/ori/library/std/io.ori"
    )));

    // User files are NOT stdlib
    assert!(!is_stdlib_path(Path::new("/home/user/project/main.ori")));
    assert!(!is_stdlib_path(Path::new("/home/user/project/src/lib.ori")));
}

#[test]
#[cfg(target_os = "windows")]
fn test_is_stdlib_path_windows() {
    assert!(is_stdlib_path(Path::new(
        "C:\\Users\\user\\ori\\library\\std\\prelude.ori"
    )));
}

// --- BorrowSigCache tests (LLVM feature only) ---

#[cfg(feature = "llvm")]
mod borrow_sig_cache {
    use super::*;
    use ori_arc::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
    use ori_types::Idx;
    use rustc_hash::FxHashMap;

    /// Build a test sigs map: function name → AnnotatedSig.
    fn make_sigs(
        entries: &[(&str, &[Ownership])],
    ) -> (
        crate::ir::StringInterner,
        FxHashMap<ori_ir::Name, AnnotatedSig>,
    ) {
        let interner = crate::ir::StringInterner::new();
        let mut map = FxHashMap::default();
        for &(name, ownerships) in entries {
            let sig = AnnotatedSig {
                params: ownerships
                    .iter()
                    .enumerate()
                    .map(|(i, &ownership)| AnnotatedParam {
                        name: interner.intern(&format!("p{i}")),
                        ty: Idx::from_raw(0),
                        ownership,
                    })
                    .collect(),
                return_type: Idx::from_raw(0),
            };
            map.insert(interner.intern(name), sig);
        }
        (interner, map)
    }

    #[test]
    fn store_and_retrieve() {
        let cache = BorrowSigCache::default();
        let path = Path::new("/tmp/test.ori");
        let (_interner, sigs) = make_sigs(&[("foo", &[Ownership::Borrowed, Ownership::Owned])]);

        cache.store(path, sigs.clone());
        let cached = cache.get(path).expect("should hit cache");
        assert_eq!(*cached, sigs);
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = BorrowSigCache::default();
        assert!(cache.get(Path::new("/tmp/nonexistent.ori")).is_none());
    }

    #[test]
    fn invalidate_clears_entry() {
        let cache = BorrowSigCache::default();
        let path = Path::new("/tmp/test.ori");
        let (_interner, sigs) = make_sigs(&[("bar", &[Ownership::Owned])]);

        cache.store(path, sigs);
        assert!(cache.get(path).is_some());

        cache.invalidate(path);
        assert!(cache.get(path).is_none());
    }

    #[test]
    fn invalidate_nonexistent_is_noop() {
        let cache = BorrowSigCache::default();
        // Should not panic
        cache.invalidate(Path::new("/tmp/nonexistent.ori"));
    }

    #[test]
    fn separate_paths_independent() {
        let cache = BorrowSigCache::default();
        let path_a = Path::new("/tmp/a.ori");
        let path_b = Path::new("/tmp/b.ori");

        let (_int_a, sigs_a) = make_sigs(&[("f", &[Ownership::Borrowed])]);
        let (_int_b, sigs_b) = make_sigs(&[("g", &[Ownership::Owned, Ownership::Owned])]);

        cache.store(path_a, sigs_a.clone());
        cache.store(path_b, sigs_b.clone());

        assert_eq!(*cache.get(path_a).unwrap(), sigs_a);
        assert_eq!(*cache.get(path_b).unwrap(), sigs_b);

        // Invalidate one, other persists
        cache.invalidate(path_a);
        assert!(cache.get(path_a).is_none());
        assert!(cache.get(path_b).is_some());
    }

    #[test]
    fn overwrite_replaces_previous() {
        let cache = BorrowSigCache::default();
        let path = Path::new("/tmp/test.ori");

        let (_int1, sigs_v1) = make_sigs(&[("f", &[Ownership::Owned])]);
        let (_int2, sigs_v2) = make_sigs(&[("f", &[Ownership::Borrowed])]);

        cache.store(path, sigs_v1);
        cache.store(path, sigs_v2.clone());

        let cached = cache.get(path).unwrap();
        assert_eq!(*cached, sigs_v2);
    }

    #[test]
    fn db_accessor_returns_cache() {
        let db = CompilerDb::new();
        let cache = db.borrow_sig_cache();
        let path = Path::new("/tmp/test.ori");

        // Should start empty
        assert!(cache.get(path).is_none());

        // Store via accessor, retrieve via accessor
        let (_interner, sigs) = make_sigs(&[("main", &[])]);
        cache.store(path, sigs.clone());
        assert_eq!(*db.borrow_sig_cache().get(path).unwrap(), sigs);
    }

    #[test]
    fn cloned_db_shares_cache() {
        let db1 = CompilerDb::new();
        let path = Path::new("/tmp/test.ori");
        let (_interner, sigs) = make_sigs(&[("f", &[Ownership::Borrowed])]);

        db1.borrow_sig_cache().store(path, sigs.clone());

        // Clone shares the underlying Arc<RwLock<...>>
        let db2 = db1.clone();
        assert_eq!(*db2.borrow_sig_cache().get(path).unwrap(), sigs);
    }
}
