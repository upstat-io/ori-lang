//! Declaration Formatting
//!
//! Formatting for top-level declarations: functions, types, traits, impls, imports, and constants.
//!
//! # Design
//!
//! Declaration formatting builds on the expression formatter by adding:
//! - Function signature formatting (params, generics, return type, capabilities)
//! - Type definition formatting (structs, sum types, newtypes)
//! - Module-level structure (imports, constants, functions, tests)
//! - Blank line handling between items
//! - Comment preservation and doc comment reordering
//!
//! # Modules
//!
//! - `parsed_types`: Type expression formatting and width calculation
//! - `functions`: Function declaration formatting
//! - `types`: Type declaration formatting (struct, sum, newtype)
//! - `traits`: Trait definition formatting
//! - `impls`: Impl block formatting
//! - `def_impls`: Default implementation block formatting
//! - `extends`: Extension block formatting
//! - `imports`: Import statement formatting
//! - `configs`: Constant definition formatting
//! - `tests_fmt`: Test definition formatting
//! - `comments`: Comment handling and emission
//! - `attributes`: File / item / repr attribute emission

mod attributes;
mod comments;
mod configs;
mod def_impls;
mod extends;
mod extern_def;
pub(crate) mod function_body;
mod functions;
mod impls;
mod imports;
pub(crate) mod parsed_types;
mod tests_fmt;
mod traits;
mod types;

pub(crate) use function_body::BodyBreakPolicy;
pub(crate) use parsed_types::format_parsed_type;

use crate::comments::CommentIndex;
use crate::context::{FormatConfig, FormatContext};
use crate::emitter::StringEmitter;
use crate::width::WidthCalculator;
use ori_ir::ast::items::Module;
use ori_ir::{CommentList, ExprArena, Spanned, StringLookup};

/// Format a complete module to a string with default config.
pub fn format_module<I: StringLookup>(module: &Module, arena: &ExprArena, interner: &I) -> String {
    format_module_with_config(module, arena, interner, FormatConfig::default())
}

/// Format a complete module to a string with custom config.
pub fn format_module_with_config<I: StringLookup>(
    module: &Module,
    arena: &ExprArena,
    interner: &I,
    config: FormatConfig,
) -> String {
    let mut formatter = ModuleFormatter::with_config(arena, interner, config);
    formatter.format_module(module);
    formatter.ctx.finalize()
}

/// Format a complete module with comment preservation and default config.
///
/// This function preserves comments from the source, associating them with
/// the declarations they precede. Doc comments are reordered to canonical order.
pub fn format_module_with_comments<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
) -> String {
    format_module_with_comments_and_config(
        module,
        comments,
        arena,
        interner,
        FormatConfig::default(),
    )
}

/// Format a complete module with comment preservation and custom config.
///
/// This function preserves comments from the source, associating them with
/// the declarations they precede. Doc comments are reordered to canonical order.
pub fn format_module_with_comments_and_config<I: StringLookup>(
    module: &Module,
    comments: &CommentList,
    arena: &ExprArena,
    interner: &I,
    config: FormatConfig,
) -> String {
    let mut formatter = ModuleFormatter::with_config(arena, interner, config);

    // Collect all item positions for comment association
    let positions = collect_module_positions(module);
    let mut comment_index = CommentIndex::new(comments, &positions);

    formatter.format_module_with_comments(module, comments, &mut comment_index);
    formatter.ctx.finalize()
}

/// Collect all start positions of items in a module.
fn collect_module_positions(module: &Module) -> Vec<u32> {
    let mut positions = Vec::new();

    if let Some(attr) = &module.file_attr {
        positions.push(attr.span().start);
    }
    for import in &module.imports {
        positions.push(import.span.start);
    }
    for ext_import in &module.extension_imports {
        positions.push(ext_import.span.start);
    }
    for const_def in &module.consts {
        positions.push(const_def.span.start);
    }
    for type_decl in &module.types {
        positions.push(type_decl.span.start);
    }
    for trait_def in &module.traits {
        positions.push(trait_def.span.start);
        // Also collect positions for items inside traits
        for item in &trait_def.items {
            positions.push(item.span().start);
        }
    }
    for impl_def in &module.impls {
        positions.push(impl_def.span.start);
        // Also collect positions for items inside impl blocks
        for assoc in &impl_def.assoc_types {
            positions.push(assoc.span.start);
        }
        for method in &impl_def.methods {
            positions.push(method.span.start);
        }
    }
    for def_impl in &module.def_impls {
        positions.push(def_impl.span.start);
        for method in &def_impl.methods {
            positions.push(method.span.start);
        }
    }
    for extend in &module.extends {
        positions.push(extend.span.start);
        for method in &extend.methods {
            positions.push(method.span.start);
        }
    }
    for func in &module.functions {
        positions.push(func.span.start);
    }
    for test in &module.tests {
        positions.push(test.span.start);
    }
    for extern_block in &module.extern_blocks {
        positions.push(extern_block.span.start);
    }

    positions.sort_unstable();
    positions
}

/// Formatter for module-level declarations.
pub struct ModuleFormatter<'a, I: StringLookup> {
    pub(super) arena: &'a ExprArena,
    pub(super) interner: &'a I,
    pub(super) ctx: FormatContext<StringEmitter>,
    pub(super) width_calc: WidthCalculator<'a, I>,
}

impl<'a, I: StringLookup> ModuleFormatter<'a, I> {
    /// Create a new module formatter with default config.
    pub fn new(arena: &'a ExprArena, interner: &'a I) -> Self {
        Self::with_config(arena, interner, FormatConfig::default())
    }

    /// Create a new module formatter with custom config.
    pub fn with_config(arena: &'a ExprArena, interner: &'a I, config: FormatConfig) -> Self {
        Self {
            arena,
            interner,
            ctx: FormatContext::with_config(config),
            width_calc: WidthCalculator::new(arena, interner),
        }
    }

    /// Finish formatting and return the result string.
    pub fn finish(self) -> String {
        self.ctx.finalize()
    }

    /// Format a complete module.
    pub fn format_module(&mut self, module: &Module) {
        let mut first_item = true;

        // File-level attribute
        if let Some(attr) = &module.file_attr {
            self.format_file_attr(attr);
            first_item = false;
        }

        // Imports first
        if !module.imports.is_empty() {
            self.format_imports(&module.imports);
            first_item = false;
        }

        // Extension imports (after regular imports)
        if !module.extension_imports.is_empty() {
            self.format_extension_imports(&module.extension_imports);
            first_item = false;
        }

        // Constants
        if !module.consts.is_empty() {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_consts(&module.consts);
            first_item = false;
        }

        // Type decls, traits, impls, def-impls, extensions, extern blocks,
        // functions, tests: each item gets a blank line before (except the
        // module's first) and after — shared skeleton in `format_items`.
        self.format_items(&module.types, &mut first_item, Self::format_type_decl);
        self.format_items(&module.traits, &mut first_item, Self::format_trait);
        self.format_items(&module.impls, &mut first_item, Self::format_impl);
        self.format_items(&module.def_impls, &mut first_item, Self::format_def_impl);
        self.format_items(&module.extends, &mut first_item, Self::format_extend);
        self.format_items(
            &module.extern_blocks,
            &mut first_item,
            Self::format_extern_block,
        );
        self.format_items(&module.functions, &mut first_item, Self::format_function);
        self.format_items(&module.tests, &mut first_item, Self::format_test);
    }

    /// Emit each `item` via `format_one`, preceded by a blank line unless it's
    /// the module's first item and followed by one — the skeleton shared by
    /// every top-level declaration category in [`Self::format_module`].
    /// Delegates to [`Self::format_items_with_comments`] (a bare `fn` pointer
    /// coerces to `impl FnMut`).
    fn format_items<T>(
        &mut self,
        items: &[T],
        first_item: &mut bool,
        format_one: fn(&mut Self, &T),
    ) {
        self.format_items_with_comments(items, first_item, format_one);
    }

    /// Format a complete module with comment preservation.
    pub fn format_module_with_comments(
        &mut self,
        module: &Module,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let mut first_item = true;

        // File-level attribute
        if let Some(attr) = &module.file_attr {
            self.format_file_attr(attr);
            first_item = false;
        }

        // Imports first
        if !module.imports.is_empty() {
            self.format_imports_with_comments(&module.imports, comments, comment_index);
            first_item = false;
        }

        // Extension imports (after regular imports)
        if !module.extension_imports.is_empty() {
            self.format_extension_imports_with_comments(
                &module.extension_imports,
                comments,
                comment_index,
            );
            first_item = false;
        }

        // Constants
        if !module.consts.is_empty() {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_consts_with_comments(&module.consts, comments, comment_index);
            first_item = false;
        }

        // Type decls, traits, impls, def-impls, extensions, extern blocks,
        // functions, tests: blank line before (except module's first) +
        // after; comments emitted first — skeleton in `format_items_with_comments`.
        self.format_items_with_comments(&module.types, &mut first_item, |s, type_decl| {
            s.emit_comments_before_type(type_decl, comments, comment_index);
            s.format_type_decl(type_decl);
        });
        self.format_items_with_comments(&module.traits, &mut first_item, |s, trait_def| {
            s.emit_comments_before(trait_def.span.start, comments, comment_index);
            s.format_trait_with_comments(trait_def, comments, comment_index);
        });
        self.format_items_with_comments(&module.impls, &mut first_item, |s, impl_def| {
            s.emit_comments_before(impl_def.span.start, comments, comment_index);
            s.format_impl_with_comments(impl_def, comments, comment_index);
        });
        self.format_items_with_comments(&module.def_impls, &mut first_item, |s, def_impl| {
            s.emit_comments_before(def_impl.span.start, comments, comment_index);
            s.format_def_impl_with_comments(def_impl, comments, comment_index);
        });
        self.format_items_with_comments(&module.extends, &mut first_item, |s, extend| {
            s.emit_comments_before(extend.span.start, comments, comment_index);
            s.format_extend_with_comments(extend, comments, comment_index);
        });
        self.format_items_with_comments(
            &module.extern_blocks,
            &mut first_item,
            |s, extern_block| {
                s.emit_comments_before(extern_block.span.start, comments, comment_index);
                s.format_extern_block(extern_block);
            },
        );
        self.format_items_with_comments(&module.functions, &mut first_item, |s, func| {
            s.emit_comments_before_function(func, comments, comment_index);
            s.format_function(func);
        });
        self.format_items_with_comments(&module.tests, &mut first_item, |s, test| {
            s.emit_comments_before(test.span.start, comments, comment_index);
            s.format_test(test);
        });

        // Emit any trailing comments
        self.emit_trailing_comments(comments, comment_index);
    }

    /// Emit each `item` via `format_one` (which itself threads comment
    /// emission before the item), preceded by a blank line unless it's the
    /// module's first item and followed by one — the SSOT loop skeleton
    /// shared by every top-level declaration category in
    /// [`Self::format_module_with_comments`] AND, via [`Self::format_items`],
    /// [`Self::format_module`]. Takes a closure rather than a bare fn pointer
    /// because comment-aware callers capture `comments` + `comment_index`
    /// from the enclosing scope.
    fn format_items_with_comments<T>(
        &mut self,
        items: &[T],
        first_item: &mut bool,
        mut format_one: impl FnMut(&mut Self, &T),
    ) {
        for item in items {
            if !*first_item {
                self.ctx.emit_newline();
            }
            format_one(self, item);
            self.ctx.emit_newline();
            *first_item = false;
        }
    }

    /// Emit the `, ` separator before a subsequent item in a comma-joined
    /// list, then mark `first` false. Shared control-flow skeleton for every
    /// keyed-attribute / optional-field emitter that joins present fields
    /// with `, `.
    fn emit_join_sep(&mut self, first: &mut bool) {
        if !*first {
            self.ctx.emit(", ");
        }
        *first = false;
    }
}
