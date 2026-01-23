// Type Size Calculator for ARC Memory Management
//
// Computes the memory layout and size of Sigil types for the target platform.
// Used to determine whether a type qualifies as a value type (inline storage)
// or reference type (heap allocated).

use crate::ir::Type;
use std::collections::HashMap;

/// Platform-specific type sizes
#[derive(Debug, Clone)]
pub struct PlatformSizes {
    /// Size of a pointer in bytes
    pub pointer_size: usize,

    /// Size of int type in bytes
    pub int_size: usize,

    /// Size of float type in bytes
    pub float_size: usize,

    /// Size of bool type in bytes
    pub bool_size: usize,

    /// Size of char type in bytes (for single codepoint)
    pub char_size: usize,

    /// Minimum alignment requirement
    pub min_alignment: usize,
}

impl Default for PlatformSizes {
    fn default() -> Self {
        Self::lp64()
    }
}

impl PlatformSizes {
    /// LP64 platform (64-bit pointers, common on Unix)
    pub fn lp64() -> Self {
        PlatformSizes {
            pointer_size: 8,
            int_size: 8,       // 64-bit integers
            float_size: 8,    // 64-bit floats
            bool_size: 1,
            char_size: 4,     // UTF-32 codepoint
            min_alignment: 8,
        }
    }

    /// LLP64 platform (64-bit pointers, common on Windows)
    pub fn llp64() -> Self {
        PlatformSizes {
            pointer_size: 8,
            int_size: 8,
            float_size: 8,
            bool_size: 1,
            char_size: 4,
            min_alignment: 8,
        }
    }

    /// 32-bit platform
    pub fn ilp32() -> Self {
        PlatformSizes {
            pointer_size: 4,
            int_size: 4,
            float_size: 8,
            bool_size: 1,
            char_size: 4,
            min_alignment: 4,
        }
    }
}

/// Calculator for type sizes and layouts
pub struct TypeSizeCalculator {
    /// Platform configuration
    platform: PlatformSizes,

    /// Cache of computed sizes for named types
    cache: HashMap<String, usize>,
}

impl Default for TypeSizeCalculator {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeSizeCalculator {
    /// Create a new calculator with default platform sizes
    pub fn new() -> Self {
        TypeSizeCalculator {
            platform: PlatformSizes::default(),
            cache: HashMap::new(),
        }
    }

    /// Create a calculator with specific platform sizes
    pub fn with_platform(platform: PlatformSizes) -> Self {
        TypeSizeCalculator {
            platform,
            cache: HashMap::new(),
        }
    }

    /// Get the size of a type in bytes
    pub fn size_of(&self, ty: &Type) -> usize {
        match ty {
            // Primitives
            Type::Int => self.platform.int_size,
            Type::Float => self.platform.float_size,
            Type::Bool => self.platform.bool_size,
            Type::Void => 0,

            // String is a reference type (pointer + length + header pointer)
            Type::Str => self.string_size(),

            // Collections are reference types (pointer to heap data)
            Type::List(_) => self.list_size(),
            Type::Map(_, _) => self.map_size(),

            // Tuples are inline (sum of element sizes with padding)
            Type::Tuple(elems) => self.tuple_size(elems),

            // Structs depend on field sizes
            Type::Struct { fields, .. } => self.struct_size(fields),

            // Enums use discriminant + largest variant
            Type::Enum { variants, .. } => self.enum_size(variants),

            // Named types should be resolved, but we return pointer size as fallback
            Type::Named(name) => {
                if let Some(&size) = self.cache.get(name) {
                    size
                } else {
                    // Unknown named type - assume reference
                    self.platform.pointer_size
                }
            }

            // Function pointers
            Type::Function { .. } => self.platform.pointer_size,

            // Result<T, E> is typically a discriminant + max(sizeof(T), sizeof(E))
            Type::Result(ok, err) => self.result_size(ok, err),

            // Option<T> is typically a discriminant + sizeof(T)
            Type::Option(inner) => self.option_size(inner),

            // Records are like anonymous structs
            Type::Record(fields) => self.struct_size(fields),

            // Range is typically two values (start, end)
            Type::Range => self.platform.int_size * 2,

            // Any is a fat pointer (data + type info)
            Type::Any => self.platform.pointer_size * 2,

            // Trait objects are fat pointers
            Type::DynTrait(_) => self.platform.pointer_size * 2,
        }
    }

    /// Get alignment requirement for a type
    pub fn alignment_of(&self, ty: &Type) -> usize {
        match ty {
            Type::Int => self.platform.int_size.min(self.platform.min_alignment),
            Type::Float => self.platform.float_size.min(self.platform.min_alignment),
            Type::Bool => 1,
            Type::Void => 1,
            Type::Str | Type::List(_) | Type::Map(_, _) => self.platform.pointer_size,
            Type::Tuple(elems) => elems.iter().map(|e| self.alignment_of(e)).max().unwrap_or(1),
            Type::Struct { fields, .. } => {
                fields
                    .iter()
                    .map(|(_, ty)| self.alignment_of(ty))
                    .max()
                    .unwrap_or(1)
            }
            Type::Enum { .. } => self.platform.pointer_size, // Conservative
            Type::Named(_) => self.platform.pointer_size,
            Type::Function { .. } => self.platform.pointer_size,
            Type::Result(ok, err) => self.alignment_of(ok).max(self.alignment_of(err)),
            Type::Option(inner) => self.alignment_of(inner),
            Type::Record(fields) => {
                fields
                    .iter()
                    .map(|(_, ty)| self.alignment_of(ty))
                    .max()
                    .unwrap_or(1)
            }
            Type::Range => self.platform.int_size,
            Type::Any | Type::DynTrait(_) => self.platform.pointer_size,
        }
    }

    /// Check if a type is a reference type (requires ARC)
    pub fn is_reference_type(&self, ty: &Type) -> bool {
        match ty {
            // Primitives are always value types
            Type::Int | Type::Float | Type::Bool | Type::Void => false,

            // Strings and collections are always reference types
            Type::Str | Type::List(_) | Type::Map(_,_) => true,

            // Tuples depend on their contents
            Type::Tuple(elems) => elems.iter().any(|e| self.is_reference_type(e)),

            // Structs depend on their fields and size
            Type::Struct { fields, .. } => {
                self.struct_size(fields) > 32 || fields.iter().any(|(_, ty)| self.is_reference_type(ty))
            }

            // Enums are reference types if large or contain references
            Type::Enum { variants, .. } => {
                self.enum_size(variants) > 32
                    || variants.iter().any(|(_, fields)| {
                        fields.iter().any(|(_, ty)| self.is_reference_type(ty))
                    })
            }

            // Named types need to be resolved (conservative: assume reference)
            Type::Named(_) => true,

            // Functions are always reference types
            Type::Function { .. } => true,

            // Result/Option depend on contents
            Type::Result(ok, err) => self.is_reference_type(ok) || self.is_reference_type(err),
            Type::Option(inner) => self.is_reference_type(inner),

            // Records depend on size and contents
            Type::Record(fields) => {
                self.struct_size(fields) > 32 || fields.iter().any(|(_, ty)| self.is_reference_type(ty))
            }

            // Range is a value type
            Type::Range => false,

            // Any and trait objects are always references
            Type::Any | Type::DynTrait(_) => true,
        }
    }

    // Helper methods for specific type sizes

    fn string_size(&self) -> usize {
        // SigilString structure:
        // - Union of heap { data: *char, len: size_t, header: *ArcHeader }
        //   or sso { data: [char; 22], len: u8, flags: u8 }
        // For simplicity, use the max of both representations
        let heap_size = self.platform.pointer_size * 2 + self.platform.pointer_size;
        let sso_size = 22 + 1 + 1; // SSO threshold + len + flags
        heap_size.max(sso_size)
    }

    fn list_size(&self) -> usize {
        // List structure: data pointer + length + capacity + ARC header pointer
        self.platform.pointer_size * 4
    }

    fn map_size(&self) -> usize {
        // Map structure: buckets pointer + size + capacity + ARC header pointer
        self.platform.pointer_size * 4
    }

    fn tuple_size(&self, elems: &[Type]) -> usize {
        let mut size = 0;
        for elem in elems {
            let elem_size = self.size_of(elem);
            let elem_align = self.alignment_of(elem);
            // Align to element alignment
            size = align_up(size, elem_align);
            size += elem_size;
        }
        // Align final size to max alignment
        let max_align = elems
            .iter()
            .map(|e| self.alignment_of(e))
            .max()
            .unwrap_or(1);
        align_up(size, max_align)
    }

    fn struct_size(&self, fields: &[(String, Type)]) -> usize {
        let mut size = 0;
        for (_, ty) in fields {
            let field_size = self.size_of(ty);
            let field_align = self.alignment_of(ty);
            // Align to field alignment
            size = align_up(size, field_align);
            size += field_size;
        }
        // Align final size to max alignment
        let max_align = fields
            .iter()
            .map(|(_, ty)| self.alignment_of(ty))
            .max()
            .unwrap_or(1);
        align_up(size, max_align)
    }

    fn enum_size(&self, variants: &[(String, Vec<(String, Type)>)]) -> usize {
        // Discriminant (typically 1 byte for small enums, up to 4 bytes)
        let discriminant_size = if variants.len() <= 256 { 1 } else { 4 };

        // Find max variant size
        let max_variant_size = variants
            .iter()
            .map(|(_, fields)| self.struct_size(fields))
            .max()
            .unwrap_or(0);

        // Total size with alignment
        let payload_align = variants
            .iter()
            .flat_map(|(_, fields)| fields.iter())
            .map(|(_, ty)| self.alignment_of(ty))
            .max()
            .unwrap_or(1);

        let payload_offset = align_up(discriminant_size, payload_align);
        let total_size = payload_offset + max_variant_size;

        align_up(total_size, payload_align.max(discriminant_size))
    }

    fn result_size(&self, ok: &Type, err: &Type) -> usize {
        // Result<T, E> = { discriminant: u8, payload: max(T, E) }
        let ok_size = self.size_of(ok);
        let err_size = self.size_of(err);
        let max_size = ok_size.max(err_size);

        let max_align = self.alignment_of(ok).max(self.alignment_of(err));
        let discriminant_size = 1;
        let payload_offset = align_up(discriminant_size, max_align);

        align_up(payload_offset + max_size, max_align)
    }

    fn option_size(&self, inner: &Type) -> usize {
        // Option<T> = { discriminant: u8, payload: T }
        // Can be optimized for pointer types (null = None)
        let inner_size = self.size_of(inner);
        let inner_align = self.alignment_of(inner);

        // For pointer types, we can use null optimization
        if matches!(inner, Type::Str | Type::List(_) | Type::Map(_, _) | Type::Function { .. }) {
            return inner_size; // Null = None
        }

        let discriminant_size = 1;
        let payload_offset = align_up(discriminant_size, inner_align);

        align_up(payload_offset + inner_size, inner_align)
    }

    /// Register a named type's size in the cache
    pub fn register_named_type(&mut self, name: String, size: usize) {
        self.cache.insert(name, size);
    }
}

/// Align a size up to a given alignment
fn align_up(size: usize, align: usize) -> usize {
    if align == 0 {
        return size;
    }
    (size + align - 1) & !(align - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_sizes() {
        let calc = TypeSizeCalculator::new();
        assert_eq!(calc.size_of(&Type::Int), 8);
        assert_eq!(calc.size_of(&Type::Float), 8);
        assert_eq!(calc.size_of(&Type::Bool), 1);
        assert_eq!(calc.size_of(&Type::Void), 0);
    }

    #[test]
    fn test_tuple_size() {
        let calc = TypeSizeCalculator::new();
        let tuple = Type::Tuple(vec![Type::Int, Type::Bool, Type::Int]);
        // int (8) + padding (7) + bool (1) + padding (7) + int (8) = depends on alignment
        let size = calc.size_of(&tuple);
        assert!(size >= 17); // At least sum of sizes
    }

    #[test]
    fn test_struct_size() {
        let calc = TypeSizeCalculator::new();
        let ty = Type::Struct {
            name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Type::Int),
                ("y".to_string(), Type::Int),
            ],
        };
        assert_eq!(calc.size_of(&ty), 16); // Two 8-byte ints
    }

    #[test]
    fn test_reference_type_detection() {
        let calc = TypeSizeCalculator::new();

        // Value types
        assert!(!calc.is_reference_type(&Type::Int));
        assert!(!calc.is_reference_type(&Type::Bool));

        // Reference types
        assert!(calc.is_reference_type(&Type::Str));
        assert!(calc.is_reference_type(&Type::List(Box::new(Type::Int))));
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 8), 16);
        assert_eq!(align_up(7, 4), 8);
    }

    #[test]
    fn test_32bit_platform() {
        let calc = TypeSizeCalculator::with_platform(PlatformSizes::ilp32());
        assert_eq!(calc.size_of(&Type::Int), 4);
        // Pointer-based types should be 4 bytes on 32-bit
        assert_eq!(calc.platform.pointer_size, 4);
    }
}
