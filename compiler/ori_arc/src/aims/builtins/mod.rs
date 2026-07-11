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

use std::sync::LazyLock;

use ori_ir::{Name, StringInterner};
use rustc_hash::FxHashMap;

use crate::borrow::BuiltinOwnershipSets;

use super::contract::{
    ContextBehavior, EffectSummary, FipContract, MemoryContract, ParamContract, ReturnAliasShape,
    ReturnContract,
};
use super::lattice::{AccessClass, Cardinality, Consumption, Locality, Uniqueness};

/// `ORI_DISABLE_PANIC_MSG_TRANSFER=1` skips the `ori_panic` Owned message
/// contract, restoring the all-borrowed default (the pre-transfer caller-side
/// no-release arrangement — the message buffer leaks on every caught-panic
/// path). Bisection surface: isolates a panic-path leak / double-free to the
/// panic-message ownership transfer vs the rest of the contract seeding.
/// Spec: Annex E §AIMS RL-2 (ownership-transferring terminal use).
static PANIC_MSG_TRANSFER_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_PANIC_MSG_TRANSFER").as_deref() == Ok("1"));

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
/// - **COW indexed update** (`updated`): consume receiver and value (arg 2),
///   borrow key (arg 1)
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

    // Sharing methods: return MaybeShared (shares receiver's backing) and
    // carry the typed sharing-view CREDIT (`returns_sharing_view`).
    // The bare surface names `take` / `drop` are NEVER seeded here: the
    // name-keyed contract map cannot discriminate the seamless-slice
    // methods from same-named callees with different ownership (a user
    // `Drop::drop`, iterator adapters), so their CREDIT rides the
    // unambiguous runtime names (`ori_list_slice_take` /
    // `ori_list_slice_drop`) seeded below.
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

    // ori_panic(msg: Owned) — RL-2 ownership TRANSFER. The panic machinery
    // copies the message into thread-local state before unwinding and the
    // runtime releases the original (slice/SSO/immortal-aware) — the caller
    // emits NO release on any panic path (no unwind-edge cleanup can reach
    // the message in frames between the panic site and the catch). Passing a
    // still-live message dup-incs per RL-1, keeping caller-side references
    // valid across a caught panic. Effects stay CONSERVATIVE so the seeded
    // contract narrows ONLY param ownership vs the no-contract default
    // (`may_throw` MUST stay true — ori_panic always unwinds).
    if !*PANIC_MSG_TRANSFER_DISABLED {
        let ori_panic = interner.intern("ori_panic");
        sigs.entry(ori_panic).or_insert_with(|| MemoryContract {
            params: vec![PARAM_OWNED_LINEAR],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary::CONSERVATIVE,
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        });
    }

    // __ori_inject_trace(err: Owned) — the compiler-injected `?`-hop trace call
    // (ori_llvm intercepts it at codegen; never a real callee). RL-34
    // forwarder-identity: the receiver Error is consumed and transfers through
    // the by-value return (returns the receiver type directly), so the caller
    // emits NO dec on the arg and exactly one release on the result. The
    // EffectSummary sets may_allocate + may_deallocate because the runtime
    // `_ori_inject_trace_entry` COW-pushes onto `error.trace` (may realloc a new
    // buffer + free the old); a default EffectSummary would under-approximate.
    // Spec: Annex E §AIMS RL-2 (ApplyToOwnedParam transfer) + RL-34.
    let ori_inject_trace = interner.intern("__ori_inject_trace");
    sigs.entry(ori_inject_trace)
        .or_insert_with(|| MemoryContract {
            params: vec![ParamContract {
                transfers_through_return: true,
                return_alias: Some(ReturnAliasShape::Direct),
                ..PARAM_OWNED_LINEAR
            }],
            return_info: ReturnContract::CONSERVATIVE,
            effects: EffectSummary {
                may_allocate: true,
                may_deallocate: true,
                ..EffectSummary::default()
            },
            context_behavior: ContextBehavior::default(),
            fip: FipContract::Never,
            is_fbip: false,
        });

    // ori_print(s: Borrowed READ-ONLY) — a pure stdout read of the string's
    // bytes (`ori_rt::io::ori_print`): never touches the RC header, never
    // COW-mutates, never retains. The `borrowed_read_only` claim keeps the
    // may-COW conservative over-approximation (`callee_may_cow_arg`) from
    // flagging a fresh str borrowed by `print` as COW-mutated — which would
    // block the Phase-7 surplus fresh-inc elision and leak one allocation per
    // printed heap template string. Spec: Annex E §AIMS RL-1.
    let ori_print = interner.intern("ori_print");
    sigs.entry(ori_print).or_insert_with(|| MemoryContract {
        params: vec![PARAM_BORROWED_READ_ONLY],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    });

    // ori_list_slice_drop returns a seamless slice (negative cap, interior data
    // pointer) sharing the parent buffer's RC. Without an explicit MaybeShared
    // contract, backward demand narrows uniqueness to Unique, drop_hints flag
    // it as is_unique_drop, and codegen emits ori_buffer_drop_unique which
    // panics on slice caps (`ori_buffer_drop_unique` in ori_rt/src/rc/list_rc.rs). The MaybeShared
    // contract routes drops through the slice-aware ori_buffer_rc_dec.
    let ori_list_slice_drop = interner.intern("ori_list_slice_drop");
    sigs.entry(ori_list_slice_drop)
        .or_insert_with(sharing_return_contract);
    // ori_list_slice_take is the make_slice_cap twin of ori_list_slice_drop
    // (both delegate to ori_list_slice); same sharing-view contract.
    let ori_list_slice_take = interner.intern("ori_list_slice_take");
    sigs.entry(ori_list_slice_take)
        .or_insert_with(sharing_return_contract);

    // Iterator adapter and consumer runtime functions.
    //
    // Every `ori_iter_*` adapter/consumer that takes `iter: *mut u8` as
    // its first parameter **consumes** that iterator via
    // `Box::from_raw(iter.cast::<IterState>())`. Before the iterator
    // triviality flip, this was invisible to the ARC pipeline because
    // iterators were Scalar and no drops were emitted. Now that
    // iterators are non-trivial, the borrow inference must mark
    // these calls as consumption events — otherwise the ARC
    // pipeline inserts a scope-exit `ori_iter_drop` for the same
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
    ("ori_iter_cycle", 4),     // iter, elem_size, elem_inc_fn, elem_dec_fn
    ("ori_iter_rev", 4),       // iter, elem_size, elem_inc_fn, elem_dec_fn
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
/// the proven realization path of the sibling COW methods (`set`, `push`).
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
            // Typed buffer-provenance CREDIT: the view result mints its own
            // +1 on the receiver's backing allocation.
            // Spec: Annex E §AIMS §12 (sharing-view producer = CREDIT).
            returns_sharing_view: true,
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
    capture_variant_return_project: None,
    iter_consumes_projected_field: None,
};

/// Borrowed parameter PROVEN read-only: the runtime function reads the value's
/// bytes and never touches its RC header, never COW-mutates, never retains
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
    capture_variant_return_project: None,
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
