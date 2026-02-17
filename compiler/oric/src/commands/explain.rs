//! The `explain` command: display documentation for compiler error codes.

use ori_diagnostic::{ErrorCode, ErrorDocs};

/// Display detailed documentation for a given error code string.
pub fn explain_error(code_str: &str) {
    let Some(code) = code_str.parse::<ErrorCode>().ok() else {
        eprintln!("Unknown error code: {code_str}");
        eprintln!();
        eprintln!("Codes have the format EXXXX (errors) or WXXXX (warnings) where X is a digit.");
        eprintln!("Examples: E0001, E1001, E2001, W2001");
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
