//! Argument ownership annotation for ARC IR call sites.
//!
//! Populates `arg_ownership` on `Apply`/`Invoke` instructions so that
//! AIMS realization and every physical consumer can read per-argument
//! ownership directly from the shared IR without re-deriving policy.

mod annotate;
pub(crate) mod closure_resolve;

pub(crate) use self::annotate::annotate_arg_ownership;

#[cfg(test)]
mod tests;
