//! Shared test utilities for `check::api::tests` and `check::bodies::tests`.
//!
//! The `parse_and_check` harness drives a full lex → parse → check cycle on
//! a source string. Both the API-level and body-pass regression test suites
//! need the same harness; this module is the single source of truth.
//!
//! This file is `#[cfg(test)]`-gated so it never contributes to production
//! builds.

use ori_ir::StringInterner;
use ori_lexer::lex;
use ori_parse::parse;

use crate::{check_module, TypeCheckResult};

/// Parse-and-check harness used by the body-pass and API regression tests.
///
/// Asserts that parsing produced no errors and returns the `TypeCheckResult`
/// plus the interner so callers can inspect the typed IR or look up names.
pub(crate) fn parse_and_check(source: &str) -> (TypeCheckResult, StringInterner) {
    let interner = StringInterner::new();
    let tokens = lex(source, &interner);
    let parsed = parse(&tokens, &interner);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let result = check_module(&parsed.module, &parsed.arena, &interner);
    (result, interner)
}
