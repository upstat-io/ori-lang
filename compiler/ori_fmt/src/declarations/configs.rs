//! Config/Constant Formatting
//!
//! Formatting for module-level constant definitions.

use crate::comments::{format_comment, CommentIndex};
use crate::formatter::Formatter;
use ori_ir::ast::items::ConfigDef;
use ori_ir::{CommentList, StringLookup, Visibility};

use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format constant/config definitions.
    pub(super) fn format_configs(&mut self, configs: &[ConfigDef]) {
        for config in configs {
            self.format_config(config);
            self.ctx.emit_newline();
        }
    }

    /// Format constant definitions with comments.
    pub(super) fn format_configs_with_comments(
        &mut self,
        configs: &[ConfigDef],
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        for config in configs {
            self.emit_comments_before_config(config.span.start, comments, comment_index);
            self.format_config(config);
            self.ctx.emit_newline();
        }
    }

    fn emit_comments_before_config(
        &mut self,
        pos: u32,
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        let indices = comment_index.take_comments_before(pos);
        for idx in indices {
            let comment = &comments[idx];
            self.ctx.emit(&format_comment(comment, self.interner));
            self.ctx.emit_newline();
        }
    }

    fn format_config(&mut self, config: &ConfigDef) {
        if config.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }
        self.ctx.emit("$");
        self.ctx.emit(self.interner.lookup(config.name));
        self.ctx.emit(" = ");

        // Format the value expression
        // Pass current column so width decisions account for full line context
        let current_column = self.ctx.column();
        let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
            .with_starting_column(current_column);
        expr_formatter.format(config.value);
        // Get the output without trailing newline
        let expr_output = expr_formatter.ctx.as_str().trim_end();
        self.ctx.emit(expr_output);
    }
}
