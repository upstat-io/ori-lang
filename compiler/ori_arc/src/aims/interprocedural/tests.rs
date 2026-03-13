//! Tests for interprocedural AIMS analysis.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, LitValue,
};
use crate::ownership::Ownership;
use crate::ArcClass;

use super::super::lattice::{Cardinality, Uniqueness};
use super::*;

// Test helpers

struct TestClassifier {
    scalars: Vec<bool>,
}

impl TestClassifier {
    fn all_ref(count: usize) -> Self {
        Self {
            scalars: vec![false; count],
        }
    }

    fn with_scalar(mut self, idx: usize) -> Self {
        if idx < self.scalars.len() {
            self.scalars[idx] = true;
        }
        self
    }
}

impl crate::ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if self
            .scalars
            .get(idx.raw() as usize)
            .copied()
            .unwrap_or(false)
        {
            ArcClass::Scalar
        } else {
            ArcClass::DefiniteRef
        }
    }
}

fn block_id(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn ty(n: u32) -> Idx {
    Idx::from_raw(n)
}

fn name(n: u32) -> Name {
    Name::from_raw(n)
}

// Extract contract from a single function (no interprocedural context)

#[test]
fn extract_contract_literal_return() {
    // fn f() -> int { return 42 }
    // v0 = literal 42; return v0
    let func = ArcFunction {
        name: name(1),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: var(0),
                ty: ty(0),
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1).with_scalar(0);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    assert!(contract.params.is_empty());
    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
}

#[test]
fn extract_contract_param_used_once() {
    // fn f(x: str) -> str { return x }
    // param v0: str; return v0
    let func = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    assert_eq!(contract.params.len(), 1);
    // Param returned directly → used once.
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
    // Returning a param → preserves freshness.
    assert!(contract.return_info.preserves_freshness);
}

#[test]
fn extract_contract_construct_return_is_unique() {
    // fn f() -> Point { return Point { x: 1, y: 2 } }
    // v0 = literal 1; v1 = literal 2; v2 = Construct(Point, [v0, v1]); return v2
    let func = ArcFunction {
        name: name(3),
        return_type: ty(2),
        var_types: vec![ty(0), ty(0), ty(2)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Let {
                    dst: var(1),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(2),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![var(0), var(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3).with_scalar(0);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    assert_eq!(contract.return_info.uniqueness, Uniqueness::Unique);
    assert!(contract.return_info.preserves_freshness);
}

#[test]
fn analyze_program_single_function() {
    // fn f(x: str) -> str { return x }
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    assert!(contracts.contains_key(&name(1)));

    let contract = &contracts[&name(1)];
    assert_eq!(contract.params.len(), 1);
    assert_eq!(contract.params[0].cardinality, Cardinality::Once);
}

#[test]
fn analyze_program_callee_before_caller() {
    // fn callee() -> T { Construct(T) }
    // fn caller() -> T { Apply(callee) }
    // Callee returns Unique → caller's return should also be Unique.
    let callee = ArcFunction {
        name: name(1),
        return_type: ty(1),
        var_types: vec![ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(0),
                ty: ty(1),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        return_type: ty(1),
        var_types: vec![ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(0),
                ty: ty(1),
                func: name(1),
                args: vec![],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    // Callee constructs → Unique.
    assert_eq!(
        contracts[&name(1)].return_info.uniqueness,
        Uniqueness::Unique
    );
    // Caller calls callee with Unique return → also Unique.
    assert_eq!(
        contracts[&name(2)].return_info.uniqueness,
        Uniqueness::Unique
    );
}

// Section 09.2: Effect Activation tests

#[test]
fn pure_function_call_preserves_caller_uniqueness() {
    // callee: fn g(x: T) -> T { return x }  — pure, no alloc, no share
    // caller: fn f(a: T) -> T { let r = g(a); return r }
    //
    // Since callee is pure (may_share=false), borrowed args preserve uniqueness.
    // The caller should pass `a` as Borrowed (callee only uses once) and the
    // callee's contract should have may_share=false.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1),
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    // Callee is pure: no allocations, no sharing.
    let callee_contract = &contracts[&name(1)];
    assert!(
        !callee_contract.effects.may_share,
        "pure callee should have may_share=false"
    );
    assert!(
        !callee_contract.effects.may_allocate,
        "pure callee should have may_allocate=false"
    );
    assert!(callee_contract.is_fbip, "pure callee should be FBIP");

    // Caller calls a pure callee → caller's effects propagate from callee.
    let caller_contract = &contracts[&name(2)];
    assert!(
        !caller_contract.effects.may_share,
        "caller of pure function should have may_share=false"
    );
}

#[test]
fn function_without_allocations_is_fbip() {
    // fn f(x: T) -> T { return x }
    // No Construct, no PartialApply → may_allocate=false → is_fbip=true.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&name(1)];

    assert!(
        !contract.effects.may_allocate,
        "no Construct → no allocation"
    );
    assert!(contract.is_fbip, "non-allocating function should be FBIP");

    // Contrast: a function WITH a Construct should NOT be FBIP.
    let allocating_func = ArcFunction {
        name: name(2),
        return_type: ty(1),
        var_types: vec![ty(0), ty(1)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: var(0),
                    ty: ty(0),
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(1),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![var(0)],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let contracts = analyze_program(&[allocating_func], &classifier, &builtins, &interner);
    let alloc_contract = &contracts[&name(2)];

    assert!(
        alloc_contract.effects.may_allocate,
        "Construct → may_allocate"
    );
    assert!(
        !alloc_contract.is_fbip,
        "allocating function should NOT be FBIP"
    );
}

#[test]
fn effect_propagation_through_scc_converges() {
    // Two mutually recursive functions:
    // fn a(x: T) -> T { let r = b(x); return r }
    // fn b(x: T) -> T { let r = a(x); return r }
    //
    // Neither allocates, neither shares — effects should converge to
    // may_allocate=false, may_share=false through the SCC fixpoint.
    let func_a = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(2), // calls b
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let func_b = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1), // calls a
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func_a, func_b], &classifier, &builtins, &interner);

    // Both functions exist in the result.
    assert!(
        contracts.contains_key(&name(1)),
        "func_a should have contract"
    );
    assert!(
        contracts.contains_key(&name(2)),
        "func_b should have contract"
    );

    // Neither allocates → may_allocate converges to false.
    let a_contract = &contracts[&name(1)];
    let b_contract = &contracts[&name(2)];

    assert!(
        !a_contract.effects.may_allocate,
        "SCC with no Construct should converge to may_allocate=false"
    );
    assert!(
        !b_contract.effects.may_allocate,
        "SCC with no Construct should converge to may_allocate=false"
    );

    // Both are FBIP (no allocations).
    assert!(
        a_contract.is_fbip,
        "non-allocating SCC member should be FBIP"
    );
    assert!(
        b_contract.is_fbip,
        "non-allocating SCC member should be FBIP"
    );

    // Effects are consistent across the SCC.
    assert_eq!(
        a_contract.effects, b_contract.effects,
        "symmetric SCC members should have identical effects"
    );
}

// Demand propagation: linear consumption tightens callee uniqueness (Section 09.1)

#[test]
fn demand_propagation_single_caller_owned_linear_once() {
    // callee(p0: T) -> T: { return p0 }
    // caller(): { v0 = Construct; v1 = callee(v0); return v1 }
    //
    // caller passes a freshly constructed value (Owned, Linear, Once)
    // to callee's param 0. Since this is the ONLY caller, the all-callers
    // condition is satisfied → callee.params[0].uniqueness should be Unique.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::Unique,
        "single caller passing Owned+Linear+Once → callee param uniqueness should be Unique"
    );
}

#[test]
fn demand_propagation_multiple_callers_all_satisfy() {
    // callee(p0: T) -> T: { return p0 }
    // caller_a(): { v0 = Construct; v1 = callee(v0); return v1 }
    // caller_b(): { v0 = Construct; v1 = callee(v0); return v1 }
    //
    // Both callers pass Owned+Linear+Once → callee param should be Unique.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let make_caller = |caller_name: u32| ArcFunction {
        name: name(caller_name),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let caller_a = make_caller(2);
    let caller_b = make_caller(3);

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(
        &[callee, caller_a, caller_b],
        &classifier,
        &builtins,
        &interner,
    );

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::Unique,
        "all callers pass Owned+Linear+Once → callee param uniqueness should be Unique"
    );
}

#[test]
fn demand_propagation_one_caller_violates() {
    // callee(p0: T) -> T: { return p0 }
    // caller_good(): { v0 = Construct; v1 = callee(v0); return v1 }
    // caller_bad(p0: T): { v1 = callee(p0); v2 = callee(p0); return v2 }
    //
    // caller_bad passes p0 twice (cardinality=Many) → all-callers condition
    // NOT satisfied → callee param stays MaybeShared.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller_good = ArcFunction {
        name: name(2),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(0),
                    ty: ty(0),
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    // caller_bad passes the same param to callee twice (cardinality=Many).
    let caller_bad = ArcFunction {
        name: name(3),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Apply {
                    dst: var(1),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::Apply {
                    dst: var(2),
                    ty: ty(0),
                    func: name(1),
                    args: vec![var(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(
        &[callee, caller_good, caller_bad],
        &classifier,
        &builtins,
        &interner,
    );

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "one caller violates the condition → callee param stays MaybeShared"
    );
}

#[test]
fn demand_propagation_no_callers_stays_maybe_shared() {
    // callee(p0: T) -> T: { return p0 }
    // No other function calls callee → no demand info → stays MaybeShared.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::MaybeShared,
        "no callers → no demand propagation → stays MaybeShared"
    );
}

#[test]
fn demand_propagation_forwarded_param_owned_linear_once() {
    // callee(p0: T) -> T: { return p0 }
    // caller(p0: T) -> T: { v1 = callee(p0); return v1 }
    //
    // caller forwards its own parameter to callee. The caller's backward
    // demand on p0 is Owned+Linear+Once (used once at the callee call).
    // Since this is the only caller, callee.params[0] should be Unique.
    let callee = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let caller = ArcFunction {
        name: name(2),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: var(1),
                ty: ty(0),
                func: name(1),
                args: vec![var(0)],
                arg_ownership: vec![ArgOwnership::Owned],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[callee, caller], &classifier, &builtins, &interner);

    let callee_contract = &contracts[&name(1)];
    assert_eq!(
        callee_contract.params[0].uniqueness,
        Uniqueness::Unique,
        "forwarded param with Owned+Linear+Once → callee param uniqueness should be Unique"
    );
}

// FIP contract classification (Section 09.2)

#[test]
fn extract_contract_fbip_still_certified() {
    // fn f(x: T) -> T { return x }
    // No allocations → FBIP → FipContract::Certified.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: var(0) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(1);
    let builtins = crate::borrow::BuiltinOwnershipSets::empty();
    let interner = ori_ir::StringInterner::new();

    let contracts = analyze_program(&[func], &classifier, &builtins, &interner);
    let contract = &contracts[&name(1)];

    assert_eq!(
        contract.fip,
        FipContract::Certified,
        "FBIP function should be FipContract::Certified"
    );
    assert!(contract.is_fbip);
}

#[test]
fn extract_contract_token_balanced_produces_conditional() {
    // fn f(x: T) -> T { v1 = Construct(T, []); return v1 }
    // 1 Construct + 1 consumed param → token balanced.
    // But param needs uniqueness for reuse → FipContract::Conditional.
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(1),
                ty: ty(0),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(1) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(2);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    // Param v0 is consumed (Dead — never used after entry) and non-scalar.
    // 1 Construct balanced by 1 consumed param.
    // Since param requires uniqueness → Conditional.
    assert!(
        matches!(contract.fip, FipContract::Conditional { .. }),
        "token-balanced with consumed param should produce Conditional, got {:?}",
        contract.fip
    );

    if let FipContract::Conditional {
        requires_unique_params,
    } = &contract.fip
    {
        assert_eq!(requires_unique_params.len(), 1);
        assert!(
            requires_unique_params[0],
            "consumed non-scalar param should require uniqueness"
        );
    }
}

#[test]
fn extract_contract_net_positive_produces_bounded() {
    // fn f(x: T) -> T { v1 = Construct; v2 = Construct; return v2 }
    // 2 Constructs + 1 consumed param → net = 1 → Bounded(1).
    let func = ArcFunction {
        name: name(1),
        params: vec![ArcParam {
            var: var(0),
            ty: ty(0),
            ownership: Ownership::Owned,
        }],
        return_type: ty(0),
        var_types: vec![ty(0), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![
                ArcInstr::Construct {
                    dst: var(1),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![],
                },
                ArcInstr::Construct {
                    dst: var(2),
                    ty: ty(0),
                    ctor: CtorKind::Struct(name(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: var(2) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(3);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    // 2 Constructs - 1 consumed param = net 1 → Bounded(1).
    assert_eq!(
        contract.fip,
        FipContract::Bounded(1),
        "net positive allocation should produce Bounded"
    );
}

#[test]
fn extract_contract_conditional_requires_unique_vector() {
    // fn f(x: T, y: int, z: T) -> T { v3 = Construct; return v3 }
    // x (v0) is consumed non-scalar → requires unique.
    // y (v1) is scalar → excluded.
    // z (v2) is consumed non-scalar → requires unique.
    // 1 Construct, 2 consumed params → balanced → Conditional with [true, false, true].
    let func = ArcFunction {
        name: name(1),
        params: vec![
            ArcParam {
                var: var(0),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(1),
                ty: ty(1), // scalar
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: var(2),
                ty: ty(0),
                ownership: Ownership::Owned,
            },
        ],
        return_type: ty(0),
        var_types: vec![ty(0), ty(1), ty(0), ty(0)],
        blocks: vec![ArcBlock {
            id: block_id(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: var(3),
                ty: ty(0),
                ctor: CtorKind::Struct(name(10)),
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: var(3) },
        }],
        ..Default::default()
    };

    let classifier = TestClassifier::all_ref(4).with_scalar(1);
    let sigs = FxHashMap::default();
    let state_map = analyze_function(&func, &classifier, &sigs, &[], Vec::new());
    let contract = extract_contract(&func, &state_map, &classifier, &sigs);

    // Token balanced: 1 construct, 2 consumed non-scalar params (surplus).
    // requires_unique_params: [true(x), false(y=scalar), true(z)].
    if let FipContract::Conditional {
        requires_unique_params,
    } = &contract.fip
    {
        assert_eq!(requires_unique_params.len(), 3);
        assert!(requires_unique_params[0], "x should require uniqueness");
        assert!(
            !requires_unique_params[1],
            "y (scalar) should not require uniqueness"
        );
        assert!(requires_unique_params[2], "z should require uniqueness");
    } else {
        panic!("expected Conditional, got {:?}", contract.fip);
    }
}
