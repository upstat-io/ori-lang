//! Pattern system for Sigil V2 compiler.
//!
//! Patterns are first-class constructs in Sigil that provide declarative
//! syntax for common operations like mapping, filtering, folding, etc.
//!
//! ## Architecture
//!
//! Each pattern is a self-contained module implementing the `PatternDefinition`
//! trait, which provides:
//! - Type checking via `infer_type`
//! - Runtime evaluation via `evaluate`
//! - Documentation and examples
//!
//! ## Built-in Patterns
//!
//! | Pattern    | Description                              |
//! |------------|------------------------------------------|
//! | `run`      | Sequential execution block               |
//! | `try`      | Error handling with fallback             |
//! | `match`    | Pattern matching on values               |
//! | `map`      | Transform each element of a collection   |
//! | `filter`   | Select elements matching a predicate     |
//! | `fold`     | Reduce a collection to a single value    |
//! | `find`     | Find first matching element              |
//! | `collect`  | Build a collection from a range          |
//! | `recurse`  | Recursive computation with memoization   |
//! | `parallel` | Concurrent execution of tasks            |
//! | `timeout`  | Operation with time limit                |
//! | `retry`    | Retry with backoff strategy              |
//! | `cache`    | Memoized computation                     |
//! | `validate` | Validation with error accumulation       |

mod definition;
mod param;
mod registry;
mod fusion;
pub mod builtins;

pub use definition::PatternDefinition;
pub use param::{ParamSpec, TypeConstraint};
pub use registry::PatternRegistry;
pub use fusion::{FusedPattern, FusionAnalyzer, FusionHints};

use crate::intern::TypeId;
use crate::syntax::PatternKind;

/// Signature for pattern template lookup and caching.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PatternSignature {
    /// The pattern kind.
    pub kind: PatternKind,
    /// Types of the arguments (for monomorphization).
    pub arg_types: Vec<TypeId>,
}

impl PatternSignature {
    /// Create a new pattern signature.
    pub fn new(kind: PatternKind, arg_types: Vec<TypeId>) -> Self {
        PatternSignature { kind, arg_types }
    }
}
