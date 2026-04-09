//! Unit tests for [`emit_terminator_rc`].
//!
//! Regression coverage for BUG-04-047 — duplicate terminator uses produced
//! zero `RcInc` emissions, creating a latent double-free when the same
//! RC-managed variable appeared multiple times in a terminator's
//! `used_vars()` multiset (e.g. `Jump { args: [v, v] }`). The formula
//! emitted by `emit_terminator_rc` is:
//!
//! ```text
//! RcInc(v) = (occurrences(v) + is_live_at_exit(v)).saturating_sub(1)
//! ```
//!
//! These tests pin the formula across every terminator shape that can
//! contain duplicated RC-managed uses: `Jump`, `Invoke`, `InvokeIndirect`,
//! plus single-use terminators (`Branch`, `Switch`, `Return`) and the
//! non-RC scalar negative pin.

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::forward_walk::emit_terminator_rc;
use crate::aims::emit_rc::helpers::{BlockCtx, LastUse};
use crate::aims::emit_rc::take_project::TakeMoveFacts;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::AimsState;
use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, ValueRepr,
};

// Fixture builder

/// Minimal fixture assembling `BlockCtx` + dependencies for a single-block
/// function. Callers supply the terminator and optional live-at-exit vars;
/// the fixture wires up an `ArcFunction` with heap-pointer RC-typed vars,
/// populates `var_reprs`, builds an `AimsStateMap` with the requested exit
/// liveness, and precomputes `use_info`.
struct TerminatorFixture {
    func: ArcFunction,
    state_map: AimsStateMap,
    pool: Pool,
    use_info: FxHashMap<ArcVarId, (usize, LastUse)>,
    defined_in_block: FxHashSet<ArcVarId>,
    borrowed_defs: FxHashSet<ArcVarId>,
    all_borrowed_defs: FxHashSet<ArcVarId>,
    project_borrowed_defs: FxHashSet<ArcVarId>,
    iter_element_defs: FxHashSet<ArcVarId>,
    inline_enum_projected_defs: FxHashSet<ArcVarId>,
    child_effective_last_use: FxHashMap<ArcVarId, LastUse>,
    take_move_facts: TakeMoveFacts,
}

impl TerminatorFixture {
    /// Build a fixture where `num_vars` variables are defined by the block
    /// (so `is_owned_at_entry` treats them as owned locals) with the given
    /// `terminator` and the given set of `live_at_exit_vars`. All vars use
    /// `Idx::STR` (a fat-pointer, RC-managed type) by default; callers can
    /// override by mutating `func.var_types` / `func.var_reprs`.
    fn new(
        num_vars: u32,
        terminator: ArcTerminator,
        live_at_exit: &[ArcVarId],
        body: Vec<ArcInstr>,
    ) -> Self {
        // `Idx::STR` gives `ValueRepr::FatValue` which routes through
        // `RcStrategy::FatPointer`. We want a heap-pointer strategy for
        // simplicity of the assertions, so use a `list<str>` type whose
        // repr is `RcPointer` → `RcStrategy::HeapPointer`.
        let mut pool = Pool::new();
        let list_str = pool.list(Idx::STR);
        let var_types: Vec<Idx> = (0..num_vars).map(|_| list_str).collect();
        let var_reprs: Vec<ValueRepr> = (0..num_vars).map(|_| ValueRepr::RcPointer).collect();

        let block = ArcBlock {
            id: ArcBlockId::new(0),
            params: Vec::new(),
            body,
            terminator,
        };
        let spans = vec![vec![None; block.body.len()]];

        let func = ArcFunction {
            name: Name::from_raw(1),
            params: Vec::new(),
            return_type: Idx::from_raw(0),
            blocks: vec![block],
            entry: ArcBlockId::new(0),
            var_types,
            var_reprs,
            spans,
            ..Default::default()
        };

        // All defined vars are considered locally-defined — satisfies
        // `is_owned_at_entry` Case 2 (cardinality Absent at entry + defined
        // in block + not in borrowed_defs).
        let defined_in_block: FxHashSet<ArcVarId> = (0..num_vars).map(ArcVarId::new).collect();

        // Pre-compute use_info the same way the real pipeline does.
        let use_info = crate::aims::emit_rc::helpers::precompute_block_uses(&func.blocks[0]);

        // Build an empty state map and populate exit liveness per caller spec.
        // `AimsState::FRESH` has `Cardinality::Once` so `is_live_at_exit`
        // returns true for any variable listed in `live_at_exit`.
        let mut state_map = AimsStateMap::new(&func);
        if !live_at_exit.is_empty() {
            let mut exit_map: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
            for &lv in live_at_exit {
                exit_map.insert(lv, AimsState::FRESH);
            }
            state_map.update_block_exit(ArcBlockId::new(0), exit_map);
        }

        Self {
            func,
            state_map,
            pool,
            use_info,
            defined_in_block,
            borrowed_defs: FxHashSet::default(),
            all_borrowed_defs: FxHashSet::default(),
            project_borrowed_defs: FxHashSet::default(),
            iter_element_defs: FxHashSet::default(),
            inline_enum_projected_defs: FxHashSet::default(),
            child_effective_last_use: FxHashMap::default(),
            take_move_facts: TakeMoveFacts::empty(),
        }
    }

    fn mark_scalar(&mut self, var: ArcVarId) {
        self.state_map.set_permanent_scalar(var);
        if let Some(slot) = self.func.var_reprs.get_mut(var.index()) {
            *slot = ValueRepr::Scalar;
        }
    }

    fn ctx(&self) -> BlockCtx<'_> {
        BlockCtx {
            func: &self.func,
            blk: ArcBlockId::new(0),
            state_map: &self.state_map,
            defined_in_block: &self.defined_in_block,
            borrowed_defs: &self.borrowed_defs,
            all_borrowed_defs: &self.all_borrowed_defs,
            project_borrowed_defs: &self.project_borrowed_defs,
            iter_element_defs: &self.iter_element_defs,
            inline_enum_projected_defs: &self.inline_enum_projected_defs,
            use_info: &self.use_info,
            pool: &self.pool,
            child_effective_last_use: &self.child_effective_last_use,
            take_move_facts: &self.take_move_facts,
        }
    }

    fn run(&self) -> Vec<ArcInstr> {
        let mut out = Vec::new();
        emit_terminator_rc(&self.ctx(), 0, &mut out);
        out
    }
}

/// Count `RcInc { var }` occurrences in the emitted body, summing per-var
/// counts across all `RcInc` instructions (an aggregate `RcInc { count: 2 }`
/// contributes 2 to the count).
fn count_inc(body: &[ArcInstr], target: ArcVarId) -> u32 {
    let mut total = 0;
    for instr in body {
        if let ArcInstr::RcInc { var, count, .. } = instr {
            if *var == target {
                total += count;
            }
        }
    }
    total
}

fn count_dec(body: &[ArcInstr], target: ArcVarId) -> usize {
    body.iter()
        .filter(|i| matches!(i, ArcInstr::RcDec { var, .. } if *var == target))
        .count()
}

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

// Jump — duplicate-use coverage (BUG-04-047 primary shape)

/// Regression: BUG-04-047 — `Jump { args: [v, v] }` with RC-typed target
/// params and `v` dead at exit previously emitted zero `RcInc`, producing
/// a latent double-free when the target block's per-param `RcDec` fired
/// against a single underlying reference.
#[test]
fn jump_with_duplicated_arg_emits_one_rc_inc() {
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0), v(0)],
    };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(
        count_inc(&body, v(0)),
        1,
        "expected exactly 1 RcInc for 2 duplicated jump args with dead-at-exit v"
    );
}

#[test]
fn jump_with_single_arg_dead_at_exit_emits_zero_rc_incs() {
    // Negative pin: rejects over-emission. Single use + dead at exit = transfer,
    // no inc needed.
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0)],
    };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 0);
}

#[test]
fn jump_with_single_arg_live_at_exit_emits_one_rc_inc() {
    // Single use + live at exit = need to hand off to successor's liveness.
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0)],
    };
    let fx = TerminatorFixture::new(1, term, &[v(0)], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1);
}

#[test]
fn jump_with_triple_duplicated_arg_dead_at_exit_emits_two_rc_incs() {
    // k=3, live=0 → (3 + 0).saturating_sub(1) = 2.
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0), v(0), v(0)],
    };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 2);
}

#[test]
fn jump_with_triple_duplicated_arg_live_at_exit_emits_three_rc_incs() {
    // k=3, live=1 → (3 + 1).saturating_sub(1) = 3.
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0), v(0), v(0)],
    };
    let fx = TerminatorFixture::new(1, term, &[v(0)], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 3);
}

#[test]
fn jump_with_interleaved_args_counts_each_var_independently() {
    // args: [v, w, v] — v appears twice (→ 1 inc), w appears once (→ 0 inc).
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0), v(1), v(0)],
    };
    let fx = TerminatorFixture::new(2, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1, "v appears twice → 1 inc");
    assert_eq!(count_inc(&body, v(1)), 0, "w appears once → 0 inc");
}

// Return

#[test]
fn return_single_value_dead_at_exit_emits_zero_rc_incs() {
    let term = ArcTerminator::Return { value: v(0) };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 0);
}

#[test]
fn return_single_value_live_at_exit_emits_one_rc_inc() {
    let term = ArcTerminator::Return { value: v(0) };
    let fx = TerminatorFixture::new(1, term, &[v(0)], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1);
}

// Invoke — duplicate coverage across owned and mixed ownership

#[test]
fn invoke_with_duplicated_owned_args_emits_one_rc_inc() {
    let term = ArcTerminator::Invoke {
        dst: v(1),
        ty: Idx::from_raw(0),
        func: Name::from_raw(2),
        args: vec![v(0), v(0)],
        arg_ownership: vec![ArgOwnership::Owned, ArgOwnership::Owned],
        normal: ArcBlockId::new(1),
        unwind: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(2, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1);
}

#[test]
fn invoke_with_triple_duplicated_mixed_ownership_args_emits_two_rc_incs() {
    // Ownership side is irrelevant on the Inc side (matches body-walk semantics):
    // k=3, live=0 → 2 incs.
    let term = ArcTerminator::Invoke {
        dst: v(1),
        ty: Idx::from_raw(0),
        func: Name::from_raw(2),
        args: vec![v(0), v(0), v(0)],
        arg_ownership: vec![
            ArgOwnership::Owned,
            ArgOwnership::Borrowed,
            ArgOwnership::Owned,
        ],
        normal: ArcBlockId::new(1),
        unwind: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(2, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 2);
}

// InvokeIndirect — closure-tail duplicate (codex matrix gap)

#[test]
fn invoke_indirect_with_closure_equal_to_arg_counts_closure_as_duplicate() {
    // `used_vars()` for InvokeIndirect returns [...args, closure]. When
    // closure == args[0], the var appears twice → k=2, live=0 → 1 inc.
    let term = ArcTerminator::InvokeIndirect {
        dst: v(1),
        ty: Idx::from_raw(0),
        closure: v(0),
        args: vec![v(0)],
        arg_ownership: vec![ArgOwnership::Owned],
        normal: ArcBlockId::new(1),
        unwind: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(2, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1);
}

// Branch — single-use, verifies no regression for cond liveness path

#[test]
fn branch_with_dead_cond_emits_dec_and_zero_inc() {
    let term = ArcTerminator::Branch {
        cond: v(0),
        then_block: ArcBlockId::new(1),
        else_block: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 0, "dead cond → no inc");
    assert_eq!(count_dec(&body, v(0)), 1, "dead cond → one dec");
}

#[test]
fn branch_with_live_cond_emits_one_inc_and_no_dec() {
    let term = ArcTerminator::Branch {
        cond: v(0),
        then_block: ArcBlockId::new(1),
        else_block: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(1, term, &[v(0)], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 1, "live cond → inc");
    assert_eq!(count_dec(&body, v(0)), 0, "live cond → no dec");
}

// Switch — same as Branch single-use path

#[test]
fn switch_with_dead_scrutinee_emits_dec_and_zero_inc() {
    let term = ArcTerminator::Switch {
        scrutinee: v(0),
        cases: vec![(0, ArcBlockId::new(1))],
        default: ArcBlockId::new(2),
    };
    let fx = TerminatorFixture::new(1, term, &[], Vec::new());
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 0);
    assert_eq!(count_dec(&body, v(0)), 1);
}

// Scalar negative pin

#[test]
fn jump_with_scalar_duplicated_arg_emits_zero_rc_incs() {
    // Negative pin: scalars are filtered out by `is_owned_at_entry`
    // → no inc regardless of occurrence count.
    let term = ArcTerminator::Jump {
        target: ArcBlockId::new(1),
        args: vec![v(0), v(0)],
    };
    let mut fx = TerminatorFixture::new(1, term, &[], Vec::new());
    fx.mark_scalar(v(0));
    let body = fx.run();
    assert_eq!(count_inc(&body, v(0)), 0);
}
