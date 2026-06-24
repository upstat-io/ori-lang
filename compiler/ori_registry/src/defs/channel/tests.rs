//! Tests for the Channel `TypeDef` AND the `Channel<T>` burden template
//! registered in `BURDEN_TABLE`.
//!
//! The burden-template tests verify the registry-layer template only —
//! composition (substituting `TYPE_PARAM_T` with concrete `Idx` values) and
//! signature-dedup live in `ori_types::registry::burden_compose` /
//! `ori_types::registry::burden_dedup`, outside this crate's zero-deps
//! purity boundary.

use super::*;

use crate::burden::{
    table::{BurdenRegistry, TYPE_ID_CHANNEL, TYPE_PARAM_T},
    BuiltinBurdenSpec,
};

#[test]
fn channel_method_count() {
    assert_eq!(CHANNEL.methods.len(), 9);
}

#[test]
fn channel_is_arc() {
    assert_eq!(CHANNEL.memory, MemoryStrategy::Arc);
}

#[test]
fn channel_is_generic_with_one_param() {
    assert_eq!(CHANNEL.type_params, TypeParamArity::Fixed(1));
}

#[test]
fn channel_no_operators() {
    assert_eq!(CHANNEL.operators, OpDefs::UNSUPPORTED);
}

#[test]
fn channel_all_methods_impure() {
    for m in CHANNEL.methods {
        assert!(
            !m.pure,
            "Channel.{} should be impure (shared mutable state)",
            m.name
        );
    }
}

#[test]
fn channel_send_takes_element_owned() {
    let m = CHANNEL
        .methods
        .iter()
        .find(|m| m.name == "send")
        .unwrap_or_else(|| panic!("send method should exist"));
    assert_eq!(m.params.len(), 1);
    assert_eq!(m.params[0].ty, ReturnTag::ElementType);
    assert_eq!(m.params[0].ownership, Ownership::Owned);
}

#[test]
fn channel_recv_returns_option_of_element() {
    for name in ["recv", "receive", "try_recv", "try_receive"] {
        let m = CHANNEL
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("recv method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::OptionOf(TypeProjection::Element),
            "Channel.{name} should return OptionOf(Element)"
        );
    }
}

#[test]
fn channel_close_and_send_return_unit() {
    for name in ["close", "send"] {
        let m = CHANNEL
            .methods
            .iter()
            .find(|m| m.name == name)
            .unwrap_or_else(|| panic!("method should exist"));
        assert_eq!(
            m.returns,
            ReturnTag::Unit,
            "Channel.{name} should return Unit"
        );
    }
}

#[test]
fn channel_trait_methods() {
    let expected_traits: &[(&str, Option<&str>)] =
        &[("is_empty", Some("IsEmpty")), ("len", Some("Len"))];
    for m in CHANNEL.methods {
        let expected = expected_traits
            .iter()
            .find(|(name, _)| *name == m.name)
            .and_then(|(_, t)| *t);
        assert_eq!(
            m.trait_name, expected,
            "Channel.{} trait_name should be {:?}",
            m.name, expected
        );
    }
}

#[test]
fn channel_methods_alphabetically_sorted() {
    let names: Vec<&str> = CHANNEL.methods.iter().map(|m| m.name).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "Channel methods must be alphabetically sorted"
    );
}

// Channel<T> burden template (registry-layer surface)

fn channel_template() -> &'static BuiltinBurdenSpec {
    match BurdenRegistry::lookup_builtin(TYPE_ID_CHANNEL) {
        Some(spec) => spec,
        None => {
            panic!("Channel template missing from BURDEN_TABLE — registration regression")
        }
    }
}

#[test]
fn channel_template_present_in_burden_table() {
    // Positive: BURDEN_TABLE registers a Channel<T> in-table template like
    // LIST / MAP / SET / OPTION / RESULT (not DeferredToComposition).
    let _ = channel_template();
}

#[test]
fn channel_template_self_heap_alloc() {
    // The channel handle is `MemoryStrategy::Arc` (heap-managed shared
    // primitive); the template MUST advertise `self_heap_alloc = true` so
    // drop-glue knows the handle itself requires deallocation.
    let template = channel_template();
    assert!(
        template.self_heap_alloc,
        "Channel<T>: self_heap_alloc must be true (Arc-managed handle)"
    );
}

#[test]
fn channel_template_carries_type_param_placeholder() {
    // Channel<T>'s element type is the parameterized T. The template MUST
    // expose `element_burden = Some(TYPE_PARAM_T)` so composition layer can
    // substitute the concrete `Idx` at first-instantiation of Channel<int>
    // / Channel<str> / Channel<{K:V}>. Matches the LIST / MAP / SET shape.
    let template = channel_template();
    assert_eq!(
        template.element_burden,
        Some(TYPE_PARAM_T),
        "Channel<T>: element_burden must carry TYPE_PARAM_T for composition"
    );
}

#[test]
fn channel_template_has_no_inline_owned_or_variant_fields() {
    // Channel<T> is structurally a collection-like heap handle, not a
    // sum-of-variants or a struct with inline fields. The template body MUST
    // be empty across owned_fields / borrowed_fields / variant_burdens
    // (mirroring LIST/MAP/SET); T is reachable EXCLUSIVELY via the
    // `element_burden` slot to keep the composition signature distinct from
    // sum-type templates.
    let template = channel_template();
    assert!(
        template.owned_fields.is_empty(),
        "Channel<T>: owned_fields template must be empty"
    );
    assert!(
        template.borrowed_fields.is_empty(),
        "Channel<T>: borrowed_fields template must be empty"
    );
    assert!(
        template.variant_burdens.is_empty(),
        "Channel<T>: variant_burdens template must be empty (not a sum type)"
    );
}

#[test]
fn channel_template_has_no_user_or_compiled_drop() {
    // The Channel<T> template ships no `user_drop` (no `#free` annotation)
    // and no `compiled_drop` — drop is emitted via the §03 Phase 5 trivial
    // emission walk from the composed `UserBurdenSpec`.
    let template = channel_template();
    assert!(
        template.compiled_drop.is_none(),
        "Channel<T>: compiled_drop must be None"
    );
    assert!(
        template.user_drop.is_none(),
        "Channel<T>: user_drop must be None"
    );
}

#[test]
fn channel_template_shape_matches_list_set_pattern() {
    // Semantic pin: Channel<T> MUST follow the LIST/SET shape (heap-alloc +
    // single element placeholder, no variants, no inline fields). Differing
    // from this shape would force composition to produce structurally
    // distinct UserBurdenSpec instances that no longer share the registry
    // dedup path with other heap-buffered collections. Regression catcher
    // for accidental drift toward sum-template or struct-template shape.
    let channel = channel_template();
    let Some(list) = BurdenRegistry::lookup_builtin(crate::burden::table::TYPE_ID_LIST) else {
        panic!("LIST template missing — §01 regression");
    };
    assert_eq!(
        channel.self_heap_alloc, list.self_heap_alloc,
        "Channel<T> + List<T> must agree on self_heap_alloc"
    );
    assert_eq!(
        channel.element_burden, list.element_burden,
        "Channel<T> + List<T> must agree on element_burden placeholder shape"
    );
    assert_eq!(
        channel.owned_fields.is_empty(),
        list.owned_fields.is_empty()
    );
    assert_eq!(
        channel.variant_burdens.is_empty(),
        list.variant_burdens.is_empty()
    );
}
