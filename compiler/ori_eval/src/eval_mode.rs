//! Evaluation modes for the Ori interpreter.
//!
//! Parameterizes the evaluator via an `EvalMode` enum so `ori run`, `ori check`,
//! and `ori test` use distinct evaluation policies. Uses enum dispatch (not generics)
//! for Salsa compatibility.
//!
//! # Prior Art
//!
//! - **Rust** `InterpCx<M: Machine>` — trait-generic, but Ori needs Salsa compat
//! - **Go** operand modes — `constant_`, `value`, `builtin`, `invalid`
//! - **Zig** `block.isComptime()` — switches compile-time vs runtime evaluation

/// Evaluation mode — determines interpreter behavior via match dispatch.
///
/// Enum (not trait) for Salsa compatibility: `Clone, Eq, Hash, Debug` required.
/// Each variant controls I/O access, recursion limits, test collection, and
/// const-eval budget through policy methods.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum EvalMode {
    /// Standard mode for `ori run` — full I/O, capabilities enabled.
    #[default]
    Interpret,
    /// Compile-time evaluation — no I/O, budget-limited, deterministic.
    ConstEval {
        /// Maximum number of function calls before aborting.
        budget: u32,
    },
    /// Test execution — captures output, collects results.
    TestRun {
        /// When true, only run tests attached to functions (not floating tests).
        only_attached: bool,
    },
}

impl EvalMode {
    /// Whether this mode allows I/O operations (print, file, network).
    ///
    /// Only `Interpret` mode has unrestricted I/O. `ConstEval` must be pure,
    /// and `TestRun` captures output via a buffer handler instead.
    #[inline]
    pub fn allows_io(&self) -> bool {
        matches!(self, Self::Interpret)
    }

    /// Whether this mode allows program entry points (`@main`).
    ///
    /// Only `Interpret` mode runs entry points. Test mode discovers and runs
    /// test functions instead.
    #[inline]
    pub fn allows_entry_point(&self) -> bool {
        matches!(self, Self::Interpret)
    }

    /// Whether this mode allows the given capability.
    ///
    /// `ConstEval` forbids all capabilities (must be pure/deterministic).
    /// Other modes allow all capabilities.
    #[inline]
    pub fn allows_capability(&self, _cap: &str) -> bool {
        !matches!(self, Self::ConstEval { .. })
    }

    /// Whether this mode collects test results.
    #[inline]
    pub fn collects_tests(&self) -> bool {
        matches!(self, Self::TestRun { .. })
    }

    /// Whether this mode eagerly evaluates compile-time constants.
    #[inline]
    pub fn eager_const_eval(&self) -> bool {
        matches!(self, Self::ConstEval { .. })
    }

    /// Maximum recursion depth, or `None` for unlimited (native `stacker` fallback).
    ///
    /// - `Interpret`: `None` on native (stacker grows the stack), 200 on WASM
    /// - `ConstEval`: Always 64 (tight budget prevents runaway)
    /// - `TestRun`: Always 500 (generous but bounded)
    #[inline]
    pub fn max_recursion_depth(&self) -> Option<usize> {
        match self {
            Self::Interpret => {
                #[cfg(target_arch = "wasm32")]
                {
                    Some(200)
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    None
                }
            }
            Self::ConstEval { .. } => Some(64),
            Self::TestRun { .. } => Some(500),
        }
    }
}

/// Per-mode mutable state stored alongside `EvalMode`.
///
/// Fields are initialized based on the active mode. Unused fields for
/// the current mode are `None` (zero overhead).
pub struct ModeState {
    /// Call counter for budget tracking (`ConstEval` only).
    pub call_count: usize,
    /// Maximum call budget (`ConstEval` only).
    budget: Option<u32>,
    /// Optional performance counters activated by `--profile`.
    ///
    /// When `None`, all counter increments are no-ops (zero cost in production).
    /// When `Some`, counters are incremented during evaluation and printed at the end.
    counters: Option<crate::diagnostics::EvalCounters>,
}

impl ModeState {
    /// Create mode state appropriate for the given evaluation mode.
    pub fn new(mode: &EvalMode) -> Self {
        match mode {
            EvalMode::ConstEval { budget } => Self {
                call_count: 0,
                budget: Some(*budget),
                counters: None,
            },
            _ => Self {
                call_count: 0,
                budget: None,
                counters: None,
            },
        }
    }

    /// Enable performance counters (activated by `--profile` CLI flag).
    pub fn enable_counters(&mut self) {
        self.counters = Some(crate::diagnostics::EvalCounters::default());
    }

    /// Check and increment the call budget for `ConstEval` mode.
    ///
    /// Returns `Ok(())` for non-`ConstEval` modes (no budget tracking).
    /// Returns `Err` if the budget is exceeded.
    #[inline]
    pub fn check_budget(&mut self) -> Result<(), BudgetExceeded> {
        if let Some(budget) = self.budget {
            self.call_count = self.call_count.saturating_add(1);
            if self.call_count > budget as usize {
                return Err(BudgetExceeded {
                    budget,
                    calls: self.call_count,
                });
            }
        }
        Ok(())
    }

    /// Increment the expression evaluation counter (no-op when profiling is off).
    #[inline]
    pub fn count_expression(&mut self) {
        if let Some(ref mut c) = self.counters {
            c.count_expression();
        }
    }

    /// Increment the function call counter (no-op when profiling is off).
    #[inline]
    pub fn count_function_call(&mut self) {
        if let Some(ref mut c) = self.counters {
            c.count_function_call();
        }
    }

    /// Increment the method call counter (no-op when profiling is off).
    #[inline]
    pub fn count_method_call(&mut self) {
        if let Some(ref mut c) = self.counters {
            c.count_method_call();
        }
    }

    /// Increment the pattern match counter (no-op when profiling is off).
    #[inline]
    pub fn count_pattern_match(&mut self) {
        if let Some(ref mut c) = self.counters {
            c.count_pattern_match();
        }
    }

    /// Get the counters for reporting (returns `None` when profiling is off).
    pub fn counters(&self) -> Option<&crate::diagnostics::EvalCounters> {
        self.counters.as_ref()
    }
}

/// Error returned when `ConstEval` budget is exceeded.
#[derive(Debug)]
pub struct BudgetExceeded {
    /// The configured budget limit.
    pub budget: u32,
    /// The number of calls made.
    pub calls: usize,
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests {
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
    fn eval_mode_is_clone() {
        let mode = EvalMode::TestRun {
            only_attached: true,
        };
        let cloned = mode.clone();
        assert_eq!(mode, cloned);
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
}
