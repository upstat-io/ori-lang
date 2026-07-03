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
    E0008, "Triple-equals (cross-language habit)";
    E0009, "Single-quote string (cross-language habit)";
    E0010, "Increment/decrement operator (cross-language habit)";
    E0011, "Unicode confusable character";
    E0012, "Detached doc comment (warning)";
    E0013, "Standalone backslash";
    E0014, "Decimal not representable as whole base units";
    E0015, "Reserved-future keyword used as identifier";
    E0860, "`break` with value in a void-typed loop";
    E0861, "`continue` with value in a non-collecting loop";
    E0911, "Floating-point duration/size literal not supported";
    E0932, "Invalid feature name";

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
    E1016, "Expected semicolon";
    E1017, "Missing `=` in typed lambda";
    E1018, "Untyped parameter in typed lambda";
    E1019, "Trait impl must be written `impl Type: Trait`";
    E1020, "Invalid assignment target";

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
    E2039, "Cannot assign to immutable binding";
    E2040, "Feature not yet supported";
    E2041, "Invalid #repr attribute";
    E2042, "Extern type passed as owned without #free annotation";
    E2043, "Conditional partial move not statically computable";
    E2044, "Pre-condition contract type must be bool";
    E2045, "Post-condition contract must be a lambda";
    E2046, "Post-condition contract cannot apply to void-returning function";
    E2047, "Pre-condition contract references unknown identifier";
    E2048, "`EDROP_PARTIAL_MOVE`: partial move on type implementing Drop";
    E2049, "`EVALUE_DROP_CONFLICT`: type marked Value cannot implement Drop";
    E2050, "Type does not support index assignment";
    E2051, "Cannot assign through a parameter binding";
    E2052, "`EOR_PATTERN_NAME_DIVERGENCE`: or-pattern alternatives bind different variable names";
    E2053, "`EOR_PATTERN_TYPE_DIVERGENCE`: or-pattern alternatives bind a name at different types";
    E2054, "`EUSE_AFTER_DROP_EARLY`: use of a binding after `drop_early` consumed it";

    // Pattern Errors (E3xxx)
    E3001, "Unknown pattern";
    E3002, "Invalid pattern arguments";
    E3003, "Pattern type error";

    // Semantic / Lint Errors (E3xxx — test coverage)
    E3010, "Function has no tests";
    E3011, "Test targets unknown function";

    // ARC Analysis Errors (E4xxx)
    E4001, "Unsupported expression in ARC IR lowering";
    E4002, "Unsupported pattern in ARC IR lowering";
    E4003, "ARC internal error";
    E4004, "FBIP enforcement violation";
    E4005, "Contract coherence violation";

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

/// Lifecycle state for a registered diagnostic code.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ErrorCodeLifecycle {
    /// A production compiler path constructs this code.
    Emitted,
    /// The code is intentionally stable but unreachable today.
    Reserved { rationale: &'static str },
    /// The code is intentionally retained pending a named bug or design item.
    Tracked {
        issue: &'static str,
        rationale: &'static str,
    },
    /// The code remains parseable for compatibility but should not be emitted.
    Retired { rationale: &'static str },
}

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

// Phase classification (derived from naming convention)

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
