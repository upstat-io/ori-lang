//! Contracts for compiler-internal runtime calls.

use std::sync::LazyLock;

use super::{
    sharing_return_contract, ContextBehavior, EffectSummary, FipContract, FxHashMap,
    MemoryContract, Name, ParamContract, ReturnAliasShape, ReturnContract, StringInterner,
    PARAM_BORROWED, PARAM_BORROWED_READ_ONLY, PARAM_OWNED_LINEAR, RETURN_UNIQUE,
};

// Env: ORI_DISABLE_PANIC_MSG_TRANSFER - restores the borrowed panic-message contract,
// debug-only. Spec: Annex E §AIMS RL-2.
static PANIC_MSG_TRANSFER_DISABLED: LazyLock<bool> = LazyLock::new(|| {
    report_panic_msg_transfer_toggle(
        std::env::var("ORI_DISABLE_PANIC_MSG_TRANSFER").as_deref() == Ok("1"),
    )
});

fn report_panic_msg_transfer_toggle(disabled: bool) -> bool {
    if disabled {
        tracing::info!(
            toggle = "ORI_DISABLE_PANIC_MSG_TRANSFER",
            effect = "seed ori_panic with the conservative borrowed-parameter contract",
            "ablation toggle fired"
        );
    }
    disabled
}

fn panic_msg_transfer_disabled() -> bool {
    *PANIC_MSG_TRANSFER_DISABLED
}

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

    // `ori_list_take` moves a compiler-owned yield buffer out of its untyped
    // scratch handle. The result is born unique at this call site; downstream
    // demand remains responsible for widening locality or uniqueness. Leaving
    // this internal finalizer without a contract applies TF-5 CONSERVATIVE and
    // loses the fresh lineage before a loop can carry it through COW updates.
    let ori_list_take = interner.intern("ori_list_take");
    sigs.entry(ori_list_take).or_insert_with(|| MemoryContract {
        params: vec![PARAM_BORROWED],
        return_info: ReturnContract {
            returns_fresh_self_alloc: true,
            ..RETURN_UNIQUE
        },
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    });

    // RL-2 transfers panic-message ownership to the runtime before unwind;
    // conservative effects preserve `may_throw`, while RL-1 duplicates a
    // still-live caller reference.
    if !panic_msg_transfer_disabled() {
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

    // RL-34 treats injected trace extension as an owned identity forwarder:
    // the argument transfers to the result, while storage replacement permits
    // both allocation and deallocation.
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

    // `ori_print` only reads string bytes; marking it RL-1 read-only prevents
    // conservative COW classification from blocking fresh-inc elision.
    let ori_print = interner.intern("ori_print");
    sigs.entry(ori_print).or_insert_with(|| MemoryContract {
        params: vec![PARAM_BORROWED_READ_ONLY],
        return_info: ReturnContract::CONSERVATIVE,
        effects: EffectSummary::default(),
        context_behavior: ContextBehavior::default(),
        fip: FipContract::Never,
        is_fbip: false,
    });

    // Seamless slices share their parent's buffer and use a negative cap;
    // MaybeShared routes release through slice-aware `ori_buffer_rc_dec`.
    let ori_list_slice_drop = interner.intern("ori_list_slice_drop");
    sigs.entry(ori_list_slice_drop)
        .or_insert_with(sharing_return_contract);
    // ori_list_slice_take is the make_slice_cap twin of ori_list_slice_drop
    // (both delegate to ori_list_slice); same sharing-view contract.
    let ori_list_slice_take = interner.intern("ori_list_slice_take");
    sigs.entry(ori_list_slice_take)
        .or_insert_with(sharing_return_contract);

    // Iterator adapters consume their first raw handle via `Box::from_raw`;
    // recording that transfer prevents a second scope-exit drop. Remaining
    // function-pointer, size, and scratch-buffer arguments are borrowed.
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

#[cfg(test)]
mod toggle_tests {
    crate::test_helpers::ablation_env_event_test!(
        panic_message_transfer_reproduces_conservative_contract_behavior,
        "ORI_DISABLE_PANIC_MSG_TRANSFER",
        "seed ori_panic with the conservative borrowed-parameter contract",
        || {
            let interner = super::StringInterner::new();
            let ori_panic = interner.intern("ori_panic");
            let mut contracts = super::FxHashMap::default();

            super::seed_internal_runtime_contracts(&mut contracts, &interner);

            assert!(
                !contracts.contains_key(&ori_panic),
                "the ablation must leave ori_panic on the conservative borrowed default"
            );
            super::panic_msg_transfer_disabled()
        },
    );
}
