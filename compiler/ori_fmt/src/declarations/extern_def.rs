//! Extern block formatting.
//!
//! Formats `extern "c" from "lib" { ... }` blocks.

use ori_ir::{ExternBlock, StringLookup, Visibility};

use super::parsed_types::format_parsed_type;
use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Format an extern block declaration.
    pub(crate) fn format_extern_block(&mut self, block: &ExternBlock) {
        if block.visibility == Visibility::Public {
            self.ctx.emit("pub ");
        }

        let convention = self.interner.lookup(block.convention);
        self.ctx.emit("extern \"");
        self.ctx.emit(convention);
        self.ctx.emit("\"");

        if let Some(library) = block.library {
            let lib = self.interner.lookup(library);
            self.ctx.emit(" from \"");
            self.ctx.emit(lib);
            self.ctx.emit("\"");
        }

        self.ctx.emit(" {");

        if block.items.is_empty() {
            self.ctx.emit_newline();
            self.ctx.emit("}");
            return;
        }

        self.ctx.emit_newline();

        for item in &block.items {
            self.ctx.emit("    @");
            let name = self.interner.lookup(item.name);
            self.ctx.emit(name);
            self.ctx.emit(" (");

            for (i, param) in item.params.iter().enumerate() {
                if i > 0 {
                    self.ctx.emit(", ");
                }
                let pname = self.interner.lookup(param.name);
                self.ctx.emit(pname);
                self.ctx.emit(": ");
                format_parsed_type(&param.ty, self.arena, self.interner, &mut self.ctx);
            }

            if item.is_c_variadic {
                if !item.params.is_empty() {
                    self.ctx.emit(", ");
                }
                self.ctx.emit("...");
            }

            self.ctx.emit(") -> ");
            format_parsed_type(&item.return_ty, self.arena, self.interner, &mut self.ctx);

            if let Some(alias) = item.alias {
                let alias_str = self.interner.lookup(alias);
                self.ctx.emit(" as \"");
                self.ctx.emit(alias_str);
                self.ctx.emit("\"");
            }

            self.ctx.emit_newline();
        }

        self.ctx.emit("}");
    }
}
