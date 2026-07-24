//! AOT regression pins for generic-STRUCT monomorphization via struct literal.
//!
//! A generic struct instantiated by a struct literal (`Box { val: 7 }`) infers
//! `Applied(Box, [Var->int])`, where the type-param `Var` link-resolves to a
//! concrete type but the structural `HAS_VAR` flag never clears. The concrete
//! `Applied(Box,[int]) -> struct{val:int}` pool resolution must be registered
//! so codegen sees the heap-aggregate (`ptr`) representation; without it,
//! codegen falls back to a scalar repr and the ARC `RcDec [AggFields]` lowers a
//! scalar where `ori_rc_dec(ptr, ptr)` expects a pointer — `error[E5001]`
//! "Call parameter type does not match function signature: i64 N, ptr".
//!
//! The struct-literal subject is NOT a method-mono instance, so the early
//! per-instance registrations miss it; the full-pool `Applied`-resolution sweep
//! in `finish_with_pool` is the catch-all that covers it. These cells drive the REAL
//! `ori build` -> native binary production entry point under
//! `ORI_CHECK_LEAKS=1` (L8 AOT, L10 leak-freedom, L12 production entry point).
//!
//! Revert-detecting semantic pin: `scalar_fields` (the `Box<int>` repro) builds
//! clean + runs exit 0 + zero leaks; with the resolution reverted it fails
//! E5001 on the malformed `ori_rc_dec(i64, ptr)`.
//!
//! `permuted_layout` is the F1-critical pin: a `swap (p: Pair<A,B>) -> Pair<B,A>`
//! returning `Pair<str,int>` must keep the permuted `{str-fat, i64}` field
//! layout — the `resolve_fully` matching-args fallback would collapse it to the
//! generic `{i64,i64}` and fail LLVM IR verification.
//!
//! `forwarder_no_overfire` is the negative over-fire pin: a fully-generic
//! forwarder over a generic struct must not cache a wrong-layout resolution for
//! the still-generic body while the concrete call site resolves correctly.
//!
//! Generic-ENUM (F2) cells are intentionally out of scope here — the enum
//! resolution path is covered by the same `materialize_*` machinery but its
//! observable-defect repro is tracked separately.

use crate::util::assert_aot_success;

// Cell 1 — generic struct, scalar fields (the canonical struct-literal repro shape).
#[test]
fn generic_struct_scalar_fields_build_and_run_clean() {
    assert_aot_success(
        include_str!("fixtures/generic_struct_mono/scalar_fields.ori"),
        "generic_struct_scalar_fields",
    );
}

// Cell 2 — generic struct, heap (RC-bearing) fields; drop decs exactly once.
#[test]
fn generic_struct_heap_fields_drop_clean_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generic_struct_mono/heap_fields.ori"),
        "generic_struct_heap_fields",
    );
}

// Cell 3 — multi-param + PERMUTED return; the F1-critical layout-preservation pin.
#[test]
fn generic_struct_permuted_layout_preserves_arg_order() {
    assert_aot_success(
        include_str!("fixtures/generic_struct_mono/permuted_layout.ori"),
        "generic_struct_permuted_layout",
    );
}

// Cell 4 — nested generic struct field; deep link-follow resolves the inner Applied.
#[test]
fn generic_struct_nested_generic_resolves_inner() {
    assert_aot_success(
        include_str!("fixtures/generic_struct_mono/nested_generic.ori"),
        "generic_struct_nested_generic",
    );
}

// Negative over-fire pin — a fully-generic forwarder must not cache a wrong resolution.
#[test]
fn generic_struct_forwarder_no_overfire() {
    assert_aot_success(
        include_str!("fixtures/generic_struct_mono/forwarder_no_overfire.ori"),
        "generic_struct_forwarder_no_overfire",
    );
}
