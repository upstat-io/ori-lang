//! The `explain` command: display documentation for compiler error codes.

use ori_diagnostic::{ErrorCode, ErrorDocs};

/// Display detailed documentation for a given error code string.
pub(crate) fn explain_error(code_str: &str) {
    let Some(code) = parse_error_code(code_str) else {
        eprintln!("Unknown error code: {code_str}");
        eprintln!();
        eprintln!("Error codes have the format EXXXX where X is a digit.");
        eprintln!("Examples: E0001, E1001, E2001");
        std::process::exit(1);
    };

    if let Some(doc) = ErrorDocs::get(code) {
        println!("{doc}");
    } else {
        eprintln!("No documentation available for {code_str}");
        eprintln!();
        eprintln!("This error code exists but doesn't have detailed documentation yet.");
        eprintln!("Please check the error message for guidance.");
        std::process::exit(1);
    }
}

/// Parse an error code string like "E2001" into an `ErrorCode` variant.
fn parse_error_code(s: &str) -> Option<ErrorCode> {
    match s.to_uppercase().as_str() {
        // Lexer errors
        "E0001" => Some(ErrorCode::E0001),
        "E0002" => Some(ErrorCode::E0002),
        "E0003" => Some(ErrorCode::E0003),
        "E0004" => Some(ErrorCode::E0004),
        "E0005" => Some(ErrorCode::E0005),
        // Parser errors
        "E1001" => Some(ErrorCode::E1001),
        "E1002" => Some(ErrorCode::E1002),
        "E1003" => Some(ErrorCode::E1003),
        "E1004" => Some(ErrorCode::E1004),
        "E1005" => Some(ErrorCode::E1005),
        "E1006" => Some(ErrorCode::E1006),
        "E1007" => Some(ErrorCode::E1007),
        "E1008" => Some(ErrorCode::E1008),
        "E1009" => Some(ErrorCode::E1009),
        "E1010" => Some(ErrorCode::E1010),
        "E1011" => Some(ErrorCode::E1011),
        "E1012" => Some(ErrorCode::E1012),
        "E1013" => Some(ErrorCode::E1013),
        "E1014" => Some(ErrorCode::E1014),
        // Type errors
        "E2001" => Some(ErrorCode::E2001),
        "E2002" => Some(ErrorCode::E2002),
        "E2003" => Some(ErrorCode::E2003),
        "E2004" => Some(ErrorCode::E2004),
        "E2005" => Some(ErrorCode::E2005),
        "E2006" => Some(ErrorCode::E2006),
        "E2007" => Some(ErrorCode::E2007),
        "E2008" => Some(ErrorCode::E2008),
        "E2009" => Some(ErrorCode::E2009),
        "E2010" => Some(ErrorCode::E2010),
        "E2011" => Some(ErrorCode::E2011),
        "E2012" => Some(ErrorCode::E2012),
        "E2013" => Some(ErrorCode::E2013),
        "E2014" => Some(ErrorCode::E2014),
        // Pattern errors
        "E3001" => Some(ErrorCode::E3001),
        "E3002" => Some(ErrorCode::E3002),
        "E3003" => Some(ErrorCode::E3003),
        // Internal errors
        "E9001" => Some(ErrorCode::E9001),
        "E9002" => Some(ErrorCode::E9002),
        _ => None,
    }
}
