//! Phase-6.99 transfer-anchor credit-net pins.
//!
//! Pins [`super::compute_transfer_anchor_credit_repairs`]: the per-rep
//! alloc-aware net over a `transfers_through_return ∧ Owned ∧ Direct`
//! forwarder-result lineage models the transferred-in CREDIT (+1 at the
//! anchor's normal successor), the `[own]` hand-offs (-1), fresh births (+1),
//! and surviving whole-var ops (±1) — then repairs ONLY when every Return
//! terminal provably nets 0 (Resume >= 0): removing the single spurious
//! fresh-site keep-alive inc, or placing ONE release after the execution-final
//! value-read. Spec: Annex E §AIMS RL-34 + RL-2 + RL-1 + RL-comp.

use super::{compute_transfer_anchor_credit_repairs, CreditRepair};
use crate::aims::contract::{MemoryContract, ReturnAliasShape};
use crate::aims::lattice::AccessClass;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcParam, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, CtorKind, ValueRepr,
};
use crate::ownership::Ownership;
use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::FxHashMap;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

fn fwd_name() -> Name {
    Name::from_raw(42)
}

fn len_name() -> Name {
    Name::from_raw(43)
}

/// A `transfers_through_return ∧ Owned ∧ Direct` contract over `n` params.
fn forwarder_contract(n: usize) -> MemoryContract {
    let mut c = MemoryContract::conservative(n);
    for p in &mut c.params {
        p.transfers_through_return = true;
        p.access = AccessClass::Owned;
        p.return_alias = Some(ReturnAliasShape::Direct);
        p.return_payload_contains_param = false;
    }
    c
}

fn forwarder_contracts(n: usize) -> FxHashMap<Name, MemoryContract> {
    let mut m = FxHashMap::default();
    m.insert(fwd_name(), forwarder_contract(n));
    m
}

fn construct_list(dst: ArcVarId) -> ArcInstr {
    ArcInstr::Construct {
        dst,
        ty: Idx::from_raw(0),
        ctor: CtorKind::ListLiteral,
        args: Vec::new(),
    }
}

fn let_alias(dst: ArcVarId, src: ArcVarId) -> ArcInstr {
    ArcInstr::Let {
        dst,
        ty: Idx::from_raw(0),
        value: ArcValue::Var(src),
    }
}

fn borrowed_read(dst: ArcVarId, arg: ArcVarId) -> ArcInstr {
    ArcInstr::Apply {
        dst,
        ty: Idx::from_raw(0),
        func: len_name(),
        args: vec![arg],
        arg_ownership: vec![ArgOwnership::Borrowed],
        mono_instance_id: None,
    }
}

/// The placement shape (the `triple`-family member): two fresh lists
/// transferred `[own]` through the forwarder, the result read once then dead.
///
/// ```text
/// bb0: %0 = Construct List; %1 = Construct List
///      binc %0; binc %1; bdec %0; bdec %1        (transfer pairs, net 0)
///      %2 = Invoke @fwd(%0 [own], %1 [own]) normal bb1 unwind bb2
/// bb1: %3 = %2; %4 = @len(%3 [borrow]); Return %4
/// bb2: Resume
/// ```
///
/// Net: bb0 = +2(alloc) +2 -2 -2(own) = 0; bb1 = +1 (credit). Return = +1.
fn placement_shape() -> ArcFunction {
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0
        ValueRepr::RcPointer, // %1
        ValueRepr::RcPointer, // %2 result
        ValueRepr::RcPointer, // %3 alias
        ValueRepr::Scalar,    // %4 len
    ];
    ArcFunction {
        var_types: vec![Idx::from_raw(0); 5],
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    construct_list(v(0)),
                    construct_list(v(1)),
                    ArcInstr::BurdenInc { var: v(0) },
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::BurdenDec { var: v(0) },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::from_raw(0),
                    func: fwd_name(),
                    args: vec![v(0), v(1)],
                    arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![let_alias(v(3), v(2)), borrowed_read(v(4), v(3))],
                terminator: ArcTerminator::Return { value: v(4) },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    }
}

/// The removal shape (the `result_list_str`-family member): a fresh member
/// carrying a SURVIVING fresh-site keep-alive inc, transferred through the
/// forwarder, with the credit's release already present downstream.
///
/// ```text
/// bb0: %0 = Construct List; binc %0
///      %1 = %0; binc %1; bdec %1                 (transfer pair)
///      %2 = Invoke @fwd(%1 [own]) normal bb1 unwind bb2
/// bb1: %3 = %2; Jump bb3(%3)
/// bb2: Resume
/// bb3: (%4); bdec %4; Return %5(scalar)
/// ```
///
/// Net: bb0 = +1(alloc) +1(binc %0) +1 -1(pair) -1(own) = +1; bb1 = +1
/// (credit); bb3 = -1. Return = +1; removing `binc %0` nets every path 0.
fn removal_shape() -> ArcFunction {
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0
        ValueRepr::RcPointer, // %1 alias
        ValueRepr::RcPointer, // %2 result
        ValueRepr::RcPointer, // %3 alias
        ValueRepr::RcPointer, // %4 block param
        ValueRepr::Scalar,    // %5 scalar return
    ];
    ArcFunction {
        var_types: vec![Idx::from_raw(0); 6],
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![
                    construct_list(v(0)),
                    ArcInstr::BurdenInc { var: v(0) },
                    let_alias(v(1), v(0)),
                    ArcInstr::BurdenInc { var: v(1) },
                    ArcInstr::BurdenDec { var: v(1) },
                ],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::from_raw(0),
                    func: fwd_name(),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![let_alias(v(3), v(2))],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(3),
                    args: vec![v(3)],
                },
            },
            ArcBlock {
                id: ArcBlockId::new(2),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Resume,
            },
            ArcBlock {
                id: ArcBlockId::new(3),
                params: vec![(v(4), Idx::from_raw(0))],
                body: vec![ArcInstr::BurdenDec { var: v(4) }],
                terminator: ArcTerminator::Return { value: v(5) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    }
}

fn run(func: &ArcFunction, contracts: &FxHashMap<Name, MemoryContract>) -> Vec<CreditRepair> {
    let pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    compute_transfer_anchor_credit_repairs(func, &pool, &interner, contracts, &FxHashMap::default())
}

#[test]
fn forwarder_result_read_then_dead_places_one_release_at_the_read_sink() {
    let func = placement_shape();
    let repairs = run(&func, &forwarder_contracts(2));
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 1,
            var: v(3)
        }],
        "the +1 Return path gets exactly ONE release after the final read"
    );
}

#[test]
fn lone_opaque_view_dec_declines_the_lineage() {
    // An UNPROVEN (opaque) view — the member type is not a niche-family
    // wrapper — carries a LONE dec: the allocation's accounting extends
    // beyond the model, so the pass must decline (removal here double-frees:
    // the unseen -1 is on the SAME allocation).
    let mut func = removal_shape();
    func.var_reprs.push(ValueRepr::RcPointer); // %6 payload view
    func.var_types.push(Idx::from_raw(0));
    func.var_reprs.push(ValueRepr::Scalar); // %7 len
    func.var_types.push(Idx::from_raw(0));
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: Idx::from_raw(0),
            value: v(4),
            field: 1,
        },
        ArcInstr::BurdenDec { var: v(6) },
        borrowed_read(v(7), v(6)),
        ArcInstr::BurdenDec { var: v(4) },
    ];
    let repairs = run(&func, &forwarder_contracts(1));
    assert!(
        repairs.is_empty(),
        "a lone opaque view dec is outside the unified ledger"
    );
}

/// The removal shape re-typed as the Option-family niche wrapper: members
/// `%0..=%4` carry `Option<[int]>` (`Aggregate` repr), the payload view
/// `%6 = Project %4.1` carries `[int]` (`RcPointer`), `%5`/`%7` scalar.
/// Returns `(func, pool)`; the caller mutates `blocks[3]` per pin.
fn option_typed_removal_shape() -> (ArcFunction, Pool) {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let opt = pool.option(list_int);
    let mut func = removal_shape();
    func.var_types = vec![opt, opt, opt, opt, opt, Idx::INT];
    func.var_reprs = vec![
        ValueRepr::Aggregate, // %0 member construct
        ValueRepr::Aggregate, // %1 alias
        ValueRepr::Aggregate, // %2 result
        ValueRepr::Aggregate, // %3 alias
        ValueRepr::Aggregate, // %4 block param
        ValueRepr::Scalar,    // %5 scalar return
    ];
    func.var_types.push(list_int); // %6 payload view
    func.var_reprs.push(ValueRepr::RcPointer);
    func.var_types.push(Idx::INT); // %7 len
    func.var_reprs.push(ValueRepr::Scalar);
    (func, pool)
}

fn run_with_pool(
    func: &ArcFunction,
    contracts: &FxHashMap<Name, MemoryContract>,
    pool: &Pool,
) -> Vec<CreditRepair> {
    let interner = ori_ir::StringInterner::new();
    compute_transfer_anchor_credit_repairs(func, pool, &interner, contracts, &FxHashMap::default())
}

#[test]
fn lone_same_alloc_view_dec_joins_the_ledger_and_proves_removal() {
    // The Option-family admission: the niche-payload view's LONE whole-var
    // dec (placed after the final read) IS the lineage's release; counted in
    // the unified ledger it pairs with the transferred-in credit, leaving
    // the spurious member fresh-site inc as the unique proving removal.
    let (mut func, pool) = option_typed_removal_shape();
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        borrowed_read(v(7), v(6)),
        ArcInstr::BurdenDec { var: v(6) },
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert_eq!(
        repairs,
        vec![CreditRepair::RemoveInc {
            block: 0,
            instr_idx: 1
        }],
        "the lone same-alloc view dec is the modeled release; the member \
         fresh-site inc is the unique removal that nets every path 0"
    );
}

#[test]
fn stored_same_alloc_view_declines_the_lineage() {
    // ADVERSARIAL: the same-alloc view is NOT the sole release — it is
    // STORED into another aggregate (an unmodeled consume). The unified
    // ledger must decline, never prove on a view that escapes.
    let (mut func, pool) = option_typed_removal_shape();
    func.var_types.push(func.var_types[6]); // %8 container
    func.var_reprs.push(ValueRepr::RcPointer);
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        ArcInstr::Construct {
            dst: v(8),
            ty: func.var_types[6],
            ctor: CtorKind::ListLiteral,
            args: vec![v(6)],
        },
        ArcInstr::BurdenDec { var: v(6) },
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert!(
        repairs.is_empty(),
        "a stored same-alloc view escapes the ledger — decline"
    );
}

#[test]
fn same_alloc_balanced_view_pair_counts_in_order_and_keeps_removal() {
    // The Result/Option-niche balanced pair now COUNTS (+1 then -1 in
    // instruction order) instead of being excluded; the removal proof is
    // unchanged and the ordered walk keeps aliveness at every view use.
    let (mut func, pool) = option_typed_removal_shape();
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        ArcInstr::BurdenInc { var: v(6) },
        borrowed_read(v(7), v(6)),
        ArcInstr::BurdenDec { var: v(6) },
        ArcInstr::BurdenDec { var: v(4) },
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert_eq!(
        repairs,
        vec![CreditRepair::RemoveInc {
            block: 0,
            instr_idx: 1
        }],
        "a counted balanced view pair nets 0 in order — removal still proves"
    );
}

#[test]
fn nested_view_off_non_wrapper_payload_stays_opaque_and_declines() {
    // A second projection OFF the payload ([int] — not a niche wrapper) is
    // OPAQUE even though its source view is same-alloc; its lone dec fails
    // the balanced-pair gate and the lineage declines.
    let (mut func, pool) = option_typed_removal_shape();
    func.var_types.push(Idx::STR); // %8 element view off the payload
    func.var_reprs.push(ValueRepr::RcPointer);
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        ArcInstr::Project {
            dst: v(8),
            ty: Idx::STR,
            value: v(6),
            field: 0,
        },
        borrowed_read(v(7), v(8)),
        ArcInstr::BurdenDec { var: v(8) },
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert!(
        repairs.is_empty(),
        "a projection off a non-wrapper payload is opaque; its lone dec declines"
    );
}

#[test]
fn same_alloc_view_partial_dec_declines_the_lineage() {
    // A partial / variant-grain dec on a same-alloc view releases an
    // unproven SUBSET of the shared allocation — unmodeled, decline.
    let (mut func, pool) = option_typed_removal_shape();
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        borrowed_read(v(7), v(6)),
        ArcInstr::BurdenDecPartial {
            var: v(6),
            skip_fields: vec![0],
        },
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert!(
        repairs.is_empty(),
        "a partial-grain view dec is outside the whole-var ledger — decline"
    );
}

#[test]
fn view_dec_before_final_read_keeps_inc_and_places_member_release() {
    // The view dec BEFORE the final read, with the member keep-alive inc
    // live: removing the inc would dead-use the read (running 0 at an alive
    // event), so the ordered walk rejects removal — and instead proves the
    // keep-alive's missing SECOND release at the end of the read sink.
    let (mut func, pool) = option_typed_removal_shape();
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        ArcInstr::BurdenDec { var: v(6) },
        borrowed_read(v(7), v(6)),
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 3,
            var: v(4)
        }],
        "removal dead-uses the read; the proven repair is the second release \
         after the final read"
    );
}

#[test]
fn view_dec_before_final_read_without_keep_alive_is_unprovable() {
    // The same dec-before-read order on a MINIMAL lineage (no member
    // keep-alive inc): the running net hits 0 at the read on the baseline
    // walk — a dead-value use — so the lineage is unprovable and no repair
    // fires (RL-2: release at last use, never before).
    let (mut func, pool) = option_typed_removal_shape();
    func.blocks[0].body = vec![
        construct_list(v(0)),
        let_alias(v(1), v(0)),
        ArcInstr::BurdenInc { var: v(1) },
        ArcInstr::BurdenDec { var: v(1) },
    ];
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: func.var_types[6],
            value: v(4),
            field: 1,
        },
        ArcInstr::BurdenDec { var: v(6) },
        borrowed_read(v(7), v(6)),
    ];
    let repairs = run_with_pool(&func, &forwarder_contracts(1), &pool);
    assert!(
        repairs.is_empty(),
        "a view dec ahead of a live read dead-uses the baseline walk — decline"
    );
}

#[test]
fn balanced_view_pair_keeps_the_removal_repair() {
    // The Result-niche shape: the payload view carries a BALANCED inc/dec
    // pair inside one block (net 0 on the shared allocation) — the member
    // model stays sound and the spurious fresh-site inc is still removed.
    let mut func = removal_shape();
    func.var_reprs.push(ValueRepr::RcPointer); // %6 payload view
    func.var_types.push(Idx::from_raw(0));
    func.var_reprs.push(ValueRepr::Scalar); // %7 len
    func.var_types.push(Idx::from_raw(0));
    func.blocks[3].body = vec![
        ArcInstr::Project {
            dst: v(6),
            ty: Idx::from_raw(0),
            value: v(4),
            field: 1,
        },
        ArcInstr::BurdenInc { var: v(6) },
        borrowed_read(v(7), v(6)),
        ArcInstr::BurdenDec { var: v(6) },
        ArcInstr::BurdenDec { var: v(4) },
    ];
    let repairs = run(&func, &forwarder_contracts(1));
    assert_eq!(
        repairs,
        vec![CreditRepair::RemoveInc {
            block: 0,
            instr_idx: 1
        }],
        "a balanced view pair nets 0 — the removal repair still proves"
    );
}

#[test]
fn foreign_call_result_member_declines_the_lineage() {
    // A member bound by a SECOND, non-anchor body call (foreign acquisition
    // merged into the lineage via the same-alloc seed) is unmodeled — the
    // foreign callee's ownership of that binding is unknown.
    let mut func = placement_shape();
    func.var_reprs.push(ValueRepr::RcPointer); // %5 foreign result
    func.var_types.push(Idx::from_raw(0));
    func.blocks[1].body = vec![
        let_alias(v(3), v(2)),
        ArcInstr::Apply {
            dst: v(5),
            ty: Idx::from_raw(0),
            func: len_name(),
            args: vec![v(3)],
            arg_ownership: vec![ArgOwnership::Borrowed],
            mono_instance_id: None,
        },
    ];
    // Merge the foreign result into the lineage via the same-alloc seed.
    let mut same_alloc = FxHashMap::default();
    same_alloc.insert(v(5), v(2));
    let pool = Pool::default();
    let interner = ori_ir::StringInterner::new();
    let repairs = compute_transfer_anchor_credit_repairs(
        &func,
        &pool,
        &interner,
        &forwarder_contracts(2),
        &same_alloc,
    );
    assert!(
        repairs.is_empty(),
        "a non-anchor call result inside the lineage is a foreign acquisition"
    );
}

#[test]
fn surviving_fresh_site_inc_on_balanced_lineage_is_removed() {
    let func = removal_shape();
    let repairs = run(&func, &forwarder_contracts(1));
    assert_eq!(
        repairs,
        vec![CreditRepair::RemoveInc {
            block: 0,
            instr_idx: 1
        }],
        "the spurious fresh-site keep-alive inc is the unique removal that nets 0"
    );
}

#[test]
fn non_ttr_callee_result_is_never_anchored() {
    // Same placement shape but the callee has NO contract — no anchor, no
    // repair (the fresh-in-callee result class stays with its own machinery).
    let func = placement_shape();
    let repairs = run(&func, &FxHashMap::default());
    assert!(repairs.is_empty(), "no contract -> no anchor -> no repair");
}

#[test]
fn borrowed_forwarder_contract_declines_admission() {
    // ttr=true but access=Borrowed (the borrow-view forwarder): the caller
    // still owns the original; the result is an alias, not a transfer credit.
    let func = placement_shape();
    let mut c = forwarder_contract(2);
    for p in &mut c.params {
        p.access = AccessClass::Borrowed;
    }
    let mut contracts = FxHashMap::default();
    contracts.insert(fwd_name(), c);
    let repairs = run(&func, &contracts);
    assert!(
        repairs.is_empty(),
        "Borrowed ttr param is not a transfer anchor"
    );
}

#[test]
fn function_param_member_declines_the_lineage() {
    // The transferred arg is a FUNCTION PARAM (caller-retained allocation,
    // not caller-fresh): the lineage is unmodeled — change nothing.
    let mut func = placement_shape();
    func.blocks[0].body.clear();
    func.params = vec![
        ArcParam {
            var: v(0),
            ty: Idx::from_raw(0),
            ownership: Ownership::Owned,
        },
        ArcParam {
            var: v(1),
            ty: Idx::from_raw(0),
            ownership: Ownership::Owned,
        },
    ];
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(
        repairs.is_empty(),
        "param-rooted lineage is not caller-fresh"
    );
}

#[test]
fn already_balanced_lineage_is_left_untouched() {
    // The placement shape WITH its release already present: every Return
    // terminal nets 0 -> clean -> no repair.
    let mut func = placement_shape();
    func.blocks[1].body.push(ArcInstr::BurdenDec { var: v(3) });
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(repairs.is_empty(), "net-0 lineage gets no second release");
}

#[test]
fn merge_disagreement_declines_the_lineage() {
    // A branch releases the result on ONE arm only -> divergent nets at the
    // merge -> unprovable -> change nothing.
    let mut func = placement_shape();
    func.blocks[1].body = vec![let_alias(v(3), v(2))];
    func.blocks[1].terminator = ArcTerminator::Branch {
        cond: v(4),
        then_block: ArcBlockId::new(3),
        else_block: ArcBlockId::new(4),
    };
    func.var_reprs[4] = ValueRepr::Scalar;
    func.var_reprs.push(ValueRepr::Scalar); // %5
    func.var_types.push(Idx::from_raw(0));
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(3),
        params: Vec::new(),
        body: vec![ArcInstr::BurdenDec { var: v(3) }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(5),
            args: Vec::new(),
        },
    });
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(4),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(5),
            args: Vec::new(),
        },
    });
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(5),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Return { value: v(5) },
    });
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(repairs.is_empty(), "divergent per-path nets are unprovable");
}

#[test]
fn returned_lineage_declines() {
    // The result is RETURNED (RL-2 transfer to the caller) — out of scope.
    let mut func = placement_shape();
    func.blocks[1].body = vec![let_alias(v(3), v(2))];
    func.blocks[1].terminator = ArcTerminator::Return { value: v(3) };
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(
        repairs.is_empty(),
        "returned lineage is the caller's release"
    );
}

#[test]
fn store_consumed_member_declines() {
    // The result is moved into another aggregate (store family) — unmodeled.
    let mut func = placement_shape();
    func.var_reprs.push(ValueRepr::RcPointer); // %5 container
    func.var_types.push(Idx::from_raw(0));
    func.blocks[1].body = vec![
        let_alias(v(3), v(2)),
        ArcInstr::Construct {
            dst: v(5),
            ty: Idx::from_raw(0),
            ctor: CtorKind::ListLiteral,
            args: vec![v(3)],
        },
    ];
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(repairs.is_empty(), "store-consumed lineage is unmodeled");
}

#[test]
fn two_read_sinks_decline_placement() {
    // The result is read on BOTH arms of a branch (two execution-final
    // sinks): placement requires a unique sink — change nothing.
    let mut func = placement_shape();
    func.blocks[1].body = vec![let_alias(v(3), v(2))];
    func.blocks[1].terminator = ArcTerminator::Branch {
        cond: v(4),
        then_block: ArcBlockId::new(3),
        else_block: ArcBlockId::new(4),
    };
    func.var_reprs[4] = ValueRepr::Scalar;
    func.var_reprs.push(ValueRepr::Scalar); // %5
    func.var_types.push(Idx::from_raw(0));
    func.var_reprs.push(ValueRepr::Scalar); // %6
    func.var_types.push(Idx::from_raw(0));
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(3),
        params: Vec::new(),
        body: vec![borrowed_read(v(5), v(3))],
        terminator: ArcTerminator::Return { value: v(5) },
    });
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(4),
        params: Vec::new(),
        body: vec![borrowed_read(v(6), v(3))],
        terminator: ArcTerminator::Return { value: v(6) },
    });
    let repairs = run(&func, &forwarder_contracts(2));
    assert!(repairs.is_empty(), "two final-read sinks decline placement");
}

#[test]
fn iter_consuming_borrowed_read_declines() {
    // The borrowed read's callee param iter-consumes (RL-2 inward transfer
    // through a borrowed position) — the hidden release is unmodeled.
    let func = placement_shape();
    let mut contracts = forwarder_contracts(2);
    let mut len_c = MemoryContract::conservative(1);
    len_c.params[0].iter_consumes = true;
    contracts.insert(len_name(), len_c);
    let repairs = run(&func, &contracts);
    assert!(repairs.is_empty(), "iter-consuming read hides a release");
}

#[test]
fn apply_anchor_credits_in_its_own_block() {
    // The anchor as a BODY `Apply` (nounwind forwarder): the credit lands in
    // the same block; the repair still places the single release.
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0
        ValueRepr::RcPointer, // %1 result
        ValueRepr::RcPointer, // %2 alias
        ValueRepr::Scalar,    // %3 len
    ];
    let func = ArcFunction {
        var_types: vec![Idx::from_raw(0); 4],
        var_reprs,
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body: vec![
                construct_list(v(0)),
                ArcInstr::BurdenInc { var: v(0) },
                ArcInstr::BurdenDec { var: v(0) },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::from_raw(0),
                    func: fwd_name(),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned],
                    mono_instance_id: None,
                },
                let_alias(v(2), v(1)),
                borrowed_read(v(3), v(2)),
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    };
    let repairs = run(&func, &forwarder_contracts(1));
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 0,
            var: v(2)
        }],
        "Apply anchor: credit in its own block; one release after the read"
    );
}

#[test]
fn unwind_dec_is_not_a_value_read_sink() {
    // A pre-existing unwind-edge release (the Phase-6.5 emission) is an RC
    // op, NOT a value-read (TF-11): it balances the unwind path without
    // becoming a second sink, so the normal-path placement still fires.
    let mut func = placement_shape();
    func.var_reprs.push(ValueRepr::Scalar); // %5 second-call result
    func.var_types.push(Idx::from_raw(0));
    func.var_reprs.push(ValueRepr::Scalar); // %6 len result
    func.var_types.push(Idx::from_raw(0));
    func.blocks[1].body = vec![let_alias(v(3), v(2))];
    func.blocks[1].terminator = ArcTerminator::Invoke {
        dst: v(5),
        ty: Idx::from_raw(0),
        func: len_name(),
        args: Vec::new(),
        arg_ownership: Vec::new(),
        mono_instance_id: None,
        normal: ArcBlockId::new(3),
        unwind: ArcBlockId::new(4),
    };
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(3),
        params: Vec::new(),
        body: vec![borrowed_read(v(6), v(3))],
        terminator: ArcTerminator::Return { value: v(6) },
    });
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(4),
        params: Vec::new(),
        body: vec![ArcInstr::BurdenDec { var: v(3) }],
        terminator: ArcTerminator::Resume,
    });
    let repairs = run(&func, &forwarder_contracts(2));
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 3,
            var: v(3)
        }],
        "the unwind-block RC op is not a sink; the read block places the release"
    );
}

// WRAPPED transfer-anchor pins — the `Ok(m)` / `Some(m)` wrap-forwarder
// coupled lineage (Spec: Annex E §AIMS RL-1 + RL-2 + TF-4).

fn build_name() -> Name {
    Name::from_raw(44)
}

fn resume_block(n: u32) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(n),
        params: Vec::new(),
        body: Vec::new(),
        terminator: ArcTerminator::Resume,
    }
}

/// A WRAPPED contract over `n` params: each param is payload-contained on
/// every return path (`@wrap (m: T) -> Option<T> = Some(m)`).
fn wrapped_contract(n: usize, access: AccessClass) -> MemoryContract {
    let mut c = MemoryContract::conservative(n);
    for p in &mut c.params {
        p.transfers_through_return = false;
        p.access = access;
        p.return_alias = None;
        p.return_payload_contains_param = true;
        p.return_payload_contains_param_all_paths = true;
        p.iter_consumes = false;
    }
    c
}

/// A fresh-owned-result contract (`@build () -> [int]` — fresh Construct
/// returned: Unique + `preserves_freshness`, no params).
fn fresh_builder_contract() -> MemoryContract {
    let mut c = MemoryContract::conservative(0);
    c.return_info.uniqueness = crate::aims::lattice::Uniqueness::Unique;
    c.return_info.preserves_freshness = true;
    c
}

/// Contracts for the wrapped corpus replica: `@build` (fresh) + `@wrap`
/// (wrapped, per `access`).
fn wrapped_contracts(access: AccessClass) -> FxHashMap<Name, MemoryContract> {
    let mut m = FxHashMap::default();
    m.insert(build_name(), fresh_builder_contract());
    m.insert(fwd_name(), wrapped_contract(1, access));
    m
}

/// The wrapped BORROW corpus replica (`result_strmap` / `result_destructure`):
///
/// ```text
/// bb0: %0 = Invoke @build() normal bb1 unwind bb2     (fresh acquisition +1)
/// bb1: binc %0                                        (live-across keep-alive)
///      %1 = %0
///      %2 = Invoke @wrap(%1 [borrow]) normal bb3 unwind bb4   (credit +1)
/// bb3: %3 = %2; %4 = Project %3.1                     (the destructure)
///      Jump bb5(%4, %0, %2)
/// bb5: (%5 payload, %6 dead, %7 dead wrapper)
///      %8 = @len(%5 [borrow]); bdec %5; Return %9
/// ```
///
/// Net at Return: +1(fresh) +1(inc) +1(credit) -1(dec) = +2 — the wrapped
/// two-deficit; the combined repair removes the keep-alive inc AND places
/// one release after the final read.
fn wrapped_borrow_shape() -> (ArcFunction, Pool) {
    let mut pool = Pool::default();
    let list_int = pool.list(Idx::INT);
    let opt = pool.option(list_int);
    let var_types = vec![
        list_int, // %0 fresh payload
        list_int, // %1 alias
        opt,      // %2 wrapper result
        opt,      // %3 wrapper alias
        list_int, // %4 extraction
        list_int, // %5 payload block param
        list_int, // %6 dead block param
        opt,      // %7 dead wrapper block param
        Idx::INT, // %8 len
        Idx::INT, // %9 scalar return
    ];
    let var_reprs = vec![
        ValueRepr::RcPointer, // %0
        ValueRepr::RcPointer, // %1
        ValueRepr::Aggregate, // %2
        ValueRepr::Aggregate, // %3
        ValueRepr::RcPointer, // %4
        ValueRepr::RcPointer, // %5
        ValueRepr::RcPointer, // %6
        ValueRepr::Aggregate, // %7
        ValueRepr::Scalar,    // %8
        ValueRepr::Scalar,    // %9
    ];
    let func = ArcFunction {
        var_types,
        var_reprs,
        blocks: vec![
            ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: Vec::new(),
                terminator: ArcTerminator::Invoke {
                    dst: v(0),
                    ty: Idx::from_raw(0),
                    func: build_name(),
                    args: Vec::new(),
                    arg_ownership: Vec::new(),
                    mono_instance_id: None,
                    normal: ArcBlockId::new(1),
                    unwind: ArcBlockId::new(2),
                },
            },
            ArcBlock {
                id: ArcBlockId::new(1),
                params: Vec::new(),
                body: vec![ArcInstr::BurdenInc { var: v(0) }, let_alias(v(1), v(0))],
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::from_raw(0),
                    func: fwd_name(),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Borrowed],
                    mono_instance_id: None,
                    normal: ArcBlockId::new(3),
                    unwind: ArcBlockId::new(4),
                },
            },
            resume_block(2),
            ArcBlock {
                id: ArcBlockId::new(3),
                params: Vec::new(),
                body: vec![
                    let_alias(v(3), v(2)),
                    ArcInstr::Project {
                        dst: v(4),
                        ty: Idx::from_raw(0),
                        value: v(3),
                        field: 1,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: ArcBlockId::new(5),
                    args: vec![v(4), v(0), v(2)],
                },
            },
            resume_block(4),
            ArcBlock {
                id: ArcBlockId::new(5),
                params: vec![
                    (v(5), Idx::from_raw(0)),
                    (v(6), Idx::from_raw(0)),
                    (v(7), Idx::from_raw(0)),
                ],
                body: vec![borrowed_read(v(8), v(5)), ArcInstr::BurdenDec { var: v(5) }],
                terminator: ArcTerminator::Return { value: v(9) },
            },
        ],
        entry: ArcBlockId::new(0),
        name: Name::from_raw(0),
        ..Default::default()
    };
    (func, pool)
}

#[test]
fn wrapped_borrow_anchor_combined_repair_clears_the_coupled_lineage() {
    // The wrapped two-deficit (corpus members `result_strmap` /
    // `result_destructure`): the wrapper's carried credit AND the
    // live-across keep-alive inc both unreleased. The combined repair
    // removes the spurious inc and places ONE release after the final
    // read — every Return path nets 0, aliveness holds at every read.
    let (func, pool) = wrapped_borrow_shape();
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert_eq!(
        repairs,
        vec![
            CreditRepair::RemoveInc {
                block: 1,
                instr_idx: 0
            },
            CreditRepair::PlaceDecEndOfBody {
                block: 5,
                var: v(5)
            }
        ],
        "the coupled wrapper+payload lineage gets the combined repair"
    );
}

#[test]
fn wrapped_anchor_declines_without_all_paths_proof() {
    // OVER-FIRE: `@maybe_wrap (m) = if c then Ok(m) else Err(e)` — the
    // OR-joined containment bit fires but the per-path proof does not.
    // A static +1 credit would over-count on the non-wrapping path
    // (double-free); the admission must decline the whole site.
    let (func, pool) = wrapped_borrow_shape();
    let mut contracts = wrapped_contracts(AccessClass::Borrowed);
    if let Some(c) = contracts.get_mut(&fwd_name()) {
        c.params[0].return_payload_contains_param_all_paths = false;
    }
    let repairs = run_with_pool(&func, &contracts, &pool);
    assert!(
        repairs.is_empty(),
        "wrap-on-some-paths callees never admit (path-dependent credit)"
    );
}

#[test]
fn wrapped_anchor_declines_boxed_wrapper_result() {
    // A wrapper with its OWN allocation (RcPointer repr — a user struct
    // box) is TWO allocations with field-drop coupling — outside the
    // same-allocation proof; the admission must decline.
    let (mut func, pool) = wrapped_borrow_shape();
    func.var_reprs[2] = ValueRepr::RcPointer; // %2 boxed wrapper
    func.var_reprs[3] = ValueRepr::RcPointer;
    func.var_reprs[7] = ValueRepr::RcPointer;
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert!(
        repairs.is_empty(),
        "a boxed wrapper is not a same-allocation wrapper — decline"
    );
}

#[test]
fn wrapped_anchor_requires_contract_ir_access_agreement() {
    // Contract says Borrowed but the IR passes the payload `[own]` —
    // contract/IR disagreement declines the site (the hand-off semantics
    // would be double-counted against the borrow-mint credit).
    let (mut func, pool) = wrapped_borrow_shape();
    let ArcTerminator::Invoke { arg_ownership, .. } = &mut func.blocks[1].terminator else {
        panic!("bb1 terminator is the wrap Invoke");
    };
    arg_ownership[0] = ArgOwnership::Owned;
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert!(
        repairs.is_empty(),
        "IR [own] against a Borrowed wrap contract declines"
    );
}

#[test]
fn payload_live_after_early_release_keeps_its_genuine_inc() {
    // OVER-FIRE: the payload is READ after an early release in bb1 — the
    // keep-alive inc is GENUINE (removing it dead-uses the read: running
    // would hit 0 before the borrow). The combined repair must NOT remove
    // it; the placement-only repair covers the remaining +1.
    let (mut func, pool) = wrapped_borrow_shape();
    func.var_types.push(Idx::INT); // %10 len
    func.var_reprs.push(ValueRepr::Scalar);
    func.blocks[1].body = vec![
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(0) },
        borrowed_read(v(10), v(0)),
        let_alias(v(1), v(0)),
    ];
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 5,
            var: v(5)
        }],
        "a genuine keep-alive inc (protecting a read past an early \
         release) survives; only the release is placed"
    );
}

#[test]
fn wrapped_over_release_lineage_gets_no_repair() {
    // ADVERSARIAL coupled double-release: the dead-arm decs already
    // over-release the coupled lineage (net negative on the Return path).
    // The pass must change NOTHING — it never manufactures an inc and
    // never places a release into a negative net.
    let (mut func, pool) = wrapped_borrow_shape();
    func.blocks[5].body = vec![
        borrowed_read(v(8), v(5)),
        ArcInstr::BurdenDec { var: v(5) },
        ArcInstr::BurdenDec { var: v(6) },
        ArcInstr::BurdenDec { var: v(7) },
        ArcInstr::BurdenDec { var: v(5) },
    ];
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert!(
        repairs.is_empty(),
        "an over-released coupled lineage is unprovable — change nothing"
    );
}

#[test]
fn wrapped_rep_dead_arm_scalar_literal_member_stays_modeled() {
    // An unreachable match arm jump-threads a Scalar literal filler into
    // the merge param (the dead-arm phi placeholder) — it joins the rep
    // via positional renaming but is ledger-inert; the combined repair
    // still fires.
    let (mut func, pool) = wrapped_borrow_shape();
    func.var_types.push(Idx::INT); // %10 scalar filler
    func.var_reprs.push(ValueRepr::Scalar);
    func.blocks.push(ArcBlock {
        id: ArcBlockId::new(6),
        params: Vec::new(),
        body: vec![ArcInstr::Let {
            dst: v(10),
            ty: Idx::INT,
            value: ArcValue::Literal(crate::ir::LitValue::Int(0)),
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(5),
            args: vec![v(10), v(0), v(2)],
        },
    });
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert_eq!(
        repairs,
        vec![
            CreditRepair::RemoveInc {
                block: 1,
                instr_idx: 0
            },
            CreditRepair::PlaceDecEndOfBody {
                block: 5,
                var: v(5)
            }
        ],
        "a dead-arm scalar literal member is neutral, not an unmodeled use"
    );
}

#[test]
fn wrapped_owned_anchor_transfer_through_places_one_release() {
    // The W-owned flavor: the payload hands off `[own]` into the wrap
    // (RL-2 transfer-through, -1) and the wrapper carries it back out
    // (+1 credit) — net the acquisition's +1, released once after the
    // final read.
    let (mut func, pool) = wrapped_borrow_shape();
    func.blocks[1].body = vec![let_alias(v(1), v(0))];
    let ArcTerminator::Invoke { arg_ownership, .. } = &mut func.blocks[1].terminator else {
        panic!("bb1 terminator is the wrap Invoke");
    };
    arg_ownership[0] = ArgOwnership::Owned;
    let ArcTerminator::Jump { args, .. } = &mut func.blocks[3].terminator else {
        panic!("bb3 terminator is the merge Jump");
    };
    // The payload was consumed by the wrap — the merge threads the
    // extraction and the wrapper only.
    *args = vec![v(4), v(4), v(2)];
    // No release exists yet: the only -1 is the own hand-off, so the
    // acquisition's +1 survives to Return until the repair places one.
    func.blocks[5].body = vec![borrowed_read(v(8), v(5))];
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Owned), &pool);
    assert_eq!(
        repairs,
        vec![CreditRepair::PlaceDecEndOfBody {
            block: 5,
            var: v(5)
        }],
        "the owned wrap transfer-through nets -1+1; one placed release \
         clears the acquisition"
    );
}

#[test]
fn wrapped_combined_tie_break_removes_the_first_proven_inc() {
    // Two equivalently-proven removal candidates (the live-across inc and
    // a self-canceling pair's inc): each passes the joint all-paths +
    // aliveness proof with the identical placed release, so the canonical
    // pick is the smallest (block, instr_idx) — deterministic across runs.
    let (mut func, pool) = wrapped_borrow_shape();
    func.blocks[3].body = vec![
        let_alias(v(3), v(2)),
        ArcInstr::BurdenInc { var: v(0) },
        ArcInstr::BurdenDec { var: v(0) },
        ArcInstr::Project {
            dst: v(4),
            ty: Idx::from_raw(0),
            value: v(3),
            field: 1,
        },
    ];
    let repairs = run_with_pool(&func, &wrapped_contracts(AccessClass::Borrowed), &pool);
    assert_eq!(
        repairs,
        vec![
            CreditRepair::RemoveInc {
                block: 1,
                instr_idx: 0
            },
            CreditRepair::PlaceDecEndOfBody {
                block: 5,
                var: v(5)
            }
        ],
        "the smallest (block, instr_idx) proven inc is the canonical removal"
    );
}
