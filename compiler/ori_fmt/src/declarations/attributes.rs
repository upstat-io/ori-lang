//! Attribute Emission
//!
//! File-level (`#!target`/`#!cfg`), item-level (`#target`/`#cfg`), and `#repr`
//! attribute syntax emission for declarations.

use crate::formatter::emit_escaped_str;
use ori_ir::ast::items::ReprAttrKind;
use ori_ir::{FileAttr, Name, StringLookup};

use super::ModuleFormatter;

impl<I: StringLookup> ModuleFormatter<'_, I> {
    /// Emit a file-level attribute (`#!target(...)` or `#!cfg(...)`).
    pub(super) fn format_file_attr(&mut self, attr: &FileAttr) {
        match attr {
            FileAttr::Target { attr: target, .. } => {
                self.ctx.emit("#!target(");
                self.emit_target_attr_args(target);
                self.ctx.emit(")");
            }
            FileAttr::Cfg { attr: cfg, .. } => {
                self.ctx.emit("#!cfg(");
                self.emit_cfg_attr_args(cfg);
                self.ctx.emit(")");
            }
        }
        self.ctx.emit_newline();
    }

    /// Emit an item-level `#target(...)` attribute if present.
    ///
    /// Spec §25.4: Conditional compilation on functions, types, trait impls, constants.
    pub(super) fn emit_item_target_attr(&mut self, target: &ori_ir::TargetAttr) {
        self.ctx.emit("#target(");
        self.emit_target_attr_args(target);
        self.ctx.emit(")");
        self.ctx.emit_newline_indent();
    }

    /// Emit an item-level `#cfg(...)` attribute if present.
    ///
    /// Spec §25.4: Conditional compilation on functions, types, trait impls, constants.
    pub(super) fn emit_item_cfg_attr(&mut self, cfg: &ori_ir::CfgAttr) {
        self.ctx.emit("#cfg(");
        self.emit_cfg_attr_args(cfg);
        self.ctx.emit(")");
        self.ctx.emit_newline_indent();
    }

    /// Emit the comma-separated `target` attribute arguments (no surrounding `(...)`).
    ///
    /// Shared by the item-level `#target(...)` and file-level `#!target(...)` callers,
    /// which own the prefix, closing paren, and newline emission.
    fn emit_target_attr_args(&mut self, target: &ori_ir::TargetAttr) {
        let mut first = true;
        for (key, val) in [
            ("os", target.os),
            ("arch", target.arch),
            ("family", target.family),
            ("not_os", target.not_os),
            ("not_arch", target.not_arch),
            ("not_family", target.not_family),
        ] {
            self.emit_attr_string_param(key, val, &mut first);
        }
        for (key, list) in [("any_os", &target.any_os), ("any_arch", &target.any_arch)] {
            self.emit_attr_string_list(key, list, &mut first);
        }
    }

    /// Emit the comma-separated `cfg` attribute arguments (no surrounding `(...)`).
    ///
    /// Shared by the item-level `#cfg(...)` and file-level `#!cfg(...)` callers,
    /// which own the prefix, closing paren, and newline emission.
    fn emit_cfg_attr_args(&mut self, cfg: &ori_ir::CfgAttr) {
        let mut first = true;
        for (flag, set) in [
            ("debug", &cfg.debug),
            ("release", &cfg.release),
            ("not_debug", &cfg.not_debug),
        ] {
            if *set {
                self.emit_join_sep(&mut first);
                self.ctx.emit(flag);
            }
        }
        for (key, val) in [("feature", cfg.feature), ("not_feature", cfg.not_feature)] {
            self.emit_attr_string_param(key, val, &mut first);
        }
        self.emit_attr_string_list("any_feature", &cfg.any_feature, &mut first);
    }

    /// Emit a `key: "value"` attribute parameter if the value is present.
    fn emit_attr_string_param(&mut self, key: &str, val: Option<Name>, first: &mut bool) {
        if let Some(name) = val {
            self.emit_join_sep(first);
            self.ctx.emit(key);
            self.ctx.emit(": ");
            let s = self.interner.lookup(name);
            emit_escaped_str(&mut self.ctx, s);
        }
    }

    /// Emit a `key: ["v1", "v2"]` attribute list parameter if non-empty.
    fn emit_attr_string_list(&mut self, key: &str, list: &[Name], first: &mut bool) {
        if !list.is_empty() {
            self.emit_join_sep(first);
            self.ctx.emit(key);
            self.ctx.emit(": [");
            let mut first_elem = true;
            for name in list {
                self.emit_join_sep(&mut first_elem);
                let s = self.interner.lookup(*name);
                emit_escaped_str(&mut self.ctx, s);
            }
            self.ctx.emit("]");
        }
    }

    /// Emit a `#repr(...)` attribute on its own line.
    ///
    /// Spec §26: `#repr("c")`, `#repr("packed")`, `#repr("transparent")`,
    /// `#repr("aligned", N)`. `CAligned(N)` is emitted as two stacked attrs
    /// since it's produced by type-checking merge of `#repr("c")` + `#repr("aligned", N)`.
    pub(super) fn emit_repr_attr(&mut self, repr: &ReprAttrKind) {
        match repr {
            ReprAttrKind::C => {
                self.ctx.emit("#repr(\"c\")");
                self.ctx.emit_newline_indent();
            }
            ReprAttrKind::Packed => {
                self.ctx.emit("#repr(\"packed\")");
                self.ctx.emit_newline_indent();
            }
            ReprAttrKind::Transparent => {
                self.ctx.emit("#repr(\"transparent\")");
                self.ctx.emit_newline_indent();
            }
            ReprAttrKind::Aligned(n) => {
                self.ctx.emit("#repr(\"aligned\", ");
                self.ctx.emit(n.to_string());
                self.ctx.emit(")");
                self.ctx.emit_newline_indent();
            }
            ReprAttrKind::CAligned(n) => {
                // CAligned is the merged form — emit as two stacked attrs
                self.ctx.emit("#repr(\"c\")");
                self.ctx.emit_newline_indent();
                self.ctx.emit("#repr(\"aligned\", ");
                self.ctx.emit(n.to_string());
                self.ctx.emit(")");
                self.ctx.emit_newline_indent();
            }
        }
    }
}
