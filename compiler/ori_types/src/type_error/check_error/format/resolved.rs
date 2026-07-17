//! Pool- and interner-backed rich formatting convenience API.

use super::super::TypeCheckError;

impl TypeCheckError {
    /// Convenience wrapper for `format_message_rich` using a `Pool` and `StringInterner`.
    ///
    /// This is the easiest way to get rich error messages when you have both
    /// a Pool (for type formatting) and a `StringInterner` (for name resolution).
    ///
    /// # Example
    ///
    /// ```rust
    /// use ori_ir::StringInterner;
    /// use ori_types::check_module_with_imports;
    ///
    /// let interner = StringInterner::new();
    /// let tokens = ori_lexer::lex("@main () -> int = \"oops\"", &interner);
    /// let parsed = ori_parse::parse(&tokens, &interner);
    /// let (result, pool) = check_module_with_imports(
    ///     &parsed.module,
    ///     &parsed.arena,
    ///     &interner,
    ///     |_| {},
    /// );
    /// let error = result.errors().first().expect("the return type should mismatch");
    /// let message = error.format_with(&pool, &interner);
    /// assert!(!message.is_empty());
    /// ```
    pub fn format_with(&self, pool: &crate::Pool, interner: &ori_ir::StringInterner) -> String {
        self.format_message_rich(&|idx| pool.format_type_resolved(idx, interner), &|name| {
            interner.lookup(name).to_string()
        })
    }
}
