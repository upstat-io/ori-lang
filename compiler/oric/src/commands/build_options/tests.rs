//! Tests for `BuildOptions` merge and CLI parsing.

use super::*;

// TPR-01-027: merge() must preserve no_repr_opt flag
#[test]
fn merge_preserves_no_repr_opt_flag() {
    let mut base = BuildOptions::default();
    assert!(!base.no_repr_opt, "default should be false");

    let other = BuildOptions {
        no_repr_opt: true,
        ..Default::default()
    };

    base.merge(&other);
    assert!(
        base.no_repr_opt,
        "merge must OR no_repr_opt: true should win"
    );
}

// TPR-01-027: semantic pin — merge must not regress
#[test]
fn merge_no_repr_opt_false_stays_false() {
    let mut base = BuildOptions::default();
    let other = BuildOptions::default();

    base.merge(&other);
    assert!(
        !base.no_repr_opt,
        "merging two false values should stay false"
    );
}

// TPR-01-027: once set, merge cannot clear it
#[test]
fn merge_no_repr_opt_true_not_cleared_by_false() {
    let mut base = BuildOptions {
        no_repr_opt: true,
        ..Default::default()
    };
    let other = BuildOptions::default(); // no_repr_opt = false

    base.merge(&other);
    assert!(base.no_repr_opt, "OR semantics: true | false = true");
}

// TPR-01-027: parse_build_options recognizes --no-repr-opt
#[test]
fn parse_recognizes_no_repr_opt_flag() {
    let args = vec!["--no-repr-opt".to_string()];
    let opts = parse_build_options(&args);
    assert!(opts.no_repr_opt);
}

// TPR-01-027: merge preserves all boolean flags (exhaustive check)
#[test]
fn merge_preserves_all_boolean_flags() {
    let mut base = BuildOptions::default();
    let other = BuildOptions {
        lib: true,
        dylib: true,
        wasm: true,
        js_bindings: true,
        wasm_opt: true,
        verbose: true,
        no_repr_opt: true,
        ..Default::default()
    };

    base.merge(&other);

    assert!(base.lib, "lib not merged");
    assert!(base.dylib, "dylib not merged");
    assert!(base.wasm, "wasm not merged");
    assert!(base.js_bindings, "js_bindings not merged");
    assert!(base.wasm_opt, "wasm_opt not merged");
    assert!(base.verbose, "verbose not merged");
    assert!(base.no_repr_opt, "no_repr_opt not merged");
}

// TPR-01-027: simulates main.rs per-arg parsing loop
#[test]
fn per_arg_merge_loop_preserves_no_repr_opt() {
    // Simulates the main.rs pattern: parse one arg at a time, merge into base
    let args = vec![
        "--release".to_string(),
        "--no-repr-opt".to_string(),
        "-v".to_string(),
    ];

    let mut options = BuildOptions::default();
    for arg in &args {
        let parsed = parse_build_options(std::slice::from_ref(arg));
        options.merge(&parsed);
    }

    assert!(options.release, "release should be set");
    assert!(options.verbose, "verbose should be set");
    assert!(
        options.no_repr_opt,
        "no_repr_opt must survive per-arg merge loop"
    );
}
