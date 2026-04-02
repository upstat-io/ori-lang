//! Named constants for enum tag discriminants and struct field indices.
//!
//! Canonical definitions consumed by all compiler crates. Replaces bare `0`/`1`
//! magic numbers that previously appeared as inline literals with comments.
//!
//! # Option Tag Convention
//!
//! `Some` = variant 0, `None` = variant 1. This matches the declaration order
//! in the prelude (`type Option<T> = Some(T) | None`).
//!
//! # Result Tag Convention
//!
//! `Ok` = variant 0, `Err` = variant 1. This matches the declaration order
//! in the prelude (`type Result<T, E> = Ok(T) | Err(E)`).

// Option discriminant tags
/// `Option::Some` tag value — first variant.
pub const OPTION_TAG_SOME: i64 = 0;
/// `Option::None` tag value — second variant.
pub const OPTION_TAG_NONE: i64 = 1;

// Result discriminant tags
/// `Result::Ok` tag value — first variant.
pub const RESULT_TAG_OK: i64 = 0;
/// `Result::Err` tag value — second variant.
pub const RESULT_TAG_ERR: i64 = 1;

// Collection field indices (list, str, map share the `{ len, cap, data }` layout)
/// Index of the `len` field in list/str/map structs.
pub const FIELD_LEN: u32 = 0;
/// Index of the `cap` field in list/str/map structs.
pub const FIELD_CAP: u32 = 1;
/// Index of the `data` field in list/str/map structs.
pub const FIELD_DATA: u32 = 2;

// Closure layout: `{ ptr fn_ptr, ptr env_ptr }`
/// Index of the function pointer in a closure struct.
pub const CLOSURE_FIELD_FN: u32 = 0;
/// Index of the environment pointer in a closure struct.
pub const CLOSURE_FIELD_ENV: u32 = 1;

// Range layout: `{ i64 start, i64 end, ... }`
/// Index of the `start` field in a range struct.
pub const RANGE_FIELD_START: u32 = 0;
/// Index of the `end` field in a range struct.
pub const RANGE_FIELD_END: u32 = 1;

// Layout computation primitives

/// Minimum number of bytes needed to represent `variant_count` enum variants.
///
/// Returns the smallest power-of-two byte count that can hold all discriminants:
/// - 0 or 1 variants → 1 byte (i8)
/// - 2–256 variants → 1 byte (i8)
/// - 257–65536 → 2 bytes (i16)
/// - 65537–4294967296 → 4 bytes (i32)
/// - larger → 8 bytes (i64)
///
/// Shared between `ori_arc` (ARC IR lowering) and `ori_repr` (layout optimization).
#[must_use]
pub const fn min_tag_bytes(variant_count: usize) -> i64 {
    match variant_count {
        0 | 1 => 1,
        _ => {
            let bits_needed = usize::BITS - ((variant_count - 1).leading_zeros());
            match bits_needed {
                0..=8 => 1,
                9..=16 => 2,
                17..=32 => 4,
                _ => 8,
            }
        }
    }
}

// u32 variants for ARC IR CtorKind::EnumVariant (variant field is u32)
/// `Option::Some` variant index in ARC IR `CtorKind`.
pub const OPTION_VARIANT_SOME: u32 = 0;
/// `Option::None` variant index in ARC IR `CtorKind`.
pub const OPTION_VARIANT_NONE: u32 = 1;
/// `Result::Ok` variant index in ARC IR `CtorKind`.
pub const RESULT_VARIANT_OK: u32 = 0;
/// `Result::Err` variant index in ARC IR `CtorKind`.
pub const RESULT_VARIANT_ERR: u32 = 1;

// Cross-type const assertions: u32 variants match i64 tags
const _: () = assert!(OPTION_VARIANT_SOME as i64 == OPTION_TAG_SOME);
const _: () = assert!(OPTION_VARIANT_NONE as i64 == OPTION_TAG_NONE);
const _: () = assert!(RESULT_VARIANT_OK as i64 == RESULT_TAG_OK);
const _: () = assert!(RESULT_VARIANT_ERR as i64 == RESULT_TAG_ERR);

// Const assertions — these fail at compile time if the values change
const _: () = assert!(OPTION_TAG_SOME == 0);
const _: () = assert!(OPTION_TAG_NONE == 1);
const _: () = assert!(RESULT_TAG_OK == 0);
const _: () = assert!(RESULT_TAG_ERR == 1);
const _: () = assert!(FIELD_LEN == 0);
const _: () = assert!(FIELD_CAP == 1);
const _: () = assert!(FIELD_DATA == 2);
