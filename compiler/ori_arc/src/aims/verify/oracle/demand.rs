//! Path-sensitive realized demand over SSA definitions and CFG alternatives.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::demand::RawDemand;
use crate::aims::lattice::Cardinality;
use crate::aims::lattice::Consumption;
use crate::aims::transfer::{backward_demands, backward_terminator_demands, BackwardDemand};
use crate::graph::{compute_predecessors, successor_block_ids};
use crate::ir::{ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, ValueRepr};

/// Identity of one realized semantic endpoint: the exact IR site and operand
/// slot. Alias definitions explicitly re-anchor demand when their calculus
/// rule introduces a new endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DemandId {
    block: usize,
    instruction: usize,
    slot: usize,
}

impl DemandId {
    fn at(block: usize, instruction: usize, slot: usize) -> Self {
        Self {
            block,
            instruction,
            slot,
        }
    }
}

type DemandShape = RawDemand;

/// Demand shapes possible for one SSA variable from one program point.
///
/// Keeping singleton endpoint identities distinguishes two sequential uses
/// from mutually exclusive one-use paths. The shape remains two-dimensional:
/// one projection endpoint can therefore carry `Many + Affine` without
/// reconstructing consumption from cardinality. `multiple` retains the exact
/// componentwise `seq_add` result for paths with multiple endpoints; this is
/// necessary because `Absent + Once = Once` while `Affine + Linear =
/// Unrestricted`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DemandPaths {
    has_empty: bool,
    singletons: FxHashMap<DemandId, DemandShape>,
    multiple: Option<DemandShape>,
}

impl DemandPaths {
    fn empty() -> Self {
        Self {
            has_empty: true,
            singletons: FxHashMap::default(),
            multiple: None,
        }
    }

    fn one(endpoint: DemandId, shape: DemandShape) -> Self {
        Self {
            has_empty: false,
            singletons: [(endpoint, shape)].into_iter().collect(),
            multiple: None,
        }
    }

    fn multiple(shape: DemandShape) -> Self {
        Self {
            has_empty: false,
            singletons: FxHashMap::default(),
            multiple: Some(shape),
        }
    }

    fn alternatives<'a>(paths: impl Iterator<Item = &'a Self>) -> Self {
        let mut joined = Self {
            has_empty: false,
            singletons: FxHashMap::default(),
            multiple: None,
        };
        for path in paths {
            joined.has_empty |= path.has_empty;
            for (&endpoint, &shape) in &path.singletons {
                joined.add_singleton_alternative(endpoint, shape);
            }
            if let Some(shape) = path.multiple {
                joined.add_multiple_alternative(shape);
            }
        }
        joined
    }

    fn sequential(&self, other: &Self) -> Self {
        let mut combined = Self {
            has_empty: self.has_empty && other.has_empty,
            singletons: FxHashMap::default(),
            multiple: None,
        };

        if other.has_empty {
            for (&endpoint, &shape) in &self.singletons {
                combined.add_singleton_alternative(endpoint, shape);
            }
            if let Some(shape) = self.multiple {
                combined.add_multiple_alternative(shape);
            }
        }
        if self.has_empty {
            for (&endpoint, &shape) in &other.singletons {
                combined.add_singleton_alternative(endpoint, shape);
            }
            if let Some(shape) = other.multiple {
                combined.add_multiple_alternative(shape);
            }
        }

        for (&endpoint, &left) in &self.singletons {
            for (&other_endpoint, &right) in &other.singletons {
                if endpoint == other_endpoint {
                    combined.add_singleton_alternative(endpoint, left.alt_join(right));
                } else {
                    combined.add_multiple_alternative(left.seq_add(right));
                }
            }
        }

        if let Some(left) = self.multiple {
            for &right in other.singletons.values() {
                combined.add_multiple_alternative(left.seq_add(right));
            }
        }
        if let Some(right) = other.multiple {
            for &left in self.singletons.values() {
                combined.add_multiple_alternative(left.seq_add(right));
            }
        }
        if let (Some(left), Some(right)) = (self.multiple, other.multiple) {
            combined.add_multiple_alternative(left.seq_add(right));
        }

        combined
    }

    fn aggregate(&self) -> DemandShape {
        let mut aggregate = self.multiple;
        for &shape in self.singletons.values() {
            aggregate = Some(aggregate.map_or(shape, |current| current.alt_join(shape)));
        }
        aggregate.unwrap_or(DemandShape::ZERO)
    }

    fn has_demand(&self) -> bool {
        self.multiple.is_some() || !self.singletons.is_empty()
    }

    fn contains_endpoint(&self, endpoint: DemandId) -> bool {
        self.multiple.is_some() || self.singletons.contains_key(&endpoint)
    }

    fn repeated_shape(&self, endpoint: DemandId, shape: DemandShape) -> DemandShape {
        self.singletons
            .get(&endpoint)
            .copied()
            .or(self.multiple)
            .unwrap_or(shape)
            .seq_add(shape)
    }

    fn add_singleton_alternative(&mut self, endpoint: DemandId, shape: DemandShape) {
        self.singletons
            .entry(endpoint)
            .and_modify(|current| *current = current.alt_join(shape))
            .or_insert(shape);
    }

    fn add_multiple_alternative(&mut self, shape: DemandShape) {
        self.multiple = Some(
            self.multiple
                .map_or(shape, |current| current.alt_join(shape)),
        );
    }

    fn observe(&mut self) {
        let mut gained_empty_path = false;
        self.singletons.retain(|_, shape| {
            *shape = (*shape).observe();
            let remains_live = *shape != DemandShape::ZERO;
            gained_empty_path |= !remains_live;
            remains_live
        });
        if let Some(shape) = self.multiple {
            let canonical = shape.observe();
            if canonical == DemandShape::ZERO {
                self.multiple = None;
                gained_empty_path = true;
            } else {
                self.multiple = Some(canonical);
            }
        }
        self.has_empty |= gained_empty_path;
    }
}

type DemandState = FxHashMap<ArcVarId, DemandPaths>;

pub(super) fn derive_param_consumptions(func: &ArcFunction) -> Vec<Consumption> {
    derive_param_demands(func)
        .into_iter()
        .map(|demand| demand.consumption)
        .collect()
}

/// Return both independently-derived demand dimensions for oracle tests.
#[cfg(test)]
pub(super) fn derive_param_demand_dimensions(
    func: &ArcFunction,
) -> Vec<(Cardinality, Consumption)> {
    derive_param_demands(func)
        .into_iter()
        .map(|demand| (demand.cardinality, demand.consumption))
        .collect()
}

fn derive_param_demands(func: &ArcFunction) -> Vec<DemandShape> {
    if func.blocks.is_empty() {
        return vec![DemandShape::ZERO; func.params.len()];
    }

    let cyclic = cyclic_blocks(func);
    let predecessors = compute_predecessors(func);
    let mut entries = vec![DemandState::default(); func.blocks.len()];
    let mut pending = (0..func.blocks.len()).collect::<Vec<_>>();
    let mut queued = vec![true; func.blocks.len()];

    while let Some(block) = pending.pop() {
        queued[block] = false;
        let updated = analyze_block(func, block, &entries, cyclic[block]);
        if updated == entries[block] {
            continue;
        }
        entries[block] = updated;
        for &predecessor in &predecessors[block] {
            if !queued[predecessor] {
                queued[predecessor] = true;
                pending.push(predecessor);
            }
        }
    }

    let entry = entries.get(func.entry.index());
    func.params
        .iter()
        .map(|param| {
            entry
                .and_then(|state| state.get(&param.var))
                .map_or(DemandShape::ZERO, DemandPaths::aggregate)
                .observe()
        })
        .collect()
}

fn analyze_block(
    func: &ArcFunction,
    block: usize,
    entries: &[DemandState],
    cyclic: bool,
) -> DemandState {
    let arc_block = &func.blocks[block];
    let mut current = successor_alternatives(func, block, entries);

    if let ArcTerminator::Invoke { dst, .. } | ArcTerminator::InvokeIndirect { dst, .. } =
        &arc_block.terminator
    {
        current.remove(dst);
    }
    let loop_back = current.clone();
    if !matches!(arc_block.terminator, ArcTerminator::Jump { .. }) {
        for (slot, demand) in backward_terminator_demands(&arc_block.terminator)
            .into_iter()
            .enumerate()
        {
            add_backward_demand(
                &mut current,
                demand,
                DemandId::at(block, usize::MAX, slot),
                cyclic,
                &loop_back,
            );
        }
    }

    let transfer = BlockTransferContext {
        func,
        block,
        cyclic,
        loop_back: &loop_back,
    };
    for (instruction, instr) in arc_block.body.iter().enumerate().rev() {
        transfer_instruction_demands(&transfer, instruction, instr, &mut current);
    }
    current.retain(|_, paths| {
        paths.observe();
        paths.has_demand()
    });
    current
}

struct BlockTransferContext<'a> {
    func: &'a ArcFunction,
    block: usize,
    cyclic: bool,
    loop_back: &'a DemandState,
}

/// Apply the oracle's independently-derived transfer for one instruction.
fn transfer_instruction_demands(
    context: &BlockTransferContext<'_>,
    instruction: usize,
    instr: &ArcInstr,
    current: &mut DemandState,
) {
    if let Some(dst) = instr.defined_var() {
        let downstream = current.remove(&dst);
        match instr {
            ArcInstr::Let {
                value: ArcValue::Var(source),
                ..
            } => {
                if let Some(downstream) = downstream {
                    merge_sequential(current, *source, &downstream);
                }
            }
            ArcInstr::Project { dst, value, .. } => {
                let destination =
                    if context.func.var_reprs.get(dst.index()) == Some(&ValueRepr::Scalar) {
                        let live = downstream
                            .as_ref()
                            .is_some_and(|paths| paths.aggregate().observe() != DemandShape::ZERO);
                        if live {
                            DemandShape::new(Cardinality::Once, Consumption::Affine)
                        } else {
                            DemandShape::ZERO.projected()
                        }
                    } else {
                        downstream
                            .as_ref()
                            .map_or(DemandShape::ZERO, DemandPaths::aggregate)
                            .projected()
                    };
                add_endpoint_demand(
                    current,
                    *value,
                    DemandId::at(context.block, instruction, 0),
                    destination,
                    context.cyclic,
                    context.loop_back,
                );
            }
            ArcInstr::Select {
                true_val,
                false_val,
                ..
            } => {
                if let Some(downstream) = downstream {
                    let shape = downstream.aggregate();
                    add_endpoint_demand(
                        current,
                        *true_val,
                        DemandId::at(context.block, instruction, 1),
                        shape,
                        context.cyclic,
                        context.loop_back,
                    );
                    add_endpoint_demand(
                        current,
                        *false_val,
                        DemandId::at(context.block, instruction, 2),
                        shape,
                        context.cyclic,
                        context.loop_back,
                    );
                }
            }
            _ => {}
        }
    }

    if let ArcInstr::PartialApply { args, .. } = instr {
        for (slot, &var) in args.iter().enumerate() {
            add_endpoint_demand(
                current,
                var,
                DemandId::at(context.block, instruction, slot),
                DemandShape::LINEAR_ONCE,
                context.cyclic,
                context.loop_back,
            );
        }
    } else {
        for (slot, demand) in backward_demands(instr).into_iter().enumerate() {
            add_backward_demand(
                current,
                demand,
                DemandId::at(context.block, instruction, slot),
                context.cyclic,
                context.loop_back,
            );
        }
    }
}

fn successor_alternatives(
    func: &ArcFunction,
    block: usize,
    entries: &[DemandState],
) -> DemandState {
    let successors = successor_block_ids(&func.blocks[block].terminator);
    if successors.is_empty() {
        return DemandState::default();
    }

    let states = successors
        .iter()
        .filter_map(|successor| edge_state(func, block, successor.index(), entries))
        .collect::<Vec<_>>();
    let keys = states
        .iter()
        .flat_map(|state| state.keys().copied())
        .collect::<FxHashSet<_>>();
    let empty = DemandPaths::empty();
    keys.into_iter()
        .filter_map(|var| {
            let joined = DemandPaths::alternatives(
                states.iter().map(|state| state.get(&var).unwrap_or(&empty)),
            );
            joined.has_demand().then_some((var, joined))
        })
        .collect()
}

fn edge_state(
    func: &ArcFunction,
    block: usize,
    successor: usize,
    entries: &[DemandState],
) -> Option<DemandState> {
    let mut state = entries.get(successor)?.clone();
    if let ArcTerminator::Jump { target, args } = &func.blocks[block].terminator {
        if target.index() == successor {
            for (&arg, &(param, _)) in args.iter().zip(func.blocks[successor].params.iter()) {
                if let Some(demand) = state.remove(&param) {
                    merge_sequential(&mut state, arg, &demand);
                }
            }
        }
    }
    Some(state)
}

fn add_backward_demand(
    state: &mut DemandState,
    demand: BackwardDemand,
    endpoint: DemandId,
    cyclic: bool,
    loop_back: &DemandState,
) {
    add_endpoint_demand(
        state,
        demand.var,
        endpoint,
        DemandShape::new(demand.cardinality, demand.consumption),
        cyclic,
        loop_back,
    );
}

fn add_endpoint_demand(
    state: &mut DemandState,
    var: ArcVarId,
    endpoint: DemandId,
    shape: DemandShape,
    cyclic: bool,
    loop_back: &DemandState,
) {
    let repeated_shape = if cyclic {
        loop_back
            .get(&var)
            .filter(|paths| paths.contains_endpoint(endpoint))
            .map(|paths| paths.repeated_shape(endpoint, shape))
    } else {
        None
    };
    let demand =
        repeated_shape.map_or_else(|| DemandPaths::one(endpoint, shape), DemandPaths::multiple);
    merge_sequential(state, var, &demand);
}

fn merge_sequential(state: &mut DemandState, var: ArcVarId, demand: &DemandPaths) {
    let combined = state
        .remove(&var)
        .map_or_else(|| demand.clone(), |current| current.sequential(demand));
    state.insert(var, combined);
}

fn cyclic_blocks(func: &ArcFunction) -> Vec<bool> {
    (0..func.blocks.len())
        .map(|start| {
            let mut seen = FxHashSet::default();
            let mut pending = successor_block_ids(&func.blocks[start].terminator)
                .into_iter()
                .map(crate::ir::ArcBlockId::index)
                .collect::<Vec<_>>();
            while let Some(block) = pending.pop() {
                if block == start {
                    return true;
                }
                if block >= func.blocks.len() || !seen.insert(block) {
                    continue;
                }
                pending.extend(
                    successor_block_ids(&func.blocks[block].terminator)
                        .into_iter()
                        .map(crate::ir::ArcBlockId::index),
                );
            }
            false
        })
        .collect()
}
