//! The `emit-scip` command: emit a minimal SCIP index for a source file.
//!
//! Walking-skeleton vertical slice: runs the frontend (lex -> parse ->
//! typecheck) and emits one trivial `SymbolInformation` for the first
//! top-level function into a self-describing `index.scip`. No occurrences,
//! no resolution, no provenance — the de-risking spine the later
//! protobuf-`scip`-crate section migrates onto.

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use oric::{CompilerDb, Db, SourceFile};
use serde::Serialize;
use std::path::PathBuf;

use super::read_file;
use super::report_frontend_errors;

/// SCIP symbol kind. Mirrors the `scip.SymbolInformation.Kind` enum subset the
/// skeleton emits; the later protobuf section maps this onto `scip::SymbolKind`.
#[derive(Serialize)]
enum ScipSymbolKind {
    Function,
}

/// One logical SCIP `SymbolInformation`.
#[derive(Serialize)]
struct ScipSymbolInformation {
    /// SCIP symbol moniker: `<scheme> <manager> <package> <version> <descriptor>`.
    symbol: String,
    /// Human-readable identifier (the source name).
    display_name: String,
    kind: ScipSymbolKind,
}

/// One logical SCIP `Document` (a single source file).
#[derive(Serialize)]
struct ScipDocument {
    relative_path: String,
    symbols: Vec<ScipSymbolInformation>,
}

/// Index-level metadata. Mirrors `scip.Metadata`.
#[derive(Serialize)]
struct ScipMetadata {
    tool: String,
    version: String,
    project_root: String,
}

/// Minimal self-describing SCIP index. Structurally mirrors the `scip.Index`
/// protobuf (`metadata` + `documents[]`) so the later real-`scip`-crate
/// section has a 1:1 migration target.
#[derive(Serialize)]
struct ScipIndex {
    /// Format tag so a consumer can detect the minimal (pre-protobuf) shape.
    format: String,
    metadata: ScipMetadata,
    documents: Vec<ScipDocument>,
}

/// SCIP symbol scheme for Ori symbols.
const SCIP_SCHEME: &str = "ori";

/// Minimal-format tag. Bumped when the later protobuf section lands.
const SCIP_MINIMAL_FORMAT: &str = "ori-scip-minimal/v1";

/// Build the SCIP symbol moniker for a top-level function name.
///
/// Format: `<scheme> <manager> <package> <version> <descriptor>` with empty
/// components rendered as `.` and a function descriptor of `name().`.
fn function_moniker(name: &str) -> String {
    format!("{SCIP_SCHEME} . . . {name}().")
}

/// Emit a minimal SCIP index for `path` into `output` (default `index.scip`).
///
/// Runs the frontend, emits one `SymbolInformation` for the first top-level
/// function, and writes the index as self-describing JSON. Exits non-zero on
/// frontend errors (mirrors `check`) or on output write failure.
pub fn emit_scip_file(path: &str, output: &str) {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    let Some(frontend) = report_frontend_errors(&db, file, &mut emitter) else {
        std::process::exit(1);
    };

    if frontend.has_errors() {
        emitter.flush();
        std::process::exit(1);
    }

    // Skeleton: emit one symbol for the first top-level function (functions are
    // sorted by name in TypedModule, so this is deterministic).
    let interner = db.interner();
    let symbols: Vec<ScipSymbolInformation> = frontend
        .type_result
        .typed
        .functions
        .first()
        .map(|func| {
            let name = interner.lookup(func.name).to_string();
            vec![ScipSymbolInformation {
                symbol: function_moniker(&name),
                display_name: name,
                kind: ScipSymbolKind::Function,
            }]
        })
        .unwrap_or_default();

    // Determine the project root for index metadata. A failed current_dir()
    // must abort with a diagnostic (mirrors the serialize/write failure paths
    // below) — silently recording an empty project_root would emit a
    // structurally-wrong index a consumer would act on.
    let project_root = match std::env::current_dir() {
        Ok(dir) => dir.display().to_string(),
        Err(e) => {
            eprintln!("error: cannot determine project root for SCIP index: {e}");
            std::process::exit(1);
        }
    };

    let index = ScipIndex {
        format: SCIP_MINIMAL_FORMAT.to_string(),
        metadata: ScipMetadata {
            tool: "oric".to_string(),
            version: oric::version::report_version(),
            project_root,
        },
        documents: vec![ScipDocument {
            relative_path: path.to_string(),
            symbols,
        }],
    };

    let json = match serde_json::to_string_pretty(&index) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("error: failed to serialize SCIP index: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = std::fs::write(output, json) {
        eprintln!("error: failed to write '{output}': {e}");
        std::process::exit(1);
    }

    let symbol_count = index
        .documents
        .iter()
        .map(|d| d.symbols.len())
        .sum::<usize>();
    println!("Wrote {output} ({symbol_count} symbols) for {path}");
}
