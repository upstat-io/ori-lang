//! Unified method registry combining built-in, inherent, and trait methods.
//!
//! The `MethodRegistry` provides a single entry point for method resolution,
//! searching built-in methods first, then inherent impls, then trait impls.
//!
//! # Method Resolution Priority
//!
//! 1. **Built-in methods** — Compiler-defined methods for primitives/collections
//! 2. **Inherent methods** — Methods from `impl Type { ... }` blocks
//! 3. **Trait methods** — Methods from `impl Trait for Type { ... }` blocks

use ori_ir::Name;
use rustc_hash::FxHashMap;

use crate::{Idx, Pool, Tag};

use super::traits::{MethodLookup, TraitRegistry};
use super::TypeRegistry;

/// Create a Name from a static string (for builtins).
/// Uses a simple hash of the string bytes.
fn static_name(s: &str) -> Name {
    Name::from_raw(s.as_bytes().iter().fold(0u32, |acc, &b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    }))
}

/// Unified method registry.
///
/// Combines built-in methods with user-defined methods from impls.
#[derive(Clone, Debug, Default)]
pub struct MethodRegistry {
    /// Built-in methods indexed by (type tag, method name).
    /// These take priority over all user-defined methods.
    builtin: FxHashMap<(Tag, Name), BuiltinMethod>,

    /// Built-in methods that work on any type with a specific tag.
    /// Indexed by tag for fast iteration.
    builtin_by_tag: FxHashMap<Tag, Vec<Name>>,
}

/// A built-in method defined by the compiler.
#[derive(Clone, Debug)]
pub struct BuiltinMethod {
    /// Method name.
    pub name: Name,

    /// The type tag this method applies to.
    pub receiver_tag: Tag,

    /// Description of the method (for LSP/docs).
    pub doc: &'static str,

    /// The method kind, which determines how to compute the signature.
    pub kind: BuiltinMethodKind,
}

/// How a built-in method computes its return type.
#[derive(Clone, Debug)]
pub enum BuiltinMethodKind {
    /// Returns a fixed type (e.g., `int.abs() -> int`).
    Fixed(Idx),

    /// Returns the receiver's element type (e.g., `list.first() -> option[T]`).
    /// The `Option` wrapper is computed at lookup time.
    Element,

    /// Returns a type computed from the receiver.
    /// Contains information about how to transform the receiver type.
    Transform(MethodTransform),
}

/// How to transform the receiver type to get the return type.
#[derive(Clone, Debug)]
pub enum MethodTransform {
    /// Return the receiver unchanged (e.g., `list.reverse() -> [T]`).
    Identity,

    /// Wrap in Option (e.g., `list.first() -> option[T]`).
    WrapOption,

    /// Return the key type of a Map.
    MapKey,

    /// Return the value type of a Map.
    MapValue,

    /// Return Ok type of Result.
    ResultOk,

    /// Return Err type of Result.
    ResultErr,

    /// Return a function type with specific signature pattern.
    /// Used for methods like `map`, `filter`, etc.
    HigherOrder(HigherOrderMethod),
}

/// Higher-order method signature pattern.
#[derive(Clone, Debug)]
pub enum HigherOrderMethod {
    /// `(T -> U) -> [U]` for List.map
    Map,

    /// `(T -> bool) -> [T]` for List.filter
    Filter,

    /// `(acc, T -> acc) -> acc` for List.fold
    Fold,

    /// `(T -> bool) -> option[T]` for List.find
    Find,

    /// `(T -> bool) -> bool` for List.any/all
    Predicate,
}

/// Result of a method resolution.
#[derive(Clone, Debug)]
pub enum MethodResolution<'a> {
    /// A built-in method.
    Builtin(&'a BuiltinMethod),

    /// A method from an inherent or trait impl.
    Impl(MethodLookup<'a>),
}

impl MethodResolution<'_> {
    /// Check if this is a built-in method.
    #[inline]
    pub fn is_builtin(&self) -> bool {
        matches!(self, Self::Builtin(_))
    }

    /// Get the method name.
    pub fn name(&self) -> Name {
        match self {
            Self::Builtin(m) => m.name,
            Self::Impl(l) => l.method().name,
        }
    }
}

impl MethodRegistry {
    /// Create a new method registry with built-in methods registered.
    pub fn new() -> Self {
        let mut registry = Self::default();
        registry.register_builtins();
        registry
    }

    /// Create an empty registry (for testing).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Register a built-in method.
    pub fn register_builtin(&mut self, method: BuiltinMethod) {
        let key = (method.receiver_tag, method.name);
        self.builtin_by_tag
            .entry(method.receiver_tag)
            .or_default()
            .push(method.name);
        self.builtin.insert(key, method);
    }

    /// Register all built-in methods.
    fn register_builtins(&mut self) {
        self.register_list_methods();
        self.register_option_methods();
        self.register_result_methods();
        self.register_map_methods();
        self.register_set_methods();
        self.register_string_methods();
        self.register_int_methods();
        self.register_float_methods();
    }

    // === Built-in Method Registration ===

    fn register_list_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("len"),
            receiver_tag: Tag::List,
            doc: "Returns the number of elements in the list",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_empty"),
            receiver_tag: Tag::List,
            doc: "Returns true if the list has no elements",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("first"),
            receiver_tag: Tag::List,
            doc: "Returns the first element, or None if empty",
            kind: BuiltinMethodKind::Transform(MethodTransform::WrapOption),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("last"),
            receiver_tag: Tag::List,
            doc: "Returns the last element, or None if empty",
            kind: BuiltinMethodKind::Transform(MethodTransform::WrapOption),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("reverse"),
            receiver_tag: Tag::List,
            doc: "Returns a new list with elements in reverse order",
            kind: BuiltinMethodKind::Transform(MethodTransform::Identity),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("contains"),
            receiver_tag: Tag::List,
            doc: "Returns true if the list contains the element",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });
    }

    fn register_option_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("is_some"),
            receiver_tag: Tag::Option,
            doc: "Returns true if this is Some",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_none"),
            receiver_tag: Tag::Option,
            doc: "Returns true if this is None",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("unwrap"),
            receiver_tag: Tag::Option,
            doc: "Returns the inner value, panics if None",
            kind: BuiltinMethodKind::Element,
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("expect"),
            receiver_tag: Tag::Option,
            doc: "Returns the inner value, panics with message if None",
            kind: BuiltinMethodKind::Element,
        });
    }

    fn register_result_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("is_ok"),
            receiver_tag: Tag::Result,
            doc: "Returns true if this is Ok",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_err"),
            receiver_tag: Tag::Result,
            doc: "Returns true if this is Err",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("unwrap"),
            receiver_tag: Tag::Result,
            doc: "Returns the Ok value, panics if Err",
            kind: BuiltinMethodKind::Transform(MethodTransform::ResultOk),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("unwrap_err"),
            receiver_tag: Tag::Result,
            doc: "Returns the Err value, panics if Ok",
            kind: BuiltinMethodKind::Transform(MethodTransform::ResultErr),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("ok"),
            receiver_tag: Tag::Result,
            doc: "Converts to Option[T], discarding the error",
            kind: BuiltinMethodKind::Transform(MethodTransform::WrapOption),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("err"),
            receiver_tag: Tag::Result,
            doc: "Converts to Option[E], discarding the success value",
            kind: BuiltinMethodKind::Transform(MethodTransform::WrapOption),
        });
    }

    fn register_map_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("len"),
            receiver_tag: Tag::Map,
            doc: "Returns the number of key-value pairs",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_empty"),
            receiver_tag: Tag::Map,
            doc: "Returns true if the map has no entries",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("contains_key"),
            receiver_tag: Tag::Map,
            doc: "Returns true if the map contains the key",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("get"),
            receiver_tag: Tag::Map,
            doc: "Returns the value for a key, or None if not found",
            kind: BuiltinMethodKind::Transform(MethodTransform::WrapOption),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("keys"),
            receiver_tag: Tag::Map,
            doc: "Returns a list of all keys",
            kind: BuiltinMethodKind::Transform(MethodTransform::MapKey),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("values"),
            receiver_tag: Tag::Map,
            doc: "Returns a list of all values",
            kind: BuiltinMethodKind::Transform(MethodTransform::MapValue),
        });
    }

    fn register_set_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("len"),
            receiver_tag: Tag::Set,
            doc: "Returns the number of elements",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_empty"),
            receiver_tag: Tag::Set,
            doc: "Returns true if the set has no elements",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("contains"),
            receiver_tag: Tag::Set,
            doc: "Returns true if the set contains the element",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });
    }

    fn register_string_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("len"),
            receiver_tag: Tag::Str,
            doc: "Returns the length in bytes",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_empty"),
            receiver_tag: Tag::Str,
            doc: "Returns true if the string is empty",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_upper"),
            receiver_tag: Tag::Str,
            doc: "Returns uppercase version",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_lower"),
            receiver_tag: Tag::Str,
            doc: "Returns lowercase version",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("trim"),
            receiver_tag: Tag::Str,
            doc: "Returns string with whitespace trimmed",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("trim_start"),
            receiver_tag: Tag::Str,
            doc: "Returns string with leading whitespace trimmed",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("trim_end"),
            receiver_tag: Tag::Str,
            doc: "Returns string with trailing whitespace trimmed",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("starts_with"),
            receiver_tag: Tag::Str,
            doc: "Returns true if starts with prefix",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("ends_with"),
            receiver_tag: Tag::Str,
            doc: "Returns true if ends with suffix",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("contains"),
            receiver_tag: Tag::Str,
            doc: "Returns true if contains substring",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("chars"),
            receiver_tag: Tag::Str,
            doc: "Returns list of characters",
            kind: BuiltinMethodKind::Fixed(Idx::from_raw(64)), // List[char] - placeholder
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("bytes"),
            receiver_tag: Tag::Str,
            doc: "Returns list of bytes",
            kind: BuiltinMethodKind::Fixed(Idx::from_raw(65)), // List[byte] - placeholder
        });
    }

    fn register_int_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("abs"),
            receiver_tag: Tag::Int,
            doc: "Returns absolute value",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_float"),
            receiver_tag: Tag::Int,
            doc: "Converts to float",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_str"),
            receiver_tag: Tag::Int,
            doc: "Converts to string representation",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("min"),
            receiver_tag: Tag::Int,
            doc: "Returns the smaller of two integers",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("max"),
            receiver_tag: Tag::Int,
            doc: "Returns the larger of two integers",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("clamp"),
            receiver_tag: Tag::Int,
            doc: "Clamps value to a range",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });
    }

    fn register_float_methods(&mut self) {
        self.register_builtin(BuiltinMethod {
            name: static_name("abs"),
            receiver_tag: Tag::Float,
            doc: "Returns absolute value",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("floor"),
            receiver_tag: Tag::Float,
            doc: "Rounds down to nearest integer",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("ceil"),
            receiver_tag: Tag::Float,
            doc: "Rounds up to nearest integer",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("round"),
            receiver_tag: Tag::Float,
            doc: "Rounds to nearest integer",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("trunc"),
            receiver_tag: Tag::Float,
            doc: "Truncates toward zero",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_int"),
            receiver_tag: Tag::Float,
            doc: "Converts to integer (truncates)",
            kind: BuiltinMethodKind::Fixed(Idx::INT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("to_str"),
            receiver_tag: Tag::Float,
            doc: "Converts to string representation",
            kind: BuiltinMethodKind::Fixed(Idx::STR),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_nan"),
            receiver_tag: Tag::Float,
            doc: "Returns true if NaN",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_infinite"),
            receiver_tag: Tag::Float,
            doc: "Returns true if infinite",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("is_finite"),
            receiver_tag: Tag::Float,
            doc: "Returns true if finite (not NaN or infinite)",
            kind: BuiltinMethodKind::Fixed(Idx::BOOL),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("sqrt"),
            receiver_tag: Tag::Float,
            doc: "Returns square root",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("sin"),
            receiver_tag: Tag::Float,
            doc: "Returns sine",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("cos"),
            receiver_tag: Tag::Float,
            doc: "Returns cosine",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("tan"),
            receiver_tag: Tag::Float,
            doc: "Returns tangent",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("ln"),
            receiver_tag: Tag::Float,
            doc: "Returns natural logarithm",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("log10"),
            receiver_tag: Tag::Float,
            doc: "Returns base-10 logarithm",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("exp"),
            receiver_tag: Tag::Float,
            doc: "Returns e raised to this power",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("pow"),
            receiver_tag: Tag::Float,
            doc: "Returns this raised to a power",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("min"),
            receiver_tag: Tag::Float,
            doc: "Returns the smaller of two floats",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("max"),
            receiver_tag: Tag::Float,
            doc: "Returns the larger of two floats",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });

        self.register_builtin(BuiltinMethod {
            name: static_name("clamp"),
            receiver_tag: Tag::Float,
            doc: "Clamps value to a range",
            kind: BuiltinMethodKind::Fixed(Idx::FLOAT),
        });
    }

    // === Method Lookup ===

    /// Look up a method on a type.
    ///
    /// Resolution priority:
    /// 1. Built-in methods (by tag)
    /// 2. Inherent methods (from `impl Type { ... }`)
    /// 3. Trait methods (from `impl Trait for Type { ... }`)
    pub fn lookup<'a>(
        &'a self,
        pool: &Pool,
        receiver_ty: Idx,
        method_name: Name,
        _type_registry: &TypeRegistry,
        trait_registry: &'a TraitRegistry,
    ) -> Option<MethodResolution<'a>> {
        // 1. Check built-in methods first
        let tag = pool.tag(receiver_ty);
        if let Some(builtin) = self.builtin.get(&(tag, method_name)) {
            return Some(MethodResolution::Builtin(builtin));
        }

        // 2. Check user-defined methods (inherent first, then trait)
        if let Some(lookup) = trait_registry.lookup_method(receiver_ty, method_name) {
            return Some(MethodResolution::Impl(lookup));
        }

        None
    }

    /// Get all built-in methods for a type tag.
    pub fn builtin_methods_for_tag(&self, tag: Tag) -> impl Iterator<Item = &BuiltinMethod> {
        self.builtin_by_tag
            .get(&tag)
            .into_iter()
            .flat_map(|names| names.iter())
            .filter_map(move |name| self.builtin.get(&(tag, *name)))
    }

    /// Check if a built-in method exists.
    #[inline]
    pub fn has_builtin(&self, tag: Tag, method_name: Name) -> bool {
        self.builtin.contains_key(&(tag, method_name))
    }

    /// Get a built-in method by tag and name.
    #[inline]
    pub fn get_builtin(&self, tag: Tag, method_name: Name) -> Option<&BuiltinMethod> {
        self.builtin.get(&(tag, method_name))
    }

    /// Compute the return type of a built-in method.
    ///
    /// This needs access to the pool to resolve element types for containers.
    pub fn builtin_return_type(
        &self,
        pool: &mut Pool,
        receiver_ty: Idx,
        method: &BuiltinMethod,
    ) -> Idx {
        match &method.kind {
            BuiltinMethodKind::Fixed(ty) => *ty,

            BuiltinMethodKind::Element => {
                // Get element type from container
                let tag = pool.tag(receiver_ty);
                match tag {
                    Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range => {
                        Idx::from_raw(pool.data(receiver_ty))
                    }
                    _ => Idx::ERROR,
                }
            }

            BuiltinMethodKind::Transform(transform) => {
                self.apply_transform(pool, receiver_ty, transform)
            }
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "may use self for complex transforms in future"
    )]
    fn apply_transform(
        &self,
        pool: &mut Pool,
        receiver_ty: Idx,
        transform: &MethodTransform,
    ) -> Idx {
        let tag = pool.tag(receiver_ty);

        match transform {
            MethodTransform::Identity => receiver_ty,

            MethodTransform::WrapOption => {
                // Get element type and wrap in Option
                let elem = match tag {
                    Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range => {
                        Idx::from_raw(pool.data(receiver_ty))
                    }
                    Tag::Result => {
                        // For result.ok(), return Option[OkType]
                        pool.result_ok(receiver_ty)
                    }
                    _ => receiver_ty,
                };
                pool.option(elem)
            }

            MethodTransform::MapKey => {
                if tag == Tag::Map {
                    let key = pool.map_key(receiver_ty);
                    pool.list(key)
                } else {
                    Idx::ERROR
                }
            }

            MethodTransform::MapValue => {
                if tag == Tag::Map {
                    let value = pool.map_value(receiver_ty);
                    pool.list(value)
                } else {
                    Idx::ERROR
                }
            }

            MethodTransform::ResultOk => {
                if tag == Tag::Result {
                    pool.result_ok(receiver_ty)
                } else {
                    Idx::ERROR
                }
            }

            MethodTransform::ResultErr => {
                if tag == Tag::Result {
                    pool.result_err(receiver_ty)
                } else {
                    Idx::ERROR
                }
            }

            MethodTransform::HigherOrder(_hom) => {
                // TODO: Higher-order methods need more complex signature computation
                // For now, return a placeholder
                Idx::ERROR
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn test_name(s: &str) -> Name {
        // Must match static_name() hash algorithm
        Name::from_raw(s.as_bytes().iter().fold(0u32, |acc, &b| {
            acc.wrapping_mul(31).wrapping_add(u32::from(b))
        }))
    }

    #[test]
    fn builtin_methods_registered() {
        let registry = MethodRegistry::new();

        // Check list methods
        assert!(registry.has_builtin(Tag::List, test_name("len")));
        assert!(registry.has_builtin(Tag::List, test_name("is_empty")));
        assert!(registry.has_builtin(Tag::List, test_name("first")));

        // Check string methods
        assert!(registry.has_builtin(Tag::Str, test_name("len")));
        assert!(registry.has_builtin(Tag::Str, test_name("trim")));
        assert!(registry.has_builtin(Tag::Str, test_name("to_upper")));

        // Check int methods
        assert!(registry.has_builtin(Tag::Int, test_name("abs")));
        assert!(registry.has_builtin(Tag::Int, test_name("to_float")));

        // Check float methods
        assert!(registry.has_builtin(Tag::Float, test_name("abs")));
        assert!(registry.has_builtin(Tag::Float, test_name("sqrt")));
        assert!(registry.has_builtin(Tag::Float, test_name("sin")));
    }

    #[test]
    fn fixed_return_type() {
        let registry = MethodRegistry::new();
        let mut pool = Pool::new();

        let method = registry
            .get_builtin(Tag::Int, test_name("abs"))
            .expect("abs should exist");

        let ret = registry.builtin_return_type(&mut pool, Idx::INT, method);
        assert_eq!(ret, Idx::INT);
    }

    #[test]
    fn element_return_type() {
        let registry = MethodRegistry::new();
        let mut pool = Pool::new();

        // Create option[int]
        let option_int = pool.option(Idx::INT);

        let method = registry
            .get_builtin(Tag::Option, test_name("unwrap"))
            .expect("unwrap should exist");

        let ret = registry.builtin_return_type(&mut pool, option_int, method);
        assert_eq!(ret, Idx::INT);
    }

    #[test]
    fn wrap_option_transform() {
        let registry = MethodRegistry::new();
        let mut pool = Pool::new();

        // Create [int]
        let list_int = pool.list(Idx::INT);

        let method = registry
            .get_builtin(Tag::List, test_name("first"))
            .expect("first should exist");

        let ret = registry.builtin_return_type(&mut pool, list_int, method);

        // Should return option[int]
        assert_eq!(pool.tag(ret), Tag::Option);
        let inner = Idx::from_raw(pool.data(ret));
        assert_eq!(inner, Idx::INT);
    }

    #[test]
    fn builtin_methods_for_tag() {
        let registry = MethodRegistry::new();

        let list_methods: Vec<_> = registry.builtin_methods_for_tag(Tag::List).collect();
        assert!(!list_methods.is_empty());

        let names: Vec<_> = list_methods.iter().map(|m| m.name).collect();
        assert!(names.contains(&test_name("len")));
        assert!(names.contains(&test_name("first")));
    }
}
