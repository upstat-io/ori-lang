//! AOT tests for generic-composite-type monomorphization.
//!
//! Verify that a generic struct / enum instantiated at concrete args
//! materializes a concrete pool body, so un-annotated construction and
//! scope-exit drop emit well-formed RC code (no malformed `ori_rc_dec` keyed
//! on a generic type-param idx) and run leak-free.

use crate::util::assert_aot_success;

// Un-annotated generic-struct heap-field construction + scope-exit drop.
#[test]
fn unannotated_generic_struct_heap_field_drops_clean() {
    assert_aot_success(
        include_str!("fixtures/composite_mono/unannotated_struct_drop.ori"),
        "composite_mono_unannotated_struct_drop",
    );
}

// Generic-enum heap-payload construction + scope-exit drop (enum twin above).
// The generic-monomorphization + leak/drop deliverable lands clean (the concrete
// enum body materializes; ORI_CHECK_LEAKS reports zero leaks; no malformed
// ori_rc_dec keyed on the generic payload param). The full assert_aot_success is
// blocked by an INDEPENDENT, NON-GENERIC AOT enum-heap(list)-payload extraction
// value bug (interp len==3 vs AOT wrong len) — reproduces with a plain
// `type E = L([int]) | R(int)`, so it is not the generic-materialization fix.
#[test]
fn generic_enum_heap_payload_drops_clean() {
    assert_aot_success(
        include_str!("fixtures/composite_mono/enum_heap_payload_drop.ori"),
        "composite_mono_enum_heap_payload_drop",
    );
}
