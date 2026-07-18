//! Text conversion for diagnostic codes.

use std::fmt;

use super::ErrorCode;

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
