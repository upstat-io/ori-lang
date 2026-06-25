//! RC-survivor remark record + emission (observability-only).
//!
//! Every RC op surviving the Phase-6 burden-elimination lattice is a specific
//! proof failure. This defines the structured `AimsRcRemark` record (modeled on
//! LLVM `-fsave-optimization-record`) and emits one per survivor to the path
//! named by `ORI_RC_REMARKS`. Reads the converged verdict only; mutates no
//! instruction (Spec: Annex E §AIMS — observability changes no RC-emission
//! semantics).
//!
//! The `cause.lattice_dim` taxonomy is pinned to the 7 AIMS lattice dimensions
//! (`aims::lattice::dimensions`); `rc_op` is pinned to the `ArcInstr` burden-op
//! variant grain. The stream envelope + `RC_SCHEMA_VERSION` + the serialization
//! path (serde vs hand-rolled) are later sibling sections; this module hand-
//! rolls one JSON object per remark.

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

/// Whether compilation is on the burden-sole RC path
/// (`ORI_DISABLE_PREDICATE_STACK_RC=1`) — the ONLY surface where a burden
/// remark is a valid RC verdict; a default-path run still co-emits the legacy
/// predicate-stack RC and is false-green on floor cells. The envelope's
/// `burden_path` is derived from this so `burden_path: true` is truthful and a
/// default-path stream is honestly `false`.
#[allow(
    dead_code,
    reason = "schema contract: consumed by the cli-producer sibling section + RcRemarkStreamEnvelope::new"
)]
pub(crate) fn on_burden_sole_path() -> bool {
    std::env::var("ORI_DISABLE_PREDICATE_STACK_RC").as_deref() == Ok("1")
}

/// Version of the RC-remark record + envelope schema. Bumped when a field is
/// added/removed/retyped so consumers can detect producer drift.
#[allow(
    dead_code,
    reason = "schema contract: consumed by the cli-producer sibling section stream driver"
)]
pub(crate) const RC_SCHEMA_VERSION: u32 = 1;

/// The once-per-stream header for a JSONL remark stream: schema version,
/// producing compiler SHA, source file, and the `burden_path` assertion (true
/// only when produced on the burden-sole path, the sole valid RC verdict
/// surface). Emitted as the first JSONL line; the remark
/// objects follow, one per line. Constructed by the cli-producer stream driver
/// (`write_rc_remarks_header`).
#[derive(Clone, Debug)]
pub(crate) struct RcRemarkStreamEnvelope {
    /// Schema version (`RC_SCHEMA_VERSION` at production time).
    pub schema_version: u32,
    /// Mangled SHA of the producing compiler build.
    pub compiler_sha: String,
    /// The source file the stream describes.
    pub source_file: String,
    /// `true` iff produced on the burden-sole path (the sole RC verdict surface).
    pub burden_path: bool,
}

impl RcRemarkStreamEnvelope {
    /// Build a header for the current schema version. `burden_path` is derived
    /// from [`on_burden_sole_path`] so `burden_path: true` is truthful — the
    /// stream is a valid RC verdict only on the burden-sole path.
    pub fn new(compiler_sha: String, source_file: String) -> Self {
        Self {
            schema_version: RC_SCHEMA_VERSION,
            compiler_sha,
            source_file,
            burden_path: on_burden_sole_path(),
        }
    }

    /// Render the header as a single JSON object line (no trailing newline).
    pub fn to_jsonl(&self) -> String {
        format!(
            "{{\"record\":\"header\",\"schema_version\":{},\"compiler_sha\":\"{}\",\
             \"source_file\":\"{}\",\"burden_path\":{}}}",
            self.schema_version,
            json_escape(&self.compiler_sha),
            json_escape(&self.source_file),
            self.burden_path,
        )
    }
}

/// Write the stream-header envelope as the first line of the RC-remark stream,
/// TRUNCATING any prior content. The cli-producer stream driver (`oric
/// --emit-rc-remarks`) calls this once before codegen, after composing the
/// burden-sole-path gating, so `burden_path` is truthful. No-op when
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
#[allow(
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
/// thing that actually survives at the Phase-6 seam). `RcOpKind`
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
#[allow(
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

impl AimsRcRemark {
    /// Render the remark as a single JSON object line (no trailing newline).
    fn to_jsonl(&self) -> String {
        let function = opt_str_field(self.function.as_deref());
        let debug_loc = match &self.debug_loc {
            Some(loc) => format!(
                "{{\"file\":\"{}\",\"line\":{},\"column\":{}}}",
                json_escape(&loc.file),
                loc.line,
                loc.column
            ),
            None => "null".to_string(),
        };
        let exit_block = opt_num_field(self.exit_block.map(i64::from));
        let burden_net = opt_num_field(self.burden_net);
        let cow_mode = opt_str_field(self.cow_mode.as_deref());
        let args = {
            let inner: Vec<String> = self
                .args
                .iter()
                .map(|a| format!("\"{}\"", json_escape(a)))
                .collect();
            format!("[{}]", inner.join(","))
        };
        format!(
            "{{\"kind\":\"{}\",\"pass\":\"{}\",\"name\":\"{}\",\"rc_op\":\"{}\",\
             \"function\":{},\"debug_loc\":{},\"ssa_value\":{},\"exit_block\":{},\
             \"cause\":{{\"proof_failure\":\"{}\",\"lattice_dim\":\"{}\",\"detail\":\"{}\"}},\
             \"burden_net\":{},\"args\":{},\"cow_mode\":{}}}",
            self.kind.as_str(),
            json_escape(self.pass),
            json_escape(self.name),
            self.rc_op.as_str(),
            function,
            debug_loc,
            self.ssa_value,
            exit_block,
            json_escape(&self.cause.proof_failure),
            self.cause.lattice_dim.as_str(),
            json_escape(&self.cause.detail),
            burden_net,
            args,
            cow_mode,
        )
    }
}

/// Minimal JSON string escaping for the hand-rolled serializer (the serde path
/// is a later sibling section). Escapes `"`, `\`, and control whitespace.
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out
}

/// Render an optional string field as a quoted JSON string or `null`.
fn opt_str_field(value: Option<&str>) -> String {
    match value {
        Some(s) => format!("\"{}\"", json_escape(s)),
        None => "null".to_string(),
    }
}

/// Render an optional numeric field as a JSON number or `null`.
fn opt_num_field(value: Option<i64>) -> String {
    match value {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
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
/// is set; a no-op otherwise. Append-only, one JSON line per call. The seam +
/// span-attribution siblings populate the currently-null fields.
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
        // Reuse the burden_balance taxonomy helpers (no parallel enum) — the
        // def-site + machine-repr attribution for the surviving var. The raw
        // byte-offset span is carried here (None for synthetic ops); oric-side
        // resolution to `debug_loc` via `LineOffsetTable` is the cli-producer's
        // job (ori_arc holds byte offsets, not a source map).
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
