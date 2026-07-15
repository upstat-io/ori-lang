//! Central ownership operations for self-describing VM values.

use ori_arc::RcStrategy;
use ori_repr::executable::FunctionId;

use crate::bytecode::{
    VmCalleeOwnerDemand, VmClosureAdapterAction, VmRetainEdge, VmRetainPlanId, VmRetainPlanKind,
};
use crate::error::invalid_verified_index;
use crate::{ExecutionError, IndexKind, ValueKind};

use super::arena::{Aggregate, AggregateKind, IteratorSourceState, ValueArena};
use super::heap::HeapObject;
use super::value::VmValue;
use super::{ArenaAllocationRoots, Interpreter};

#[derive(Clone, Copy)]
enum AdapterRetainDestination {
    Slot {
        index: usize,
        wrap_iterator_step: bool,
    },
    AggregateField {
        aggregate: VmValue,
        field: u32,
        wrap_iterator_step: bool,
    },
}

impl AdapterRetainDestination {
    fn wrap_iterator_step(self) -> Result<Self, ExecutionError> {
        match self {
            Self::Slot {
                index,
                wrap_iterator_step: false,
            } => Ok(Self::Slot {
                index,
                wrap_iterator_step: true,
            }),
            Self::AggregateField {
                aggregate,
                field,
                wrap_iterator_step: false,
            } => Ok(Self::AggregateField {
                aggregate,
                field,
                wrap_iterator_step: true,
            }),
            Self::Slot {
                wrap_iterator_step: true,
                ..
            }
            | Self::AggregateField {
                wrap_iterator_step: true,
                ..
            } => Err(ExecutionError::InvalidClosureAdapterExecution {
                details: "retain topology requires a nested iterator-step representation",
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct AdapterRetainWork {
    value: VmValue,
    plan: VmRetainPlanId,
    destination: AdapterRetainDestination,
    copy_aggregate: bool,
}

#[derive(Clone, Copy, Default)]
struct AdapterRetainRequirements {
    aggregate_copies: usize,
}

impl AdapterRetainRequirements {
    fn record_aggregate_copy(&mut self, copy: bool) -> Result<(), ExecutionError> {
        if copy {
            self.aggregate_copies =
                self.aggregate_copies
                    .checked_add(1)
                    .ok_or(ExecutionError::MetricOverflow {
                        metric: "closure adapter aggregate copies",
                    })?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct PreparedAdapterAggregate {
    source: Option<Aggregate>,
    destination: Option<VmValue>,
}

impl Interpreter<'_> {
    pub(super) fn prepare_closure_adapter_values(
        &mut self,
        callee: FunctionId,
        values: &mut [VmValue],
    ) -> Result<(), ExecutionError> {
        self.require_idle_adapter_retain_scratch()?;
        let result = (|| {
            let function = self.program.functions.get(callee.index()).ok_or_else(|| {
                invalid_verified_index(
                    IndexKind::Function,
                    callee.index(),
                    self.program.functions.len(),
                )
            })?;
            let adapter = function.closure_adapter.as_ref().ok_or(
                ExecutionError::InvalidClosureAdapterExecution {
                    details: "closure target has no frozen adapter plan",
                },
            )?;
            if adapter.slots.len() != values.len() {
                return Err(ExecutionError::InvalidClosureAdapterExecution {
                    details: "adapter slot count disagrees with the concrete argument vector",
                });
            }

            let mut requirements = AdapterRetainRequirements::default();
            for (index, (&value, &slot)) in values.iter().zip(&adapter.slots).enumerate() {
                let VmClosureAdapterAction::Retain(plan) = slot.action else {
                    continue;
                };
                let copy_aggregate = self.adapter_root_copies_aggregate(slot.demand, plan)?;
                self.scan_adapter_retain_plan(
                    value,
                    plan,
                    copy_aggregate,
                    index,
                    &mut requirements,
                )?;
            }

            self.ownership_scratch
                .sort_unstable_by_key(|value| value.heap_index().unwrap_or(usize::MAX));
            self.heap
                .validate_increment_plan(&self.ownership_scratch, 1)?;
            self.prepare_value_arena_allocations(
                requirements.aggregate_copies,
                ArenaAllocationRoots::preserving(values),
            )?;

            for index in 0..adapter.slots.len() {
                let slot = adapter.slots[index];
                let VmClosureAdapterAction::Retain(plan) = slot.action else {
                    continue;
                };
                let copy_aggregate = self.adapter_root_copies_aggregate(slot.demand, plan)?;
                self.apply_adapter_retain_plan(values, index, plan, copy_aggregate)?;
            }
            Ok(())
        })();
        self.adapter_retain_scratch.clear();
        if result.is_err() {
            self.ownership_scratch.clear();
        }
        result
    }

    fn require_idle_adapter_retain_scratch(&self) -> Result<(), ExecutionError> {
        if !self.ownership_scratch.is_empty() {
            return Err(ExecutionError::InvalidClosureAdapterExecution {
                details: "closure adapter preparation overlaps a staged ownership operation",
            });
        }
        if !self.adapter_retain_scratch.is_empty() {
            return Err(ExecutionError::InvalidClosureAdapterExecution {
                details: "closure adapter traversal scratch was not drained",
            });
        }
        Ok(())
    }

    fn adapter_root_copies_aggregate(
        &self,
        demand: VmCalleeOwnerDemand,
        plan: VmRetainPlanId,
    ) -> Result<bool, ExecutionError> {
        let node = self.retain_plan(plan)?;
        match demand {
            VmCalleeOwnerDemand::WholeValue => {
                Ok(!matches!(node.kind, VmRetainPlanKind::SelfOwnedIdentity))
            }
            VmCalleeOwnerDemand::ProjectedField(_) => match &node.kind {
                VmRetainPlanKind::OwnedFields(edges) => {
                    for edge in edges {
                        if !matches!(
                            self.retain_plan(edge.child)?.kind,
                            VmRetainPlanKind::SelfOwnedIdentity
                        ) {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                VmRetainPlanKind::SelfOwnedIdentity | VmRetainPlanKind::OwnedVariants(_) => {
                    Err(ExecutionError::InvalidClosureAdapterExecution {
                        details: "projected-field demand has a non-product retain root",
                    })
                }
            },
            VmCalleeOwnerDemand::Borrow => Err(ExecutionError::InvalidClosureAdapterExecution {
                details: "borrowed adapter slot unexpectedly carries a retain action",
            }),
        }
    }

    fn scan_adapter_retain_plan(
        &mut self,
        value: VmValue,
        plan: VmRetainPlanId,
        copy_aggregate: bool,
        slot: usize,
        requirements: &mut AdapterRetainRequirements,
    ) -> Result<(), ExecutionError> {
        self.begin_adapter_retain_traversal(AdapterRetainWork {
            value,
            plan,
            destination: AdapterRetainDestination::Slot {
                index: slot,
                wrap_iterator_step: false,
            },
            copy_aggregate,
        })?;
        while let Some(work) = self.adapter_retain_scratch.pop() {
            self.scan_adapter_retain_work(work, requirements)?;
        }
        Ok(())
    }

    fn scan_adapter_retain_work(
        &mut self,
        work: AdapterRetainWork,
        requirements: &mut AdapterRetainRequirements,
    ) -> Result<(), ExecutionError> {
        let node = self
            .program
            .retain_plans
            .get(work.plan.index())
            .ok_or_else(|| {
                invalid_verified_index(
                    IndexKind::RetainPlan,
                    work.plan.index(),
                    self.program.retain_plans.len(),
                )
            })?;
        match &node.kind {
            VmRetainPlanKind::SelfOwnedIdentity => {
                scan_adapter_identity(work.value, &mut self.ownership_scratch)
            }
            VmRetainPlanKind::OwnedFields(edges) => scan_adapter_product(
                &self.value_arena,
                &mut self.adapter_retain_scratch,
                work,
                edges,
                requirements,
            ),
            VmRetainPlanKind::OwnedVariants(variants) => scan_adapter_variant(
                &self.value_arena,
                &mut self.adapter_retain_scratch,
                work,
                variants,
                requirements,
            ),
        }
    }

    fn apply_adapter_retain_plan(
        &mut self,
        values: &mut [VmValue],
        slot: usize,
        plan: VmRetainPlanId,
        copy_aggregate: bool,
    ) -> Result<(), ExecutionError> {
        self.begin_adapter_retain_traversal(AdapterRetainWork {
            value: values[slot],
            plan,
            destination: AdapterRetainDestination::Slot {
                index: slot,
                wrap_iterator_step: false,
            },
            copy_aggregate,
        })?;
        while let Some(work) = self.adapter_retain_scratch.pop() {
            self.apply_adapter_retain_work(values, work)?;
        }
        Ok(())
    }

    fn apply_adapter_retain_work(
        &mut self,
        values: &mut [VmValue],
        work: AdapterRetainWork,
    ) -> Result<(), ExecutionError> {
        let prepared = self.prepare_adapter_aggregate(values, work)?;
        let node = self
            .program
            .retain_plans
            .get(work.plan.index())
            .ok_or_else(|| {
                invalid_verified_index(
                    IndexKind::RetainPlan,
                    work.plan.index(),
                    self.program.retain_plans.len(),
                )
            })?;
        match &node.kind {
            VmRetainPlanKind::SelfOwnedIdentity => Ok(()),
            VmRetainPlanKind::OwnedFields(edges) => {
                apply_adapter_product(&mut self.adapter_retain_scratch, work, prepared, edges)
            }
            VmRetainPlanKind::OwnedVariants(variants) => {
                apply_adapter_variant(&mut self.adapter_retain_scratch, work, prepared, variants)
            }
        }
    }

    fn prepare_adapter_aggregate(
        &mut self,
        values: &mut [VmValue],
        work: AdapterRetainWork,
    ) -> Result<PreparedAdapterAggregate, ExecutionError> {
        let aggregate = if work.value.kind() == ValueKind::Aggregate {
            Some(*self.value_arena.aggregate(work.value)?)
        } else {
            None
        };
        let destination = if work.copy_aggregate {
            if let Some(aggregate) = aggregate {
                let copied = self
                    .value_arena
                    .allocate_aggregate(aggregate, self.config.max_value_arena_entries)?;
                self.write_adapter_destination(values, work.destination, copied)?;
                Some(copied)
            } else {
                None
            }
        } else {
            aggregate.map(|_| work.value)
        };
        Ok(PreparedAdapterAggregate {
            source: aggregate,
            destination,
        })
    }

    fn begin_adapter_retain_traversal(
        &mut self,
        root: AdapterRetainWork,
    ) -> Result<(), ExecutionError> {
        if !self.adapter_retain_scratch.is_empty() {
            return Err(ExecutionError::InvalidClosureAdapterExecution {
                details: "closure adapter traversal overlaps unfinished scratch work",
            });
        }
        self.adapter_retain_scratch.push(root);
        Ok(())
    }

    fn write_adapter_destination(
        &mut self,
        values: &mut [VmValue],
        destination: AdapterRetainDestination,
        value: VmValue,
    ) -> Result<(), ExecutionError> {
        match destination {
            AdapterRetainDestination::Slot {
                index,
                wrap_iterator_step,
            } => {
                values[index] = if wrap_iterator_step {
                    VmValue::iterator_step(Some(value))?
                } else {
                    value
                };
            }
            AdapterRetainDestination::AggregateField {
                aggregate,
                field,
                wrap_iterator_step,
            } => {
                let field = field as usize;
                let aggregate = self.value_arena.aggregate_mut(aggregate)?;
                if field >= usize::from(aggregate.length) {
                    return Err(ExecutionError::AggregateFieldOutOfBounds {
                        field,
                        length: usize::from(aggregate.length),
                    });
                }
                aggregate.fields[field] = if wrap_iterator_step {
                    VmValue::iterator_step(Some(value))?
                } else {
                    value
                };
            }
        }
        Ok(())
    }

    fn retain_plan(
        &self,
        plan: VmRetainPlanId,
    ) -> Result<&crate::bytecode::VmRetainPlan, ExecutionError> {
        self.program.retain_plans.get(plan.index()).ok_or_else(|| {
            invalid_verified_index(
                IndexKind::RetainPlan,
                plan.index(),
                self.program.retain_plans.len(),
            )
        })
    }

    pub(super) fn retain_with_strategy(
        &mut self,
        value: VmValue,
        count: u32,
        strategy: RcStrategy,
    ) -> Result<(), ExecutionError> {
        match strategy {
            RcStrategy::HeapPointer => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Heap])?;
                self.heap.increment(value, count)
            }
            RcStrategy::FatPointer => {
                Self::require_rc_kind(
                    value,
                    strategy,
                    &[ValueKind::Heap, ValueKind::ConstantString],
                )?;
                self.heap.increment(value, count)
            }
            RcStrategy::AggregateFields | RcStrategy::InlineEnum => {
                Self::require_rc_kind(
                    value,
                    strategy,
                    &[ValueKind::Aggregate, ValueKind::IteratorStep],
                )?;
                self.retain_owned_value(value, count)
            }
            RcStrategy::Iterator => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Iterator])?;
                self.value_arena.iterator(value).map(|_| ())
            }
            RcStrategy::Closure => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Heap])?;
                Self::require_closure_object(&self.heap.get(value)?.object)?;
                self.heap.increment(value, count)
            }
            RcStrategy::UserDrop => Err(ExecutionError::UnsupportedRcStrategy { strategy }),
        }
    }

    pub(super) fn release_with_strategy(
        &mut self,
        value: VmValue,
        strategy: RcStrategy,
    ) -> Result<(), ExecutionError> {
        match strategy {
            RcStrategy::HeapPointer => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Heap])?;
                self.release_owned_value(value)
            }
            RcStrategy::FatPointer => {
                Self::require_rc_kind(
                    value,
                    strategy,
                    &[ValueKind::Heap, ValueKind::ConstantString],
                )?;
                self.release_owned_value(value)
            }
            RcStrategy::AggregateFields | RcStrategy::InlineEnum => {
                Self::require_rc_kind(
                    value,
                    strategy,
                    &[ValueKind::Aggregate, ValueKind::IteratorStep],
                )?;
                self.release_owned_value(value)
            }
            RcStrategy::Iterator => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Iterator])?;
                self.drop_iterator(value)
            }
            RcStrategy::Closure => {
                Self::require_rc_kind(value, strategy, &[ValueKind::Heap])?;
                Self::require_closure_object(&self.heap.get(value)?.object)?;
                self.release_owned_value(value)
            }
            RcStrategy::UserDrop => Err(ExecutionError::UnsupportedRcStrategy { strategy }),
        }
    }

    /// Retain one logical owner of a VM value `count` additional times.
    ///
    /// Heap values retain only their outer allocation. Inline aggregates have
    /// no container count, so retaining them walks their active fields.
    pub(super) fn retain_owned_value(
        &mut self,
        value: VmValue,
        count: u32,
    ) -> Result<(), ExecutionError> {
        if count == 0 {
            return Ok(());
        }
        self.prepare_retain_owned_values([value], count)?;
        self.heap
            .commit_increment_plan(&self.ownership_scratch, count);
        self.ownership_scratch.clear();
        Ok(())
    }

    /// Release one logical owner and transitively release owned children.
    pub(super) fn release_owned_value(&mut self, value: VmValue) -> Result<(), ExecutionError> {
        self.prepare_release_owned_value(value)?;
        self.commit_prepared_release(value)
    }

    pub(super) fn prepare_release_owned_value(
        &mut self,
        value: VmValue,
    ) -> Result<(), ExecutionError> {
        self.prepare_release_owned_values(&[value])
    }

    pub(super) fn prepare_release_owned_values(
        &mut self,
        values: &[VmValue],
    ) -> Result<(), ExecutionError> {
        self.preflight_release_owned_values(values)
    }

    pub(super) fn commit_prepared_release(&mut self, value: VmValue) -> Result<(), ExecutionError> {
        self.commit_prepared_releases(&[value])
    }

    pub(super) fn commit_prepared_releases(
        &mut self,
        values: &[VmValue],
    ) -> Result<(), ExecutionError> {
        let result = self.commit_release_owned_values(values);
        self.ownership_count_scratch.clear();
        result
    }

    pub(super) fn discard_prepared_release(&mut self) {
        self.ownership_count_scratch.clear();
    }

    pub(super) fn prepare_retain_owned_values(
        &mut self,
        values: impl IntoIterator<Item = VmValue>,
        count: u32,
    ) -> Result<(), ExecutionError> {
        debug_assert!(self.ownership_scratch.is_empty());
        self.ownership_scratch.extend(values);
        self.prepare_staged_retains(count)
    }

    pub(super) fn prepare_staged_retains(&mut self, count: u32) -> Result<(), ExecutionError> {
        let result = (|| {
            let mut cursor = 0;
            while cursor < self.ownership_scratch.len() {
                let current = self.ownership_scratch[cursor];
                cursor += 1;
                match current.kind() {
                    ValueKind::Aggregate => {
                        let aggregate = *self.value_arena.aggregate(current)?;
                        self.ownership_scratch.extend(
                            aggregate.fields[..usize::from(aggregate.length)]
                                .iter()
                                .copied(),
                        );
                    }
                    ValueKind::Iterator => {
                        self.value_arena.iterator(current)?;
                    }
                    ValueKind::IteratorStep => {
                        if let Some(item) = current.as_iterator_step()? {
                            self.ownership_scratch.push(item);
                        }
                    }
                    ValueKind::Heap
                    | ValueKind::Unit
                    | ValueKind::Int
                    | ValueKind::Bool
                    | ValueKind::Float
                    | ValueKind::Char
                    | ValueKind::Null
                    | ValueKind::ConstantString => {}
                }
            }
            self.ownership_scratch
                .retain(|value| value.kind() == ValueKind::Heap);
            self.ownership_scratch
                .sort_unstable_by_key(|value| value.heap_index().unwrap_or(usize::MAX));
            self.heap
                .validate_increment_plan(&self.ownership_scratch, count)
        })();
        if result.is_err() {
            self.ownership_scratch.clear();
        }
        result
    }

    pub(super) fn commit_prepared_retains(&mut self, count: u32) {
        self.heap
            .commit_increment_plan(&self.ownership_scratch, count);
        self.ownership_scratch.clear();
    }

    pub(super) fn discard_prepared_retains(&mut self) {
        self.ownership_scratch.clear();
    }

    fn preflight_release_owned_values(&mut self, values: &[VmValue]) -> Result<(), ExecutionError> {
        debug_assert!(self.ownership_scratch.is_empty());
        debug_assert!(self.ownership_count_scratch.is_empty());
        self.ownership_scratch.extend_from_slice(values);
        let result = (|| {
            let mut released_objects = 0_usize;
            let mut released_payload_bytes = 0_usize;
            while let Some(current) = self.ownership_scratch.pop() {
                match current.kind() {
                    ValueKind::Heap => {
                        let index = current.heap_index()?;
                        let position = self
                            .ownership_count_scratch
                            .binary_search_by_key(&index, |(value, _)| {
                                value.heap_index().unwrap_or(usize::MAX)
                            });
                        let position = match position {
                            Ok(position) => position,
                            Err(position) => {
                                let references = self.heap.references(current)?;
                                self.ownership_count_scratch
                                    .insert(position, (current, references));
                                position
                            }
                        };
                        let references = self.ownership_count_scratch[position]
                            .1
                            .checked_sub(1)
                            .ok_or(ExecutionError::ReferenceCountUnderflow)?;
                        self.ownership_count_scratch[position].1 = references;
                        if references == 0 {
                            let object = &self.heap.get(current)?.object;
                            released_objects = released_objects.checked_add(1).ok_or(
                                ExecutionError::MetricOverflow {
                                    metric: "live heap objects",
                                },
                            )?;
                            released_payload_bytes = released_payload_bytes
                                .checked_add(object.owned_payload_bytes())
                                .ok_or(ExecutionError::MetricOverflow {
                                    metric: "live heap payload bytes",
                                })?;
                            self.ownership_scratch.extend(
                                object
                                    .owned_children()
                                    .iter()
                                    .copied()
                                    .filter(requires_owned_release),
                            );
                        }
                    }
                    ValueKind::Aggregate => {
                        let aggregate = *self.value_arena.aggregate(current)?;
                        self.ownership_scratch.extend(
                            aggregate.fields[..usize::from(aggregate.length)]
                                .iter()
                                .copied()
                                .filter(requires_owned_release),
                        );
                    }
                    ValueKind::Iterator => {
                        let state = *self.value_arena.iterator(current)?;
                        if state.live {
                            if let IteratorSourceState::List {
                                source,
                                owns_source: true,
                                ..
                            } = state.source
                            {
                                self.ownership_scratch.push(source);
                            }
                        }
                    }
                    ValueKind::IteratorStep => {
                        if let Some(item) = current.as_iterator_step()? {
                            if requires_owned_release(&item) {
                                self.ownership_scratch.push(item);
                            }
                        }
                    }
                    ValueKind::Unit
                    | ValueKind::Int
                    | ValueKind::Bool
                    | ValueKind::Float
                    | ValueKind::Char
                    | ValueKind::Null
                    | ValueKind::ConstantString => {}
                }
            }
            self.heap
                .validate_release_metrics(released_objects, released_payload_bytes)
        })();
        self.ownership_scratch.clear();
        if result.is_err() {
            self.ownership_count_scratch.clear();
        }
        result
    }

    fn commit_release_owned_values(&mut self, values: &[VmValue]) -> Result<(), ExecutionError> {
        debug_assert!(self.ownership_scratch.is_empty());
        self.ownership_scratch.extend_from_slice(values);
        let result = (|| {
            while let Some(current) = self.ownership_scratch.pop() {
                match current.kind() {
                    ValueKind::Heap => {
                        if let Some(children) = self
                            .heap
                            .decrement(current)?
                            .and_then(HeapObject::into_owned_children)
                        {
                            self.ownership_scratch
                                .extend(children.into_iter().filter(requires_owned_release));
                        }
                    }
                    ValueKind::Aggregate => {
                        let aggregate = *self.value_arena.aggregate(current)?;
                        self.ownership_scratch.extend(
                            aggregate.fields[..usize::from(aggregate.length)]
                                .iter()
                                .copied()
                                .filter(requires_owned_release),
                        );
                    }
                    ValueKind::IteratorStep => {
                        if let Some(item) = current.as_iterator_step()? {
                            if requires_owned_release(&item) {
                                self.ownership_scratch.push(item);
                            }
                        }
                    }
                    ValueKind::Unit
                    | ValueKind::Int
                    | ValueKind::Bool
                    | ValueKind::Float
                    | ValueKind::Char
                    | ValueKind::Null
                    | ValueKind::ConstantString => {}
                    ValueKind::Iterator => {
                        let state = *self.value_arena.iterator(current)?;
                        if state.live {
                            let owned_source = match state.source {
                                IteratorSourceState::List {
                                    source,
                                    owns_source: true,
                                    ..
                                } => Some(source),
                                IteratorSourceState::Range { .. }
                                | IteratorSourceState::List {
                                    owns_source: false, ..
                                } => None,
                            };
                            let state = self.iterator_mut(current)?;
                            state.live = false;
                            if let IteratorSourceState::List { owns_source, .. } = &mut state.source
                            {
                                *owns_source = false;
                            }
                            if let Some(source) = owned_source {
                                self.ownership_scratch.push(source);
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        self.ownership_scratch.clear();
        result
    }

    pub(super) fn drop_iterator(&mut self, value: VmValue) -> Result<(), ExecutionError> {
        if self.value_arena.iterator(value)?.live {
            self.release_owned_value(value)
        } else {
            Ok(())
        }
    }

    fn require_rc_kind(
        value: VmValue,
        strategy: RcStrategy,
        expected: &[ValueKind],
    ) -> Result<(), ExecutionError> {
        let found = value.kind();
        if expected.contains(&found) {
            Ok(())
        } else {
            Err(ExecutionError::RcStrategyMismatch { strategy, found })
        }
    }

    fn require_closure_object(object: &HeapObject) -> Result<(), ExecutionError> {
        if matches!(object, HeapObject::Closure { .. }) {
            Ok(())
        } else {
            Err(ExecutionError::InvalidClosureObject)
        }
    }
}

fn scan_adapter_product(
    arena: &ValueArena,
    scratch: &mut Vec<AdapterRetainWork>,
    work: AdapterRetainWork,
    edges: &[VmRetainEdge],
    requirements: &mut AdapterRetainRequirements,
) -> Result<(), ExecutionError> {
    let aggregate = *arena.aggregate(work.value)?;
    require_adapter_aggregate_kind(
        aggregate,
        AggregateKind::Product,
        "product retain topology reached a variant aggregate",
    )?;
    requirements.record_aggregate_copy(work.copy_aggregate)?;
    push_scanned_aggregate_edges(scratch, aggregate, edges, work.destination)
}

fn scan_adapter_variant(
    arena: &ValueArena,
    scratch: &mut Vec<AdapterRetainWork>,
    work: AdapterRetainWork,
    variants: &[Box<[VmRetainEdge]>],
    requirements: &mut AdapterRetainRequirements,
) -> Result<(), ExecutionError> {
    match work.value.kind() {
        ValueKind::Aggregate => {
            let aggregate = *arena.aggregate(work.value)?;
            require_adapter_aggregate_kind(
                aggregate,
                AggregateKind::Variant,
                "variant retain topology reached a product aggregate",
            )?;
            requirements.record_aggregate_copy(work.copy_aggregate)?;
            let edges = aggregate_variant_edges(aggregate, variants)?;
            push_scanned_aggregate_edges(scratch, aggregate, edges, work.destination)
        }
        ValueKind::IteratorStep => {
            let item = work.value.as_iterator_step()?;
            let edges = retain_variant_edges(variants, usize::from(item.is_none()))?;
            push_iterator_step_edges(scratch, item, edges, work.destination)
        }
        found => Err(ExecutionError::ClosureAdapterValueShape {
            expected: "an active variant",
            found,
        }),
    }
}

fn apply_adapter_product(
    scratch: &mut Vec<AdapterRetainWork>,
    work: AdapterRetainWork,
    prepared: PreparedAdapterAggregate,
    edges: &[VmRetainEdge],
) -> Result<(), ExecutionError> {
    let aggregate = prepared
        .source
        .ok_or(ExecutionError::ClosureAdapterValueShape {
            expected: "an inline product",
            found: work.value.kind(),
        })?;
    let destination =
        prepared
            .destination
            .ok_or(ExecutionError::InvalidClosureAdapterExecution {
                details: "product retain topology has no aggregate destination",
            })?;
    push_applied_aggregate_edges(scratch, aggregate, edges, destination)
}

fn apply_adapter_variant(
    scratch: &mut Vec<AdapterRetainWork>,
    work: AdapterRetainWork,
    prepared: PreparedAdapterAggregate,
    variants: &[Box<[VmRetainEdge]>],
) -> Result<(), ExecutionError> {
    match work.value.kind() {
        ValueKind::Aggregate => {
            let aggregate =
                prepared
                    .source
                    .ok_or(ExecutionError::InvalidClosureAdapterExecution {
                        details: "variant retain topology has no aggregate source",
                    })?;
            let destination =
                prepared
                    .destination
                    .ok_or(ExecutionError::InvalidClosureAdapterExecution {
                        details: "variant retain topology has no aggregate destination",
                    })?;
            let edges = aggregate_variant_edges(aggregate, variants)?;
            push_applied_aggregate_edges(scratch, aggregate, edges, destination)
        }
        ValueKind::IteratorStep => {
            let item = work.value.as_iterator_step()?;
            let edges = retain_variant_edges(variants, usize::from(item.is_none()))?;
            push_iterator_step_edges(scratch, item, edges, work.destination)
        }
        found => Err(ExecutionError::ClosureAdapterValueShape {
            expected: "an active variant",
            found,
        }),
    }
}

fn scan_adapter_identity(
    value: VmValue,
    ownership_scratch: &mut Vec<VmValue>,
) -> Result<(), ExecutionError> {
    match value.kind() {
        ValueKind::Heap => {
            ownership_scratch.push(value);
            Ok(())
        }
        ValueKind::ConstantString => Ok(()),
        found => Err(ExecutionError::ClosureAdapterValueShape {
            expected: "a shared identity",
            found,
        }),
    }
}

fn require_adapter_aggregate_kind(
    aggregate: Aggregate,
    expected: AggregateKind,
    details: &'static str,
) -> Result<(), ExecutionError> {
    if aggregate.kind == expected {
        Ok(())
    } else {
        Err(ExecutionError::InvalidClosureAdapterExecution { details })
    }
}

fn aggregate_field(aggregate: Aggregate, field: u32) -> Result<VmValue, ExecutionError> {
    let field = field as usize;
    aggregate
        .fields
        .get(field)
        .copied()
        .filter(|_| field < usize::from(aggregate.length))
        .ok_or(ExecutionError::AggregateFieldOutOfBounds {
            field,
            length: usize::from(aggregate.length),
        })
}

fn retain_variant_edges(
    variants: &[Box<[VmRetainEdge]>],
    variant: usize,
) -> Result<&[VmRetainEdge], ExecutionError> {
    variants
        .get(variant)
        .map(Box::as_ref)
        .ok_or(ExecutionError::ClosureAdapterVariantOutOfBounds {
            variant,
            variants: variants.len(),
        })
}

fn aggregate_variant_edges(
    aggregate: Aggregate,
    variants: &[Box<[VmRetainEdge]>],
) -> Result<&[VmRetainEdge], ExecutionError> {
    let variant = usize::try_from(aggregate.variant).map_err(|_| {
        ExecutionError::InvalidClosureAdapterExecution {
            details: "aggregate variant does not fit the host index range",
        }
    })?;
    retain_variant_edges(variants, variant)
}

fn push_scanned_aggregate_edges(
    scratch: &mut Vec<AdapterRetainWork>,
    aggregate: Aggregate,
    edges: &[VmRetainEdge],
    destination: AdapterRetainDestination,
) -> Result<(), ExecutionError> {
    for edge in edges.iter().rev() {
        scratch.push(AdapterRetainWork {
            value: aggregate_field(aggregate, edge.field)?,
            plan: edge.child,
            destination,
            copy_aggregate: true,
        });
    }
    Ok(())
}

fn push_applied_aggregate_edges(
    scratch: &mut Vec<AdapterRetainWork>,
    aggregate: Aggregate,
    edges: &[VmRetainEdge],
    destination: VmValue,
) -> Result<(), ExecutionError> {
    for edge in edges.iter().rev() {
        scratch.push(AdapterRetainWork {
            value: aggregate_field(aggregate, edge.field)?,
            plan: edge.child,
            destination: AdapterRetainDestination::AggregateField {
                aggregate: destination,
                field: edge.field,
                wrap_iterator_step: false,
            },
            copy_aggregate: true,
        });
    }
    Ok(())
}

fn push_iterator_step_edges(
    scratch: &mut Vec<AdapterRetainWork>,
    item: Option<VmValue>,
    edges: &[VmRetainEdge],
    destination: AdapterRetainDestination,
) -> Result<(), ExecutionError> {
    let Some(item) = item else {
        if edges.is_empty() {
            return Ok(());
        }
        return Err(ExecutionError::InvalidClosureAdapterExecution {
            details: "empty iterator step has an ownership-bearing payload edge",
        });
    };
    for edge in edges.iter().rev() {
        if edge.field != 0 {
            return Err(ExecutionError::AggregateFieldOutOfBounds {
                field: edge.field as usize,
                length: 1,
            });
        }
        scratch.push(AdapterRetainWork {
            value: item,
            plan: edge.child,
            destination: destination.wrap_iterator_step()?,
            copy_aggregate: true,
        });
    }
    Ok(())
}

fn requires_owned_release(value: &VmValue) -> bool {
    matches!(
        value.kind(),
        ValueKind::Heap | ValueKind::Aggregate | ValueKind::Iterator | ValueKind::IteratorStep
    )
}
