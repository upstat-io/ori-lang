//! Block → HTML table node rendering for ARC DOT visualization.
//!
//! Each `ArcBlock` becomes a DOT node with an HTML-like table label.
//! Instructions are color-coded by category:
//! - **Green** (`#D4EDDA`): `RcInc` operations
//! - **Red** (`#F8D7DA`): `RcDec` operations
//! - **Amber** (`#FFF3CD`): Reuse/mutation ops (`Reset`, `Reuse`, `IsShared`, `Set`, `SetTag`)
//! - **Blue** (`#CCE5FF`): Terminator
//! - **Header** (`#E8E8E8`): Block ID + parameters (orange for owned, light blue for borrowed)

use std::fmt::Write;

use ori_arc::{ArcBlock, ArcFunction, ArcInstr, Ownership};
use ori_ir::StringInterner;
use ori_types::Pool;

use crate::arc_dump::instr::{fmt_instr, fmt_terminator};

/// Row background colors by instruction category.
const COLOR_HEADER: &str = "#E8E8E8";
const COLOR_RC_INC: &str = "#D4EDDA";
const COLOR_RC_DEC: &str = "#F8D7DA";
const COLOR_REUSE: &str = "#FFF3CD";
const COLOR_TERMINATOR: &str = "#CCE5FF";

/// Render a basic block as a DOT node with an HTML table label.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn render_block_node(
    out: &mut String,
    block: &ArcBlock,
    func: &ArcFunction,
    pool: &Pool,
    interner: &StringInterner,
) {
    let block_id = block.id.raw();

    write!(out, "  bb{block_id} [label=<").unwrap();
    write!(
        out,
        "<TABLE BORDER=\"1\" CELLBORDER=\"0\" CELLSPACING=\"0\">"
    )
    .unwrap();

    // Header row: block ID + parameters
    write!(
        out,
        "<TR><TD BGCOLOR=\"{COLOR_HEADER}\"><B>bb{block_id}</B>"
    )
    .unwrap();
    if !block.params.is_empty() {
        write!(out, " (").unwrap();
        for (i, (var, ty)) in block.params.iter().enumerate() {
            if i > 0 {
                write!(out, ", ").unwrap();
            }
            let ts = pool.format_type_resolved(*ty, interner);
            write!(out, "%{}: {}", var.raw(), escape_html(&ts)).unwrap();
        }
        write!(out, ")").unwrap();
    }
    write!(out, "</TD></TR>").unwrap();

    // Instruction rows
    for instr in &block.body {
        let color = instruction_color(instr);
        let mut text = String::new();
        fmt_instr(&mut text, instr, func, pool, interner);
        let escaped = escape_html(&text);

        write!(out, "<TR><TD ALIGN=\"LEFT\"").unwrap();
        if let Some(c) = color {
            write!(out, " BGCOLOR=\"{c}\"").unwrap();
        }
        write!(out, ">{escaped}</TD></TR>").unwrap();
    }

    // Terminator row
    {
        let mut text = String::new();
        fmt_terminator(&mut text, &block.terminator, func, pool, interner);
        let escaped = escape_html(&text);
        write!(
            out,
            "<TR><TD ALIGN=\"LEFT\" BGCOLOR=\"{COLOR_TERMINATOR}\">{escaped}</TD></TR>"
        )
        .unwrap();
    }

    write!(out, "</TABLE>>];\n").unwrap();
}

/// Render function parameters as a header node for the function graph.
#[expect(clippy::unwrap_used, reason = "write! to String is infallible")]
pub(super) fn render_param_header(
    out: &mut String,
    func: &ArcFunction,
    pool: &Pool,
    interner: &StringInterner,
) {
    write!(
        out,
        "  params [label=<\
         <TABLE BORDER=\"1\" CELLBORDER=\"0\" CELLSPACING=\"0\">"
    )
    .unwrap();

    let name = interner.lookup(func.name);
    let ret_str = pool.format_type_resolved(func.return_type, interner);
    write!(
        out,
        "<TR><TD BGCOLOR=\"#D0D0D0\"><B>fn @{}</B> -&gt; {}</TD></TR>",
        escape_html(name),
        escape_html(&ret_str),
    )
    .unwrap();

    for param in &func.params {
        let ty = pool.format_type_resolved(param.ty, interner);
        let (own_str, color) = match param.ownership {
            Ownership::Owned => ("own", "#FFE0B2"),       // orange
            Ownership::Borrowed => ("borrow", "#BBDEFB"), // light blue
        };
        write!(
            out,
            "<TR><TD ALIGN=\"LEFT\" BGCOLOR=\"{color}\">%{}: {} [{own_str}]</TD></TR>",
            param.var.raw(),
            escape_html(&ty),
        )
        .unwrap();
    }

    write!(out, "</TABLE>>];\n").unwrap();
    // Edge from param header to entry block
    write!(out, "  params -> bb{} [style=dashed];\n", func.entry.raw()).unwrap();
}

/// Determine the background color for an instruction row.
fn instruction_color(instr: &ArcInstr) -> Option<&'static str> {
    match instr {
        ArcInstr::RcInc { .. } => Some(COLOR_RC_INC),
        ArcInstr::RcDec { .. } => Some(COLOR_RC_DEC),
        ArcInstr::IsShared { .. }
        | ArcInstr::Set { .. }
        | ArcInstr::SetTag { .. }
        | ArcInstr::Reset { .. }
        | ArcInstr::Reuse { .. } => Some(COLOR_REUSE),
        _ => None,
    }
}

/// Escape a string for use in an HTML-like DOT label.
///
/// DOT's HTML-like labels use a subset of HTML — `<`, `>`, `&`, and `"`
/// all need escaping.
pub(super) fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}
