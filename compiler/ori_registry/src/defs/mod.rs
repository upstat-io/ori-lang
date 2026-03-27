//! Builtin type definitions — one `pub static` per type.
//!
//! Each submodule exports a single `pub static TypeDef`.
//! When adding new builtin types, add a new file here and
//! update `BUILTIN_TYPES`.

// Shared parameter definitions used by multiple type def modules.
pub mod params;

// Primitives
mod bool;
mod byte;
mod char;
mod float;
mod int;

// String
mod str;

// Compound types
mod duration;
mod error;
mod ordering;
mod size;

// Collections & wrappers
mod channel;
mod list;
mod map;
mod option;
mod range;
mod result;
mod set;
mod tuple;

// Iterators
mod iterator;

pub use self::bool::BOOL;
pub use self::byte::BYTE;
pub use self::channel::CHANNEL;
pub use self::char::CHAR;
pub use self::duration::DURATION;
pub use self::error::ERROR;
pub use self::float::FLOAT;
pub use self::int::INT;
pub use self::list::LIST;
pub use self::map::MAP;
pub use self::option::OPTION;
pub use self::ordering::{ORDERING, ORDERING_VARIANTS};
pub use self::range::RANGE;
pub use self::result::RESULT;
pub use self::set::SET;
pub use self::size::SIZE;
pub use self::str::STR;
pub use self::tuple::TUPLE;

pub use self::iterator::ITERATOR;

/// All builtin type definitions in the registry.
///
/// This is the master list used by query functions. Adding a new type
/// requires adding its `&'static TypeDef` here.
///
/// Order matches `TypeTag` declaration order for predictable iteration.
///
// TODO: Enforce completeness at compile time via exhaustive match in a const fn.
pub static BUILTIN_TYPES: &[&crate::TypeDef] = &[
    // Sorted by TypeTag discriminant value (repr(u8))
    &INT,   // 0
    &FLOAT, // 1
    &BOOL,  // 2
    &CHAR,  // 3
    &BYTE,  // 4
    // Unit (5), Never (6) excluded — no methods
    &DURATION, // 7
    &SIZE,     // 8
    &ORDERING, // 9
    &STR,      // 10
    &ERROR,    // 11
    &LIST,     // 12
    &MAP,      // 13
    &SET,      // 14
    &RANGE,    // 15
    &TUPLE,    // 16
    &OPTION,   // 17
    &RESULT,   // 18
    &CHANNEL,  // 19
    // Function (20) excluded — no methods
    &ITERATOR, // 21
];

#[cfg(test)]
mod tests;
