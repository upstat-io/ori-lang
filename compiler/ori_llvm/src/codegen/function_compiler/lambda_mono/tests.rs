//! Tests for lambda monomorphization `var_reprs` fixup.

use ori_arc::ir::{
    ArcBlock, ArcBlockId, ArcInstr, ArcParam, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
};
use ori_arc::ownership::Ownership;
use ori_arc::ArcClassifier;
use ori_ir::Name;
use ori_types::{Idx, Pool};

/// Helper: build a minimal `ArcFunction` with given `var_types` and `var_reprs`.
fn make_func_with_reprs(
    var_types: Vec<Idx>,
    var_reprs: Vec<ValueRepr>,
    body: Vec<ArcInstr>,
) -> ori_arc::ArcFunction {
    ori_arc::ArcFunction {
        name: Name::from_raw(1),
        params: vec![ArcParam {
            var: ArcVarId::new(0),
            ty: var_types[0],
            ownership: Ownership::Owned,
        }],
        return_type: Idx::UNIT,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body,
            terminator: ArcTerminator::Return {
                value: ArcVarId::new(0),
            },
        }],
        entry: ArcBlockId::new(0),
        spans: vec![vec![]],
        var_types,
        var_reprs,
        is_fbip: false,
        num_captures: 0,
        cow_annotations: ori_arc::uniqueness::CowAnnotations::default(),
        drop_hints: ori_arc::uniqueness::DropHints::default(),
        tail_calls: Vec::new(),
    }
}

/// When a var's type changes from `Var`→`int` (`RcPointer`→`Scalar`), the fixup
/// must remove `RcInc`/`RcDec` on that var.
#[test]
fn strips_rc_ops_on_vars_that_became_scalar() {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    // Var 0: int (Scalar) — but stale var_reprs says RcPointer
    // Var 1: str (FatValue) — unchanged, RC ops should remain
    let var_types = vec![Idx::INT, Idx::STR];
    let stale_reprs = vec![ValueRepr::RcPointer, ValueRepr::FatValue];
    let body = vec![
        ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 1,
            strategy: RcStrategy::HeapPointer, // wrong for int
        },
        ArcInstr::RcDec {
            var: ArcVarId::new(0),
            strategy: RcStrategy::HeapPointer, // wrong for int
        },
        ArcInstr::RcDec {
            var: ArcVarId::new(1),
            strategy: RcStrategy::FatPointer, // correct for str
        },
    ];

    let mut func = make_func_with_reprs(var_types, stale_reprs, body);
    // Correct the span count to match body length.
    func.spans = vec![vec![None; 3]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    // var_reprs updated.
    assert_eq!(func.var_reprs[0], ValueRepr::Scalar, "int → Scalar");
    assert_eq!(func.var_reprs[1], ValueRepr::FatValue, "str → FatValue");

    // RC ops on var 0 removed (was RcPointer, now Scalar).
    let remaining: Vec<_> = func.blocks[0]
        .body
        .iter()
        .filter(|i| matches!(i, ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. }))
        .collect();
    assert_eq!(remaining.len(), 1, "only str's RcDec should remain");

    // Remaining op is var 1's RcDec with correct strategy.
    if let ArcInstr::RcDec { var, strategy } = &remaining[0] {
        assert_eq!(var.raw(), 1);
        assert_eq!(*strategy, RcStrategy::FatPointer);
    } else {
        panic!("expected RcDec, got {:?}", remaining[0]);
    }
}

/// When a var's type changes from `Var`→`str` (`RcPointer`→`FatValue`), the fixup
/// must update the `RcStrategy` from `HeapPointer` to `FatPointer`.
#[test]
fn updates_strategy_when_repr_changes_between_ref_types() {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    // Var 0: str — but stale var_reprs says RcPointer (was Var before fixup)
    let var_types = vec![Idx::STR];
    let stale_reprs = vec![ValueRepr::RcPointer]; // should be FatValue
    let body = vec![ArcInstr::RcDec {
        var: ArcVarId::new(0),
        strategy: RcStrategy::HeapPointer, // wrong — should be FatPointer
    }];

    let mut func = make_func_with_reprs(var_types, stale_reprs, body);
    func.spans = vec![vec![None; 1]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    // var_reprs updated.
    assert_eq!(func.var_reprs[0], ValueRepr::FatValue);

    // Strategy updated to FatPointer.
    assert_eq!(func.blocks[0].body.len(), 1, "RC op retained");
    if let ArcInstr::RcDec { strategy, .. } = &func.blocks[0].body[0] {
        assert_eq!(*strategy, RcStrategy::FatPointer, "strategy updated");
    } else {
        panic!("expected RcDec");
    }
}

/// When `var_reprs` are already correct, fixup is a no-op.
#[test]
fn noop_when_reprs_already_correct() {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    let var_types = vec![Idx::INT, Idx::STR];
    let correct_reprs = vec![ValueRepr::Scalar, ValueRepr::FatValue];
    let body = vec![ArcInstr::RcDec {
        var: ArcVarId::new(1),
        strategy: RcStrategy::FatPointer,
    }];

    let mut func = make_func_with_reprs(var_types, correct_reprs, body);
    func.spans = vec![vec![None; 1]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    // Nothing changed.
    assert_eq!(func.var_reprs[0], ValueRepr::Scalar);
    assert_eq!(func.var_reprs[1], ValueRepr::FatValue);
    assert_eq!(func.blocks[0].body.len(), 1, "RC op retained");
}

/// When `var_reprs` is empty (pre-pipeline), fixup returns early.
#[test]
fn noop_when_var_reprs_empty() {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);

    let var_types = vec![Idx::INT];
    let body = vec![ArcInstr::RcDec {
        var: ArcVarId::new(0),
        strategy: RcStrategy::HeapPointer,
    }];

    let mut func = make_func_with_reprs(var_types, Vec::new(), body);
    func.spans = vec![vec![None; 1]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    // Empty var_reprs stays empty.
    assert!(func.var_reprs.is_empty());
    // RC op not touched.
    assert_eq!(func.blocks[0].body.len(), 1);
}

/// When a var changes from `Var`→`[int]` (`RcPointer`→`RcPointer`), strategy stays
/// `HeapPointer` — no change needed.
#[test]
fn no_change_when_repr_stays_same() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);
    let classifier = ArcClassifier::new(&pool);

    let var_types = vec![list_int];
    let stale_reprs = vec![ValueRepr::RcPointer]; // same as new
    let body = vec![ArcInstr::RcDec {
        var: ArcVarId::new(0),
        strategy: RcStrategy::HeapPointer,
    }];

    let mut func = make_func_with_reprs(var_types, stale_reprs, body);
    func.spans = vec![vec![None; 1]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    assert_eq!(func.var_reprs[0], ValueRepr::RcPointer);
    assert_eq!(func.blocks[0].body.len(), 1);
    if let ArcInstr::RcDec { strategy, .. } = &func.blocks[0].body[0] {
        assert_eq!(*strategy, RcStrategy::HeapPointer, "strategy unchanged");
    }
}

/// Strategy update for `Var`→`Option<str>`: `RcPointer`→`Aggregate`, `HeapPointer`→`InlineEnum`.
/// (`Option<int>` is trivially `Scalar` — needs a ref-containing payload to be `Aggregate`.)
#[test]
fn updates_strategy_for_option_type() {
    let mut pool = Pool::new();
    let option_str = pool.option(Idx::STR);
    let classifier = ArcClassifier::new(&pool);

    let var_types = vec![option_str];
    let stale_reprs = vec![ValueRepr::RcPointer]; // was Var → RcPointer
    let body = vec![ArcInstr::RcDec {
        var: ArcVarId::new(0),
        strategy: RcStrategy::HeapPointer, // wrong for Option<str>
    }];

    let mut func = make_func_with_reprs(var_types, stale_reprs, body);
    func.spans = vec![vec![None; 1]];

    super::fixup_parent_var_reprs_and_rc_ops(&mut func, &classifier, &pool);

    assert_eq!(
        func.var_reprs[0],
        ValueRepr::Aggregate,
        "Option<str> → Aggregate"
    );
    if let ArcInstr::RcDec { strategy, .. } = &func.blocks[0].body[0] {
        assert_eq!(
            *strategy,
            RcStrategy::InlineEnum,
            "Option<str> → InlineEnum"
        );
    }
}
