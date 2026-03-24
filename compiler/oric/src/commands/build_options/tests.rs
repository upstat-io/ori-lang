//! Tests for `BuildOptions` merge and CLI parsing.

use ori_repr::NarrowingPolicy;

use super::*;

// TPR-01-035: merge() preserves Disabled narrowing policy
#[test]
fn merge_preserves_disabled_policy() {
    let mut base = BuildOptions::default();
    assert_eq!(
        base.narrowing_policy,
        NarrowingPolicy::Aggressive,
        "default should be Aggressive"
    );

    let other = BuildOptions {
        narrowing_policy: NarrowingPolicy::Disabled,
        ..Default::default()
    };

    base.merge(&other);
    assert_eq!(
        base.narrowing_policy,
        NarrowingPolicy::Disabled,
        "merge must preserve non-default policy"
    );
}

// TPR-01-035: merge default policy does not override non-default
#[test]
fn merge_default_policy_does_not_override() {
    let mut base = BuildOptions {
        narrowing_policy: NarrowingPolicy::Disabled,
        ..Default::default()
    };
    let other = BuildOptions::default(); // Aggressive (default)

    base.merge(&other);
    assert_eq!(
        base.narrowing_policy,
        NarrowingPolicy::Disabled,
        "merging default policy should not override non-default"
    );
}

// TPR-01-035: merge Conservative policy
#[test]
fn merge_preserves_conservative_policy() {
    let mut base = BuildOptions::default();
    let other = BuildOptions {
        narrowing_policy: NarrowingPolicy::Conservative,
        ..Default::default()
    };

    base.merge(&other);
    assert_eq!(
        base.narrowing_policy,
        NarrowingPolicy::Conservative,
        "merge must preserve Conservative policy"
    );
}

// TPR-01-035: parse_build_options recognizes --no-repr-opt
#[test]
fn parse_recognizes_no_repr_opt_flag() {
    let args = vec!["--no-repr-opt".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.narrowing_policy, NarrowingPolicy::Disabled);
}

// TPR-01-035: parse_build_options recognizes --repr-opt=conservative
#[test]
fn parse_recognizes_repr_opt_conservative() {
    let args = vec!["--repr-opt=conservative".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.narrowing_policy, NarrowingPolicy::Conservative);
}

// TPR-01-035: parse_build_options recognizes --repr-opt=aggressive
#[test]
fn parse_recognizes_repr_opt_aggressive() {
    let args = vec!["--repr-opt=aggressive".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.narrowing_policy, NarrowingPolicy::Aggressive);
}

// TPR-01-035: parse_build_options recognizes --repr-opt=disabled
#[test]
fn parse_recognizes_repr_opt_disabled() {
    let args = vec!["--repr-opt=disabled".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.narrowing_policy, NarrowingPolicy::Disabled);
}

// TPR-01-035: default narrowing policy is Aggressive
#[test]
fn default_policy_is_aggressive() {
    let opts = BuildOptions::default();
    assert_eq!(opts.narrowing_policy, NarrowingPolicy::Aggressive);
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
        ..Default::default()
    };

    base.merge(&other);

    assert!(base.lib, "lib not merged");
    assert!(base.dylib, "dylib not merged");
    assert!(base.wasm, "wasm not merged");
    assert!(base.js_bindings, "js_bindings not merged");
    assert!(base.wasm_opt, "wasm_opt not merged");
    assert!(base.verbose, "verbose not merged");
}

// TPR-01-035: per-arg merge loop preserves --no-repr-opt as NarrowingPolicy
#[test]
fn per_arg_merge_loop_preserves_disabled_policy() {
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
    assert_eq!(
        options.narrowing_policy,
        NarrowingPolicy::Disabled,
        "Disabled policy must survive per-arg merge loop"
    );
}

// TPR-01-035: per-arg merge loop with --repr-opt=conservative
#[test]
fn per_arg_merge_loop_preserves_conservative_policy() {
    let args = vec![
        "--release".to_string(),
        "--repr-opt=conservative".to_string(),
    ];

    let mut options = BuildOptions::default();
    for arg in &args {
        let parsed = parse_build_options(std::slice::from_ref(arg));
        options.merge(&parsed);
    }

    assert_eq!(
        options.narrowing_policy,
        NarrowingPolicy::Conservative,
        "Conservative policy must survive per-arg merge loop"
    );
}

// TPR-01-034: merge() must preserve link_mode
#[test]
fn merge_preserves_link_mode() {
    let mut base = BuildOptions::default();
    assert_eq!(base.link_mode, LinkMode::Static, "default is Static");

    let other = BuildOptions {
        link_mode: LinkMode::Dynamic,
        ..Default::default()
    };

    base.merge(&other);
    assert_eq!(
        base.link_mode,
        LinkMode::Dynamic,
        "merge must preserve non-default link_mode"
    );
}

// TPR-01-034: merge() must preserve jobs
#[test]
fn merge_preserves_jobs() {
    let mut base = BuildOptions::default();
    assert_eq!(base.jobs, None, "default is None");

    let other = BuildOptions {
        jobs: Some(4),
        ..Default::default()
    };

    base.merge(&other);
    assert_eq!(base.jobs, Some(4), "merge must preserve jobs when set");
}

// TPR-01-034: merge() does not clear jobs with None
#[test]
fn merge_does_not_clear_jobs() {
    let mut base = BuildOptions {
        jobs: Some(8),
        ..Default::default()
    };
    let other = BuildOptions::default(); // jobs = None

    base.merge(&other);
    assert_eq!(
        base.jobs,
        Some(8),
        "merging None should not clear existing jobs"
    );
}

// TPR-01-034: parse_build_options recognizes --link=dynamic
#[test]
fn parse_recognizes_link_dynamic() {
    let args = vec!["--link=dynamic".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.link_mode, LinkMode::Dynamic);
}

// TPR-01-034: parse_build_options recognizes --jobs=4
#[test]
fn parse_recognizes_jobs() {
    let args = vec!["--jobs=4".to_string()];
    let opts = parse_build_options(&args);
    assert_eq!(opts.jobs, Some(4));
}

// TPR-01-034: per-arg merge loop with link_mode and jobs
#[test]
fn per_arg_merge_preserves_scalar_options() {
    let args = vec![
        "--link=dynamic".to_string(),
        "--jobs=4".to_string(),
        "--release".to_string(),
    ];

    let mut options = BuildOptions::default();
    for arg in &args {
        let parsed = parse_build_options(std::slice::from_ref(arg));
        options.merge(&parsed);
    }

    assert_eq!(
        options.link_mode,
        LinkMode::Dynamic,
        "link_mode must survive per-arg merge"
    );
    assert_eq!(options.jobs, Some(4), "jobs must survive per-arg merge");
    assert!(options.release, "release should be set");
}

// TPR-01-034: semantic pin — default link_mode merge does not override
#[test]
fn merge_default_link_mode_does_not_override() {
    let mut base = BuildOptions {
        link_mode: LinkMode::Dynamic,
        ..Default::default()
    };
    let other = BuildOptions::default(); // link_mode = Static (default)

    base.merge(&other);
    assert_eq!(
        base.link_mode,
        LinkMode::Dynamic,
        "merging default link_mode should not override non-default"
    );
}
