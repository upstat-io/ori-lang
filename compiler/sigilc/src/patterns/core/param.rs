// Parameter specification for patterns
//
// Provides enhanced parameter definitions with type constraints
// for better validation and documentation.

use crate::ast::TypeExpr;

/// Type constraint for a pattern parameter.
#[derive(Debug, Clone)]
pub enum TypeConstraint {
    /// Any type is allowed
    Any,
    /// Must be a specific primitive type
    Primitive(&'static str),
    /// Must be a list of any element type
    List,
    /// Must be a list with specific element type
    ListOf(Box<TypeConstraint>),
    /// Must be iterable (list, range, or iterator)
    Iterable,
    /// Must be a function type
    Function,
    /// Must be a function with specific arity
    FunctionArity(usize),
    /// Must be a numeric type (int or float)
    Numeric,
    /// Must be a boolean
    Boolean,
    /// Must match the type of another parameter
    SameAs(&'static str),
    /// Must be the element type of another parameter (which is a list)
    ElementOf(&'static str),
    /// Must be a function that takes element and accumulator and returns accumulator
    FoldFunction(&'static str, &'static str),
    /// Custom constraint with description
    Custom(&'static str),
}

impl TypeConstraint {
    /// Check if a type satisfies this constraint.
    ///
    /// Note: Full type checking requires context, so this is a preliminary check.
    pub fn matches_type(&self, ty: &TypeExpr) -> bool {
        match self {
            TypeConstraint::Any => true,
            TypeConstraint::Primitive(name) => {
                matches!(ty, TypeExpr::Named(n) if n == *name)
            }
            TypeConstraint::List | TypeConstraint::ListOf(_) => {
                matches!(ty, TypeExpr::List(_))
            }
            TypeConstraint::Iterable => {
                match ty {
                    TypeExpr::List(_) => true,
                    TypeExpr::Generic(name, _) => name == "Range" || name == "Iterator",
                    _ => false,
                }
            }
            TypeConstraint::Function | TypeConstraint::FunctionArity(_) => {
                matches!(ty, TypeExpr::Function(_, _))
            }
            TypeConstraint::Numeric => {
                matches!(ty, TypeExpr::Named(n) if n == "int" || n == "float")
            }
            TypeConstraint::Boolean => {
                matches!(ty, TypeExpr::Named(n) if n == "bool")
            }
            TypeConstraint::SameAs(_)
            | TypeConstraint::ElementOf(_)
            | TypeConstraint::FoldFunction(_, _)
            | TypeConstraint::Custom(_) => {
                // These require context to validate
                true
            }
        }
    }

    /// Get a human-readable description of this constraint.
    pub fn description(&self) -> String {
        match self {
            TypeConstraint::Any => "any type".to_string(),
            TypeConstraint::Primitive(name) => format!("{}", name),
            TypeConstraint::List => "list".to_string(),
            TypeConstraint::ListOf(inner) => format!("[{}]", inner.description()),
            TypeConstraint::Iterable => "iterable (list, range, or iterator)".to_string(),
            TypeConstraint::Function => "function".to_string(),
            TypeConstraint::FunctionArity(n) => format!("function with {} parameters", n),
            TypeConstraint::Numeric => "numeric (int or float)".to_string(),
            TypeConstraint::Boolean => "bool".to_string(),
            TypeConstraint::SameAs(param) => format!("same type as {}", param),
            TypeConstraint::ElementOf(param) => format!("element type of {}", param),
            TypeConstraint::FoldFunction(elem, acc) => {
                format!("function ({}, {}) -> {}", elem, acc, acc)
            }
            TypeConstraint::Custom(desc) => desc.to_string(),
        }
    }
}

/// Enhanced parameter specification for patterns.
#[derive(Debug, Clone)]
pub struct ParamSpec {
    /// The parameter name (with leading dot, e.g., ".over")
    pub name: &'static str,
    /// Whether this parameter is required
    pub required: bool,
    /// Human-readable description
    pub description: &'static str,
    /// Type constraint for validation
    pub constraint: TypeConstraint,
    /// Default value expression (for optional params)
    pub default: Option<&'static str>,
}

impl ParamSpec {
    /// Create a required parameter with any type.
    pub const fn required(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: true,
            description,
            constraint: TypeConstraint::Any,
            default: None,
        }
    }

    /// Create a required parameter with a type constraint.
    pub const fn required_with(
        name: &'static str,
        description: &'static str,
        constraint: TypeConstraint,
    ) -> Self {
        ParamSpec {
            name,
            required: true,
            description,
            constraint,
            default: None,
        }
    }

    /// Create an optional parameter with any type.
    pub const fn optional(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Any,
            default: None,
        }
    }

    /// Create an optional parameter with a type constraint.
    pub const fn optional_with(
        name: &'static str,
        description: &'static str,
        constraint: TypeConstraint,
    ) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint,
            default: None,
        }
    }

    /// Create an optional parameter with a default value.
    pub const fn optional_default(
        name: &'static str,
        description: &'static str,
        default: &'static str,
    ) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Any,
            default: Some(default),
        }
    }

    /// Create an optional boolean flag (defaults to false).
    pub const fn flag(name: &'static str, description: &'static str) -> Self {
        ParamSpec {
            name,
            required: false,
            description,
            constraint: TypeConstraint::Boolean,
            default: Some("false"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_constraint_matches() {
        let int_ty = TypeExpr::Named("int".to_string());
        let float_ty = TypeExpr::Named("float".to_string());
        let str_ty = TypeExpr::Named("str".to_string());
        let list_ty = TypeExpr::List(Box::new(TypeExpr::Named("int".to_string())));

        assert!(TypeConstraint::Any.matches_type(&int_ty));
        assert!(TypeConstraint::Numeric.matches_type(&int_ty));
        assert!(TypeConstraint::Numeric.matches_type(&float_ty));
        assert!(!TypeConstraint::Numeric.matches_type(&str_ty));
        assert!(TypeConstraint::List.matches_type(&list_ty));
        assert!(!TypeConstraint::List.matches_type(&int_ty));
    }

    #[test]
    fn test_param_spec_required() {
        let param = ParamSpec::required(".over", "collection to fold");
        assert!(param.required);
        assert_eq!(param.name, ".over");
    }

    #[test]
    fn test_param_spec_optional() {
        let param = ParamSpec::optional(".timeout", "optional timeout");
        assert!(!param.required);
    }

    #[test]
    fn test_param_spec_flag() {
        let param = ParamSpec::flag(".memo", "enable memoization");
        assert!(!param.required);
        assert_eq!(param.default, Some("false"));
    }

    #[test]
    fn test_constraint_description() {
        assert_eq!(TypeConstraint::Numeric.description(), "numeric (int or float)");
        assert_eq!(TypeConstraint::List.description(), "list");
    }
}
