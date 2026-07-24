//! Lifecycle classification for registered diagnostic codes.

use super::{ErrorCode, ErrorCodeLifecycle};

impl ErrorCodeLifecycle {
    /// Whether this lifecycle marks a source-emitted code.
    pub const fn is_emitted(self) -> bool {
        matches!(self, Self::Emitted)
    }

    /// Whether this lifecycle marks an intentionally reserved code.
    pub const fn is_reserved(self) -> bool {
        matches!(self, Self::Reserved { .. })
    }

    /// Whether this lifecycle marks a code tracked by a named issue.
    pub const fn is_tracked(self) -> bool {
        matches!(self, Self::Tracked { .. })
    }

    /// Whether this lifecycle marks a retired compatibility code.
    pub const fn is_retired(self) -> bool {
        matches!(self, Self::Retired { .. })
    }
}

impl ErrorCode {
    /// Return the registry lifecycle state for this diagnostic code.
    pub const fn lifecycle(self) -> ErrorCodeLifecycle {
        if let Some(rationale) = self.reserved_rationale() {
            return ErrorCodeLifecycle::Reserved { rationale };
        }

        match self {
            Self::E0911 => ErrorCodeLifecycle::Tracked {
                issue: "BUG-01-016",
                rationale: "stale lexer proposal/code identity conflict is tracked separately",
            },
            _ => ErrorCodeLifecycle::Emitted,
        }
    }

    const fn reserved_rationale(self) -> Option<&'static str> {
        let rationale = match self {
            Self::E1007 => "stable parser code reserved for missing function body recovery; formatter consumes it for suggestions",
            Self::E1011 => "stable parser code reserved for multi-arg call recovery; formatter consumes it for suggestions",
            Self::E1012 => "stable parser code reserved for function sequence syntax diagnostics",
            Self::E1014 => "stable parser code reserved for built-in function name diagnostics",
            Self::E1017 => "stable parser code reserved for typed-lambda missing-equals diagnostics",
            Self::E2002 => "stable type-code slot retained for unknown-type diagnostics and associated fix metadata",
            Self::E2007 => "stable type-code slot reserved for closure self-reference diagnostics",
            Self::E2009 => "stable trait-bound diagnostic slot reserved by docs/spec; current type checker reports related failures through other paths",
            Self::E2011 => "stable named-argument diagnostic slot reserved by call syntax proposals and docs",
            Self::E2012 => "stable capability-diagnostic slot reserved for unknown capability reporting",
            Self::E2013 => "stable capability-diagnostic slot reserved for provider/trait mismatch reporting",
            Self::E2015 => "stable generic-parameter diagnostic slot reserved for ordering violations",
            Self::E2016 => "stable generic-parameter diagnostic slot reserved for missing type arguments",
            Self::E2017 => "stable generic-parameter diagnostic slot reserved for too many type arguments",
            Self::E2042 => "stable FFI burden diagnostic slot reserved by extern-owned-value rules",
            Self::E2045 => "documented stable slot; parser rejects current non-lambda post-condition syntax with E1002 first",
            Self::E3001 => "stable pattern diagnostic slot reserved by pattern docs; current parser emits E1xxx pattern diagnostics",
            Self::E4001 => "stable ARC fallback slot; active ARC diagnostics use E4002-E4005",
            _ => return None,
        };
        Some(rationale)
    }

    /// Check if this is a lexer error (E0xxx range).
    pub fn is_lexer_error(&self) -> bool {
        self.as_str().starts_with("E0")
    }

    /// Check if this is a parser/syntax error (E1xxx range).
    pub fn is_parser_error(&self) -> bool {
        self.as_str().starts_with("E1")
    }

    /// Check if this is a type error (E2xxx range).
    pub fn is_type_error(&self) -> bool {
        self.as_str().starts_with("E2")
    }

    /// Check if this is a pattern error (E3xxx range).
    pub fn is_pattern_error(&self) -> bool {
        self.as_str().starts_with("E3")
    }

    /// Check if this is an ARC analysis error (E4xxx range).
    pub fn is_arc_error(&self) -> bool {
        self.as_str().starts_with("E4")
    }

    /// Check if this is a codegen/LLVM error (E5xxx range).
    pub fn is_codegen_error(&self) -> bool {
        self.as_str().starts_with("E5")
    }

    /// Check if this is a runtime/eval error (E6xxx range).
    pub fn is_eval_error(&self) -> bool {
        self.as_str().starts_with("E6")
    }

    /// Check if this is an internal compiler error (E9xxx range).
    pub fn is_internal_error(&self) -> bool {
        self.as_str().starts_with("E9")
    }

    /// Check if this is a warning code (Wxxx range).
    pub fn is_warning(&self) -> bool {
        self.as_str().starts_with('W')
    }
}
