//! The `cache` pattern - memoized computation.
//!
//! ```sigil
//! cache(.key: request.url, .compute: () -> fetch(request))
//! ```

use crate::patterns::definition::PatternDefinition;
use crate::patterns::param::{ParamSpec, TypeConstraint};

/// Memoized computation with explicit cache key.
pub struct CachePattern;

static CACHE_PARAMS: &[ParamSpec] = &[
    ParamSpec::required("key", "cache key"),
    ParamSpec::required_with(
        "compute",
        "function to compute value if not cached",
        TypeConstraint::FunctionArity(0),
    ),
    ParamSpec::optional_with("ttl", "time-to-live for cached value", TypeConstraint::Duration),
];

impl PatternDefinition for CachePattern {
    fn keyword(&self) -> &'static str {
        "cache"
    }

    fn params(&self) -> &'static [ParamSpec] {
        CACHE_PARAMS
    }

    fn description(&self) -> &'static str {
        "Memoized computation with explicit cache key"
    }

    fn help(&self) -> &'static str {
        r#"The `cache` pattern provides explicit caching with a specified key.
If the key is in cache, the cached value is returned.
Otherwise, the compute function is called and its result is cached.

Type signature: cache(.key: K, .compute: () -> V) -> V"#
    }

    fn examples(&self) -> &'static [&'static str] {
        &[
            "cache(.key: user_id, .compute: () -> fetch_user(user_id))",
            "cache(.key: url, .compute: () -> fetch(url), .ttl: 5m)",
        ]
    }
}
