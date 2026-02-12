//! Built-in method resolver.
//!
//! Resolves methods on primitive types (int, float, str, list, map, etc.)
//! by checking against the pre-interned method table from `EVAL_BUILTIN_METHODS`.

use rustc_hash::FxHashSet;

use ori_ir::{Name, StringInterner};

use super::{MethodResolution, MethodResolver, Value};

/// Resolver for built-in methods on primitive types.
///
/// Priority 2 (lowest) — built-in methods are the fallback when no other
/// resolver handles the method.
///
/// At construction, interns all `(type_name, method_name)` pairs from
/// `EVAL_BUILTIN_METHODS` into an `FxHashSet<(Name, Name)>` for O(1) lookup.
/// This means the resolver does *real* resolution: it returns `Builtin` only
/// for methods that actually exist, and `NotFound` for everything else.
///
/// Newtypes have dynamic user-defined type names that can't be pre-registered,
/// so the resolver also checks the receiver's `Value` variant for newtypes.
#[derive(Clone)]
pub struct BuiltinMethodResolver {
    /// Pre-interned (`type_name`, `method_name`) pairs for O(1) existence check.
    known_methods: FxHashSet<(Name, Name)>,
    /// Pre-interned "unwrap" for newtype method dispatch.
    unwrap_name: Name,
}

impl BuiltinMethodResolver {
    /// Create a new built-in method resolver with pre-interned method names.
    pub fn new(interner: &StringInterner) -> Self {
        let known_methods = crate::methods::EVAL_BUILTIN_METHODS
            .iter()
            .map(|(type_name, method_name)| {
                (interner.intern(type_name), interner.intern(method_name))
            })
            .collect();
        Self {
            known_methods,
            unwrap_name: interner.intern("unwrap"),
        }
    }
}

impl MethodResolver for BuiltinMethodResolver {
    fn resolve(&self, receiver: &Value, type_name: Name, method_name: Name) -> MethodResolution {
        // Static lookup for known type/method pairs
        if self.known_methods.contains(&(type_name, method_name)) {
            return MethodResolution::Builtin;
        }

        // Newtypes have user-defined type names that can't be pre-registered.
        // Check the receiver variant directly for known newtype methods.
        if matches!(receiver, Value::Newtype { .. }) && method_name == self.unwrap_name {
            return MethodResolution::Builtin;
        }

        MethodResolution::NotFound
    }

    fn priority(&self) -> u8 {
        2 // Lowest priority - fallback
    }

    fn name(&self) -> &'static str {
        "BuiltinMethodResolver"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_ir::SharedInterner;

    #[test]
    fn test_priority() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);
        assert_eq!(resolver.priority(), 2);
    }

    #[test]
    fn known_method_returns_builtin() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);

        let int_type = interner.intern("int");
        let add_method = interner.intern("add");
        let str_type = interner.intern("str");
        let len_method = interner.intern("len");

        let result = resolver.resolve(&Value::int(42), int_type, add_method);
        assert!(matches!(result, MethodResolution::Builtin));

        let result = resolver.resolve(&Value::string("hello"), str_type, len_method);
        assert!(matches!(result, MethodResolution::Builtin));
    }

    #[test]
    fn unknown_method_returns_not_found() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);

        let int_type = interner.intern("int");
        let nonexistent = interner.intern("nonexistent_method");

        let result = resolver.resolve(&Value::int(42), int_type, nonexistent);
        assert!(matches!(result, MethodResolution::NotFound));
    }

    #[test]
    fn wrong_type_returns_not_found() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);

        // "len" exists on "str" but not on "int"
        let int_type = interner.intern("int");
        let len_method = interner.intern("len");

        let result = resolver.resolve(&Value::int(42), int_type, len_method);
        assert!(matches!(result, MethodResolution::NotFound));
    }

    #[test]
    fn newtype_unwrap_resolves_builtin() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);

        let user_type = interner.intern("UserId");
        let unwrap = interner.intern("unwrap");

        let newtype_val = Value::newtype(user_type, Value::int(42));
        let result = resolver.resolve(&newtype_val, user_type, unwrap);
        assert!(matches!(result, MethodResolution::Builtin));
    }

    #[test]
    fn newtype_unknown_method_returns_not_found() {
        let interner = SharedInterner::default();
        let resolver = BuiltinMethodResolver::new(&interner);

        let user_type = interner.intern("UserId");
        let nonexistent = interner.intern("nonexistent");

        let newtype_val = Value::newtype(user_type, Value::int(42));
        let result = resolver.resolve(&newtype_val, user_type, nonexistent);
        assert!(matches!(result, MethodResolution::NotFound));
    }
}
