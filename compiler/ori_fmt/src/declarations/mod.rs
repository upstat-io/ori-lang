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
//! - [`parsed_types`]: Type expression formatting and width calculation
//! - [`functions`]: Function declaration formatting
//! - [`types`]: Type declaration formatting (struct, sum, newtype)
//! - [`traits`]: Trait definition formatting
//! - [`impls`]: Impl block formatting
//! - [`imports`]: Import statement formatting
//! - [`configs`]: Constant definition formatting
//! - [`tests_fmt`]: Test definition formatting
//! - [`comments`]: Comment handling and emission

mod comments;
mod configs;
mod extern_def;
mod functions;
mod impls;
mod imports;
mod parsed_types;
mod tests_fmt;
mod traits;
mod types;

pub(crate) use parsed_types::format_parsed_type;

use crate::comments::CommentIndex;
use crate::context::{FormatConfig, FormatContext};
use crate::emitter::StringEmitter;
use crate::width::WidthCalculator;
use ori_ir::ast::items::Module;
use ori_ir::{CommentList, ExprArena, FileAttr, Spanned, StringLookup};

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

    /// Emit a file-level attribute (`#!target(...)` or `#!cfg(...)`).
    fn format_file_attr(&mut self, attr: &FileAttr) {
        match attr {
            FileAttr::Target { attr: target, .. } => {
                self.ctx.emit("#!target(");
                let mut first = true;
                for (key, val) in [
                    ("os", &target.os),
                    ("arch", &target.arch),
                    ("family", &target.family),
                    ("not_os", &target.not_os),
                ] {
                    if let Some(name) = val {
                        if !first {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit(key);
                        self.ctx.emit(": \"");
                        self.ctx.emit(self.interner.lookup(*name));
                        self.ctx.emit("\"");
                        first = false;
                    }
                }
                if !target.any_os.is_empty() {
                    if !first {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("any_os: [");
                    for (i, name) in target.any_os.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit("\"");
                        self.ctx.emit(self.interner.lookup(*name));
                        self.ctx.emit("\"");
                    }
                    self.ctx.emit("]");
                    first = false;
                }
                let _ = first;
                self.ctx.emit(")");
            }
            FileAttr::Cfg { attr: cfg, .. } => {
                self.ctx.emit("#!cfg(");
                let mut first = true;
                for (flag, set) in [
                    ("debug", &cfg.debug),
                    ("release", &cfg.release),
                    ("not_debug", &cfg.not_debug),
                ] {
                    if *set {
                        if !first {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit(flag);
                        first = false;
                    }
                }
                for (key, val) in [("feature", &cfg.feature), ("not_feature", &cfg.not_feature)] {
                    if let Some(name) = val {
                        if !first {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit(key);
                        self.ctx.emit(": \"");
                        self.ctx.emit(self.interner.lookup(*name));
                        self.ctx.emit("\"");
                        first = false;
                    }
                }
                if !cfg.any_feature.is_empty() {
                    if !first {
                        self.ctx.emit(", ");
                    }
                    self.ctx.emit("any_feature: [");
                    for (i, name) in cfg.any_feature.iter().enumerate() {
                        if i > 0 {
                            self.ctx.emit(", ");
                        }
                        self.ctx.emit("\"");
                        self.ctx.emit(self.interner.lookup(*name));
                        self.ctx.emit("\"");
                    }
                    self.ctx.emit("]");
                    first = false;
                }
                let _ = first;
                self.ctx.emit(")");
            }
        }
        self.ctx.emit_newline();
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

        // Type definitions
        for type_decl in &module.types {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_type_decl(type_decl);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Traits
        for trait_def in &module.traits {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_trait(trait_def);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Impls
        for impl_def in &module.impls {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_impl(impl_def);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Extern blocks
        for extern_block in &module.extern_blocks {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_extern_block(extern_block);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Functions
        for func in &module.functions {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_function(func);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Tests
        for test in &module.tests {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.format_test(test);
            self.ctx.emit_newline();
            first_item = false;
        }
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

        // Type definitions
        for type_decl in &module.types {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before_type(type_decl, comments, comment_index);
            self.format_type_decl(type_decl);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Traits
        for trait_def in &module.traits {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(trait_def.span.start, comments, comment_index);
            self.format_trait_with_comments(trait_def, comments, comment_index);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Impls
        for impl_def in &module.impls {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(impl_def.span.start, comments, comment_index);
            self.format_impl_with_comments(impl_def, comments, comment_index);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Extern blocks
        for extern_block in &module.extern_blocks {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(extern_block.span.start, comments, comment_index);
            self.format_extern_block(extern_block);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Functions
        for func in &module.functions {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before_function(func, comments, comment_index);
            self.format_function(func);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Tests
        for test in &module.tests {
            if !first_item {
                self.ctx.emit_newline();
            }
            self.emit_comments_before(test.span.start, comments, comment_index);
            self.format_test(test);
            self.ctx.emit_newline();
            first_item = false;
        }

        // Emit any trailing comments
        self.emit_trailing_comments(comments, comment_index);
    }
}
