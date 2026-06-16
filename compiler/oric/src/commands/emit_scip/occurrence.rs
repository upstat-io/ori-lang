//! SCIP reference-`Occurrence` emission for type-checker-resolved call sites.
//!
//! Walks the expression arena and, for every call site whose target the type
//! checker resolved, mints the target's SCIP symbol string via [`ModuleMinter`]
//! (byte-identical to the definition side) and records a `scip::types::Occurrence`
//! carrying that symbol, the call-site range, and a reference role bit.
//!
//! The downstream occurrence -> definition join keys on exact symbol-string
//! equality, so a trait-method call references the concrete impl method's symbol
//! (`Type#method().`) — discriminated by the receiver's resolved type, never the
//! syntactic method name alone. Per-kind coverage lives on the
//! [`method_call_occurrence`] and [`free_call_occurrence`] minters.

use ori_ir::{ExprArena, ExprId, ExprKind, Module, Name, Span, StringInterner};
use ori_types::{Pool, Tag, TypedModule};
use scip::types::SymbolRole;
use std::collections::BTreeSet;

use super::symbol::{ModuleMinter, NameAnchor, ScipDef};

/// `SymbolRole` bitset emitted for a reference occurrence: a call READS the
/// callable, so `ReadAccess` is the reference role (NOT `Definition`, which is
/// reserved for the def site).
pub const REFERENCE_ROLE: i32 = SymbolRole::ReadAccess as i32;

/// `SymbolRole` bitset emitted for a definition occurrence: the def site of a
/// declaration. Caller-attribution containment keys on the Definition
/// occurrence's `enclosing_range`.
pub const DEFINITION_ROLE: i32 = SymbolRole::Definition as i32;

/// One emitted occurrence: its SCIP symbol string, its 0-based identifier-token
/// `range`, the `symbol_roles` bitset, and (for a Definition occurrence) the
/// body-spanning `enclosing_range`. Reference occurrences carry `range` = the
/// call-site token, empty `enclosing_range`, role `REFERENCE_ROLE`; Definition
/// occurrences carry `range` = the IDENTIFIER-token span, `enclosing_range` =
/// the definition's full extent, role `DEFINITION_ROLE`. Ordered by
/// `(range, symbol)` for a byte-deterministic stable-sort + dedup.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct ScipOccurrence {
    /// 0-based `[start_line, start_col, end_line, end_col]` of the identifier
    /// token (Definition) or the reference token (reference).
    pub range: Vec<i32>,
    /// 0-based `[start_line, start_col, end_line, end_col]` of the definition's
    /// full source extent (Definition occurrences only; empty for references).
    pub enclosing_range: Vec<i32>,
    /// The globally-stable SCIP symbol string of the resolved target.
    pub symbol: String,
    /// The SCIP `SymbolRole` bitset: `REFERENCE_ROLE` or `DEFINITION_ROLE`.
    pub symbol_roles: i32,
}

/// The read-only resolution context threaded to the per-kind occurrence
/// minters: the symbol minter, the IR views method/free dispatch resolves
/// against, the interner, the line index, and the in-module free-function set.
struct OccurrenceCtx<'a> {
    minter: &'a ModuleMinter,
    arena: &'a ExprArena,
    typed: &'a TypedModule,
    pool: &'a Pool,
    interner: &'a StringInterner,
    lines: &'a LineIndex,
    /// Names of functions declared in THIS module — a free-function occurrence
    /// is minted only when its callee is in this set (so the occurrence ->
    /// definition join lands; prelude / builtin free functions have no
    /// in-module definition to reference).
    local_fns: &'a BTreeSet<&'a str>,
}

/// Collect one [`ScipOccurrence`] per resolvable call site in the module,
/// dispatching method calls to [`method_call_occurrence`] and free-function
/// calls to [`free_call_occurrence`].
///
/// # Returns
///
/// The canonicalized occurrence set (stable sort + dedup on `(range, symbol)`).
pub fn collect_occurrences(
    module_id: &str,
    arena: &ExprArena,
    module: &Module,
    typed: &TypedModule,
    pool: &Pool,
    interner: &StringInterner,
    source: &str,
) -> Vec<ScipOccurrence> {
    let minter = ModuleMinter::new(module_id);
    let lines = LineIndex::new(source);
    let local_fns: BTreeSet<&str> = module
        .functions
        .iter()
        .map(|f| interner.lookup(f.name))
        .collect();
    let ctx = OccurrenceCtx {
        minter: &minter,
        arena,
        typed,
        pool,
        interner,
        lines: &lines,
        local_fns: &local_fns,
    };

    let mut occurrences = Vec::new();
    let expr_count = u32::try_from(arena.expr_count()).unwrap_or(u32::MAX);

    for raw in 0..expr_count {
        let id = ExprId::new(raw);
        let occurrence = match arena.expr_kind(id) {
            // Method calls mint `<ReceiverType>#method().` from the receiver's resolved type.
            ExprKind::MethodCall {
                receiver, method, ..
            }
            | ExprKind::MethodCallNamed {
                receiver, method, ..
            } => method_call_occurrence(&ctx, *receiver, *method, arena.expr_span(id)),
            // Free-function calls mint `fn().` for functions declared in this module.
            ExprKind::Call { func, .. } | ExprKind::CallNamed { func, .. } => {
                free_call_occurrence(&ctx, *func)
            }
            _ => None,
        };
        if let Some(occurrence) = occurrence {
            occurrences.push(occurrence);
        }
    }

    occurrences.sort();
    occurrences.dedup();
    occurrences
}

/// Mint the `<ReceiverType>#method().` occurrence for a method call, keyed on
/// the receiver's resolved concrete type.
///
/// # Returns
///
/// `None` for a non-nominal receiver (builtin / primitive / unresolved). A
/// trait-default / def-impl call mints `<ReceiverType>#method().` while the def
/// side mints `<Trait>#method().`; the two differ, so it dangles non-fatally.
fn method_call_occurrence(
    ctx: &OccurrenceCtx,
    receiver: ExprId,
    method: Name,
    span: Span,
) -> Option<ScipOccurrence> {
    let parent = resolved_receiver_type_name(receiver, ctx.typed, ctx.pool, ctx.interner)?;
    let symbol = ctx
        .minter
        .method(parent, ctx.interner.lookup(method), span)
        .symbol;
    Some(ScipOccurrence {
        range: ctx.lines.range(span),
        enclosing_range: Vec::new(),
        symbol,
        symbol_roles: REFERENCE_ROLE,
    })
}

/// Mint the `fn().` occurrence for a free-function call.
///
/// # Returns
///
/// `None` unless the callee is a bare function reference declared in THIS
/// module (`ctx.local_fns`); prelude / builtin free functions have no in-module
/// definition to reference. The reference range is the callee-identifier token.
fn free_call_occurrence(ctx: &OccurrenceCtx, func: ExprId) -> Option<ScipOccurrence> {
    let name = callee_function_name(func, ctx.arena)?;
    let name_str = ctx.interner.lookup(name);
    if !ctx.local_fns.contains(name_str) {
        return None;
    }
    let func_span = ctx.arena.expr_span(func);
    let symbol = ctx.minter.function(name_str, func_span).symbol;
    Some(ScipOccurrence {
        range: ctx.lines.range(func_span),
        enclosing_range: Vec::new(),
        symbol,
        symbol_roles: REFERENCE_ROLE,
    })
}

/// Build one `Definition`-role [`ScipOccurrence`] per per-SITE definition in
/// `raw_defs` (the PRE-DEDUP list — distinct sites minting the same symbol each
/// get their own Definition occurrence).
///
/// Each occurrence's `range` is the IDENTIFIER-TOKEN span (the name span,
/// anchored at the declaration keyword per the def's [`NameAnchor`]) and its
/// `enclosing_range` is the definition's full source extent (body included), so
/// the importer keys caller-attribution containment on the body-spanning extent
/// (SCIP convention: rust-analyzer / scip-clang / scip-java).
pub fn collect_definition_occurrences(raw_defs: &[ScipDef], source: &str) -> Vec<ScipOccurrence> {
    let lines = LineIndex::new(source);
    let mut occurrences = Vec::new();
    for def in raw_defs {
        let ident = identifier_token_span(source, def.span, &def.display_name, def.name_anchor);
        occurrences.push(ScipOccurrence {
            range: lines.range(ident),
            enclosing_range: lines.range(def.span),
            symbol: def.symbol.clone(),
            symbol_roles: DEFINITION_ROLE,
        });
    }
    occurrences.sort();
    occurrences.dedup();
    occurrences
}

/// Derive the identifier-token [`Span`] for a definition whose full extent is
/// `item_span`, by skipping the declaration prefix from `item_span.start` per
/// `anchor`, then matching `name` at the first identifier position.
///
/// Anchored at the declaration keyword (NOT a bare substring search): the scan
/// starts at the item's first byte and consumes the known prefix (`@` sigil for
/// functions/methods, `pub`/`type`/`trait`/`impl` keyword for type/trait decls,
/// nothing for fields/variants), so the name span never false-matches the name
/// text inside a body, doc comment, or string literal. Falls back to the full
/// item span when the source slice cannot be located (never panics).
fn identifier_token_span(source: &str, item_span: Span, name: &str, anchor: NameAnchor) -> Span {
    let start = item_span.start as usize;
    let end = (item_span.end as usize).min(source.len());
    if start >= end {
        return item_span;
    }
    let slice = &source[start..end];

    // Offset (within `slice`) where the identifier scan begins, after skipping
    // the anchor's declaration prefix.
    let scan_from = match anchor {
        // `@name` / `pub @name`: skip to the `@` sigil, then one byte past it.
        NameAnchor::AtSigil => match slice.find('@') {
            Some(at) => at + 1,
            None => 0,
        },
        // `type Name` / `trait Name` / `pub type Name`: skip the leading
        // declaration keyword token, then whitespace, to the name.
        NameAnchor::Keyword => skip_declaration_keyword(slice),
        // The item span starts at the name token itself.
        NameAnchor::Direct => 0,
    };

    // Find `name` at or after `scan_from`, requiring an identifier boundary so a
    // shorter name does not match inside a longer leading token.
    if let Some(rel) = locate_identifier(&slice[scan_from..], name) {
        let name_start = start + scan_from + rel;
        let name_end = name_start + name.len();
        if name_end <= end {
            return Span {
                start: name_start as u32,
                end: name_end as u32,
            };
        }
    }
    item_span
}

/// Skip a leading `pub` modifier then the declaration keyword (`type` / `trait`)
/// and the whitespace after it, returning the byte offset of the name token.
/// Returns 0 when no keyword is recognized (the name is then matched from the
/// slice start).
fn skip_declaration_keyword(slice: &str) -> usize {
    let mut rest = slice.trim_start();
    let mut consumed = slice.len() - rest.len();

    // Optional `pub ` visibility modifier.
    if let Some(after_pub) = rest.strip_prefix("pub") {
        if starts_with_boundary(after_pub) {
            consumed += 3;
            let trimmed = after_pub.trim_start();
            consumed += after_pub.len() - trimmed.len();
            rest = trimmed;
        }
    }

    for kw in ["type", "trait"] {
        if let Some(after_kw) = rest.strip_prefix(kw) {
            if starts_with_boundary(after_kw) {
                consumed += kw.len();
                let trimmed = after_kw.trim_start();
                consumed += after_kw.len() - trimmed.len();
                return consumed;
            }
        }
    }
    consumed
}

/// True iff `s` is empty or begins with a non-identifier byte (so the preceding
/// token was a complete keyword, not a prefix of a longer identifier).
fn starts_with_boundary(s: &str) -> bool {
    match s.as_bytes().first() {
        None => true,
        Some(&b) => !(b.is_ascii_alphanumeric() || b == b'_'),
    }
}

/// Find `name` in `haystack` at an identifier boundary (the char before the
/// match is not an identifier char, and the char after is not either), so a
/// short name does not match inside a longer adjacent token. Returns the byte
/// offset within `haystack`, or `None`.
fn locate_identifier(haystack: &str, name: &str) -> Option<usize> {
    if name.is_empty() {
        return None;
    }
    let bytes = haystack.as_bytes();
    let nbytes = name.as_bytes();
    let mut i = 0;
    while i + nbytes.len() <= bytes.len() {
        if &bytes[i..i + nbytes.len()] == nbytes {
            let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
            let after_idx = i + nbytes.len();
            let after_ok = after_idx >= bytes.len() || !is_ident_byte(bytes[after_idx]);
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// True iff `b` is an ASCII identifier byte (alphanumeric or `_`).
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Resolve a method-call receiver's type-checker-resolved type to its nominal
/// type name, or `None` when the receiver is not a user-defined nominal type
/// (builtin container, primitive, unresolved) — those receivers have no def-
/// side definition to reference.
fn resolved_receiver_type_name<'a>(
    receiver: ExprId,
    typed: &TypedModule,
    pool: &Pool,
    interner: &'a StringInterner,
) -> Option<&'a str> {
    let idx = typed.expr_type(receiver.index())?;
    let resolved = pool.resolve_fully(idx);
    let name = match pool.tag(resolved) {
        Tag::Struct => pool.struct_name(resolved),
        Tag::Enum => pool.enum_name(resolved),
        Tag::Named => pool.named_name(resolved),
        Tag::Applied => pool.applied_name(resolved),
        _ => return None,
    };
    Some(interner.lookup(name))
}

/// The callee function name of a `Call` / `CallNamed`, when the callee is a
/// bare function reference (`Ident` or `@`-prefixed `FunctionRef`).
fn callee_function_name(func: ExprId, arena: &ExprArena) -> Option<Name> {
    match arena.expr_kind(func) {
        ExprKind::Ident(name) | ExprKind::FunctionRef(name) => Some(*name),
        _ => None,
    }
}

/// Byte-offset -> 0-based `(line, column)` index over the source text.
///
/// Columns are UTF-8 byte offsets within the line. Deterministic and derived
/// purely from the source bytes.
struct LineIndex {
    /// Byte offset of the first character of each line (line 0 starts at 0).
    line_starts: Vec<u32>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        let mut offset: u32 = 0;
        for b in source.bytes() {
            offset = offset.saturating_add(1);
            if b == b'\n' {
                line_starts.push(offset);
            }
        }
        Self { line_starts }
    }

    /// 0-based `(line, column)` for a byte offset.
    fn position(&self, offset: u32) -> (u32, u32) {
        let line = match self.line_starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next.saturating_sub(1),
        };
        let line_start = self.line_starts.get(line).copied().unwrap_or(0);
        let column = offset.saturating_sub(line_start);
        (u32::try_from(line).unwrap_or(u32::MAX), column)
    }

    /// 0-based `[start_line, start_col, end_line, end_col]` for a span.
    fn range(&self, span: Span) -> Vec<i32> {
        let (start_line, start_col) = self.position(span.start);
        let (end_line, end_col) = self.position(span.end);
        vec![
            to_i32(start_line),
            to_i32(start_col),
            to_i32(end_line),
            to_i32(end_col),
        ]
    }
}

/// Saturating `u32 -> i32` (SCIP ranges are `i32`); positions never exceed
/// `i32::MAX` in any real source file.
fn to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
