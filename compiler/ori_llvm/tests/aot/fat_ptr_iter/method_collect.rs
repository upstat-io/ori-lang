//! Method collect tests — F13: `.iter().collect()` exercises `ori_iter_collect` with `elem_inc_fn`.

use crate::util::assert_aot_success;

// [str].iter().collect() → [str]

#[test]
fn test_iter_collect_str_list() {
    // .iter().collect() exercises ori_iter_collect with elem_inc_fn (RcInc per element).
    // Distinct from for-yield which uses ori_list_push (per-element push).
    assert_aot_success(
        r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let collected: [str] = words.iter().collect();
    let total = 0;
    for w in collected do {
        total = total + w.len();
    };
    // Verify original is still valid
    let orig_total = 0;
    for w in words do {
        orig_total = orig_total + w.len();
    };
    if total == 109 && orig_total == 109 then 0 else 1
}
"#,
        "iter_collect_str_list",
    );
}

// Set<str>.iter().collect() → Set<str>

#[test]
fn test_iter_collect_set_str() {
    // ori_iter_collect_set with elem_inc_fn (Section 02.3 TPR-02-009 fix).
    assert_aot_success(
        r#"
@main () -> int = {
    let original: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let collected: Set<str> = original.iter().collect();
    let total = 0;
    for item in collected do {
        total = total + item.len();
    };
    if total == 109 then 0 else 1
}
"#,
        "iter_collect_set_str",
    );
}
