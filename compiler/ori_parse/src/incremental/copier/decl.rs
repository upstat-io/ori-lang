//! Declaration copying for the AST copier.
//!
//! Contains function, test, type, const, extern, extension import, and use
//! declaration copying, plus generic param and where clause helpers.
//! Trait/impl/def-impl/extend copying lives in `trait_impl.rs`.

use super::AstCopier;
use ori_ir::{
    CapabilityRef, ConstDef, ExprArena, ExternBlock, ExternItem, ExternParam, Function,
    GenericParam, PostContract, PreContract, TestDef, TypeDecl, UseDef, WhereClause,
};

impl AstCopier<'_> {
    /// Copy a function declaration.
    pub fn copy_function(&self, func: &Function, new_arena: &mut ExprArena) -> Function {
        let old_generics = self.old_arena.get_generic_params(func.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let old_params = self.old_arena.get_params(func.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = func
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        let new_pre_contracts = func
            .pre_contracts
            .iter()
            .map(|c| PreContract {
                condition: self.copy_expr(c.condition, new_arena),
                message: c.message,
                span: self.adjust_span(c.span),
            })
            .collect();

        let new_post_contracts = func
            .post_contracts
            .iter()
            .map(|c| PostContract {
                params: c.params.clone(),
                condition: self.copy_expr(c.condition, new_arena),
                message: c.message,
                span: self.adjust_span(c.span),
            })
            .collect();

        Function {
            name: func.name,
            generics: new_arena.alloc_generic_params(new_generics),
            params: new_arena.alloc_params(new_params),
            return_ty: func
                .return_ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            capabilities: func
                .capabilities
                .iter()
                .map(|c| CapabilityRef {
                    name: c.name,
                    span: self.adjust_span(c.span),
                })
                .collect(),
            where_clauses: new_where_clauses,
            guard: func.guard.map(|g| self.copy_expr(g, new_arena)),
            pre_contracts: new_pre_contracts,
            post_contracts: new_post_contracts,
            body: self.copy_expr(func.body, new_arena),
            span: self.adjust_span(func.span),
            visibility: func.visibility,
            is_fbip: func.is_fbip,
            target_attr: func.target_attr.clone(),
            cfg_attr: func.cfg_attr.clone(),
        }
    }

    /// Copy a test definition.
    pub fn copy_test(&self, test: &TestDef, new_arena: &mut ExprArena) -> TestDef {
        let old_params = self.old_arena.get_params(test.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TestDef {
            name: test.name,
            targets: test.targets.clone(),
            params: new_arena.alloc_params(new_params),
            return_ty: test
                .return_ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            body: self.copy_expr(test.body, new_arena),
            span: self.adjust_span(test.span),
            skip_reason: test.skip_reason,
            expected_errors: test.expected_errors.clone(),
            fail_expected: test.fail_expected,
        }
    }

    /// Copy a type declaration.
    pub fn copy_type_decl(&self, decl: &TypeDecl, new_arena: &mut ExprArena) -> TypeDecl {
        let old_generics = self.old_arena.get_generic_params(decl.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = decl
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        TypeDecl {
            name: decl.name,
            generics: new_arena.alloc_generic_params(new_generics),
            where_clauses: new_where_clauses,
            kind: self.copy_type_decl_kind(&decl.kind, new_arena),
            span: self.adjust_span(decl.span),
            visibility: decl.visibility,
            derives: decl.derives.clone(),
            repr_attrs: decl.repr_attrs.clone(),
            target_attr: decl.target_attr.clone(),
            cfg_attr: decl.cfg_attr.clone(),
        }
    }

    /// Copy a type declaration kind.
    fn copy_type_decl_kind(
        &self,
        kind: &ori_ir::TypeDeclKind,
        new_arena: &mut ExprArena,
    ) -> ori_ir::TypeDeclKind {
        match kind {
            ori_ir::TypeDeclKind::Struct(fields) => {
                let new_fields: Vec<_> = fields
                    .iter()
                    .map(|f| ori_ir::StructField {
                        name: f.name,
                        ty: self.copy_parsed_type(&f.ty, new_arena),
                        span: self.adjust_span(f.span),
                    })
                    .collect();
                ori_ir::TypeDeclKind::Struct(new_fields)
            }
            ori_ir::TypeDeclKind::Sum(variants) => {
                let new_variants: Vec<_> = variants
                    .iter()
                    .map(|v| ori_ir::Variant {
                        name: v.name,
                        fields: v
                            .fields
                            .iter()
                            .map(|f| ori_ir::VariantField {
                                name: f.name,
                                ty: self.copy_parsed_type(&f.ty, new_arena),
                                span: self.adjust_span(f.span),
                            })
                            .collect(),
                        span: self.adjust_span(v.span),
                    })
                    .collect();
                ori_ir::TypeDeclKind::Sum(new_variants)
            }
            ori_ir::TypeDeclKind::Newtype(ty) => {
                ori_ir::TypeDeclKind::Newtype(self.copy_parsed_type(ty, new_arena))
            }
        }
    }

    /// Copy a constant definition.
    pub fn copy_const(&self, const_def: &ConstDef, new_arena: &mut ExprArena) -> ConstDef {
        ConstDef {
            name: const_def.name,
            ty: const_def
                .ty
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            value: self.copy_expr(const_def.value, new_arena),
            span: self.adjust_span(const_def.span),
            visibility: const_def.visibility,
            target_attr: const_def.target_attr.clone(),
            cfg_attr: const_def.cfg_attr.clone(),
        }
    }

    /// Copy an extension import definition.
    ///
    /// Extension imports are pure data (no `ExprId` children), so only spans
    /// need adjustment; the rest is cloned directly.
    pub fn copy_extension_import(
        &self,
        ext_import: &ori_ir::ExtensionImport,
    ) -> ori_ir::ExtensionImport {
        ori_ir::ExtensionImport {
            path: ext_import.path.clone(),
            items: ext_import
                .items
                .iter()
                .map(|item| ori_ir::ExtensionImportItem {
                    type_name: item.type_name,
                    method_name: item.method_name,
                    span: self.adjust_span(item.span),
                })
                .collect(),
            visibility: ext_import.visibility,
            span: self.adjust_span(ext_import.span),
        }
    }

    /// Copy an extern block, adjusting spans and deep-copying parsed types.
    ///
    /// Extern blocks have no `ExprId` children but do contain `ParsedType`
    /// fields that reference arena-allocated compound types (e.g., `[float]`,
    /// `Option<CPtr>`). These must be deep-copied to avoid dangling references
    /// in the new arena.
    pub fn copy_extern_block(&self, block: &ExternBlock, new_arena: &mut ExprArena) -> ExternBlock {
        ExternBlock {
            convention: block.convention,
            library: block.library,
            items: block
                .items
                .iter()
                .map(|item| self.copy_extern_item(item, new_arena))
                .collect(),
            visibility: block.visibility,
            span: self.adjust_span(block.span),
        }
    }

    /// Copy an extern item (function declaration), adjusting spans and types.
    fn copy_extern_item(&self, item: &ExternItem, new_arena: &mut ExprArena) -> ExternItem {
        ExternItem {
            name: item.name,
            params: item
                .params
                .iter()
                .map(|p| self.copy_extern_param(p, new_arena))
                .collect(),
            return_ty: self.copy_parsed_type(&item.return_ty, new_arena),
            alias: item.alias,
            is_c_variadic: item.is_c_variadic,
            span: self.adjust_span(item.span),
        }
    }

    /// Copy an extern parameter, adjusting span and deep-copying its type.
    fn copy_extern_param(&self, param: &ExternParam, new_arena: &mut ExprArena) -> ExternParam {
        ExternParam {
            name: param.name,
            ty: self.copy_parsed_type(&param.ty, new_arena),
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a use definition (import).
    pub fn copy_use(&self, use_def: &UseDef) -> UseDef {
        UseDef {
            path: use_def.path.clone(),
            items: use_def.items.clone(),
            module_alias: use_def.module_alias,
            visibility: use_def.visibility,
            span: self.adjust_span(use_def.span),
            target_attr: use_def.target_attr.clone(),
            cfg_attr: use_def.cfg_attr.clone(),
        }
    }

    /// Copy a generic parameter.
    pub(super) fn copy_generic_param(
        &self,
        param: &GenericParam,
        new_arena: &mut ExprArena,
    ) -> GenericParam {
        GenericParam {
            name: param.name,
            bounds: param
                .bounds
                .iter()
                .map(|b| self.copy_trait_bound(b))
                .collect(),
            default_type: param
                .default_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            is_const: param.is_const,
            const_type: param
                .const_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            default_value: param.default_value.map(|e| self.copy_expr(e, new_arena)),
            span: self.adjust_span(param.span),
        }
    }

    /// Copy a where clause.
    pub(super) fn copy_where_clause(
        &self,
        clause: &WhereClause,
        new_arena: &mut ExprArena,
    ) -> WhereClause {
        match clause {
            WhereClause::TypeBound {
                param,
                projection,
                bounds,
                span,
            } => WhereClause::TypeBound {
                param: *param,
                projection: *projection,
                bounds: bounds.iter().map(|b| self.copy_trait_bound(b)).collect(),
                span: self.adjust_span(*span),
            },
            WhereClause::ConstBound { expr, span } => WhereClause::ConstBound {
                expr: self.copy_expr(*expr, new_arena),
                span: self.adjust_span(*span),
            },
        }
    }

    /// Copy a trait bound.
    pub(super) fn copy_trait_bound(&self, bound: &ori_ir::TraitBound) -> ori_ir::TraitBound {
        ori_ir::TraitBound {
            first: bound.first,
            rest: bound.rest.clone(),
            span: self.adjust_span(bound.span),
        }
    }
}
