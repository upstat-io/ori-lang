//! Code generation context and state.
//!
//! The `CodegenContext` holds all state needed during code generation,
//! including type information, ownership analysis results, and output buffers.

use rustc_hash::FxHashSet;
use ori_ir::{ExprId, Name, StringInterner, TypeId};
use ori_types::TypeInterner;

use crate::analysis::OwnershipInfo;

/// Code generation context.
///
/// Holds all state needed to generate C code from a typed Ori module.
pub struct CodegenContext<'a> {
    /// String interner for resolving names.
    pub interner: &'a StringInterner,
    /// Type interner for resolving types.
    pub type_interner: &'a TypeInterner,
    /// Types for each expression (indexed by ExprId).
    pub expr_types: &'a [TypeId],
    /// Ownership analysis results for ARC elision.
    pub ownership: OwnershipInfo,
    /// Current indentation level.
    indent: usize,
    /// Generated code output.
    output: String,
    /// Set of generated helper functions to avoid duplicates.
    generated_helpers: FxHashSet<String>,
    /// Counter for generating unique temporary names.
    temp_counter: u32,
}

impl<'a> CodegenContext<'a> {
    /// Create a new codegen context.
    pub fn new(
        interner: &'a StringInterner,
        type_interner: &'a TypeInterner,
        expr_types: &'a [TypeId],
    ) -> Self {
        Self {
            interner,
            type_interner,
            expr_types,
            ownership: OwnershipInfo::default(),
            indent: 0,
            output: String::with_capacity(4096),
            generated_helpers: FxHashSet::default(),
            temp_counter: 0,
        }
    }

    /// Set ownership analysis results.
    pub fn with_ownership(mut self, ownership: OwnershipInfo) -> Self {
        self.ownership = ownership;
        self
    }

    /// Get the type of an expression.
    #[inline]
    pub fn expr_type(&self, id: ExprId) -> TypeId {
        self.expr_types[id.index()]
    }

    /// Check if ARC operations can be elided for an expression.
    #[inline]
    pub fn can_elide_arc(&self, id: ExprId) -> bool {
        self.ownership.elide_arc.contains(&id)
    }

    /// Check if a binding needs release on scope exit.
    #[inline]
    pub fn needs_release(&self, name: Name) -> bool {
        self.ownership.needs_release.contains(&name)
    }

    /// Resolve a name to its string representation.
    #[inline]
    pub fn resolve_name(&self, name: Name) -> &str {
        self.interner.lookup(name)
    }

    /// Mangle a Ori name for C compatibility.
    ///
    /// C identifiers can only contain alphanumeric characters and underscores,
    /// and cannot start with a digit.
    pub fn mangle(&self, name: Name) -> String {
        let s = self.interner.lookup(name);
        let mut result = String::with_capacity(s.len() + 6);
        result.push_str("ori_");
        for c in s.chars() {
            if c.is_alphanumeric() {
                result.push(c);
            } else {
                result.push('_');
            }
        }
        result
    }

    /// Generate a unique temporary variable name.
    pub fn fresh_temp(&mut self) -> String {
        let n = self.temp_counter;
        self.temp_counter += 1;
        format!("_tmp{n}")
    }

    /// Increase indentation level.
    pub fn indent(&mut self) {
        self.indent += 1;
    }

    /// Decrease indentation level.
    pub fn dedent(&mut self) {
        debug_assert!(self.indent > 0, "dedent called with zero indent");
        self.indent = self.indent.saturating_sub(1);
    }

    /// Write indentation to output.
    pub fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    /// Write a string to output.
    pub fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    /// Write a line to output (with indentation and newline).
    pub fn writeln(&mut self, s: &str) {
        self.write_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    /// Write a newline.
    pub fn newline(&mut self) {
        self.output.push('\n');
    }

    /// Take the generated output.
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output)
    }

    /// Check if a helper function has been generated.
    pub fn has_helper(&self, name: &str) -> bool {
        self.generated_helpers.contains(name)
    }

    /// Mark a helper function as generated.
    pub fn mark_helper(&mut self, name: impl Into<String>) {
        self.generated_helpers.insert(name.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mangle_simple() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let name = interner.intern("foo");
        let ctx = CodegenContext::new(&interner, &type_interner, &[]);
        assert_eq!(ctx.mangle(name), "ori_foo");
    }

    #[test]
    fn test_mangle_special_chars() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let name = interner.intern("my-func");
        let ctx = CodegenContext::new(&interner, &type_interner, &[]);
        assert_eq!(ctx.mangle(name), "ori_my_func");
    }

    #[test]
    fn test_fresh_temp() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let mut ctx = CodegenContext::new(&interner, &type_interner, &[]);
        assert_eq!(ctx.fresh_temp(), "_tmp0");
        assert_eq!(ctx.fresh_temp(), "_tmp1");
        assert_eq!(ctx.fresh_temp(), "_tmp2");
    }

    #[test]
    fn test_indent_dedent() {
        let interner = StringInterner::new();
        let type_interner = TypeInterner::new();
        let mut ctx = CodegenContext::new(&interner, &type_interner, &[]);

        ctx.writeln("line1");
        ctx.indent();
        ctx.writeln("line2");
        ctx.indent();
        ctx.writeln("line3");
        ctx.dedent();
        ctx.writeln("line4");
        ctx.dedent();
        ctx.writeln("line5");

        let output = ctx.take_output();
        assert_eq!(
            output,
            "line1\n    line2\n        line3\n    line4\nline5\n"
        );
    }
}
