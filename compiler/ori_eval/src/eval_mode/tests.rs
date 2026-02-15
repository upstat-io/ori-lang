use super::*;

// === EvalMode policy tests ===

#[test]
fn interpret_allows_io() {
    assert!(EvalMode::Interpret.allows_io());
}

#[test]
fn const_eval_forbids_io() {
    assert!(!EvalMode::ConstEval { budget: 100 }.allows_io());
}

#[test]
fn test_run_forbids_io() {
    assert!(!EvalMode::TestRun {
        only_attached: false
    }
    .allows_io());
}

#[test]
fn interpret_allows_entry_point() {
    assert!(EvalMode::Interpret.allows_entry_point());
}

#[test]
fn test_run_forbids_entry_point() {
    assert!(!EvalMode::TestRun {
        only_attached: false
    }
    .allows_entry_point());
}

#[test]
fn const_eval_forbids_capabilities() {
    assert!(!EvalMode::ConstEval { budget: 100 }.allows_capability("Http"));
}

#[test]
fn interpret_allows_capabilities() {
    assert!(EvalMode::Interpret.allows_capability("Http"));
}

#[test]
fn test_run_collects_tests() {
    assert!(EvalMode::TestRun {
        only_attached: true
    }
    .collects_tests());
}

#[test]
fn interpret_does_not_collect_tests() {
    assert!(!EvalMode::Interpret.collects_tests());
}

#[test]
fn const_eval_eager_const_eval() {
    assert!(EvalMode::ConstEval { budget: 100 }.eager_const_eval());
}

#[test]
fn interpret_not_eager_const_eval() {
    assert!(!EvalMode::Interpret.eager_const_eval());
}

#[test]
fn const_eval_recursion_depth_64() {
    assert_eq!(
        EvalMode::ConstEval { budget: 100 }.max_recursion_depth(),
        Some(64)
    );
}

#[test]
fn test_run_recursion_depth_500() {
    assert_eq!(
        EvalMode::TestRun {
            only_attached: false
        }
        .max_recursion_depth(),
        Some(500)
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn interpret_native_unlimited_depth() {
    assert_eq!(EvalMode::Interpret.max_recursion_depth(), None);
}

#[cfg(target_arch = "wasm32")]
#[test]
fn interpret_wasm_depth_200() {
    assert_eq!(EvalMode::Interpret.max_recursion_depth(), Some(200));
}

#[test]
fn default_is_interpret() {
    assert_eq!(EvalMode::default(), EvalMode::Interpret);
}

// === EvalMode Salsa compatibility ===

#[test]
fn eval_mode_is_copy() {
    let mode = EvalMode::TestRun {
        only_attached: true,
    };
    let copied = mode;
    assert_eq!(mode, copied);
}

#[test]
fn eval_mode_is_hashable() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(EvalMode::Interpret);
    set.insert(EvalMode::ConstEval { budget: 100 });
    set.insert(EvalMode::TestRun {
        only_attached: false,
    });
    assert_eq!(set.len(), 3);
}

// === ModeState tests ===

#[test]
fn mode_state_interpret_no_budget() {
    let mode = EvalMode::Interpret;
    let mut state = ModeState::new(&mode);
    // No budget, check_budget always succeeds
    assert!(state.check_budget().is_ok());
    assert!(state.check_budget().is_ok());
}

#[test]
fn mode_state_const_eval_budget_tracking() {
    let mode = EvalMode::ConstEval { budget: 3 };
    let mut state = ModeState::new(&mode);
    assert!(state.check_budget().is_ok()); // call 1
    assert!(state.check_budget().is_ok()); // call 2
    assert!(state.check_budget().is_ok()); // call 3
    assert!(state.check_budget().is_err()); // call 4 — exceeds budget
}

#[test]
fn mode_state_budget_exceeded_has_correct_values() {
    let mode = EvalMode::ConstEval { budget: 1 };
    let mut state = ModeState::new(&mode);
    assert!(state.check_budget().is_ok()); // call 1
    let err = state.check_budget().unwrap_err(); // call 2
    assert_eq!(err.budget, 1);
    assert_eq!(err.calls, 2);
}

// === EvalCounters integration tests ===

#[test]
fn counters_disabled_by_default() {
    let state = ModeState::new(&EvalMode::Interpret);
    assert!(state.counters().is_none());
}

#[test]
fn enable_counters() {
    let mut state = ModeState::new(&EvalMode::Interpret);
    state.enable_counters();
    assert!(state.counters().is_some());
    assert_eq!(state.counters().unwrap().expressions_evaluated, 0);
}

#[test]
fn counter_increments_when_enabled() {
    let mut state = ModeState::new(&EvalMode::Interpret);
    state.enable_counters();
    state.count_expression();
    state.count_expression();
    state.count_function_call();
    state.count_method_call();
    state.count_pattern_match();
    let c = state.counters().unwrap();
    assert_eq!(c.expressions_evaluated, 2);
    assert_eq!(c.function_calls, 1);
    assert_eq!(c.method_calls, 1);
    assert_eq!(c.pattern_matches, 1);
}

#[test]
fn counter_noop_when_disabled() {
    let mut state = ModeState::new(&EvalMode::Interpret);
    // These are no-ops when counters are disabled
    state.count_expression();
    state.count_function_call();
    state.count_method_call();
    state.count_pattern_match();
    assert!(state.counters().is_none());
}

// === Child mode state tests ===

#[test]
fn child_inherits_profiling_enablement() {
    let mode = EvalMode::Interpret;
    let mut parent = ModeState::new(&mode);
    parent.enable_counters();

    let child = ModeState::child(&mode, &parent);
    assert!(child.counters().is_some());
    assert_eq!(child.counters().unwrap().expressions_evaluated, 0);
}

#[test]
fn child_without_profiling_parent() {
    let mode = EvalMode::Interpret;
    let parent = ModeState::new(&mode);

    let child = ModeState::child(&mode, &parent);
    assert!(child.counters().is_none());
}

#[test]
fn merge_child_counters_into_parent() {
    let mode = EvalMode::Interpret;
    let mut parent = ModeState::new(&mode);
    parent.enable_counters();
    parent.count_expression(); // parent: 1 expression

    let mut child = ModeState::child(&mode, &parent);
    child.count_expression();
    child.count_expression();
    child.count_function_call(); // child: 2 expressions, 1 call

    parent.merge_child_counters(&child);

    let c = parent.counters().unwrap();
    assert_eq!(c.expressions_evaluated, 3); // 1 + 2
    assert_eq!(c.function_calls, 1); // 0 + 1
}

#[test]
fn merge_noop_when_profiling_disabled() {
    let mode = EvalMode::Interpret;
    let mut parent = ModeState::new(&mode);
    let child = ModeState::new(&mode);

    // No panic, no-op
    parent.merge_child_counters(&child);
    assert!(parent.counters().is_none());
}
