//! Structural regression tests for niche-encoded Option/Result helpers.
//!
//! These tests pin they assert that the niche-encoded
//! `unwrap` / `unwrap_err` / `unwrap_or` / `expect` / `expect_err` helper
//! bodies in `option_result_helpers.rs` contain the required tag-guard
//! and RC-retain calls, mirroring the explicit-tag pattern from
//! `option_result.rs`.
//!
//! Why structural and not behavioral: the niche helpers are dead code
//! today (`NICHE_CODEGEN_READY = false` in `ori_repr/src/canonical/type_repr.rs`).
//! Constructing a synthetic LLVM receiver of the right shape to invoke them
//! end-to-end requires a niche layout the rest of the codegen pipeline does
//! not yet produce. Behavioral verification rides on the
//! `<!-- blocked-by:NICHE_CODEGEN_READY gate -->` items already tracked in
//! `plans/repr-opt/section-07-enum-repr.md` §07.2 — when the gate flips,
//! the niche spec tests under `tests/spec/types/enum/niche/` will exercise
//! these helpers end-to-end.
//!
//! Until then, these structural assertions are the regression guard. A
//! revert of the fix would remove the `emit_unwrap_branch` /
//! `emit_expect_branch` / `inc_value_rc` calls from the helper bodies, and
//! these tests would fail at compile time of the test crate (zero runtime
//! cost, immediate signal).

const HELPER_SRC: &str = include_str!("../option_result_helpers.rs");

/// Walk braces from `body_start` (just past an opening `{`) and return the
/// substring up to the matching closing brace at depth 0.
fn slice_to_matching_brace<'a>(body_region: &'a str, what: &str) -> &'a str {
    let mut depth: i32 = 1;
    let mut end = 0usize;
    for (i, ch) in body_region.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(end > 0, "unclosed body for `{what}`");
    &body_region[..end]
}

/// Extract the body of a method's match arm from the helper source.
///
/// Returns the substring between `"<method>" => {` and the matching closing
/// brace at the same nesting level. Used to scope substring assertions to
/// a single arm so an unrelated arm's body cannot accidentally satisfy the
/// check.
fn extract_arm_body<'a>(src: &'a str, start_after: &str, arm_label: &str) -> &'a str {
    let after_start = src
        .find(start_after)
        .unwrap_or_else(|| panic!("missing function start marker `{start_after}`"));
    let region = &src[after_start..];

    // Find the arm header. Accept both `"unwrap" =>` and `"unwrap" if ... =>`.
    let arm_start = region
        .find(arm_label)
        .unwrap_or_else(|| panic!("missing arm `{arm_label}` in region after `{start_after}`"));
    let after_arm_label = &region[arm_start..];

    // Walk forward to the first `{` that opens the arm body.
    let body_open = after_arm_label
        .find('{')
        .unwrap_or_else(|| panic!("no opening brace after arm `{arm_label}`"));
    let body_start = arm_start + body_open + 1;
    let body_region = &region[body_start..];

    slice_to_matching_brace(body_region, arm_label)
}

/// Extract the body of a free function from the helper source.
///
/// Walks braces starting from the first `{` after `fn_start` so the helper
/// is robust to line-ending differences (LF vs CRLF) and incidental
/// formatting changes — no string-pattern dependency on the exact closing
/// `}\n` shape.
fn extract_fn_body<'a>(src: &'a str, fn_start: &str) -> &'a str {
    let after_start = src
        .find(fn_start)
        .unwrap_or_else(|| panic!("missing function start marker `{fn_start}`"));
    let region = &src[after_start..];

    let body_open = region
        .find('{')
        .unwrap_or_else(|| panic!("no opening brace after fn start `{fn_start}`"));
    let body_region = &region[body_open + 1..];

    slice_to_matching_brace(body_region, fn_start)
}

/// Marker that scopes assertions to within `emit_option_niche` only.
const OPTION_NICHE_FN: &str = "fn emit_option_niche(";

/// Marker that scopes assertions to within `emit_result_niche` only.
const RESULT_NICHE_FN: &str = "fn emit_result_niche(";

// ── §1: Option niche helpers must guard and retain ──

#[test]
fn bug_04_019_option_niche_unwrap_has_panic_guard_and_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, OPTION_NICHE_FN, "\"unwrap\" =>");
    assert!(
        body.contains("emit_unwrap_branch"),
        "Option.unwrap niche arm must call emit_unwrap_branch (panic guard)\nbody:\n{body}",
    );
    assert!(
        body.contains("inc_value_rc"),
        "Option.unwrap niche arm must call inc_value_rc on the extracted payload (RC retain)\nbody:\n{body}",
    );
    assert!(
        body.contains("`Option.unwrap()` on a `None` value"),
        "Option.unwrap niche panic message must match the explicit-tag wording\nbody:\n{body}",
    );
}

#[test]
fn bug_04_019_option_niche_expect_has_expect_branch_and_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, OPTION_NICHE_FN, "\"expect\" if");
    assert!(
        body.contains("emit_expect_branch"),
        "Option.expect niche arm must call emit_expect_branch (user-msg guard)\nbody:\n{body}",
    );
    assert!(
        body.contains("inc_value_rc"),
        "Option.expect niche arm must call inc_value_rc on the extracted payload (RC retain)\nbody:\n{body}",
    );
}

#[test]
fn bug_04_019_option_niche_unwrap_or_has_conditional_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, OPTION_NICHE_FN, "\"unwrap_or\" if");
    // unwrap_or has no panic — it returns the default on None — but it MUST
    // conditionally retain the payload when Some so the inner heap data is
    // not dropped twice (mirrors option_result.rs `unwrap_or` pattern).
    assert!(
        body.contains("inc_value_rc"),
        "Option.unwrap_or niche arm must call inc_value_rc when Some (RC retain)\nbody:\n{body}",
    );
    // Conditional retain pattern: cond_br + inc + br + merge — at least one cond_br.
    assert!(
        body.contains("cond_br"),
        "Option.unwrap_or niche arm must use cond_br + merge for conditional retain (not unconditional, would corrupt None case)\nbody:\n{body}",
    );
}

// ── §2: Result niche helpers must differentiate methods + guard + retain ──

#[test]
fn bug_04_019_result_niche_unwrap_has_panic_guard_and_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"unwrap\" =>");
    assert!(
        body.contains("emit_unwrap_branch"),
        "Result.unwrap niche arm must call emit_unwrap_branch (panic guard)\nbody:\n{body}",
    );
    assert!(
        body.contains("inc_value_rc"),
        "Result.unwrap niche arm must call inc_value_rc on the extracted Ok payload (RC retain)\nbody:\n{body}",
    );
    assert!(
        body.contains("`Result.unwrap()` on an `Err` value"),
        "Result.unwrap niche panic message must match the explicit-tag wording\nbody:\n{body}",
    );
    // Must use the Ok type for the retain (not Err).
    assert!(
        body.contains("ok: ok_ty"),
        "Result.unwrap niche arm must extract ok_ty from TypeInfo::Result for the retain\nbody:\n{body}",
    );
}

#[test]
fn bug_04_019_result_niche_unwrap_err_is_distinct_from_unwrap() {
    let unwrap_body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"unwrap\" =>");
    let unwrap_err_body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"unwrap_err\" =>");
    // Pre-fix bug: `unwrap | unwrap_err | unwrap_or` were collapsed into a
    // single arm. Post-fix: each method has its own body. The semantic pin
    // is that the bodies must NOT be byte-identical.
    assert_ne!(
        unwrap_body.trim(),
        unwrap_err_body.trim(),
        "Result.unwrap and Result.unwrap_err must be SEPARATE arms with distinct bodies — collapsed-arm regression",
    );
    assert!(
        unwrap_err_body.contains("emit_unwrap_branch"),
        "Result.unwrap_err niche arm must call emit_unwrap_branch (panic on Ok)\nbody:\n{unwrap_err_body}",
    );
    assert!(
        unwrap_err_body.contains("inc_value_rc"),
        "Result.unwrap_err niche arm must call inc_value_rc on the extracted Err payload\nbody:\n{unwrap_err_body}",
    );
    // Must use the Err type, not the Ok type.
    assert!(
        unwrap_err_body.contains("err: err_ty"),
        "Result.unwrap_err niche arm must extract err_ty from TypeInfo::Result for the retain\nbody:\n{unwrap_err_body}",
    );
    assert!(
        unwrap_err_body.contains("`Result.unwrap_err()` on an `Ok` value"),
        "Result.unwrap_err niche panic message must match the explicit-tag wording\nbody:\n{unwrap_err_body}",
    );
}

#[test]
fn bug_04_019_result_niche_unwrap_or_has_conditional_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"unwrap_or\" if");
    assert!(
        body.contains("inc_value_rc"),
        "Result.unwrap_or niche arm must call inc_value_rc when Ok\nbody:\n{body}",
    );
    assert!(
        body.contains("cond_br"),
        "Result.unwrap_or niche arm must use cond_br for conditional retain\nbody:\n{body}",
    );
}

#[test]
fn bug_04_019_result_niche_expect_has_expect_branch_and_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"expect\" if");
    assert!(
        body.contains("emit_expect_branch"),
        "Result.expect niche arm must call emit_expect_branch\nbody:\n{body}",
    );
    assert!(
        body.contains("inc_value_rc"),
        "Result.expect niche arm must call inc_value_rc on the extracted Ok payload\nbody:\n{body}",
    );
    assert!(
        body.contains("ok: ok_ty"),
        "Result.expect niche arm must extract ok_ty from TypeInfo::Result\nbody:\n{body}",
    );
}

#[test]
fn bug_04_019_result_niche_expect_err_has_expect_branch_and_rc_retain() {
    let body = extract_arm_body(HELPER_SRC, RESULT_NICHE_FN, "\"expect_err\" if");
    assert!(
        body.contains("emit_expect_branch"),
        "Result.expect_err niche arm must call emit_expect_branch\nbody:\n{body}",
    );
    assert!(
        body.contains("inc_value_rc"),
        "Result.expect_err niche arm must call inc_value_rc on the extracted Err payload\nbody:\n{body}",
    );
    assert!(
        body.contains("err: err_ty"),
        "Result.expect_err niche arm must extract err_ty from TypeInfo::Result\nbody:\n{body}",
    );
}

// ── §3: Result niche unwrap_or must NOT be in a collapsed arm ──

#[test]
fn bug_04_019_result_niche_no_collapsed_unwrap_arm() {
    // Pre-fix bug body: `"unwrap" | "unwrap_err" | "unwrap_or" => { extract... }`.
    // Post-fix: each method has its own arm with the appropriate semantics.
    // The negative pin is that the source must NOT contain the collapsed-arm
    // pattern within emit_result_niche.
    let result_fn_body = extract_fn_body(HELPER_SRC, RESULT_NICHE_FN);

    let collapsed_pattern = "\"unwrap\" | \"unwrap_err\" | \"unwrap_or\"";
    assert!(
        !result_fn_body.contains(collapsed_pattern),
        "regression: Result.unwrap/unwrap_err/unwrap_or are collapsed into a single match arm. Each must be a separate arm with its own tag guard and RC retain.",
    );
}
