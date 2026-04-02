//! Trait, impl, def impl, and extend declaration copying for the AST copier.

use super::AstCopier;
use ori_ir::{
    DefImplDef, ExprArena, ExtendDef, ImplAssocType, ImplDef, ImplMethod, TraitAssocType, TraitDef,
    TraitDefaultMethod, TraitItem, TraitMethodSig,
};

impl AstCopier<'_> {
    /// Copy a trait definition.
    pub fn copy_trait(&self, trait_def: &TraitDef, new_arena: &mut ExprArena) -> TraitDef {
        let old_generics = self.old_arena.get_generic_params(trait_def.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_items: Vec<_> = trait_def
            .items
            .iter()
            .map(|item| self.copy_trait_item(item, new_arena))
            .collect();

        TraitDef {
            name: trait_def.name,
            generics: new_arena.alloc_generic_params(new_generics),
            super_traits: trait_def
                .super_traits
                .iter()
                .map(|t| self.copy_trait_bound(t))
                .collect(),
            items: new_items,
            span: self.adjust_span(trait_def.span),
            visibility: trait_def.visibility,
        }
    }

    /// Copy a trait item.
    fn copy_trait_item(&self, item: &TraitItem, new_arena: &mut ExprArena) -> TraitItem {
        match item {
            TraitItem::MethodSig(sig) => {
                TraitItem::MethodSig(self.copy_trait_method_sig(sig, new_arena))
            }
            TraitItem::DefaultMethod(method) => {
                TraitItem::DefaultMethod(self.copy_trait_default_method(method, new_arena))
            }
            TraitItem::AssocType(assoc) => {
                TraitItem::AssocType(self.copy_trait_assoc_type(assoc, new_arena))
            }
        }
    }

    /// Copy a trait method signature.
    fn copy_trait_method_sig(
        &self,
        sig: &TraitMethodSig,
        new_arena: &mut ExprArena,
    ) -> TraitMethodSig {
        let old_params = self.old_arena.get_params(sig.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TraitMethodSig {
            name: sig.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&sig.return_ty, new_arena),
            span: self.adjust_span(sig.span),
        }
    }

    /// Copy a trait default method.
    fn copy_trait_default_method(
        &self,
        method: &TraitDefaultMethod,
        new_arena: &mut ExprArena,
    ) -> TraitDefaultMethod {
        let old_params = self.old_arena.get_params(method.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        TraitDefaultMethod {
            name: method.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&method.return_ty, new_arena),
            body: self.copy_expr(method.body, new_arena),
            span: self.adjust_span(method.span),
        }
    }

    /// Copy a trait associated type.
    fn copy_trait_assoc_type(
        &self,
        assoc: &TraitAssocType,
        new_arena: &mut ExprArena,
    ) -> TraitAssocType {
        TraitAssocType {
            name: assoc.name,
            default_type: assoc
                .default_type
                .as_ref()
                .map(|t| self.copy_parsed_type(t, new_arena)),
            span: self.adjust_span(assoc.span),
        }
    }

    /// Copy an impl definition.
    pub fn copy_impl(&self, impl_def: &ImplDef, new_arena: &mut ExprArena) -> ImplDef {
        let old_generics = self.old_arena.get_generic_params(impl_def.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let old_trait_type_args = self
            .old_arena
            .get_parsed_type_list(impl_def.trait_type_args);
        let new_trait_type_args: Vec<_> = old_trait_type_args
            .iter()
            .map(|id| self.copy_parsed_type_id(*id, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = impl_def
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        let new_methods: Vec<_> = impl_def
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        let new_assoc_types: Vec<_> = impl_def
            .assoc_types
            .iter()
            .map(|a| self.copy_impl_assoc_type(a, new_arena))
            .collect();

        ImplDef {
            generics: new_arena.alloc_generic_params(new_generics),
            trait_path: impl_def.trait_path.clone(),
            trait_type_args: new_arena.alloc_parsed_type_list(new_trait_type_args),
            self_path: impl_def.self_path.clone(),
            self_ty: self.copy_parsed_type(&impl_def.self_ty, new_arena),
            where_clauses: new_where_clauses,
            methods: new_methods,
            assoc_types: new_assoc_types,
            span: self.adjust_span(impl_def.span),
            target_attr: impl_def.target_attr.clone(),
            cfg_attr: impl_def.cfg_attr.clone(),
        }
    }

    /// Copy an impl method.
    pub(super) fn copy_impl_method(
        &self,
        method: &ImplMethod,
        new_arena: &mut ExprArena,
    ) -> ImplMethod {
        let old_params = self.old_arena.get_params(method.params);
        let new_params: Vec<_> = old_params
            .iter()
            .map(|p| self.copy_param(p, new_arena))
            .collect();

        ImplMethod {
            name: method.name,
            params: new_arena.alloc_params(new_params),
            return_ty: self.copy_parsed_type(&method.return_ty, new_arena),
            body: self.copy_expr(method.body, new_arena),
            span: self.adjust_span(method.span),
        }
    }

    /// Copy an impl associated type.
    fn copy_impl_assoc_type(
        &self,
        assoc: &ImplAssocType,
        new_arena: &mut ExprArena,
    ) -> ImplAssocType {
        ImplAssocType {
            name: assoc.name,
            ty: self.copy_parsed_type(&assoc.ty, new_arena),
            span: self.adjust_span(assoc.span),
        }
    }

    /// Copy a def impl definition.
    pub fn copy_def_impl(&self, def_impl: &DefImplDef, new_arena: &mut ExprArena) -> DefImplDef {
        let new_methods: Vec<_> = def_impl
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        DefImplDef {
            trait_name: def_impl.trait_name,
            methods: new_methods,
            span: self.adjust_span(def_impl.span),
            visibility: def_impl.visibility,
        }
    }

    /// Copy an extend definition.
    pub fn copy_extend(&self, extend: &ExtendDef, new_arena: &mut ExprArena) -> ExtendDef {
        let old_generics = self.old_arena.get_generic_params(extend.generics);
        let new_generics: Vec<_> = old_generics
            .iter()
            .map(|g| self.copy_generic_param(g, new_arena))
            .collect();

        let new_where_clauses: Vec<_> = extend
            .where_clauses
            .iter()
            .map(|w| self.copy_where_clause(w, new_arena))
            .collect();

        let new_methods: Vec<_> = extend
            .methods
            .iter()
            .map(|m| self.copy_impl_method(m, new_arena))
            .collect();

        ExtendDef {
            generics: new_arena.alloc_generic_params(new_generics),
            target_ty: self.copy_parsed_type(&extend.target_ty, new_arena),
            target_type_name: extend.target_type_name,
            where_clauses: new_where_clauses,
            methods: new_methods,
            span: self.adjust_span(extend.span),
        }
    }
}
