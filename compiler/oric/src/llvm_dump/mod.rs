//! Annotated LLVM IR pretty-printer for phase dumps.
//!
//! Post-processes raw LLVM IR text to add Ori-aware annotations:
//! - Function name demangling (`_ori_main` → `@main`)
//! - RC operation type hints (`RC++`/`RC-- <type>` from drop function names)
//! - COW operation annotations (collection + operation name)
//!
//! This is the in-compiler equivalent of `diagnostics/ir-dump.sh`, triggered via
//! `ORI_DUMP_AFTER_LLVM=1` (or the legacy `ORI_DEBUG_LLVM=1` alias).

use std::fmt::Write;

use ori_ir::StringInterner;
use ori_llvm::aot::mangle::demangle;
use ori_types::{Idx, Pool};

/// Dump annotated LLVM IR to stderr.
///
/// Reads raw IR text line-by-line and injects inline annotations for:
/// - `define` lines: demangled Ori function name + signature header
/// - `ori_rc_inc` calls: `; RC++` marker
/// - `ori_rc_dec` calls: `; RC-- <type>` with type resolved from drop function
/// - `ori_list_*_cow` / `ori_str_*_cow` calls: `; COW <op> <collection>` marker
///
/// Output goes to stderr (consistent with other phase dumps).
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub fn dump_llvm_ir(ir_text: &str, pool: &Pool, interner: &StringInterner, path: &str) {
    let mut out = String::with_capacity(ir_text.len() + 4096);

    writeln!(out, "=== LLVM IR after codegen: {path} ===").unwrap();

    for line in ir_text.lines() {
        // Function header: demangle and emit annotation before the define line
        if line.starts_with("define ") {
            if let Some(ori_name) = demangle_define(line, pool, interner) {
                writeln!(out, "; --- {ori_name} ---").unwrap();
            }
        }

        // Write the original line
        write!(out, "{line}").unwrap();

        // Append inline RC/COW annotations
        if let Some(annotation) = annotate_rc_op(line, pool, interner) {
            write!(out, "  ; {annotation}").unwrap();
        } else if let Some(annotation) = annotate_cow_op(line) {
            write!(out, "  ; {annotation}").unwrap();
        }

        writeln!(out).unwrap();
    }

    writeln!(out, "=== END LLVM IR ===").unwrap();
    eprint!("{out}");
}

/// Check if LLVM IR dumping is requested (new or legacy flag).
///
/// Returns `true` if either `ORI_DUMP_AFTER_LLVM` or `ORI_DEBUG_LLVM` is set.
/// In release builds, always returns `false` (zero overhead).
pub fn llvm_dump_requested() -> bool {
    crate::dbg_set!(crate::debug_flags::ORI_DUMP_AFTER_LLVM)
        || crate::dbg_set!(crate::debug_flags::ORI_DEBUG_LLVM)
}

// --- Annotation helpers ---

/// Extract and demangle the function name from a `define` line.
///
/// Handles both unquoted (`@_ori_main`) and quoted (`@"_ori_drop$3"`) names.
/// Drop functions (`_ori_drop$N`) are handled specially — the pool index is
/// resolved to the Ori type name (e.g., `drop [int]` instead of `drop.@96`).
fn demangle_define(line: &str, pool: &Pool, interner: &StringInterner) -> Option<String> {
    let at_pos = line.find('@')?;
    let after_at = &line[at_pos + 1..];

    // Quoted names: @"_ori_foo$bar"(...)
    let name = if after_at.starts_with('"') {
        let end_quote = after_at[1..].find('"')?;
        &after_at[1..1 + end_quote]
    } else {
        // Unquoted: @_ori_main(...)
        let end = after_at.find('(')?;
        &after_at[..end]
    };

    // Drop functions: resolve pool index to type name
    if let Some(drop_idx) = name.strip_prefix("_ori_drop$") {
        let raw: u32 = drop_idx.parse().ok()?;
        let idx = Idx::from_raw(raw);
        if !idx.is_none() && !idx.is_error() {
            let ty = pool.format_type_resolved(idx, interner);
            return Some(format!("drop {ty}"));
        }
    }

    demangle(name)
}

/// Annotate RC increment/decrement calls with type information.
///
/// Only annotates `call` instructions — skips `declare` lines. For `ori_rc_dec`,
/// extracts the drop function name (`_ori_drop$N`) to resolve the Ori type via
/// Pool lookup. For `ori_rc_inc`, no type info is directly available.
fn annotate_rc_op(line: &str, pool: &Pool, interner: &StringInterner) -> Option<String> {
    // Only annotate call instructions, not declarations
    let trimmed = line.trim_start();
    if !trimmed.starts_with("call ") && !trimmed.starts_with("invoke ") {
        return None;
    }

    if line.contains("@ori_rc_inc") {
        Some("RC++".to_string())
    } else if line.contains("@ori_rc_dec") {
        let type_hint = extract_drop_type(line, pool, interner);
        match type_hint {
            Some(ty) => Some(format!("RC-- {ty}")),
            None => Some("RC--".to_string()),
        }
    } else {
        None
    }
}

/// Extract the Ori type from a drop function reference in an `ori_rc_dec` call.
///
/// Drop functions follow the naming convention `_ori_drop$<idx_raw>`, where
/// `<idx_raw>` is the raw Pool index. The function parses this index and
/// resolves the type via `Pool::format_type_resolved`.
fn extract_drop_type(line: &str, pool: &Pool, interner: &StringInterner) -> Option<String> {
    let drop_marker = "_ori_drop$";
    let pos = line.find(drop_marker)?;
    let after = &line[pos + drop_marker.len()..];

    // Extract digits (the raw index)
    let idx_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    let raw: u32 = idx_str.parse().ok()?;

    let idx = Idx::from_raw(raw);

    // Validate: index must be within the pool's range
    if idx.is_none() || idx.is_error() {
        return None;
    }

    Some(pool.format_type_resolved(idx, interner))
}

/// Annotate COW (copy-on-write) operation calls.
///
/// Matches runtime functions like `ori_list_push_cow`, `ori_str_concat_cow`,
/// `ori_map_insert_cow` and extracts the collection type + operation name.
fn annotate_cow_op(line: &str) -> Option<String> {
    for (prefix, collection) in [
        ("ori_list_", "list"),
        ("ori_str_", "str"),
        ("ori_map_", "map"),
        ("ori_set_", "set"),
    ] {
        if let Some(annotation) = try_extract_cow(line, prefix, collection) {
            return Some(annotation);
        }
    }
    None
}

/// Try to extract a COW annotation for a specific collection type.
///
/// Returns `Some("COW <op> <collection>")` if the line contains a `_cow` suffixed
/// call for the given collection prefix, `None` otherwise.
fn try_extract_cow(line: &str, fn_prefix: &str, collection: &str) -> Option<String> {
    let at_prefix = format!("@{fn_prefix}");
    let pos = line.find(&at_prefix)?;
    let after = &line[pos + 1..]; // skip '@'

    // Check that this call contains "_cow"
    let call_end = after.find(|c: char| c == '(' || c.is_whitespace())?;
    let fn_name = &after[..call_end];
    if !fn_name.ends_with("_cow") {
        return None;
    }

    // Extract operation: strip prefix and "_cow" suffix
    let op = fn_name.strip_prefix(fn_prefix)?.strip_suffix("_cow")?;

    Some(format!("COW {op} {collection}"))
}
