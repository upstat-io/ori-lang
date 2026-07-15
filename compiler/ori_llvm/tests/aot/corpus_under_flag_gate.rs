//! Predicate-stack-retirement readiness gate: the corpus IS the probe.
//!
//! The narrow per-shape `predicate_stack_probe` suite certifies self-sufficiency
//! on a fixed subset of value shapes; certifying a subset while the corpus
//! regresses on the complement is the recurring under-coverage failure mode. The
//! readiness signal is therefore the FULL AOT corpus under
//! `ORI_DISABLE_PREDICATE_STACK_RC=1`, asserted as a failing-test-ID-SET subset:
//!
//!     failing_ids_under_flag  is a SUBSET of  baseline_failing_ids
//!
//! `baseline_failing_ids` is the checked-in fixture
//! `fixtures/corpus_under_flag_gate/baseline_failing_ids.txt` -- the set of test
//! IDs that fail on the burden-default path under the test-all.sh verification
//! environment. The gate forbids ADDITIONS to that set; the terminal target is
//! the EMPTY set (Spec: Annex E §AIMS).
//!
//! METRIC CONTRACT: the baseline capture and the live gate run BOTH set
//! `ORI_VERIFY_ARC=1` + `ORI_VERIFY_EACH=1` -- the same gates test-all.sh and
//! CI export. A run without them counts only behavioral failures (leaks,
//! double-frees, wrong output) and silently excludes every VF-1
//! burden-imbalance verification failure, so its failing set is an under-count
//! that MUST NOT be compared against this gate's operands.
//!
//! A SET-subset, NOT an equal-or-lower count: a count-only check masks a
//! regression when a pre-existing baseline failure is incidentally fixed and a
//! NEW failure swaps in at the same count.
//!
//! Two test surfaces:
//! - The fixture-load + subset-comparison HELPERS are unit-tested now
//!   (non-ignored) on synthetic inputs, so the harness logic itself is green.
//! - The live-corpus RUN is `#[ignore]`-gated: the under-flag failing set is not
//!   yet a subset of the baseline (the burden path is not yet the sole RC
//!   emitter corpus-wide), so the gate's PASS VERDICT lands when predicate-stack
//!   retirement + the `BurdenInc -> RcInc` activation dissolve the residual
//!   (Spec: Annex E §AIMS RL-2 / RL-4 / RL-5). The ignore reason carries that
//!   anchor so the disposition is tracked, not red-by-default.
//!
//! On-demand stale-baseline audit (this gate is subset-only — it forbids
//! ADDITIONS but does not flag a baseline cell that is now PASSING): run
//! `diagnostics/aot-guardrail.sh --floor`. It re-runs the corpus
//! under the gated env (`ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1
//! ORI_VERIFY_EACH=1`) vs this baseline and lists STALE entries (now-passing ->
//! prune) plus NEW regressions. Validate a baseline cell there before treating it
//! as live floor; a plain default-path run is a false-green, never a floor verdict.

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

/// Parse libtest's human-readable output, returning the set of `FAILED` test
/// IDs. Matches lines of the form `test <id> ... FAILED`. Used by the live gate
/// to collect the under-flag failing set from a re-run of the aot binary.
fn parse_failed_ids_from_libtest_output(output: &str) -> BTreeSet<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("test ")?;
            let id = rest.strip_suffix(" ... FAILED")?;
            Some(id.trim().to_owned())
        })
        .collect()
}

#[test]
fn baseline_fixture_loads_and_entries_are_well_formed() {
    // A DRAINED baseline (zero entries) is the cohort's terminal state — the
    // gated burden-sole floor reads zero and every former cell passes. The
    // fixture itself must still parse (comment header intact) and any entry
    // that IS present must be a `<module>::<test>` ID -- no stray comment /
    // blank leaked through the parser.
    let baseline = parse_failing_id_set(BASELINE_FIXTURE);
    for id in &baseline {
        assert!(
            id.contains("::") && !id.starts_with('#'),
            "baseline entry `{id}` is not a `<module>::<test>` ID"
        );
    }
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

#[test]
fn failed_id_parser_extracts_ids_from_libtest_output() {
    let output = "\
running 3 tests
test mod_a::test_one ... ok
test mod_b::test_two ... FAILED
test mod_c::test_three ... FAILED

failures:
    mod_b::test_two
    mod_c::test_three

test result: FAILED. 1 passed; 2 failed; 0 ignored
";
    let failed = parse_failed_ids_from_libtest_output(output);
    assert_eq!(
        failed,
        BTreeSet::from(["mod_b::test_two".to_owned(), "mod_c::test_three".to_owned(),]),
        "the parser must extract exactly the `... FAILED` test IDs"
    );
}

// Live-corpus readiness gate. IGNORED until the burden path is the sole RC
// emitter corpus-wide: the under-flag failing set is not yet a subset of the
// baseline, so this would be red-by-default. It runs on demand after predicate-
// stack retirement + the `BurdenInc -> RcInc` activation dissolve the residual
// (the predicate-stack-coupled cohort dissolving at the activation flip, plus the
// remaining joint shapes dissolving via the broad-shape burden-emission
// completion). The PASS VERDICT is the readiness signal for predicate-stack
// retirement.
//
// Mechanism: re-exec THIS aot test binary as a subprocess with
// `ORI_DISABLE_PREDICATE_STACK_RC=1` plus the test-all.sh verification gates,
// excluding this gate test itself (it would recurse), parse the `... FAILED`
// IDs from libtest output, and assert that set adds no ID outside the
// checked-in baseline. Re-capture the baseline under the same gated
// environment per the fixture's re-capture protocol before trusting this
// verdict.
#[test]
#[ignore = "BUG-04-121: burden-path emission fidelity gap. Spec: Annex E §AIMS -- corpus-under-flag SET-subset readiness gate. \
            PASS after predicate-stack retirement + BurdenInc->RcInc activation \
            (RL-2/RL-4/RL-5) make the burden path the current compiled-counter \
            adapter's sole RC emitter corpus-wide; \
            under-flag failing set is not yet a subset of the baseline until then. \
            Re-capture the baseline under the gated environment (ORI_VERIFY_ARC=1 \
            ORI_VERIFY_EACH=1) per the fixture protocol before trusting the \
            subset verdict."]
fn corpus_under_flag_failing_set_is_subset_of_baseline() {
    let baseline = parse_failing_id_set(BASELINE_FIXTURE);
    assert!(
        !baseline.is_empty(),
        "baseline fixture must be populated before the gate can run"
    );

    let test_bin = std::env::current_exe().expect("aot test binary path");

    // Re-run the whole aot corpus under the flag, but exclude this gate test so
    // the subprocess does not recurse into itself.
    let output = std::process::Command::new(&test_bin)
        .env("ORI_DISABLE_PREDICATE_STACK_RC", "1")
        // METRIC CONTRACT: match the test-all.sh verification environment, or
        // the failing set under-counts (VF-1 imbalances become invisible) and
        // the subset verdict is meaningless against the gated baseline.
        .env("ORI_VERIFY_ARC", "1")
        .env("ORI_VERIFY_EACH", "1")
        .args([
            "--skip",
            "corpus_under_flag_gate::corpus_under_flag_failing_set_is_subset_of_baseline",
        ])
        .output()
        .expect("failed to re-exec the aot test binary under the flag");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let failing_under_flag = parse_failed_ids_from_libtest_output(&combined);

    let new_ids = new_ids_over_baseline(&failing_under_flag, &baseline);
    assert!(
        new_ids.is_empty(),
        "predicate-stack-retirement readiness gate FAILED: the under-flag corpus added \
         {} failing ID(s) outside the baseline set:\n{}",
        new_ids.len(),
        new_ids
            .iter()
            .map(|id| format!("  + {id}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
