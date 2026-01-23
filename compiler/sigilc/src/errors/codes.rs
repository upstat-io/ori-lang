// Error codes for the Sigil compiler
//
// Error codes follow a structured format:
// - E0xxx: General/unknown errors
// - E1xxx: Lexer errors
// - E2xxx: Parser errors
// - E3xxx: Type checker errors
// - E4xxx: Evaluator errors
// - E5xxx: Codegen errors

/// Structured error codes for categorization and documentation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ErrorCode {
    // =====================================================================
    // General errors (E0xxx)
    // =====================================================================
    /// Unknown/uncategorized error (for migration from string errors)
    E0000 = 0,

    // =====================================================================
    // Lexer errors (E1xxx)
    // =====================================================================
    /// Unexpected character in input
    E1001 = 1001,
    /// Unterminated string literal
    E1002 = 1002,
    /// Invalid number literal
    E1003 = 1003,
    /// Invalid escape sequence in string
    E1004 = 1004,
    /// Unexpected end of input
    E1005 = 1005,

    // =====================================================================
    // Parser errors (E2xxx)
    // =====================================================================
    /// Unexpected token
    E2001 = 2001,
    /// Expected specific token
    E2002 = 2002,
    /// Invalid function definition
    E2003 = 2003,
    /// Invalid type expression
    E2004 = 2004,
    /// Invalid pattern syntax
    E2005 = 2005,
    /// Missing required element
    E2006 = 2006,
    /// Invalid expression
    E2007 = 2007,
    /// Invalid struct/enum definition
    E2008 = 2008,
    /// Invalid import statement
    E2009 = 2009,
    /// Unexpected end of file
    E2010 = 2010,

    // =====================================================================
    // Type checker errors (E3xxx)
    // =====================================================================
    /// Type mismatch
    E3001 = 3001,
    /// Unknown identifier
    E3002 = 3002,
    /// Unknown type
    E3003 = 3003,
    /// Wrong number of arguments
    E3004 = 3004,
    /// Cannot infer type
    E3005 = 3005,
    /// Invalid operation for type
    E3006 = 3006,
    /// Missing function test
    E3007 = 3007,
    /// Unknown method
    E3008 = 3008,
    /// Invalid pattern usage
    E3009 = 3009,
    /// Duplicate definition
    E3010 = 3010,
    /// Unknown function being tested
    E3011 = 3011,

    // =====================================================================
    // Evaluator errors (E4xxx)
    // =====================================================================
    /// Division by zero
    E4001 = 4001,
    /// Index out of bounds
    E4002 = 4002,
    /// Assertion failed
    E4003 = 4003,
    /// Runtime type error
    E4004 = 4004,
    /// Stack overflow
    E4005 = 4005,
    /// Invalid argument
    E4006 = 4006,
    /// Pattern evaluation error
    E4007 = 4007,

    // =====================================================================
    // Codegen errors (E5xxx)
    // =====================================================================
    /// Unsupported feature in codegen
    E5001 = 5001,
    /// Invalid C type conversion
    E5002 = 5002,
    /// Codegen internal error
    E5003 = 5003,

    // =====================================================================
    // ARC memory management errors (E6xxx)
    // =====================================================================
    /// Self-referential type (direct cycle)
    E6001 = 6001,
    /// Mutually referential types (indirect cycle)
    E6002 = 6002,
    /// Closure captures containing type
    E6003 = 6003,
}

impl ErrorCode {
    /// Get the numeric code
    pub fn code(&self) -> u16 {
        *self as u16
    }

    /// Get a formatted error code string like "E0001"
    pub fn as_string(&self) -> String {
        format!("E{:04}", self.code())
    }

    /// Get a brief description of the error code
    pub fn description(&self) -> &'static str {
        match self {
            // General
            ErrorCode::E0000 => "unknown error",

            // Lexer
            ErrorCode::E1001 => "unexpected character",
            ErrorCode::E1002 => "unterminated string literal",
            ErrorCode::E1003 => "invalid number literal",
            ErrorCode::E1004 => "invalid escape sequence",
            ErrorCode::E1005 => "unexpected end of input",

            // Parser
            ErrorCode::E2001 => "unexpected token",
            ErrorCode::E2002 => "expected specific token",
            ErrorCode::E2003 => "invalid function definition",
            ErrorCode::E2004 => "invalid type expression",
            ErrorCode::E2005 => "invalid pattern syntax",
            ErrorCode::E2006 => "missing required element",
            ErrorCode::E2007 => "invalid expression",
            ErrorCode::E2008 => "invalid struct/enum definition",
            ErrorCode::E2009 => "invalid import statement",
            ErrorCode::E2010 => "unexpected end of file",

            // Type checker
            ErrorCode::E3001 => "type mismatch",
            ErrorCode::E3002 => "unknown identifier",
            ErrorCode::E3003 => "unknown type",
            ErrorCode::E3004 => "wrong number of arguments",
            ErrorCode::E3005 => "cannot infer type",
            ErrorCode::E3006 => "invalid operation for type",
            ErrorCode::E3007 => "missing function test",
            ErrorCode::E3008 => "unknown method",
            ErrorCode::E3009 => "invalid pattern usage",
            ErrorCode::E3010 => "duplicate definition",
            ErrorCode::E3011 => "unknown function being tested",

            // Evaluator
            ErrorCode::E4001 => "division by zero",
            ErrorCode::E4002 => "index out of bounds",
            ErrorCode::E4003 => "assertion failed",
            ErrorCode::E4004 => "runtime type error",
            ErrorCode::E4005 => "stack overflow",
            ErrorCode::E4006 => "invalid argument",
            ErrorCode::E4007 => "pattern evaluation error",

            // Codegen
            ErrorCode::E5001 => "unsupported feature in code generation",
            ErrorCode::E5002 => "invalid C type conversion",
            ErrorCode::E5003 => "code generation internal error",

            // ARC
            ErrorCode::E6001 => "self-referential type (cyclic type definition)",
            ErrorCode::E6002 => "mutually referential types (indirect cycle)",
            ErrorCode::E6003 => "closure captures containing type",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_format() {
        assert_eq!(ErrorCode::E0000.as_string(), "E0000");
        assert_eq!(ErrorCode::E1001.as_string(), "E1001");
        assert_eq!(ErrorCode::E3001.as_string(), "E3001");
    }

    #[test]
    fn test_error_code_numeric() {
        assert_eq!(ErrorCode::E0000.code(), 0);
        assert_eq!(ErrorCode::E1001.code(), 1001);
        assert_eq!(ErrorCode::E3001.code(), 3001);
    }

    #[test]
    fn test_error_code_description() {
        assert!(!ErrorCode::E3001.description().is_empty());
        assert!(ErrorCode::E3001.description().contains("mismatch"));
    }

    #[test]
    fn test_error_code_display() {
        assert_eq!(format!("{}", ErrorCode::E3001), "E3001");
    }
}
