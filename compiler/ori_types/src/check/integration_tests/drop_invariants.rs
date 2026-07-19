use super::support::*;

// E2048 EDROP_PARTIAL_MOVE — match-destructure whole-value-consumption
// invariant. A PARTIAL by-value destructure of a Drop type (binding a proper
// subset of owned fields, leaving the residual live) is a double-free / leak
// hazard; the bind-all-fields whole-value case is the sound consumption form.
// Spec: drop-trait-proposal.md §Execution Timing.
//
// `check_source` does not load the prelude, so each source declares a local
// `trait Drop` to register it in the trait registry (the validator silently
// no-ops when `Drop` is unregistered — the pre-deployment shape).

impl CheckResult {
    /// Count `DropPartialMove` (E2048) errors.
    fn drop_partial_move_count(&self) -> usize {
        self.error_kinds()
            .iter()
            .filter(|k| matches!(k, TypeErrorKind::DropPartialMove { .. }))
            .count()
    }
}

fn assert_drop_partial_move_count(source: &'static str, expected: usize, context: &str) {
    let result = check_source(fixture_without_trailing_newline(source));
    assert_eq!(
        result.drop_partial_move_count(),
        expected,
        "{context}; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_partial_struct_destructure_rejected_e2048() {
    // Negative pin: `Pair { a, .. }` binds 1 of 2 owned fields by value —
    // proper subset → E2048.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_partial_struct_destructure_rejected_e2048.ori"
        ),
        1,
        "partial struct destructure of a Drop type MUST fire exactly one E2048",
    );
}

#[test]
fn drop_match_whole_value_struct_destructure_accepted() {
    // Positive pin: `Pair { a, b }` binds EVERY owned field — whole-value
    // consumption → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_whole_value_struct_destructure_accepted.ori"
        ),
        0,
        "whole-value struct destructure of a Drop type MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_partial_enum_variant_destructure_rejected_e2048() {
    // Negative pin: `Pair(x)` binds 1 of 2 payload fields → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/drop_match_partial_enum_variant_destructure_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "partial enum-variant destructure of a Drop type MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_whole_payload_enum_variant_destructure_accepted() {
    // Positive pin: `Pair(x, y)` binds every payload field of the matched
    // variant → whole-value consumption → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_whole_payload_enum_variant_destructure_accepted.ori"
        ),
        0,
        "whole-payload enum-variant destructure MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_partial_destructure_on_non_drop_type_accepted() {
    // Negative-space pin: the E2048 axis is Drop-only. A partial destructure of
    // a NON-Drop type must NOT fire E2048 (it is governed by E2043's
    // conditional-move axis, not E2048's unconditional-Drop axis).
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_partial_destructure_on_non_drop_type_accepted.ori"
        ),
        0,
        "partial destructure of a non-Drop type MUST NOT fire E2048",
    );
}

#[test]
fn drop_let_projection_rejected_e2048() {
    // Impl lookup checks both the nominal and resolved receiver keys.
    // `let $f = p.a` on a Drop type produces E2048.
    assert_drop_partial_move_count(
        include_str!("../fixtures/integration/drop_let_projection_rejected_e2048.ori"),
        1,
        "let-projection of a Drop-type field MUST fire exactly one E2048",
    );
}

#[test]
fn drop_match_nested_let_projection_in_arm_rejected_e2048() {
    // Negative pin: a `let $f = v.field` projection nested inside a match arm
    // body must be reached by the validator's FunctionSeq recursion.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_nested_let_projection_in_arm_rejected_e2048.ori"
        ),
        1,
        "nested let-projection inside a match arm MUST fire E2048",
    );
}

// E2048 nested-destructure recursion — a NESTED partial by-value destructure
// of a Drop-typed field is the same double-free / leak hazard as the top-level
// case, one level down. The OUTER pattern binds every field (whole-value
// consume at the top level), so the top-level field-count check alone does not
// flag it; the validator must recurse into nested struct/variant sub-patterns
// over Drop-typed fields. Matrix clamps the boundary from above (partial → fire)
// and below (bind-all nested → no fire).

#[test]
fn drop_match_nested_partial_struct_destructure_rejected_e2048() {
    // Negative pin: outer `Outer { inner: ... }` binds Outer's only field
    // (whole-value at the top level), but nested `Inner { x, .. }` binds 1 of 2
    // owned fields of the Drop-typed `inner` → partial move → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/drop_match_nested_partial_struct_destructure_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "nested partial struct destructure of a Drop-typed field MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_nested_whole_value_struct_destructure_accepted() {
    // Positive pin: nested `Inner { x, y }` binds EVERY owned field of the
    // Drop-typed `inner` — whole-value consumption at every level → no E2048.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_nested_whole_value_struct_destructure_accepted.ori"
        ),
        0,
        "nested whole-value struct destructure MUST NOT fire E2048",
    );
}

#[test]
fn drop_match_nested_partial_in_enum_payload_rejected_e2048() {
    // Negative pin: outer variant `Wrap(inner)` binds its single payload field
    // (whole-value at the top level), but nested `Inner { x, .. }` partially
    // destructures the Drop-typed payload → E2048.
    let result = check_source(fixture_without_trailing_newline(include_str!(
        "../fixtures/integration/drop_match_nested_partial_in_enum_payload_rejected_e2048.ori"
    )));
    assert!(
        result.drop_partial_move_count() >= 1,
        "nested partial destructure inside an enum payload MUST fire E2048; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn drop_match_nested_partial_on_non_drop_inner_accepted() {
    // Negative-space pin: the E2048 axis is Drop-only. When the NESTED type is
    // NOT a Drop type, a nested partial destructure must NOT fire E2048 — even
    // though the outer type IS Drop. Recursion gates on the nested field's own
    // Drop status, not the outer's.
    assert_drop_partial_move_count(
        include_str!(
            "../fixtures/integration/drop_match_nested_partial_on_non_drop_inner_accepted.ori"
        ),
        0,
        "nested partial destructure of a NON-Drop inner type MUST NOT fire E2048",
    );
}

// E2049 EVALUE_DROP_CONFLICT — runnable source-level enforcement at both
// non-derived registration surfaces. The conflict is pinned through the full
// lex→parse→typecheck pipeline (`check_source`).
//
// `check_source` does not load the prelude, so each source declares a local
// `trait Drop`. The Value marker is supplied via `#derive(Value)`, the
// parseable surface exercised by this integration harness. It co-fires E2033
// (Value-not-derivable), which these tests tolerate by counting E2049
// specifically.

impl CheckResult {
    /// Count `ValueDropConflict` (E2049) errors.
    fn value_drop_conflict_count(&self) -> usize {
        self.error_kinds()
            .iter()
            .filter(|k| matches!(k, TypeErrorKind::ValueDropConflict { .. }))
            .count()
    }
}

fn assert_value_drop_conflict_count(
    check: fn(&str) -> CheckResult,
    source: &'static str,
    expected: usize,
    context: &str,
) {
    let result = check(fixture_without_trailing_newline(source));
    assert_eq!(
        result.value_drop_conflict_count(),
        expected,
        "{context}; kinds: {:?}",
        result.error_kinds()
    );
}

#[test]
fn value_marker_with_drop_impl_fires_e2049_value_first() {
    // Value marker registered FIRST (type decl), Drop impl SECOND → E2049 at
    // the Drop-impl registration surface (Surface 2). Parse-error-tolerant:
    // the `#derive(Value)` form co-emits an E1016 parse diagnostic; E2049
    // still fires.
    assert_value_drop_conflict_count(
        check_source_allow_parse_errors,
        include_str!(
            "../fixtures/integration/value_marker_with_drop_impl_fires_e2049_value_first.ori"
        ),
        1,
        "Value + Drop on the same type MUST fire exactly one E2049",
    );
}

#[test]
fn value_marker_without_drop_impl_no_e2049() {
    // Negative-space pin: the E2049 axis requires BOTH markers. A Value type
    // with NO Drop impl must NOT fire E2049 (it may still fire E2033 for the
    // non-derivable `#derive(Value)` form — that is a different axis).
    assert_value_drop_conflict_count(
        check_source_allow_parse_errors,
        include_str!("../fixtures/integration/value_marker_without_drop_impl_no_e2049.ori"),
        0,
        "Value type without a Drop impl MUST NOT fire E2049",
    );
}

#[test]
fn drop_impl_without_value_marker_no_e2049() {
    // Negative-space pin: a Drop type with NO Value marker must NOT fire
    // E2049 (the conflict requires the Value marker too).
    assert_value_drop_conflict_count(
        check_source,
        include_str!("../fixtures/integration/drop_impl_without_value_marker_no_e2049.ori"),
        0,
        "Drop type without the Value marker MUST NOT fire E2049",
    );
}
