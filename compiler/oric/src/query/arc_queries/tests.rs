//! Tests for ARC borrow inference Salsa types.

use ori_arc::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};
use ori_arc::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use super::{ArcModuleInput, BorrowSigResult};
use crate::db::{CompilerDb, Db};

/// Create a minimal ArcFunction stub for testing.
///
/// Produces a valid function with one block that returns immediately.
fn stub_function(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::UNIT],
        var_reprs: vec![],
        spans: vec![],
        is_fbip: false,
    }
}

/// Create a simple AnnotatedSig for testing.
fn stub_sig(name: Name) -> AnnotatedSig {
    AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        return_type: Idx::INT,
    }
}

// ── BorrowSigResult tests ──────────────────────────────────────────

#[test]
fn borrow_sig_result_from_map_is_sorted() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_c = interner.intern("c_func");
    let name_a = interner.intern("a_func");
    let name_b = interner.intern("b_func");

    let mut map = FxHashMap::default();
    map.insert(name_c, stub_sig(name_c));
    map.insert(name_a, stub_sig(name_a));
    map.insert(name_b, stub_sig(name_b));

    let result = BorrowSigResult::from_map(map);

    // Verify sorted by Name (which is Ord on u32 index)
    let names: Vec<Name> = result.iter().map(|(n, _)| *n).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();
    assert_eq!(names, sorted_names, "from_map should produce sorted output");
}

#[test]
fn borrow_sig_result_get_finds_entry() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_x = interner.intern("func_x");
    let name_y = interner.intern("func_y");

    let sig_x = stub_sig(name_x);
    let sig_y = AnnotatedSig {
        params: vec![AnnotatedParam {
            name: name_y,
            ty: Idx::FLOAT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::FLOAT,
    };

    let mut map = FxHashMap::default();
    map.insert(name_x, sig_x.clone());
    map.insert(name_y, sig_y.clone());

    let result = BorrowSigResult::from_map(map);

    assert_eq!(result.get(name_x), Some(&sig_x));
    assert_eq!(result.get(name_y), Some(&sig_y));

    // Nonexistent name
    let name_z = interner.intern("func_z");
    assert_eq!(result.get(name_z), None);
}

#[test]
fn borrow_sig_result_eq_ignores_insertion_order() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("alpha");
    let name_b = interner.intern("beta");

    let sig_a = stub_sig(name_a);
    let sig_b = stub_sig(name_b);

    // Insert in order a, b
    let mut map1 = FxHashMap::default();
    map1.insert(name_a, sig_a.clone());
    map1.insert(name_b, sig_b.clone());

    // Insert in reverse order b, a
    let mut map2 = FxHashMap::default();
    map2.insert(name_b, sig_b);
    map2.insert(name_a, sig_a);

    let result1 = BorrowSigResult::from_map(map1);
    let result2 = BorrowSigResult::from_map(map2);

    assert_eq!(
        result1, result2,
        "same sigs in different insertion order should be equal"
    );
}

#[test]
fn borrow_sig_result_roundtrip_map() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("rta");
    let name_b = interner.intern("rtb");

    let mut map = FxHashMap::default();
    map.insert(name_a, stub_sig(name_a));
    map.insert(name_b, stub_sig(name_b));

    let result = BorrowSigResult::from_map(map.clone());
    let roundtripped = result.into_map();

    assert_eq!(roundtripped.len(), map.len());
    for (name, sig) in &map {
        assert_eq!(roundtripped.get(name), Some(sig));
    }
}

#[test]
fn borrow_sig_result_empty() {
    let result = BorrowSigResult::empty();
    assert!(result.is_empty());
    assert_eq!(result.len(), 0);
    assert_eq!(result.get(Name::EMPTY), None);
}

// ── ArcModuleInput tests ──────────────────────────────────────────

#[test]
fn arc_module_input_roundtrip() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_foo = interner.intern("foo");
    let name_bar = interner.intern("bar");

    let func_foo = stub_function(name_foo);
    let func_bar = stub_function(name_bar);

    let mut funcs_map = FxHashMap::default();
    funcs_map.insert(name_foo, func_foo.clone());
    funcs_map.insert(name_bar, func_bar.clone());

    let sorted_funcs = ArcModuleInput::sorted_functions(funcs_map);
    let module = ArcModuleInput::new(
        &db,
        std::path::PathBuf::from("/test/module.ori"),
        sorted_funcs,
    );

    // Read back fields
    let path = module.path(&db);
    assert_eq!(path.to_str().unwrap(), "/test/module.ori");

    let functions = module.functions(&db);
    assert_eq!(functions.len(), 2);

    // Verify functions are accessible
    let found_foo = module.get_function(&db, name_foo);
    assert!(found_foo.is_some());
    assert_eq!(found_foo.unwrap().name, name_foo);

    let found_bar = module.get_function(&db, name_bar);
    assert!(found_bar.is_some());
    assert_eq!(found_bar.unwrap().name, name_bar);

    // Nonexistent function
    let name_baz = interner.intern("baz");
    assert!(module.get_function(&db, name_baz).is_none());
}

#[test]
fn arc_module_input_sorted_functions_produces_sorted_output() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_z = interner.intern("z_func");
    let name_a = interner.intern("a_func");
    let name_m = interner.intern("m_func");

    let mut map = FxHashMap::default();
    map.insert(name_z, stub_function(name_z));
    map.insert(name_a, stub_function(name_a));
    map.insert(name_m, stub_function(name_m));

    let sorted = ArcModuleInput::sorted_functions(map);
    let names: Vec<Name> = sorted.iter().map(|(n, _)| *n).collect();
    let mut expected = names.clone();
    expected.sort();
    assert_eq!(
        names, expected,
        "sorted_functions should produce Name-sorted output"
    );
}

#[test]
fn arc_module_input_function_list() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("fl_a");
    let name_b = interner.intern("fl_b");

    let mut map = FxHashMap::default();
    map.insert(name_a, stub_function(name_a));
    map.insert(name_b, stub_function(name_b));

    let module = ArcModuleInput::new(
        &db,
        std::path::PathBuf::from("/test/list.ori"),
        ArcModuleInput::sorted_functions(map),
    );

    let list = module.function_list(&db);
    assert_eq!(list.len(), 2);
    // Verify the functions are valid ArcFunction instances
    for func in &list {
        assert_eq!(func.blocks.len(), 1);
        assert_eq!(func.return_type, Idx::UNIT);
    }
}
