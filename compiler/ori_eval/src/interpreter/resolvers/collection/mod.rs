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
    rev: Name,
    last: Name,
    rfind: Name,
    rfold: Name,
    join: Name,
    // Internal (rewritten by canonicalization)
    collect_set: Name,
    // Option/Result — closure-taking methods
    and_then: Name,
    or_else: Name,
    map_err: Name,
    // Ordering — closure-taking method
    then_with: Name,
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
            rev: interner.intern("rev"),
            last: interner.intern("last"),
            rfind: interner.intern("rfind"),
            rfold: interner.intern("rfold"),
            join: interner.intern("join"),
            and_then: interner.intern("and_then"),
            or_else: interner.intern("or_else"),
            map_err: interner.intern("map_err"),
            collect_set: interner
                .intern(ori_ir::builtin_constants::protocol::ProtocolBuiltin::CollectSet.name()),
            then_with: interner.intern("then_with"),
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
    #[expect(
        clippy::cognitive_complexity,
        reason = "linear name-to-method dispatch over 25 pre-interned iterator methods"
    )]
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
        } else if method_name == m.rev {
            Some(CollectionMethod::IterRev)
        } else if method_name == m.last {
            Some(CollectionMethod::IterLast)
        } else if method_name == m.rfind {
            Some(CollectionMethod::IterRFind)
        } else if method_name == m.rfold {
            Some(CollectionMethod::IterRFold)
        } else if method_name == m.join {
            Some(CollectionMethod::IterJoin)
        } else if method_name == m.collect_set {
            Some(CollectionMethod::IterCollectSet)
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
            Value::List(_) => {
                if method_name == self.methods.join {
                    MethodResolution::Collection(CollectionMethod::Join)
                } else {
                    self.resolve_iterable_method(method_name)
                        .map_or(MethodResolution::NotFound, MethodResolution::Collection)
                }
            }
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
            Value::Ordering(_) if method_name == self.methods.then_with => {
                MethodResolution::Collection(CollectionMethod::OrderingThenWith)
            }
            // Option closure methods — need evaluator access to call closures
            Value::Some(_) | Value::None => {
                if method_name == self.methods.map {
                    MethodResolution::Collection(CollectionMethod::OptionMap)
                } else if method_name == self.methods.and_then {
                    MethodResolution::Collection(CollectionMethod::OptionAndThen)
                } else if method_name == self.methods.flat_map {
                    MethodResolution::Collection(CollectionMethod::OptionFlatMap)
                } else if method_name == self.methods.filter {
                    MethodResolution::Collection(CollectionMethod::OptionFilter)
                } else if method_name == self.methods.or_else {
                    MethodResolution::Collection(CollectionMethod::OptionOrElse)
                } else {
                    MethodResolution::NotFound
                }
            }
            // Result closure methods — need evaluator access to call closures
            Value::Ok(_) | Value::Err(_) => {
                if method_name == self.methods.map {
                    MethodResolution::Collection(CollectionMethod::ResultMap)
                } else if method_name == self.methods.map_err {
                    MethodResolution::Collection(CollectionMethod::ResultMapErr)
                } else if method_name == self.methods.and_then {
                    MethodResolution::Collection(CollectionMethod::ResultAndThen)
                } else if method_name == self.methods.or_else {
                    MethodResolution::Collection(CollectionMethod::ResultOrElse)
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
