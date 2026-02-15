//! Collection method resolver.
//!
//! Resolves methods on collections (list, map, range) that require
//! evaluator access to call function arguments.

use super::{CollectionMethod, MethodResolution, MethodResolver, Value};
use ori_ir::{Name, StringInterner};

/// Pre-interned method names for efficient comparison.
#[derive(Clone)]
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
#[derive(Clone)]
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
mod tests;
