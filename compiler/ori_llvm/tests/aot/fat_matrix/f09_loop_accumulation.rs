//! F09: Loop Accumulation — accumulating fat pointer values across iterations.
//!
//! Tests phi nodes for mutable bindings, RC correctness when values are
//! reassigned in loops, and cleanup of replaced values.

#![expect(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// Accumulate int sum in loop (scalar baseline)
#[test]
fn test_fm_loop_acc_scalar() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for i in [10, 20, 30] do {
        total = total + i;
    };
    if total == 60 then 0 else 1
}
"#,
        "fm_loop_acc_scalar",
    );
}

// Accumulate list length in loop
#[test]
fn test_fm_loop_acc_list_len() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    let words = ["hello", "world", "test"];
    for s in words do {
        total = total + s.length();
    };
    if total == 14 then 0 else 1
}
"#,
        "fm_loop_acc_list_len",
    );
}

// Accumulate map sizes
#[test]
fn test_fm_loop_acc_map() {
    assert_aot_success(
        r#"
@main () -> int = {
    let total = 0;
    for (k, v) in {"a": 10, "b": 20, "c": 30} do {
        total = total + v;
    };
    if total == 60 then 0 else 1
}
"#,
        "fm_loop_acc_map",
    );
}

// Accumulate through function calls on fat values
#[test]
fn test_fm_loop_acc_fn_call() {
    assert_aot_success(
        r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let total = 0;
    for s in ["abc", "de", "fghi"] do {
        total = total + get_len(s: s);
    };
    if total == 9 then 0 else 1
}
"#,
        "fm_loop_acc_fn_call",
    );
}
