//! Collection method resolver.
//!
//! Resolves methods on collections (list, map, range) that require
//! evaluator access to call function arguments.

use sigil_ir::{Name, StringInterner};
use super::{CollectionMethod, MethodResolution, MethodResolver, Value};

/// Pre-interned method names for efficient comparison.
struct MethodNames {
    map: Name,
    filter: Name,
    fold: Name,
    find: Name,
    collect: Name,
    any: Name,
    all: Name,
}

impl MethodNames {
    fn new(interner: &StringInterner) -> Self {
        Self {
            map: interner.intern("map"),
            filter: interner.intern("filter"),
            fold: interner.intern("fold"),
            find: interner.intern("find"),
            collect: interner.intern("collect"),
            any: interner.intern("any"),
            all: interner.intern("all"),
        }
    }
}

/// Resolver for collection methods that require evaluator access.
///
/// Priority 1 - collection methods are checked after user/derived methods.
///
/// These methods take function arguments and need evaluator access to call them:
/// - map, filter, fold, find on lists
/// - collect on ranges
/// - map, filter on maps
/// - any, all on lists
pub struct CollectionMethodResolver {
    methods: MethodNames,
}

impl CollectionMethodResolver {
    /// Create a new collection method resolver.
    pub fn new(interner: &StringInterner) -> Self {
        Self {
            methods: MethodNames::new(interner),
        }
    }

    /// Resolve methods common to all iterable types (List, Range).
    fn resolve_iterable_method(&self, method_name: Name) -> Option<CollectionMethod> {
        if method_name == self.methods.map {
            Some(CollectionMethod::Map)
        } else if method_name == self.methods.filter {
            Some(CollectionMethod::Filter)
        } else if method_name == self.methods.fold {
            Some(CollectionMethod::Fold)
        } else if method_name == self.methods.find {
            Some(CollectionMethod::Find)
        } else if method_name == self.methods.any {
            Some(CollectionMethod::Any)
        } else if method_name == self.methods.all {
            Some(CollectionMethod::All)
        } else {
            None
        }
    }
}

impl MethodResolver for CollectionMethodResolver {
    fn resolve(&self, receiver: &Value, _type_name: Name, method_name: Name) -> MethodResolution {
        // Check if this is a collection type and the method is a known collection method
        match receiver {
            Value::List(_) => self
                .resolve_iterable_method(method_name)
                .map_or(MethodResolution::NotFound, MethodResolution::Collection),
            Value::Range(_) => {
                // Range has collect() in addition to iterable methods
                if method_name == self.methods.collect {
                    MethodResolution::Collection(CollectionMethod::Collect)
                } else {
                    self.resolve_iterable_method(method_name)
                        .map_or(MethodResolution::NotFound, MethodResolution::Collection)
                }
            }
            Value::Map(_) => {
                // Map uses special *Entries variants for map/filter
                if method_name == self.methods.map {
                    MethodResolution::Collection(CollectionMethod::MapEntries)
                } else if method_name == self.methods.filter {
                    MethodResolution::Collection(CollectionMethod::FilterEntries)
                } else {
                    MethodResolution::NotFound
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
        let interner = StringInterner::new();
        let resolver = CollectionMethodResolver::new(&interner);
        assert_eq!(resolver.priority(), 1);
    }

    #[test]
    fn test_list_map_resolves() {
        let interner = StringInterner::new();
        let resolver = CollectionMethodResolver::new(&interner);
        let list = Value::list(vec![Value::Int(1), Value::Int(2)]);

        let list_type = interner.intern("[int]");
        let map_method = interner.intern("map");
        let result = resolver.resolve(&list, list_type, map_method);
        assert!(matches!(result, MethodResolution::Collection(CollectionMethod::Map)));
    }

    #[test]
    fn test_list_filter_resolves() {
        let interner = StringInterner::new();
        let resolver = CollectionMethodResolver::new(&interner);
        let list = Value::list(vec![Value::Int(1), Value::Int(2)]);

        let list_type = interner.intern("[int]");
        let filter_method = interner.intern("filter");
        let result = resolver.resolve(&list, list_type, filter_method);
        assert!(matches!(result, MethodResolution::Collection(CollectionMethod::Filter)));
    }

    #[test]
    fn test_list_unknown_not_found() {
        let interner = StringInterner::new();
        let resolver = CollectionMethodResolver::new(&interner);
        let list = Value::list(vec![Value::Int(1)]);

        let list_type = interner.intern("[int]");
        let unknown = interner.intern("unknown");
        let result = resolver.resolve(&list, list_type, unknown);
        assert!(matches!(result, MethodResolution::NotFound));
    }

    #[test]
    fn test_int_not_collection() {
        let interner = StringInterner::new();
        let resolver = CollectionMethodResolver::new(&interner);

        let int_type = interner.intern("int");
        let map_method = interner.intern("map");
        let result = resolver.resolve(&Value::Int(42), int_type, map_method);
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
