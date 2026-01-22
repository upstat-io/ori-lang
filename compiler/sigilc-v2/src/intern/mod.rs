//! String and type interning for efficient memory usage and fast comparison.

mod strings;
mod types;

pub use strings::{Name, StringInterner};
pub use types::{TypeId, TypeInterner, TypeKind, TypeRange};

// Re-export for benchmarks
pub use strings::Name as InternedName;
