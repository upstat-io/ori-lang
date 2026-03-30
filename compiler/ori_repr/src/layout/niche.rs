//! Niche analysis for enum representation optimization (§07.2).
//!
//! A "niche" is an invalid bit pattern in a type. If an enum variant's payload
//! has a niche, we can use it to encode a different variant, eliminating the
//! explicit tag.
//!
//! Examples:
//! - `bool` has 254 niches (values 2..=255)
//! - `Ordering` has 253 niches (values 3..=255)
//! - `char` has ~4 billion niches (0x110000..=0xFFFFFFFF)
//! - `RcPointer` has 1 niche (null, since heap pointers are always non-null)
//! - `str` has 1 niche (null data pointer, since SSO ensures non-zero)
//!
//! This module provides [`find_niches`] to discover available niches for a
//! given [`MachineRepr`], and [`optimize_option_repr`] / [`optimize_result_repr`]
//! to apply niche filling to Option/Result layouts.
