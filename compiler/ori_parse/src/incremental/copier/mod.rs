//! Deep copier for AST nodes with span adjustment.

mod decl;
mod expr;
mod helpers;
mod trait_impl;
mod types;

use super::decl::{DeclKind, DeclRef};
use ori_ir::incremental::ChangeMarker;
use ori_ir::{ExprArena, Module, Span};

/// Deep copier for AST nodes with span adjustment.
///
/// This struct handles copying expressions and declarations from an old arena
/// to a new arena while adjusting spans according to a change marker.
pub struct AstCopier<'old> {
    old_arena: &'old ExprArena,
    marker: ChangeMarker,
}

impl<'old> AstCopier<'old> {
    /// Create a new AST copier.
    pub fn new(old_arena: &'old ExprArena, marker: ChangeMarker) -> Self {
        AstCopier { old_arena, marker }
    }

    /// Adjust a span from old positions to new positions.
    fn adjust_span(&self, span: Span) -> Span {
        self.marker.adjust_span(span).unwrap_or(span)
    }

    /// Copy a reusable declaration from the old module to the new module.
    ///
    /// Dispatches by `DeclKind`, copying the declaration and adjusting spans
    /// via the change marker. Used by the incremental parser's reuse path.
    pub fn copy_declaration_to_module(
        &self,
        decl_ref: DeclRef,
        old_module: &Module,
        new_module: &mut Module,
        new_arena: &mut ExprArena,
    ) {
        match decl_ref.kind {
            DeclKind::Function => {
                let old = &old_module.functions[decl_ref.index];
                new_module
                    .functions
                    .push(self.copy_function(old, new_arena));
            }
            DeclKind::Test => {
                let old = &old_module.tests[decl_ref.index];
                new_module.tests.push(self.copy_test(old, new_arena));
            }
            DeclKind::Type => {
                let old = &old_module.types[decl_ref.index];
                new_module.types.push(self.copy_type_decl(old, new_arena));
            }
            DeclKind::Trait => {
                let old = &old_module.traits[decl_ref.index];
                new_module.traits.push(self.copy_trait(old, new_arena));
            }
            DeclKind::Impl => {
                let old = &old_module.impls[decl_ref.index];
                new_module.impls.push(self.copy_impl(old, new_arena));
            }
            DeclKind::DefImpl => {
                let old = &old_module.def_impls[decl_ref.index];
                new_module
                    .def_impls
                    .push(self.copy_def_impl(old, new_arena));
            }
            DeclKind::Extend => {
                let old = &old_module.extends[decl_ref.index];
                new_module.extends.push(self.copy_extend(old, new_arena));
            }
            DeclKind::Const => {
                let old = &old_module.consts[decl_ref.index];
                new_module.consts.push(self.copy_const(old, new_arena));
            }
            DeclKind::ExtensionImport => {
                let old = &old_module.extension_imports[decl_ref.index];
                new_module
                    .extension_imports
                    .push(self.copy_extension_import(old));
            }
            DeclKind::ExternBlock => {
                let old = &old_module.extern_blocks[decl_ref.index];
                new_module
                    .extern_blocks
                    .push(self.copy_extern_block(old, new_arena));
            }
            DeclKind::Import => {
                unreachable!("imports should not appear in declaration list");
            }
        }
    }
}
