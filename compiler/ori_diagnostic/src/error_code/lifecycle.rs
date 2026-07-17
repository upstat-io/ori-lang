//! Lifecycle classification for registered diagnostic codes.

use super::{ErrorCode, ErrorCodeLifecycle};

impl ErrorCode {
    /// Return the registry lifecycle state for this diagnostic code.
    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive one-arm-per-ErrorCode data table, not control flow"
    )]
    pub const fn lifecycle(self) -> ErrorCodeLifecycle {
        match self {
            ErrorCode::E0911 => ErrorCodeLifecycle::Tracked {
                issue: "BUG-01-016",
                rationale: "stale lexer proposal/code identity conflict is tracked separately",
            },
            ErrorCode::E1007 => ErrorCodeLifecycle::Reserved {
                rationale: "stable parser code reserved for missing function body recovery; formatter consumes it for suggestions",
            },
            ErrorCode::E1011 => ErrorCodeLifecycle::Reserved {
                rationale: "stable parser code reserved for multi-arg call recovery; formatter consumes it for suggestions",
            },
            ErrorCode::E1012 => ErrorCodeLifecycle::Reserved {
                rationale: "stable parser code reserved for function sequence syntax diagnostics",
            },
            ErrorCode::E1014 => ErrorCodeLifecycle::Reserved {
                rationale: "stable parser code reserved for built-in function name diagnostics",
            },
            ErrorCode::E1017 => ErrorCodeLifecycle::Reserved {
                rationale: "stable parser code reserved for typed-lambda missing-equals diagnostics",
            },
            ErrorCode::E2002 => ErrorCodeLifecycle::Reserved {
                rationale: "stable type-code slot retained for unknown-type diagnostics and associated fix metadata",
            },
            ErrorCode::E2007 => ErrorCodeLifecycle::Reserved {
                rationale: "stable type-code slot reserved for closure self-reference diagnostics",
            },
            ErrorCode::E2009 => ErrorCodeLifecycle::Reserved {
                rationale: "stable trait-bound diagnostic slot reserved by docs/spec; current type checker reports related failures through other paths",
            },
            ErrorCode::E2011 => ErrorCodeLifecycle::Reserved {
                rationale: "stable named-argument diagnostic slot reserved by call syntax proposals and docs",
            },
            ErrorCode::E2012 => ErrorCodeLifecycle::Reserved {
                rationale: "stable capability-diagnostic slot reserved for unknown capability reporting",
            },
            ErrorCode::E2013 => ErrorCodeLifecycle::Reserved {
                rationale: "stable capability-diagnostic slot reserved for provider/trait mismatch reporting",
            },
            ErrorCode::E2015 => ErrorCodeLifecycle::Reserved {
                rationale: "stable generic-parameter diagnostic slot reserved for ordering violations",
            },
            ErrorCode::E2016 => ErrorCodeLifecycle::Reserved {
                rationale: "stable generic-parameter diagnostic slot reserved for missing type arguments",
            },
            ErrorCode::E2017 => ErrorCodeLifecycle::Reserved {
                rationale: "stable generic-parameter diagnostic slot reserved for too many type arguments",
            },
            ErrorCode::E2042 => ErrorCodeLifecycle::Reserved {
                rationale: "stable FFI burden diagnostic slot reserved by extern-owned-value rules",
            },
            ErrorCode::E2045 => ErrorCodeLifecycle::Reserved {
                rationale: "documented stable slot; parser rejects current non-lambda post-condition syntax with E1002 first",
            },
            ErrorCode::E3001 => ErrorCodeLifecycle::Reserved {
                rationale: "stable pattern diagnostic slot reserved by pattern docs; current parser emits E1xxx pattern diagnostics",
            },
            ErrorCode::E4001 => ErrorCodeLifecycle::Reserved {
                rationale: "stable ARC fallback slot; active ARC diagnostics currently use E4002-E4005",
            },
            ErrorCode::E0001
            | ErrorCode::E0002
            | ErrorCode::E0003
            | ErrorCode::E0004
            | ErrorCode::E0005
            | ErrorCode::E0006
            | ErrorCode::E0008
            | ErrorCode::E0009
            | ErrorCode::E0010
            | ErrorCode::E0011
            | ErrorCode::E0012
            | ErrorCode::E0013
            | ErrorCode::E0014
            | ErrorCode::E0015
            | ErrorCode::E0860
            | ErrorCode::E0861
            | ErrorCode::E0932
            | ErrorCode::E1001
            | ErrorCode::E1002
            | ErrorCode::E1003
            | ErrorCode::E1004
            | ErrorCode::E1005
            | ErrorCode::E1006
            | ErrorCode::E1008
            | ErrorCode::E1009
            | ErrorCode::E1010
            | ErrorCode::E1013
            | ErrorCode::E1015
            | ErrorCode::E1016
            | ErrorCode::E1018
            | ErrorCode::E1019
            | ErrorCode::E1020
            | ErrorCode::E2001
            | ErrorCode::E2003
            | ErrorCode::E2004
            | ErrorCode::E2005
            | ErrorCode::E2006
            | ErrorCode::E2008
            | ErrorCode::E2010
            | ErrorCode::E2014
            | ErrorCode::E2018
            | ErrorCode::E2019
            | ErrorCode::E2020
            | ErrorCode::E2021
            | ErrorCode::E2022
            | ErrorCode::E2023
            | ErrorCode::E2024
            | ErrorCode::E2025
            | ErrorCode::E2026
            | ErrorCode::E2027
            | ErrorCode::E2028
            | ErrorCode::E2029
            | ErrorCode::E2030
            | ErrorCode::E2031
            | ErrorCode::E2032
            | ErrorCode::E2033
            | ErrorCode::E2034
            | ErrorCode::E2035
            | ErrorCode::E2036
            | ErrorCode::E2037
            | ErrorCode::E2038
            | ErrorCode::E2039
            | ErrorCode::E2040
            | ErrorCode::E2041
            | ErrorCode::E2043
            | ErrorCode::E2044
            | ErrorCode::E2046
            | ErrorCode::E2047
            | ErrorCode::E2048
            | ErrorCode::E2049
            | ErrorCode::E2050
            | ErrorCode::E2051
            | ErrorCode::E2052
            | ErrorCode::E2053
            | ErrorCode::E2054
            | ErrorCode::E3002
            | ErrorCode::E3003
            | ErrorCode::E3010
            | ErrorCode::E3011
            | ErrorCode::E4002
            | ErrorCode::E4003
            | ErrorCode::E4004
            | ErrorCode::E4005
            | ErrorCode::E5001
            | ErrorCode::E5002
            | ErrorCode::E5003
            | ErrorCode::E5004
            | ErrorCode::E5005
            | ErrorCode::E5006
            | ErrorCode::E5007
            | ErrorCode::E5008
            | ErrorCode::E5009
            | ErrorCode::E6001
            | ErrorCode::E6002
            | ErrorCode::E6003
            | ErrorCode::E6004
            | ErrorCode::E6005
            | ErrorCode::E6006
            | ErrorCode::E6010
            | ErrorCode::E6011
            | ErrorCode::E6012
            | ErrorCode::E6020
            | ErrorCode::E6021
            | ErrorCode::E6022
            | ErrorCode::E6023
            | ErrorCode::E6024
            | ErrorCode::E6025
            | ErrorCode::E6026
            | ErrorCode::E6027
            | ErrorCode::E6030
            | ErrorCode::E6031
            | ErrorCode::E6032
            | ErrorCode::E6040
            | ErrorCode::E6050
            | ErrorCode::E6051
            | ErrorCode::E6060
            | ErrorCode::E6070
            | ErrorCode::E6080
            | ErrorCode::E6099
            | ErrorCode::E9001
            | ErrorCode::E9002
            | ErrorCode::W1001
            | ErrorCode::W1002
            | ErrorCode::W2001 => ErrorCodeLifecycle::Emitted,
        }
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

// Display and FromStr
