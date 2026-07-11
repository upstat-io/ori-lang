//! Compiler diagnostic-dump and IR-verification debug flags.
//!
//! Phase-dump toggles (AST/typed-IR/ARC-IR/LLVM-IR to stderr), the repr-opt
//! disable toggle, Alive2 IR-capture toggles, and post-pipeline IR
//! verification/audit toggles. See the crate-level `debug_flags` module doc
//! for the `dbg_set!`/`dbg_do!` macro pattern and usage.

flags! {
    // Phase Dumps

    /// Dump the parsed AST to stderr after parsing.
    ///
    /// Shows the raw AST structure before type checking.
    /// Usage: `ORI_DUMP_AFTER_PARSE=1 ori check file.ori`
    ORI_DUMP_AFTER_PARSE

    /// Dump the typed IR to stderr after type checking.
    ///
    /// Shows type annotations on every node and resolved method dispatch.
    /// Usage: `ORI_DUMP_AFTER_TYPECK=1 ori check file.ori`
    ORI_DUMP_AFTER_TYPECK

    /// Annotate every type in the `ORI_DUMP_AFTER_TYPECK` dump with its resolved
    /// pool `Idx` (and each composite's body + field/payload idxs), exposing
    /// nested-generic instantiation duplication + un-substituted generic fields.
    ///
    /// Usage: `ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_TYPE_IDX=1 ori check file.ori`
    ORI_DUMP_TYPE_IDX

    /// Emit a read-only provenance DAG (STRUCTURE + RESOLUTION + MONO edges,
    /// generic-leaf DIVERGENCE verdicts, drop-glue CONSUMER attribution) to
    /// stderr for one type-pool `Idx`, after type checking. The value is the
    /// target `Idx` as a raw `u32` (discover it with
    /// `ORI_DUMP_AFTER_TYPECK=1 ORI_DUMP_TYPE_IDX=1`). The walk never mutates the
    /// pool or compilation; it is a diagnostic view over the stable session
    /// type-pool `Idx`. An unset or empty value emits nothing; a non-numeric or
    /// out-of-range value names the cause and emits no trace.
    ///
    /// Usage: `ORI_TRACE_IDX=102 ori check file.ori`
    ///
    /// Equivalent CLI surface: `ori explain idx <index> <file.ori>` prints the
    /// same DAG to stdout (`ori explain idx --help` documents it). Both surfaces
    /// funnel through the one tracer entry — no duplicate DAG-walk / render logic.
    ORI_TRACE_IDX

    /// Dump the ARC IR to stderr after ARC lowering.
    ///
    /// Shows RC strategy decisions, drop placement, and COW operations.
    /// Usage: `ORI_DUMP_AFTER_ARC=1 ori build file.ori`
    ORI_DUMP_AFTER_ARC

    /// Dump annotated LLVM IR to stderr after LLVM codegen.
    ///
    /// Enhanced version of `ORI_DEBUG_LLVM` with Ori-aware annotations.
    /// Usage: `ORI_DUMP_AFTER_LLVM=1 ori build file.ori`
    ORI_DUMP_AFTER_LLVM

    /// Emit `GraphViz` DOT output of ARC IR control-flow graphs to stderr.
    ///
    /// Each function becomes a digraph with basic blocks as table nodes and
    /// RC operations color-highlighted. Pipe to file and render with `dot`.
    /// Usage: `ORI_EMIT_ARC_DOT=1 ori build file.ori 2> arc.dot`
    ORI_EMIT_ARC_DOT

    // Repr-Opt Configuration
    // Note: Consumed directly in `ori_repr` (which can't depend on `oric`).
    // Defined here for documentation and `check-debug-flags.sh` consistency.

    /// Disable all representation optimizations (integer narrowing, enum packing).
    ///
    /// CLI alternative: `--no-repr-opt`
    /// Usage: `ORI_NO_REPR_OPT=1 ori build file.ori`
    ORI_NO_REPR_OPT

    // Alive2 IR Capture

    /// Dump raw LLVM IR to a `.preopt.ll` file after verification, before optimization.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Distinct from `ORI_DUMP_AFTER_LLVM` which dumps annotated IR to stderr
    /// for human debugging (and before verification).
    /// Usage: `ORI_DUMP_PREOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_PREOPT_LLVM

    /// Dump raw LLVM IR to a `.postopt.ll` file after optimization, before emission.
    ///
    /// Produces machine-readable IR suitable for alive-tv input.
    /// Usage: `ORI_DUMP_POSTOPT_LLVM=1 ori build file.ori`
    ORI_DUMP_POSTOPT_LLVM

    /// Enable Alive2 IR capture: dumps both pre-opt and post-opt IR.
    ///
    /// Convenience flag that enables both `ORI_DUMP_PREOPT_LLVM` and
    /// `ORI_DUMP_POSTOPT_LLVM` and places output in `build/alive2-results/`.
    /// Usage: `ORI_ALIVE2_CAPTURE=1 ori build file.ori`
    ORI_ALIVE2_CAPTURE

    // Verification

    /// Enable ARC IR verification after the AIMS pipeline.
    ///
    /// Adds extra correctness checks (RC balance, drop placement).
    /// Usage: `ORI_VERIFY_ARC=1 ori build file.ori`
    ORI_VERIFY_ARC

    /// Enable LLVM IR verification after every optimization pass.
    ///
    /// Catches which optimization pass breaks IR well-formedness.
    /// Significant performance impact (~30-60% slower LLVM tests).
    /// Usage: `ORI_VERIFY_EACH=1 ori build file.ori`
    ORI_VERIFY_EACH

    /// Run in-pipeline RC audit on emitted LLVM IR.
    ///
    /// Detects leaks, double-frees, COW sequencing bugs, and ABI violations.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ori build file.ori`
    ORI_AUDIT_CODEGEN

    /// Enable strict (pessimistic) mode for the codegen audit.
    ///
    /// Treats COW functions as always-freeing (potential double-free becomes
    /// definite error). Also tracks function pointer parameters as RC-managed.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1 ori build file.ori`
    ORI_AUDIT_STRICT

    /// Filter codegen audit to a single function by name.
    ///
    /// Only analyzes the function whose LLVM name contains the given string.
    /// Usage: `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_FUNCTION=main ori build file.ori`
    ORI_AUDIT_FUNCTION

    /// Run LLVM lint pass (`function(lint)`) to detect likely-undefined behavior.
    ///
    /// Detects division by potential zero, suspicious alignment, unreachable
    /// patterns, and UB in instruction operands. Auto-enabled by `ORI_AUDIT_CODEGEN=1`.
    /// Usage: `ORI_LLVM_LINT=1 ori build file.ori`
    ORI_LLVM_LINT

}
