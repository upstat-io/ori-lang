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

mod occurrence;
pub(super) mod symbol;

use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_ir::{ImplMethod, Module, Span, StringInterner, TraitItem, TypeDeclKind, Variant};
use oric::{CompilerDb, Db, SourceFile};
use protobuf::{Message, MessageField};
use scip::types::symbol_information::Kind;
use scip::types::{Document, Index, Metadata, Occurrence, SymbolInformation, ToolInfo};
use std::path::{Path, PathBuf};

use super::read_file;
use super::report_frontend_errors;
use occurrence::{collect_definition_occurrences, collect_occurrences, ScipOccurrence};
use symbol::{ModuleMinter, ScipDef};

/// Collect every per-SITE definition [`ScipDef`] in `module`, in source order,
/// WITHOUT dedup. Walks functions, types + fields/variants, traits + methods,
/// inherent + trait-impl blocks, default impls, and extensions, minting every
/// symbol under `module_id` so cross-file homonyms stay distinct. The pre-dedup
/// list preserves distinct SITES that mint the same symbol string (inherent +
/// trait-impl methods → one `Type#m().`), each with its own source span — the
/// per-site Definition occurrences build from this list, never the deduped one.
fn collect_raw_defs(module_id: &str, module: &Module, interner: &StringInterner) -> Vec<ScipDef> {
    let minter = ModuleMinter::new(module_id);
    let mut defs = Vec::new();

    for func in &module.functions {
        defs.push(minter.function(interner.lookup(func.name), func.span));
    }

    for ty in &module.types {
        let type_name = interner.lookup(ty.name);
        match &ty.kind {
            TypeDeclKind::Struct(fields) => {
                defs.push(minter.type_def(type_name, Kind::Struct, ty.span));
                for f in fields {
                    defs.push(minter.field(type_name, interner.lookup(f.name), f.span));
                }
            }
            TypeDeclKind::Sum(variants) => {
                push_sum_type_defs(&mut defs, &minter, interner, type_name, ty.span, variants);
            }
            // A newtype is a distinct nominal type with no fields to index.
            TypeDeclKind::Newtype(_) => {
                defs.push(minter.type_def(type_name, Kind::Type, ty.span));
            }
        }
    }

    for tr in &module.traits {
        let trait_name = interner.lookup(tr.name);
        defs.push(minter.trait_def(trait_name, tr.span));
        for item in &tr.items {
            let method = match item {
                TraitItem::MethodSig(m) => Some((m.name, m.span)),
                TraitItem::DefaultMethod(m) => Some((m.name, m.span)),
                TraitItem::AssocType(_) => None,
            };
            if let Some((method_name, method_span)) = method {
                defs.push(minter.method(trait_name, interner.lookup(method_name), method_span));
            }
        }
    }

    for imp in &module.impls {
        if let Some(type_name_id) = imp.type_name() {
            let parent = interner.lookup(type_name_id);
            push_methods(&mut defs, &minter, interner, parent, &imp.methods);
        }
    }

    // A `pub def impl Trait { @m }` declares default-impl methods keyed on the
    // trait; an `extend Type { @m }` declares extension methods keyed on the
    // target type. Both are real definitions downstream references resolve to.
    for di in &module.def_impls {
        let parent = interner.lookup(di.trait_name);
        push_methods(&mut defs, &minter, interner, parent, &di.methods);
    }

    for ext in &module.extends {
        let parent = interner.lookup(ext.target_type_name);
        push_methods(&mut defs, &minter, interner, parent, &ext.methods);
    }

    defs
}

/// The symbol-unique [`ScipDef`] set emitted as `SymbolInformation` — the raw
/// per-site list (stable-sorted) with same-symbol duplicates collapsed.
///
/// Why dedup: inherent + trait-impl methods can mint the same `Type#m().`, and
/// one logical symbol gets exactly one `SymbolInformation`. Distinct SITES are
/// preserved separately via [`collect_raw_defs`] for per-site Definition
/// occurrences — the dedup here only governs the symbol table.
fn collect_defs(module_id: &str, module: &Module, interner: &StringInterner) -> Vec<ScipDef> {
    let mut defs = collect_raw_defs(module_id, module, interner);
    defs.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    defs.dedup_by(|a, b| a.symbol == b.symbol);
    defs
}

/// Push one method [`ScipDef`] per entry in `methods`, keyed on `parent` (the
/// receiver type name for inherent / extension methods, the trait name for
/// default-impl methods). Each method's source `span` rides on its `ScipDef`.
fn push_methods(
    defs: &mut Vec<ScipDef>,
    minter: &ModuleMinter,
    interner: &StringInterner,
    parent: &str,
    methods: &[ImplMethod],
) {
    for m in methods {
        defs.push(minter.method(parent, interner.lookup(m.name), m.span));
    }
}

/// Push the `Kind::Enum` type def plus one [`ScipDef`] per variant and per
/// variant field for a sum-type declaration named `type_name`. `type_span` is
/// the sum-type declaration's full extent (the enum def occurrence's enclosing
/// range); each variant / variant field carries its own source span.
fn push_sum_type_defs(
    defs: &mut Vec<ScipDef>,
    minter: &ModuleMinter,
    interner: &StringInterner,
    type_name: &str,
    type_span: Span,
    variants: &[Variant],
) {
    defs.push(minter.type_def(type_name, Kind::Enum, type_span));
    for v in variants {
        let variant_name = interner.lookup(v.name);
        defs.push(minter.variant(type_name, variant_name, v.span));
        for vf in &v.fields {
            defs.push(minter.variant_field(
                type_name,
                variant_name,
                interner.lookup(vf.name),
                vf.span,
            ));
        }
    }
}

/// Derive the project-relative source path used as the SCIP document path and
/// the basis of the module identity.
///
/// An absolute path under `project_root` is stripped to its relative tail; an
/// absolute path outside the root falls back to its file name. The result is
/// forward-slash normalized so the identity is stable across platforms and
/// never embeds an absolute, machine-specific directory.
pub(super) fn project_relative_path(path: &str, project_root: &str) -> String {
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
pub(super) fn module_identity(relative_path: &str) -> String {
    relative_path
        .strip_suffix(".ori")
        .unwrap_or(relative_path)
        .to_string()
}

/// Build the `scip` protobuf `Index` from the collected definitions and
/// reference occurrences.
fn build_index(
    relative_path: &str,
    project_root: String,
    defs: Vec<ScipDef>,
    occurrences: Vec<ScipOccurrence>,
) -> Index {
    let symbols: Vec<SymbolInformation> = defs
        .into_iter()
        .map(|d| SymbolInformation {
            symbol: d.symbol,
            display_name: d.display_name,
            kind: d.kind.into(),
            ..Default::default()
        })
        .collect();

    let occurrences: Vec<Occurrence> = occurrences
        .into_iter()
        .map(|o| Occurrence {
            range: o.range,
            enclosing_range: o.enclosing_range,
            symbol: o.symbol,
            // Per-occurrence role: Definition occurrences carry DEFINITION_ROLE,
            // references keep REFERENCE_ROLE (read from the ScipOccurrence, not
            // hardcoded).
            symbol_roles: o.symbol_roles,
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
            occurrences,
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

    let source = file.text(&db);

    // Definition occurrences come from the PRE-DEDUP raw defs so distinct sites
    // minting one symbol each emit their own Definition occurrence; the symbol
    // table above uses the deduped `defs`.
    let raw_defs = collect_raw_defs(&module_id, &frontend.parse_result.module, db.interner());
    let mut occurrences = collect_definition_occurrences(&raw_defs, source.as_str());
    occurrences.extend(collect_occurrences(
        &module_id,
        &frontend.parse_result.arena,
        &frontend.parse_result.module,
        &frontend.type_result.typed,
        &frontend.pool,
        db.interner(),
        source.as_str(),
    ));
    let occurrence_count = occurrences.len();

    let index = build_index(&relative_path, project_root, defs, occurrences);

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

    println!("Wrote {output} ({symbol_count} symbols, {occurrence_count} occurrences) for {path}");
}
