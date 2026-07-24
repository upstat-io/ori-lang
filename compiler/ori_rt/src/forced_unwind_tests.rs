extern "C" {
    fn test_forced_unwind_skips_catch() -> i32;
    fn test_forced_unwind_runs_cleanup() -> i32;
}

/// Verify that forced unwind skips catches while running cleanup pads.
#[test]
fn forced_unwind_personality_behavior() {
    // SAFETY: The build script links these precondition-free C harness functions.
    let result = unsafe { test_forced_unwind_skips_catch() };
    assert_eq!(
        result, 0,
        "catch-all handler should not run during forced unwind"
    );

    // SAFETY: The build script links these precondition-free C harness functions.
    let result = unsafe { test_forced_unwind_runs_cleanup() };
    assert_eq!(
        result, 0,
        "cleanup pads should still run during forced unwind"
    );
}
