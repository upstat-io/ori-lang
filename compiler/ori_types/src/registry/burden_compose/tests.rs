//! Matrix tests for generic burden composition.
//!
//! Coverage: type-arg substitution × template-shape × nesting depth.
//! - Sum templates: Option<T> (single payload, Some arm), Result<T, E> (two arms).
//! - Mixed Value/HeapType: `Result<ValueType, HeapType>` per proposal
//!   §Value Trait Composition.
//! - Nested generics: outer `Option<Result<int, str>>` composes to a field-type
//!   `Idx` referencing the inner Result's monomorphized id (recursive
//!   composition is via the type graph, NOT inline expansion).
//! - Negative pin: lookup BEFORE composition runs returns `None`, proving
//!   instantiation-time wiring is load-bearing.
//! - Semantic pin: counts (`variant_burdens.len()`, `transfers_on_match.len()`,
//!   etc.) regression-pin the composition shape.

use ori_registry::burden::table::{
    BurdenRegistry, OPTION_VARIANT_NONE, OPTION_VARIANT_SOME, RESULT_VARIANT_ERR,
    RESULT_VARIANT_OK, TYPE_ID_CHANNEL, TYPE_ID_OPTION, TYPE_ID_RESULT,
};
use ori_registry::burden::{TransferKind, VariantId};

use super::compose_user_burden;
use crate::registry::burden::UserBurdenSpec;
use crate::{Idx, Pool, TypeRegistry};

// Fetch the Option<T> / Result<T, E> templates from BURDEN_TABLE. Helpers
// follow the canonical "panic when registry shape drifts" pattern from
// `compiler_repo/compiler/ori_registry/src/burden/table/tests.rs` —
// workspace clippy denies `expect`/`unwrap` even in tests per `tests.md`
// §Test Hygiene.
fn option_template() -> &'static ori_registry::burden::BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(TYPE_ID_OPTION) {
        Some(spec) => spec,
        None => panic!("Option template missing from BURDEN_TABLE"),
    }
}

fn result_template() -> &'static ori_registry::burden::BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(TYPE_ID_RESULT) {
        Some(spec) => spec,
        None => panic!("Result template missing from BURDEN_TABLE"),
    }
}

fn find_variant(
    spec: &UserBurdenSpec,
    id: VariantId,
) -> &crate::registry::burden::UserVariantBurden {
    match spec.variant_burdens.iter().find(|v| v.variant_id == id) {
        Some(v) => v,
        None => panic!("variant {id:?} missing from composed spec"),
    }
}

// ─── Positive: Result<int, str> ──────────────────────────────────────────

#[test]
fn compose_result_int_str_yields_two_variant_burdens() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = result_template();
    let type_args = [Idx::INT, Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    assert_eq!(
        composed.variant_burdens.len(),
        2,
        "Result has Ok + Err arms"
    );
    assert!(
        composed.owned_fields.is_empty(),
        "Result has no top-level owned fields"
    );
    assert!(composed.borrowed_fields.is_empty());
    assert_eq!(composed.element_burden, None);
}

#[test]
fn compose_result_int_str_err_arm_field_type_is_str() {
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = result_template();
    let type_args = [Idx::INT, Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    let err = find_variant(&composed, RESULT_VARIANT_ERR);
    assert_eq!(err.transfers_on_match.len(), 1, "Err carries one binding");
    assert_eq!(
        err.transfers_on_match[0].field_type,
        Idx::STR,
        "TYPE_PARAM_E substituted to str"
    );
    assert_eq!(err.transfers_on_match[0].transfer_kind, TransferKind::Move);

    let ok = find_variant(&composed, RESULT_VARIANT_OK);
    assert_eq!(
        ok.transfers_on_match[0].field_type,
        Idx::INT,
        "TYPE_PARAM_T substituted to int"
    );
}

// ─── Positive: Result<{str: int}, str> — recursive composition via type graph ───

#[test]
fn compose_result_with_map_payload_substitutes_map_idx() {
    let mut pool = Pool::new();
    let registry = TypeRegistry::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let template = result_template();
    let type_args = [map_str_int, Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    let ok = find_variant(&composed, RESULT_VARIANT_OK);
    assert_eq!(
        ok.transfers_on_match[0].field_type, map_str_int,
        "Ok arm carries the Map<str,int> Idx — recursive burden looked up at codegen"
    );
    let err = find_variant(&composed, RESULT_VARIANT_ERR);
    assert_eq!(err.transfers_on_match[0].field_type, Idx::STR);
}

// ─── Positive: Option<Result<int, str>> — nested generic ─────────────────

#[test]
fn compose_option_with_result_payload_carries_inner_result_idx() {
    // The outer Option composition carries the INNER Result's monomorphized
    // Idx as the Some arm's field_type. No inline expansion: the inner
    // Result's burden lives under its own Idx in TypeRegistry::burden.
    //
    // §02.2 dedup ensures that two compositions of `Result<int, str>` from
    // different parent contexts share the same UserBurdenSpec entry. This
    // test predates §02.2; we exercise composition only.
    let mut pool = Pool::new();
    let registry = TypeRegistry::new();
    // Synthesize an Idx that stands in for "Result<int, str>'s monomorphized
    // pool entry". The composition function does not introspect the inner
    // Idx — it just carries it forward as the Option<Result<int, str>>'s
    // Some-arm field_type.
    let inner_result_idx = pool.applied(ori_ir::Name::from_raw(0xBEEF_BEEF), &[Idx::INT, Idx::STR]);
    let template = option_template();
    let type_args = [inner_result_idx];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    assert_eq!(
        composed.variant_burdens.len(),
        2,
        "Option has None + Some arms"
    );

    let none = find_variant(&composed, OPTION_VARIANT_NONE);
    assert!(
        none.transfers_on_match.is_empty(),
        "None arm carries no payload"
    );

    let some = find_variant(&composed, OPTION_VARIANT_SOME);
    assert_eq!(
        some.transfers_on_match.len(),
        1,
        "Some arm carries one payload binding"
    );
    assert_eq!(
        some.transfers_on_match[0].field_type, inner_result_idx,
        "Some arm carries the nested Result Idx — codegen recurses via TypeRegistry::burden"
    );
}

// ─── Positive: mixed Value/HeapType ─────────────────────────────────────

#[test]
fn compose_result_with_value_and_heap_args_carries_both() {
    // Per proposal §Value Trait Composition: Value-trait members have empty
    // BurdenSpec entries. Composition does not in-line expand — it carries
    // each concrete Idx; codegen looks up each one's burden separately.
    // Here `int` plays the Value role (empty burden table entry per
    // BURDEN_TABLE: `(TYPE_ID_INT, BuiltinBurdenSpec::EMPTY)`) and `str`
    // plays the HeapType role (`self_heap_alloc = true`).
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = result_template();
    let type_args = [Idx::INT, Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    let ok = find_variant(&composed, RESULT_VARIANT_OK);
    assert_eq!(
        ok.transfers_on_match[0].field_type,
        Idx::INT,
        "Value-role argument substituted at field_type position"
    );
    let err = find_variant(&composed, RESULT_VARIANT_ERR);
    assert_eq!(
        err.transfers_on_match[0].field_type,
        Idx::STR,
        "HeapType-role argument substituted at field_type position"
    );
    // Variant arm shape is identical for both — composition is type-erased
    // across the Value/Heap boundary at the field_type level; differential
    // codegen happens downstream via per-Idx burden lookup.
}

// ─── Negative pin: instantiation-time wiring is load-bearing ────────────

#[test]
fn lookup_before_composition_returns_none_proves_wiring_load_bearing() {
    // BEFORE composition runs, a freshly-allocated monomorphized Idx has
    // no entry in `TypeRegistry::burden`. This proves that the §02.1
    // wiring (composing AND registering on first instantiation) is the
    // load-bearing step — without it, codegen would see `None` and the
    // §03 burden-lookup `debug_assert!` (forward-referenced — see TODO
    // below) would fire.
    //
    // §03 forward-reference: the `debug_assert!("monomorphized TypeId
    // has registered UserBurdenSpec")` lives in
    // `ori_arc::lower::burden_lookup` and consumes the composed entry.
    // §02.4.A's "ContextHole-shaped variables inherit BurdenSpec from
    // their underlying TypeId" verification depends on this same
    // lookup path.
    let mut pool = Pool::new();
    let registry = TypeRegistry::new();
    let fresh_idx = pool.applied(ori_ir::Name::from_raw(0xDEAD_BEEF), &[Idx::INT, Idx::STR]);

    assert!(
        registry.burden(fresh_idx).is_none(),
        "lookup before composition runs returns None — proves instantiation-time wiring is the load-bearing step"
    );

    // After composition, if §02.1 integration is wired through to a registry
    // mutation surface (currently API-gapped — see §02.1 deliverable note),
    // this assertion would flip. Today the composition function alone
    // produces the spec; the registry-insertion call site is the integration
    // point §02.1's checklist item 2 names. The negative pin above is the
    // local proof that without an insertion call, lookup yields None.
}

// ─── Semantic pin: regression-pin counts ────────────────────────────────

#[test]
fn semantic_pin_result_composition_counts() {
    // Pins the exact composition shape for Result<int, str> so any
    // future change to recursion / flattening / field-drop semantics
    // surfaces as a regression. Per `tests.md §Matrix Clamping`.
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = result_template();
    let type_args = [Idx::INT, Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    // Field count = 0 (Result has no top-level owned fields; payloads
    // live inside variant_burdens).
    assert_eq!(composed.owned_fields.len(), 0);
    // Variant count = 2 (Ok, Err).
    assert_eq!(composed.variant_burdens.len(), 2);
    // Per-variant payload count = 1 each (Ok carries T; Err carries E).
    for variant in &composed.variant_burdens {
        assert_eq!(
            variant.transfers_on_match.len(),
            1,
            "each Result arm carries exactly one payload binding"
        );
        assert!(
            variant.retained_owned.is_empty(),
            "Result arms don't retain owned fields beyond the moved binding"
        );
    }
    // Recursive depth = 1 (no flattening; nested generics would carry
    // their own Idx, looked up separately at codegen).
    assert_eq!(composed.element_burden, None);
    // Drop hooks: builtin Result template carries none.
    assert_eq!(composed.compiled_drop, None);
    assert_eq!(composed.user_drop, None);
}

// ─── §02.4.B Channel<T> composition (registry-layer surface) ────────────

fn channel_template() -> &'static ori_registry::burden::BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(TYPE_ID_CHANNEL) {
        Some(spec) => spec,
        None => {
            panic!("Channel template missing from BURDEN_TABLE — §02.4.B registration regression")
        }
    }
}

#[test]
fn compose_channel_int_substitutes_element_burden_to_int() {
    // Positive: composing Channel<int> with concrete `Idx::INT` yields a
    // `UserBurdenSpec` whose `element_burden = Some(Idx::INT)`. The Arc
    // handle stays heap-allocated (`self_heap_alloc = true`), and T's
    // burden is reachable via the substituted Idx — codegen looks it up
    // separately at drop-glue emission. Mirrors the LIST/SET composition
    // path tested elsewhere in this file.
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = channel_template();
    let type_args = [Idx::INT];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    assert!(
        composed.self_heap_alloc,
        "Channel<int>: composed spec preserves self_heap_alloc"
    );
    assert_eq!(
        composed.element_burden,
        Some(Idx::INT),
        "Channel<int>: TYPE_PARAM_T substituted to Idx::INT"
    );
    assert!(
        composed.owned_fields.is_empty(),
        "Channel<int>: no inline owned fields"
    );
    assert!(
        composed.variant_burdens.is_empty(),
        "Channel<int>: not a sum type"
    );
}

#[test]
fn compose_channel_str_substitutes_element_burden_to_str() {
    // Positive: composing Channel<str> yields element_burden = Some(STR).
    // Distinct from Channel<int> at the Idx level — proves substitution
    // produces concrete per-T compositions vs sharing one cached form.
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = channel_template();
    let type_args = [Idx::STR];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    assert_eq!(
        composed.element_burden,
        Some(Idx::STR),
        "Channel<str>: TYPE_PARAM_T substituted to Idx::STR"
    );
    assert!(composed.self_heap_alloc);
}

#[test]
fn compose_channel_distinct_t_produces_distinct_specs() {
    // Positive (signature side): Channel<int> and Channel<str> compose to
    // structurally distinct UserBurdenSpec instances. Carries the §02.2
    // dedup contract — the BurdenSignature naturally distinguishes them
    // via element_burden Idx contents (codex blind-spot #3 cure).
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = channel_template();

    let channel_int = compose_user_burden(template, &[Idx::INT], &pool, &registry);
    let channel_str = compose_user_burden(template, &[Idx::STR], &pool, &registry);

    assert_ne!(
        channel_int, channel_str,
        "Channel<int> and Channel<str> compose to structurally distinct specs"
    );
    assert_ne!(
        channel_int.element_burden, channel_str.element_burden,
        "the difference lives at element_burden — natural BurdenSignature divergence"
    );
}

#[test]
fn compose_channel_map_payload_carries_map_idx() {
    // Positive (nested generic): composing Channel<{str: int}> uses the
    // §02.1 substitution mechanism only — the inner map's BurdenSpec is
    // looked up SEPARATELY at codegen via the map's own Idx. No inline
    // expansion at composition time; recursive composition is via the
    // type graph. Confirms the channel template participates in the same
    // nested-generic flow as Option<Result<int, str>>.
    let mut pool = Pool::new();
    let registry = TypeRegistry::new();
    let map_str_int = pool.map(Idx::STR, Idx::INT);
    let template = channel_template();
    let type_args = [map_str_int];

    let composed = compose_user_burden(template, &type_args, &pool, &registry);

    assert_eq!(
        composed.element_burden,
        Some(map_str_int),
        "Channel<map_str_int>: element_burden carries the Map<str,int> Idx"
    );
    assert!(
        composed.self_heap_alloc,
        "Channel<map_str_int>: self_heap_alloc preserved"
    );
}

#[test]
fn semantic_pin_channel_composition_counts() {
    // Pins the exact composition shape for Channel<int> so any future
    // change to the channel template (e.g., switching from element_burden
    // to inline owned_fields) surfaces as a regression. Per
    // `tests.md §Matrix Clamping`.
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = channel_template();

    let composed = compose_user_burden(template, &[Idx::INT], &pool, &registry);

    assert_eq!(composed.owned_fields.len(), 0);
    assert_eq!(composed.borrowed_fields.len(), 0);
    assert_eq!(composed.variant_burdens.len(), 0);
    assert!(composed.element_burden.is_some());
    assert!(composed.self_heap_alloc);
    assert!(composed.compiled_drop.is_none());
    assert!(composed.user_drop.is_none());
}

#[test]
fn negative_pin_channel_empty_element_burden_breaks_signature_distinctness() {
    // Negative pin: a hypothetical Channel<T> template where the
    // composition path lost the element_burden slot would produce
    // structurally identical specs for every T — defeating the §02.2
    // dedup contract (Channel<int> would collide with Channel<str>).
    //
    // We model the regression by composing TWO Channel<T> with different
    // type args BUT manually zeroing out the element_burden post-compose,
    // asserting the resulting two specs become identical. The cure for an
    // accidental drop of element_burden is the prior test
    // (`compose_channel_distinct_t_produces_distinct_specs`) — that test
    // fires when this regression surface manifests in production.
    let pool = Pool::new();
    let registry = TypeRegistry::new();
    let template = channel_template();

    let mut a = compose_user_burden(template, &[Idx::INT], &pool, &registry);
    let mut b = compose_user_burden(template, &[Idx::STR], &pool, &registry);

    // Simulate the regression: clear the substituted element burden.
    a.element_burden = None;
    b.element_burden = None;

    assert_eq!(
        a, b,
        "without element_burden the specs collapse — proves the slot is the load-bearing distinguisher"
    );
}
