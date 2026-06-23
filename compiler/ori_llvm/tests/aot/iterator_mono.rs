//! AOT pins for `DoubleEndedIterator` methods (`rev` / `last` / `rfind` /
//! `rfold`) reached through the builtin-method dispatch chain.
//!
//! Real AOT compile + run + leak-check (`assert_cell_output` enables
//! `ORI_CHECK_LEAKS=1`) — the runtime gate the §09.2 typeck pins (`ori_types`
//! `s09_2_iterator_method_typechecks_to_list` / `s09_2_dei_consumer_typechecks_to_option`)
//! cannot reach. Every iterator handle realizes as `TypeInfo::Iterator`
//! (`type_name` `"Iterator"`) at codegen, but the registry registers DEI methods
//! under the `"DoubleEndedIterator"` key (`dei_only`); the dispatch re-keys an
//! `"Iterator"` receiver under `"DoubleEndedIterator"` for a `dei_only` method
//! (`codegen/arc_emitter/builtins/mod.rs`) so the existing emitters
//! (`emit_iter_rev` / `emit_iter_last` / `emit_iter_rfind` / `emit_iter_rfold`,
//! `codegen/arc_emitter/builtins/iterator.rs` + `iterator_consumers.rs`) are
//! reached. Reverting the re-key reproduces the `emit_apply` E5001
//! ("unresolved function `rev`") this pin guards against. The printed values
//! are the negative pin against a wrong direction.
//!
//! Read-back uses list indexing / scalar results, not `.join()` on a collected
//! list — `iter().collect().join(sep:)` independently SIGSEGVs at AOT (a
//! pre-existing collect-result-to-join interaction, orthogonal to the DEI
//! dispatch under test; the JIT path is clean).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

// `rev().collect()` over int elements, indexed back in reversed order.
const REV_COLLECT_SRC: &str = r#"
@main () -> void = {
    let $r = [1, 2, 3, 4].iter().rev().collect();
    print(msg: `{r[0]} {r[1]} {r[2]} {r[3]}`);
}
"#;

#[test]
fn test_rev_collect_aot() {
    assert_cell_output(REV_COLLECT_SRC, "rev_collect", "4 3 2 1");
}

// The three DEI consumers (`rfind` / `rfold` / `last`), each returning a scalar
// or Option. Mirrors the interpreter (`ori_eval` `eval_iter_next_back`):
// `rfind` finds the last match, `rfold` folds right-to-left, `last` returns the
// final element.
const DEI_CONSUMERS_SRC: &str = r#"
@main () -> void = {
    let $f = [1, 2, 3, 4, 5].iter().rfind(p -> p > 2);
    let $s = [1, 2, 3].iter().rfold(0, (acc, x) -> acc - x);
    let $l = [10, 20, 30].iter().last();
    print(msg: `{f.unwrap()} {s} {l.unwrap()}`);
}
"#;

#[test]
fn test_dei_consumers_aot() {
    assert_cell_output(DEI_CONSUMERS_SRC, "dei_consumers", "5 -6 30");
}

// `rev().collect()` over heap (str) elements — leak gate on the reversed-buffer
// element ownership (`emit_iter_rev` passes elem inc/dec fns).
const REV_STR_COLLECT_SRC: &str = r#"
@main () -> void = {
    let $r = ["a", "b", "c"].iter().rev().collect();
    print(msg: `{r[0]}{r[1]}{r[2]}`);
}
"#;

#[test]
fn test_rev_str_collect_aot() {
    assert_cell_output(REV_STR_COLLECT_SRC, "rev_str_collect", "cba");
}

// `rev()` chained after a forward adapter — confirms the re-keyed DEI dispatch
// composes with the `"Iterator"`-keyed adapters (`map`).
const REV_AFTER_MAP_SRC: &str = r#"
@main () -> void = {
    let $r = [1, 2, 3].iter().map(x -> x * 10).rev().collect();
    print(msg: `{r[0]} {r[1]} {r[2]}`);
}
"#;

#[test]
fn test_rev_after_map_aot() {
    assert_cell_output(REV_AFTER_MAP_SRC, "rev_after_map", "30 20 10");
}
