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
//! Read-back uses list indexing, scalar results, and direct iterator join.

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

// Selection consumers retain the escaping Option payload, then release the
// dynamic mapped yield. Each mapped value exceeds the SSO boundary.
const MAPPED_MANAGED_SELECTION_SRC: &str = r#"
@main () -> void = {
    let suffix = "-mapped-managed-payload-beyond-sso";
    let $found = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .find((s: str) -> s.starts_with(prefix: "beta"));
    let value = match found {
        Some(s) -> s,
        None -> "",
    };
    let $last = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .last();
    let last_value = match last {
        Some(s) -> s,
        None -> "",
    };
    let $rfind = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .rfind((s: str) -> s.starts_with(prefix: "beta"));
    let rfind_value = match rfind {
        Some(s) -> s,
        None -> "",
    };
    print(msg: `{value}|{last_value}|{rfind_value}`);
}
"#;

#[test]
fn mapped_managed_selection_outputs_survive_iterator_teardown() {
    let suffix = "-mapped-managed-payload-beyond-sso";
    let expected = format!("beta{suffix}|gamma{suffix}|beta{suffix}");
    assert_cell_output(
        MAPPED_MANAGED_SELECTION_SRC,
        "mapped_managed_selection",
        &expected,
    );
}

// Every terminal family consumes fresh heap strings from map. The output pins
// short-circuit consumers, full traversal, backward order, and join.
const MAPPED_MANAGED_TERMINALS_SRC: &str = r#"
@rank (s: str) -> int =
    if s.starts_with(prefix: "alpha") then 1
    else if s.starts_with(prefix: "beta") then 2
    else 3;

@main () -> void = {
    let suffix = "-mapped-managed-payload-beyond-sso";
    let count = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .count();
    let any = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .any((s: str) -> s.starts_with(prefix: "beta"));
    let all = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .all((s: str) -> s.contains(substr: "mapped-managed"));
    let expected_beta = "beta" + suffix;
    let filtered = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .filter((s: str) -> s == expected_beta)
        .count();
    ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .for_each((s: str) -> s.length());
    let fold = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .fold(0, (acc: int, s: str) -> int = acc + rank(s: s));
    let rfold = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .rfold(0, (acc: int, s: str) -> int = acc * 10 + rank(s: s));
    let joined = ["alpha", "beta", "gamma"]
        .iter()
        .map((s: str) -> str = s + suffix)
        .join(separator: ",");
    print(msg: `{count}|{any}|{all}|{filtered}|{fold}|{rfold}|{joined}`);
}
"#;

#[test]
fn mapped_managed_terminal_consumers_release_every_yield() {
    let suffix = "-mapped-managed-payload-beyond-sso";
    let expected = format!("3|true|true|1|6|321|alpha{suffix},beta{suffix},gamma{suffix}");
    assert_cell_output(
        MAPPED_MANAGED_TERMINALS_SRC,
        "mapped_managed_terminals",
        &expected,
    );
}

// The first map result is retained into rev's buffer. The second transform
// unwinds, so both that retained prefix and the consumed source yield must drop.
const MAPPED_MANAGED_REV_UNWIND_SRC: &str = r#"
@main () -> void = {
    let suffix = "-mapped-managed-payload-beyond-sso";
    let result = catch(expr: {
        ["alpha", "panic", "gamma"]
            .iter()
            .map((s: str) -> str =
                if s == "panic" then panic(msg: "reverse transform panic")
                else s + suffix)
            .rev()
            .count()
    });
    match result {
        Ok(_) -> panic(msg: "reverse transform should unwind"),
        Err(_) -> print(msg: "caught"),
    }
}
"#;

#[test]
fn mapped_managed_rev_releases_partial_buffer_on_unwind() {
    assert_cell_output(
        MAPPED_MANAGED_REV_UNWIND_SRC,
        "mapped_managed_rev_unwind",
        "caught",
    );
}
