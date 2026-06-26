//! The `emit-aims-state` command: project per-function AIMS contract + lattice
//! state to JSONL for intel-graph ingestion.
//!
//! Drives the full pipeline — lex -> parse -> typecheck -> canonicalize -> ARC
//! lowering -> AIMS interprocedural contract computation — then, per top-level
//! function, projects the 18-property `MemoryContract` / `ReturnContract`
//! / `EffectSummary` / `FipContract` surface plus the converged `AimsStateMap`
//! per-variable lattice values to one JSONL record. The enum serialization runs
//! through explicit snake-case match tables in [`record`], never a derive.
//!
//! This is a READ-ONLY projection of compile-time AIMS analysis results: it
//! emits no RC, mutates no realization, and does not gate on the burden path
//! (Spec: Annex E §AIMS). Each record carries the producer-emitted identity
//! `(repo, qualified_name, file, line, scip_moniker)`; a downstream
//! code-intelligence consumer owns its own symbol join key and bridges to it
//! via `file:line`.

mod record;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use ori_arc::aims::intraprocedural::{analyze_function, AimsStateMap};
use ori_arc::aims::lattice::AimsState;
use ori_arc::{
    ArcBlockId, ArcClassifier, ArcFunction, ArcVarId, BuiltinOwnershipSets, MemoryContract,
};
use ori_diagnostic::emitter::{ColorMode, DiagnosticEmitter, TerminalEmitter};
use ori_ir::canon::CanonResult;
use ori_ir::{Name, StringInterner};
use ori_types::{FunctionSig, Pool};
use oric::{CompilerDb, Db, SourceFile};
use rustc_hash::FxHashMap;
use serde_json::Value;

use super::emit_scip::symbol::ModuleMinter;
use super::emit_scip::{module_identity, project_relative_path};
use super::read_file;
use super::report_frontend_errors;
use crate::parser::ParseOutput;

/// The 1-based source line containing `byte_offset`, counting `\n` up to it.
fn line_of(source: &str, byte_offset: u32) -> u32 {
    let end = usize::try_from(byte_offset)
        .unwrap_or(usize::MAX)
        .min(source.len());
    let newlines = source.as_bytes()[..end]
        .iter()
        .fold(0usize, |acc, &b| acc + usize::from(b == b'\n'));
    u32::try_from(newlines)
        .unwrap_or(u32::MAX)
        .saturating_add(1)
}

/// Project the converged per-variable lattice values, `position`-qualified by
/// `ArcVarId` index.
///
/// Per-variable dimension sourcing:
/// - The six backward-demand dimensions (`access`, `consumption`, `cardinality`,
///   `uniqueness`, `locality`, `effect`) are the JOIN of the variable's
///   block-boundary states and its definition-point demand — the converged
///   per-variable lattice value the realization passes consume. The
///   definition-point query recovers vars defined-and-consumed within one block
///   (stripped from the block-boundary maps, demand held in `def_demand`).
/// - The `shape` dimension is sourced from [`AimsStateMap::var_shape`], the
///   per-variable shape SSOT populated by `populate_var_shapes` at Step-4
///   post-convergence. Shape is a per-definition structural property, not a
///   backward-demand dimension; the flat `ShapeClass` join collapses every
///   `ReusableCtor`/`CollectionBuffer` to `NonReusable` (`equal stays, unequal
///   -> NonReusable`), so the join is never a faithful shape source.
fn project_lattice_values(state_map: &AimsStateMap) -> Vec<Value> {
    let num_vars = state_map.num_vars();
    let num_blocks = state_map.num_blocks();
    let mut values = Vec::new();

    for v in 0..num_vars {
        let Ok(vu) = u32::try_from(v) else { continue };
        let var = ArcVarId::new(vu);
        if state_map.is_scalar(var) || state_map.is_immortal(var) {
            continue;
        }

        let mut joined = AimsState::BOTTOM;
        let mut seen = false;
        for b in 0..num_blocks {
            let Ok(bu) = u32::try_from(b) else { continue };
            let block = ArcBlockId::new(bu);
            for state in [
                state_map.var_state_at_block_entry(block, var),
                state_map.var_state_at_block_exit(block, var),
                state_map.var_state_at_definition(block, var),
            ] {
                if state != AimsState::BOTTOM && !state.is_scalar() {
                    joined = joined.join(&state);
                    seen = true;
                }
            }
        }

        if seen {
            // Overlay the per-variable shape SSOT: the flat-lattice join above
            // destroys it, so report the definition-derived classification.
            joined.shape = state_map.var_shape(var);
            values.push(record::lattice_value_json(vu, joined));
        }
    }

    values
}

/// Emit AIMS-state JSONL for `path` into `output`.
///
/// Runs the frontend; on any frontend error, exits non-zero before writing so
/// no partial JSONL is produced. Otherwise lowers every non-generic top-level
/// function to ARC IR, computes AIMS contracts, and writes one JSONL record per
/// top-level function.
pub fn emit_aims_state_file(path: &str, output: &str) {
    let records = match build_records(path) {
        Ok(records) => records,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(1);
        }
    };

    let mut out = String::new();
    for (_, value) in &records {
        match serde_json::to_string(value) {
            Ok(line) => {
                out.push_str(&line);
                out.push('\n');
            }
            Err(e) => {
                eprintln!("error: failed to serialize AIMS state record: {e}");
                std::process::exit(1);
            }
        }
    }

    if let Err(e) = std::fs::write(output, &out) {
        eprintln!("error: failed to write '{output}': {e}");
        std::process::exit(1);
    }

    let record_count = records.len();
    println!("Wrote {output} ({record_count} records) for {path}");
}

/// Drive the pipeline and build the sorted `(scip_moniker, record)` set.
///
/// Lower every non-generic top-level function (and its lambdas) to ARC IR.
/// Lambdas join the analysis set so interprocedural contracts are complete;
/// records are emitted only for the named top-level functions (lambdas carry no
/// source span / SCIP moniker and map to no `:Symbol`).
///
/// Returns `Err` when ARC lowering surfaces problems — the lowered IR is then
/// possibly malformed and the projected AIMS state would be unsound, so no
/// partial JSONL is emitted (the normal codegen path gates the same set).
fn lower_module_to_arc(
    parse_result: &ParseOutput,
    function_sigs: &[FunctionSig],
    canon: &CanonResult,
    interner: &StringInterner,
    pool: &Pool,
) -> Result<Vec<ArcFunction>, String> {
    let mut arc_problems = Vec::new();
    let mut all_funcs: Vec<ArcFunction> = Vec::new();
    for (func, sig) in parse_result
        .module
        .functions
        .iter()
        .zip(function_sigs.iter())
    {
        if sig.is_generic() {
            continue;
        }
        let (arc_fn, lambdas) = crate::arc_lowering::lower_to_arc(
            func.name,
            sig,
            func.name,
            canon,
            interner,
            pool,
            &mut arc_problems,
            None,
        );
        all_funcs.push(arc_fn);
        all_funcs.extend(lambdas);
    }

    if !arc_problems.is_empty() {
        return Err(format!(
            "error: ARC lowering reported {} problem(s); AIMS state not emitted",
            arc_problems.len()
        ));
    }
    Ok(all_funcs)
}

/// Returns `Err` with a diagnostic message on a frontend error (diagnostics are
/// already emitted to stderr by then) so the caller controls process exit; this
/// keeps the projection testable without `std::process::exit`.
fn build_records(path: &str) -> Result<Vec<(String, Value)>, String> {
    let content = read_file(path);
    let db = CompilerDb::new();
    let file = SourceFile::new(&db, PathBuf::from(path), content);

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    let mut emitter = TerminalEmitter::with_color_mode(std::io::stderr(), ColorMode::Auto, is_tty)
        .with_source(file.text(&db).as_str())
        .with_file_path(path);

    let Some(frontend) = report_frontend_errors(&db, file, &mut emitter) else {
        return Err("error: frontend pipeline failed (Pool not cached)".to_string());
    };
    if frontend.has_errors() {
        emitter.flush();
        return Err("error: frontend reported errors; AIMS state not emitted".to_string());
    }

    let interner = db.interner();
    let pool: &Pool = &frontend.pool;

    // Canonicalize: AST + types -> self-contained canonical IR.
    let shared_canon = crate::query::canonicalize_cached(
        &db,
        file,
        &frontend.parse_result,
        &frontend.type_result,
        &frontend.pool,
    );
    let canon = &*shared_canon;

    let function_sigs =
        crate::typeck::build_function_sigs(&frontend.parse_result, &frontend.type_result);

    let all_funcs = lower_module_to_arc(
        &frontend.parse_result,
        &function_sigs,
        canon,
        interner,
        pool,
    )?;

    // Source identity for the named top-level functions: the SCIP moniker (same
    // `ModuleMinter` the SCIP index uses) + source line.
    let project_root = match std::env::current_dir() {
        Ok(dir) => dir.display().to_string(),
        Err(e) => {
            eprintln!("error: cannot determine project root for AIMS state: {e}");
            std::process::exit(1);
        }
    };
    let relative_path = project_relative_path(path, &project_root);
    let minter = ModuleMinter::new(module_identity(&relative_path));
    let source = file.text(&db);
    let mut fn_meta: FxHashMap<Name, (String, u32)> = FxHashMap::default();
    for func in &frontend.parse_result.module.functions {
        let def = minter.function(interner.lookup(func.name), func.span);
        let line = line_of(source.as_str(), func.span.start);
        fn_meta.insert(func.name, (def.symbol, line));
    }

    let classifier = ArcClassifier::new(pool);
    let builtins = BuiltinOwnershipSets::new(interner);

    // Compute contracts on a clone so `all_funcs` stays in the pre-ownership
    // shape `analyze_program` extracted contracts from; re-running
    // `analyze_function` on it with the converged contracts reproduces the same
    // `AimsStateMap` (the empty context-regions / immortals match
    // `analyze_scc_single`).
    let mut funcs_for_contracts = all_funcs.clone();
    let contracts: FxHashMap<Name, MemoryContract> =
        ori_arc::compute_aims_contracts(&mut funcs_for_contracts, &classifier, interner, &builtins);

    let mut records: Vec<(String, Value)> = Vec::new();
    for func in &all_funcs {
        let Some((scip_moniker, line)) = fn_meta.get(&func.name) else {
            continue;
        };
        let Some(contract) = contracts.get(&func.name) else {
            continue;
        };
        let state_map = analyze_function(func, &classifier, &contracts, &[], Vec::new());
        let lattice_values = project_lattice_values(&state_map);
        let qualified_name = interner.lookup(func.name);
        let id = record::RecordIdentity {
            qualified_name,
            file: &relative_path,
            line: *line,
            scip_moniker,
        };
        let value = record::build_record(
            &id,
            contract,
            state_map.fip_construct_count(),
            state_map.fip_consumed_count(),
            lattice_values,
        );
        records.push((scip_moniker.clone(), value));
    }

    // Deterministic emission order: by SCIP moniker (a globally-stable key).
    records.sort_by(|a, b| a.0.cmp(&b.0));

    Ok(records)
}
