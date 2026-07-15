//! ARC IR instruction definitions.
//!
//! [`ArcInstr`] represents a single instruction in an ARC IR basic block.
//! Most produce a value bound to a `dst` variable. Logical owner-credit and
//! cleanup events currently use the transitional `RcInc`/`RcDec` carrier and
//! are subsequently pair-elided where valid.

use ori_ir::canon::MonoInstanceId;
use ori_types::Idx;

use super::{ArcValue, ArcVarId, ArgOwnership, CtorKind, RcAtomicity, RcStrategy};

/// A single instruction in an ARC IR basic block.
///
/// Instructions are executed sequentially within a block. Most produce
/// a value bound to a `dst` variable. `RcInc`/`RcDec` are compatibility names
/// for the current carrier of logical ownership events, not a backend mandate.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "cache", derive(serde::Serialize, serde::Deserialize))]
pub enum ArcInstr {
    /// Bind a value to a variable: `let dst: ty = value`.
    Let {
        dst: ArcVarId,
        ty: Idx,
        value: ArcValue,
    },

    /// Direct function call: `let dst: ty = func(args...)`.
    Apply {
        dst: ArcVarId,
        ty: Idx,
        func: ori_ir::Name,
        args: Vec<ArcVarId>,
        /// Per-argument ownership at this call site.
        /// Parallel to `args`: `arg_ownership[i]` describes `args[i]`.
        /// Defaults to all `Owned`; populated by ownership-event realization.
        arg_ownership: Vec<ArgOwnership>,
        /// Abstract dispatch index for generic-instantiated calls.
        /// `Some(id)` when the call resolved to a specific monomorphic
        /// instance during type checking; `None` otherwise (most builtins
        /// and non-generic calls). Sourced from
        /// `CanonResult.mono_dispatch_map_can` during ARC lowering. Physical
        /// realization consumers use it to identify the selected instance;
        /// `ori_llvm` maps it to `TypedModule.mono_instances[id.0]` and calls
        /// `mangle_mono_name` locally, keeping LLVM names inside that
        /// projection. The evaluator independently reads the canonical
        /// dispatch map and does not consume this post-AIMS carrier.
        mono_instance_id: Option<MonoInstanceId>,
    },

    /// Indirect call through a closure: `let dst: ty = closure(args...)`.
    ApplyIndirect {
        dst: ArcVarId,
        ty: Idx,
        closure: ArcVarId,
        args: Vec<ArcVarId>,
        /// Per-argument ownership at this indirect call site.
        /// Parallel to `args`: `arg_ownership[i]` describes `args[i]`.
        /// Empty before annotation; populated by ownership-event realization.
        /// Unlike `Apply`, empty defaults to all-Borrowed (conservative for
        /// unknown callees — caller retains cleanup responsibility).
        arg_ownership: Vec<ArgOwnership>,
    },

    /// Partial application / closure creation: `let dst: ty = func(args...)`.
    ///
    /// Creates a closure that captures `args` and awaits remaining arguments.
    PartialApply {
        dst: ArcVarId,
        ty: Idx,
        func: ori_ir::Name,
        args: Vec<ArcVarId>,
    },

    /// Field projection: `let dst: ty = value.field`.
    Project {
        dst: ArcVarId,
        ty: Idx,
        value: ArcVarId,
        field: u32,
    },

    /// Constructor application: `let dst: ty = ctor(args...)`.
    Construct {
        dst: ArcVarId,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
    },

    // Logical ownership events with transitional physical adapter fields.
    /// Add `count` logical ownership credits. `strategy` and `atomicity` retain
    /// shipped adapter behavior without downstream Pool queries; they are not
    /// AIMS policy and move into validated physical plans in the production seam.
    RcInc {
        var: ArcVarId,
        count: u32,
        strategy: RcStrategy,
        atomicity: RcAtomicity,
    },

    /// Consume one logical ownership credit and perform its cleanup obligation.
    /// `strategy` and `atomicity` are transitional physical adapter fields, not
    /// backend-neutral AIMS facts.
    RcDec {
        var: ArcVarId,
        strategy: RcStrategy,
        atomicity: RcAtomicity,
    },

    /// Realized (Phase-7-lowered) form of `BurdenDecPartial`: partial-move
    /// drop of `var`'s owned fields, skipping the moved-out `skip_fields`
    /// top-level indices. Codegen emits the per-field / per-variant drop
    /// glue; NOT a whole-var `RcDec` (the glue walks interior fields only —
    /// a whole-var dec would double-drop). Spelled out of the burden census:
    /// the Step-11 burden-balance ledger counts SURVIVING burden ops, and a
    /// mechanically-lowered op must leave the burden stream (Spec: Annex E
    /// §AIMS RL-comp net-preservation).
    RcDecPartial {
        var: ArcVarId,
        skip_fields: Vec<u32>,
    },

    /// Realized (Phase-7-lowered) form of `BurdenDecField`: release of the
    /// prior value of `base.field` before an in-place `Set` store. Codegen
    /// emits the single-field drop glue. See `RcDecPartial` for the
    /// realized-spelling contract.
    RcDecField { base: ArcVarId, field: u32 },

    /// Realized (Phase-7-lowered) form of `BurdenDecVariant`: `SetTag`
    /// old-variant payload release for `var` before the tag change clobbers
    /// the discriminant. Codegen emits the per-variant drop-glue walk. See
    /// `RcDecPartial` for the realized-spelling contract.
    RcDecVariant { var: ArcVarId },

    /// Burden-increment marker. Trivial side-effect-only annotation emitted
    /// by Phase 5 ARC lowering at every owned-arg transfer point. Carries
    /// only the SSA variable that is the subject of the burden transfer —
    /// no class info, no transitive markers. Parallel to `RcInc` but tracks
    /// the logical burden lattice rather than asserting a physical counter.
    BurdenInc { var: ArcVarId },

    /// Burden-decrement marker. Trivial side-effect-only annotation emitted
    /// by Phase 5 ARC lowering at every last-use along every reachable CFG
    /// path. Carries only the SSA variable that is the subject of the burden
    /// release — see `BurdenInc` above for shape contract.
    BurdenDec { var: ArcVarId },

    /// Field-aware partial-drop sibling to `BurdenDec`. Carries
    /// `skip_fields: Vec<u32>` naming the top-level field indices of
    /// `var`'s `Burden::owned_fields()` whose drop should be skipped
    /// (because field-projection transfers moved them out per RL-2
    /// two-stage rule). Emission constructs this variant when
    /// `moved_out_fields[var]` is non-empty but not full-move; codegen
    /// skips `skip_fields` when walking the `owned_fields` drop-glue.
    /// `Vec<u32>` chosen over `SmallVec<[u32; 4]>` because smallvec
    /// workspace dep lacks `serde` feature; cache-feature derives require
    /// Serializable payloads.
    BurdenDecPartial {
        var: ArcVarId,
        skip_fields: Vec<u32>,
    },

    /// Set old-value drop emission: `BurdenDec(base.field.old_value)`
    /// before Set mutation. Symmetric with `BurdenInc { value }` for Set;
    /// the `BurdenInc` transfers ownership of the new value INTO the field
    /// position, the `BurdenDecField` releases ownership of the prior value
    /// OUT of that position before the in-place store. Carries `base` SSA
    /// var + `field: u32` top-level index; codegen iterates
    /// `Burden::owned_fields()` entries whose `field_path` has `field` as
    /// its top-level prefix and emits per-subtree releases, currently spelled
    /// `RcDec`, against the loaded prior values.
    BurdenDecField { base: ArcVarId, field: u32 },

    /// `SetTag` old-variant drop emission. Whole-var pattern (NOT
    /// field-positional) per TF-15a + RL-10 — `SetTag` invalidates ALL
    /// payload fields of `base`'s current variant before the tag change
    /// clobbers the discriminant. Emitted BEFORE the `SetTag` instruction
    /// so codegen can GEP the tag field + load the current discriminant +
    /// dispatch per-variant burden walk BEFORE the store overwrites the
    /// tag. Mirrors the `BurdenInc` / `BurdenDec` / `BurdenDecPartial`
    /// cluster (whole-var burden walks); does NOT mirror `BurdenDecField`
    /// (field-positional). AIMS Invariant 5 case (b) — extends `ArcInstr`
    /// enum on the same dimension as `BurdenDecPartial` / `BurdenDec`;
    /// no parallel emission, no shadow tracker.
    BurdenDecVariant { var: ArcVarId },

    // Reuse operations (inserted by reuse emission pass)
    /// Test whether the selected physical plan can satisfy the value's logical
    /// unique-owner fast path. Result is a `bool` bound to `dst`; AIMS fixes
    /// the sharing observation, not a counter or header representation.
    /// `IsShared` is the transitional carrier spelling for that query.
    IsShared { dst: ArcVarId, var: ArcVarId },

    /// In-place field update: `base.field = value`.
    /// Only valid when the object is uniquely owned.
    Set {
        base: ArcVarId,
        field: u32,
        value: ArcVarId,
    },

    /// In-place tag update for enum variants: `base.tag = tag`.
    /// Only valid when the object is uniquely owned.
    SetTag { base: ArcVarId, tag: u64 },

    /// Reset intermediate: marks a value for potential reuse.
    /// Expanded by reuse realization into a sharing query plus conditional
    /// reuse; the current carrier names that query `IsShared`.
    Reset { var: ArcVarId, token: ArcVarId },

    /// Reuse intermediate: construct using a reuse token's memory.
    /// Expanded by reuse emission into conditional alloc-or-reuse.
    Reuse {
        token: ArcVarId,
        dst: ArcVarId,
        ty: Idx,
        ctor: CtorKind,
        args: Vec<ArcVarId>,
    },

    /// Collection buffer reuse: replaces one logical release plus
    /// `Construct(ListLiteral)`; the current carrier spells that release
    /// `RcDec(old)`.
    ///
    /// Unlike struct reuse (whose current carrier expands `Reset`/`Reuse`
    /// through the transitional `IsShared` query),
    /// collection reuse is self-contained. A physical consumer implements the
    /// same unique-versus-shared decision through its validated layout plan.
    /// The shipped LLVM projection currently calls `ori_list_reset_buffer`;
    /// that helper and its counter/header mechanics are not part of this
    /// operation's semantics.
    ///
    /// Only valid for `ListLiteral` and `SetLiteral` constructors.
    CollectionReuse {
        /// The old collection being recycled (its logical release, currently
        /// carried by `RcDec`, was removed).
        old_var: ArcVarId,
        /// Destination variable for the new collection.
        dst: ArcVarId,
        /// Collection type (must be list or set).
        ty: Idx,
        /// Constructor kind (`ListLiteral` or `SetLiteral`).
        ctor: CtorKind,
        /// New element values.
        args: Vec<ArcVarId>,
    },

    /// Conditional value selection: `let dst: ty = if cond then true_val else false_val`.
    ///
    /// Used by decision-tree lowering to represent a value choice without
    /// trivial match-arm blocks. LLVM maps it directly to `select`; other
    /// physical consumers choose their equivalent encoding.
    Select {
        dst: ArcVarId,
        ty: Idx,
        cond: ArcVarId,
        true_val: ArcVarId,
        false_val: ArcVarId,
    },
}

mod methods;
