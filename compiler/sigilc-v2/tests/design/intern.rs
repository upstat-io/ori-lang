//! Design Tests: String and Type Interning
//!
//! These tests validate the interning architecture from the design plan:
//! - Phase 1: String Interner with 16-shard design
//! - Phase 2: Type Interner with pre-interned primitives
//!
//! Reference: docs/compiler-design/v2/specs/A-data-structures.md

use sigilc_v2::intern::{Name, StringInterner, TypeId, TypeInterner, TypeKind};

// TypeId has the primitive constants, not TypeInterner
// TypeId::INT, TypeId::FLOAT, etc.

// =============================================================================
// String Interner Design Contracts
// =============================================================================

/// Design: Name(u32) is a 32-bit handle
#[test]
fn design_name_is_32_bits() {
    assert_eq!(std::mem::size_of::<Name>(), 4);
}

/// Design: Interning the same string twice returns the same Name
#[test]
fn design_intern_idempotent() {
    let interner = StringInterner::new();
    let name1 = interner.intern("hello");
    let name2 = interner.intern("hello");
    assert_eq!(name1, name2);
}

/// Design: Different strings get different Names
#[test]
fn design_different_strings_different_names() {
    let interner = StringInterner::new();
    let name1 = interner.intern("hello");
    let name2 = interner.intern("world");
    assert_ne!(name1, name2);
}

/// Design: Lookup returns the original string
#[test]
fn design_lookup_returns_original() {
    let interner = StringInterner::new();
    let name = interner.intern("test_string");
    assert_eq!(interner.lookup(name), "test_string");
}

/// Design: Keywords are pre-interned at known indices
#[test]
fn design_keywords_pre_interned() {
    let interner = StringInterner::new();

    // Design: Common keywords should be available immediately
    let if_name = interner.intern("if");
    let else_name = interner.intern("else");
    let let_name = interner.intern("let");

    // Verify they're valid
    assert_eq!(interner.lookup(if_name), "if");
    assert_eq!(interner.lookup(else_name), "else");
    assert_eq!(interner.lookup(let_name), "let");
}

/// Design: Empty string is a valid interned value
#[test]
fn design_empty_string_valid() {
    let interner = StringInterner::new();
    let empty = interner.intern("");
    assert_eq!(interner.lookup(empty), "");
}

/// Design: Unicode strings are supported
#[test]
fn design_unicode_support() {
    let interner = StringInterner::new();
    let name = interner.intern("λ→∀");
    assert_eq!(interner.lookup(name), "λ→∀");
}

/// Design: Concurrent interning is safe (16-shard design)
#[test]
fn design_concurrent_interning() {
    use std::sync::Arc;
    use std::thread;

    let interner = Arc::new(StringInterner::new());
    let mut handles = vec![];

    for i in 0..16 {
        let interner = Arc::clone(&interner);
        handles.push(thread::spawn(move || {
            let name = interner.intern(&format!("thread_{}", i));
            (i, name)
        }));
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    // Verify all interned correctly
    for (i, name) in results {
        assert_eq!(interner.lookup(name), format!("thread_{}", i));
    }
}

// =============================================================================
// Type Interner Design Contracts
// =============================================================================

/// Design: TypeId(u32) is a 32-bit handle
#[test]
fn design_typeid_is_32_bits() {
    assert_eq!(std::mem::size_of::<TypeId>(), 4);
}

/// Design: Primitives are pre-interned at known indices
#[test]
fn design_primitives_pre_interned() {
    // Design: INT=0, FLOAT=1, BOOL=2, STR=3, CHAR=4, BYTE=5, VOID=6
    assert_eq!(TypeId::INT.index(), 0);
    assert_eq!(TypeId::FLOAT.index(), 1);
    assert_eq!(TypeId::BOOL.index(), 2);
    assert_eq!(TypeId::STR.index(), 3);
    assert_eq!(TypeId::CHAR.index(), 4);
    assert_eq!(TypeId::BYTE.index(), 5);
    assert_eq!(TypeId::VOID.index(), 6);
}

/// Design: Type interning is idempotent
#[test]
fn design_type_intern_idempotent() {
    let interner = TypeInterner::new();
    let t1 = interner.intern_list(TypeId::INT);
    let t2 = interner.intern_list(TypeId::INT);
    assert_eq!(t1, t2);
}

/// Design: Complex types can be interned
#[test]
fn design_complex_types() {
    let interner = TypeInterner::new();

    // [int]
    let list_int = interner.intern_list(TypeId::INT);

    // [[int]]
    let list_list_int = interner.intern_list(list_int);

    // Verify they're different
    assert_ne!(list_int, list_list_int);
}

/// Design: Function types are supported
#[test]
fn design_function_types() {
    let interner = TypeInterner::new();

    // (int, int) -> int
    let fn_type = interner.intern_function(&[TypeId::INT, TypeId::INT], TypeId::INT);

    // Lookup should work
    match interner.lookup(fn_type) {
        Some(TypeKind::Function { params, ret }) => {
            let param_types = interner.get_list(params);
            assert_eq!(param_types.len(), 2);
            assert_eq!(ret, TypeId::INT);
        }
        _ => panic!("Expected function type"),
    }
}

/// Design: Option types are supported
#[test]
fn design_option_types() {
    let interner = TypeInterner::new();

    // Option<int>
    let opt_int = interner.intern_option(TypeId::INT);

    // Verify lookup works
    assert!(interner.lookup(opt_int).is_some());
}

/// Design: Result types are supported
#[test]
fn design_result_types() {
    let interner = TypeInterner::new();

    // Result<int, str>
    let result_type = interner.intern_result(TypeId::INT, TypeId::STR);

    // Verify lookup works
    assert!(interner.lookup(result_type).is_some());
}
