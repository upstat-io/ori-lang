//! Read-only provenance DAG tracer for one type-pool `Idx`.
//!
//! SSOT for the idx-provenance trace: both the `ORI_TRACE_IDX` env knob
//! ([`emit_provenance_trace`], driven from [`super::report_frontend_errors`])
//! and the `ori explain idx` verb ([`super::explain_idx::explain_idx`]) funnel
//! through [`trace_idx_provenance`] so the DAG-walk / render logic has one home.

/// Emit a read-only provenance DAG for one type-pool `Idx` named by
/// `ORI_TRACE_IDX`. Read-only — never mutates the pool or compilation.
///
/// An unset or empty value is the off state (silent); a non-empty value is a
/// user-supplied index whose parse failure / out-of-range condition is surfaced
/// with its cause, never silently swallowed.
pub(super) fn emit_provenance_trace(
    pool: &ori_types::Pool,
    mono_instances: &[ori_types::MonoInstance],
    interner: &crate::ir::StringInterner,
) {
    let flag = crate::debug_flags::ORI_TRACE_IDX;
    let Ok(raw) = std::env::var(flag) else {
        return;
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    match trimmed.parse::<u32>() {
        Ok(idx_raw) => match trace_idx_provenance(idx_raw, pool, mono_instances, interner) {
            IdxTrace::Rendered(rendered) => eprint!("{rendered}"),
            IdxTrace::OutOfRange { idx, pool_len } => {
                eprintln!("ori: {flag}={idx} {}", idx_out_of_range_detail(pool_len));
            }
        },
        Err(_) => {
            eprintln!(
                "ori: {flag}={trimmed:?} is not a valid type-pool index \
                 (expected a non-negative integer; discover indices with \
                 {dump}=1 {dump_idx}=1); no trace emitted.",
                dump = crate::debug_flags::ORI_DUMP_AFTER_TYPECK,
                dump_idx = crate::debug_flags::ORI_DUMP_TYPE_IDX,
            );
        }
    }
}

/// Outcome of a provenance-`Idx` trace request: the rendered DAG, or an
/// out-of-range index carrying the data needed to name the cause.
pub(crate) enum IdxTrace {
    /// The rendered provenance DAG for a valid index.
    Rendered(String),
    /// The requested index is outside the type pool's valid range.
    OutOfRange {
        /// The requested raw index.
        idx: u32,
        /// The number of entries in the type pool.
        pool_len: usize,
    },
}

/// Build the full edge-set provenance DAG for one type-pool index and render it:
/// STRUCTURE + RESOLUTION + MONO edges, generic-leaf DIVERGENCE verdicts, and
/// drop-glue CONSUMER attribution. Read-only — never mutates the pool or
/// compilation.
#[must_use]
pub(crate) fn trace_idx_provenance(
    idx_raw: u32,
    pool: &ori_types::Pool,
    mono_instances: &[ori_types::MonoInstance],
    interner: &crate::ir::StringInterner,
) -> IdxTrace {
    let root = ori_types::Idx::from_raw(idx_raw);
    if pool.is_valid_idx(root) {
        // Full edge set: STRUCTURE + RESOLUTION (collected in the walk) +
        // MONO/divergence projected off the session mono instances
        // (read-only — never re-substitutes).
        let dag = ori_types::ProvenanceDag::walk(pool, root, ori_types::PROVENANCE_MAX_DEPTH)
            .with_mono_edges(pool, mono_instances)
            .with_consumer_edges(consumer_attribution(pool, root));
        IdxTrace::Rendered(dag.render(pool, interner))
    } else {
        IdxTrace::OutOfRange {
            idx: idx_raw,
            pool_len: pool.len(),
        }
    }
}

/// Shared out-of-range diagnostic detail for an invalid provenance `Idx`: the
/// body both trace surfaces (the `ORI_TRACE_IDX` env knob via
/// [`emit_provenance_trace`] and the `ori explain idx` verb via
/// [`super::explain_idx::explain_idx`]) emit after their own `ori: <surface>` prefix.
/// One home so the valid-range wording never drifts between the two surfaces.
pub(crate) fn idx_out_of_range_detail(pool_len: usize) -> String {
    format!(
        "out of range (type pool has {pool_len} entries; \
         valid indices 0..{pool_len}); no trace emitted."
    )
}

/// Read-only CONSUMER-edge attribution for the provenance DAG: each logical
/// drop plan is attributed back to the `Idx` chain that produced it.
///
/// The drop/RC machinery lives behind the `llvm` feature; with it, the
/// attribution walks the drop-descriptor tree (`ori_arc::drop`) read-only.
#[cfg(feature = "llvm")]
fn consumer_attribution(
    pool: &ori_types::Pool,
    root: ori_types::Idx,
) -> Vec<ori_types::ConsumerEdge> {
    let classifier = ori_arc::ArcClassifier::new(pool);
    ori_arc::compute_consumer_attribution(root, &classifier, pool, ori_types::PROVENANCE_MAX_DEPTH)
}

/// Without the current ARC-enabled compiler feature, attribution is empty.
#[cfg(not(feature = "llvm"))]
fn consumer_attribution(
    _pool: &ori_types::Pool,
    _root: ori_types::Idx,
) -> Vec<ori_types::ConsumerEdge> {
    Vec::new()
}
