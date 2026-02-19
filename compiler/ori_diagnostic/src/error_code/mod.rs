//! Error codes for all compiler diagnostics.
//!
//! Each error code is a unique identifier (e.g., `E1001`) with the first digit
//! indicating the compiler phase. Used for `--explain` lookups and documentation.
//!
//! All error codes are declared in a single [`define_error_codes!`] invocation.
//! The macro generates: the `ErrorCode` enum, `ALL`, `COUNT`, `as_str()`,
//! `description()`, `Display`, and `FromStr`.

use std::fmt;

/// Declare all error codes in a single location.
///
/// Each entry is `$variant, $description` where:
/// - `$variant` is the enum variant name (e.g., `E2001`, `W1001`)
/// - `$description` is a one-line summary string
///
/// Generates:
/// - `ErrorCode` enum with doc comments from descriptions
/// - `ALL: &[ErrorCode]` — all variants for iteration
/// - `COUNT: usize` — variant count
/// - `as_str()` — variant name as `&'static str` (e.g., `"E2001"`)
/// - `description()` — one-line summary
macro_rules! define_error_codes {
    ($( $variant:ident, $desc:literal );+ $(;)?) => {
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
        /// - W1xxx: Parser warnings
        /// - W2xxx: Type checker warnings
        #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
        pub enum ErrorCode {
            $(
                #[doc = $desc]
                $variant,
            )+
        }

        impl ErrorCode {
            /// All error code variants, for exhaustive iteration and testing.
            pub const ALL: &[ErrorCode] = &[ $( ErrorCode::$variant, )+ ];

            /// Number of error code variants.
            pub const COUNT: usize = [ $( ErrorCode::$variant, )+ ].len();

            /// Get the code as a string (e.g., `"E1001"`, `"W2001"`).
            pub fn as_str(&self) -> &'static str {
                match self {
                    $( ErrorCode::$variant => stringify!($variant), )+
                }
            }

            /// Get the one-line description of this error code.
            pub fn description(&self) -> &'static str {
                match self {
                    $( ErrorCode::$variant => $desc, )+
                }
            }
        }
    };
}

define_error_codes! {
    // Lexer Errors (E0xxx)
    E0001, "Unterminated string literal";
    E0002, "Invalid character in source";
    E0003, "Invalid number literal";
    E0004, "Unterminated character literal";
    E0005, "Invalid escape sequence";
    E0006, "Unterminated template literal";
    E0007, "Semicolon (cross-language habit)";
    E0008, "Triple-equals (cross-language habit)";
    E0009, "Single-quote string (cross-language habit)";
    E0010, "Increment/decrement operator (cross-language habit)";
    E0011, "Unicode confusable character";
    E0012, "Detached doc comment (warning)";
    E0013, "Standalone backslash";
    E0014, "Decimal not representable as whole base units";
    E0015, "Reserved-future keyword used as identifier";
    E0911, "Floating-point duration/size literal not supported";

    // Parser Errors (E1xxx)
    E1001, "Unexpected token";
    E1002, "Expected expression";
    E1003, "Unclosed delimiter";
    E1004, "Expected identifier";
    E1005, "Expected type";
    E1006, "Invalid function definition";
    E1007, "Missing function body";
    E1008, "Invalid pattern syntax";
    E1009, "Missing pattern argument";
    E1010, "Unknown pattern argument";
    E1011, "Multi-arg function call requires named arguments";
    E1012, "Invalid `function_seq` syntax";
    E1013, "`function_exp` requires named properties";
    E1014, "Reserved built-in function name";
    E1015, "Unsupported keyword";

    // Type Errors (E2xxx)
    E2001, "Type mismatch";
    E2002, "Unknown type";
    E2003, "Unknown identifier";
    E2004, "Argument count mismatch";
    E2005, "Cannot infer type";
    E2006, "Duplicate definition";
    E2007, "Closure self-reference";
    E2008, "Cyclic type definition";
    E2009, "Missing trait bound";
    E2010, "Coherence violation";
    E2011, "Named arguments required";
    E2012, "Unknown capability";
    E2013, "Provider does not implement capability trait";
    E2014, "Missing capability declaration";
    E2015, "Type parameter ordering violation";
    E2016, "Missing type argument";
    E2017, "Too many type arguments";
    E2018, "Missing associated type";
    E2019, "Never type used as struct field";
    E2020, "Unsupported operator";
    E2021, "Overlapping implementations with equal specificity";
    E2022, "Conflicting default methods from multiple super-traits";
    E2023, "Ambiguous method call";
    E2024, "Trait is not object-safe";
    E2025, "Type does not implement Index";
    E2026, "Wrong key type for Index impl";
    E2027, "Ambiguous index key type";
    E2028, "Cannot derive Default for sum type";
    E2029, "Cannot derive Hashable without Eq";
    E2030, "Hashable implementation may violate hash invariant";
    E2031, "Type cannot be used as map key";
    E2032, "Field type does not implement trait required by derive";
    E2033, "Trait cannot be derived";
    E2034, "Invalid format specification in template string";
    E2035, "Format type not supported for expression type";
    E2036, "Type does not implement Into<T>";
    E2037, "Multiple Into implementations apply";
    E2038, "Type does not implement Printable";

    // Pattern Errors (E3xxx)
    E3001, "Unknown pattern";
    E3002, "Invalid pattern arguments";
    E3003, "Pattern type error";

    // ARC Analysis Errors (E4xxx)
    E4001, "Unsupported expression in ARC IR lowering";
    E4002, "Unsupported pattern in ARC IR lowering";
    E4003, "ARC internal error";

    // Codegen / LLVM Errors (E5xxx)
    E5001, "LLVM module verification failed";
    E5002, "Optimization pipeline failed";
    E5003, "Object/assembly/bitcode emission failed";
    E5004, "Target not supported";
    E5005, "Runtime library not found";
    E5006, "Linker failed";
    E5007, "Debug info creation failed";
    E5008, "WASM-specific error";
    E5009, "Module target configuration failed";

    // Runtime / Eval Errors (E6xxx)
    E6001, "Division by zero";
    E6002, "Modulo by zero";
    E6003, "Integer overflow";
    E6004, "Size subtraction would be negative";
    E6005, "Size multiply by negative";
    E6006, "Size divide by negative";
    E6010, "Type mismatch (runtime)";
    E6011, "Invalid binary operator for type";
    E6012, "Binary type mismatch";
    E6020, "Undefined variable";
    E6021, "Undefined function";
    E6022, "Undefined constant";
    E6023, "Undefined field";
    E6024, "Undefined method";
    E6025, "Index out of bounds";
    E6026, "Key not found";
    E6027, "Immutable binding";
    E6030, "Arity mismatch";
    E6031, "Stack overflow";
    E6032, "Not callable";
    E6040, "Non-exhaustive match";
    E6050, "Assertion failed";
    E6051, "Panic called";
    E6060, "Missing capability (runtime)";
    E6070, "Const-eval budget exceeded";
    E6080, "Not implemented feature";
    E6099, "Custom runtime error";

    // Internal Errors (E9xxx)
    E9001, "Internal compiler error";
    E9002, "Too many errors";

    // Parser Warnings (W1xxx)
    W1001, "Detached doc comment";
    W1002, "Unknown calling convention in extern block";

    // Type Checker Warnings (W2xxx)
    W2001, "Infinite iterator consumed without bound";
}

// ---------------------------------------------------------------------------
// Phase classification (derived from naming convention)
// ---------------------------------------------------------------------------

impl ErrorCode {
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

// ---------------------------------------------------------------------------
// Display and FromStr
// ---------------------------------------------------------------------------

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Parse an error code string like `"E2001"` or `"W2001"`.
///
/// Case-insensitive. Derived from [`ErrorCode::ALL`] and [`ErrorCode::as_str()`],
/// so it is automatically exhaustive — no manual mirroring needed.
impl std::str::FromStr for ErrorCode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.to_uppercase();
        Self::ALL
            .iter()
            .find(|code| code.as_str() == upper)
            .copied()
            .ok_or(())
    }
}

#[cfg(test)]
mod tests;
