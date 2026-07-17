//! Contracts for compiler-internal runtime calls.

use std::sync::LazyLock;

use super::{
    sharing_return_contract, ContextBehavior, EffectSummary, FipContract, FxHashMap,
    MemoryContract, Name, ParamContract, ReturnAliasShape, ReturnContract, StringInterner,
    PARAM_BORROWED, PARAM_BORROWED_READ_ONLY, PARAM_OWNED_LINEAR,
};

/// `ORI_DISABLE_PANIC_MSG_TRANSFER=1` skips the `ori_panic` Owned message
/// contract, restoring the all-borrowed default (the pre-transfer caller-side
/// no-release arrangement — the message buffer leaks on every caught-panic
/// path). Bisection surface: isolates a panic-path leak / double-free to the
/// panic-message ownership transfer vs the rest of the contract seeding.
/// Spec: Annex E §AIMS RL-2 (ownership-transferring terminal use).
static PANIC_MSG_TRANSFER_DISABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("ORI_DISABLE_PANIC_MSG_TRANSFER").as_deref() == Ok("1"));

/// Seed contracts for internal `ori_*` runtime functions used by ARC IR lowering.
///
/// `ori_list_push(list_ptr, elem, elem_size)` — used by for-yield lowering.
/// The element bytes are copied into the list buffer, creating a new reference
/// to ownership-bearing data (for example string storage). Without an Owned
/// contract on the element arg, AIMS records a borrow rather than the additional
/// logical owner, so both collections can later discharge a credit that the plan
/// never established. In the current RC projection this appears as a missing
/// retain and can cause a double-free.
pub(super) fn seed_internal_runtime_contracts(
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

    // __ori_inject_trace(err: Owned) — the compiler-injected `?`-hop trace
    // operation. It is a closed logical runtime-call identity; each physical
    // adapter must realize it without changing this contract. RL-34
    // forwarder-identity: the receiver Error is consumed and transfers through
    // the by-value return (returns the receiver type directly), so the caller
    // emits NO dec on the arg and exactly one release on the result. The
    // EffectSummary sets may_allocate + may_deallocate because trace extension
    // may replace owned trace storage; a default summary would under-approximate.
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
    // bytes (`ori_rt::io::ori_print`): never observes or changes ownership
    // state, never COW-mutates, never retains. The `borrowed_read_only` claim keeps the
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
    ("ori_iter_map", 5),       // iter, fn, env, in_size, output_dec_fn
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
        let Some(borrowed_arity) = arity.checked_sub(1) else {
            panic!("single-iterator runtime arity must include its iterator");
        };
        params.extend(std::iter::repeat_n(PARAM_BORROWED, borrowed_arity));
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
        let Some(borrowed_arity) = arity.checked_sub(2) else {
            panic!("double-iterator runtime arity must include both iterators");
        };
        params.extend(std::iter::repeat_n(PARAM_BORROWED, borrowed_arity));
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
