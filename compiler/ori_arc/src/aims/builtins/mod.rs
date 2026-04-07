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

    // Internal runtime functions called by ARC IR lowering (not user-facing).
    // These are `ori_*` C functions that would otherwise default to all-borrowed
    // in `compute_arg_ownership`. Where the runtime copies element bytes into a
    // collection buffer (creating a new reference), the element arg must be Owned
    // so AIMS emits RcInc for fat pointer elements.
    seed_internal_runtime_contracts(sigs, interner);
}

/// Seed contracts for internal `ori_*` runtime functions used by ARC IR lowering.
///
/// `ori_list_push(list_ptr, elem, elem_size)` — used by for-yield lowering.
/// The element bytes are copied into the list buffer, creating a new reference
/// to any RC-managed data (e.g., str data pointers). Without an Owned contract
/// on the element arg, the AIMS pipeline treats it as borrowed and doesn't emit
/// `RcInc`, causing double-frees when both the source collection and the yield
/// result list try to drop the same element.
fn seed_internal_runtime_contracts(
    sigs: &mut FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) {
    // ori_list_push(list_ptr: Borrowed, elem: Owned, elem_size: Borrowed)
    let ori_list_push = interner.intern("ori_list_push");
    sigs.entry(ori_list_push).or_insert_with(|| MemoryContract {
        params: vec![PARAM_BORROWED, PARAM_OWNED_LINEAR, PARAM_BORROWED],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    });

    // TPR-07-008: Iterator adapter and consumer runtime functions.
    //
    // Every `ori_iter_*` adapter/consumer that takes `iter: *mut u8` as
    // its first parameter **consumes** that iterator via
    // `Box::from_raw(iter.cast::<IterState>())`. Before the iterator
    // triviality flip, this was invisible to the ARC pipeline because
    // iterators were Scalar and no drops were emitted. Now that
    // iterators are non-trivial, we must tell the borrow inference
    // that these calls are consumption events — otherwise the ARC
    // pipeline will insert a scope-exit `ori_iter_drop` for the same
    // handle that the adapter/consumer already freed (double-free).
    //
    // The remaining arguments (transform_fn, elem_size, predicates,
    // out pointers, etc.) are borrowed — they're raw function pointers,
    // element size constants, or scratch buffers that the caller owns.
    seed_iter_consuming_runtime(sigs, interner);
}

// Adapters and consumers that consume a single iterator (arg 0).
// Each takes `iter` first, then various borrowed arguments
// (function pointers, element sizes, out pointers).
const SINGLE_ITER_CONSUMERS: &[(&str, usize)] = &[
    // Adapters — `iter, ...other_borrowed`
    ("ori_iter_map", 4),       // iter, fn, env, in_size
    ("ori_iter_filter", 4),    // iter, fn, env, elem_size
    ("ori_iter_take", 2),      // iter, n
    ("ori_iter_skip", 2),      // iter, n
    ("ori_iter_enumerate", 1), // iter
    ("ori_iter_flatten", 2),   // iter, inner_elem_size
    ("ori_iter_cycle", 2),     // iter, elem_size
    ("ori_iter_rev", 2),       // iter, elem_size
    // Consumers — `iter, ...other_borrowed`
    ("ori_iter_collect", 3), // iter, elem_size, elem_inc_fn
    // collect_set excluded — already handled by ProtocolBuiltin::CollectSet.
    ("ori_iter_count", 2),    // iter, elem_size
    ("ori_iter_any", 4),      // iter, pred_fn, pred_env, elem_size
    ("ori_iter_all", 4),      // iter, pred_fn, pred_env, elem_size
    ("ori_iter_find", 5),     // iter, pred_fn, pred_env, elem_size, out_ptr
    ("ori_iter_for_each", 4), // iter, each_fn, each_env, elem_size
    ("ori_iter_fold", 5),     // iter, init_ptr, fold_fn, fold_env, elem_size
    ("ori_iter_last", 3),     // iter, elem_size, out_ptr
    ("ori_iter_join", 5),     // iter, sep_f0, sep_f1, sep_f2, out_ptr
    ("ori_iter_rfold", 5),    // iter, init_ptr, fold_fn, fold_env, elem_size
    ("ori_iter_rfind", 5),    // iter, pred_fn, pred_env, elem_size, out_ptr
];

// Adapters that consume *two* iterators: ori_iter_zip and
// ori_iter_chain. zip also takes a trailing elem_size (borrowed).
const DOUBLE_ITER_CONSUMERS: &[(&str, usize)] = &[
    ("ori_iter_zip", 3),   // left, right, left_elem_size
    ("ori_iter_chain", 2), // first, second
];

/// Seed `Owned` contracts for every `ori_iter_*` runtime function that
/// consumes its iterator argument(s). See the caller for context.
fn seed_iter_consuming_runtime(
    sigs: &mut FxHashMap<Name, MemoryContract>,
    interner: &StringInterner,
) {
    for &(name, arity) in SINGLE_ITER_CONSUMERS {
        let name_id = interner.intern(name);
        let mut params = Vec::with_capacity(arity);
        params.push(PARAM_OWNED_LINEAR);
        params.extend(std::iter::repeat_n(PARAM_BORROWED, arity - 1));
        sigs.entry(name_id).or_insert_with(|| MemoryContract {
            params,
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        });
    }

    for &(name, arity) in DOUBLE_ITER_CONSUMERS {
        let name_id = interner.intern(name);
        let mut params = Vec::with_capacity(arity);
        params.push(PARAM_OWNED_LINEAR); // left / first
        params.push(PARAM_OWNED_LINEAR); // right / second
        params.extend(std::iter::repeat_n(PARAM_BORROWED, arity - 2));
        sigs.entry(name_id).or_insert_with(|| MemoryContract {
            params,
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::default(),
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        });
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
