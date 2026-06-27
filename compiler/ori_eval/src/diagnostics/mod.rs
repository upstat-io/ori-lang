//! Diagnostic infrastructure for the evaluator.
//!
//! This module provides:
//! - `CallStack` — call frame tracking with depth checking
//! - `CallFrame` — per-call metadata (name, span)
//! - `EvalCounters` — optional performance counters for `--profile`
//!
//! `CallStack` captures backtraces at error sites, providing rich context
//! for runtime error diagnostics. The backtrace is stored on `EvalError`
//! as `EvalBacktrace` (defined in `ori_patterns`).

// Rc is the immutable structural-sharing backbone of the persistent CallStack
// cons-list; no interior mutability is needed, so LocalScope<T> (Rc<RefCell<T>>)
// is the wrong abstraction here.
#![expect(
    clippy::disallowed_types,
    reason = "Rc backs the immutable persistent CallStack cons-list"
)]

use std::rc::Rc;

use ori_ir::{Name, Span, StringInterner};
use ori_patterns::{BacktraceFrame, EvalBacktrace, EvalError};

/// A single frame in the live call stack.
///
/// Stored in `CallStack` during evaluation. When an error occurs,
/// frames are snapshotted into an `EvalBacktrace` via `capture()`.
#[derive(Clone, Debug)]
pub struct CallFrame {
    /// Interned function or method name.
    pub name: Name,
    /// Source location of the call site (where the call was made, not the definition).
    pub call_span: Option<Span>,
}

/// One node of the persistent call-stack cons-list.
///
/// Immutable once constructed: `parent` is shared via `Rc`, so a child call's
/// frame is a single new node pointing at the parent chain.
#[derive(Debug)]
struct CallStackNode {
    frame: CallFrame,
    /// Total frame count up to and including this node — cached so `depth()`
    /// is O(1) and never walks the chain.
    depth: usize,
    parent: Option<Rc<CallStackNode>>,
}

/// Live call stack for the interpreter.
///
/// Each function/method call pushes a frame; return pops it. The depth
/// check is integrated into `push()`.
///
/// # Persistent (structural-sharing) model
///
/// `CallStack` is an immutable singly-linked cons-list: each node holds a
/// frame, a cached `depth`, and an `Rc` to its parent. Cloning the stack for
/// a child call (`create_function_interpreter`) is an O(1) `Rc` refcount bump;
/// pushing a frame onto the clone allocates ONE node pointing at the shared
/// parent, leaving the parent's stack untouched. `depth()` reads the cached
/// head depth in O(1); call-stack maintenance over a recursion chain of depth
/// D is therefore O(D) total.
///
/// # Example
///
/// ```ignore
/// let mut stack = CallStack::new(Some(200));
/// stack.push(CallFrame { name, call_span: Some(span) })?;
/// // ... evaluate function body ...
/// stack.pop();
/// ```
#[derive(Clone, Debug)]
pub struct CallStack {
    head: Option<Rc<CallStackNode>>,
    max_depth: Option<usize>,
}

impl CallStack {
    /// Create a new empty call stack with the given depth limit.
    ///
    /// `max_depth` is `None` for unlimited (native `Interpret` mode)
    /// or `Some(n)` for bounded modes (WASM, `ConstEval`, `TestRun`).
    pub fn new(max_depth: Option<usize>) -> Self {
        Self {
            head: None,
            max_depth,
        }
    }

    /// Push a call frame, checking the depth limit.
    ///
    /// Returns `Err(EvalError)` with `StackOverflow` kind if the limit
    /// is exceeded. The frame is NOT pushed on overflow.
    pub fn push(&mut self, frame: CallFrame) -> Result<(), EvalError> {
        let depth = self.depth();
        if let Some(max) = self.max_depth {
            if depth >= max {
                return Err(ori_patterns::recursion_limit_exceeded(max));
            }
        }
        self.head = Some(Rc::new(CallStackNode {
            frame,
            depth: depth.saturating_add(1),
            parent: self.head.take(),
        }));
        Ok(())
    }

    /// Pop the most recent call frame.
    ///
    /// # Panics
    ///
    /// Panics in debug mode if the stack is empty. In release mode,
    /// this is a no-op on an empty stack.
    pub fn pop(&mut self) {
        debug_assert!(
            self.head.is_some(),
            "CallStack::pop() called on empty stack"
        );
        self.head = self.head.as_ref().and_then(|n| n.parent.clone());
    }

    /// Current call depth.
    #[inline]
    pub fn depth(&self) -> usize {
        self.head.as_ref().map_or(0, |n| n.depth)
    }

    /// Check if the stack is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// The current (most recent) call frame, if any.
    #[inline]
    pub fn current_frame(&self) -> Option<&CallFrame> {
        self.head.as_ref().map(|n| &n.frame)
    }

    /// Capture a snapshot of the current call stack as an `EvalBacktrace`.
    ///
    /// The frames are converted to `BacktraceFrame` using the string interner
    /// to resolve interned `Name`s to display strings. The head is the most
    /// recent call, so walking head -> parent yields most-recent-first directly.
    pub fn capture(&self, interner: &StringInterner) -> EvalBacktrace {
        let Some(mut node) = self.head.as_ref() else {
            return EvalBacktrace::default();
        };
        let mut frames = Vec::with_capacity(node.depth);
        loop {
            frames.push(BacktraceFrame {
                name: interner.lookup(node.frame.name).to_string(),
                span: node.frame.call_span,
            });
            match &node.parent {
                Some(parent) => node = parent,
                None => break,
            }
        }
        EvalBacktrace::new(frames)
    }

    /// Attach a backtrace from this call stack to an error.
    ///
    /// Convenience method for the common pattern of capturing a backtrace
    /// and attaching it to an error at the error site.
    pub fn attach_backtrace(&self, err: EvalError, interner: &StringInterner) -> EvalError {
        if self.head.is_none() {
            return err;
        }
        err.with_backtrace(self.capture(interner))
    }
}

impl Default for CallStack {
    /// Creates an unlimited call stack (native `Interpret` mode default).
    fn default() -> Self {
        Self::new(None)
    }
}

/// Optional performance counters for `--profile` instrumentation.
///
/// Stored as `Option<EvalCounters>` on `ModeState`. When `None`, all
/// counter increments are no-ops (zero cost in production).
///
/// Activated by `--profile` CLI flag.
#[derive(Clone, Debug, Default)]
pub struct EvalCounters {
    pub expressions_evaluated: u64,
    pub function_calls: u64,
    pub method_calls: u64,
    pub pattern_matches: u64,
}

impl EvalCounters {
    /// Increment the expression counter.
    #[inline]
    pub fn count_expression(&mut self) {
        self.expressions_evaluated = self.expressions_evaluated.wrapping_add(1);
    }

    /// Increment the function call counter.
    #[inline]
    pub fn count_function_call(&mut self) {
        self.function_calls = self.function_calls.wrapping_add(1);
    }

    /// Increment the method call counter.
    #[inline]
    pub fn count_method_call(&mut self) {
        self.method_calls = self.method_calls.wrapping_add(1);
    }

    /// Increment the pattern match counter.
    #[inline]
    pub fn count_pattern_match(&mut self) {
        self.pattern_matches = self.pattern_matches.wrapping_add(1);
    }

    /// Merge counters from a child interpreter into this one.
    ///
    /// Used to accumulate profiling data from child interpreters created
    /// for function/method calls back into the parent's counters.
    pub fn merge(&mut self, other: &EvalCounters) {
        self.expressions_evaluated = self
            .expressions_evaluated
            .wrapping_add(other.expressions_evaluated);
        self.function_calls = self.function_calls.wrapping_add(other.function_calls);
        self.method_calls = self.method_calls.wrapping_add(other.method_calls);
        self.pattern_matches = self.pattern_matches.wrapping_add(other.pattern_matches);
    }

    /// Format a summary report.
    pub fn report(&self) -> String {
        format!(
            "Evaluation profile:\n  \
             Expressions evaluated: {}\n  \
             Function calls:        {}\n  \
             Method calls:          {}\n  \
             Pattern matches:       {}",
            self.expressions_evaluated,
            self.function_calls,
            self.method_calls,
            self.pattern_matches,
        )
    }
}

#[cfg(test)]
#[expect(clippy::expect_used, reason = "Tests use expect for brevity")]
mod tests;
