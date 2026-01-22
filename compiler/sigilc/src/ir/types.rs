// Resolved types for Sigil TIR (Typed Intermediate Representation)
// These types are fully resolved - no inference placeholders or type variables

use std::fmt;

/// Fully resolved type (no inference placeholders)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    // Primitives
    Int,
    Float,
    Bool,
    Str,
    Void,

    // Collections
    List(Box<Type>),
    Map(Box<Type>, Box<Type>),
    Tuple(Vec<Type>),

    // User-defined types
    Struct {
        name: String,
        fields: Vec<(String, Type)>,
    },
    Enum {
        name: String,
        variants: Vec<(String, Vec<(String, Type)>)>,
    },

    // Type alias (resolved to its underlying type during lowering)
    Named(String),

    // Function type
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },

    // Result/Option types
    Result(Box<Type>, Box<Type>),
    Option(Box<Type>),

    // Anonymous record type: { field1: T1, field2: T2 }
    Record(Vec<(String, Type)>),

    // Range type (for iteration)
    Range,

    // Any type (for builtins that accept any type)
    Any,

    // Dynamic trait object: dyn Trait
    // Represents a type-erased pointer to any type implementing the trait
    DynTrait(String),
}

impl Type {
    /// Check if this type is numeric (int or float)
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }

    /// Check if this type is a primitive type
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            Type::Int | Type::Float | Type::Bool | Type::Str | Type::Void
        )
    }

    /// Get the element type of a list, if this is a list type
    pub fn list_elem(&self) -> Option<&Type> {
        match self {
            Type::List(elem) => Some(elem),
            _ => None,
        }
    }

    /// Get the key and value types of a map, if this is a map type
    pub fn map_types(&self) -> Option<(&Type, &Type)> {
        match self {
            Type::Map(k, v) => Some((k, v)),
            _ => None,
        }
    }

    /// Check if this is a function type
    pub fn is_function(&self) -> bool {
        matches!(self, Type::Function { .. })
    }

    /// Check if this is a result type
    pub fn is_result(&self) -> bool {
        matches!(self, Type::Result(_, _))
    }

    /// Check if this is an option type
    pub fn is_option(&self) -> bool {
        matches!(self, Type::Option(_))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "str"),
            Type::Void => write!(f, "void"),
            Type::List(elem) => write!(f, "[{}]", elem),
            Type::Map(k, v) => write!(f, "{{{}: {}}}", k, v),
            Type::Tuple(elems) => {
                write!(f, "(")?;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", elem)?;
                }
                write!(f, ")")
            }
            Type::Struct { name, .. } => write!(f, "{}", name),
            Type::Enum { name, .. } => write!(f, "{}", name),
            Type::Named(name) => write!(f, "{}", name),
            Type::Function { params, ret } => {
                write!(f, "(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            Type::Option(inner) => write!(f, "?{}", inner),
            Type::Record(fields) => {
                write!(f, "{{ ")?;
                for (i, (name, ty)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", name, ty)?;
                }
                write!(f, " }}")
            }
            Type::Range => write!(f, "Range"),
            Type::Any => write!(f, "any"),
            Type::DynTrait(trait_name) => write!(f, "dyn {}", trait_name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_display() {
        assert_eq!(Type::Int.to_string(), "int");
        assert_eq!(Type::List(Box::new(Type::Int)).to_string(), "[int]");
        assert_eq!(
            Type::Function {
                params: vec![Type::Int, Type::Int],
                ret: Box::new(Type::Int)
            }
            .to_string(),
            "(int, int) -> int"
        );
    }

    #[test]
    fn test_type_is_numeric() {
        assert!(Type::Int.is_numeric());
        assert!(Type::Float.is_numeric());
        assert!(!Type::Bool.is_numeric());
        assert!(!Type::Str.is_numeric());
    }
}
