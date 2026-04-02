//! Recursive descent parser for Ori.
//!
//! Produces flat AST using `ExprArena`.

mod context;
mod cursor;
mod dispatch;
mod error;
mod foreign_keywords;
mod grammar;
pub mod incremental;
mod module_parse;
mod outcome;
mod parser_capture;
mod parser_context;
mod recovery;
pub mod series;
mod snapshot;

#[cfg(test)]
mod tests;

pub use context::ParseContext;
pub(crate) use cursor::Cursor;
pub use error::{DetachmentReason, ErrorContext, ParseError, ParseWarning};
pub use outcome::ParseOutcome;
pub use recovery::{synchronize, TokenSet, FUNCTION_BOUNDARY, STMT_BOUNDARY};
pub use series::{SeriesConfig, TrailingSeparator};

// Re-export backtracking macros at crate root
// Note: These are defined in outcome.rs and use #[macro_export]
// They're automatically available at crate root via #[macro_export]

use ori_ir::{
    ExprArena, Function, Module, ModuleExtra, Name, SharedArena, StringInterner, TestDef, TokenList,
};

/// Result of parsing a definition starting with @.
/// Can be either a function or a test.
enum FunctionOrTest {
    Function(Function),
    Test(TestDef),
}

// Re-export ParsedAttrs from grammar module.
pub(crate) use grammar::ParsedAttrs;

/// Pre-interned `Name` values for contextual keywords used in identifier comparisons.
///
/// Avoids acquiring interner read-locks during parsing by comparing `Name` values
/// (u32 equality) instead of looking up strings via `interner().lookup()`.
pub(crate) struct KnownNames {
    // Channel constructors
    pub channel: Name,
    pub channel_in: Name,
    pub channel_out: Name,
    pub channel_all: Name,
    // For-pattern properties
    pub over: Name,
    pub map: Name,
    pub match_: Name,
    pub default: Name,
    // Type syntax
    pub max: Name,
}

impl KnownNames {
    /// Intern all known contextual keywords once.
    fn new(interner: &StringInterner) -> Self {
        Self {
            channel: interner.intern("channel"),
            channel_in: interner.intern("channel_in"),
            channel_out: interner.intern("channel_out"),
            channel_all: interner.intern("channel_all"),
            over: interner.intern("over"),
            map: interner.intern("map"),
            match_: interner.intern("match"),
            default: interner.intern("default"),
            max: interner.intern("max"),
        }
    }
}

/// Parser state.
pub struct Parser<'a> {
    pub(crate) cursor: Cursor<'a>,
    arena: ExprArena,
    /// Current parsing context flags.
    pub(crate) context: ParseContext,
    /// Pre-interned names for contextual keyword comparisons.
    pub(crate) known: KnownNames,
    /// Errors from sub-parsers that lack `&mut Vec<ParseError>` access
    /// (e.g., `parse_type()` detecting reserved syntax like `&T`).
    /// Drained into the main error list in `parse_module()`.
    pub(crate) deferred_errors: Vec<ParseError>,
    /// Warnings from sub-parsers (e.g., unknown calling conventions).
    /// Drained into `ParseOutput.warnings` alongside post-parse warnings.
    pub(crate) deferred_warnings: Vec<ParseWarning>,
}

impl<'a> Parser<'a> {
    /// Create a new parser.
    pub fn new(tokens: &'a TokenList, interner: &'a StringInterner) -> Self {
        // Estimate source size for pre-allocation (~5 bytes per token)
        let estimated_source_len = tokens.len() * 5;
        Parser {
            cursor: Cursor::new(tokens, interner),
            arena: ExprArena::with_capacity(estimated_source_len),
            context: ParseContext::new(),
            known: KnownNames::new(interner),
            deferred_errors: Vec::new(),
            deferred_warnings: Vec::new(),
        }
    }

    /// Estimate source size from token count for capacity hints.
    ///
    /// Heuristic: ~5 bytes per token on average.
    #[inline]
    fn estimated_source_len(&self) -> usize {
        self.cursor.token_count() * 5
    }

    /// Take ownership of the arena, replacing it with an empty one.
    ///
    /// This is useful for tests that need to access the arena after parsing.
    #[cfg(test)]
    pub fn take_arena(&mut self) -> ExprArena {
        std::mem::take(&mut self.arena)
    }
}

/// Output from parsing a module, containing the module, arena, and any errors.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct ParseOutput {
    pub module: Module,
    pub arena: SharedArena,
    pub errors: Vec<ParseError>,
    /// Non-fatal warnings (e.g., detached doc comments).
    pub warnings: Vec<ParseWarning>,
    /// Non-semantic metadata for formatting and IDE support.
    ///
    /// Contains comments, blank line positions, and other trivia
    /// that enables lossless roundtrip formatting.
    pub metadata: ModuleExtra,
}

impl ParseOutput {
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }

    // --- Post-parse analysis ---

    /// Generate warnings for detached doc comments.
    ///
    /// Call this after parsing to populate the warnings field with any
    /// doc comments that aren't attached to declarations.
    pub fn check_detached_doc_comments(&mut self) {
        // Collect all declaration start positions
        let mut decl_starts: Vec<u32> = Vec::new();

        for func in &self.module.functions {
            decl_starts.push(func.span.start);
        }
        for test in &self.module.tests {
            decl_starts.push(test.span.start);
        }
        for typ in &self.module.types {
            decl_starts.push(typ.span.start);
        }
        for trait_def in &self.module.traits {
            decl_starts.push(trait_def.span.start);
        }
        for impl_def in &self.module.impls {
            decl_starts.push(impl_def.span.start);
        }
        for ext_import in &self.module.extension_imports {
            decl_starts.push(ext_import.span.start);
        }

        // Sort for binary search efficiency (though unattached_doc_comments does linear scan)
        decl_starts.sort_unstable();

        // Find unattached doc comments
        let unattached = self.metadata.unattached_doc_comments(&decl_starts);

        for comment in unattached {
            // Determine why it's detached
            let reason = if decl_starts.is_empty() {
                DetachmentReason::NoFollowingDeclaration
            } else {
                // Find next declaration after this comment
                let next_decl = decl_starts.iter().find(|&&start| start > comment.span.end);

                match next_decl {
                    Some(&decl_start) => {
                        if self
                            .metadata
                            .has_blank_line_between(comment.span.end, decl_start)
                        {
                            DetachmentReason::BlankLine
                        } else if self
                            .metadata
                            .has_comment_between(comment.span.end, decl_start)
                        {
                            DetachmentReason::RegularCommentInterrupting
                        } else {
                            DetachmentReason::TooFarFromDeclaration
                        }
                    }
                    None => DetachmentReason::NoFollowingDeclaration,
                }
            };

            self.warnings
                .push(ParseWarning::detached_doc_comment(comment.span, reason));
        }
    }
}

/// Parse tokens into a module.
///
/// This is the basic parsing function that doesn't preserve formatting metadata.
/// For formatters and IDEs, use [`parse_with_metadata`] instead.
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseOutput {
    let parser = Parser::new(tokens, interner);
    parser.parse_module()
}

/// Parse tokens with full metadata preservation.
///
/// This function takes tokens and pre-collected metadata from the lexer,
/// producing a `ParseOutput` with full formatting information. Use this for:
/// - Formatters (lossless roundtrip)
/// - IDEs (doc comment display)
/// - Tooling that needs comment information
///
/// # Usage
///
/// Call [`ori_lexer::lex_with_comments`] first, then convert to metadata:
///
/// ```ignore
/// let lex_output = ori_lexer::lex_with_comments(source, &interner);
/// let metadata = lex_output.into_metadata();
/// let parse_output = ori_parse::parse_with_metadata(&lex_output.tokens, metadata, &interner);
///
/// // Access comments attached to declarations
/// let docs = parse_output.metadata.doc_comments_for(fn_start);
/// ```
pub fn parse_with_metadata(
    tokens: &TokenList,
    metadata: ModuleExtra,
    interner: &StringInterner,
) -> ParseOutput {
    let parser = Parser::new(tokens, interner);
    let mut output = parser.parse_module();

    // Transfer metadata from lexer
    output.metadata = metadata;

    output
}

/// Parse tokens with incremental reuse from a previous parse result.
///
/// Uses the old AST to reuse unchanged declarations, only re-parsing
/// those that overlap with the text change. This can provide significant
/// speedups for IDE scenarios where only small edits are made.
///
/// # Arguments
///
/// * `tokens` - The new token list after the edit
/// * `interner` - String interner (must be the same instance used for old result)
/// * `old_result` - The previous parse result to reuse from
/// * `change` - Description of the text change
///
/// # Returns
///
/// A new `ParseOutput` with reused declarations having adjusted spans.
pub fn parse_incremental(
    tokens: &TokenList,
    interner: &StringInterner,
    old_result: &ParseOutput,
    change: ori_ir::incremental::TextChange,
) -> ParseOutput {
    use incremental::{IncrementalState, SyntaxCursor};
    use ori_ir::incremental::ChangeMarker;

    // Find the token before the change for lookahead safety
    let prev_token_end = find_token_end_before(tokens, change.start);

    // Create the change marker with extended region
    let marker = ChangeMarker::from_change(&change, prev_token_end);

    // Create syntax cursor for navigating old AST
    let cursor = SyntaxCursor::new(&old_result.module, &old_result.arena, marker);

    // Create incremental state
    let state = IncrementalState::new(cursor);

    // Parse with incremental reuse
    let parser = Parser::new(tokens, interner);
    parser.parse_module_incremental(state, &old_result.arena)
}

/// Find the end position of the token that ends before `pos`.
///
/// This is used to determine how far back to extend the change region
/// for lookahead safety. Returns 0 if no token ends before `pos`.
///
/// Uses binary search (O(log n)) since tokens are sorted by span position.
fn find_token_end_before(tokens: &TokenList, pos: u32) -> u32 {
    let slice = tokens.as_slice();
    let idx = slice.partition_point(|t| t.span.start < pos);
    if idx > 0 {
        slice[idx - 1].span.end
    } else {
        0
    }
}
