//! Collection method resolver.
//!
//! Resolves methods on collections (list, map, range) that require
//! evaluator access to call function arguments.

use super::{CollectionMethod, MethodResolution, MethodResolver, Value};

/// Resolver for collection methods that require evaluator access.
///
/// Priority 1 - collection methods are checked after user/derived methods.
///
/// These methods take function arguments and need evaluator access to call them:
/// - map, filter, fold, find on lists
/// - collect on ranges
/// - map, filter on maps
/// - any, all on lists
pub struct CollectionMethodResolver;

impl CollectionMethodResolver {
    /// Create a new collection method resolver.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CollectionMethodResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl MethodResolver for CollectionMethodResolver {
    fn resolve(&self, receiver: &Value, _type_name: &str, method_name: &str) -> MethodResolution {
        // Check if this is a collection type and the method is a known collection method
        match receiver {
            Value::List(_) => {
                match method_name {
                    "map" => MethodResolution::Collection(CollectionMethod::Map),
                    "filter" => MethodResolution::Collection(CollectionMethod::Filter),
                    "fold" => MethodResolution::Collection(CollectionMethod::Fold),
                    "find" => MethodResolution::Collection(CollectionMethod::Find),
                    "any" => MethodResolution::Collection(CollectionMethod::Any),
                    "all" => MethodResolution::Collection(CollectionMethod::All),
                    _ => MethodResolution::NotFound,
                }
            }
            Value::Range(_) => {
                match method_name {
                    "collect" => MethodResolution::Collection(CollectionMethod::Collect),
                    "map" => MethodResolution::Collection(CollectionMethod::Map),
                    "filter" => MethodResolution::Collection(CollectionMethod::Filter),
                    "fold" => MethodResolution::Collection(CollectionMethod::Fold),
                    "find" => MethodResolution::Collection(CollectionMethod::Find),
                    "any" => MethodResolution::Collection(CollectionMethod::Any),
                    "all" => MethodResolution::Collection(CollectionMethod::All),
                    _ => MethodResolution::NotFound,
                }
            }
            Value::Map(_) => {
                match method_name {
                    "map" => MethodResolution::Collection(CollectionMethod::MapEntries),
                    "filter" => MethodResolution::Collection(CollectionMethod::FilterEntries),
                    _ => MethodResolution::NotFound,
                }
            }
            _ => MethodResolution::NotFound,
        }
    }

    fn priority(&self) -> u8 {
        1 // After user/derived methods (priority 0)
    }

    fn name(&self) -> &'static str {
        "CollectionMethodResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority() {
        let resolver = CollectionMethodResolver::new();
        assert_eq!(resolver.priority(), 1);
    }

    #[test]
    fn test_list_map_resolves() {
        let resolver = CollectionMethodResolver::new();
        let list = Value::list(vec![Value::Int(1), Value::Int(2)]);

        let result = resolver.resolve(&list, "[int]", "map");
        assert!(matches!(result, MethodResolution::Collection(CollectionMethod::Map)));
    }

    #[test]
    fn test_list_filter_resolves() {
        let resolver = CollectionMethodResolver::new();
        let list = Value::list(vec![Value::Int(1), Value::Int(2)]);

        let result = resolver.resolve(&list, "[int]", "filter");
        assert!(matches!(result, MethodResolution::Collection(CollectionMethod::Filter)));
    }

    #[test]
    fn test_list_unknown_not_found() {
        let resolver = CollectionMethodResolver::new();
        let list = Value::list(vec![Value::Int(1)]);

        let result = resolver.resolve(&list, "[int]", "unknown");
        assert!(matches!(result, MethodResolution::NotFound));
    }

    #[test]
    fn test_int_not_collection() {
        let resolver = CollectionMethodResolver::new();

        let result = resolver.resolve(&Value::Int(42), "int", "map");
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
