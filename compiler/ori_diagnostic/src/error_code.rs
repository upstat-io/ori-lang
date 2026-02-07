use std::fmt;

/// Error codes for all compiler diagnostics.
///
/// Format: E#### where first digit indicates phase:
/// - E0xxx: Lexer errors
/// - E1xxx: Parser errors
/// - E2xxx: Type errors
/// - E3xxx: Pattern errors
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
    /// Check if this is a parser/syntax error (E1xxx range).
    pub fn is_parser_error(&self) -> bool {
        self.as_str().starts_with("E1")
    }

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
            // Internal
            ErrorCode::E9001 => "E9001",
            ErrorCode::E9002 => "E9002",
            // Warnings
            ErrorCode::W1001 => "W1001",
        }
    }

    /// Check if this is a warning code (Wxxx range).
    pub fn is_warning(&self) -> bool {
        self.as_str().starts_with('W')
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
}
