// AST to TIR lowering for Sigil
// Combines type checking with IR building
//
// This module converts a type-checked AST to the Typed Intermediate Representation (TIR).
// Every expression in the TIR carries its resolved type.
//
// The module is split into focused submodules:
// - types.rs: Type resolution utilities
// - expr.rs: Expression lowering
// - patterns.rs: Match patterns and pattern expressions

mod captures;
mod check_lower;
mod expr;
mod patterns;
mod types;

// Re-export public API
pub use types::{is_builtin, is_type_param, type_expr_to_type};

use crate::ast::{self, Module};
use crate::ir::{
    LocalId, LocalTable, TConfig, TField, TFunction, TImport, TImportItem,
    TModule, TParam, TTest, TTypeDef, TTypeDefKind, TVariant,
};
use crate::types::compat::infer_type;
use crate::types::TypeContext;
use std::collections::HashMap;

/// Lowerer converts AST to TIR
pub struct Lowerer {
    pub(crate) ctx: TypeContext,
    pub(crate) locals: LocalTable,
    pub(crate) local_scope: HashMap<String, LocalId>,
    pub(crate) param_indices: HashMap<String, usize>,
}

impl Lowerer {
    pub fn new(ctx: &TypeContext) -> Self {
        Lowerer {
            ctx: ctx.child(),
            locals: LocalTable::new(),
            local_scope: HashMap::new(),
            param_indices: HashMap::new(),
        }
    }

    /// Lower a complete module to TIR
    pub fn lower_module(module: &Module, ctx: &TypeContext) -> Result<TModule, String> {
        let mut tmodule = TModule::new(module.name.clone());

        // First pass: lower type definitions
        for item in &module.items {
            if let ast::Item::TypeDef(td) = item {
                tmodule.types.push(Self::lower_typedef(td, ctx)?);
            }
        }

        // Second pass: lower imports
        for item in &module.items {
            if let ast::Item::Use(u) = item {
                tmodule.imports.push(TImport {
                    path: u.path.clone(),
                    items: u
                        .items
                        .iter()
                        .map(|i| TImportItem {
                            name: i.name.clone(),
                            alias: i.alias.clone(),
                        })
                        .collect(),
                    span: u.span.clone(),
                });
            }
        }

        // Third pass: lower configs
        for item in &module.items {
            if let ast::Item::Config(cd) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.configs.push(lowerer.lower_config(cd)?);
            }
        }

        // Fourth pass: lower functions
        for item in &module.items {
            if let ast::Item::Function(fd) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.functions.push(lowerer.lower_function(fd)?);
            }
        }

        // Fifth pass: lower tests
        for item in &module.items {
            if let ast::Item::Test(td) = item {
                let mut lowerer = Lowerer::new(ctx);
                tmodule.tests.push(lowerer.lower_test(td)?);
            }
        }

        Ok(tmodule)
    }

    /// Lower a type definition
    fn lower_typedef(td: &ast::TypeDef, ctx: &TypeContext) -> Result<TTypeDef, String> {
        let kind = match &td.kind {
            ast::TypeDefKind::Alias(ty) => TTypeDefKind::Alias(type_expr_to_type(ty, ctx)?),
            ast::TypeDefKind::Struct(fields) => {
                let tfields = fields
                    .iter()
                    .map(|f| {
                        Ok(TField {
                            name: f.name.clone(),
                            ty: type_expr_to_type(&f.ty, ctx)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                TTypeDefKind::Struct(tfields)
            }
            ast::TypeDefKind::Enum(variants) => {
                let tvariants = variants
                    .iter()
                    .map(|v| {
                        let fields = v
                            .fields
                            .iter()
                            .map(|f| {
                                Ok(TField {
                                    name: f.name.clone(),
                                    ty: type_expr_to_type(&f.ty, ctx)?,
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        Ok(TVariant {
                            name: v.name.clone(),
                            fields,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                TTypeDefKind::Enum(tvariants)
            }
        };

        Ok(TTypeDef {
            name: td.name.clone(),
            public: td.public,
            params: td.params.clone(),
            kind,
            span: td.span.clone(),
        })
    }

    /// Lower a config definition
    fn lower_config(&mut self, cd: &ast::ConfigDef) -> Result<TConfig, String> {
        let ty = if let Some(ref t) = cd.ty {
            type_expr_to_type(t, &self.ctx)?
        } else {
            let inferred = infer_type(&cd.value.expr)?;
            type_expr_to_type(&inferred, &self.ctx)?
        };

        let value = self.lower_spanned_expr(&cd.value)?;

        Ok(TConfig {
            name: cd.name.clone(),
            ty,
            value,
            span: cd.span.clone(),
        })
    }

    /// Lower a function definition
    fn lower_function(&mut self, fd: &ast::FunctionDef) -> Result<TFunction, String> {
        // Setup parameter indices and add parameters to type context
        for (i, param) in fd.params.iter().enumerate() {
            self.param_indices.insert(param.name.clone(), i);
            // Add parameter to context so check_expr can find it (parameters are immutable)
            self.ctx.define_local(param.name.clone(), param.ty.clone(), false);
        }

        // Set return type for self() calls in recurse patterns
        self.ctx.set_current_return_type(fd.return_type.clone());

        // Convert parameters
        let params: Vec<TParam> = fd
            .params
            .iter()
            .map(|p| {
                Ok(TParam {
                    name: p.name.clone(),
                    ty: type_expr_to_type(&p.ty, &self.ctx)?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;

        let return_type = type_expr_to_type(&fd.return_type, &self.ctx)?;

        // Lower the body expression (with span from SpannedExpr)
        let body = self.lower_spanned_expr(&fd.body)?;

        Ok(TFunction {
            name: fd.name.clone(),
            public: fd.public,
            params,
            return_type,
            locals: std::mem::take(&mut self.locals),
            body,
            span: fd.span.clone(),
        })
    }

    /// Lower a test definition
    fn lower_test(&mut self, td: &ast::TestDef) -> Result<TTest, String> {
        let body = self.lower_spanned_expr(&td.body)?;

        Ok(TTest {
            name: td.name.clone(),
            target: td.target.clone(),
            locals: std::mem::take(&mut self.locals),
            body,
            span: td.span.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Type;

    #[test]
    fn test_type_expr_to_type_primitives() {
        let ctx = TypeContext::new();
        assert_eq!(
            type_expr_to_type(&ast::TypeExpr::Named("int".to_string()), &ctx).unwrap(),
            Type::Int
        );
        assert_eq!(
            type_expr_to_type(&ast::TypeExpr::Named("str".to_string()), &ctx).unwrap(),
            Type::Str
        );
        assert_eq!(
            type_expr_to_type(&ast::TypeExpr::Named("bool".to_string()), &ctx).unwrap(),
            Type::Bool
        );
    }

    #[test]
    fn test_type_expr_to_type_list() {
        let ctx = TypeContext::new();
        let list_ty = ast::TypeExpr::List(Box::new(ast::TypeExpr::Named("int".to_string())));
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
}
