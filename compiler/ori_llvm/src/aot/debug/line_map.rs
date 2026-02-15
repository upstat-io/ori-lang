//! Line map for byte-offset to line/column conversion.

/// Helper to convert byte offset spans to line/column.
///
/// This structure pre-computes line start offsets for efficient lookup.
#[derive(Debug, Clone)]
pub struct LineMap {
    /// Byte offsets where each line starts (0-indexed).
    /// `line_starts[0]` is always 0 (start of file).
    /// `line_starts[n]` is the byte offset of line n+1.
    line_starts: Vec<u32>,
}

impl LineMap {
    /// Create a line map from source text.
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, c) in source.char_indices() {
            if c == '\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    /// Convert a byte offset to (line, column).
    ///
    /// Both line and column are 1-indexed (standard for debug info).
    #[must_use]
    pub fn offset_to_line_col(&self, offset: u32) -> (u32, u32) {
        // Binary search for the line containing this offset
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(i) => i,      // Exact match at line start
            Err(i) => i - 1, // Between line starts
        };

        let line = (line_idx + 1) as u32; // 1-indexed
        let col = offset - self.line_starts[line_idx] + 1; // 1-indexed

        (line, col)
    }

    /// Get the number of lines in the source.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}
