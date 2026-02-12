//! Error codes for all compiler diagnostics.
//!
//! Each error code is a unique identifier (e.g., `E1001`) with the first digit
//! indicating the compiler phase. Used for `--explain` lookups and documentation.

use std::fmt;

/// Error codes for all compiler diagnostics.
///
/// Format: E#### where first digit indicates phase:
/// - E0xxx: Lexer errors
/// - E1xxx: Parser errors
/// - E2xxx: Type errors
/// - E3xxx: Pattern errors
/// - E4xxx: ARC analysis errors
/// - E5xxx: Codegen / LLVM errors
/// - E6xxx: Runtime / eval errors
/// - E9xxx: Internal compiler errors
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ErrorCode {
    // Lexer Errors (E0xxx)
    /// Unterminated string literal
    E0001,
    /// Invalid character in source
    E0002,
    /// Invalid number literal
    E0003,
    /// Unterminated character literal
    E0004,
    /// Invalid escape sequence
    E0005,
    /// Unterminated template literal
    E0006,
    /// Semicolon (cross-language habit)
    E0007,
    /// Triple-equals (cross-language habit)
    E0008,
    /// Single-quote string (cross-language habit)
    E0009,
    /// Increment/decrement operator (cross-language habit)
    E0010,
    /// Unicode confusable character
    E0011,
    /// Detached doc comment (warning)
    E0012,
    /// Standalone backslash
    E0013,
    /// Decimal not representable as whole base units
    E0014,
    /// Reserved-future keyword used as identifier
    E0015,
    /// Floating-point duration/size literal not supported
    E0911,

    // Parser Errors (E1xxx)
    /// Unexpected token
    E1001,
    /// Expected expression
    E1002,
    /// Unclosed delimiter
    E1003,
    /// Expected identifier
    E1004,
    /// Expected type
    E1005,
    /// Invalid function definition
    E1006,
    /// Missing function body
    E1007,
    /// Invalid pattern syntax
    E1008,
    /// Missing pattern argument
    E1009,
    /// Unknown pattern argument
    E1010,
    /// Multi-arg function call requires named arguments
    E1011,
    /// Invalid `function_seq` syntax
    E1012,
    /// `function_exp` requires named properties
    E1013,
    /// Reserved built-in function name
    E1014,
    /// Unsupported keyword (e.g., `return` is not valid in Ori)
    E1015,

    // Type Errors (E2xxx)
    /// Type mismatch
    E2001,
    /// Unknown type
    E2002,
    /// Unknown identifier
    E2003,
    /// Argument count mismatch
    E2004,
    /// Cannot infer type
    E2005,
    /// Duplicate definition
    E2006,
    /// Closure self-reference (closure cannot capture itself)
    E2007,
    /// Cyclic type definition
    E2008,
    /// Missing trait bound
    E2009,
    /// Coherence violation (conflicting implementations)
    E2010,
    /// Named arguments required
    E2011,
    /// Unknown capability (uses clause references non-existent trait)
    E2012,
    /// Provider does not implement capability trait
    E2013,
    /// Missing capability declaration (function uses capability without declaring it)
    E2014,
    /// Type parameter ordering violation (non-default after default)
    E2015,
    /// Missing type argument (no default available)
    E2016,
    /// Too many type arguments
    E2017,
    /// Missing associated type (impl missing required associated type)
    E2018,

    // Pattern Errors (E3xxx)
    /// Unknown pattern
    E3001,
    /// Invalid pattern arguments
    E3002,
    /// Pattern type error
    E3003,

    // ARC Analysis Errors (E4xxx)
    /// Unsupported expression in ARC IR lowering
    E4001,
    /// Unsupported pattern in ARC IR lowering
    E4002,
    /// ARC internal error (invariant violation)
    E4003,

    // Codegen / LLVM Errors (E5xxx)
    /// LLVM module verification failed (ICE)
    E5001,
    /// Optimization pipeline failed
    E5002,
    /// Object/assembly/bitcode emission failed
    E5003,
    /// Target not supported / target configuration failed
    E5004,
    /// Runtime library (`libori_rt.a`) not found
    E5005,
    /// Linker failed
    E5006,
    /// Debug info creation failed
    E5007,
    /// WASM-specific error
    E5008,
    /// Module target configuration failed
    E5009,

    // Runtime / Eval Errors (E6xxx)
    /// Division by zero
    E6001,
    /// Modulo by zero
    E6002,
    /// Integer overflow
    E6003,
    /// Type mismatch
    E6010,
    /// Invalid binary operator for type
    E6011,
    /// Binary type mismatch
    E6012,
    /// Undefined variable
    E6020,
    /// Undefined function
    E6021,
    /// Undefined constant
    E6022,
    /// Undefined field
    E6023,
    /// Undefined method
    E6024,
    /// Index out of bounds
    E6025,
    /// Key not found
    E6026,
    /// Immutable binding
    E6027,
    /// Arity mismatch
    E6030,
    /// Stack overflow (recursion limit)
    E6031,
    /// Not callable
    E6032,
    /// Non-exhaustive match
    E6040,
    /// Assertion failed
    E6050,
    /// Panic called
    E6051,
    /// Missing capability
    E6060,
    /// Const-eval budget exceeded
    E6070,
    /// Not implemented feature
    E6080,
    /// Custom runtime error
    E6099,

    // Internal Errors (E9xxx)
    /// Internal compiler error
    E9001,
    /// Too many errors
    E9002,

    // Parser Warnings (W1xxx)
    /// Detached doc comment
    W1001,
}

impl ErrorCode {
    /// Get the numeric code as a string (e.g., "E1001").
    pub fn as_str(&self) -> &'static str {
        match self {
            // Lexer
            ErrorCode::E0001 => "E0001",
            ErrorCode::E0002 => "E0002",
            ErrorCode::E0003 => "E0003",
            ErrorCode::E0004 => "E0004",
            ErrorCode::E0005 => "E0005",
            ErrorCode::E0006 => "E0006",
            ErrorCode::E0007 => "E0007",
            ErrorCode::E0008 => "E0008",
            ErrorCode::E0009 => "E0009",
            ErrorCode::E0010 => "E0010",
            ErrorCode::E0011 => "E0011",
            ErrorCode::E0012 => "E0012",
            ErrorCode::E0013 => "E0013",
            ErrorCode::E0014 => "E0014",
            ErrorCode::E0015 => "E0015",
            ErrorCode::E0911 => "E0911",
            // Parser
            ErrorCode::E1001 => "E1001",
            ErrorCode::E1002 => "E1002",
            ErrorCode::E1003 => "E1003",
            ErrorCode::E1004 => "E1004",
            ErrorCode::E1005 => "E1005",
            ErrorCode::E1006 => "E1006",
            ErrorCode::E1007 => "E1007",
            ErrorCode::E1008 => "E1008",
            ErrorCode::E1009 => "E1009",
            ErrorCode::E1010 => "E1010",
            ErrorCode::E1011 => "E1011",
            ErrorCode::E1012 => "E1012",
            ErrorCode::E1013 => "E1013",
            ErrorCode::E1014 => "E1014",
            ErrorCode::E1015 => "E1015",
            // Type
            ErrorCode::E2001 => "E2001",
            ErrorCode::E2002 => "E2002",
            ErrorCode::E2003 => "E2003",
            ErrorCode::E2004 => "E2004",
            ErrorCode::E2005 => "E2005",
            ErrorCode::E2006 => "E2006",
            ErrorCode::E2007 => "E2007",
            ErrorCode::E2008 => "E2008",
            ErrorCode::E2009 => "E2009",
            ErrorCode::E2010 => "E2010",
            ErrorCode::E2011 => "E2011",
            ErrorCode::E2012 => "E2012",
            ErrorCode::E2013 => "E2013",
            ErrorCode::E2014 => "E2014",
            ErrorCode::E2015 => "E2015",
            ErrorCode::E2016 => "E2016",
            ErrorCode::E2017 => "E2017",
            ErrorCode::E2018 => "E2018",
            // Pattern
            ErrorCode::E3001 => "E3001",
            ErrorCode::E3002 => "E3002",
            ErrorCode::E3003 => "E3003",
            // ARC
            ErrorCode::E4001 => "E4001",
            ErrorCode::E4002 => "E4002",
            ErrorCode::E4003 => "E4003",
            // Codegen / LLVM
            ErrorCode::E5001 => "E5001",
            ErrorCode::E5002 => "E5002",
            ErrorCode::E5003 => "E5003",
            ErrorCode::E5004 => "E5004",
            ErrorCode::E5005 => "E5005",
            ErrorCode::E5006 => "E5006",
            ErrorCode::E5007 => "E5007",
            ErrorCode::E5008 => "E5008",
            ErrorCode::E5009 => "E5009",
            // Runtime / Eval
            ErrorCode::E6001 => "E6001",
            ErrorCode::E6002 => "E6002",
            ErrorCode::E6003 => "E6003",
            ErrorCode::E6010 => "E6010",
            ErrorCode::E6011 => "E6011",
            ErrorCode::E6012 => "E6012",
            ErrorCode::E6020 => "E6020",
            ErrorCode::E6021 => "E6021",
            ErrorCode::E6022 => "E6022",
            ErrorCode::E6023 => "E6023",
            ErrorCode::E6024 => "E6024",
            ErrorCode::E6025 => "E6025",
            ErrorCode::E6026 => "E6026",
            ErrorCode::E6027 => "E6027",
            ErrorCode::E6030 => "E6030",
            ErrorCode::E6031 => "E6031",
            ErrorCode::E6032 => "E6032",
            ErrorCode::E6040 => "E6040",
            ErrorCode::E6050 => "E6050",
            ErrorCode::E6051 => "E6051",
            ErrorCode::E6060 => "E6060",
            ErrorCode::E6070 => "E6070",
            ErrorCode::E6080 => "E6080",
            ErrorCode::E6099 => "E6099",
            // Internal
            ErrorCode::E9001 => "E9001",
            ErrorCode::E9002 => "E9002",
            // Warnings
            ErrorCode::W1001 => "W1001",
        }
    }

    /// Check if this is a lexer error (E0xxx range).
    pub fn is_lexer_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::E0001
                | ErrorCode::E0002
                | ErrorCode::E0003
                | ErrorCode::E0004
                | ErrorCode::E0005
                | ErrorCode::E0006
                | ErrorCode::E0007
                | ErrorCode::E0008
                | ErrorCode::E0009
                | ErrorCode::E0010
                | ErrorCode::E0011
                | ErrorCode::E0012
                | ErrorCode::E0013
                | ErrorCode::E0014
                | ErrorCode::E0015
                | ErrorCode::E0911
        )
    }

    /// Check if this is a parser/syntax error (E1xxx range).
    pub fn is_parser_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::E1001
                | ErrorCode::E1002
                | ErrorCode::E1003
                | ErrorCode::E1004
                | ErrorCode::E1005
                | ErrorCode::E1006
                | ErrorCode::E1007
                | ErrorCode::E1008
                | ErrorCode::E1009
                | ErrorCode::E1010
                | ErrorCode::E1011
                | ErrorCode::E1012
                | ErrorCode::E1013
                | ErrorCode::E1014
                | ErrorCode::E1015
        )
    }

    /// Check if this is a type error (E2xxx range).
    pub fn is_type_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::E2001
                | ErrorCode::E2002
                | ErrorCode::E2003
                | ErrorCode::E2004
                | ErrorCode::E2005
                | ErrorCode::E2006
                | ErrorCode::E2007
                | ErrorCode::E2008
                | ErrorCode::E2009
                | ErrorCode::E2010
                | ErrorCode::E2011
                | ErrorCode::E2012
                | ErrorCode::E2013
                | ErrorCode::E2014
                | ErrorCode::E2015
                | ErrorCode::E2016
                | ErrorCode::E2017
                | ErrorCode::E2018
        )
    }

    /// Check if this is a pattern error (E3xxx range).
    pub fn is_pattern_error(&self) -> bool {
        matches!(self, ErrorCode::E3001 | ErrorCode::E3002 | ErrorCode::E3003)
    }

    /// Check if this is an ARC analysis error (E4xxx range).
    pub fn is_arc_error(&self) -> bool {
        matches!(self, ErrorCode::E4001 | ErrorCode::E4002 | ErrorCode::E4003)
    }

    /// Check if this is a codegen/LLVM error (E5xxx range).
    pub fn is_codegen_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::E5001
                | ErrorCode::E5002
                | ErrorCode::E5003
                | ErrorCode::E5004
                | ErrorCode::E5005
                | ErrorCode::E5006
                | ErrorCode::E5007
                | ErrorCode::E5008
                | ErrorCode::E5009
        )
    }

    /// Check if this is a runtime/eval error (E6xxx range).
    pub fn is_eval_error(&self) -> bool {
        matches!(
            self,
            ErrorCode::E6001
                | ErrorCode::E6002
                | ErrorCode::E6003
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
        )
    }

    /// Check if this is an internal compiler error (E9xxx range).
    pub fn is_internal_error(&self) -> bool {
        matches!(self, ErrorCode::E9001 | ErrorCode::E9002)
    }

    /// Check if this is a warning code (Wxxx range).
    pub fn is_warning(&self) -> bool {
        matches!(self, ErrorCode::W1001)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_display() {
        assert_eq!(ErrorCode::E1001.to_string(), "E1001");
        assert_eq!(ErrorCode::E2001.as_str(), "E2001");
    }

    #[test]
    fn test_arc_error_codes() {
        assert_eq!(ErrorCode::E4001.as_str(), "E4001");
        assert_eq!(ErrorCode::E4002.as_str(), "E4002");
        assert_eq!(ErrorCode::E4003.as_str(), "E4003");

        assert!(ErrorCode::E4001.is_arc_error());
        assert!(ErrorCode::E4002.is_arc_error());
        assert!(ErrorCode::E4003.is_arc_error());

        assert!(!ErrorCode::E4001.is_codegen_error());
        assert!(!ErrorCode::E4001.is_parser_error());
        assert!(!ErrorCode::E4001.is_warning());
    }

    #[test]
    fn test_codegen_error_codes() {
        assert_eq!(ErrorCode::E5001.as_str(), "E5001");
        assert_eq!(ErrorCode::E5005.as_str(), "E5005");
        assert_eq!(ErrorCode::E5009.as_str(), "E5009");

        assert!(ErrorCode::E5001.is_codegen_error());
        assert!(ErrorCode::E5006.is_codegen_error());
        assert!(ErrorCode::E5009.is_codegen_error());

        assert!(!ErrorCode::E5001.is_arc_error());
        assert!(!ErrorCode::E5001.is_parser_error());
        assert!(!ErrorCode::E5001.is_warning());
    }

    #[test]
    fn test_eval_error_codes() {
        assert_eq!(ErrorCode::E6001.as_str(), "E6001");
        assert_eq!(ErrorCode::E6020.as_str(), "E6020");
        assert_eq!(ErrorCode::E6099.as_str(), "E6099");

        assert!(ErrorCode::E6001.is_eval_error());
        assert!(ErrorCode::E6031.is_eval_error());
        assert!(ErrorCode::E6099.is_eval_error());

        assert!(!ErrorCode::E6001.is_arc_error());
        assert!(!ErrorCode::E6001.is_codegen_error());
        assert!(!ErrorCode::E6001.is_parser_error());
        assert!(!ErrorCode::E6001.is_warning());
    }

    #[test]
    fn test_predicate_exclusivity() {
        // Ensure predicates don't overlap
        let all_codes = [
            ErrorCode::E0001,
            ErrorCode::E1001,
            ErrorCode::E2001,
            ErrorCode::E3001,
            ErrorCode::E4001,
            ErrorCode::E5001,
            ErrorCode::E6001,
            ErrorCode::E9001,
            ErrorCode::W1001,
        ];

        for code in &all_codes {
            let flags = [
                code.is_lexer_error(),
                code.is_parser_error(),
                code.is_type_error(),
                code.is_pattern_error(),
                code.is_arc_error(),
                code.is_codegen_error(),
                code.is_eval_error(),
                code.is_internal_error(),
                code.is_warning(),
            ];
            // Exactly one predicate should be true for every code
            let true_count = flags.iter().filter(|&&f| f).count();
            assert_eq!(
                true_count, 1,
                "expected exactly 1 predicate true for {code}, got {true_count}"
            );
        }
    }

    #[test]
    fn test_new_predicates() {
        assert!(ErrorCode::E0001.is_lexer_error());
        assert!(ErrorCode::E0911.is_lexer_error());
        assert!(!ErrorCode::E0001.is_parser_error());

        assert!(ErrorCode::E2001.is_type_error());
        assert!(ErrorCode::E2018.is_type_error());
        assert!(!ErrorCode::E2001.is_parser_error());

        assert!(ErrorCode::E3001.is_pattern_error());
        assert!(!ErrorCode::E3001.is_type_error());

        assert!(ErrorCode::E9001.is_internal_error());
        assert!(ErrorCode::E9002.is_internal_error());
        assert!(!ErrorCode::E9001.is_eval_error());
    }
}
