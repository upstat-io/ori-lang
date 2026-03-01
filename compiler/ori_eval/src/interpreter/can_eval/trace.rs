//! Error trace injection for the `?` operator.
//!
//! When `?` propagates an `Err(Error(...))`, these helpers append a
//! `TraceEntryData` with the current source location (function, file,
//! line, column). Extracted from `can_eval/mod.rs` to keep the main
//! dispatch module focused.

use ori_ir::canon::CanId;
use ori_patterns::{TraceEntryData, Value};

use super::Interpreter;

impl Interpreter<'_> {
    /// Inject a trace entry into an error value at a `?` operator site.
    ///
    /// If the value is `Value::Err(Value::Error(...))`, appends a `TraceEntryData`
    /// recording the current function name and source location. Non-error values
    /// are returned unchanged.
    ///
    /// Uses `Heap::make_mut` for copy-on-write: when the error is uniquely owned
    /// (common case — errors propagate linearly through `?`), the trace entry
    /// is appended in place with no cloning.
    pub(super) fn inject_trace_entry(&self, mut value: Value, can_id: CanId) -> Value {
        // Guard: only Err(Error(...)) values carry traces
        if !matches!(&value, Value::Err(inner) if matches!(&**inner, Value::Error(_))) {
            return value;
        }

        // Build the function name from the call stack
        let function_name = self.call_stack.current_frame().map_or_else(
            || "<top-level>".to_string(),
            |f| self.interner.lookup(f.name).to_string(),
        );

        // Compute line/column from span byte offset
        let span = self.can_span(can_id);
        let (line, column) = self.line_col_from_offset(span.start);

        let file = self
            .source_file_path
            .as_deref()
            .cloned()
            .unwrap_or_else(|| "<unknown>".to_string());

        let entry = TraceEntryData {
            function: function_name,
            file,
            line,
            column,
        };

        // Copy-on-write through two Heap layers: Err(Heap<Value>) → Error(Heap<ErrorValue>)
        if let Value::Err(ref mut outer) = value {
            if let Value::Error(ref mut ev_heap) = *outer.make_mut() {
                ev_heap.make_mut().push_trace(entry);
            }
        }
        value
    }

    /// Compute 1-based line and column from a byte offset in the source text.
    ///
    /// Counts newlines in `source_text[..offset]` to determine line number,
    /// then computes column from the last newline position. If no source text
    /// is available, returns `(0, 0)`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "u32↔usize: source offsets and line/column numbers fit in u32"
    )]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "column = (end - last_newline) + 1: last_newline ≤ end by construction"
    )]
    pub(super) fn line_col_from_offset(&self, offset: u32) -> (u32, u32) {
        let Some(src) = &self.source_text else {
            return (0, 0);
        };
        let offset = offset as usize;
        let bytes = src.as_bytes();
        let end = offset.min(bytes.len());

        let mut line: u32 = 1;
        let mut last_newline: usize = 0;
        for (i, &b) in bytes[..end].iter().enumerate() {
            if b == b'\n' {
                line = line.wrapping_add(1);
                last_newline = i.wrapping_add(1);
            }
        }
        let column = (end - last_newline) as u32 + 1;
        (line, column)
    }
}
