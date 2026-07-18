//! Shared full-pipeline test harness.

pub(super) use ori_ir::StringInterner;

use crate::check::check_module_with_pool;
pub(super) use crate::{Idx, Pool, Tag, TypeCheckResult, TypeErrorKind};

pub(super) fn fixture_without_trailing_newline(source: &'static str) -> &'static str {
    source
        .strip_suffix('\n')
        .expect("committed Ori fixtures end with a newline")
}

// Test Infrastructure

/// Result of checking a source string through the full pipeline.
pub(super) struct CheckResult {
    pub(super) result: TypeCheckResult,
    pub(super) pool: Pool,
    pub(super) interner: StringInterner,
    pub(super) parsed: ori_parse::ParseOutput,
}

impl CheckResult {
    /// Whether any type errors were reported.
    pub(super) fn has_errors(&self) -> bool {
        self.result.has_errors()
    }

    /// Number of type errors.
    pub(super) fn error_count(&self) -> usize {
        self.result.typed.errors.len()
    }

    /// Number of functions in the typed module.
    pub(super) fn function_count(&self) -> usize {
        self.result.typed.functions.len()
    }

    /// Get all error kinds for assertion.
    pub(super) fn error_kinds(&self) -> Vec<&TypeErrorKind> {
        self.result.typed.errors.iter().map(|e| &e.kind).collect()
    }

    /// Look up the body expression type of the first function.
    ///
    /// Returns the type of the function's body expression (its return value).
    pub(super) fn first_function_body_type(&self) -> Option<Idx> {
        let func = self.parsed.module.functions.first()?;
        let body_index = func.body.raw() as usize;
        self.result.typed.expr_type(body_index)
    }

    /// Look up the body expression type of a function by name.
    pub(super) fn function_body_type(&self, name: &str) -> Option<Idx> {
        let name_id = self.interner.intern(name);
        let func = self
            .parsed
            .module
            .functions
            .iter()
            .find(|f| f.name == name_id)?;
        let body_index = func.body.raw() as usize;
        self.result.typed.expr_type(body_index)
    }

    /// Get the tag (type kind) of a resolved type.
    pub(super) fn tag(&self, idx: Idx) -> Tag {
        self.pool.tag(idx)
    }

    /// Find mono instances for a given function name.
    pub(super) fn mono_instances_for(&self, name: &str) -> Vec<&crate::MonoInstance> {
        let name_id = self.interner.intern(name);
        self.result
            .typed
            .mono_instances
            .iter()
            .filter(|m| m.fn_name == name_id)
            .collect()
    }

    /// All mono instances recorded for the module (name-agnostic).
    ///
    /// Returns every recorded mono instance without filtering by function name.
    /// Builtin-only tests use the complete set when name resolution does not
    /// expose a stable function name.
    pub(super) fn mono_instances_all(&self) -> &[crate::MonoInstance] {
        &self.result.typed.mono_instances
    }

    /// Find the first `Tag::Applied` pool entry whose name and resolved args
    /// match `name` + `args`. Used by the generic-composite-monomorphization
    /// pins to locate the `Applied(Generic, [concrete])` handle and inspect its
    /// `Pool.resolutions` materialization.
    pub(super) fn find_applied(&self, name: &str, args: &[Idx]) -> Option<Idx> {
        let name_id = self.interner.intern(name);
        let len =
            u32::try_from(self.pool.len()).expect("type pool length must fit the Idx u32 domain");
        (Idx::FIRST_DYNAMIC..len).map(Idx::from_raw).find(|&idx| {
            self.pool.tag(idx) == Tag::Applied
                && self.pool.applied_name(idx) == name_id
                && self.pool.applied_args(idx).as_slice() == args
        })
    }
}

/// Parse and type-check an Ori source string.
pub(super) fn check_source(source: &str) -> CheckResult {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);

    // Ensure no parse errors before type checking
    assert!(
        parsed.errors.is_empty(),
        "Parse errors in test source: {:?}",
        parsed.errors
    );

    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);

    CheckResult {
        result,
        pool,
        interner,
        parsed,
    }
}

/// Parse and type-check, allowing parse errors (for testing that we handle them).
pub(super) fn check_source_allow_parse_errors(source: &str) -> CheckResult {
    let interner = StringInterner::new();
    let tokens = ori_lexer::lex(source, &interner);
    let parsed = ori_parse::parse(&tokens, &interner);
    let (result, pool) = check_module_with_pool(&parsed.module, &parsed.arena, &interner);

    CheckResult {
        result,
        pool,
        interner,
        parsed,
    }
}

/// Parse source into a `ParseOutput` using a shared interner.
///
/// Cross-module tests parse each module with its own arena while sharing the
/// interner so that `Name` handles remain consistent.
pub(super) fn parse_source(source: &str, interner: &StringInterner) -> ori_parse::ParseOutput {
    let tokens = ori_lexer::lex(source, interner);
    let parsed = ori_parse::parse(&tokens, interner);
    assert!(
        parsed.errors.is_empty(),
        "Parse errors in test source: {:?}",
        parsed.errors
    );
    parsed
}
