//! Audit report types for the RC verification pass.
//!
//! All fields are `String` (not `&str`) because the report must outlive
//! inkwell's LLVM context — we extract names during the walk and own them.

use std::fmt;

/// Severity of an audit finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "error"),
            Self::Warning => write!(f, "warning"),
            Self::Note => write!(f, "note"),
        }
    }
}

/// Categorized finding kinds for structured filtering and testing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingKind {
    // 04.1 — RC Balance
    /// An `ori_rc_alloc` result was never passed to `ori_rc_dec`.
    RcLeak { alloc_site: String },
    /// `ori_rc_dec` called on a pointer already consumed by a COW function.
    RcDecAfterCow { ptr_name: String, cow_fn: String },
    /// `ori_rc_dec` called on a pointer already decremented (strict mode only).
    RcDoubleDec { ptr_name: String },

    // 04.2 — COW Rules
    /// A pointer was used after being passed to a COW function (not via `ori_rc_dec`).
    CowInputReusedAfterCall { ptr_name: String, cow_fn: String },
    /// `ori_rc_dec` was called on a pointer *before* it was passed to a COW function.
    CowInputDecBeforeCall { ptr_name: String, cow_fn: String },

    // 04.4 — Safety Checks
    /// A call to a panic/assert runtime function (safety check).
    SafetyCheckCall { callee: String },
    /// A conditional branch targeting a panic block (safety guard).
    SafetyCheckBranch { panic_block: String },
    /// Per-function summary: check count and instruction count.
    /// Density (checks per 100 instructions) is derived: `check_count * 100 / instruction_count`.
    SafetyCheckSummary {
        check_count: usize,
        instruction_count: usize,
    },

    // 04.3 — ABI
    /// A call to a runtime function has the wrong number of arguments.
    RuntimeArgCountMismatch {
        fn_name: String,
        expected: usize,
        actual: usize,
    },
    /// An LLVM `load` instruction loads a struct type larger than 16 bytes.
    LargeAggregateLoad { type_size_bytes: u64 },
    /// A `nounwind` function was called via `invoke` (should use `call`).
    NounwindCalledWithInvoke { fn_name: String },
}

/// A single finding from the audit pass.
#[derive(Debug, Clone)]
pub struct AuditFinding {
    /// The LLVM function in which this finding was detected.
    pub function_name: String,
    pub severity: Severity,
    pub kind: FindingKind,
    /// Human-readable description of the issue.
    pub description: String,
}

/// Collected findings from auditing an LLVM module.
#[derive(Debug, Default)]
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
}

impl AuditReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a finding.
    pub fn add(&mut self, finding: AuditFinding) {
        self.findings.push(finding);
    }

    /// Count findings at `Error` severity.
    pub fn error_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .count()
    }

    /// Count findings at `Note` severity.
    pub fn note_count(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Note)
            .count()
    }

    /// True if any finding is at `Error` severity.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Emit all findings to stderr in a human-readable format.
    pub fn emit_to_stderr(&self) {
        for f in &self.findings {
            eprintln!(
                "codegen audit: {}: [{}] {}",
                f.severity, f.function_name, f.description
            );
        }
        let errors = self.error_count();
        let warnings = self
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .count();
        let notes = self.note_count();
        if errors > 0 || warnings > 0 || notes > 0 {
            eprintln!(
                "codegen audit summary: {errors} error(s), {warnings} warning(s), {notes} note(s)"
            );
        }
    }
}
