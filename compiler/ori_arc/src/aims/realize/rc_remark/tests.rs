//! Relocation-regression pin: `emit_survivor_remarks_all_kept` is callable
//! from `aims::realize::rc_remark::survivor_walk`, and its walk + JSONL
//! formatting behave identically to the pinned baseline below.
//!
//! `ORI_RC_REMARKS` gates emission behind a process-wide `LazyLock` shared
//! with every other test in this binary (including the pipeline tests that
//! now call `run_aims_pipeline` unconditionally); the gate cannot be
//! reliably forced from a single `#[test]` in a parallel test binary. This
//! pin instead exercises the two relocated units directly: the walk logic
//! (via the always-callable `emit_survivor_remarks_all_kept` entry point,
//! smoke-tested for panic-freedom regardless of gate state) and the JSONL
//! record shape (`AimsRcRemark::to_jsonl`, gate-independent).

use ori_types::Idx;

use super::emit_survivor_remarks_all_kept;
use super::RC_SCHEMA_VERSION;
use super::{AimsRcRemark, Cause, LatticeDim, RcRemarkOp, RcRemarkStreamEnvelope, RemarkKind};
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// One-block function with a single `BurdenInc` on var 0, returning it.
fn burden_inc_func() -> ArcFunction {
    let mut function = ArcFunction {
        var_types: vec![Idx::from_raw(0)],
        blocks: vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::BurdenInc { var: v(0) }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        ..Default::default()
    };
    function.replace_variable_representations(vec![ValueRepr::RcPointer]);
    function
}

/// Relocation pin: `emit_survivor_remarks_all_kept` is reachable from
/// `aims::realize::rc_remark::survivor_walk` and does not panic walking a
/// function carrying a `BurdenInc`, regardless of the `ORI_RC_REMARKS`
/// gate's process-wide state.
#[test]
fn emit_survivor_remarks_all_kept_walks_burden_inc_without_panic() {
    let func = burden_inc_func();
    let state_map = AimsStateMap::new(&func);
    let interner = ori_ir::StringInterner::new();
    emit_survivor_remarks_all_kept(&func, &state_map, &interner);
}

/// Header envelope shape pin: the emitted first line carries the current
/// schema version, the `record`/`schema_version`/`compiler_sha`/`source_file`
/// fields, and not the retired path label. Field PRESENCE is asserted, not
/// exhaustiveness — an unrelated added field would pass.
///
/// The negative half is the load-bearing one. A consumer distinguishes stream
/// generations by `schema_version` alone, so emitting a retired field under a
/// bumped version, or a bumped field set under an unchanged version, silently
/// desynchronizes every reader.
#[test]
fn stream_header_to_jsonl_carries_current_schema_and_no_retired_field() {
    let envelope = RcRemarkStreamEnvelope::new("000de0232".to_string(), "survivor.ori".to_string());
    let json = envelope.to_jsonl();

    assert!(json.contains("\"record\":\"header\""), "json = {json}");
    assert!(
        json.contains(&format!("\"schema_version\":{RC_SCHEMA_VERSION}")),
        "header declares the current schema version\njson = {json}"
    );
    assert_eq!(RC_SCHEMA_VERSION, 2, "current wire generation");
    assert!(
        json.contains("\"compiler_sha\":\"000de0232\""),
        "json = {json}"
    );
    assert!(
        json.contains("\"source_file\":\"survivor.ori\""),
        "json = {json}"
    );

    assert!(
        !json.contains("burden_path"),
        "the retired path label must not appear in a schema-2 header\njson = {json}"
    );
}

/// JSONL record shape pin: `AimsRcRemark::to_jsonl` emits the
/// `kind`/`pass`/`name`/`rc_op`/`cause` field set, with null optional
/// fields rendered as JSON `null`.
#[test]
fn survivor_remark_to_jsonl_matches_pre_relocation_shape() {
    let remark = AimsRcRemark {
        kind: RemarkKind::Missed,
        pass: "aims-burden-elim",
        name: "rc-inc-not-elided",
        rc_op: RcRemarkOp::Inc,
        function: Some("main".to_string()),
        debug_loc: None,
        ssa_value: 0,
        exit_block: None,
        cause: Cause {
            proof_failure: "burden_inc_kept_by_whole_var_disposition".to_string(),
            lattice_dim: LatticeDim::Cardinality,
            detail: "consumption=Linear locality=BlockLocal cardinality=Many".to_string(),
        },
        burden_net: None,
        args: vec!["def_kind=fresh".to_string(), "var_repr=heap".to_string()],
        cow_mode: None,
    };
    let json = remark.to_jsonl();
    assert!(json.contains("\"kind\":\"missed\""), "json = {json}");
    assert!(
        json.contains("\"pass\":\"aims-burden-elim\""),
        "json = {json}"
    );
    assert!(json.contains("\"rc_op\":\"burden_inc\""), "json = {json}");
    assert!(json.contains("\"function\":\"main\""), "json = {json}");
    assert!(json.contains("\"debug_loc\":null"), "json = {json}");
    assert!(
        json.contains("\"proof_failure\":\"burden_inc_kept_by_whole_var_disposition\""),
        "json = {json}"
    );
    assert!(
        json.contains("\"lattice_dim\":\"cardinality\""),
        "json = {json}"
    );
    assert!(json.contains("\"burden_net\":null"), "json = {json}");
    assert!(json.contains("\"cow_mode\":null"), "json = {json}");
}
