//! Constant Formatting
//!
//! Formatting for module-level constant definitions.

use crate::comments::{format_comment, CommentIndex};
use crate::formatter::Formatter;
use ori_ir::ast::items::ConstDef;
use ori_ir::{CommentList, StringLookup, Visibility};

use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format constant definitions.
    pub(super) fn format_consts(&mut self, consts: &[ConstDef]) {
        for const_def in consts {
            self.format_const(const_def);
            self.ctx.emit_newline();
        }
    }

    /// Format constant definitions with comments.
    pub(super) fn format_consts_with_comments(
        &mut self,
        consts: &[ConstDef],
        comments: &CommentList,
        comment_index: &mut CommentIndex,
    ) {
        for const_def in consts {
            self.emit_comments_before_const(const_def.span.start, comments, comment_index);
            self.format_const(const_def);
            self.ctx.emit_newline();
        }
    }

    fn emit_comments_before_const(
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

    fn format_const(&mut self, const_def: &ConstDef) {
        if const_def.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }
        self.ctx.emit("$");
        self.ctx.emit(self.interner.lookup(const_def.name));
        self.ctx.emit(" = ");

        // Format the value expression
        // Pass current column so width decisions account for full line context
        let current_column = self.ctx.column();
        let mut expr_formatter = Formatter::with_config(self.arena, self.interner, self.config)
            .with_starting_column(current_column);
        expr_formatter.format(const_def.value);
        // Get the output without trailing newline
        let expr_output = expr_formatter.ctx.as_str().trim_end();
        self.ctx.emit(expr_output);
    }
}
