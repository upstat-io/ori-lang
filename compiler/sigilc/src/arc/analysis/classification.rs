// Type Classification for ARC Memory Management
//
// Classifies Sigil types as Value, Reference, or Hybrid for ARC purposes.
// The 32-byte threshold determines when a type is stored inline (value)
// versus on the heap (reference).

use crate::ir::Type;

use super::super::traits::{StorageClass, TypeClassification, TypeClassifier};
use super::size_calculator::TypeSizeCalculator;

/// Default value type size threshold in bytes
/// Types at or below this size are stored inline (value types)
pub const VALUE_TYPE_THRESHOLD: usize = 32;

/// Default implementation of TypeClassifier
pub struct DefaultTypeClassifier {
    /// Size calculator for computing type sizes
    size_calc: TypeSizeCalculator,

    /// Threshold for value types (types <= this size are values)
    threshold: usize,
}

impl Default for DefaultTypeClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultTypeClassifier {
    /// Create a new classifier with default settings
    pub fn new() -> Self {
        DefaultTypeClassifier {
            size_calc: TypeSizeCalculator::new(),
            threshold: VALUE_TYPE_THRESHOLD,
        }
    }

    /// Create a classifier with a custom threshold
    pub fn with_threshold(threshold: usize) -> Self {
        DefaultTypeClassifier {
            size_calc: TypeSizeCalculator::new(),
            threshold,
        }
    }

    /// Check if a type contains any reference fields (recursive)
    fn contains_references(&self, ty: &Type) -> bool {
        match ty {
            // Primitives never contain references
            Type::Int | Type::Float | Type::Bool | Type::Void => false,

            // Strings are reference types
            Type::Str => true,

            // Collections are reference types
            Type::List(_) | Type::Map(_, _) => true,

            // Tuples may contain references
            Type::Tuple(elems) => elems.iter().any(|e| self.contains_references(e)),

            // Structs may contain references
            Type::Struct { fields, .. } => {
                fields.iter().any(|(_, ty)| self.contains_references(ty))
            }

            // Enums may contain references
            Type::Enum { variants, .. } => variants.iter().any(|(_, fields)| {
                fields.iter().any(|(_, ty)| self.contains_references(ty))
            }),

            // Named types conservatively assumed to contain references
            Type::Named(_) => true,

            // Functions are always references (closures may capture)
            Type::Function { .. } => true,

            // Result/Option may contain references
            Type::Result(ok, err) => {
                self.contains_references(ok) || self.contains_references(err)
            }
            Type::Option(inner) => self.contains_references(inner),

            // Records may contain references
            Type::Record(fields) => fields.iter().any(|(_, ty)| self.contains_references(ty)),

            // Range is a value type
            Type::Range => false,

            // Any and trait objects are always references
            Type::Any | Type::DynTrait(_) => true,
        }
    }

    /// Determine the storage class for a type
    fn determine_storage(&self, ty: &Type, size: usize, has_refs: bool) -> StorageClass {
        // Certain types are always reference types regardless of size
        // because they own heap data
        match ty {
            Type::Str | Type::List(_) | Type::Map(_, _) | Type::Function { .. } => {
                return StorageClass::Reference;
            }
            Type::Any | Type::DynTrait(_) => {
                return StorageClass::Reference;
            }
            _ => {}
        }

        // Primitives and small types without references are values
        if !has_refs && size <= self.threshold {
            return StorageClass::Value;
        }

        // Large types or types with references that are still small could be hybrid
        // (e.g., a small struct containing a string field)
        if has_refs && size <= self.threshold {
            return StorageClass::Hybrid;
        }

        // Everything else is a reference type
        StorageClass::Reference
    }

    /// Get the size calculator (for advanced use)
    pub fn size_calculator(&self) -> &TypeSizeCalculator {
        &self.size_calc
    }
}

impl TypeClassifier for DefaultTypeClassifier {
    fn classify(&self, ty: &Type) -> TypeClassification {
        let size = self.size_calc.size_of(ty);
        let has_refs = self.contains_references(ty);
        let storage = self.determine_storage(ty, size, has_refs);

        TypeClassification {
            storage,
            size_bytes: size,
            contains_references: has_refs,
            requires_destruction: has_refs || matches!(storage, StorageClass::Reference),
        }
    }

    fn is_value_type(&self, ty: &Type) -> bool {
        // Quick check for common value types
        match ty {
            Type::Int | Type::Float | Type::Bool | Type::Void | Type::Range => true,
            Type::Str | Type::List(_) | Type::Map(_, _) | Type::Function { .. } => false,
            Type::Any | Type::DynTrait(_) => false,
            _ => {
                let classification = self.classify(ty);
                classification.storage == StorageClass::Value
            }
        }
    }

    fn size_of(&self, ty: &Type) -> usize {
        self.size_calc.size_of(ty)
    }

    fn requires_destruction(&self, ty: &Type) -> bool {
        // Quick check for types that never need destruction
        match ty {
            Type::Int | Type::Float | Type::Bool | Type::Void | Type::Range => false,
            Type::Str | Type::List(_) | Type::Map(_, _) | Type::Function { .. } => true,
            Type::Any | Type::DynTrait(_) => true,
            _ => self.classify(ty).requires_destruction,
        }
    }
}

/// Classify a type for ARC using the default classifier
pub fn classify(ty: &Type) -> TypeClassification {
    DefaultTypeClassifier::new().classify(ty)
}

/// Quick check if a type is a value type
pub fn is_value(ty: &Type) -> bool {
    DefaultTypeClassifier::new().is_value_type(ty)
}

/// Quick check if a type requires destruction
pub fn requires_destruction(ty: &Type) -> bool {
    DefaultTypeClassifier::new().requires_destruction(ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_classification() {
        let classifier = DefaultTypeClassifier::new();

        let int_class = classifier.classify(&Type::Int);
        assert_eq!(int_class.storage, StorageClass::Value);
        assert!(!int_class.contains_references);
        assert!(!int_class.requires_destruction);

        let bool_class = classifier.classify(&Type::Bool);
        assert_eq!(bool_class.storage, StorageClass::Value);
    }

    #[test]
    fn test_string_classification() {
        let classifier = DefaultTypeClassifier::new();

        let str_class = classifier.classify(&Type::Str);
        // String is a reference type because it contains heap data
        assert!(str_class.contains_references);
        assert!(str_class.requires_destruction);
    }

    #[test]
    fn test_list_classification() {
        let classifier = DefaultTypeClassifier::new();

        let list_class = classifier.classify(&Type::List(Box::new(Type::Int)));
        assert_eq!(list_class.storage, StorageClass::Reference);
        assert!(list_class.contains_references);
        assert!(list_class.requires_destruction);
    }

    #[test]
    fn test_small_struct_classification() {
        let classifier = DefaultTypeClassifier::new();

        // Small struct with only value types (16 bytes)
        let small_struct = Type::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::Int),
                ("y".to_string(), Type::Int),
            ],
        };
        let class = classifier.classify(&small_struct);
        assert_eq!(class.storage, StorageClass::Value);
        assert!(!class.contains_references);
    }

    #[test]
    fn test_struct_with_reference_classification() {
        let classifier = DefaultTypeClassifier::new();

        // Struct with a reference field
        let struct_with_ref = Type::Struct {
            name: "Named".to_string(),
            fields: vec![
                ("id".to_string(), Type::Int),
                ("name".to_string(), Type::Str),
            ],
        };
        let class = classifier.classify(&struct_with_ref);
        assert!(class.contains_references);
        assert!(class.requires_destruction);
    }

    #[test]
    fn test_large_struct_classification() {
        let classifier = DefaultTypeClassifier::new();

        // Large struct (5 * 8 = 40 bytes > 32 threshold)
        let large_struct = Type::Struct {
            name: "Large".to_string(),
            fields: vec![
                ("a".to_string(), Type::Int),
                ("b".to_string(), Type::Int),
                ("c".to_string(), Type::Int),
                ("d".to_string(), Type::Int),
                ("e".to_string(), Type::Int),
            ],
        };
        let class = classifier.classify(&large_struct);
        assert_eq!(class.storage, StorageClass::Reference);
    }

    #[test]
    fn test_option_classification() {
        let classifier = DefaultTypeClassifier::new();

        // Option<int> should be a value type
        let opt_int = Type::Option(Box::new(Type::Int));
        let class = classifier.classify(&opt_int);
        assert_eq!(class.storage, StorageClass::Value);

        // Option<str> should have references
        let opt_str = Type::Option(Box::new(Type::Str));
        let class = classifier.classify(&opt_str);
        assert!(class.contains_references);
    }

    #[test]
    fn test_result_classification() {
        let classifier = DefaultTypeClassifier::new();

        // Result<int, int> should be a value type
        let result_int = Type::Result(Box::new(Type::Int), Box::new(Type::Int));
        let class = classifier.classify(&result_int);
        assert_eq!(class.storage, StorageClass::Value);

        // Result<str, Error> should have references
        let result_str = Type::Result(Box::new(Type::Str), Box::new(Type::Named("Error".to_string())));
        let class = classifier.classify(&result_str);
        assert!(class.contains_references);
    }

    #[test]
    fn test_custom_threshold() {
        let classifier = DefaultTypeClassifier::with_threshold(16);

        // Point (16 bytes) should now be at the threshold
        let point = Type::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::Int),
                ("y".to_string(), Type::Int),
            ],
        };
        let class = classifier.classify(&point);
        assert_eq!(class.storage, StorageClass::Value);

        // Triple (24 bytes) should be a reference type with 16-byte threshold
        let triple = Type::Struct {
            name: "Triple".to_string(),
            fields: vec![
                ("x".to_string(), Type::Int),
                ("y".to_string(), Type::Int),
                ("z".to_string(), Type::Int),
            ],
        };
        let class = classifier.classify(&triple);
        assert_eq!(class.storage, StorageClass::Reference);
    }

    #[test]
    fn test_is_value_quick_check() {
        let classifier = DefaultTypeClassifier::new();

        assert!(classifier.is_value_type(&Type::Int));
        assert!(classifier.is_value_type(&Type::Float));
        assert!(classifier.is_value_type(&Type::Bool));
        assert!(classifier.is_value_type(&Type::Range));

        assert!(!classifier.is_value_type(&Type::Str));
        assert!(!classifier.is_value_type(&Type::List(Box::new(Type::Int))));
    }

    #[test]
    fn test_requires_destruction() {
        let classifier = DefaultTypeClassifier::new();

        assert!(!classifier.requires_destruction(&Type::Int));
        assert!(!classifier.requires_destruction(&Type::Bool));
        assert!(!classifier.requires_destruction(&Type::Range));

        assert!(classifier.requires_destruction(&Type::Str));
        assert!(classifier.requires_destruction(&Type::List(Box::new(Type::Int))));
    }
}
