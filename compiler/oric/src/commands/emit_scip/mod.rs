//! The `emit-scip` command: emit a SCIP definition index for a source file.
//!
//! Runs the frontend (lex -> parse -> typecheck) and, when it is error-free,
//! emits a `scip` protobuf `Index` (`index.scip`) carrying one
//! `SymbolInformation` per declared definition entity — every top-level
//! function, type, trait, impl method, trait-method, default-impl method,
//! extension method, struct field, sum-type variant, and variant field — each
//! with a globally-stable, deterministic SCIP symbol string carrying the source
//! module's identity (so cross-file homonyms never collide). A frontend error
//! suppresses emission and exits non-zero (mirrors `check`).

mod symbol;

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_ir::{Module, StringInterner, TraitItem, TypeDeclKind};
use oric::{CompilerDb, Db, SourceFile};
use protobuf::{Message, MessageField};
use scip::types::symbol_information::Kind;
use scip::types::{Document, Index, Metadata, SymbolInformation, ToolInfo};
use std::path::{Path, PathBuf};

use super::read_file;
use super::report_frontend_errors;
use symbol::{ModuleMinter, ScipDef};

/// Collect one [`ScipDef`] per declared definition entity in `module`.
///
/// Walks the parsed module (functions, types + fields/variants, traits +
/// methods, inherent + trait impl blocks, default impls, extensions) in source
/// order, minting every symbol under `module_id` so cross-file homonyms stay
/// distinct. Output order is canonicalized by the caller (stable sort + dedup
/// on the symbol string), so the emitted index is independent of source
/// ordering.
fn collect_defs(module_id: &str, module: &Module, interner: &StringInterner) -> Vec<ScipDef> {
    let minter = ModuleMinter::new(module_id);
    let mut defs = Vec::new();

    for func in &module.functions {
        defs.push(minter.function(interner.lookup(func.name)));
    }

    for ty in &module.types {
        let type_name = interner.lookup(ty.name);
        match &ty.kind {
            TypeDeclKind::Struct(fields) => {
                defs.push(minter.type_def(type_name, Kind::Struct));
                for f in fields {
                    defs.push(minter.field(type_name, interner.lookup(f.name)));
                }
            }
            TypeDeclKind::Sum(variants) => {
                defs.push(minter.type_def(type_name, Kind::Enum));
                for v in variants {
                    let variant_name = interner.lookup(v.name);
                    defs.push(minter.variant(type_name, variant_name));
                    for vf in &v.fields {
                        defs.push(minter.variant_field(
                            type_name,
                            variant_name,
                            interner.lookup(vf.name),
                        ));
                    }
                }
            }
            // A newtype is a distinct nominal type with no fields to index.
            TypeDeclKind::Newtype(_) => {
                defs.push(minter.type_def(type_name, Kind::Type));
            }
        }
    }

    for tr in &module.traits {
        let trait_name = interner.lookup(tr.name);
        defs.push(minter.trait_def(trait_name));
        for item in &tr.items {
            let method_name = match item {
                TraitItem::MethodSig(m) => Some(m.name),
                TraitItem::DefaultMethod(m) => Some(m.name),
                TraitItem::AssocType(_) => None,
            };
            if let Some(method_name) = method_name {
                defs.push(minter.method(trait_name, interner.lookup(method_name)));
            }
        }
    }

    for imp in &module.impls {
        let Some(type_name_id) = imp.type_name() else {
            continue;
        };
        let type_name = interner.lookup(type_name_id);
        for m in &imp.methods {
            defs.push(minter.method(type_name, interner.lookup(m.name)));
        }
    }

    // A `pub def impl Trait { @m }` declares default-impl methods keyed on the
    // trait; an `extend Type { @m }` declares extension methods keyed on the
    // target type. Both are real definitions downstream references resolve to.
    for di in &module.def_impls {
        let trait_name = interner.lookup(di.trait_name);
        for m in &di.methods {
            defs.push(minter.method(trait_name, interner.lookup(m.name)));
        }
    }

    for ext in &module.extends {
        let target_name = interner.lookup(ext.target_type_name);
        for m in &ext.methods {
            defs.push(minter.method(target_name, interner.lookup(m.name)));
        }
    }

    // Canonical order: sort by symbol string, then collapse any duplicate
    // symbols to one definition entry (a definition index has at most one
    // entry per symbol — e.g. an inherent + trait impl method that mint the
    // same `Type#m().`). Deterministic regardless of source ordering.
    defs.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    defs.dedup_by(|a, b| a.symbol == b.symbol);
    defs
}

/// Derive the project-relative source path used as the SCIP document path and
/// the basis of the module identity.
///
/// An absolute path under `project_root` is stripped to its relative tail; an
/// absolute path outside the root falls back to its file name. The result is
/// forward-slash normalized so the identity is stable across platforms and
/// never embeds an absolute, machine-specific directory.
fn project_relative_path(path: &str, project_root: &str) -> String {
    let p = Path::new(path);
    let rel = if p.is_absolute() {
        match p.strip_prefix(project_root) {
            Ok(stripped) => stripped.to_path_buf(),
            Err(_) => PathBuf::from(p.file_name().unwrap_or(p.as_os_str())),
        }
    } else {
        p.to_path_buf()
    };
    rel.to_string_lossy().replace('\\', "/")
}

/// The module identity injected into every minted symbol: the project-relative
/// path with its `.ori` extension dropped. Deterministic + machine-independent.
fn module_identity(relative_path: &str) -> String {
    relative_path
        .strip_suffix(".ori")
        .unwrap_or(relative_path)
        .to_string()
}

/// Build the `scip` protobuf `Index` from the collected definitions.
fn build_index(relative_path: &str, project_root: String, defs: Vec<ScipDef>) -> Index {
    let symbols: Vec<SymbolInformation> = defs
        .into_iter()
        .map(|d| SymbolInformation {
            symbol: d.symbol,
            display_name: d.display_name,
            kind: d.kind.into(),
            ..Default::default()
        })
        .collect();

    Index {
        metadata: MessageField::some(Metadata {
            project_root,
            tool_info: MessageField::some(ToolInfo {
                name: "oric".to_string(),
                version: oric::version::report_version(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        documents: vec![Document {
            relative_path: relative_path.to_string(),
            symbols,
            ..Default::default()
        }],
        ..Default::default()
    }
}

/// Emit a SCIP definition index for `path` into `output` (default `index.scip`).
///
/// Runs the frontend, walks the parsed module for every declared definition
/// entity, and writes a `scip` protobuf `Index`. Exits non-zero on frontend
/// errors (mirrors `check`) or on output write failure.
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

    // Why: a failed current_dir() must abort — silently recording an empty
    // project_root would emit a structurally-wrong index and a machine-specific
    // module identity.
    let project_root = match std::env::current_dir() {
        Ok(dir) => dir.display().to_string(),
        Err(e) => {
            eprintln!("error: cannot determine project root for SCIP index: {e}");
            std::process::exit(1);
        }
    };

    let relative_path = project_relative_path(path, &project_root);
    let module_id = module_identity(&relative_path);

    let defs = collect_defs(&module_id, &frontend.parse_result.module, db.interner());
    let symbol_count = defs.len();

    let index = build_index(&relative_path, project_root, defs);

    let bytes = match index.write_to_bytes() {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("error: failed to serialize SCIP index: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = std::fs::write(output, bytes) {
        eprintln!("error: failed to write '{output}': {e}");
        std::process::exit(1);
    }

    println!("Wrote {output} ({symbol_count} symbols) for {path}");
}
