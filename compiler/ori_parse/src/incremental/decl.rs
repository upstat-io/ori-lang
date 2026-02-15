//! Declaration collection for incremental parsing.

use ori_ir::{Module, Span};

/// Kind of top-level declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeclKind {
    Import,
    ExtensionImport,
    Const,
    Function,
    Test,
    Type,
    Trait,
    Impl,
    DefImpl,
    Extend,
    ExternBlock,
}

/// Reference to a declaration with its source span.
#[derive(Debug, Clone, Copy)]
pub struct DeclRef {
    pub kind: DeclKind,
    pub index: usize,
    pub span: Span,
}

/// Collect all top-level declarations from a module, sorted by start position.
pub fn collect_declarations(module: &Module) -> Vec<DeclRef> {
    let mut decls = Vec::new();

    for (i, import) in module.imports.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Import,
            index: i,
            span: import.span,
        });
    }

    for (i, ext_import) in module.extension_imports.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::ExtensionImport,
            index: i,
            span: ext_import.span,
        });
    }

    for (i, const_def) in module.consts.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Const,
            index: i,
            span: const_def.span,
        });
    }

    for (i, func) in module.functions.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Function,
            index: i,
            span: func.span,
        });
    }

    for (i, test) in module.tests.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Test,
            index: i,
            span: test.span,
        });
    }

    for (i, type_decl) in module.types.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Type,
            index: i,
            span: type_decl.span,
        });
    }

    for (i, trait_def) in module.traits.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Trait,
            index: i,
            span: trait_def.span,
        });
    }

    for (i, impl_def) in module.impls.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Impl,
            index: i,
            span: impl_def.span,
        });
    }

    for (i, def_impl) in module.def_impls.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::DefImpl,
            index: i,
            span: def_impl.span,
        });
    }

    for (i, extend) in module.extends.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::Extend,
            index: i,
            span: extend.span,
        });
    }

    for (i, extern_block) in module.extern_blocks.iter().enumerate() {
        decls.push(DeclRef {
            kind: DeclKind::ExternBlock,
            index: i,
            span: extern_block.span,
        });
    }

    // Sort by start position for binary search
    decls.sort_by_key(|d| d.span.start);
    decls
}
