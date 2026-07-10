//! RL-1 yield-identity push duplication funding: a `for w in borrowed_words
//! yield w` loop pushes each iterator-element borrow-view into a FRESH result
//! list via `@ori_list_push(result, w [own], elem_size)`. The element is a
//! borrow into the source buffer; the source is a BORROWED parameter, so the
//! caller retains it across the whole call (RL-2). The push copies the element
//! bytes into the fresh buffer — a real SECOND reference — so per RL-1
//! (`RL1_duplication_balanced`) the store owes one inc, matched by the result
//! collection's `elem_dec_fn` drop.
//!
//! The iterator-element-view exclusion (`collect_iter_element_defs` keeps the
//! view OUT of `owned_vars_needing_rc` to suppress a spurious last-use dec)
//! dropped both the spurious dec AND this load-bearing store-dup inc; the cure
//! (`compute_yield_identity_push_dup_args`) restores ONLY the inc on the
//! genuine duplication. Under-inc surfaces as a double-free (SIGABRT) once both
//! the source's caller-retained reference and the result list dec the shared
//! element.
//!
//! Source-ownership discriminator (GREEN clamp): an OWNED / freshly-`Construct`ed
//! iteration source (`for w in [..] yield w`) is consumed by its own iteration
//! and needs no inc — firing there over-counts (leak). The cure declines it.
//! All cells run under `ORI_CHECK_LEAKS=1` (harness default); interpreter-
//! verified via `assert_cell_output`. Spec: Annex E §AIMS RL-1 + RL-2.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

/// Borrowed-source yield-identity, single call + source reuse: the cured shape.
/// `@clone_list(words: [str])` yields each borrowed element into a fresh list;
/// the caller iterates the copy AND re-iterates the original `words`. Without
/// the store-dup inc both the copy and the surviving `words` dec the shared
/// element -> double-free. With the inc each list owns its own reference.
/// Interpreter prints total=109 total2=109.
const BORROWED_SINGLE_REUSE_SRC: &str = r#"
@clone_list (words: [str]) -> [str] = {
    for w in words yield w
}

@main () -> void = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = clone_list(words: words);
    let total = 0;
    for w in copy do { total = total + w.len(); };
    let total2 = 0;
    for w in words do { total2 = total2 + w.len(); };
    print(msg: `total={total} total2={total2}`)
}
"#;

#[test]
fn test_yield_identity_borrowed_source_single_reuse_no_double_free() {
    assert_cell_output(
        BORROWED_SINGLE_REUSE_SRC,
        "yield_identity_borrowed_source_single_reuse",
        "total=109 total2=109",
    );
}

/// GREEN over-fire clamp: an OWNED `Construct`ed source iterated yield-identity.
/// The source is consumed by its own iteration (`@iter` + `ori_iter_drop`), so
/// the element references transfer — NO store-dup inc is owed. The cure's
/// borrowed-param-source gate DECLINES this shape; a wrong over-fire here would
/// leak (exit 2). Interpreter prints total=109.
#[test]
fn test_yield_identity_owned_source_no_inc_no_leak() {
    assert_cell_output(
        r#"
@main () -> void = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = for w in words yield w;
    let total = 0;
    for w in copy do { total = total + w.len(); };
    print(msg: `total={total}`)
}
"#,
        "yield_identity_owned_source_no_inc",
        "total=109",
    );
}

/// Toggle-parity semantic pin: `ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC=1`
/// removes the store-dup inc, restoring the pre-cure double-free on the
/// borrowed-source single-reuse shape — proving the inc is the load-bearing
/// cure surface.
#[test]
fn test_yield_identity_with_push_dup_inc_disabled_double_frees_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        BORROWED_SINGLE_REUSE_SRC,
        &[
            ("ORI_DISABLE_YIELD_IDENTITY_PUSH_DUP_INC", "1"),
            ("ORI_CLASS_LEDGER_EMITTER", "0"),
        ],
    );
    assert_ne!(
        exit, 0,
        "with the yield-identity push-dup inc disabled, the borrowed-source \
         single-reuse cell must regress (double-free abort, exit != 0)\n\
         stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("double-free"),
        "the regression is the pre-cure double-free shape\nexit={exit}\n\
         stderr:\n{stderr}"
    );
}
