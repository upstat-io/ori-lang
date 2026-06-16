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

use super::symbol::ModuleMinter;

/// `SymbolRole` bitset emitted for a reference occurrence: a call READS the
/// callable, so `ReadAccess` is the reference role (NOT `Definition`, which is
/// reserved for the def site).
pub const REFERENCE_ROLE: i32 = SymbolRole::ReadAccess as i32;

/// One emitted reference occurrence: its SCIP symbol string and its 0-based
/// `[start_line, start_col, end_line, end_col]` range.
///
/// Ordered by `(range, symbol)` so the caller can stable-sort + dedup for a
/// byte-deterministic index.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct ScipOccurrence {
    /// 0-based `[start_line, start_col, end_line, end_col]`.
    pub range: Vec<i32>,
    /// The globally-stable SCIP symbol string of the resolved target.
    pub symbol: String,
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
        .method(parent, ctx.interner.lookup(method))
        .symbol;
    Some(ScipOccurrence {
        range: ctx.lines.range(span),
        symbol,
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
    let symbol = ctx.minter.function(name_str).symbol;
    Some(ScipOccurrence {
        range: ctx.lines.range(ctx.arena.expr_span(func)),
        symbol,
    })
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
