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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
    call_count: usize,
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

    /// Create child mode state that inherits profiling enablement from the parent.
    ///
    /// Fresh counters (zeroed) are created if the parent has profiling enabled,
    /// ensuring child calls are tracked. The caller is responsible for merging
    /// child counters back via `merge_child_counters` after the call returns.
    pub fn child(mode: &EvalMode, parent: &ModeState) -> Self {
        let mut state = Self::new(mode);
        if parent.counters.is_some() {
            state.counters = Some(crate::diagnostics::EvalCounters::default());
        }
        state
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

    /// Merge counters from a child interpreter's `ModeState` into this one.
    ///
    /// No-op when profiling is disabled on either side. Called after each
    /// function/method call returns to accumulate child counters into the parent.
    pub fn merge_child_counters(&mut self, child: &ModeState) {
        if let (Some(parent_counters), Some(child_counters)) =
            (self.counters.as_mut(), child.counters.as_ref())
        {
            parent_counters.merge(child_counters);
        }
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
mod tests;
