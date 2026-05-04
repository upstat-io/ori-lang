//! Consumer-level tests for [`emit_dead_at_entry_decs`].
//!
//! The 5 unit tests for `let_alias_rep` in `take_project/tests.rs` only
//! exercise the helper layer. These tests exercise the full
//! `emit_dead_at_entry_decs` path with synthetic `BlockCtx` topologies
//! to verify that the `let_alias_rep` dedup produces the correct number
//! of `RcDec` instructions.

use ori_ir::Name;
use ori_types::{Idx, Pool};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::emit_rc::helpers::{BlockCtx, LastUse};
use crate::aims::emit_rc::take_project::TakeMoveFacts;
use crate::aims::intraprocedural::state_map::AimsStateMap;
use crate::aims::lattice::AimsState;
use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcVarId, ValueRepr};

use super::emit_dead_at_entry_decs;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Build a minimal two-block function where block 1 receives two
/// phi-merged params. Used to test that `emit_dead_at_entry_decs`
/// correctly deduplicates (or not) based on `let_alias_rep`.
fn make_two_block_func(pool: &mut Pool) -> ArcFunction {
    let list_str = pool.list(Idx::STR);
    // Block 0: jumps to block 1 with args [v(0), v(1)]
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: vec![],
        body: vec![],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: vec![v(0), v(1)],
        },
    };
    // Block 1: receives two params (v(2), v(3)), returns v(2)
    let block1 = ArcBlock {
        id: ArcBlockId::new(1),
        params: vec![(v(2), list_str), (v(3), list_str)],
        body: vec![],
        terminator: ArcTerminator::Return { value: v(2) },
    };
    let spans = vec![vec![]; 2];
    ArcFunction {
        name: Name::from_raw(1),
        params: vec![],
        return_type: list_str,
        blocks: vec![block0, block1],
        entry: ArcBlockId::new(0),
        var_types: vec![list_str; 4],
        var_reprs: vec![ValueRepr::RcPointer; 4],
        spans,
        ..Default::default()
    }
}

/// Fixture for building a `BlockCtx` with all the empty sets wired up.
struct DeadCleanupFixture {
    func: ArcFunction,
    state_map: AimsStateMap,
    pool: Pool,
    use_info: FxHashMap<ArcVarId, (usize, LastUse)>,
    take_move_facts: TakeMoveFacts,
    defined_in_block: FxHashSet<ArcVarId>,
    borrowed_defs: FxHashSet<ArcVarId>,
    all_borrowed_defs: FxHashSet<ArcVarId>,
    project_borrowed_defs: FxHashSet<ArcVarId>,
    iter_element_defs: FxHashSet<ArcVarId>,
    inline_enum_projected_defs: FxHashSet<ArcVarId>,
    child_effective_last_use: FxHashMap<ArcVarId, LastUse>,
    return_transfer_params: FxHashSet<ArcVarId>,
    alias_to_param: FxHashMap<ArcVarId, FxHashSet<usize>>,
    return_project_inc_targets: FxHashMap<ArcVarId, crate::ir::RcStrategy>,
}

impl DeadCleanupFixture {
    fn ctx(&self, blk_idx: u32) -> BlockCtx<'_> {
        BlockCtx {
            func: &self.func,
            blk: ArcBlockId::new(blk_idx),
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
            return_transfer_params: &self.return_transfer_params,
            alias_to_param: &self.alias_to_param,
            return_project_inc_targets: &self.return_project_inc_targets,
        }
    }
}

/// Semantic pin: two phi-merged block params with DIFFERENT let-alias
/// reps must each get their own `RcDec`. If `let_alias_rep` dedup is
/// reverted to `lineage_of`, this test fails (only 1 dec instead of 2).
///
/// Topology: block 1 has params v(2) and v(3). Both are in the same
/// take-project class (same lineage), and block 1 is a bypass-safe
/// entry. v(2) maps to let-alias rep v(2), v(3) maps to let-alias rep
/// v(3) — different reps because they hold different runtime values
/// (swapped phi). Neither is used in block 1 body, and neither is live
/// at exit. So both should get `RcDec`.
#[test]
fn swapped_phi_params_emit_two_rc_decs() {
    let mut pool = Pool::new();
    let func = make_two_block_func(&mut pool);

    let mut state_map = AimsStateMap::new(&func);
    let mut entry_states: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    entry_states.insert(v(2), AimsState::FRESH);
    entry_states.insert(v(3), AimsState::FRESH);
    state_map.update_block_entry(ArcBlockId::new(1), entry_states);

    let fixture = DeadCleanupFixture {
        func,
        state_map,
        pool,
        use_info: FxHashMap::default(),
        take_move_facts: TakeMoveFacts::with_bypass_entries(
            &[v(2), v(3)],
            &[(v(2), v(2)), (v(3), v(3))], // different reps
            &[1],
        ),
        defined_in_block: FxHashSet::default(),
        borrowed_defs: FxHashSet::default(),
        all_borrowed_defs: FxHashSet::default(),
        project_borrowed_defs: FxHashSet::default(),
        iter_element_defs: FxHashSet::default(),
        inline_enum_projected_defs: FxHashSet::default(),
        child_effective_last_use: FxHashMap::default(),
        return_transfer_params: FxHashSet::default(),
        alias_to_param: FxHashMap::default(),
        return_project_inc_targets: FxHashMap::default(),
    };

    let ctx = fixture.ctx(1);
    let mut new_body = Vec::new();
    let (_deferred, _merge_edge) = emit_dead_at_entry_decs(&ctx, &mut new_body);

    let rc_decs: Vec<_> = new_body
        .iter()
        .filter(|instr| matches!(instr, ArcInstr::RcDec { .. }))
        .collect();
    assert_eq!(
        rc_decs.len(),
        2,
        "swapped phi params with different let-alias reps must each get RcDec, got: {rc_decs:?}"
    );
}

/// Negative pin: two vars that ARE true Let aliases (same let-alias rep)
/// should produce only ONE `RcDec`. Emitting two would double-free.
#[test]
fn let_alias_siblings_emit_one_rc_dec() {
    let mut pool = Pool::new();
    let func = make_two_block_func(&mut pool);

    let mut state_map = AimsStateMap::new(&func);
    let mut entry_states: FxHashMap<ArcVarId, AimsState> = FxHashMap::default();
    entry_states.insert(v(2), AimsState::FRESH);
    entry_states.insert(v(3), AimsState::FRESH);
    state_map.update_block_entry(ArcBlockId::new(1), entry_states);

    let fixture = DeadCleanupFixture {
        func,
        state_map,
        pool,
        use_info: FxHashMap::default(),
        take_move_facts: TakeMoveFacts::with_bypass_entries(
            &[v(2), v(3)],
            &[(v(2), v(2)), (v(3), v(2))], // same rep → true aliases
            &[1],
        ),
        defined_in_block: FxHashSet::default(),
        borrowed_defs: FxHashSet::default(),
        all_borrowed_defs: FxHashSet::default(),
        project_borrowed_defs: FxHashSet::default(),
        iter_element_defs: FxHashSet::default(),
        inline_enum_projected_defs: FxHashSet::default(),
        child_effective_last_use: FxHashMap::default(),
        return_transfer_params: FxHashSet::default(),
        alias_to_param: FxHashMap::default(),
        return_project_inc_targets: FxHashMap::default(),
    };

    let ctx = fixture.ctx(1);
    let mut new_body = Vec::new();
    let (_deferred, _merge_edge) = emit_dead_at_entry_decs(&ctx, &mut new_body);

    let rc_decs: Vec<_> = new_body
        .iter()
        .filter(|instr| matches!(instr, ArcInstr::RcDec { .. }))
        .collect();
    assert_eq!(
        rc_decs.len(),
        1,
        "Let-alias siblings (same rep) must produce only one RcDec to avoid double-free, got: {rc_decs:?}"
    );
}
