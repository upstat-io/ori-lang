//! Map-key bracket rule (Layer 4).
//!
//! Decides whether a map-literal key must be re-emitted wrapped in `[ ]` to
//! round-trip the source. Sibling delimiter-decision rule to
//! [`super::needs_parens`]; consumed by both the width calculator (Layer 3) and
//! the formatter (Layer 5).
//!
//! Spec: grammar.ebnf § `map_key`.

use ori_ir::ExprKind;

/// Whether a map-literal key must be emitted wrapped in `[ ]`.
///
/// Per grammar.ebnf § `map_key`, a bare (unbracketed) key is only valid when it
/// is a literal the parser accepts in key position — `string_literal`,
/// `identifier` (parsed to an interned `String`), or a bare `Int`/`Bool`/`Char`
/// literal. Every other key node (`Ident`, `Float`, a method call, an operator
/// expression, ...) exists in source only as a computed `[ expr ]` key, so the
/// formatter re-emits the brackets to round-trip the AST shape.
pub fn map_key_needs_brackets(kind: &ExprKind) -> bool {
    !matches!(
        kind,
        ExprKind::String(_) | ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Char(_)
    )
}
