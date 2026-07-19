//! Transfer functions for the AIMS lattice.
//!
//! Each ARC IR instruction has a transfer function that defines how it
//! transforms the [`AimsState`] of variables it touches:
//!
//! - **Forward (definition)**: what state does the destination variable get?
//! - **Backward (demand)**: what cardinality and consumption does each use add?
//!
//! The dataflow analysis engine applies these functions in
//! its worklist iteration. This module defines only the mathematical rules.
//!
//! References:
//! - Perceus dup/drop placement (Reinking et al., PLDI 2021)
//! - GHC demand analysis `seq_add`/`alt_join` (Sergey et al., POPL 2014)
//! - `updateLiveVars` / `addInc` / `addDec` borrow-liveness shape (Ullrich & de Moura, IFL 2019, Lean 4)

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

use smallvec::SmallVec;

use crate::ir::{
    ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind, PrimitiveFact,
};

use ori_registry::{PrimitiveAllocationEffect, PrimitiveResultOwnership};

use super::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, EffectClass, Locality,
    ReuseCtorKind, ShapeClass, Uniqueness,
};

// Forward transfer result

/// Result of a forward transfer function for a value-defining instruction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefTransfer {
    /// The [`AimsState`] for the defined variable.
    pub state: AimsState,
    /// Borrow provenance, if the defined variable is borrowed.
    pub borrow_source: Option<BorrowSource>,
}

impl DefTransfer {
    /// Transfer result with no borrow source.
    fn state(state: AimsState) -> Self {
        Self {
            state,
            borrow_source: None,
        }
    }

    /// Transfer result with a borrow source.
    fn borrowed(state: AimsState, source: BorrowSource) -> Self {
        Self {
            state,
            borrow_source: Some(source),
        }
    }
}

// Forward transfer: instruction definitions

/// Compute the [`AimsState`] for a variable defined by an instruction.
///
/// Returns `None` for side-effect-only instructions (`RcInc`, `RcDec`,
/// `Set`, `SetTag`) that don't define a variable.
///
/// The `get_state` closure retrieves the current state of any variable.
/// For unconstrained variables, it should return [`AimsState::TOP`].
#[cfg(test)]
pub fn transfer_def(
    instr: &ArcInstr,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> Option<DefTransfer> {
    transfer_def_impl(
        instr,
        get_state,
        &|_| None,
        PrimitiveFactRequirement::OptionalForUnitTransfer,
    )
}

/// Compute a definition transfer from the exact primitive facts frozen on the
/// ARC artifact.
///
/// Production analysis uses this entry point. Primitive facts are validated at
/// the AIMS input seam; a missing fact here is therefore an internal phase-order
/// violation and cannot fall back to scalar behavior.
pub(crate) fn transfer_def_resolved(
    func: &ArcFunction,
    instr: &ArcInstr,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> Option<DefTransfer> {
    if let ArcInstr::Let {
        dst,
        value: ArcValue::PrimOp { .. },
        ..
    } = instr
    {
        if func.primitive_facts.get(*dst).is_none() {
            let frozen_destinations = func
                .primitive_facts
                .iter()
                .map(|(destination, _)| destination.raw())
                .collect::<Vec<_>>();
            panic!(
                "validated PrimOp v{} in function {:?} is missing its frozen fact; frozen destinations: {frozen_destinations:?}; rerun whole-program AIMS primitive-fact freezing before analysis",
                dst.raw(),
                func.name
            );
        }
    }
    transfer_def_impl(
        instr,
        get_state,
        &|dst| func.primitive_facts.get(dst),
        PrimitiveFactRequirement::Required,
    )
}

#[derive(Clone, Copy)]
enum PrimitiveFactRequirement {
    #[cfg(test)]
    OptionalForUnitTransfer,
    Required,
}

fn transfer_def_impl(
    instr: &ArcInstr,
    get_state: &impl Fn(ArcVarId) -> AimsState,
    get_primitive_fact: &impl Fn(ArcVarId) -> Option<PrimitiveFact>,
    primitive_fact_requirement: PrimitiveFactRequirement,
) -> Option<DefTransfer> {
    match instr {
        ArcInstr::Let { dst, value, .. } => Some(match value {
            ArcValue::Var(v) => DefTransfer::state(get_state(*v)),
            ArcValue::Literal(_) => DefTransfer::state(AimsState::SCALAR),
            ArcValue::PrimOp { .. } => {
                if let Some(fact) = get_primitive_fact(*dst) {
                    transfer_primitive(value, fact, get_state)
                } else {
                    match primitive_fact_requirement {
                        #[cfg(test)]
                        PrimitiveFactRequirement::OptionalForUnitTransfer => {
                            transfer_let(value, get_state)
                        }
                        PrimitiveFactRequirement::Required => {
                            panic!(
                                "validated PrimOp v{} is missing its frozen fact; rerun whole-program AIMS primitive-fact freezing before analysis",
                                dst.raw()
                            )
                        }
                    }
                }
            }
        }),
        ArcInstr::Construct { ctor, .. } => Some(transfer_construct(ctor)),
        ArcInstr::Project { value, field, .. } => Some(transfer_project(*value, *field, get_state)),
        ArcInstr::Apply { .. } => Some(transfer_apply_conservative()),
        ArcInstr::ApplyIndirect { .. } => Some(DefTransfer::state(AimsState::TOP)),
        ArcInstr::PartialApply { .. } => Some(transfer_partial_apply()),
        ArcInstr::Select {
            true_val,
            false_val,
            ..
        } => Some(transfer_select(*true_val, *false_val, get_state)),
        ArcInstr::CollectionReuse { .. } => Some(transfer_collection_reuse()),
        ArcInstr::IsShared { .. } | ArcInstr::Reset { .. } => {
            Some(DefTransfer::state(AimsState::SCALAR))
        }
        ArcInstr::Reuse { ctor, .. } => Some(transfer_reuse(ctor)),
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::RcDecPartial { .. }
        | ArcInstr::RcDecField { .. }
        | ArcInstr::RcDecVariant { .. }
        | ArcInstr::BurdenInc { .. }
        | ArcInstr::BurdenDec { .. }
        | ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. } => None,
    }
}

fn transfer_primitive(
    value: &ArcValue,
    fact: PrimitiveFact,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> DefTransfer {
    let ArcValue::PrimOp { args, .. } = value else {
        unreachable!("primitive transfer requires PrimOp")
    };
    match fact.descriptor.result {
        PrimitiveResultOwnership::Scalar => DefTransfer::state(AimsState::SCALAR),
        PrimitiveResultOwnership::IndependentOwned
        | PrimitiveResultOwnership::OwnedFromConsumedOrIndependent { .. } => {
            let mut state = AimsState::FRESH;
            state.shape = ShapeClass::NonReusable;
            state.effect = EffectClass {
                may_alloc: !matches!(fact.descriptor.allocation, PrimitiveAllocationEffect::None),
                ..EffectClass::NONE
            };
            state.canonicalize();
            DefTransfer::state(state)
        }
        PrimitiveResultOwnership::Alias { operand } => {
            let source = args.get(usize::from(operand)).copied().unwrap_or_else(|| {
                panic!("validated primitive alias operand {operand} is out of bounds")
            });
            DefTransfer::state(get_state(source))
        }
    }
}

/// `Let { value }` — bind a value to a variable.
///
/// - `Var(v)` → inherit source state
/// - `Literal(_)` → `SCALAR` (no RC)
/// - `PrimOp { .. }` → `SCALAR` (arithmetic on primitives)
#[cfg(test)]
fn transfer_let(value: &ArcValue, get_state: &impl Fn(ArcVarId) -> AimsState) -> DefTransfer {
    match value {
        ArcValue::Var(v) => DefTransfer::state(get_state(*v)),
        ArcValue::Literal(_) | ArcValue::PrimOp { .. } => DefTransfer::state(AimsState::SCALAR),
    }
}

/// `Construct { ctor, args }` — build struct/enum/tuple/collection/closure.
///
/// Destination gets `FRESH` base with shape from constructor kind:
/// - `Struct` → `ReusableCtor(Struct)`
/// - `EnumVariant` → `ReusableCtor(EnumVariant)`
/// - `ListLiteral`/`SetLiteral`/`MapLiteral` → `CollectionBuffer`
/// - `Tuple`/`Closure` → `NonReusable`
///
/// Per-variable effect: `may_alloc = true` — construction allocates heap memory.
/// (Effect computation: precise effect computation.)
fn transfer_construct(ctor: &CtorKind) -> DefTransfer {
    let shape = shape_from_ctor(ctor);
    let mut state = AimsState::FRESH;
    state.shape = shape;
    state.effect = EffectClass {
        may_alloc: true,
        ..EffectClass::NONE
    };
    state.canonicalize();
    DefTransfer::state(state)
}

/// `Project { value, field }` — extract a field from a struct/enum.
///
/// Destination is a borrowed view: `(Borrowed, Linear, Once, source.uniqueness)`
/// with `BorrowSource::exact_field(value, field)`.
///
/// Key insight: borrowing doesn't affect the source's uniqueness.
fn transfer_project(
    value: ArcVarId,
    field: u32,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> DefTransfer {
    let source = get_state(value);
    let mut state = AimsState {
        access: AccessClass::Borrowed,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        uniqueness: source.uniqueness,
        locality: source.locality,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };
    state.canonicalize();
    DefTransfer::borrowed(state, BorrowSource::exact_field(value, field))
}

/// `Apply { func, args }` — direct function call (conservative).
///
/// Without interprocedural information, the return value
/// is conservatively `(Owned, Unrestricted, Many, MaybeShared)`.
/// Interprocedural analysis refines this using `MemoryContract`.
fn transfer_apply_conservative() -> DefTransfer {
    let mut state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Unrestricted,
        cardinality: Cardinality::Many,
        uniqueness: Uniqueness::MaybeShared,
        locality: Locality::Unknown,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::ALL,
    };
    state.canonicalize();
    DefTransfer::state(state)
}

/// `PartialApply { func, args }` — create a closure capturing args.
///
/// Destination gets `FRESH` with `may_alloc` effect (closure environment
/// allocation). The analysis engine separately updates
/// captured args' states via [`capture_state_update`].
///
/// Per-variable effect: `may_alloc = true` — closure creation allocates.
/// (Effect computation: precise effect computation.)
fn transfer_partial_apply() -> DefTransfer {
    let mut state = AimsState::FRESH;
    state.effect = EffectClass {
        may_alloc: true,
        ..EffectClass::NONE
    };
    DefTransfer::state(state)
}

/// `Select { cond, true_val, false_val }` — conditional value.
///
/// Destination gets the join of both branch states.
fn transfer_select(
    true_val: ArcVarId,
    false_val: ArcVarId,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> DefTransfer {
    DefTransfer::state(get_state(true_val).join(&get_state(false_val)))
}

/// `CollectionReuse { old_var, dst }` — reuse a collection buffer.
///
/// Destination gets `FRESH` with `CollectionBuffer` shape.
fn transfer_collection_reuse() -> DefTransfer {
    let mut state = AimsState::FRESH;
    state.shape = ShapeClass::CollectionBuffer;
    state.canonicalize();
    DefTransfer::state(state)
}

/// `Reuse { token, dst, ctor }` — construct using reused memory.
///
/// Destination gets `FRESH` with shape from the constructor kind.
fn transfer_reuse(ctor: &CtorKind) -> DefTransfer {
    let shape = shape_from_ctor(ctor);
    let mut state = AimsState::FRESH;
    state.shape = shape;
    state.canonicalize();
    DefTransfer::state(state)
}

// Forward transfer: terminator definitions

/// Compute the [`AimsState`] for a variable defined by a terminator.
///
/// Only `Invoke` defines a variable (its `dst`, available in the normal
/// successor). Other terminators don't define variables.
pub fn transfer_terminator_def(term: &ArcTerminator) -> Option<DefTransfer> {
    match term {
        ArcTerminator::Invoke { .. } => Some(transfer_apply_conservative()),
        _ => None,
    }
}

// Backward demand

/// One explicit TF-11 operand demand.
///
/// Cardinality and consumption are independent lattice dimensions. Keeping
/// both in the carrier prevents consumers from reconstructing one from the
/// other and preserves valid states such as `Many + Affine`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackwardDemand {
    /// Operand receiving the demand.
    pub var: ArcVarId,
    /// Quantitative use count contributed by this instruction.
    pub cardinality: Cardinality,
    /// Ownership-consumption mode contributed by this instruction.
    pub consumption: Consumption,
}

impl BackwardDemand {
    fn linear_once(var: ArcVarId) -> Self {
        Self {
            var,
            cardinality: Cardinality::Once,
            consumption: Consumption::Linear,
        }
    }
}

/// Compute backward demand contributions for an instruction.
///
/// Each returned value carries both TF-11 demand dimensions. The analysis
/// engine composes each dimension with its own `seq_add` operation.
///
/// Historical influence: GHC demand analysis SHAPE (Sergey et al., POPL 2014):
/// - Sequential: `seq_add` along one execution path
/// - Alternative: `alt_join` at control-flow merge points
pub fn backward_demands(instr: &ArcInstr) -> SmallVec<[BackwardDemand; 4]> {
    match instr {
        ArcInstr::Let { value, .. } => match value {
            // Var: transparent-alias transfer — dst's accumulated demand
            // transfers to v in block.rs::analyze_block before dst is removed;
            // returning (v, Once) here would double-count. Literal: no var demand.
            ArcValue::Var(_) | ArcValue::Literal(_) => SmallVec::new(),
            ArcValue::PrimOp { args, .. } => args
                .iter()
                .map(|&var| BackwardDemand::linear_once(var))
                .collect(),
        },

        // Construct/Apply: each arg consumed once.
        ArcInstr::Construct { args, .. } | ArcInstr::Apply { args, .. } => {
            args.iter()
                .map(|&var| BackwardDemand::linear_once(var))
                .collect()
        }

        // ApplyIndirect: closure + all args demanded once.
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push(BackwardDemand::linear_once(*closure));
            d.extend(
                args.iter()
                    .map(|&var| BackwardDemand::linear_once(var)),
            );
            d
        }

        // PartialApply: captured arg demand is handled entirely by
        // `capture_state_update` in block.rs, which sets precise
        // access/consumption/cardinality/locality based on the closure's
        // own demand state. Returning demand here would double-count.
        // Current-carrier RC/burden operations are already-realized ownership
        // events, not user-code uses. The AIMS lattice does not consume them,
        // and their counter-shaped spelling belongs to the compiled adapter.
        // No backward demand is contributed.
        ArcInstr::PartialApply { .. }
        | ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::RcDecPartial { .. }
        | ArcInstr::RcDecField { .. }
        | ArcInstr::RcDecVariant { .. }
        | ArcInstr::BurdenInc { .. }
        | ArcInstr::BurdenDec { .. }
        | ArcInstr::BurdenDecPartial { .. }
        | ArcInstr::BurdenDecField { .. }
        | ArcInstr::BurdenDecVariant { .. }
        // Project is a borrow whose destination demand transfers through
        // TF-14. Adding a standard demand here would double-count it.
        | ArcInstr::Project { .. } => SmallVec::new(),

        // Select operands are conditional aliases. Only the condition has a
        // standard demand; destination demand transfers to both values in
        // IA-5 step (1).
        ArcInstr::Select { cond, .. } => SmallVec::from_buf_and_len(
            [BackwardDemand::linear_once(*cond); 4],
            1,
        ),

        // CollectionReuse: old_var consumed + args consumed once.
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push(BackwardDemand::linear_once(*old_var));
            d.extend(
                args.iter()
                    .map(|&var| BackwardDemand::linear_once(var)),
            );
            d
        }

        // IsShared observes logical sharing; Reset consumes the source owner.
        ArcInstr::IsShared { var, .. } | ArcInstr::Reset { var, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*var); 4], 1)
        }

        // Set: base + value each read once.
        ArcInstr::Set { base, value, .. } => {
            let mut d = SmallVec::new();
            d.push(BackwardDemand::linear_once(*base));
            d.push(BackwardDemand::linear_once(*value));
            d
        }

        // SetTag: base read once.
        ArcInstr::SetTag { base, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*base); 4], 1)
        }

        // Reuse: token consumed + args consumed once.
        ArcInstr::Reuse { token, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push(BackwardDemand::linear_once(*token));
            d.extend(
                args.iter()
                    .map(|&var| BackwardDemand::linear_once(var)),
            );
            d
        }
    }
}

/// Compute backward demand contributions for a terminator.
pub fn backward_terminator_demands(term: &ArcTerminator) -> SmallVec<[BackwardDemand; 4]> {
    match term {
        // Return: value demanded once.
        ArcTerminator::Return { value } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*value); 4], 1)
        }

        // Jump: args flow to target block params.
        ArcTerminator::Jump { args, .. } => args
            .iter()
            .map(|&var| BackwardDemand::linear_once(var))
            .collect(),

        // Branch: cond read once.
        ArcTerminator::Branch { cond, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*cond); 4], 1)
        }

        // Switch: scrutinee read once (tag check).
        ArcTerminator::Switch { scrutinee, .. } => {
            SmallVec::from_buf_and_len([BackwardDemand::linear_once(*scrutinee); 4], 1)
        }

        // Invoke: all args demanded once (conservative).
        ArcTerminator::Invoke { args, .. } => args
            .iter()
            .map(|&var| BackwardDemand::linear_once(var))
            .collect(),

        // InvokeIndirect: closure + all args demanded once.
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push(BackwardDemand::linear_once(*closure));
            d.extend(args.iter().map(|&var| BackwardDemand::linear_once(var)));
            d
        }

        // Terminal: no uses.
        ArcTerminator::Resume | ArcTerminator::Unreachable => SmallVec::new(),
    }
}

// Logical ownership-event predicates

/// Whether a logical release event is unnecessary at this program point.
///
/// If cardinality is `Absent` or consumption is `Dead`, the value has
/// no future uses and no surviving ownership obligation at this point.
pub fn is_release_event_unnecessary(state: &AimsState) -> bool {
    state.cardinality == Cardinality::Absent || state.consumption == Consumption::Dead
}

/// Whether an additional logical owner-credit event can be elided at a use
/// site.
///
/// DP-3: `cardinality = Once ∧ consumption ∈ {Linear, Affine}`. A value used
/// exactly once is not duplicated, so no additional credit is required at its
/// single use, whether that use MOVES it (`Linear`: the consumer takes
/// ownership and assumes its release) or BORROWS it (`Affine`: read
/// non-consumingly, then released by its own RL-2 scope-exit event). Neither
/// creates a new owner. `Unrestricted` is excluded (co-occurs only with
/// `Many`); `Dead` is excluded by the `Once` gate. The historical proof name
/// is `DP3_is_rc_inc_elidable_table`.
pub fn is_additional_credit_elidable(state: &AimsState) -> bool {
    state.cardinality == Cardinality::Once
        && (state.consumption == Consumption::Linear || state.consumption == Consumption::Affine)
}

/// Determine the logical COW mutation obligation from uniqueness.
///
/// - `Unique` → same-identity mutation is permitted
/// - `MaybeShared` → a physical sharing observation is required
/// - `Shared` → mutation must be isolated from aliases
pub fn cow_obligation_from_uniqueness(uniqueness: Uniqueness) -> CowMutationObligation {
    match uniqueness {
        Uniqueness::Unique => CowMutationObligation::SameIdentityPermitted,
        Uniqueness::MaybeShared => CowMutationObligation::SharingObservationRequired,
        Uniqueness::Shared => CowMutationObligation::AliasIsolationRequired,
    }
}

/// Backend-neutral COW obligation frozen by AIMS.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CowMutationObligation {
    /// The facts permit mutation while preserving the same logical identity.
    SameIdentityPermitted,
    /// The physical plan must observe sharing before selecting an action.
    SharingObservationRequired,
    /// The physical plan must isolate the mutation from existing aliases.
    AliasIsolationRequired,
}

// Constraint predicates

/// Lattice-level prerequisite for in-place mutation: `Owned + Unique`.
///
/// This is NOT the full DP-5 predicate — the full decision also requires
/// `no_active_overlapping_borrows(var, field, point)` (checked by
/// `has_borrows_from_aggregate()` + `is_borrow_disjoint_from_siblings()`
/// in `emit_rc/cow.rs`). The full DP-5 decision lives in `decide_cow()`
/// (`realize/decide.rs`), which gates on this lattice check plus the
/// borrow overlap check.
///
/// Used by lattice property tests and proptests for the lattice-level
/// invariant: `Owned + Unique` is necessary (but not sufficient) for
/// in-place mutation.
pub fn is_owned_and_unique(state: &AimsState) -> bool {
    state.access == AccessClass::Owned && state.uniqueness == Uniqueness::Unique
}

// State update helpers

/// Update a captured variable's state for `PartialApply`.
///
/// Locality is closure-aware (effect computation): captured args inherit the
/// closure's own locality — if the closure stays function-local, captured
/// args need only `FunctionLocal`; if it escapes to the heap, captured
/// args get `HeapEscaping`. The closure's state comes from the backward
/// analysis (how the closure variable is demanded downstream).
///
/// If the closure is invoked at most once (`cardinality <= Once`),
/// captured values preserve uniqueness — this is the `OxCaml` "lock"
/// mechanism (LAM rule). A once-closure invokes captured values exactly
/// once, so no duplication occurs. The consumption dimension is
/// orthogonal: a closure with `Affine` consumption (may be dropped
/// without use) still only invokes captured values at most once.
///
/// Returns the input unchanged for scalar variables.
pub fn capture_state_update(current: &AimsState, closure_state: &AimsState) -> AimsState {
    if current.is_scalar() {
        return *current;
    }
    let mut state = *current;
    state.access = AccessClass::Owned;

    // Once-closure optimization (OxCaml LAM rule): if the closure is
    // invoked at most once (cardinality <= Once), captured variables are
    // used at most once through the closure, preserving linearity and
    // uniqueness. The consumption dimension (whether the closure may be
    // dropped) is orthogonal — a dropped closure uses captured vars
    // zero times, not additionally.
    if closure_state.cardinality <= Cardinality::Once {
        // Captured var is used through the closure at most once.
        // Keep consumption/cardinality from current state (don't widen).
        // Only ensure at least Affine (may be dropped if closure is dropped).
        if state.consumption < Consumption::Affine {
            state.consumption = Consumption::Affine;
        }
        if state.cardinality < Cardinality::Once {
            state.cardinality = Cardinality::Once;
        }
    } else {
        // Multi-use closure: captured values may be used many times.
        state.consumption = Consumption::Unrestricted;
        state.cardinality = Cardinality::Many;
    }

    // Closure-aware locality: captured vars inherit the closure's locality.
    // No artificial FunctionLocal floor — a block-local closure capturing a
    // block-local variable preserves BlockLocal (both scoped to the same block).
    // Per TF-13.
    if state.locality < closure_state.locality {
        state.locality = closure_state.locality;
    }

    state.canonicalize();
    state
}

/// State for a consumed variable (after `CollectionReuse` consumes `old_var`,
/// or `Reset` consumes its source).
pub fn consumed_state() -> AimsState {
    let mut state = AimsState {
        access: AccessClass::Owned,
        consumption: Consumption::Dead,
        cardinality: Cardinality::Absent,
        uniqueness: Uniqueness::Unique,
        locality: Locality::BlockLocal,
        shape: ShapeClass::NonReusable,
        effect: EffectClass::NONE,
    };
    state.canonicalize();
    state
}

// Helpers

/// Map a [`CtorKind`] to a [`ShapeClass`]. Canonical home for the
/// ctor-to-shape classification (collection literals → `CollectionBuffer`,
/// struct/variant → `ReusableCtor`, tuple/closure → `NonReusable`); consumed
/// by `TF-3` Construct/Reuse transfer and post-convergence shape backfill.
pub(crate) fn shape_from_ctor(ctor: &CtorKind) -> ShapeClass {
    match ctor {
        CtorKind::Struct(_) => ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        CtorKind::EnumVariant { .. } => ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        CtorKind::ListLiteral | CtorKind::SetLiteral | CtorKind::MapLiteral => {
            ShapeClass::CollectionBuffer
        }
        CtorKind::Tuple | CtorKind::Closure { .. } => ShapeClass::NonReusable,
    }
}
