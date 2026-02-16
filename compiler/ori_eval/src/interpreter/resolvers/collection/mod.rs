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
    // Iterator-specific
    next: Name,
    take: Name,
    skip: Name,
    count: Name,
    for_each: Name,
    enumerate: Name,
    zip: Name,
    chain: Name,
    flatten: Name,
    flat_map: Name,
    cycle: Name,
    next_back: Name,
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
            next: interner.intern("next"),
            take: interner.intern("take"),
            skip: interner.intern("skip"),
            count: interner.intern("count"),
            for_each: interner.intern("for_each"),
            enumerate: interner.intern("enumerate"),
            zip: interner.intern("zip"),
            chain: interner.intern("chain"),
            flatten: interner.intern("flatten"),
            flat_map: interner.intern("flat_map"),
            cycle: interner.intern("cycle"),
            next_back: interner.intern("next_back"),
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

    /// Resolve methods on `Iterator<T>` values.
    fn resolve_iterator_method(&self, method_name: Name) -> Option<CollectionMethod> {
        let m = &self.methods;
        if method_name == m.next {
            Some(CollectionMethod::IterNext)
        } else if method_name == m.map {
            Some(CollectionMethod::IterMap)
        } else if method_name == m.filter {
            Some(CollectionMethod::IterFilter)
        } else if method_name == m.take {
            Some(CollectionMethod::IterTake)
        } else if method_name == m.skip {
            Some(CollectionMethod::IterSkip)
        } else if method_name == m.fold {
            Some(CollectionMethod::IterFold)
        } else if method_name == m.count {
            Some(CollectionMethod::IterCount)
        } else if method_name == m.find {
            Some(CollectionMethod::IterFind)
        } else if method_name == m.any {
            Some(CollectionMethod::IterAny)
        } else if method_name == m.all {
            Some(CollectionMethod::IterAll)
        } else if method_name == m.for_each {
            Some(CollectionMethod::IterForEach)
        } else if method_name == m.collect {
            Some(CollectionMethod::IterCollect)
        } else if method_name == m.enumerate {
            Some(CollectionMethod::IterEnumerate)
        } else if method_name == m.zip {
            Some(CollectionMethod::IterZip)
        } else if method_name == m.chain {
            Some(CollectionMethod::IterChain)
        } else if method_name == m.flatten {
            Some(CollectionMethod::IterFlatten)
        } else if method_name == m.flat_map {
            Some(CollectionMethod::IterFlatMap)
        } else if method_name == m.cycle {
            Some(CollectionMethod::IterCycle)
        } else if method_name == m.next_back {
            Some(CollectionMethod::IterNextBack)
        } else {
            None
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
            Value::Iterator(_) => self
                .resolve_iterator_method(method_name)
                .map_or(MethodResolution::NotFound, MethodResolution::Collection),
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
