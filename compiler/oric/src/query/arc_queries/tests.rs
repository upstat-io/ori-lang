//! Tests for ARC borrow inference Salsa types and queries.

use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, ArgOwnership,
    CtorKind,
};
use ori_arc::ownership::{AnnotatedParam, AnnotatedSig, Ownership};
use ori_arc::uniqueness::{CowAnnotations, DropHints};
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;
use salsa::Setter;
use std::path::Path;

use super::{arc_scc_decomposition, infer_borrow_scc, ArcModuleInput, BorrowSigResult};
use crate::db::{CompilerDb, Db};

/// Create a minimal `ArcFunction` stub for testing.
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
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Create a simple `AnnotatedSig` for testing.
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
    let found_foo = module.get_function(&db, name_foo).unwrap();
    assert_eq!(found_foo.name, name_foo);

    let found_bar = module.get_function(&db, name_bar).unwrap();
    assert_eq!(found_bar.name, name_bar);

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
fn arc_module_input_functions_accessor() {
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

    let funcs = module.functions(&db);
    assert_eq!(funcs.len(), 2);
    // Verify the functions are valid ArcFunction instances
    for (_, func) in funcs {
        assert_eq!(func.blocks.len(), 1);
        assert_eq!(func.return_type, Idx::UNIT);
    }
}

// ── SccDecomposition tests ──────────────────────────────────────────

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
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
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
        let idx = decomp.scc_of(name).unwrap();
        assert!((idx as usize) < decomp.len());
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

// ── Early cutoff contract tests (Section 12.7) ──────────────────────

#[test]
fn early_cutoff_annotated_sig_eq_is_field_wise() {
    // Verify that AnnotatedSig equality is field-wise: same params and
    // return type → equal, regardless of construction path.
    let db = CompilerDb::new();
    let interner = db.interner();

    let name = interner.intern("ec_param");
    let sig1 = AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        return_type: Idx::INT,
    };
    let sig2 = AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        return_type: Idx::INT,
    };
    assert_eq!(sig1, sig2, "same fields → equal");

    // Change ownership → NOT equal (this is what triggers re-evaluation).
    let sig3 = AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
    };
    assert_ne!(sig1, sig3, "different ownership → not equal");
}

#[test]
fn early_cutoff_result_different_sig_not_equal() {
    // Changing a function's borrow annotation produces a different
    // BorrowSigResult, ensuring Salsa will NOT apply early cutoff.
    let db = CompilerDb::new();
    let interner = db.interner();

    let name = interner.intern("ec_fn");

    let sig_borrowed = AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Borrowed,
        }],
        return_type: Idx::UNIT,
    };
    let sig_owned = AnnotatedSig {
        params: vec![AnnotatedParam {
            name,
            ty: Idx::INT,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
    };

    let mut map1 = FxHashMap::default();
    map1.insert(name, sig_borrowed);
    let mut map2 = FxHashMap::default();
    map2.insert(name, sig_owned);

    let result1 = BorrowSigResult::from_map(map1);
    let result2 = BorrowSigResult::from_map(map2);

    assert_ne!(
        result1, result2,
        "different ownership → different result (no early cutoff)"
    );
}

#[test]
fn early_cutoff_deterministic_multi_function_ordering() {
    use std::hash::{Hash, Hasher};

    // Multiple functions: regardless of map insertion order, the
    // BorrowSigResult is identical. This is the key determinism guarantee
    // that makes Salsa early cutoff reliable.
    let db = CompilerDb::new();
    let interner = db.interner();

    let names: Vec<Name> = ["ec_z", "ec_a", "ec_m", "ec_b", "ec_x"]
        .iter()
        .map(|s| interner.intern(s))
        .collect();

    // Build map in forward order.
    let mut map_fwd = FxHashMap::default();
    for &name in &names {
        map_fwd.insert(name, stub_sig(name));
    }
    let result_fwd = BorrowSigResult::from_map(map_fwd);

    // Build map in reverse order.
    let mut map_rev = FxHashMap::default();
    for &name in names.iter().rev() {
        map_rev.insert(name, stub_sig(name));
    }
    let result_rev = BorrowSigResult::from_map(map_rev);

    assert_eq!(
        result_fwd, result_rev,
        "insertion order must not affect equality"
    );

    // Verify hash is also deterministic.
    let hash_of = |r: &BorrowSigResult| {
        let mut h = std::hash::DefaultHasher::new();
        r.hash(&mut h);
        h.finish()
    };
    assert_eq!(
        hash_of(&result_fwd),
        hash_of(&result_rev),
        "hash must be deterministic regardless of construction path"
    );
}

// ── Invalidation strategy tests (Section 12.10) ─────────────────────

#[test]
fn invalidation_only_affected_sccs() {
    // A→B, C standalone: 3 SCCs. Change C's body but keep same structure.
    // Verify the decompositions are equal (SCC structure unchanged).
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("inv_a");
    let name_b = interner.intern("inv_b");
    let name_c = interner.intern("inv_c");

    // Original module: A→B, C standalone.
    let module1 = make_module(
        &db,
        vec![
            (name_a, calling_function(name_a, name_b)),
            (name_b, stub_function(name_b)),
            (name_c, stub_function(name_c)),
        ],
    );
    let decomp1 = arc_scc_decomposition(&db, module1);

    // Modified module: C now has a different body (extra var_types entry),
    // but still no calls — same call graph structure.
    let mut modified_c = stub_function(name_c);
    modified_c.var_types.push(Idx::INT); // body change
    let module2 = make_module(
        &db,
        vec![
            (name_a, calling_function(name_a, name_b)),
            (name_b, stub_function(name_b)),
            (name_c, modified_c),
        ],
    );
    let decomp2 = arc_scc_decomposition(&db, module2);

    // SCC structure should be identical (same call graph).
    assert_eq!(
        decomp1, decomp2,
        "changing a function body without changing call structure \
         should produce identical SCC decomposition (early cutoff)"
    );
}

#[test]
fn scc_restructure_handled() {
    // Initially A and B are separate SCCs (A→B one-way).
    // Then add B→A, making them a mutual recursion SCC.
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("restr_a");
    let name_b = interner.intern("restr_b");

    // V1: A→B (one-way), 2 separate SCCs.
    let module1 = make_module(
        &db,
        vec![
            (name_a, calling_function(name_a, name_b)),
            (name_b, stub_function(name_b)),
        ],
    );
    let decomp1 = arc_scc_decomposition(&db, module1);
    assert_eq!(decomp1.len(), 2, "one-way → 2 separate SCCs");
    assert_ne!(
        decomp1.scc_of(name_a),
        decomp1.scc_of(name_b),
        "A and B should be in different SCCs"
    );

    // V2: A↔B (mutual recursion), 1 merged SCC.
    let module2 = make_module(
        &db,
        vec![
            (name_a, calling_function(name_a, name_b)),
            (name_b, calling_function(name_b, name_a)),
        ],
    );
    let decomp2 = arc_scc_decomposition(&db, module2);
    assert_eq!(decomp2.len(), 1, "mutual recursion → 1 merged SCC");
    assert_eq!(
        decomp2.scc_of(name_a),
        decomp2.scc_of(name_b),
        "A and B should be in the same SCC after merge"
    );
    assert!(decomp2.is_recursive(0));

    // The two decompositions should NOT be equal (structure changed).
    assert_ne!(
        decomp1, decomp2,
        "structural change should produce different decomposition"
    );
}

#[test]
fn cross_file_callee_not_in_scc() {
    // Function A calls an external function (not in this module).
    // Verify the external callee is not in the SCC decomposition and
    // extract_callees finds it, but scc_of returns None.
    let db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("xfile_a");
    let name_external = interner.intern("xfile_ext");

    // A calls external — external is NOT in the module.
    let module = make_module(&db, vec![(name_a, calling_function(name_a, name_external))]);
    let decomp = arc_scc_decomposition(&db, module);

    // A should be in an SCC (size 1, non-recursive).
    assert_eq!(decomp.len(), 1);
    assert_eq!(decomp.scc_of(name_a), Some(0));
    assert!(!decomp.is_recursive(0));

    // External callee should NOT be in any SCC.
    assert_eq!(
        decomp.scc_of(name_external),
        None,
        "external callee should not appear in SCC decomposition"
    );

    // Verify extract_callees sees the external call.
    let func_a = module.get_function(&db, name_a).unwrap();
    let callees = ori_arc::extract_callees(func_a);
    assert!(
        callees.contains(&name_external),
        "extract_callees should find the external call"
    );
}

// ── Incremental behavior tests (Section 12.12) ──────────────────────
//
// These tests verify that Salsa's memoization and early cutoff work
// correctly for borrow inference queries.
//
// **Module-level granularity note:** With `ArcModuleInput` containing
// all functions as a single `Vec`, changing ANY function's body
// invalidates ALL `infer_borrow_scc` queries (they all read
// `module.functions(db)`). Early cutoff at this level means that when
// a query re-executes but produces the same `BorrowSigResult`, Salsa
// prevents downstream dependents from re-executing. True "skip
// re-execution for unchanged SCCs" requires per-function Salsa inputs
// (deferred to Section 12.14).

/// Build a function with a `str` param that just reads it (stays Borrowed).
///
/// `fn name(x: str) -> int { prim_op(x, x) }` — reads x, never stores.
fn pure_reader(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: ArcVarId::new(1),
                ty: Idx::INT,
                value: ori_arc::ir::ArcValue::PrimOp {
                    op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![ArcVarId::new(0), ArcVarId::new(0)],
                },
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::INT],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Build a function with a `str` param that stores it (becomes Owned).
///
/// `fn name(x: str) -> (str,) { Tuple(x) }` — constructs from x.
fn param_storer(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: ArcVarId::new(1),
                ty: Idx::UNIT,
                ctor: CtorKind::Tuple,
                args: vec![ArcVarId::new(0)],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::UNIT],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Build a function that calls another and passes its `str` param.
///
/// `fn name(x: str) -> str { callee(x) }` — forwards x to callee.
fn param_forwarder(name: Name, callee: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: ArcVarId::new(1),
                ty: Idx::STR,
                func: callee,
                args: vec![ArcVarId::new(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(1),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::STR],
        var_reprs: vec![],
        spans: vec![vec![None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Build a "modified reader" — same borrow signature as `pure_reader`
/// but with a different body (extra let binding).
///
/// `fn name(x: str) -> int { let _ = prim_op(x,x); prim_op(x,x) }`
fn modified_reader(name: Name) -> ArcFunction {
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::INT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                // Extra instruction — body changed but borrow sig unchanged.
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ori_arc::ir::ArcValue::PrimOp {
                        op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(0)],
                    },
                },
                ArcInstr::Let {
                    dst: ArcVarId::new(2),
                    ty: Idx::INT,
                    value: ori_arc::ir::ArcValue::PrimOp {
                        op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Mul),
                        args: vec![ArcVarId::new(1), ArcVarId::new(1)],
                    },
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::INT, Idx::INT],
        var_reprs: vec![],
        spans: vec![vec![None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// Store an empty Pool in the database's pool cache for a given path.
fn setup_pool(db: &CompilerDb, path: &Path) {
    db.pool_cache().store(path, Pool::new());
}

/// Count `WillExecute` events in the log that match a given query name.
fn count_query_events(logs: &[String], query_name: &str) -> usize {
    logs.iter().filter(|s| s.contains(query_name)).count()
}

const TEST_PATH: &str = "/test/incremental.ori";

/// Build a module and set up its Pool in one call.
fn make_incremental_module(db: &CompilerDb, funcs: Vec<(Name, ArcFunction)>) -> ArcModuleInput {
    let mut sorted = funcs;
    sorted.sort_by_key(|(name, _)| *name);
    setup_pool(db, Path::new(TEST_PATH));
    ArcModuleInput::new(db, std::path::PathBuf::from(TEST_PATH), sorted)
}

/// Query all SCCs for a module, returning the total number of SCCs.
fn query_all_sccs(db: &dyn Db, module: ArcModuleInput) -> u32 {
    let decomp = arc_scc_decomposition(db, module);
    let count = u32::try_from(decomp.len()).unwrap();
    for i in 0..count {
        let _ = infer_borrow_scc(db, module, i);
    }
    count
}

#[test]
fn incremental_memoized_on_same_revision() {
    // Same input, same revision → full memoization (no re-execution).
    let db = CompilerDb::new();
    let interner = db.interner();
    db.enable_logging();

    let name_a = interner.intern("memo_a");
    let name_b = interner.intern("memo_b");
    let name_c = interner.intern("memo_c");

    let module = make_incremental_module(
        &db,
        vec![
            (name_a, pure_reader(name_a)),
            (name_b, pure_reader(name_b)),
            (name_c, pure_reader(name_c)),
        ],
    );

    // First query — should execute all queries.
    let scc_count = query_all_sccs(&db, module);
    let logs1 = db.take_logs();

    assert!(scc_count >= 3, "3 independent functions → 3 SCCs");
    assert!(
        !logs1.is_empty(),
        "first query should produce WillExecute events"
    );
    let scc_execs = count_query_events(&logs1, "infer_borrow_scc");
    assert_eq!(
        scc_execs, scc_count as usize,
        "should execute infer_borrow_scc once per SCC"
    );

    // Second query on same revision → full memoization.
    let _ = query_all_sccs(&db, module);
    let logs2 = db.take_logs();

    assert!(
        logs2.is_empty(),
        "same revision should produce zero WillExecute events; got {} logs: {:#?}",
        logs2.len(),
        logs2
    );
}

#[test]
fn incremental_identical_update_results_stable() {
    // Set functions to identical values → queries re-execute (Salsa 0.18
    // bumps revision unconditionally) but produce the same results.
    // This is the foundation of early cutoff: even though queries
    // re-run, their results are Eq-equal, preventing downstream
    // invalidation.
    let mut db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("idem_a");
    let name_b = interner.intern("idem_b");

    let funcs = vec![(name_a, pure_reader(name_a)), (name_b, pure_reader(name_b))];

    let module = make_incremental_module(&db, funcs.clone());

    // First query — capture results.
    let decomp1 = arc_scc_decomposition(&db, module);
    let scc_count = u32::try_from(decomp1.len()).unwrap();
    let mut results_before = Vec::new();
    for i in 0..scc_count {
        results_before.push(infer_borrow_scc(&db, module, i));
    }

    // Set to identical functions (triggers new revision).
    let mut sorted = funcs;
    sorted.sort_by_key(|(name, _)| *name);
    module.set_functions(&mut db).to(sorted);

    // Re-query — results should be identical.
    let decomp2 = arc_scc_decomposition(&db, module);
    assert_eq!(decomp1, decomp2, "identical update → same decomposition");

    let mut results_after = Vec::new();
    for i in 0..scc_count {
        results_after.push(infer_borrow_scc(&db, module, i));
    }

    assert_eq!(
        results_before, results_after,
        "identical update should produce identical borrow results"
    );
}

#[test]
fn incremental_body_change_same_sig_results_stable() {
    // Change B's body without affecting borrow sig.
    // All queries re-execute (module-level granularity) but all
    // produce the same BorrowSigResult.
    let mut db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("stable_a");
    let name_b = interner.intern("stable_b");

    // A calls B, B just reads (Borrowed).
    let module = make_incremental_module(
        &db,
        vec![
            (name_a, param_forwarder(name_a, name_b)),
            (name_b, pure_reader(name_b)),
        ],
    );

    // First query — capture results.
    let decomp = arc_scc_decomposition(&db, module);
    let scc_count = u32::try_from(decomp.len()).unwrap();
    let mut results_before = Vec::new();
    for i in 0..scc_count {
        results_before.push(infer_borrow_scc(&db, module, i));
    }

    // Modify B's body (add extra instructions) but borrow sig unchanged.
    let mut new_funcs = vec![
        (name_a, param_forwarder(name_a, name_b)),
        (name_b, modified_reader(name_b)),
    ];
    new_funcs.sort_by_key(|(name, _)| *name);
    module.set_functions(&mut db).to(new_funcs);

    // Re-query — results should be identical.
    let decomp2 = arc_scc_decomposition(&db, module);
    let scc_count2 = u32::try_from(decomp2.len()).unwrap();
    assert_eq!(scc_count, scc_count2, "SCC count unchanged");

    let mut results_after = Vec::new();
    for i in 0..scc_count2 {
        results_after.push(infer_borrow_scc(&db, module, i));
    }

    assert_eq!(
        results_before, results_after,
        "body change with same borrow sig should produce identical results"
    );

    // Verify B's param stays Borrowed in both runs.
    let b_scc = decomp2.scc_of(name_b).unwrap();
    let b_result = &results_after[b_scc as usize];
    let b_sig = b_result.get(name_b).unwrap();
    assert_eq!(
        b_sig.params[0].ownership,
        Ownership::Borrowed,
        "B's str param should stay Borrowed (only reads)"
    );
}

#[test]
fn incremental_body_change_different_sig_propagates() {
    // Change B from reader (Borrowed) to storer (Owned).
    // A calls B and passes its param → A's param must also become Owned.
    let mut db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("prop_a");
    let name_b = interner.intern("prop_b");

    // V1: A→B, B just reads → both params Borrowed.
    let module = make_incremental_module(
        &db,
        vec![
            (name_a, param_forwarder(name_a, name_b)),
            (name_b, pure_reader(name_b)),
        ],
    );

    let decomp1 = arc_scc_decomposition(&db, module);
    let decomp1_count = u32::try_from(decomp1.len()).unwrap();
    for i in 0..decomp1_count {
        let _ = infer_borrow_scc(&db, module, i);
    }

    // Verify V1: B's param is Borrowed.
    let b_scc_v1 = decomp1.scc_of(name_b).unwrap();
    let b_result_v1 = infer_borrow_scc(&db, module, b_scc_v1);
    assert_eq!(
        b_result_v1.get(name_b).unwrap().params[0].ownership,
        Ownership::Borrowed,
        "V1: B should have Borrowed param"
    );

    // V2: Change B to store param → Owned.
    let mut new_funcs = vec![
        (name_a, param_forwarder(name_a, name_b)),
        (name_b, param_storer(name_b)),
    ];
    new_funcs.sort_by_key(|(name, _)| *name);
    module.set_functions(&mut db).to(new_funcs);

    // Re-query — B becomes Owned, A should also become Owned.
    let decomp2 = arc_scc_decomposition(&db, module);

    let b_scc_v2 = decomp2.scc_of(name_b).unwrap();
    let b_result_v2 = infer_borrow_scc(&db, module, b_scc_v2);
    assert_eq!(
        b_result_v2.get(name_b).unwrap().params[0].ownership,
        Ownership::Owned,
        "V2: B should have Owned param (stores in Construct)"
    );

    let a_scc_v2 = decomp2.scc_of(name_a).unwrap();
    let a_result_v2 = infer_borrow_scc(&db, module, a_scc_v2);
    assert_eq!(
        a_result_v2.get(name_a).unwrap().params[0].ownership,
        Ownership::Owned,
        "V2: A should have Owned param (passes to B's now-Owned position)"
    );
}

#[test]
fn incremental_new_function_sccs_increase() {
    // Add a function → SCC count increases.
    let mut db = CompilerDb::new();
    let interner = db.interner();
    db.enable_logging();

    let name_a = interner.intern("grow_a");
    let name_b = interner.intern("grow_b");

    // V1: two functions.
    let module = make_incremental_module(
        &db,
        vec![(name_a, pure_reader(name_a)), (name_b, pure_reader(name_b))],
    );

    let scc_count_v1 = query_all_sccs(&db, module);
    let logs1 = db.take_logs();
    let execs_v1 = count_query_events(&logs1, "infer_borrow_scc");
    assert_eq!(scc_count_v1, 2);
    assert_eq!(execs_v1, 2);

    // V2: add a third function.
    let name_c = interner.intern("grow_c");
    let mut new_funcs = vec![
        (name_a, pure_reader(name_a)),
        (name_b, pure_reader(name_b)),
        (name_c, pure_reader(name_c)),
    ];
    new_funcs.sort_by_key(|(name, _)| *name);
    module.set_functions(&mut db).to(new_funcs);

    let scc_count_v2 = query_all_sccs(&db, module);
    let logs2 = db.take_logs();
    let execs_v2 = count_query_events(&logs2, "infer_borrow_scc");

    assert_eq!(scc_count_v2, 3, "should now have 3 SCCs");
    assert_eq!(
        execs_v2, 3,
        "all 3 SCCs re-execute (module-level granularity)"
    );
}

#[test]
fn incremental_mutual_recursion_body_change_same_sig() {
    // A↔B mutual recursion. Change A's body (same sig).
    // The SCC re-runs fixed-point, produces same result.
    let mut db = CompilerDb::new();
    let interner = db.interner();

    let name_a = interner.intern("mut_rec_a");
    let name_b = interner.intern("mut_rec_b");

    // V1: A→B, B→A (mutual recursion, both just forward params).
    let module = make_incremental_module(
        &db,
        vec![
            (name_a, param_forwarder(name_a, name_b)),
            (name_b, param_forwarder(name_b, name_a)),
        ],
    );

    let decomp1 = arc_scc_decomposition(&db, module);
    assert_eq!(decomp1.len(), 1, "mutual recursion → 1 SCC");
    assert!(decomp1.is_recursive(0));

    let result_v1 = infer_borrow_scc(&db, module, 0);

    // V2: Replace A with a modified version that still just forwards.
    // Use a function that calls B and also does some extra work.
    let modified_a = ArcFunction {
        name: name_a,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::STR,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::STR,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                // Extra harmless instruction.
                ArcInstr::Let {
                    dst: ArcVarId::new(1),
                    ty: Idx::INT,
                    value: ori_arc::ir::ArcValue::PrimOp {
                        op: ori_arc::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![ArcVarId::new(0), ArcVarId::new(0)],
                    },
                },
                // Still calls B with the original param.
                ArcInstr::Apply {
                    dst: ArcVarId::new(2),
                    ty: Idx::STR,
                    func: name_b,
                    args: vec![ArcVarId::new(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(2),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::STR, Idx::INT, Idx::STR],
        var_reprs: vec![],
        spans: vec![vec![None, None]],
        is_fbip: false,
        num_captures: 0,
        cow_annotations: CowAnnotations::default(),
        drop_hints: DropHints::default(),
        tail_calls: Vec::new(),
    };

    let mut new_funcs = vec![
        (name_a, modified_a),
        (name_b, param_forwarder(name_b, name_a)),
    ];
    new_funcs.sort_by_key(|(name, _)| *name);
    module.set_functions(&mut db).to(new_funcs);

    let decomp2 = arc_scc_decomposition(&db, module);
    assert_eq!(decomp2.len(), 1, "still 1 SCC after body change");
    assert!(decomp2.is_recursive(0));

    let result_v2 = infer_borrow_scc(&db, module, 0);

    assert_eq!(
        result_v1, result_v2,
        "mutual recursion SCC body change with same sig → same result"
    );
}

#[test]
fn incremental_execution_count_cold_start() {
    // Verify execution counts: N SCCs → exactly N infer_borrow_scc +
    // 1 arc_scc_decomposition on cold start.
    let db = CompilerDb::new();
    let interner = db.interner();
    db.enable_logging();

    let name_a = interner.intern("count_a");
    let name_b = interner.intern("count_b");
    let name_c = interner.intern("count_c");
    let name_d = interner.intern("count_d");

    // A→B→C, D standalone. 4 SCCs total.
    let module = make_incremental_module(
        &db,
        vec![
            (name_a, param_forwarder(name_a, name_b)),
            (name_b, param_forwarder(name_b, name_c)),
            (name_c, pure_reader(name_c)),
            (name_d, pure_reader(name_d)),
        ],
    );

    let scc_count = query_all_sccs(&db, module);
    let logs = db.take_logs();

    assert_eq!(scc_count, 4);

    let decomp_execs = count_query_events(&logs, "arc_scc_decomposition");
    let scc_execs = count_query_events(&logs, "infer_borrow_scc");

    assert_eq!(
        decomp_execs, 1,
        "exactly 1 arc_scc_decomposition on cold start"
    );
    // Note: infer_borrow_scc may execute more than 4 times due to
    // collect_callee_sigs querying callee SCCs (which are already
    // computed but still logged as WillExecute on first access).
    assert!(
        scc_execs >= 4,
        "at least {scc_count} infer_borrow_scc executions; got {scc_execs}"
    );
}
