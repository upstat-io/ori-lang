//! Borrowed-alias and COW-inc scans: which borrowed aliases require a
//! caller-side keep-alive inc because a COW-method mutator consumes the
//! aliased collection.

use ori_ir::Name;
use rustc_hash::FxHashSet;

use crate::ir::{ArcFunction, ArcInstr, ArcValue, ArcVarId, LitValue, ValueRepr};
use crate::ownership::Ownership;

/// Compute the set of borrowed-derived vars: borrowed parameters plus every
/// `Let { Var(src) }` alias AND every `Project { value: src }` borrow-view
/// transitively reachable from one. A borrowed value carries no RC obligation
/// (`Spec: Annex E §AIMS RL-2`):
/// - TF-2 propagates a source's borrow to its `Let { Var }` alias.
/// - TF-4 makes a `Project` dst Borrowed, inheriting the source's uniqueness /
///   locality; a Project of a borrowed source is itself a borrow-view that owns
///   no allocation, so a last-use `BurdenDec` on it is a double-free.
///
/// `collect_owned_burdens` already excludes borrowed PARAMS; this set
/// additionally excludes their local aliases AND their borrow-views.
///
/// Project propagation is SOURCE-GATED, NOT a blanket Project-dst exclusion: a
/// dst is marked borrowed ONLY when its `value` source is itself in the borrowed
/// set. A blanket exclusion would be unsafe — a `Project` of an OWNED source may
/// carry an RC obligation (RL-15a project-escape inc; RL-33 projection promotion
/// can flow ownership UP a projection chain). Gating on the source's
/// borrowed-ness matches TF-4 exactly: TF-4 inherits the source's access, so a
/// Borrowed source yields a Borrowed projection while an Owned source does not.
/// Forward fixpoint over both edge kinds (a 1-hop alias / projection of a
/// borrowed alias is borrowed too).
pub(crate) fn compute_borrowed_alias_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut borrowed: FxHashSet<ArcVarId> = func
        .params
        .iter()
        .filter(|p| matches!(p.ownership, Ownership::Borrowed))
        .map(|p| p.var)
        .collect();
    if borrowed.is_empty() {
        return borrowed;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for block in &func.blocks {
            for instr in &block.body {
                let edge = match instr {
                    ArcInstr::Let {
                        dst,
                        value: ArcValue::Var(src),
                        ..
                    } => Some((*dst, *src)),
                    // TF-4: a Project of a borrowed source is a borrow-view.
                    // Source-gated — only propagates when `src` is borrowed.
                    ArcInstr::Project { dst, value, .. } => Some((*dst, *value)),
                    _ => None,
                };
                if let Some((dst, src)) = edge {
                    if borrowed.contains(&src) && borrowed.insert(dst) {
                        changed = true;
                    }
                }
            }
        }
    }
    borrowed
}

/// Compute the step-1 COW-inc set + the step-2 COW-mutator-release-gate names.
///
/// - `cow_inc`: `compute_cow_inc_borrowed_aliases` (borrowed-alias COW-MUTATOR
///   receivers needing a step-1 `BurdenInc` per RL-1).
/// - `cow_mutators`: the COW-MUTATOR method names (`all_cow_method_names` MINUS
///   `iter`) gating step-2 release — a COW-mutator's result is fresh so the
///   original receiver is released by step 2; `iter` is released by the runtime
///   `ori_iter_drop`, never a burden-dec (the iterator owns the buffer).
pub(super) fn compute_cow_inc_and_mutators(
    func: &ArcFunction,
    borrowed_aliases: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> (FxHashSet<ArcVarId>, FxHashSet<Name>) {
    let cow_inc = compute_cow_inc_borrowed_aliases(func, borrowed_aliases, interner);
    let mut cow_mutators = crate::borrow::all_cow_method_names(interner);
    cow_mutators.remove(
        &interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name()),
    );
    (cow_inc, cow_mutators)
}

/// Step-1 COW-inc set: borrowed-param-alias vars consumed as the RECEIVER (arg
/// 0) of a COW-MUTATING builtin `Apply` / `Invoke`. Per AIMS RL-1, a COW-mutation
/// operand re-read after the call is a DUPLICATING use whose inc is NOT elidable
/// (`AimsProof.Realization::RL1_emit_iff_not_elidable`).
///
/// Discriminator = COW-MUTATING-METHOD-NAME on the `RcPtr` receiver: the COW
/// mutators (`push`, `insert`, `set`, `remove`, `pop`, `sort`, `reverse`, `add`,
/// `concat`, map+set COW per `crate::borrow::all_cow_method_names`). These
/// builtins read the receiver's runtime refcount to choose copy-vs-mutate-in-place;
/// the borrowed alias carries rc=1 absent an inc, so the helper mutates the
/// SHARED buffer in place, corrupting the caller's still-live value. The inc
/// raises the runtime rc to at least 2 so the helper COPIES.
///
/// The METHOD-NAME gate is load-bearing and narrower than a raw owned-position
/// `RcPtr` test: `iter` is a consuming-receiver builtin (it takes the buffer
/// owned into the iterator state) but is NOT a COW mutation — it does not realloc
/// on a refcount read, so a COW-inc on an `iter` receiver leaks (the iterator's
/// `ori_iter_drop` releases its hold but the orphaned inc never decs). `slice`
/// and `substring` SHARE the receiver's buffer and take it BORROWED (already
/// excluded by the receiver-owned-position requirement). Only COW mutators qualify.
///
/// `Set` / `SetTag` (TF-15 in-place mutation) on a borrowed-alias base are COW
/// mutations too — the codegen guards them with `IsShared` and copies on a
/// shared base; the inc keeps the runtime rc ≥ 2 so the guard copies.
///
/// `borrowed_aliases` is the precomputed `compute_borrowed_alias_vars` fixpoint
/// set (borrowed params + their Let-Var/Project transitive closure). Only its
/// members qualify — an OWNED value already gets a normal `BurdenInc` via
/// `emit_owned_position_incs` / `terminator_inc_vars`.
pub(crate) fn compute_cow_inc_borrowed_aliases(
    func: &ArcFunction,
    borrowed_aliases: &FxHashSet<ArcVarId>,
    interner: &ori_ir::StringInterner,
) -> FxHashSet<ArcVarId> {
    let mut cow_inc: FxHashSet<ArcVarId> = FxHashSet::default();
    if borrowed_aliases.is_empty() {
        return cow_inc;
    }
    // COW-MUTATOR set = `all_cow_method_names` MINUS `iter` (A' scope). A
    // COW-mutator (push / insert / set / remove / pop / sort / reverse / add /
    // concat / map+set COW) reads the RcPtr receiver's refcount to choose
    // copy-vs-mutate-in-place; a borrowed-alias receiver carries rc=1 absent an
    // inc → the helper mutates the SHARED buffer in place, corrupting the
    // caller's still-live value. The COW-inc raises rc ≥ 2 so the helper COPIES;
    // step 2 emits the paired freeing dec (the result is fresh, nothing else
    // holds the original). `iter` is EXCLUDED: it moves the buffer into the
    // iterator state whose lifecycle is owned by the runtime `ori_iter_drop`
    // (+ the caller's borrow) — the iterator-handle freeing is the runtime drop's
    // job. A COW-inc on an `iter` receiver mis-balances multi-call / unwind
    // iterator shapes (the runtime drop + caller borrow already account for the
    // buffer).
    let mut cow_methods = crate::borrow::all_cow_method_names(interner);
    cow_methods.remove(
        &interner.intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::Iter.name()),
    );
    let is_rcptr = |v: ArcVarId| matches!(func.var_repr(v), Some(ValueRepr::RcPointer));
    // The COW receiver is arg 0 of the call. A COW-mutating builtin re-reads its
    // refcount; the borrowed-alias receiver needs the inc so the COW helper copies.
    let consider_call = |callee: &Name, args: &[ArcVarId], cow_inc: &mut FxHashSet<ArcVarId>| {
        if !cow_methods.contains(callee) {
            return;
        }
        if let Some(&recv) = args.first() {
            if borrowed_aliases.contains(&recv) && is_rcptr(recv) {
                cow_inc.insert(recv);
            }
        }
    };
    for block in &func.blocks {
        for instr in &block.body {
            match instr {
                ArcInstr::Apply {
                    func: callee, args, ..
                } => {
                    consider_call(callee, args, &mut cow_inc);
                }
                // TF-15 in-place mutation on a borrowed-alias base: the codegen
                // `IsShared`-guards it and copies on a shared base; the inc keeps
                // rc ≥ 2 so the guard copies vs corrupting the caller's value.
                ArcInstr::Set { base, .. } | ArcInstr::SetTag { base, .. } => {
                    if borrowed_aliases.contains(base) && is_rcptr(*base) {
                        cow_inc.insert(*base);
                    }
                }
                _ => {}
            }
        }
        if let crate::ir::ArcTerminator::Invoke {
            func: callee, args, ..
        } = &block.terminator
        {
            consider_call(callee, args, &mut cow_inc);
        }
    }
    cow_inc
}

/// Compute the set of vars defined by a scalar `Literal`: a `Let { value:
/// ArcValue::Literal(lit) }` where `lit` is NOT `LitValue::String`. Such a var
/// is a scalar sentinel carrying NO RC burden regardless of its declared
/// `var_types[v]` (`Spec: Annex E §AIMS L-9`; TF-1 `Let { Literal } -> SCALAR`).
///
/// Mirrors `fresh_site_burden_inc_dst` (`emit.rs`), which emits a FRESH-site
/// `BurdenInc` for a `Let { Literal }` ONLY when the literal is `String` (heap
/// str body); every scalar literal (`Int`/`Float`/`Bool`/`Char`/`Duration`/
/// `Size`/`Unit`/`Null`) emits no inc. `collect_owned_burdens` keys DEC-side
/// membership on the declared type, so a var typed as a heap aggregate but
/// defined `Literal(Int(0))` (the `__iter_next` element-type-marker scratch slot)
/// would carry an unbalanced `BurdenDec`. Excluding the scalar-`Literal` set
/// from `owned_vars_needing_rc` restores INC/DEC symmetry on the definition grain.
///
/// `String` literals are NOT excluded — their heap str body carries RC and the
/// inc side emits a paired `BurdenInc` for them.
pub(super) fn compute_scalar_literal_vars(func: &ArcFunction) -> FxHashSet<ArcVarId> {
    let mut scalar_lits: FxHashSet<ArcVarId> = FxHashSet::default();
    for block in &func.blocks {
        for instr in &block.body {
            if let ArcInstr::Let {
                dst,
                value: ArcValue::Literal(lit),
                ..
            } = instr
            {
                if !matches!(lit, LitValue::String(_)) {
                    scalar_lits.insert(*dst);
                }
            }
        }
    }
    scalar_lits
}
