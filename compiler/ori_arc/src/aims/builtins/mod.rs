//! Builtin function contracts for AIMS analysis.
//!
//! Pre-seeds the interprocedural analysis signature map with
//! [`MemoryContract`] entries for builtin methods that are not analyzed
//! intraprocedurally (they have no ARC IR body).
//!
//! [`crate::borrow::builtins`] encodes the same semantic facts as ownership
//! sets in a different format.

mod runtime;
#[cfg(test)]
mod tests;

use runtime::seed_internal_runtime_contracts;

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnAliasShape,
    ReturnContract,
};
use super::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};

/// Adds contracts for builtin methods that have no ARC IR bodies.
///
/// Contract ownership derives from [`BuiltinOwnershipSets`].
pub(crate) fn seed_builtin_contracts(
    sigs: &mut FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
    interner: &StringInterner,
) {
    // Why: COW ownership takes precedence when an unqualified method name is also borrowing.

    // IndexSet `updated(key, value)`: receiver consumed-and-returned, key
    // borrowed, value MOVED into the collection. Seeded BEFORE the generic
    // COW loops — `updated` is also in `consuming_receiver`, whose 1-param
    // base contract cannot express the value-param ownership transfer.
    // One uniform contract across `[T]` / `[T, max N]` / `{K: V}` (fixed
    // lists erase to the List tag). Without the Owned value param, AIMS
    // emits an erroneous RcDec on the moved value (double-free on the
    // unique in-place path; reference-balance leak on copy paths).
    for &name in &builtins.consuming_third_arg {
        sigs.entry(name).or_insert_with(cow_indexed_update_contract);
    }

    // COW map/set methods: receiver consumed, other args borrowed.
    // Seeded BEFORE consuming_receiver because some methods (e.g., "remove")
    // appear in both sets — consuming_receiver_only is more specific.
    for &name in &builtins.consuming_receiver_only {
        sigs.entry(name).or_insert_with(cow_receiver_only_contract);
    }

    // COW collection methods: seeded as Borrowed (base contract).
    // `apply_consuming_overrides` then overrides to Owned for List/Map/Set
    // receivers. String methods (concat, iter, etc.) stay Borrowed because
    // the runtime borrows string data (Inc's internally, doesn't consume).
    for &name in &builtins.consuming_receiver {
        sigs.entry(name).or_insert_with(|| borrowing_contract(1));
    }

    // Fixed/dynamic list conversions are physical value-identity forwarders:
    // the receiver credit moves directly into the result. They do not mint a
    // second credit like a sharing view. If the source remains live, normal
    // owned-argument duplication funds that separate owner before the call.
    // Seed these before the generic borrowing loop so result and receiver
    // occupy one allocation class and only the returned owner is discharged.
    for name in ["to_dynamic", "to_fixed"] {
        sigs.entry(interner.intern(name))
            .or_insert_with(value_identity_forwarder_contract);
    }

    // Sharing methods: return MaybeShared (shares receiver's backing) and
    // carry the typed sharing-view CREDIT (`returns_sharing_view`).
    // The contract map excludes the bare surface names `take` and `drop`: the
    // name-keyed contract map cannot discriminate the seamless-slice
    // methods from same-named callees with different ownership (a user
    // `Drop::drop`, iterator adapters), so their CREDIT rides the
    // unambiguous runtime names (`ori_list_slice_take` /
    // `ori_list_slice_drop`) in the runtime contract table.
    let sharing = crate::borrow::sharing_builtin_names(interner);
    for name in sharing {
        sigs.entry(name).or_insert_with(sharing_return_contract);
    }

    // Protocol builtins: per-arg ownership from ProtocolBuiltin.
    // Seeded BEFORE borrowing methods because protocol builtins have specific
    // per-arg ownership from the source of truth (ProtocolBuiltin::arg_ownership).
    // Without this priority, __index (2 borrowed params) gets a generic 1-param
    // borrowing_contract(1) from the borrowing set, losing the second param.
    for (&name, arg_ownership) in &builtins.protocol {
        sigs.entry(name)
            .or_insert_with(|| protocol_contract(arg_ownership));
    }

    // Borrowing builtins: receiver borrowed, return conservative.
    // Seeded after COW and protocol methods so specific contracts aren't overwritten.
    for &name in &builtins.borrowing {
        sigs.entry(name).or_insert_with(|| borrowing_contract(1));
    }

    // Internal runtime functions called by ARC IR lowering (not user-facing).
    // These are `ori_*` C functions that would otherwise default to all-borrowed
    // in `compute_arg_ownership`. Where the runtime copies element bytes into a
    // collection buffer (creating another logical owner), the element arg must be
    // Owned so AIMS records the exact transfer/sharing obligation. An RC-backed
    // physical plan may satisfy that obligation with a retain.
    seed_internal_runtime_contracts(sigs, interner);
}

// Contract constructors

/// Borrowing method: receiver borrowed, return conservative.
fn borrowing_contract(num_params: usize) -> MemoryContract {
    MemoryContract {
        params: vec![PARAM_BORROWED; num_params],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// COW map/set method: receiver consumed, other args borrowed, return Unique.
///
/// For operations like `map.remove(key)` where the receiver is COW-consumed
/// but the key is only used for comparison (borrowed).
fn cow_receiver_only_contract() -> MemoryContract {
    // INVARIANT: Every receiver-only COW method has one non-receiver argument.
    MemoryContract {
        params: vec![PARAM_OWNED_LINEAR, PARAM_BORROWED],
        return_info: RETURN_UNIQUE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// COW indexed update (`updated(key, value)` — IndexSet): key borrowed,
/// value consumed (moved into the collection), return Unique.
///
/// The 2-param [`cow_receiver_only_contract`] cannot express the value-param
/// ownership transfer — `updated` is 3-param (`self, key, value`) and the
/// runtime takes ownership of `value` (no caller-side `RcDec` after insert).
///
/// The receiver is seeded Borrowed (COW base-contract idiom, same as the
/// `consuming_receiver` loop): `apply_consuming_overrides` marks it Owned
/// at collection call sites, so the consumed-and-returned receiver shares
/// the proven realization path shared by the COW methods (`set`, `push`).
fn cow_indexed_update_contract() -> MemoryContract {
    MemoryContract {
        params: vec![PARAM_BORROWED, PARAM_BORROWED, PARAM_OWNED_LINEAR],
        return_info: RETURN_UNIQUE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// Method returning a value that shares the receiver's logical storage identity.
///
/// E.g., `slice`, `substring` — the returned value aliases the receiver's
/// storage identity, so its uniqueness is `MaybeShared`. The receiver is
/// **borrowed**; the result receives its own logical credit while the caller
/// retains the receiver credit. A physical plan chooses how to realize both.
fn sharing_return_contract() -> MemoryContract {
    MemoryContract {
        params: vec![ParamContract {
            may_share: true,
            ..PARAM_BORROWED
        }],
        return_info: ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            // Typed buffer-provenance CREDIT: the view result mints its own
            // +1 on the receiver's backing allocation.
            // Spec: Annex E §AIMS §12 (sharing-view producer = CREDIT).
            returns_sharing_view: true,
            ..ReturnContract::CONSERVATIVE
        },
        // The result credit is backed by the receiver's storage identity.
        // Call transfer must therefore invalidate a pre-call Unique fact on
        // the borrowed receiver as well as classify the result MaybeShared.
        // Without this effect, `list.slice(..); list.push(..)` can select the
        // StaticUnique COW path while the slice still owns the same buffer.
        effects: EffectSummary {
            may_share: true,
            ..EffectSummary::default()
        },
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// Ownership-transfer contract for a builtin that returns its argument
/// unchanged at the physical level.
fn value_identity_forwarder_contract() -> MemoryContract {
    MemoryContract {
        params: vec![ParamContract {
            transfers_through_return: true,
            return_alias: Some(ReturnAliasShape::Direct),
            ..PARAM_OWNED_LINEAR
        }],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// Protocol builtin: per-arg ownership from `ProtocolArgOwnership`.
fn protocol_contract(
    arg_ownership: &[ori_ir::builtin_constants::protocol::ProtocolArgOwnership],
) -> MemoryContract {
    use ori_ir::builtin_constants::protocol::ProtocolArgOwnership;
    let params = arg_ownership
        .iter()
        .map(|o| match o {
            ProtocolArgOwnership::Borrowed => PARAM_BORROWED,
            ProtocolArgOwnership::Owned => PARAM_OWNED_LINEAR,
        })
        .collect();
    MemoryContract {
        params,
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

// Common parameter contract constants

/// Borrowed parameter: read-only, used once.
const PARAM_BORROWED: ParamContract = ParamContract {
    access: AccessClass::Borrowed,
    consumption: Consumption::Dead,
    cardinality: Cardinality::Once,
    may_escape: false,
    may_share: false,
    locality_bound: Locality::Unknown,
    uniqueness: Uniqueness::MaybeShared,
    transfers_through_return: false,
    return_alias: None,
    return_payload_contains_param: false,
    iter_consumes: false,
    // Conservative: builtin seed contracts never carry a read-only claim. The
    // user-call carve-out gate (`compute_user_call_arg_lineages`) returns early
    // for `is_builtin` callees, so this field is never consulted on a builtin;
    // per-user-fn `borrowed_read_only` is computed from the body scan in
    // `extract_contract`, which reads Apply-arg POSITIONS directly, not this seed.
    borrowed_read_only: false,
    borrowed_cow_consumed: false,
    borrowed_cow_mutated: false,
    iter_consumes_projected_field: None,
};

/// Borrowed parameter PROVEN read-only: the runtime function reads the value's
/// bytes, never observes or changes ownership state, never COW-mutates, and never retains
/// (`ori_print`). The `borrowed_read_only: true` claim is consulted by
/// `callee_may_cow_arg`; seed it ONLY for runtime functions whose `ori_rt`
/// implementation provably performs no RC operation on the param.
const PARAM_BORROWED_READ_ONLY: ParamContract = ParamContract {
    borrowed_read_only: true,
    ..PARAM_BORROWED
};

/// Owned parameter consumed exactly once (linear).
const PARAM_OWNED_LINEAR: ParamContract = ParamContract {
    access: AccessClass::Owned,
    consumption: Consumption::Linear,
    cardinality: Cardinality::Once,
    may_escape: false,
    may_share: false,
    locality_bound: Locality::Unknown,
    uniqueness: Uniqueness::MaybeShared,
    transfers_through_return: false,
    return_alias: None,
    return_payload_contains_param: false,
    iter_consumes: false,
    // Owned param is consumed, never read-only.
    borrowed_read_only: false,
    borrowed_cow_consumed: false,
    borrowed_cow_mutated: false,
    iter_consumes_projected_field: None,
};

/// Return contract for methods producing unique results (COW operations).
const RETURN_UNIQUE: ReturnContract = ReturnContract {
    uniqueness: Uniqueness::Unique,
    preserves_freshness: true,
    locality: Locality::Unknown,
    shape: super::lattice::ShapeClass::NonReusable,
    // Builtin COW-method results are fresh, but the fresh-self-alloc admission
    // is scoped to USER for-yield finalizers (`@ori_list_take`) extracted from
    // the body; builtins keep `false` to preserve the status-quo store-dup path.
    returns_fresh_self_alloc: false,
    returns_sharing_view: false,
};
