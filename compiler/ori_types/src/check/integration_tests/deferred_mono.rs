use super::support::*;

// Deferred Monomorphization — Union-Find Root Extension
//
// Rank-weighted union-find can make a fresh
// instantiation var the root of a scheme var's equivalence class. Without
// `extend_var_subst_with_roots` the deferred-resolve path leaves the root
// var's `Tag::Var` leaf unsubstituted in the callee body, leaking through
// to ARC IR where the PC-2 seam fires. These tests verify that a
// multi-hop generic forwarding chain produces fully-concrete MonoInstances
// for every intermediate callee — signalling the root-extension fired on
// the deferred path.

/// Return `true` when every concrete parameter and return type is free of
/// unresolved variables after deferred resolution.
fn instance_signatures_fully_concrete(result: &CheckResult, fn_name: &str) -> bool {
    let instances = result.mono_instances_for(fn_name);
    if instances.is_empty() {
        return false;
    }
    for inst in instances {
        if result.pool.flags(inst.concrete_return_type).has_vars() {
            return false;
        }
        for &p in &inst.concrete_param_types {
            if result.pool.flags(p).has_vars() {
                return false;
            }
        }
    }
    true
}

/// Return `true` iff the `MonoInstance`s for `fn_name` all contain at least
/// one `body_type_map` entry whose value is `expected_concrete`. This is
/// the positive pin: without the root-extension, a `Tag::Var` ROOT leaf
/// in the callee's body would fall through `substitute_var` unchanged and
/// never produce a mapping to `expected_concrete` in the entry list.
fn instance_body_type_map_covers_concrete(
    result: &CheckResult,
    fn_name: &str,
    expected_concrete: Idx,
) -> bool {
    let instances = result.mono_instances_for(fn_name);
    if instances.is_empty() {
        return false;
    }
    instances.iter().all(|inst| {
        inst.body_type_map
            .iter()
            .any(|(_, v)| *v == expected_concrete)
    })
}

#[test]
fn deferred_mono_resolution_root_extension_applied_3_hop() {
    // 3-hop forwarder chain: @main → @double_wrap<int> → @wrap<int> → @id<int>.
    // The two middle hops are deferred monomorphization calls (generic
    // calling generic). With the root-extension fix, every MonoInstance
    // produced for the chain is fully concrete and every body_type_map
    // entry substitutes to a non-Var concrete type.
    let source = include_str!(
        "../fixtures/integration/deferred_mono_resolution_root_extension_applied_3_hop.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "3-hop forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    // Each of the three generic functions should have at least one
    // MonoInstance recorded for T = int (direct for @double_wrap from
    // @main; deferred for @wrap and @id).
    for fn_name in ["id", "wrap", "double_wrap"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded for T = int"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete \
             (no Tag::Var); a leaked Var signals the root-extension did NOT \
             fire on the deferred path. Instances: {instances:?}"
        );

        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an entry mapping to Idx::INT \
             — the root-extension's job is to route the callee body's Tag::Var \
             root-leaves through var_subst so they materialize in body_type_map"
        );
    }

    // Positive pin on the int instance: param + return are Idx::INT.
    let wrap_int_instances: Vec<_> = result
        .mono_instances_for("wrap")
        .into_iter()
        .filter(|m| m.concrete_param_types == vec![Idx::INT])
        .collect();
    assert_eq!(
        wrap_int_instances.len(),
        1,
        "wrap<int> should have exactly one MonoInstance, got {} — {wrap_int_instances:?}",
        wrap_int_instances.len()
    );
    assert_eq!(wrap_int_instances[0].concrete_return_type, Idx::INT);
}

#[test]
fn deferred_mono_resolution_root_extension_applied_4_hop() {
    // 4-hop chain: @main → @a → @b → @c → @d. The three middle callees
    // (@b, @c, @d) are deferred. Verifies the root-extension holds beyond
    // 3-hop — guards against off-by-one in transitive resolution.
    let source = include_str!(
        "../fixtures/integration/deferred_mono_resolution_root_extension_applied_4_hop.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "4-hop forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    for fn_name in ["a", "b", "c", "d"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded for T = int"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete"
        );

        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an Idx::INT target"
        );
    }

    // Positive pin: int propagates all the way down through the chain.
    for fn_name in ["a", "b", "c", "d"] {
        let int_instances: Vec<_> = result
            .mono_instances_for(fn_name)
            .into_iter()
            .filter(|m| m.concrete_param_types == vec![Idx::INT])
            .collect();
        assert_eq!(
            int_instances.len(),
            1,
            "{fn_name}<int> should have exactly one MonoInstance"
        );
        assert_eq!(int_instances[0].concrete_return_type, Idx::INT);
    }
}

#[test]
fn deferred_mono_resolution_multi_param_forwarding() {
    // Multi-param forwarder with REORDERED arguments:
    //   @f<A, B> (x: A, y: B) -> B = g(x: y, y: x)
    // At @g's instantiation A_g ← B_f and B_g ← A_f — the union-find
    // root walk must handle each scheme var independently. With the
    // root-extension, @g's MonoInstance comes out fully concrete at
    // every call site; without it, the reordered binding can leave
    // Tag::Var leaves depending on which scheme var roots the class.
    let source =
        include_str!("../fixtures/integration/deferred_mono_resolution_multi_param_forwarding.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "multi-param forwarder chain should type-check: {:?}",
        result.error_kinds()
    );

    for fn_name in ["f", "g"] {
        let instances = result.mono_instances_for(fn_name);
        assert!(
            !instances.is_empty(),
            "{fn_name} should have a MonoInstance recorded"
        );

        assert!(
            instance_signatures_fully_concrete(&result, fn_name),
            "{fn_name} MonoInstance param/return types must be fully concrete"
        );
    }

    // Coverage pin: the body_type_map must route to both Idx::INT and
    // Idx::STR — the concrete types threaded through A and B.
    for fn_name in ["f", "g"] {
        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::INT),
            "{fn_name}.body_type_map must contain an Idx::INT target"
        );
        assert!(
            instance_body_type_map_covers_concrete(&result, fn_name, Idx::STR),
            "{fn_name}.body_type_map must contain an Idx::STR target"
        );
    }

    // Positive pin: @f<int, str> has param types (int, str) and return str.
    let f_instances = result.mono_instances_for("f");
    let f_int_str: Vec<_> = f_instances
        .iter()
        .filter(|m| m.concrete_param_types == vec![Idx::INT, Idx::STR])
        .collect();
    assert_eq!(
        f_int_str.len(),
        1,
        "f<int, str> should have exactly one MonoInstance, got {}",
        f_int_str.len()
    );
    assert_eq!(f_int_str[0].concrete_return_type, Idx::STR);

    // Positive pin: @g is called inside @f with reordered args — the
    // resulting @g instance has (B_f, A_f) = (str, int) as its concrete
    // param types and A_f = int as its return type.
    let g_instances = result.mono_instances_for("g");
    let g_str_int: Vec<_> = g_instances
        .iter()
        .filter(|m| m.concrete_param_types == vec![Idx::STR, Idx::INT])
        .collect();
    assert_eq!(
        g_str_int.len(),
        1,
        "g<str, int> (reordered from f) should have exactly one MonoInstance, got {}",
        g_str_int.len()
    );
    assert_eq!(g_str_int[0].concrete_return_type, Idx::INT);
}

#[test]
fn deferred_mono_same_named_impl_callers_are_producer_qualified() {
    let source = include_str!(
        "../fixtures/integration/deferred_mono_same_named_impl_callers_are_producer_qualified.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "same-named generic impl methods should type-check: {:?}",
        result.error_kinds()
    );

    let forwards = result.mono_instances_for("forward");
    assert_eq!(
        forwards.len(),
        2,
        "expected two method instances: {forwards:?}"
    );
    assert!(forwards.iter().all(|instance| {
        matches!(
            instance.method_producer,
            Some(crate::MethodProducer::Impl(_))
        )
    }));
    assert_ne!(
        forwards[0].method_producer, forwards[1].method_producer,
        "same-spelled methods must retain distinct source producers"
    );

    let left = result.mono_instances_for("left_id");
    assert_eq!(left.len(), 1, "expected only left_id<int>: {left:?}");
    assert_eq!(
        left[0].generic_args,
        vec![crate::GenericArg::Type(Idx::INT)]
    );
    assert_eq!(left[0].concrete_param_types, vec![Idx::INT]);
    assert_eq!(left[0].concrete_return_type, Idx::INT);

    let right = result.mono_instances_for("right_id");
    assert_eq!(right.len(), 1, "expected only right_id<str>: {right:?}");
    assert_eq!(
        right[0].generic_args,
        vec![crate::GenericArg::Type(Idx::STR)]
    );
    assert_eq!(right[0].concrete_param_types, vec![Idx::STR]);
    assert_eq!(right[0].concrete_return_type, Idx::STR);
}
