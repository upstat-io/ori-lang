//! Error value with trace storage for the Traceable trait.
//!
//! `ErrorValue` extends the simple error string with a trace of propagation
//! sites, recording where `?` operators forwarded the error. This implements
//! the Ori spec's Traceable trait (§3.13) for the `Error` type.
//!
//! # Trace Accumulation
//!
//! Each `?` operator site that propagates an error appends a `TraceEntryData`
//! with the function name, file path, line, and column. The trace grows as
//! the error bubbles up through the call chain, providing a complete
//! propagation history (similar to Zig's error return traces).

use std::fmt;

/// A single trace entry recording where an error was propagated.
///
/// Corresponds to the `TraceEntry` struct type in the Ori type system.
/// Created by the `?` operator at each propagation site.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraceEntryData {
    /// The function name where propagation occurred.
    pub function: String,
    /// The source file path.
    pub file: String,
    /// The 1-based line number.
    pub line: u32,
    /// The 1-based column number.
    pub column: u32,
}

impl TraceEntryData {
    /// Format this entry as `function at file:line:column`.
    pub fn format(&self) -> String {
        format!(
            "{} at {}:{}:{}",
            self.function, self.file, self.line, self.column
        )
    }
}

impl fmt::Display for TraceEntryData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {}:{}:{}",
            self.function, self.file, self.line, self.column
        )
    }
}

/// An error value with optional trace storage.
///
/// Replaces the previous `Value::Error(String)` representation. When an error
/// is first created, the trace is empty. Each `?` operator that propagates the
/// error appends a `TraceEntryData` entry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ErrorValue {
    /// The error message.
    message: String,
    /// Trace entries accumulated during error propagation.
    trace: Vec<TraceEntryData>,
}

impl ErrorValue {
    /// Create a new error with no trace entries.
    #[inline]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            trace: Vec::new(),
        }
    }

    /// Create a new error with pre-existing trace entries.
    pub fn with_trace(message: impl Into<String>, trace: Vec<TraceEntryData>) -> Self {
        Self {
            message: message.into(),
            trace,
        }
    }

    /// The error message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The accumulated trace entries.
    #[inline]
    pub fn trace(&self) -> &[TraceEntryData] {
        &self.trace
    }

    /// Whether this error has any trace entries.
    #[inline]
    pub fn has_trace(&self) -> bool {
        !self.trace.is_empty()
    }

    /// Append a trace entry (mutates in place).
    ///
    /// Used internally after `Arc::make_mut` or `try_unwrap` to avoid
    /// cloning when the error has a single owner.
    pub fn push_trace(&mut self, entry: TraceEntryData) {
        self.trace.push(entry);
    }

    /// Create a new `ErrorValue` with an additional trace entry appended.
    ///
    /// Returns a new value (functional style) for use when the error is
    /// shared via `Arc` and cannot be mutated in place.
    #[must_use]
    pub fn with_entry(&self, entry: TraceEntryData) -> Self {
        let mut trace = self.trace.clone();
        trace.push(entry);
        Self {
            message: self.message.clone(),
            trace,
        }
    }

    /// Format the trace as a multi-line string.
    ///
    /// Each entry is formatted as `  function at file:line:column`.
    /// Returns an empty string if there are no trace entries.
    pub fn format_trace(&self) -> String {
        if self.trace.is_empty() {
            return String::new();
        }
        self.trace
            .iter()
            .map(TraceEntryData::format)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl fmt::Display for ErrorValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if !self.trace.is_empty() {
            write!(f, "\ntrace:")?;
            for entry in &self.trace {
                write!(f, "\n  {entry}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
