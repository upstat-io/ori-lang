//! Tests for ARC borrow inference Salsa types and queries.

use ori_arc::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId};
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

// ── SccDecomposition tests ──────────────────────────────────────────

use super::arc_scc_decomposition;

/// Build a function that calls another function.
///
/// `fn caller(x: unit) -> unit { callee(x) }`.
fn calling_function(name: Name, callee: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(0),
                ty: Idx::UNIT,
                func: callee,
                args: vec![],
                arg_ownership: vec![],
            }],
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

/// Helper: build a module from a list of (name, function) pairs.
fn make_module(db: &CompilerDb, funcs: Vec<(Name, ArcFunction)>) -> ArcModuleInput {
    let mut sorted = funcs;
    sorted.sort_by_key(|(name, _)| *name);
    ArcModuleInput::new(db, std::path::PathBuf::from("/test/scc.ori"), sorted)
}

#[test]
fn scc_decomposition_single_function() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("scc_a");
    let func_a = stub_function(name_a);
    let module = make_module(&db, vec![(name_a, func_a)]);

    let decomp = arc_scc_decomposition(&db, module);

    assert_eq!(decomp.len(), 1);
    assert_eq!(decomp.scc_of(name_a), Some(0));
    assert!(!decomp.is_recursive(0));
}

#[test]
fn scc_decomposition_linear_chain() {
    // A→B→C: 3 separate SCCs in topological order (C first, then B, then A).
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("chain_a");
    let name_b = interner.intern("chain_b");
    let name_c = interner.intern("chain_c");

    let func_a = calling_function(name_a, name_b);
    let func_b = calling_function(name_b, name_c);
    let func_c = stub_function(name_c);

    let module = make_module(
        &db,
        vec![(name_a, func_a), (name_b, func_b), (name_c, func_c)],
    );
    let decomp = arc_scc_decomposition(&db, module);

    assert_eq!(decomp.len(), 3, "linear chain → 3 separate SCCs");

    // Each function has its own SCC.
    let scc_a = decomp.scc_of(name_a).unwrap();
    let scc_b = decomp.scc_of(name_b).unwrap();
    let scc_c = decomp.scc_of(name_c).unwrap();

    // All different SCCs.
    assert_ne!(scc_a, scc_b);
    assert_ne!(scc_b, scc_c);

    // Topological order: C before B before A.
    assert!(scc_c < scc_b, "C should come before B in topological order");
    assert!(scc_b < scc_a, "B should come before A in topological order");

    // None are recursive.
    assert!(!decomp.is_recursive(scc_a));
    assert!(!decomp.is_recursive(scc_b));
    assert!(!decomp.is_recursive(scc_c));
}

#[test]
fn scc_decomposition_mutual_recursion() {
    // A↔B: 1 recursive SCC with 2 members.
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("mut_a");
    let name_b = interner.intern("mut_b");

    let func_a = calling_function(name_a, name_b);
    let func_b = calling_function(name_b, name_a);

    let module = make_module(&db, vec![(name_a, func_a), (name_b, func_b)]);
    let decomp = arc_scc_decomposition(&db, module);

    assert_eq!(decomp.len(), 1, "mutual recursion → 1 SCC");

    let scc_a = decomp.scc_of(name_a).unwrap();
    let scc_b = decomp.scc_of(name_b).unwrap();
    assert_eq!(scc_a, scc_b, "both in same SCC");
    assert!(decomp.is_recursive(scc_a));
}

#[test]
fn scc_of_returns_correct_index() {
    let db = CompilerDb::new();
    let interner = db.interner();

    let names: Vec<Name> = (0..5)
        .map(|i| interner.intern(&format!("idx_{i}")))
        .collect();
    let funcs: Vec<(Name, ArcFunction)> = names.iter().map(|&n| (n, stub_function(n))).collect();
    let module = make_module(&db, funcs);
    let decomp = arc_scc_decomposition(&db, module);

    // Each function should map to a valid SCC index.
    for &name in &names {
        let idx = decomp.scc_of(name);
        assert!(idx.is_some(), "function should be in an SCC");
        assert!((idx.unwrap() as usize) < decomp.len());
    }

    // Non-existent function should return None.
    let unknown = interner.intern("unknown_func");
    assert_eq!(decomp.scc_of(unknown), None);
}

#[test]
fn scc_decomposition_eq_is_deterministic() {
    // Same module → same decomposition (deterministic).
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("det_a");
    let name_b = interner.intern("det_b");

    let funcs = vec![
        (name_a, calling_function(name_a, name_b)),
        (name_b, stub_function(name_b)),
    ];
    let module = make_module(&db, funcs);

    let d1 = arc_scc_decomposition(&db, module);
    let d2 = arc_scc_decomposition(&db, module);

    assert_eq!(d1, d2, "same input should produce equal decompositions");
}
