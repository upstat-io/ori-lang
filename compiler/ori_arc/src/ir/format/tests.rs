//! Tests for the canonical ARC IR formatter.

use ori_ir::StringInterner;
use ori_types::{Idx, Pool};

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy,
};
use crate::ownership::Ownership;

/// Build a minimal function for formatter tests.
fn simple_function(interner: &StringInterner) -> ArcFunction {
    let name = interner.intern("test_func");
    ArcFunction {
        name,
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: Idx::NONE,
            ownership: Ownership::Owned,
        }],
        return_type: Idx::NONE,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE],
        ..Default::default()
    }
}

#[test]
fn format_function_called_twice_on_same_input_produces_identical_output() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let func = simple_function(&interner);

    let first = super::format_function(&func, &pool, &interner);
    let second = super::format_function(&func, &pool, &interner);

    assert_eq!(first, second, "formatter must be deterministic");
}

#[test]
fn format_function_with_heap_data_excludes_pointer_addresses_from_output() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let func = simple_function(&interner);

    let output = super::format_function(&func, &pool, &interner);

    // No hex pointer addresses in output.
    assert!(
        !output.contains("0x"),
        "formatter output must not contain pointer addresses: {output}"
    );
}

#[test]
fn format_function_with_known_ir_produces_stable_golden_output() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let func = simple_function(&interner);

    let output = super::format_function(&func, &pool, &interner);

    // Verify structural elements are present.
    assert!(
        output.contains("fn @test_func("),
        "output must contain function name: {output}"
    );
    assert!(
        output.contains("[entry: bb0]"),
        "output must contain entry block: {output}"
    );
    assert!(
        output.contains("bb0:"),
        "output must contain block label: {output}"
    );
    assert!(
        output.contains("Return %0"),
        "output must contain return instruction: {output}"
    );
    assert!(
        output.contains("[own]"),
        "output must contain ownership annotation: {output}"
    );
}

#[test]
fn format_function_with_rc_ops_includes_rc_inc_and_dec_in_output() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let name = interner.intern("rc_func");

    let func = ArcFunction {
        name,
        params: vec![],
        return_type: Idx::NONE,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: ArcVarId::new(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                    atomicity: crate::ir::RcAtomicity::default_atomic(),
                },
                ArcInstr::RcDec {
                    var: ArcVarId::new(0),
                    strategy: RcStrategy::HeapPointer,
                    atomicity: crate::ir::RcAtomicity::default_atomic(),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE],
        ..Default::default()
    };

    let output = super::format_function(&func, &pool, &interner);

    assert!(
        output.contains("RcInc %0 [HeapPtr]"),
        "output must contain RcInc: {output}"
    );
    assert!(
        output.contains("RcDec %0 [HeapPtr]"),
        "output must contain RcDec: {output}"
    );
}

/// BurdenInc/BurdenDec format as `burden_inc %N` / `burden_dec %N`
/// (parallel to `RcInc %N [strategy]` but no strategy slot — burden ops
/// are trivial side-effect markers).
#[test]
fn format_function_with_burden_ops_includes_burden_inc_and_dec_in_output() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let name = interner.intern("burden_func");

    let func = ArcFunction {
        name,
        params: vec![],
        return_type: Idx::NONE,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::BurdenInc {
                    var: ArcVarId::new(0),
                },
                ArcInstr::BurdenDec {
                    var: ArcVarId::new(0),
                },
            ],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE],
        ..Default::default()
    };

    let output = super::format_function(&func, &pool, &interner);

    assert!(
        output.contains("burden_inc %0"),
        "output must contain burden_inc: {output}"
    );
    assert!(
        output.contains("burden_dec %0"),
        "output must contain burden_dec: {output}"
    );
}

#[test]
fn format_function_with_mixed_ownership_shows_own_and_borrow_annotations() {
    let interner = StringInterner::new();
    let pool = Pool::default();
    let name = interner.intern("mixed_ownership");

    let func = ArcFunction {
        name,
        params: vec![
            ArcParam {
                var: ArcVarId::new(0),
                ty: Idx::NONE,
                ownership: Ownership::Owned,
            },
            ArcParam {
                var: ArcVarId::new(1),
                ty: Idx::NONE,
                ownership: Ownership::Borrowed,
            },
        ],
        return_type: Idx::NONE,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::NONE; 2],
        ..Default::default()
    };

    let output = super::format_function(&func, &pool, &interner);

    assert!(
        output.contains("[own]"),
        "output must contain [own] annotation: {output}"
    );
    assert!(
        output.contains("[borrow]"),
        "output must contain [borrow] annotation: {output}"
    );
}
