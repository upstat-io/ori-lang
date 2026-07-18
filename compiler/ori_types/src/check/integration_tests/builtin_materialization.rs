use super::support::*;

// Pin 4 — builtin Duration ctor (`Duration.from_seconds`). The factory family
// is a non-generic builtin (`MethodKind::Associated` in `ori_registry`, not in
// `impl_sigs`), so a recorded MonoInstance would be skipped by
// `collect_mono_functions` — the family is delivered codegen-direct via
// `try_emit_builtin_associated`, not by the mono recorder. This pin holds the
// typeck-layer contract: the call type-checks and its return resolves to
// `Duration`. AOT execution also validates the runtime value.
#[test]
fn builtin_duration_constructor_typechecks_to_duration() {
    let source = include_str!(
        "../fixtures/integration/s09_2_builtin_duration_ctor_typechecks_to_duration.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "Duration ctor program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        body_ty,
        Idx::DURATION,
        "Duration.from_seconds(s: 5) must resolve to Duration; got {body_ty:?}"
    );
}

// Pin 5 — builtin iterator/DEI method chain (`.iter().rev().collect()`).
// Builtin iterator methods take the `ReceiverDispatch::Return` arm and are
// delivered codegen-direct (the `"Iterator"` / DEI re-keyed dispatch in
// `ori_llvm` `codegen/arc_emitter/builtins/mod.rs` reaches the existing
// `emit_iter_*` emitters), NOT by the mono recorder — so an instance-recorded
// assertion is the wrong contract (the factory family proved this). This pin
// holds the durable typeck-layer contract: the chain type-checks and its
// `.collect()` result resolves to `[int]`. AOT execution also validates the
// runtime value.
#[test]
fn reversed_iterator_collect_typechecks_to_int_list() {
    let source =
        include_str!("../fixtures/integration/s09_2_iterator_method_typechecks_to_list.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "iterator .rev().collect() program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        result.tag(body_ty),
        Tag::List,
        "[1, 2, 3].iter().rev().collect() must resolve to a list; got {body_ty:?}"
    );
    assert_eq!(
        result.pool.list_elem(body_ty),
        Idx::INT,
        "the collected list element must be int; got {body_ty:?}"
    );
}

// Pin 6 — deferred-route NEGATIVE clamp. A generic-calling-generic
// (`wrap6<U>` body calls `id6(x: y)` while `y: U` is still a variable) MUST
// route to `record_deferred_mono_call`, never record a bogus EAGER instance
// whose concrete types still carry a `Tag::Var`.
#[test]
fn deferred_generic_call_records_no_variable_typed_instance() {
    let source = include_str!(
        "../fixtures/integration/s09_2_deferred_route_records_no_var_typed_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-calling-generic program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    // No recorded instance may carry an unresolved Var in its concrete types —
    // the deferred path must NOT leak a half-resolved eager instance.
    for inst in result.mono_instances_all() {
        for &pt in &inst.concrete_param_types {
            assert_ne!(
                result.tag(pt),
                Tag::Var,
                "deferred route leaked a Var-typed concrete param into instance {inst:?}"
            );
        }
        assert_ne!(
            result.tag(inst.concrete_return_type),
            Tag::Var,
            "deferred route leaked a Var-typed concrete return into instance {inst:?}"
        );
    }
}

// Generic STRUCT body materialization. `Applied(P3Pair,[int,str])` must resolve
// through `Pool.resolutions` to a concrete `Tag::Struct` whose field types are the
// concrete args (`int`/`str`), NOT the generic param refs. Without materialization
// the `Applied` carries no resolution (codegen reads the generic field).
#[test]
fn generic_struct_applied_resolves_to_concrete_body() {
    let source = include_str!(
        "../fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-struct program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let applied = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("Applied(P3Pair, [int, str]) must exist in the pool");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied(P3Pair, [int, str]) must resolve to a concrete body");
    assert_eq!(
        result.tag(concrete),
        Tag::Struct,
        "the materialized resolution must be a concrete Struct; got {concrete:?}"
    );
    let fields = result.pool.struct_fields(concrete);
    let field_tys: Vec<Idx> = fields.iter().map(|&(_, ty)| ty).collect();
    assert_eq!(
        field_tys,
        vec![Idx::INT, Idx::STR],
        "materialized struct fields must be concrete int/str, not the generic params; got {field_tys:?}"
    );
}

// Struct-field NEGATIVE behavioral pin. The materialized struct field type must
// NOT be a `Tag::Named` generic-param ref — without materialization the field
// stays the declared `Tag::Named(A)`/`Named(B)`; materialized it is concrete.
#[test]
fn materialized_struct_field_is_not_generic_param() {
    let source = include_str!(
        "../fixtures/integration/s09_2_derived_method_on_generic_composite_typechecks_to_bool.ori"
    );
    let result = check_source(source);
    let applied = result
        .find_applied("P3Pair", &[Idx::INT, Idx::STR])
        .expect("Applied(P3Pair, [int, str]) must exist");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied must resolve to a concrete body post-fix");
    for (fname, fty) in result.pool.struct_fields(concrete) {
        assert_ne!(
            result.tag(fty),
            Tag::Named,
            "field {fname:?} kept a generic-param Named ref after materialization: {fty:?}"
        );
    }
}

// Enum variant-payload NEGATIVE behavioral pin. `Either<int,str>` `R`-variant
// payload Idx must be the concrete `str`, NOT the declared generic param
// `Tag::Named(B)`. Pins behavior (the payload Idx value), not symbol existence.
#[test]
fn generic_enum_r_payload_resolves_to_concrete() {
    let source =
        include_str!("../fixtures/integration/generic_enum_r_payload_resolves_to_concrete.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "generic-enum program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let applied = result
        .find_applied("Either", &[Idx::INT, Idx::STR])
        .expect("Applied(Either, [int, str]) must exist in the pool");
    let concrete = result
        .pool
        .resolve(applied)
        .expect("Applied(Either, [int, str]) must resolve to a concrete enum body");
    assert_eq!(
        result.tag(concrete),
        Tag::Enum,
        "the materialized resolution must be a concrete Enum; got {concrete:?}"
    );
    let variants = result.pool.enum_variants(concrete);
    // R is the second variant; its single payload must be concrete `str`.
    let r_payload = variants
        .iter()
        .find(|(vname, _)| *vname == result.interner.intern("R"))
        .map(|(_, payloads)| payloads.clone())
        .expect("the R variant must be present");
    assert_eq!(
        r_payload,
        vec![Idx::STR],
        "R payload must materialize to concrete str, not the generic param; got {r_payload:?}"
    );
}

#[test]
fn deferred_generic_call_resolves_to_concrete_instance() {
    let source = include_str!(
        "../fixtures/integration/s09_2_deferred_resolve_produces_concrete_instance.ori"
    );
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "deferred-resolve program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let id_instances = result.mono_instances_for("p7_id");
    assert!(
        id_instances
            .iter()
            .any(|m| m.concrete_param_types == vec![Idx::INT]
                && m.concrete_return_type == Idx::INT),
        "deferred `p7_id` (called only from generic `p7_wrap`) must resolve to a \
         concrete p7_id<int> instance, got: {id_instances:?}"
    );
}

#[test]
fn double_ended_iterator_last_typechecks_to_int_option() {
    let source =
        include_str!("../fixtures/integration/s09_2_dei_consumer_typechecks_to_option.ori");
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "iterator .last() program must type-check; kinds: {:?}",
        result.error_kinds()
    );
    let body_ty = result.first_function_body_type().unwrap();
    assert_eq!(
        result.tag(body_ty),
        Tag::Option,
        "[1, 2, 3].iter().last() must resolve to an Option; got {body_ty:?}"
    );
    assert_eq!(
        result.pool.option_inner(body_ty),
        Idx::INT,
        "the Option element from .last() must be int; got {body_ty:?}"
    );
}
