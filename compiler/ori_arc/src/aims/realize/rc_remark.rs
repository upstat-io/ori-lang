//! RC-survivor remark record + emission (observability-only).
//!
//! This module observes the current compiled counter projection after logical
//! AIMS realization. Every surviving physical RC operation names the exact
//! logical owner-credit obligation that its selected plan could not erase; the
//! operation is not itself an AIMS fact. This defines the structured
//! `AimsRcRemark` record (modeled on
//! LLVM `-fsave-optimization-record`) and emits one per survivor to the path
//! named by `ORI_RC_REMARKS`. Reads the converged verdict only; mutates no
//! instruction (Spec: Annex E §AIMS — observability changes neither the logical
//! ownership plan nor physical RC-emission semantics).
//!
//! The `cause.lattice_dim` taxonomy is pinned to the 7 AIMS lattice dimensions
//! (`aims::lattice::dimensions`); `rc_op` is pinned to the `ArcInstr` burden-op
//! variant grain. The stream envelope + `RC_SCHEMA_VERSION` + the serialization
//! path (serde vs hand-rolled) are later sibling sections; this module hand-
//! rolls one JSON object per remark.

mod json;
mod survivor_walk;
#[cfg(test)]
mod tests;

pub(crate) use survivor_walk::emit_survivor_remarks_all_kept;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::LazyLock;

use crate::aims::lattice::dimensions::{Cardinality, Consumption, Locality};
use crate::ir::{ArcInstr, ArcVarId};

/// Destination path for the RC-remark JSONL stream, read once from
/// `ORI_RC_REMARKS`. `None` (unset or empty) disables emission, so default
/// builds are byte-identical.
static RC_REMARKS_PATH: LazyLock<Option<String>> = LazyLock::new(|| {
    std::env::var("ORI_RC_REMARKS")
        .ok()
        .filter(|p| !p.is_empty())
});

/// Whether RC-remark emission is enabled (`ORI_RC_REMARKS` set). Cheap check so
/// callers skip survivor-walk overhead on the default (disabled) path.
pub(crate) fn rc_remarks_enabled() -> bool {
    RC_REMARKS_PATH.is_some()
}

/// Version of the RC-remark record + envelope schema. Bumped when a field is
/// added/removed/retyped so consumers can detect producer drift.
///
/// Version 2 drops the path label the header carried through version 1. The
/// class ledger is the sole logical ownership-event placement authority, so no
/// path distinction exists for the header to record.
pub(crate) const RC_SCHEMA_VERSION: u32 = 2;

/// The once-per-stream header for a JSONL remark stream: schema version,
/// producing compiler SHA, and source file. Emitted as the first JSONL line;
/// the remark objects follow, one per line. Constructed by the cli-producer
/// stream driver (`write_rc_remarks_header`).
#[derive(Clone, Debug)]
pub(crate) struct RcRemarkStreamEnvelope {
    /// Schema version (`RC_SCHEMA_VERSION` at production time).
    pub schema_version: u32,
    /// Mangled SHA of the producing compiler build.
    pub compiler_sha: String,
    /// The source file the stream describes.
    pub source_file: String,
}

impl RcRemarkStreamEnvelope {
    /// Build a header for the current schema version.
    fn new(compiler_sha: String, source_file: String) -> Self {
        Self {
            schema_version: RC_SCHEMA_VERSION,
            compiler_sha,
            source_file,
        }
    }
}

/// Write the stream-header envelope as the first line of the RC-remark stream,
/// TRUNCATING any prior content. The cli-producer stream driver (`oric
/// --emit-rc-remarks`) calls this once before codegen. No-op when
/// `ORI_RC_REMARKS` is unset. Remarks are appended after by the emitter.
pub fn write_rc_remarks_header(source_file: &str, compiler_sha: &str) {
    let Some(path) = RC_REMARKS_PATH.as_ref() else {
        return;
    };
    let envelope = RcRemarkStreamEnvelope::new(compiler_sha.to_string(), source_file.to_string());
    match OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
    {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "{}", envelope.to_jsonl()) {
                tracing::warn!(
                    target: "ori_arc::aims::realize",
                    %error,
                    "rc-remark header write failed"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                target: "ori_arc::aims::realize",
                %error,
                "rc-remark header file open failed"
            );
        }
    }
}

/// LLVM opt-record remark kind. A surviving (non-eliminated) RC op is `Missed`.
#[expect(
    dead_code,
    reason = "schema contract: Passed/Analysis constructed by the emission-seam sibling section"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RemarkKind {
    /// The optimization fired (the RC op was eliminated).
    Passed,
    /// The optimization did NOT fire (the RC op survived to codegen).
    Missed,
    /// Informational analysis remark (neither a pass nor a miss).
    Analysis,
}

impl RemarkKind {
    /// Lowercase wire spelling for the JSONL `kind` field.
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Missed => "missed",
            Self::Analysis => "analysis",
        }
    }
}

/// The surviving RC op, pinned to the `ArcInstr` burden-op variant grain (the
/// thing that actually survives at the class-ledger Step-4b seam). `RcOpKind`
/// (`Alloc/Inc/Dec/Free/Cow`) is the lowered projection, handled downstream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RcRemarkOp {
    /// `ArcInstr::BurdenInc`.
    Inc,
    /// `ArcInstr::BurdenDec`.
    Dec,
    /// `ArcInstr::BurdenDecPartial`.
    DecPartial,
    /// `ArcInstr::BurdenDecField`.
    DecField,
    /// `ArcInstr::BurdenDecVariant`.
    DecVariant,
}

impl RcRemarkOp {
    /// Map an `ArcInstr` to its remark op, or `None` for a non-burden-op
    /// instruction. EXHAUSTIVE over `ArcInstr` (no catch-all): adding an
    /// `ArcInstr` variant is a compile error here, keeping the structured-
    /// emission variant coverage in sync with `fmt_instr` (`ir/format/instr.rs`)
    /// — the SSOT this mirrors.
    fn from_arc_instr(instr: &ArcInstr) -> Option<Self> {
        match instr {
            ArcInstr::BurdenInc { .. } => Some(Self::Inc),
            ArcInstr::BurdenDec { .. } => Some(Self::Dec),
            ArcInstr::BurdenDecPartial { .. } => Some(Self::DecPartial),
            ArcInstr::BurdenDecField { .. } => Some(Self::DecField),
            ArcInstr::BurdenDecVariant { .. } => Some(Self::DecVariant),
            ArcInstr::Let { .. }
            | ArcInstr::Apply { .. }
            | ArcInstr::ApplyIndirect { .. }
            | ArcInstr::PartialApply { .. }
            | ArcInstr::Project { .. }
            | ArcInstr::Construct { .. }
            | ArcInstr::RcInc { .. }
            | ArcInstr::RcDec { .. }
            | ArcInstr::RcDecPartial { .. }
            | ArcInstr::RcDecField { .. }
            | ArcInstr::RcDecVariant { .. }
            | ArcInstr::IsShared { .. }
            | ArcInstr::Set { .. }
            | ArcInstr::SetTag { .. }
            | ArcInstr::Reset { .. }
            | ArcInstr::Reuse { .. }
            | ArcInstr::CollectionReuse { .. }
            | ArcInstr::Select { .. } => None,
        }
    }

    /// Lowercase wire spelling for the JSONL `rc_op` field.
    fn as_str(self) -> &'static str {
        match self {
            Self::Inc => "burden_inc",
            Self::Dec => "burden_dec",
            Self::DecPartial => "burden_dec_partial",
            Self::DecField => "burden_dec_field",
            Self::DecVariant => "burden_dec_variant",
        }
    }
}

/// The AIMS lattice dimension whose proof failure blocked elimination. Pinned
/// to the 7 dimensions in `aims::lattice::dimensions` (one variant per
/// dimension identity — NOT the per-dimension value set).
#[expect(
    dead_code,
    reason = "schema contract: Access/Uniqueness/Shape/Effect constructed by the emission-seam sibling section"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LatticeDim {
    /// `AccessClass`.
    Access,
    /// `Consumption`.
    Consumption,
    /// `Cardinality`.
    Cardinality,
    /// `Uniqueness`.
    Uniqueness,
    /// `Locality`.
    Locality,
    /// `ShapeClass`.
    Shape,
    /// `EffectClass`.
    Effect,
}

impl LatticeDim {
    /// Lowercase wire spelling matching the graph dimension name.
    fn as_str(self) -> &'static str {
        match self {
            Self::Access => "access",
            Self::Consumption => "consumption",
            Self::Cardinality => "cardinality",
            Self::Uniqueness => "uniqueness",
            Self::Locality => "locality",
            Self::Shape => "shape",
            Self::Effect => "effect",
        }
    }
}

/// Source location of the surviving op (`debug_loc` field). Null when the op is
/// synthetic / span-less; span attribution is a later sibling section.
#[derive(Clone, Debug)]
pub(crate) struct DebugLoc {
    /// Source file path.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub column: u32,
}

/// Why the RC op survived: the proof-failure label, the blocking lattice
/// dimension, and a free-form detail string.
#[derive(Clone, Debug)]
pub(crate) struct Cause {
    /// The proof-failure label (e.g. the realization rule that did not fire).
    pub proof_failure: String,
    /// The blocking lattice dimension.
    pub lattice_dim: LatticeDim,
    /// Free-form detail (e.g. the converged dimension values).
    pub detail: String,
}

/// One structured RC-survivor remark. Field set mirrors the LLVM opt-record
/// model plus the Ori-specific `cause` lattice-dimension provenance. Nullable
/// fields (`function`, `debug_loc`, `exit_block`, `burden_net`, `cow_mode`) are
/// populated by later sibling sections (emission-seam, span-attribution).
#[derive(Clone, Debug)]
pub(crate) struct AimsRcRemark {
    /// `passed` / `missed` / `analysis`.
    pub kind: RemarkKind,
    /// The pass that produced the remark.
    pub pass: &'static str,
    /// The remark rule name within the pass.
    pub name: &'static str,
    /// The surviving RC op.
    pub rc_op: RcRemarkOp,
    /// Mangled function name, or `None` until the seam threads it.
    pub function: Option<String>,
    /// Source location, or `None` for span-less ops.
    pub debug_loc: Option<DebugLoc>,
    /// `ArcVarId` index of the variable carrying the surviving op.
    pub ssa_value: u32,
    /// Exit block index for a scope-exit dec, or `None`.
    pub exit_block: Option<u32>,
    /// Why the op survived.
    pub cause: Cause,
    /// Net burden delta for the var, or `None`.
    pub burden_net: Option<i64>,
    /// Additional structured arguments.
    pub args: Vec<String>,
    /// COW mode at the site, or `None`.
    pub cow_mode: Option<String>,
}

/// Derive the lattice dimension whose proof failure blocked DP-3 inc elision at
/// realization. A surviving inc fails one of: cardinality not `Once` (DP-3),
/// consumption not move/borrow (DP-3), or the realization escape-safety gate
/// (`Affine` + non-`BlockLocal` locality) — the else arm.
fn blocking_dim(consumption: Consumption, cardinality: Cardinality) -> LatticeDim {
    if cardinality != Cardinality::Once {
        LatticeDim::Cardinality
    } else if consumption != Consumption::Linear && consumption != Consumption::Affine {
        LatticeDim::Consumption
    } else {
        // Once + (Linear|Affine) but inc still survived: the realization
        // escape-safety gate blocked it (non-BlockLocal Affine value).
        LatticeDim::Locality
    }
}

/// One surviving-RC-op site: the variable + op carrying the survivor, its
/// converged lattice state (consumption / locality / cardinality — which
/// explains why elimination did not fire), and the def-site / function
/// attribution threaded into the remark. The fields co-vary at the single
/// `emit_rc_survivor_remark` call site (all read from one site walk).
pub(crate) struct RcSurvivorSite<'a> {
    /// The variable carrying the surviving op.
    pub var: ArcVarId,
    /// Converged consumption at the def site.
    pub consumption: Consumption,
    /// Converged locality at the def site.
    pub locality: Locality,
    /// Converged cardinality at the def site.
    pub cardinality: Cardinality,
    /// The var's def-site kind (`burden_balance` taxonomy).
    pub def_kind: &'a str,
    /// The var's machine repr (`burden_balance` taxonomy).
    pub var_repr: &'a str,
    /// The surviving op.
    pub instr: &'a ArcInstr,
    /// Raw byte-offset span, or `None` for synthetic / span-less ops.
    pub span: Option<(u32, u32)>,
    /// Enclosing mangled function name.
    pub function_name: &'a str,
}

/// Emit one `missed` remark for a surviving `BurdenInc` when `ORI_RC_REMARKS`
/// is set; a no-op otherwise. Append-only, one JSON line per call. The record
/// permits null seam and span-attribution fields.
pub(crate) fn emit_rc_survivor_remark(site: &RcSurvivorSite) {
    let Some(path) = RC_REMARKS_PATH.as_ref() else {
        return;
    };
    let remark = AimsRcRemark {
        kind: RemarkKind::Missed,
        pass: "aims-burden-elim",
        name: "rc-inc-not-elided",
        // rc_op via the format_function-synced mapping; the survivor walk only
        // serves BurdenInc today, so the fallback is unreachable in practice.
        rc_op: RcRemarkOp::from_arc_instr(site.instr).unwrap_or(RcRemarkOp::Inc),
        // Enclosing-function attribution (the function-level fallback when the
        // op span is None — the synthetic-RC-op common case).
        function: Some(site.function_name.to_string()),
        debug_loc: None,
        ssa_value: u32::try_from(site.var.index()).unwrap_or(u32::MAX),
        exit_block: None,
        cause: Cause {
            proof_failure: "burden_inc_kept_by_whole_var_disposition".to_string(),
            lattice_dim: blocking_dim(site.consumption, site.cardinality),
            detail: format!(
                "consumption={:?} locality={:?} cardinality={:?}",
                site.consumption, site.locality, site.cardinality
            ),
        },
        burden_net: None,
        // Reuse burden-balance taxonomy and carry only the raw byte span;
        // oric resolves source locations because ori_arc has no source map.
        args: vec![
            format!("def_kind={}", site.def_kind),
            format!("var_repr={}", site.var_repr),
            match site.span {
                Some((start, end)) => format!("span={start}:{end}"),
                None => "span=none".to_string(),
            },
        ],
        cow_mode: None,
    };
    match OpenOptions::new().create(true).append(true).open(path) {
        Ok(mut file) => {
            if let Err(error) = writeln!(file, "{}", remark.to_jsonl()) {
                tracing::warn!(
                    target: "ori_arc::aims::realize",
                    %error,
                    "rc-remark write failed"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                target: "ori_arc::aims::realize",
                %error,
                "rc-remark file open failed"
            );
        }
    }
}
