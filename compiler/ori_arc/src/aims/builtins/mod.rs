//! Builtin function contracts for AIMS analysis.
//!
//! Pre-seeds the interprocedural analysis signature map with
//! [`MemoryContract`] entries for builtin methods that are not analyzed
//! intraprocedurally (they have no ARC IR body).
//!
//! Ported from [`crate::borrow::builtins`] — the ownership sets there
//! encode the same semantic facts but in a different format.

#[cfg(test)]
mod tests;

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnContract,
};
use super::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};

/// Pre-seed the signature map with contracts for all builtin methods.
///
/// Builtin methods (e.g., `push`, `len`, `slice`) are not lowered to ARC IR,
/// so interprocedural analysis cannot analyze them. Instead, we provide
/// hardcoded contracts derived from the same semantic facts encoded in
/// [`BuiltinOwnershipSets`].
///
/// Contract categories:
/// - **Borrowing** methods: read-only access, no ownership transfer
/// - **COW receiver** methods: consume receiver, return unique result
/// - **COW receiver+arg** methods: consume receiver and second arg
/// - **COW receiver-only** methods: consume receiver, borrow other args
/// - **Sharing** methods: return values sharing receiver's backing storage
#[expect(
    clippy::implicit_hasher,
    reason = "FxHashMap is the project-wide hasher"
)]
pub fn seed_builtin_contracts(
    sigs: &mut FxHashMap<Name, MemoryContract>,
    builtins: &BuiltinOwnershipSets,
    interner: &StringInterner,
) {
    // COW methods seeded first — they have specific ownership requirements
    // and take precedence over borrowing when the same method name appears
    // in both sets (method names are not type-qualified in ARC IR).

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

    // Sharing methods: return MaybeShared (shares receiver's backing).
    let sharing = crate::borrow::sharing_builtin_names(interner);
    for name in sharing {
        sigs.entry(name).or_insert_with(sharing_return_contract);
    }

    // Borrowing builtins: receiver borrowed, return conservative.
    // Seeded after COW methods so consuming methods aren't overwritten.
    for &name in &builtins.borrowing {
        sigs.entry(name).or_insert_with(|| borrowing_contract(1));
    }

    // Protocol builtins: per-arg ownership from ProtocolBuiltin.
    for (&name, arg_ownership) in &builtins.protocol {
        sigs.entry(name)
            .or_insert_with(|| protocol_contract(arg_ownership));
    }
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
    // Receiver is consumed; other args are borrowed.
    // Hard-codes 2 params (receiver + one arg). If a future COW builtin
    // has 3+ params, extend this function or use a parameterized variant.
    MemoryContract {
        params: vec![PARAM_OWNED_LINEAR, PARAM_BORROWED],
        return_info: RETURN_UNIQUE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    }
}

/// Method returning a value that shares receiver's backing storage.
///
/// E.g., `slice`, `substring` — the returned value references the receiver's
/// heap data, so its uniqueness is `MaybeShared`. The receiver is **borrowed**:
/// the runtime Inc's the original buffer for the slice/view but doesn't
/// consume the receiver. The caller retains ownership and must Dec.
fn sharing_return_contract() -> MemoryContract {
    MemoryContract {
        params: vec![PARAM_BORROWED],
        return_info: ReturnContract {
            uniqueness: Uniqueness::MaybeShared,
            preserves_freshness: false,
            ..ReturnContract::CONSERVATIVE
        },
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
};

/// Return contract for methods producing unique results (COW operations).
const RETURN_UNIQUE: ReturnContract = ReturnContract {
    uniqueness: Uniqueness::Unique,
    preserves_freshness: true,
    locality: Locality::Unknown,
    shape: super::lattice::ShapeClass::NonReusable,
};
