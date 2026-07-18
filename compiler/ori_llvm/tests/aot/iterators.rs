//! AOT Iterator Tests
//!
//! End-to-end tests for iterator support in LLVM codegen: constructors
//! (`from_list`, `from_range`), adapters (`map`, `filter`, `take`, `skip`,
//! `enumerate`, `zip`, `chain`), consumers (`collect`, `count`, `any`, `all`,
//! `find`, `for_each`, `fold`), and for-loop over Iterator.

use crate::util::{assert_aot_success, assert_cell_output, compile_and_run_capture};

// List.iter() — construct from list

#[test]
fn test_list_iter_count() {
    assert_aot_success(
        include_str!("fixtures/iterators/list_iter_count.ori"),
        "list_iter_count",
    );
}

#[test]
fn test_list_iter_collect() {
    assert_aot_success(
        include_str!("fixtures/iterators/list_iter_collect.ori"),
        "list_iter_collect",
    );
}

// Range.iter() — construct from range

#[test]
fn test_range_iter_count() {
    assert_aot_success(
        include_str!("fixtures/iterators/range_iter_count.ori"),
        "range_iter_count",
    );
}

#[test]
fn test_range_iter_collect() {
    assert_aot_success(
        include_str!("fixtures/iterators/range_iter_collect.ori"),
        "range_iter_collect",
    );
}

#[test]
fn test_range_direct_eager_methods() {
    assert_aot_success(
        include_str!("fixtures/iterators/range_direct_eager_methods.ori"),
        "range_direct_eager_methods",
    );
}

// map adapter

#[test]
fn test_iter_map() {
    assert_aot_success(include_str!("fixtures/iterators/iter_map.ori"), "iter_map");
}

// filter adapter

#[test]
fn test_iter_filter() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_filter.ori"),
        "iter_filter",
    );
}

// flatten adapter

#[test]
fn test_iter_flatten_closure_collection_ownership() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_flatten_closure_collection_ownership.ori"),
        "iter_flatten_closure_collection_ownership",
    );
}

// take adapter

#[test]
fn test_iter_take() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_take.ori"),
        "iter_take",
    );
}

// skip adapter

#[test]
fn test_iter_skip() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_skip.ori"),
        "iter_skip",
    );
}

// count consumer

#[test]
fn test_iter_count_range() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_count_range.ori"),
        "iter_count_range",
    );
}

// for-loop over Iterator

#[test]
fn test_for_over_iterator() {
    assert_aot_success(
        include_str!("fixtures/iterators/for_over_iterator.ori"),
        "for_over_iterator",
    );
}

#[test]
fn test_for_over_range_iterator() {
    assert_aot_success(
        include_str!("fixtures/iterators/for_over_range_iterator.ori"),
        "for_over_range_iterator",
    );
}

// Chained adapters

#[test]
fn test_chained_map_filter_take() {
    assert_aot_success(
        include_str!("fixtures/iterators/chained_map_filter_take.ori"),
        "chained_map_filter_take",
    );
}

// any consumer

#[test]
fn test_iter_any_true() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_any_true.ori"),
        "iter_any_true",
    );
}

#[test]
fn test_iter_any_false() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_any_false.ori"),
        "iter_any_false",
    );
}

// all consumer

#[test]
fn test_iter_all_true() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_all_true.ori"),
        "iter_all_true",
    );
}

#[test]
fn test_iter_all_false() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_all_false.ori"),
        "iter_all_false",
    );
}

// find consumer

#[test]
fn test_iter_find_some() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_find_some.ori"),
        "iter_find_some",
    );
}

#[test]
fn test_iter_find_none() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_find_none.ori"),
        "iter_find_none",
    );
}

// fold consumer

#[test]
fn test_iter_fold_sum() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_fold_sum.ori"),
        "iter_fold_sum",
    );
}

#[test]
fn test_iter_fold_with_filter() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_fold_with_filter.ori"),
        "iter_fold_with_filter",
    );
}

// for_each consumer

#[test]
fn test_iter_for_each() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_for_each.ori"),
        "iter_for_each",
    );
}

// join consumer — string elements (SSO-safe separator ABI)

#[test]
fn test_iter_join_str() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_str.ori"),
        "iter_join_str",
    );
}

// join consumer — non-string elements (to_str trampoline)
// Regression: missing to_str_fn trampoline caused SIGSEGV

#[test]
fn test_iter_join_int() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_int.ori"),
        "iter_join_int",
    );
}

#[test]
fn test_iter_join_float() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_float.ori"),
        "iter_join_float",
    );
}

#[test]
fn test_iter_join_bool() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_bool.ori"),
        "iter_join_bool",
    );
}

#[test]
fn test_iter_join_single_int() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_single_int.ori"),
        "iter_join_single_int",
    );
}

#[test]
fn test_iter_join_int_after_map() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_int_after_map.ori"),
        "iter_join_int_after_map",
    );
}

#[test]
fn test_iter_join_empty_int() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_empty_int.ori"),
        "iter_join_empty_int",
    );
}

/// Cross-feature: join on filtered int iterator exercises filter adapter + `to_str` trampoline.
#[test]
fn test_iter_join_int_after_filter() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_int_after_filter.ori"),
        "iter_join_int_after_filter",
    );
}

/// Char join exercises `ori_str_from_char` via the `to_str` trampoline.
#[test]
fn test_iter_join_char() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_char.ori"),
        "iter_join_char",
    );
}

/// Pins explicit RC decrement for heap-backed `OriStr` join temporaries.
#[test]
fn test_iter_join_long_float() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_join_long_float.ori"),
        "iter_join_long_float",
    );
}

/// Positive pin: join over Printable `Duration` elements compiles, runs, and
/// emits the `to_str`-converted elements ("1s, 2s"). Duration is Printable, so
/// `emit_iter_join`'s `ln_to_str_trampoline` path converts each element.
#[test]
fn test_iter_join_duration_supported() {
    assert_cell_output(
        include_str!("fixtures/iterators/iter_join_duration_supported.ori"),
        "iter_join_duration_supported",
        "1s, 2s",
    );
}

/// Negative pin: join over a genuinely non-Printable element type (a struct
/// with no Printable / `to_str` impl) has no `to_str` trampoline, so
/// `emit_iter_join`'s codegen guard rejects it at compile time (E5001). Pins
/// the reject-genuinely-unsupported-join invariant.
#[test]
fn test_iter_join_nonprintable_rejected() {
    let (exit_code, _, stderr) = compile_and_run_capture(include_str!(
        "fixtures/iterators/iter_join_nonprintable_rejected.ori"
    ));
    // Must fail at compile time (-1), not crash at runtime or silently succeed.
    assert_eq!(
        exit_code, -1,
        "non-Printable struct join should be rejected at compile time, got exit code {exit_code}"
    );
    assert!(
        stderr.contains("not yet supported in LLVM backend"),
        "Expected codegen error for unsupported join element type, got:\n{stderr}"
    );
}

// zip adapter

#[test]
#[ignore = "BUG-02-065: zip + count result-binding misclassified ReturnTag::Fresh -> E2005 unresolved type variable"]
fn test_iter_zip_count() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_zip_count.ori"),
        "iter_zip_count",
    );
}

#[test]
#[ignore = "BUG-02-065: zip unequal-length result-binding misclassified ReturnTag::Fresh -> E2005 unresolved type variable"]
fn test_iter_zip_unequal() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_zip_unequal.ori"),
        "iter_zip_unequal",
    );
}

// chain adapter

#[test]
fn test_iter_chain_count() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_chain_count.ori"),
        "iter_chain_count",
    );
}

#[test]
fn test_iter_chain_collect() {
    assert_aot_success(
        include_str!("fixtures/iterators/iter_chain_collect.ori"),
        "iter_chain_collect",
    );
}

// repeat() — prelude builtin generic, infinite iterator source.

#[test]
fn test_repeat_int_take_sum() {
    assert_aot_success(
        include_str!("fixtures/iterators/repeat_int_take_sum.ori"),
        "repeat_int_take_sum",
    );
}

#[test]
fn test_repeat_str_take_collect() {
    // Heap element: pins per-yield RC clone + master release (leak-checked).
    assert_cell_output(
        include_str!("fixtures/iterators/repeat_str_take_collect.ori"),
        "repeat_str_take_collect",
        "hi,hi,hi",
    );
}
