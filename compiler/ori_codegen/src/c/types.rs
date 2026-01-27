//! Type Mapping: Ori Types → C Types
//!
//! Maps Ori types to their C representations, with optimizations:
//!
//! - Primitives map directly to C types
//! - `Option<primitive>` uses unboxed tagged unions
//! - `Result<primitive, E>` uses unboxed tagged unions
//! - Strings use SSO (determined at runtime)

use ori_ir::{ParsedType, StringInterner, TypeId};
use ori_types::{TypeData, TypeInterner};

/// Type mapper for converting Ori types to C types.
pub struct CTypeMapper;

impl CTypeMapper {
    /// Map a `TypeId` to a C type string.
    pub fn map_type_id(type_id: TypeId, interner: &TypeInterner) -> String {
        // Handle pre-interned primitives directly
        match type_id {
            TypeId::INT => return "int64_t".to_string(),
            TypeId::FLOAT => return "double".to_string(),
            TypeId::BOOL => return "bool".to_string(),
            TypeId::STR => return "ori_string_t".to_string(),
            TypeId::CHAR => return "uint32_t".to_string(), // Unicode codepoint
            TypeId::BYTE => return "uint8_t".to_string(),
            TypeId::VOID | TypeId::NEVER => return "void".to_string(),
            TypeId::INFER | TypeId::SELF_TYPE => return "void*".to_string(),
            _ => {}
        }

        // Look up compound types
        let data = interner.lookup(type_id);
        Self::map_type_data(&data, interner)
    }

    /// Map `TypeData` to a C type string.
    fn map_type_data(data: &TypeData, interner: &TypeInterner) -> String {
        match data {
            // Primitives (shouldn't reach here due to early return above)
            TypeData::Int | TypeData::Duration => "int64_t".to_string(),
            TypeData::Float => "double".to_string(),
            TypeData::Bool => "bool".to_string(),
            TypeData::Str => "ori_string_t".to_string(),
            TypeData::Char => "uint32_t".to_string(),
            TypeData::Byte => "uint8_t".to_string(),
            TypeData::Unit | TypeData::Never => "void".to_string(),
            TypeData::Size => "uint64_t".to_string(), // Bytes
            TypeData::Error | TypeData::Var(_) | TypeData::Projection { .. } => "void*".to_string(),

            // Option types - use unboxed version for primitives
            TypeData::Option(inner) => Self::map_option(*inner, interner),

            // Result types - use unboxed version for primitives
            TypeData::Result { ok, err } => Self::map_result(*ok, *err, interner),

            // List type
            TypeData::List(_) => "ori_list_t".to_string(),

            // Map and Set types (Set is implemented as map with void values)
            TypeData::Map { .. } | TypeData::Set(_) => "ori_map_t".to_string(),

            // Range type (start, end, step)
            TypeData::Range(elem) => {
                let elem_type = Self::map_type_id(*elem, interner);
                format!("ori_range_{}_t", Self::type_suffix(&elem_type))
            }

            // Channel type
            TypeData::Channel(_) => "ori_channel_t*".to_string(),

            // Tuple type - use struct
            TypeData::Tuple(elems) => {
                if elems.is_empty() {
                    "void".to_string()
                } else {
                    // Generate unique tuple type name based on elements
                    let suffix: String = elems
                        .iter()
                        .map(|&e| Self::type_suffix(&Self::map_type_id(e, interner)))
                        .collect::<Vec<_>>()
                        .join("_");
                    format!("ori_tuple_{suffix}_t")
                }
            }

            // Function type - use function pointer
            TypeData::Function { params, ret } => {
                let ret_type = Self::map_type_id(*ret, interner);
                let param_types: Vec<_> = params
                    .iter()
                    .map(|&p| Self::map_type_id(p, interner))
                    .collect();

                if param_types.is_empty() {
                    format!("{ret_type} (*)(void)")
                } else {
                    format!("{ret_type} (*)({})", param_types.join(", "))
                }
            }

            // Named type - use the mangled name
            TypeData::Named(name) => {
                // For now, treat as opaque pointer
                // In the future, we should look up the actual type definition
                format!("struct ori_{}_s*", name.raw())
            }

            // Applied generic type
            TypeData::Applied { name, args } => {
                // Generate specialized type name
                let suffix: String = args
                    .iter()
                    .map(|&a| Self::type_suffix(&Self::map_type_id(a, interner)))
                    .collect::<Vec<_>>()
                    .join("_");
                format!("struct ori_{}_{}_s*", name.raw(), suffix)
            } // Note: Var(_), Projection { .. }, and Error are handled above
        }
    }

    /// Map an Option type, using unboxed representation for primitives.
    fn map_option(inner: TypeId, interner: &TypeInterner) -> String {
        // Check if inner type is a primitive that can be unboxed
        match inner {
            TypeId::INT => "ori_option_int_t".to_string(),
            TypeId::FLOAT => "ori_option_float_t".to_string(),
            TypeId::BOOL => "ori_option_bool_t".to_string(),
            TypeId::CHAR => "ori_option_char_t".to_string(),
            TypeId::BYTE => "ori_option_byte_t".to_string(),
            _ => {
                // Non-primitive Option - use generic boxed version
                let inner_type = Self::map_type_id(inner, interner);
                format!("ori_option_{}_t", Self::type_suffix(&inner_type))
            }
        }
    }

    /// Map a Result type, using unboxed representation for primitives.
    fn map_result(ok: TypeId, err: TypeId, interner: &TypeInterner) -> String {
        // Check if we have a pre-defined unboxed Result type
        let ok_suffix = match ok {
            TypeId::INT => "int",
            TypeId::FLOAT => "float",
            TypeId::BOOL => "bool",
            TypeId::VOID => "void",
            _ => "",
        };

        let err_suffix = if err == TypeId::STR { "str" } else { "" };

        if !ok_suffix.is_empty() && !err_suffix.is_empty() {
            format!("ori_result_{ok_suffix}_{err_suffix}_t")
        } else {
            // Generic Result type
            let ok_type = Self::map_type_id(ok, interner);
            let err_type = Self::map_type_id(err, interner);
            format!(
                "ori_result_{}_{}_t",
                Self::type_suffix(&ok_type),
                Self::type_suffix(&err_type)
            )
        }
    }

    /// Get a short suffix for a C type (for generating unique type names).
    fn type_suffix(c_type: &str) -> String {
        match c_type {
            "int64_t" => "i64".to_string(),
            "double" => "f64".to_string(),
            "bool" => "bool".to_string(),
            "uint32_t" => "u32".to_string(),
            "uint8_t" => "u8".to_string(),
            "void" => "void".to_string(),
            "ori_string_t" => "str".to_string(),
            "ori_list_t" => "list".to_string(),
            "ori_map_t" => "map".to_string(),
            "void*" => "ptr".to_string(),
            _ => {
                // Hash the type name for uniqueness
                let mut hash = 0u32;
                for b in c_type.bytes() {
                    hash = hash.wrapping_mul(31).wrapping_add(u32::from(b));
                }
                format!("t{hash:x}")
            }
        }
    }

    /// Map a parsed type annotation to a C type.
    pub fn map_parsed_type(parsed: &ParsedType, interner: &StringInterner) -> String {
        match parsed {
            ParsedType::Primitive(type_id) => match *type_id {
                TypeId::INT => "int64_t".to_string(),
                TypeId::FLOAT => "double".to_string(),
                TypeId::BOOL => "bool".to_string(),
                TypeId::STR => "ori_string_t".to_string(),
                TypeId::CHAR => "uint32_t".to_string(),
                TypeId::BYTE => "uint8_t".to_string(),
                TypeId::VOID | TypeId::NEVER => "void".to_string(),
                _ => "void*".to_string(),
            },

            ParsedType::Named { name, type_args } => {
                let name_str = interner.lookup(*name);
                if type_args.is_empty() {
                    // Non-generic named type
                    match name_str {
                        "int" | "Duration" => "int64_t".to_string(),
                        "float" => "double".to_string(),
                        "bool" => "bool".to_string(),
                        "str" => "ori_string_t".to_string(),
                        "char" => "uint32_t".to_string(),
                        "byte" => "uint8_t".to_string(),
                        "void" | "Never" => "void".to_string(),
                        "Size" => "uint64_t".to_string(),
                        _ => format!("struct ori_{name_str}_s*"),
                    }
                } else {
                    // Generic type with args
                    match name_str {
                        "Option" if type_args.len() == 1 => {
                            Self::map_option_parsed(&type_args[0], interner)
                        }
                        "Result" if type_args.len() == 2 => {
                            Self::map_result_parsed(&type_args[0], &type_args[1], interner)
                        }
                        "List" => "ori_list_t".to_string(),
                        "Map" | "Set" => "ori_map_t".to_string(),
                        "Channel" => "ori_channel_t*".to_string(),
                        "Range" => {
                            if let Some(elem) = type_args.first() {
                                let elem_type = Self::map_parsed_type(elem, interner);
                                format!("ori_range_{}_t", Self::type_suffix(&elem_type))
                            } else {
                                "ori_range_i64_t".to_string()
                            }
                        }
                        _ => format!("struct ori_{name_str}_s*"),
                    }
                }
            }

            ParsedType::Tuple(elems) => {
                if elems.is_empty() {
                    "void".to_string()
                } else {
                    let suffix: String = elems
                        .iter()
                        .map(|e| Self::type_suffix(&Self::map_parsed_type(e, interner)))
                        .collect::<Vec<_>>()
                        .join("_");
                    format!("ori_tuple_{suffix}_t")
                }
            }

            ParsedType::Function { params, ret } => {
                let ret_type = Self::map_parsed_type(ret, interner);
                let param_types: Vec<_> = params
                    .iter()
                    .map(|p| Self::map_parsed_type(p, interner))
                    .collect();

                if param_types.is_empty() {
                    format!("{ret_type} (*)(void)")
                } else {
                    format!("{ret_type} (*)({})", param_types.join(", "))
                }
            }

            ParsedType::List(elem) => {
                // List is always the same C type regardless of element
                let _ = Self::map_parsed_type(elem, interner);
                "ori_list_t".to_string()
            }

            ParsedType::Map { key, value } => {
                let _ = Self::map_parsed_type(key, interner);
                let _ = Self::map_parsed_type(value, interner);
                "ori_map_t".to_string()
            }

            ParsedType::SelfType | ParsedType::Infer | ParsedType::AssociatedType { .. } => {
                "void*".to_string()
            }
        }
    }

    /// Map a parsed Option type.
    fn map_option_parsed(inner: &ParsedType, interner: &StringInterner) -> String {
        if let ParsedType::Primitive(type_id) = inner {
            match *type_id {
                TypeId::INT => return "ori_option_int_t".to_string(),
                TypeId::FLOAT => return "ori_option_float_t".to_string(),
                TypeId::BOOL => return "ori_option_bool_t".to_string(),
                TypeId::CHAR => return "ori_option_char_t".to_string(),
                TypeId::BYTE => return "ori_option_byte_t".to_string(),
                _ => {}
            }
        }

        if let ParsedType::Named { name, type_args } = inner {
            if type_args.is_empty() {
                let name_str = interner.lookup(*name);
                match name_str {
                    "int" => return "ori_option_int_t".to_string(),
                    "float" => return "ori_option_float_t".to_string(),
                    "bool" => return "ori_option_bool_t".to_string(),
                    "char" => return "ori_option_char_t".to_string(),
                    "byte" => return "ori_option_byte_t".to_string(),
                    _ => {}
                }
            }
        }

        let inner_type = Self::map_parsed_type(inner, interner);
        format!("ori_option_{}_t", Self::type_suffix(&inner_type))
    }

    /// Map a parsed Result type.
    fn map_result_parsed(ok: &ParsedType, err: &ParsedType, interner: &StringInterner) -> String {
        let ok_suffix = Self::get_primitive_suffix(ok, interner);
        let err_suffix = Self::get_primitive_suffix(err, interner);

        if let (Some(ok_s), Some(err_s)) = (ok_suffix, err_suffix) {
            if err_s == "str" {
                return format!("ori_result_{ok_s}_{err_s}_t");
            }
        }

        let ok_type = Self::map_parsed_type(ok, interner);
        let err_type = Self::map_parsed_type(err, interner);
        format!(
            "ori_result_{}_{}_t",
            Self::type_suffix(&ok_type),
            Self::type_suffix(&err_type)
        )
    }

    /// Get the primitive suffix for a parsed type if it's a primitive.
    fn get_primitive_suffix(ty: &ParsedType, interner: &StringInterner) -> Option<&'static str> {
        match ty {
            ParsedType::Primitive(type_id) => match *type_id {
                TypeId::INT => Some("int"),
                TypeId::FLOAT => Some("float"),
                TypeId::BOOL => Some("bool"),
                TypeId::VOID => Some("void"),
                TypeId::STR => Some("str"),
                _ => None,
            },
            ParsedType::Named { name, type_args } if type_args.is_empty() => {
                let name_str = interner.lookup(*name);
                match name_str {
                    "int" => Some("int"),
                    "float" => Some("float"),
                    "bool" => Some("bool"),
                    "void" => Some("void"),
                    "str" => Some("str"),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_mapping() {
        let interner = TypeInterner::new();

        assert_eq!(CTypeMapper::map_type_id(TypeId::INT, &interner), "int64_t");
        assert_eq!(CTypeMapper::map_type_id(TypeId::FLOAT, &interner), "double");
        assert_eq!(CTypeMapper::map_type_id(TypeId::BOOL, &interner), "bool");
        assert_eq!(
            CTypeMapper::map_type_id(TypeId::STR, &interner),
            "ori_string_t"
        );
        assert_eq!(CTypeMapper::map_type_id(TypeId::VOID, &interner), "void");
    }

    #[test]
    fn test_option_unboxed() {
        let interner = TypeInterner::new();

        let opt_int = interner.option(TypeId::INT);
        let opt_float = interner.option(TypeId::FLOAT);

        assert_eq!(
            CTypeMapper::map_type_id(opt_int, &interner),
            "ori_option_int_t"
        );
        assert_eq!(
            CTypeMapper::map_type_id(opt_float, &interner),
            "ori_option_float_t"
        );
    }

    #[test]
    fn test_result_unboxed() {
        let interner = TypeInterner::new();

        let result_int_str = interner.result(TypeId::INT, TypeId::STR);
        let result_bool_str = interner.result(TypeId::BOOL, TypeId::STR);

        assert_eq!(
            CTypeMapper::map_type_id(result_int_str, &interner),
            "ori_result_int_str_t"
        );
        assert_eq!(
            CTypeMapper::map_type_id(result_bool_str, &interner),
            "ori_result_bool_str_t"
        );
    }

    #[test]
    fn test_list_mapping() {
        let interner = TypeInterner::new();

        let list_int = interner.list(TypeId::INT);
        let list_str = interner.list(TypeId::STR);

        // Lists are all the same C type (element type handled at runtime)
        assert_eq!(CTypeMapper::map_type_id(list_int, &interner), "ori_list_t");
        assert_eq!(CTypeMapper::map_type_id(list_str, &interner), "ori_list_t");
    }

    #[test]
    fn test_function_mapping() {
        let interner = TypeInterner::new();

        let fn_type = interner.function(vec![TypeId::INT, TypeId::BOOL], TypeId::STR);

        assert_eq!(
            CTypeMapper::map_type_id(fn_type, &interner),
            "ori_string_t (*)(int64_t, bool)"
        );
    }
}
