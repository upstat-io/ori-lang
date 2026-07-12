//! Element-ownership matrix for `iter().join(separator:)` chains.
//!
//! Pins the uniform +1-owned yield contract at the join consumer: an
//! adapter-produced fresh element is released exactly once by join, while a
//! source-borrowed element stays owned by the source list (its drop is the
//! sole release). `assert_cell_output` catches all three failure shapes —
//! leak (exit 2 under `ORI_CHECK_LEAKS=1`), double-free (signal exit), and
//! silent wrong value. Every fixture uses >23-byte strings so heap RC (not
//! SSO inline storage) carries the elements.

use crate::util::assert_cell_output;

#[test]
fn test_map_join_heap_strs_leak_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_join_heap_strs.ori"),
        "map_join_heap_strs",
        "alpha-alpha-alpha-alpha-alpha-suffix-suffix-suffix-suffix, beta-beta-beta-beta-beta-beta-suffix-suffix-suffix-suffix",
    );
}

#[test]
fn test_map_join_single_element_leak_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_join_single.ori"),
        "map_join_single",
        "gamma-gamma-gamma-gamma-gamma-suffix-suffix-suffix-suffix",
    );
}

#[test]
fn test_map_filter_join_pass_through_owned_leak_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_filter_join.ori"),
        "map_filter_join",
        "delta-delta-delta-delta-delta-suffix-suffix-suffix-suffix|epsilon-epsilon-epsilon-epsilon-suffix-suffix-suffix-suffix",
    );
}

#[test]
fn test_filter_map_join_terminal_fresh_leak_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/filter_map_join.ori"),
        "filter_map_join",
        "zeta-zeta-zeta-zeta-zeta-zeta-suffix-suffix-suffix-suffix; eta-eta-eta-eta-eta-eta-eta-suffix-suffix-suffix-suffix",
    );
}

#[test]
fn test_map_take_join_untaken_element_leak_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_take_join.ori"),
        "map_take_join",
        "theta-theta-theta-theta-theta-suffix-suffix-suffix-suffix,iota-iota-iota-iota-iota-iota-suffix-suffix-suffix-suffix",
    );
}

#[test]
fn test_plain_join_source_borrowed_no_double_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/plain_join_no_double_free.ori"),
        "plain_join_no_double_free",
        "lambda-lambda-lambda-lambda-lambda, mu-mu-mu-mu-mu-mu-mu-mu-mu",
    );
}

#[test]
fn test_filter_only_join_pass_through_borrowed_no_double_free() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/filter_only_join.ori"),
        "filter_only_join",
        "nu-nu-nu-nu-nu-nu-nu-nu-nu-nu xi-xi-xi-xi-xi-xi-xi-xi-xi-xi",
    );
}

#[test]
fn test_int_join_trampoline_produced_string_freed() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/int_join_trampoline.ori"),
        "int_join_trampoline",
        "1, 2, 3",
    );
}

#[test]
fn test_empty_source_map_join_no_release_owed() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/empty_map_join.ori"),
        "empty_map_join",
        "",
    );
}

/// Boundary cell: filter DISCARDS a map-produced fresh element — the leak is
/// inside the adapter's discard path, not the join consumer.
#[test]
#[ignore = "BUG-05-019: adapter-internal discard leak — filter drops a fresh element with no release; cured by the adapter yield/discard contract migration, not the join consumer fix"]
fn test_map_filter_discard_join_rejected_element_freed() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_filter_discard_join.ori"),
        "map_filter_discard_join",
        "rho-rho-rho-rho-rho-rho-rho-suffix-suffix-suffix-suffix, tau-tau-tau-tau-tau-tau-tau-suffix-suffix-suffix-suffix",
    );
}

/// Boundary cell: producer-over-producer chain — the inner map's fresh
/// intermediates are consumed inside the outer map adapter and leak there.
#[test]
#[ignore = "BUG-05-019: adapter-internal intermediate leak — outer map consumes the inner map's fresh element with no release; cured by the adapter yield contract migration, not the join consumer fix"]
fn test_map_map_join_intermediate_freed() {
    assert_cell_output(
        include_str!("../fixtures/fat_ptr_iter/join_ownership/map_map_join.ori"),
        "map_map_join",
        "upsilon-upsilon-upsilon-upsilon-mid-mid-mid-mid-mid-mid-outer-outer-outer, phi-phi-phi-phi-phi-phi-phi-mid-mid-mid-mid-mid-mid-outer-outer-outer",
    );
}
