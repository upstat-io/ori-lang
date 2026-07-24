//! Private raw-demand carrier for one backward block walk.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::demand::RawDemand;
use crate::aims::lattice::{AccessClass, AimsState, Locality, Uniqueness};
use crate::ir::ArcVarId;

#[derive(Clone, Copy, Debug)]
struct PendingState {
    facts: AimsState,
    demand: RawDemand,
}

impl PendingState {
    fn from_observed(state: AimsState) -> Self {
        Self {
            facts: state,
            demand: RawDemand::new(state.cardinality, state.consumption),
        }
    }

    fn observe(self) -> AimsState {
        let mut state = self.facts;
        state.cardinality = self.demand.cardinality;
        state.consumption = self.demand.consumption;
        state.canonicalize();
        state
    }
}

/// Sparse state used only while one block's same-path demand is accumulated.
pub(super) struct BlockState {
    states: FxHashMap<ArcVarId, PendingState>,
    /// L-9-excluded scalar values whose copied bits are demanded later on the
    /// current path. This is liveness only; no scalar enters `PendingState`.
    live_scalars: FxHashSet<ArcVarId>,
}

impl BlockState {
    pub(super) fn from_observed(
        states: FxHashMap<ArcVarId, AimsState>,
        live_scalars: FxHashSet<ArcVarId>,
    ) -> Self {
        Self {
            states: states
                .into_iter()
                .map(|(var, state)| (var, PendingState::from_observed(state)))
                .collect(),
            live_scalars,
        }
    }

    pub(super) fn observe(&self, var: ArcVarId) -> Option<AimsState> {
        self.states.get(&var).copied().map(PendingState::observe)
    }

    pub(super) fn observe_or_bottom(&self, var: ArcVarId) -> AimsState {
        self.observe(var).unwrap_or(AimsState::BOTTOM)
    }

    /// Whether the reverse walk has seen any post-definition demand evidence.
    ///
    /// A dead projection contributes `Absent + Affine`: CN-1 observes it as
    /// Dead, but it still proves that the source is referenced after an alias
    /// is created. Effect derivation must not erase that occurrence evidence.
    pub(super) fn has_raw_demand(&self, var: ArcVarId) -> bool {
        self.states
            .get(&var)
            .is_some_and(|state| state.demand != RawDemand::ZERO)
    }

    pub(super) fn observed_entries(&self) -> impl Iterator<Item = (ArcVarId, AimsState)> + '_ {
        self.states
            .iter()
            .map(|(&var, &state)| (var, state.observe()))
    }

    pub(super) fn remove(&mut self, var: ArcVarId) {
        self.states.remove(&var);
        self.live_scalars.remove(&var);
    }

    pub(super) fn mark_scalar_live(&mut self, var: ArcVarId) {
        self.live_scalars.insert(var);
    }

    pub(super) fn is_scalar_live(&self, var: ArcVarId) -> bool {
        self.live_scalars.contains(&var)
    }

    pub(super) fn seq_add(&mut self, var: ArcVarId, demand: RawDemand) {
        let state = self
            .states
            .entry(var)
            .or_insert_with(|| PendingState::from_observed(AimsState::BOTTOM));
        state.demand = state.demand.seq_add(demand);
    }

    pub(super) fn transfer_alias(&mut self, source: ArcVarId, destination: AimsState) {
        self.seq_add(
            source,
            RawDemand::new(destination.cardinality, destination.consumption),
        );
        self.widen_locality(source, destination.locality);
    }

    pub(super) fn transfer_project(&mut self, source: ArcVarId, destination: AimsState) {
        self.seq_add(
            source,
            RawDemand::new(destination.cardinality, destination.consumption).projected(),
        );
        self.widen_locality(source, destination.locality);
    }

    pub(super) fn transfer_scalar_project(&mut self, source: ArcVarId, live: bool) {
        self.seq_add(source, RawDemand::scalar_project_contribution(live));
    }

    /// Join a complete state at an explicit lattice-join boundary.
    pub(super) fn alt_join_state(&mut self, var: ArcVarId, other: AimsState) {
        let joined = self.observe(var).map_or(other, |state| state.join(&other));
        self.states.insert(var, PendingState::from_observed(joined));
    }

    pub(super) fn promote_owned(&mut self, var: ArcVarId, locality: Locality) {
        let state = self
            .states
            .entry(var)
            .or_insert_with(|| PendingState::from_observed(AimsState::BOTTOM));
        state.facts.access = AccessClass::Owned;
        state.facts.locality = state.facts.locality.max(locality);
    }

    pub(super) fn widen_locality(&mut self, var: ArcVarId, locality: Locality) {
        let state = self
            .states
            .entry(var)
            .or_insert_with(|| PendingState::from_observed(AimsState::BOTTOM));
        state.facts.locality = state.facts.locality.max(locality);
    }

    pub(super) fn widen_uniqueness(&mut self, var: ArcVarId, uniqueness: Uniqueness) {
        let state = self
            .states
            .entry(var)
            .or_insert_with(|| PendingState::from_observed(AimsState::BOTTOM));
        state.facts.uniqueness = state.facts.uniqueness.max(uniqueness);
    }

    pub(super) fn mark_returned(&mut self, var: ArcVarId) {
        let state = self
            .states
            .entry(var)
            .or_insert_with(|| PendingState::from_observed(AimsState::BOTTOM));
        state.facts.access = AccessClass::Owned;
        state.facts.locality = state.facts.locality.max(Locality::HeapEscaping);
    }

    pub(super) fn into_observed(self) -> (FxHashMap<ArcVarId, AimsState>, FxHashSet<ArcVarId>) {
        let states = self
            .states
            .into_iter()
            .map(|(var, state)| (var, state.observe()))
            .collect();
        (states, self.live_scalars)
    }
}
