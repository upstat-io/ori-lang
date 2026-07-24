use ori_ir::{ModuleExtra, StringInterner, TokenList};

use crate::{ParseOutput, Parser};

/// Parse tokens without formatting metadata.
#[tracing::instrument(level = "debug", skip_all)]
pub fn parse(tokens: &TokenList, interner: &StringInterner) -> ParseOutput {
    Parser::new(tokens, interner).parse_module()
}

/// Parse tokens with lexer-collected formatting metadata.
#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_with_metadata(
    tokens: &TokenList,
    metadata: ModuleExtra,
    interner: &StringInterner,
) -> ParseOutput {
    let mut output = Parser::new(tokens, interner).parse_module();
    output.metadata = metadata;
    output
}

/// Parse tokens while reusing unchanged declarations from `old_result`.
#[tracing::instrument(level = "debug", skip_all)]
pub fn parse_incremental(
    tokens: &TokenList,
    interner: &StringInterner,
    old_result: &ParseOutput,
    change: ori_ir::incremental::TextChange,
) -> ParseOutput {
    use ori_ir::incremental::ChangeMarker;

    use crate::incremental::{IncrementalState, SyntaxCursor};

    let previous_end = find_token_end_before(tokens, change.start);
    let marker = ChangeMarker::from_change(&change, previous_end);
    let cursor = SyntaxCursor::new(&old_result.module, &old_result.arena, marker);
    let state = IncrementalState::new(cursor);
    Parser::new(tokens, interner).parse_module_incremental(state, &old_result.arena)
}

fn find_token_end_before(tokens: &TokenList, pos: u32) -> u32 {
    let slice = tokens.as_slice();
    let index = slice.partition_point(|token| token.span.start < pos);
    index
        .checked_sub(1)
        .map_or(0, |previous| slice[previous].span.end)
}
