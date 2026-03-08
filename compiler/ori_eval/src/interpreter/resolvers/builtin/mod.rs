//! Built-in method resolver.
//!
//! Resolves methods on primitive types (int, float, str, list, map, etc.)
//! by checking against method declarations from `ori_registry::BUILTIN_TYPES`.

use rustc_hash::FxHashSet;

use ori_ir::{Name, StringInterner};

use super::{MethodResolution, MethodResolver, Value};

/// Map registry `PascalCase` type names to evaluator convention.
///
/// The evaluator's `get_value_type_name()` (via `TypeNames`) uses lowercase
/// for 5 types that the registry stores as `PascalCase`:
/// `List`, `Map`, `Range`, `Tuple`, `Error`.
///
/// Must stay in sync with `TypeNames::new()` in `interpreter/interned_names.rs`
/// and `Value::type_name()` in `ori_patterns/src/value/conversions.rs`.
fn eval_type_name(registry_name: &str) -> &str {
    match registry_name {
        "List" => "list",
        "Map" => "map",
        "Range" => "range",
        "Tuple" => "tuple",
        "Error" => "error",
        other => other, // int, float, str, Duration, Size, etc. already match
    }
}

/// Resolver for built-in methods on primitive types.
///
/// Priority 2 (lowest) — built-in methods are the fallback when no other
/// resolver handles the method.
///
/// At construction, interns all `(type_name, method_name)` pairs from
/// `ori_registry::BUILTIN_TYPES` into an `FxHashSet<(Name, Name)>` for O(1)
/// lookup. This means the resolver does *real* resolution: it returns `Builtin`
/// only for methods that actually exist, and `NotFound` for everything else.
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
    ///
    /// Reads from `ori_registry::BUILTIN_TYPES` (the single source of truth)
    /// and maps type names to evaluator convention via [`eval_type_name()`].
    pub fn new(interner: &StringInterner) -> Self {
        let known_methods = ori_registry::BUILTIN_TYPES
            .iter()
            .flat_map(|type_def| {
                let type_name = interner.intern(eval_type_name(type_def.name));
                type_def
                    .methods
                    .iter()
                    .map(move |method| (type_name, interner.intern(method.name)))
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
mod tests;
