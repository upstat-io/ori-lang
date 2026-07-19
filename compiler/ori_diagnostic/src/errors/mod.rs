//! Embedded error documentation for `--explain` support.
//!
//! Each error code has a markdown documentation file that explains the error,
//! shows examples, and provides solutions. These are embedded at compile time
//! and can be accessed via `ErrorDocs::get()`.
//!
//! # Adding New Documentation
//!
//! 1. Create a new file `EXXXX.md` in this directory
//! 2. Add an entry to the `DOCS` array below
//! 3. Run `cargo build` to embed the new documentation

use std::collections::HashMap;
use std::sync::LazyLock;

use crate::ErrorCode;

/// Lazily-initialized `HashMap` for O(1) error documentation lookup.
static DOCS_MAP: LazyLock<HashMap<ErrorCode, &'static str>> =
    LazyLock::new(|| DOCS.iter().copied().collect());

/// Registry of embedded error documentation.
///
/// Use `ErrorDocs::get(code)` to retrieve the documentation for an error code.
pub struct ErrorDocs;

impl ErrorDocs {
    /// Get the documentation for an error code in O(1) time.
    ///
    /// Returns `Some(markdown)` if documentation exists for the code,
    /// `None` otherwise.
    ///
    /// # Example
    ///
    /// ```text
    /// if let Some(doc) = ErrorDocs::get(ErrorCode::E2001) {
    ///     println!("{}", doc);
    /// }
    /// ```
    pub fn get(code: ErrorCode) -> Option<&'static str> {
        DOCS_MAP.get(&code).copied()
    }

    /// Get all documented error codes.
    pub fn all_codes() -> impl Iterator<Item = ErrorCode> {
        DOCS.iter().map(|(code, _)| *code)
    }

    /// Check if an error code has documentation in O(1) time.
    pub fn has_docs(code: ErrorCode) -> bool {
        DOCS_MAP.contains_key(&code)
    }
}

/// Embedded documentation for each error code.
///
/// Add new entries here when creating new error documentation.
static DOCS: &[(ErrorCode, &str)] = &[
    // Lexer errors (E0xxx)
    (ErrorCode::E0001, include_str!("E0001.md")),
    (ErrorCode::E0002, include_str!("E0002.md")),
    (ErrorCode::E0003, include_str!("E0003.md")),
    (ErrorCode::E0004, include_str!("E0004.md")),
    (ErrorCode::E0005, include_str!("E0005.md")),
    (ErrorCode::E0006, include_str!("E0006.md")),
    (ErrorCode::E0008, include_str!("E0008.md")),
    (ErrorCode::E0009, include_str!("E0009.md")),
    (ErrorCode::E0010, include_str!("E0010.md")),
    (ErrorCode::E0011, include_str!("E0011.md")),
    (ErrorCode::E0012, include_str!("E0012.md")),
    (ErrorCode::E0013, include_str!("E0013.md")),
    (ErrorCode::E0014, include_str!("E0014.md")),
    (ErrorCode::E0015, include_str!("E0015.md")),
    (ErrorCode::E0860, include_str!("E0860.md")),
    (ErrorCode::E0861, include_str!("E0861.md")),
    (ErrorCode::E0911, include_str!("E0911.md")),
    (ErrorCode::E0932, include_str!("E0932.md")),
    // Parser errors (E1xxx)
    (ErrorCode::E1001, include_str!("E1001.md")),
    (ErrorCode::E1002, include_str!("E1002.md")),
    (ErrorCode::E1003, include_str!("E1003.md")),
    (ErrorCode::E1004, include_str!("E1004.md")),
    (ErrorCode::E1005, include_str!("E1005.md")),
    (ErrorCode::E1006, include_str!("E1006.md")),
    (ErrorCode::E1007, include_str!("E1007.md")),
    (ErrorCode::E1008, include_str!("E1008.md")),
    (ErrorCode::E1009, include_str!("E1009.md")),
    (ErrorCode::E1010, include_str!("E1010.md")),
    (ErrorCode::E1011, include_str!("E1011.md")),
    (ErrorCode::E1012, include_str!("E1012.md")),
    (ErrorCode::E1013, include_str!("E1013.md")),
    (ErrorCode::E1014, include_str!("E1014.md")),
    (ErrorCode::E1015, include_str!("E1015.md")),
    (ErrorCode::E1016, include_str!("E1016.md")),
    (ErrorCode::E1017, include_str!("E1017.md")),
    (ErrorCode::E1018, include_str!("E1018.md")),
    (ErrorCode::E1019, include_str!("E1019.md")),
    (ErrorCode::E1020, include_str!("E1020.md")),
    // Type errors (E2xxx)
    (ErrorCode::E2001, include_str!("E2001.md")),
    (ErrorCode::E2002, include_str!("E2002.md")),
    (ErrorCode::E2003, include_str!("E2003.md")),
    (ErrorCode::E2004, include_str!("E2004.md")),
    (ErrorCode::E2005, include_str!("E2005.md")),
    (ErrorCode::E2006, include_str!("E2006.md")),
    (ErrorCode::E2007, include_str!("E2007.md")),
    (ErrorCode::E2008, include_str!("E2008.md")),
    (ErrorCode::E2009, include_str!("E2009.md")),
    (ErrorCode::E2010, include_str!("E2010.md")),
    (ErrorCode::E2011, include_str!("E2011.md")),
    (ErrorCode::E2012, include_str!("E2012.md")),
    (ErrorCode::E2013, include_str!("E2013.md")),
    (ErrorCode::E2014, include_str!("E2014.md")),
    (ErrorCode::E2015, include_str!("E2015.md")),
    (ErrorCode::E2016, include_str!("E2016.md")),
    (ErrorCode::E2017, include_str!("E2017.md")),
    (ErrorCode::E2018, include_str!("E2018.md")),
    (ErrorCode::E2019, include_str!("E2019.md")),
    (ErrorCode::E2020, include_str!("E2020.md")),
    (ErrorCode::E2021, include_str!("E2021.md")),
    (ErrorCode::E2022, include_str!("E2022.md")),
    (ErrorCode::E2023, include_str!("E2023.md")),
    (ErrorCode::E2024, include_str!("E2024.md")),
    (ErrorCode::E2025, include_str!("E2025.md")),
    (ErrorCode::E2026, include_str!("E2026.md")),
    (ErrorCode::E2027, include_str!("E2027.md")),
    (ErrorCode::E2028, include_str!("E2028.md")),
    (ErrorCode::E2029, include_str!("E2029.md")),
    (ErrorCode::E2030, include_str!("E2030.md")),
    (ErrorCode::E2031, include_str!("E2031.md")),
    (ErrorCode::E2032, include_str!("E2032.md")),
    (ErrorCode::E2033, include_str!("E2033.md")),
    (ErrorCode::E2034, include_str!("E2034.md")),
    (ErrorCode::E2035, include_str!("E2035.md")),
    (ErrorCode::E2036, include_str!("E2036.md")),
    (ErrorCode::E2037, include_str!("E2037.md")),
    (ErrorCode::E2038, include_str!("E2038.md")),
    (ErrorCode::E2039, include_str!("E2039.md")),
    (ErrorCode::E2040, include_str!("E2040.md")),
    (ErrorCode::E2041, include_str!("E2041.md")),
    (ErrorCode::E2042, include_str!("E2042.md")),
    (ErrorCode::E2043, include_str!("E2043.md")),
    (ErrorCode::E2044, include_str!("E2044.md")),
    (ErrorCode::E2045, include_str!("E2045.md")),
    (ErrorCode::E2046, include_str!("E2046.md")),
    (ErrorCode::E2047, include_str!("E2047.md")),
    (ErrorCode::E2048, include_str!("E2048.md")),
    (ErrorCode::E2049, include_str!("E2049.md")),
    (ErrorCode::E2050, include_str!("E2050.md")),
    (ErrorCode::E2051, include_str!("E2051.md")),
    (ErrorCode::E2052, include_str!("E2052.md")),
    (ErrorCode::E2053, include_str!("E2053.md")),
    (ErrorCode::E2054, include_str!("E2054.md")),
    (ErrorCode::E2056, include_str!("E2056.md")),
    (ErrorCode::E2057, include_str!("E2057.md")),
    (ErrorCode::E2058, include_str!("E2058.md")),
    (ErrorCode::E2059, include_str!("E2059.md")),
    // Pattern errors (E3xxx)
    (ErrorCode::E3001, include_str!("E3001.md")),
    (ErrorCode::E3002, include_str!("E3002.md")),
    (ErrorCode::E3003, include_str!("E3003.md")),
    // Semantic / lint errors (E3xxx — test coverage)
    (ErrorCode::E3010, include_str!("E3010.md")),
    (ErrorCode::E3011, include_str!("E3011.md")),
    // ARC analysis errors (E4xxx)
    (ErrorCode::E4001, include_str!("E4001.md")),
    (ErrorCode::E4002, include_str!("E4002.md")),
    (ErrorCode::E4003, include_str!("E4003.md")),
    (ErrorCode::E4004, include_str!("E4004.md")),
    (ErrorCode::E4005, include_str!("E4005.md")),
    // Codegen / LLVM errors (E5xxx)
    (ErrorCode::E5001, include_str!("E5001.md")),
    (ErrorCode::E5002, include_str!("E5002.md")),
    (ErrorCode::E5003, include_str!("E5003.md")),
    (ErrorCode::E5004, include_str!("E5004.md")),
    (ErrorCode::E5005, include_str!("E5005.md")),
    (ErrorCode::E5006, include_str!("E5006.md")),
    (ErrorCode::E5007, include_str!("E5007.md")),
    (ErrorCode::E5008, include_str!("E5008.md")),
    (ErrorCode::E5009, include_str!("E5009.md")),
    // Runtime / evaluator errors (E6xxx)
    (ErrorCode::E6001, include_str!("E6001.md")),
    (ErrorCode::E6002, include_str!("E6002.md")),
    (ErrorCode::E6003, include_str!("E6003.md")),
    (ErrorCode::E6004, include_str!("E6004.md")),
    (ErrorCode::E6005, include_str!("E6005.md")),
    (ErrorCode::E6006, include_str!("E6006.md")),
    (ErrorCode::E6010, include_str!("E6010.md")),
    (ErrorCode::E6011, include_str!("E6011.md")),
    (ErrorCode::E6012, include_str!("E6012.md")),
    (ErrorCode::E6020, include_str!("E6020.md")),
    (ErrorCode::E6021, include_str!("E6021.md")),
    (ErrorCode::E6022, include_str!("E6022.md")),
    (ErrorCode::E6023, include_str!("E6023.md")),
    (ErrorCode::E6024, include_str!("E6024.md")),
    (ErrorCode::E6025, include_str!("E6025.md")),
    (ErrorCode::E6026, include_str!("E6026.md")),
    (ErrorCode::E6027, include_str!("E6027.md")),
    (ErrorCode::E6030, include_str!("E6030.md")),
    (ErrorCode::E6031, include_str!("E6031.md")),
    (ErrorCode::E6032, include_str!("E6032.md")),
    (ErrorCode::E6040, include_str!("E6040.md")),
    (ErrorCode::E6050, include_str!("E6050.md")),
    (ErrorCode::E6051, include_str!("E6051.md")),
    (ErrorCode::E6060, include_str!("E6060.md")),
    (ErrorCode::E6070, include_str!("E6070.md")),
    (ErrorCode::E6080, include_str!("E6080.md")),
    (ErrorCode::E6099, include_str!("E6099.md")),
    // Internal errors (E9xxx)
    (ErrorCode::E9001, include_str!("E9001.md")),
    (ErrorCode::E9002, include_str!("E9002.md")),
    // Parser warnings (W1xxx)
    (ErrorCode::W1001, include_str!("W1001.md")),
    (ErrorCode::W1002, include_str!("W1002.md")),
    // Type checker warnings (W2xxx)
    (ErrorCode::W2001, include_str!("W2001.md")),
];

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
