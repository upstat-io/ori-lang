//! Transfer functions for the AIMS lattice.
//!
//! Each ARC IR instruction has a transfer function that defines how it
//! transforms the [`AimsState`] of variables it touches:
//!
//! - **Forward (definition)**: what state does the destination variable get?
//! - **Backward (demand)**: what cardinality demand does each use add?
//!
//! The dataflow analysis engine (Section 02) applies these functions in
//! its worklist iteration. This module defines only the mathematical rules.
//!
//! References:
//! - Perceus dup/drop placement (Reinking et al., PLDI 2021)
//! - GHC demand analysis `seq_add`/`alt_join` (Sergey et al., POPL 2014)
//! - Lean 4 `updateLiveVars` / `addInc` / `addDec` (Ullrich & de Moura, IFL 2019)

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "tests use expect for clearer failure messages"
)]
mod tests;

use smallvec::SmallVec;

use crate::ir::{ArcInstr, ArcTerminator, ArcValue, ArcVarId, CtorKind};

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
pub fn transfer_def(
    instr: &ArcInstr,
    get_state: &impl Fn(ArcVarId) -> AimsState,
) -> Option<DefTransfer> {
    match instr {
        ArcInstr::Let { value, .. } => Some(transfer_let(value, get_state)),
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

        // Side-effect-only — no defined variable.
        ArcInstr::RcInc { .. }
        | ArcInstr::RcDec { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. } => None,
    }
}

/// `Let { value }` — bind a value to a variable.
///
/// - `Var(v)` → inherit source state
/// - `Literal(_)` → `SCALAR` (no RC)
/// - `PrimOp { .. }` → `SCALAR` (arithmetic on primitives)
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
/// (Section 09.2: precise effect computation.)
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
/// Without interprocedural information (Section 03), the return value
/// is conservatively `(Owned, Unrestricted, Many, MaybeShared)`.
/// Section 03 refines this using `MemoryContract`.
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
/// allocation). The analysis engine (Section 02) separately updates
/// captured args' states via [`capture_state_update`].
///
/// Per-variable effect: `may_alloc = true` — closure creation allocates.
/// (Section 09.2: precise effect computation.)
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

/// Compute backward demand contributions for an instruction.
///
/// Each `(var, cardinality)` pair means the instruction adds that much
/// demand on the variable. The analysis engine applies this via
/// [`Cardinality::seq_add`] when walking backward.
///
/// Based on GHC demand analysis (Sergey et al., POPL 2014):
/// - Sequential: `seq_add` along one execution path
/// - Alternative: `alt_join` at control-flow merge points
pub fn backward_demands(instr: &ArcInstr) -> SmallVec<[(ArcVarId, Cardinality); 4]> {
    match instr {
        ArcInstr::Let { value, .. } => match value {
            ArcValue::Var(v) => SmallVec::from_buf_and_len([(*v, Cardinality::Once); 4], 1),
            ArcValue::Literal(_) => SmallVec::new(),
            ArcValue::PrimOp { args, .. } => args.iter().map(|v| (*v, Cardinality::Once)).collect(),
        },

        // Construct/Apply: each arg consumed once.
        ArcInstr::Construct { args, .. } | ArcInstr::Apply { args, .. } => {
            args.iter().map(|v| (*v, Cardinality::Once)).collect()
        }

        // ApplyIndirect: closure + all args demanded once.
        ArcInstr::ApplyIndirect { closure, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push((*closure, Cardinality::Once));
            d.extend(args.iter().map(|v| (*v, Cardinality::Once)));
            d
        }

        // PartialApply: captured arg demand is handled entirely by
        // `capture_state_update` in block.rs, which sets precise
        // access/consumption/cardinality/locality based on the closure's
        // own demand state. Returning demand here would double-count.
        // RC operations: AIMS outputs. During migration, no demand.
        ArcInstr::PartialApply { .. } | ArcInstr::RcInc { .. } | ArcInstr::RcDec { .. } => {
            SmallVec::new()
        }

        // Project: one read of the source.
        ArcInstr::Project { value, .. } => {
            SmallVec::from_buf_and_len([(*value, Cardinality::Once); 4], 1)
        }

        // Select: cond + both branches each read once.
        // When true_val == false_val, emit one demand to avoid double-counting
        // (seq_add would produce Many instead of the correct Once).
        ArcInstr::Select {
            cond,
            true_val,
            false_val,
            ..
        } => {
            let mut d = SmallVec::new();
            d.push((*cond, Cardinality::Once));
            d.push((*true_val, Cardinality::Once));
            if true_val != false_val {
                d.push((*false_val, Cardinality::Once));
            }
            d
        }

        // CollectionReuse: old_var consumed + args consumed once.
        ArcInstr::CollectionReuse { old_var, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push((*old_var, Cardinality::Once));
            d.extend(args.iter().map(|v| (*v, Cardinality::Once)));
            d
        }

        // IsShared/Reset: one read (refcount check / var consumed).
        ArcInstr::IsShared { var, .. } | ArcInstr::Reset { var, .. } => {
            SmallVec::from_buf_and_len([(*var, Cardinality::Once); 4], 1)
        }

        // Set: base + value each read once.
        ArcInstr::Set { base, value, .. } => {
            let mut d = SmallVec::new();
            d.push((*base, Cardinality::Once));
            d.push((*value, Cardinality::Once));
            d
        }

        // SetTag: base read once.
        ArcInstr::SetTag { base, .. } => {
            SmallVec::from_buf_and_len([(*base, Cardinality::Once); 4], 1)
        }

        // Reuse: token consumed + args consumed once.
        ArcInstr::Reuse { token, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push((*token, Cardinality::Once));
            d.extend(args.iter().map(|v| (*v, Cardinality::Once)));
            d
        }
    }
}

/// Compute backward demand contributions for a terminator.
pub fn backward_terminator_demands(term: &ArcTerminator) -> SmallVec<[(ArcVarId, Cardinality); 4]> {
    match term {
        // Return: value demanded once.
        ArcTerminator::Return { value } => {
            SmallVec::from_buf_and_len([(*value, Cardinality::Once); 4], 1)
        }

        // Jump: args flow to target block params.
        ArcTerminator::Jump { args, .. } => args.iter().map(|v| (*v, Cardinality::Once)).collect(),

        // Branch: cond read once.
        ArcTerminator::Branch { cond, .. } => {
            SmallVec::from_buf_and_len([(*cond, Cardinality::Once); 4], 1)
        }

        // Switch: scrutinee read once (tag check).
        ArcTerminator::Switch { scrutinee, .. } => {
            SmallVec::from_buf_and_len([(*scrutinee, Cardinality::Once); 4], 1)
        }

        // Invoke: all args demanded once (conservative).
        ArcTerminator::Invoke { args, .. } => {
            args.iter().map(|v| (*v, Cardinality::Once)).collect()
        }

        // InvokeIndirect: closure + all args demanded once.
        ArcTerminator::InvokeIndirect { closure, args, .. } => {
            let mut d = SmallVec::with_capacity(1 + args.len());
            d.push((*closure, Cardinality::Once));
            d.extend(args.iter().map(|v| (*v, Cardinality::Once)));
            d
        }

        // Terminal: no uses.
        ArcTerminator::Resume | ArcTerminator::Unreachable => SmallVec::new(),
    }
}

// RC reasoning predicates

/// Whether an RC decrement is unnecessary at this program point.
///
/// If cardinality is `Absent` or consumption is `Dead`, the value has
/// no future uses — a dec would be redundant.
pub fn is_rc_dec_unnecessary(state: &AimsState) -> bool {
    state.cardinality == Cardinality::Absent || state.consumption == Consumption::Dead
}

/// Whether an RC increment can be elided at a use site.
///
/// If cardinality is `Once` and consumption is `Linear`, the value has
/// a single consumer that moves it — no inc needed.
pub fn is_rc_inc_elidable(state: &AimsState) -> bool {
    state.cardinality == Cardinality::Once && state.consumption == Consumption::Linear
}

/// Determine COW mode from the uniqueness dimension.
///
/// - `Unique` → static unique (no runtime check)
/// - `MaybeShared` → dynamic (runtime uniqueness check)
/// - `Shared` → static shared (always copy)
pub fn cow_mode_from_uniqueness(uniqueness: Uniqueness) -> CowModeFromAims {
    match uniqueness {
        Uniqueness::Unique => CowModeFromAims::StaticUnique,
        Uniqueness::MaybeShared => CowModeFromAims::Dynamic,
        Uniqueness::Shared => CowModeFromAims::StaticShared,
    }
}

/// COW mode as determined by AIMS analysis.
///
/// Maps to [`crate::uniqueness::CowMode`] at pipeline integration
/// (Section 06). Defined separately to avoid coupling the lattice
/// module to the existing uniqueness pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CowModeFromAims {
    /// RC provably 1 — mutate in place, no check.
    StaticUnique,
    /// Runtime uniqueness check needed.
    Dynamic,
    /// RC provably > 1 — always copy.
    StaticShared,
}

// Constraint predicates

/// Whether a variable can be mutated in place (`Set`/`SetTag`).
///
/// Mutation requires `(Owned, *, *, Unique)` — owned and uniquely
/// referenced. `MaybeShared` needs a COW check first; `Shared` must
/// copy.
pub fn can_mutate_in_place(state: &AimsState) -> bool {
    state.access == AccessClass::Owned && state.uniqueness == Uniqueness::Unique
}

// State update helpers

/// Update a captured variable's state for `PartialApply`.
///
/// Locality is closure-aware (Section 09.2): captured args inherit the
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

    // Closure-aware locality: captured vars inherit the closure's locality,
    // but at least FunctionLocal (they escape the defining block into the
    // closure's scope).
    let capture_locality = closure_state.locality.max(Locality::FunctionLocal);
    if state.locality < capture_locality {
        state.locality = capture_locality;
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

/// Map a [`CtorKind`] to a [`ShapeClass`].
fn shape_from_ctor(ctor: &CtorKind) -> ShapeClass {
    match ctor {
        CtorKind::Struct(_) => ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        CtorKind::EnumVariant { .. } => ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        CtorKind::ListLiteral | CtorKind::SetLiteral | CtorKind::MapLiteral => {
            ShapeClass::CollectionBuffer
        }
        CtorKind::Tuple | CtorKind::Closure { .. } => ShapeClass::NonReusable,
    }
}
