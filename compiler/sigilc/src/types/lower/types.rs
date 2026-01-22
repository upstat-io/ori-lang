// Type resolution utilities for AST to TIR lowering
// Converts TypeExpr to resolved Type

use crate::ast::TypeExpr;
use crate::ir::Type;
use crate::types::TypeContext;

/// Convert a TypeExpr to a resolved Type
pub fn type_expr_to_type(ty: &TypeExpr, ctx: &TypeContext) -> Result<Type, String> {
    match ty {
        TypeExpr::Named(name) => match name.as_str() {
            "int" => Ok(Type::Int),
            "float" => Ok(Type::Float),
            "bool" => Ok(Type::Bool),
            "str" => Ok(Type::Str),
            "void" => Ok(Type::Void),
            "any" => Ok(Type::Any),
            "Range" => Ok(Type::Range),
            // Single uppercase letters are type parameters - keep as named
            _ if is_type_param(name) => Ok(Type::Named(name.clone())),
            // User-defined types (or forward references)
            _ => Ok(Type::Named(name.clone())),
        },

        TypeExpr::Generic(name, args) => {
            let targs: Vec<Type> = args
                .iter()
                .map(|a| type_expr_to_type(a, ctx))
                .collect::<Result<Vec<_>, _>>()?;

            match name.as_str() {
                "Result" if targs.len() == 2 => Ok(Type::Result(
                    Box::new(targs[0].clone()),
                    Box::new(targs[1].clone()),
                )),
                "Option" if targs.len() == 1 => Ok(Type::Option(Box::new(targs[0].clone()))),
                "List" if targs.len() == 1 => Ok(Type::List(Box::new(targs[0].clone()))),
                "Map" if targs.len() == 2 => Ok(Type::Map(
                    Box::new(targs[0].clone()),
                    Box::new(targs[1].clone()),
                )),
                _ => Ok(Type::Named(name.clone())), // User-defined generic type
            }
        }

        TypeExpr::Optional(inner) => {
            let inner_ty = type_expr_to_type(inner, ctx)?;
            Ok(Type::Option(Box::new(inner_ty)))
        }

        TypeExpr::List(inner) => {
            let inner_ty = type_expr_to_type(inner, ctx)?;
            Ok(Type::List(Box::new(inner_ty)))
        }

        TypeExpr::Map(key, value) => {
            let key_ty = type_expr_to_type(key, ctx)?;
            let value_ty = type_expr_to_type(value, ctx)?;
            Ok(Type::Map(Box::new(key_ty), Box::new(value_ty)))
        }

        TypeExpr::Tuple(elems) => {
            let elem_types: Vec<Type> = elems
                .iter()
                .map(|e| type_expr_to_type(e, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Type::Tuple(elem_types))
        }

        TypeExpr::Function(param, ret) => {
            let param_ty = type_expr_to_type(param, ctx)?;
            let ret_ty = type_expr_to_type(ret, ctx)?;

            // If param is a tuple, expand it to multiple params
            let params = match param_ty {
                Type::Tuple(types) => types,
                _ => vec![param_ty],
            };

            Ok(Type::Function {
                params,
                ret: Box::new(ret_ty),
            })
        }

        TypeExpr::Record(fields) => {
            let field_types: Vec<(String, Type)> = fields
                .iter()
                .map(|(name, ty)| Ok((name.clone(), type_expr_to_type(ty, ctx)?)))
                .collect::<Result<Vec<_>, String>>()?;
            Ok(Type::Record(field_types))
        }

        TypeExpr::DynTrait(trait_name) => Ok(Type::DynTrait(trait_name.clone())),
    }
}

/// Check if a name is a type parameter (single uppercase letter)
pub fn is_type_param(name: &str) -> bool {
    name.len() == 1
        && name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
}

/// Check if a function name is a builtin
pub fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "print"
            | "str"
            | "int"
            | "float"
            | "len"
            | "assert"
            | "assert_eq"
            | "assert_err"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_expr_to_type_primitives() {
        let ctx = TypeContext::new();
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("int".to_string()), &ctx).unwrap(),
            Type::Int
        );
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("str".to_string()), &ctx).unwrap(),
            Type::Str
        );
        assert_eq!(
            type_expr_to_type(&TypeExpr::Named("bool".to_string()), &ctx).unwrap(),
            Type::Bool
        );
    }

    #[test]
    fn test_type_expr_to_type_list() {
        let ctx = TypeContext::new();
        let list_ty = TypeExpr::List(Box::new(TypeExpr::Named("int".to_string())));
        assert_eq!(
            type_expr_to_type(&list_ty, &ctx).unwrap(),
            Type::List(Box::new(Type::Int))
        );
    }

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("print"));
        assert!(is_builtin("+"));
        assert!(!is_builtin("foo"));
    }

    #[test]
    fn test_is_type_param() {
        assert!(is_type_param("T"));
        assert!(is_type_param("A"));
        assert!(!is_type_param("int"));
        assert!(!is_type_param("TT"));
    }
}
