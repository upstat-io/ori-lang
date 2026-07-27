//! Predicate-stack-retirement certificate: the ordinary AOT corpus is the gate.
//!
//! The predicate-stack RC emitter has been retired from production realization.
//! Therefore the normal AOT run under test-all's verification environment is
//! already the full burden-path gate. Re-executing this test binary from inside
//! itself would merely duplicate the same corpus and obscure its real counts.
//!
//! The checked-in historical failing-ID set is retained as a retirement
//! certificate and must stay drained. Its set-comparison helpers remain pinned
//! so a future cohort gate cannot regress to a count-only comparison that masks
//! swapped failures.
//!
//! A SET-subset, NOT an equal-or-lower count: a count-only check masks a
//! regression when a pre-existing baseline failure is incidentally fixed and a
//! NEW failure swaps in at the same count.
//!
//! Spec: Annex E §AIMS RL-2 / RL-4 / RL-5.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in fixture / output literals"
)]

use std::collections::BTreeSet;

/// The checked-in baseline failing-ID SET (the gate's LEFT operand), embedded at
/// compile time so the harness needs no path resolution to read it.
const BASELINE_FIXTURE: &str =
    include_str!("fixtures/corpus_under_flag_gate/baseline_failing_ids.txt");

/// Parse a baseline / failing-ID list: strip `#` comment lines and blank lines,
/// trim each remaining line, and collect the `<module>::<test>` IDs into a set.
fn parse_failing_id_set(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

/// The IDs in `failing` that are NOT in `baseline` -- the regression set the
/// gate forbids. The subset assertion passes iff this set is empty.
fn new_ids_over_baseline(
    failing: &BTreeSet<String>,
    baseline: &BTreeSet<String>,
) -> BTreeSet<String> {
    failing.difference(baseline).cloned().collect()
}

#[test]
fn baseline_fixture_loads_and_entries_are_well_formed() {
    // A drained baseline is the retirement certificate. Any reintroduced ID
    // would recreate a tolerated failure floor instead of failing the ordinary
    // AOT suite where the regression occurs.
    let baseline = parse_failing_id_set(BASELINE_FIXTURE);
    assert!(
        baseline.is_empty(),
        "retired predicate-stack baseline must remain drained; found {baseline:?}"
    );
    // The fixture file itself is non-empty (the doc header survives even at a
    // drained entry set).
    assert!(
        !BASELINE_FIXTURE.trim().is_empty(),
        "baseline fixture file is empty — the doc header must survive draining"
    );
}

#[test]
fn subset_helper_passes_when_failing_is_subset_of_baseline() {
    let baseline =
        parse_failing_id_set("# header\nmod_a::test_one\nmod_b::test_two\nmod_c::test_three\n");
    // A strict subset (one baseline failure incidentally fixed) -- the gate
    // PASSES: zero new IDs over the baseline.
    let failing = parse_failing_id_set("mod_a::test_one\nmod_c::test_three\n");
    let new_ids = new_ids_over_baseline(&failing, &baseline);
    assert!(
        new_ids.is_empty(),
        "a subset of the baseline must add zero new IDs; got {new_ids:?}"
    );
}

#[test]
fn subset_helper_flags_a_swapped_in_failure_at_equal_count() {
    // The exact regression a count-only check would mask: one baseline failure
    // fixed, one NEW failure swapped in -- count is unchanged, but the SET gains
    // an ID outside the baseline. The subset helper MUST flag it.
    let baseline = parse_failing_id_set("mod_a::test_one\nmod_b::test_two\n");
    let failing = parse_failing_id_set("mod_a::test_one\nmod_z::test_new\n");
    assert_eq!(
        failing.len(),
        baseline.len(),
        "constructed inputs must have equal count so the count-only blind spot is exercised"
    );
    let new_ids = new_ids_over_baseline(&failing, &baseline);
    assert_eq!(
        new_ids,
        BTreeSet::from(["mod_z::test_new".to_owned()]),
        "the swapped-in failure must surface as a new ID over the baseline"
    );
}
