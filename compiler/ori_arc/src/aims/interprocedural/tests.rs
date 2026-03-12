//! Tests for interprocedural AIMS analysis.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::FxHashMap;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    CtorKind, LitValue,
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
