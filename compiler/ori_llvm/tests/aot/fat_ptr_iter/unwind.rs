//! Unwind path tests — panic during iteration with catch recovery.

use crate::util::assert_aot_success;

#[test]
fn test_unwind_panic_during_str_iteration() {
    assert_aot_success(
        r#"
@might_panic (s: str) -> int = {
    if s == "boom this is a long enough string for heap allocation" then {
        panic(msg: "kaboom during iteration")
    };
    s.len()
}

@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + might_panic(s: w)
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should never be reached at all"
    ];
    let result = catch(expr: process(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_panic_during_str_iteration",
    );
}

/// Panic during iteration, then reuse the list — verifies RC is correct
/// after unwind (list still accessible, no double-free).
#[test]
fn test_unwind_list_reusable_after_catch() {
    assert_aot_success(
        r#"
@panicking_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w == "boom this is a long enough string for heap allocation" then {
            panic(msg: "kaboom")
        };
        total = total + w.len()
    };
    total
}

@safe_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let r1 = catch(expr: panicking_iter(words: words));
    let r2 = safe_iter(words: words);
    match r1 {
        Ok(_) -> 1,
        Err(_) -> if r2 == 160 then 0 else 1
    }
}
"#,
        "unwind_list_reusable_after_catch",
    );
}

/// Multiple invoke calls in one function — panic at second call, verify
/// cleanup is correct for both call sites.
#[test]
fn test_unwind_multiple_invokes_with_panic() {
    assert_aot_success(
        r#"
@count_lengths (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len()
    };
    total
}

@panicking_count (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w == "boom this is a long enough string for heap allocation" then {
            panic(msg: "kaboom in second call")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let r1 = count_lengths(words: words);
    let r2 = catch(expr: panicking_count(words: words));
    match r2 {
        Ok(_) -> 1,
        Err(_) -> if r1 == 160 then 0 else 1
    }
}
"#,
        "unwind_multiple_invokes_with_panic",
    );
}

/// Panic inside nested function call chain — A calls B calls C, C panics.
/// Verifies unwind cleanup propagates correctly through multiple frames.
#[test]
fn test_unwind_nested_call_chain_panic() {
    assert_aot_success(
        r#"
@inner (s: str) -> int = {
    if s == "boom this is a long enough string for heap allocation" then {
        panic(msg: "deep panic")
    };
    s.len()
}

@middle (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + inner(s: w)
    };
    total
}

@outer (words: [str]) -> int = {
    middle(words: words)
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "boom this is a long enough string for heap allocation",
        "third long string that should also be on the heap here"
    ];
    let result = catch(expr: outer(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_nested_call_chain_panic",
    );
}

/// Partial iteration + break, then panic in separate call — verifies
/// that break cleanup and unwind cleanup are independent and correct.
#[test]
fn test_unwind_break_then_panic() {
    assert_aot_success(
        r#"
@partial_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.len() > 60 then break;
        total = total + w.len()
    };
    total
}

@panicking_func (words: [str]) -> int = {
    panic(msg: "always panics")
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold plus",
        "third long string that should also be on the heap here now"
    ];
    let r1 = partial_iter(words: words);
    let r2 = catch(expr: panicking_func(words: words));
    match r2 {
        Ok(_) -> 1,
        Err(_) -> if r1 == 53 then 0 else 1
    }
}
"#,
        "unwind_break_then_panic",
    );
}

/// Panic at FIRST element during iteration — iterator is live but no
/// elements have been consumed yet. Verifies cleanup handles zero-consumed
/// iterator state correctly.
#[test]
fn test_unwind_panic_at_first_element() {
    assert_aot_success(
        r#"
@process (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.starts_with(prefix: "boom") then {
            panic(msg: "panic at first element")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "boom this is a long enough string for heap allocation",
        "second very long string that also exceeds the threshold",
        "third long string that should also be on the heap here"
    ];
    let result = catch(expr: process(words: words));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_panic_at_first_element",
    );
}

/// Repeated catch/panic cycles on the same list — stresses RC balance
/// across multiple unwind/recovery sequences.
#[test]
fn test_unwind_repeated_catch_cycles() {
    assert_aot_success(
        r#"
@panicking_iter (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        if w.starts_with(prefix: "boom") then {
            panic(msg: "cycle panic")
        };
        total = total + w.len()
    };
    total
}

@main () -> int = {
    let words = [
        "boom this is a long enough string for heap allocation",
        "second very long string that also exceeds the threshold"
    ];
    let r1 = catch(expr: panicking_iter(words: words));
    let r2 = catch(expr: panicking_iter(words: words));
    let r3 = catch(expr: panicking_iter(words: words));
    let all_err = match r1 { Ok(_) -> false, Err(_) -> true }
        && match r2 { Ok(_) -> false, Err(_) -> true }
        && match r3 { Ok(_) -> false, Err(_) -> true };
    if all_err then 0 else 1
}
"#,
        "unwind_repeated_catch_cycles",
    );
}

/// Non-iterator local heap value in callee + panic — verifies general
/// RC cleanup for non-iterator heap variables on unwind path.
#[test]
fn test_unwind_callee_local_heap_value() {
    assert_aot_success(
        r#"
@process (input: str) -> int = {
    let local_copy = input + " extra text to ensure heap allocation";
    if local_copy.len() > 50 then {
        panic(msg: "callee local panic")
    };
    local_copy.len()
}

@main () -> int = {
    let s = "this is a very long string that exceeds SSO threshold";
    let result = catch(expr: process(input: s));
    match result {
        Ok(_) -> 1,
        Err(_) -> 0
    }
}
"#,
        "unwind_callee_local_heap_value",
    );
}
