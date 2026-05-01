//! Synergy metrics for cross-dimensional AIMS reasoning.
//!
//! Quantifies how much value the product lattice provides over any single
//! dimension alone. Accumulated during realization (Phases 1+2) and reported
//! via `tracing::info!` and [`RealizationResult::synergy_metrics`].
//!
//! References: Section 11.2 of the AIMS plan.

/// Metrics tracking cross-dimensional decision contributions.
///
/// Accumulated during [`realize_rc_reuse`](super::realize_rc_reuse) (Phase 1)
/// and [`realize_annotations`](super::realize_annotations) (Phase 2).
/// The `canonicalize_cross_fires` field is accumulated separately during
/// backward analysis (intraprocedural) and merged before reporting.
///
/// Cross-dimensional evidence is measured by `canonicalize_cross_fires`
/// (analysis-stage rule firings that required 2+ lattice dimensions). The
/// prior realization-stage counters `cow_upgrades` and `cross_dim_reuse`
/// were removed together with the unsound cross-dimensional promotions
/// they tracked (RL-13).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SynergyMetrics {
    /// Reuse decisions made during realization (any non-None reuse).
    pub reuse_decisions: usize,

    /// Total RC decisions made (Inc + Dec + Skip + Defer, excluding None).
    pub total_rc_decisions: usize,

    /// Total COW decisions made (`StaticUnique` + `Dynamic` + `StaticShared`).
    pub total_cow_decisions: usize,

    /// FIP certifications achieved via `extract_contract()` reading converged
    /// effect+locality state (contract layer, not realization).
    ///
    /// Set by interprocedural analysis, not by the realization walk.
    pub natural_fip: usize,

    /// Cross-dimension canonicalize rule firings (canonicalization rules
    /// that reason across 2+ lattice dimensions). Accumulated during
    /// backward analysis convergence. Each firing represents a state
    /// change that required reasoning across 2+ lattice dimensions.
    pub canonicalize_cross_fires: usize,
}

impl SynergyMetrics {
    /// Percentage of RC decisions that resulted in reuse.
    ///
    /// Returns 0.0 if no RC decisions were made.
    #[must_use]
    pub fn reuse_percent(&self) -> f64 {
        if self.total_rc_decisions == 0 {
            0.0
        } else {
            #[expect(
                clippy::cast_precision_loss,
                reason = "metrics display — precision loss acceptable"
            )]
            let pct = (self.reuse_decisions as f64 / self.total_rc_decisions as f64) * 100.0;
            pct
        }
    }

    /// Total cross-dimensional evidence count.
    ///
    /// Counts `canonicalize_cross_fires` — analysis-stage canonicalization
    /// rule firings that reasoned across 2+ lattice dimensions. Prior
    /// realization-stage cross-dimensional counters (`cow_upgrades`,
    /// `cross_dim_reuse`) were removed together with the unsound paths
    /// they tracked.
    #[must_use]
    pub fn cross_dim_evidence_total(&self) -> usize {
        self.canonicalize_cross_fires
    }

    /// Whether any cross-dimensional reasoning occurred.
    #[must_use]
    pub fn has_cross_dim_evidence(&self) -> bool {
        self.cross_dim_evidence_total() > 0
    }

    /// Merge another metrics instance into this one (additive).
    pub fn merge(&mut self, other: &SynergyMetrics) {
        self.reuse_decisions += other.reuse_decisions;
        self.total_rc_decisions += other.total_rc_decisions;
        self.total_cow_decisions += other.total_cow_decisions;
        self.natural_fip += other.natural_fip;
        self.canonicalize_cross_fires += other.canonicalize_cross_fires;
    }

    /// Log metrics summary via tracing.
    ///
    /// `function_id` is the interned [`Name`] raw value for the function.
    pub fn report(&self, function_id: u32) {
        if self.total_rc_decisions == 0 && self.total_cow_decisions == 0 {
            return;
        }

        tracing::info!(
            function = function_id,
            total_rc = self.total_rc_decisions,
            reuse = self.reuse_decisions,
            reuse_pct = format_args!("{:.1}%", self.reuse_percent()),
            cross_dim_evidence = self.cross_dim_evidence_total(),
            total_cow = self.total_cow_decisions,
            natural_fip = self.natural_fip,
            canonicalize_cross_fires = self.canonicalize_cross_fires,
            "AIMS synergy metrics"
        );
    }
}
