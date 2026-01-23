// Type mapping for TIR-based C code generation
// Converts resolved Type to C types - no inference needed
//
// Also provides ARC (Automatic Reference Counting) support for
// determining which types need retain/release operations.

use super::TirCodeGen;
use crate::arc::is_value_type;
use crate::ir::Type;

impl TirCodeGen {
    /// Convert a resolved Type to its C representation
    pub(super) fn type_to_c(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "int64_t".to_string(),
            Type::Float => "double".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Str => "String".to_string(),
            Type::Void => "void".to_string(),
            Type::Any => "void*".to_string(),
            Type::Range => "Range".to_string(),

            Type::List(inner) => {
                let inner_c = self.type_to_c(inner);
                format!("Array_{}", inner_c)
            }

            Type::Map(key, value) => {
                let key_c = self.type_to_c(key);
                let value_c = self.type_to_c(value);
                format!("Map_{}_{}", key_c, value_c)
            }

            Type::Tuple(elems) => {
                if elems.is_empty() {
                    "void".to_string()
                } else {
                    // Generate a tuple struct name
                    let types: Vec<_> = elems.iter().map(|t| self.type_to_c(t)).collect();
                    format!("Tuple_{}", types.join("_"))
                }
            }

            Type::Struct { name, .. } => name.clone(),

            Type::Enum { name, .. } => name.clone(),

            Type::Named(name) => match name.as_str() {
                "int" => "int64_t".to_string(),
                "float" => "double".to_string(),
                "bool" => "bool".to_string(),
                "str" => "String".to_string(),
                "void" => "void".to_string(),
                other => other.to_string(),
            },

            Type::Function { ret, .. } => {
                // For function pointers
                let ret_c = self.type_to_c(ret);
                format!("{}(*)", ret_c)
            }

            Type::Result(ok, err) => {
                let ok_c = self.type_to_c(ok);
                let err_c = self.type_to_c(err);
                format!("Result_{}_{}", ok_c, err_c)
            }

            Type::Option(inner) => {
                let inner_c = self.type_to_c(inner);
                format!("Option_{}", inner_c)
            }

            Type::Record(fields) => {
                // Anonymous record - generate a struct name from fields
                let field_names: Vec<_> = fields.iter().map(|(n, _)| n.as_str()).collect();
                format!("Record_{}", field_names.join("_"))
            }

            Type::DynTrait(trait_name) => {
                // Trait object - represented as a pointer to vtable struct
                format!("Dyn{}*", trait_name)
            }
        }
    }

    /// Check if a type is a string type
    pub(super) fn is_string_type(&self, ty: &Type) -> bool {
        matches!(ty, Type::Str)
    }

    /// Get the default value for a type
    pub(super) fn default_value(&self, ty: &Type) -> String {
        match ty {
            Type::Int => "0".to_string(),
            Type::Float => "0.0".to_string(),
            Type::Bool => "false".to_string(),
            Type::Str => "str_new(\"\")".to_string(),
            Type::Void => "".to_string(),
            Type::List(_) => "NULL".to_string(), // TODO: proper list init
            Type::Option(_) => "NULL".to_string(),
            Type::Result(_, _) => "NULL".to_string(),
            _ => "NULL".to_string(),
        }
    }

    // =========================================================================
    // ARC Memory Management Helpers
    // =========================================================================

    /// Check if a type requires ARC management
    ///
    /// Returns true for reference types (strings, lists, maps, etc.)
    /// that need retain/release operations.
    pub(super) fn needs_arc(&self, ty: &Type) -> bool {
        if !self.use_arc {
            return false;
        }
        !is_value_type(ty)
    }

    /// Emit a retain call for a variable if the type requires it
    ///
    /// Returns the C code for retaining the reference, or an empty string
    /// if the type is a value type.
    pub(super) fn emit_retain(&self, var: &str, ty: &Type) -> String {
        if !self.needs_arc(ty) {
            return String::new();
        }

        match ty {
            Type::Str => format!("sigil_string_retain(&{})", var),
            Type::List(_) => format!("sigil_arc_retain({}.header)", var),
            Type::Map(_, _) => format!("sigil_arc_retain({}.header)", var),
            _ => format!("sigil_arc_retain((void*){})", var),
        }
    }

    /// Emit a release call for a variable if the type requires it
    ///
    /// Returns the C code for releasing the reference, or an empty string
    /// if the type is a value type.
    pub(super) fn emit_release(&self, var: &str, ty: &Type) -> String {
        if !self.needs_arc(ty) {
            return String::new();
        }

        match ty {
            Type::Str => format!("sigil_string_release(&{})", var),
            Type::List(_) => format!("sigil_arc_release({}.header)", var),
            Type::Map(_, _) => format!("sigil_arc_release({}.header)", var),
            _ => format!("sigil_arc_release((void*){})", var),
        }
    }

    /// Check if a type is a string (for special string handling)
    pub(super) fn is_arc_string(&self, ty: &Type) -> bool {
        matches!(ty, Type::Str)
    }

    /// Check if a type is a collection (list or map)
    pub(super) fn is_arc_collection(&self, ty: &Type) -> bool {
        matches!(ty, Type::List(_) | Type::Map(_, _))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_to_c_primitives() {
        let cg = TirCodeGen::new();
        assert_eq!(cg.type_to_c(&Type::Int), "int64_t");
        assert_eq!(cg.type_to_c(&Type::Float), "double");
        assert_eq!(cg.type_to_c(&Type::Bool), "bool");
        assert_eq!(cg.type_to_c(&Type::Str), "String");
        assert_eq!(cg.type_to_c(&Type::Void), "void");
    }

    #[test]
    fn test_type_to_c_collections() {
        let cg = TirCodeGen::new();
        assert_eq!(
            cg.type_to_c(&Type::List(Box::new(Type::Int))),
            "Array_int64_t"
        );
        assert_eq!(
            cg.type_to_c(&Type::Map(Box::new(Type::Str), Box::new(Type::Int))),
            "Map_String_int64_t"
        );
    }
}
