//! Backward-demand algebra before product-state observation.
//!
//! Same-path cardinality and consumption evidence must compose independently.
//! Product-state canonicalization is an observation boundary, not part of the
//! sequential algebra.

use super::lattice::{Cardinality, Consumption};

/// Independent cardinality and consumption evidence for one execution path.
///
/// This is deliberately not an [`AimsState`]. Raw pairs such as
/// `Absent + Unrestricted` are valid intermediate evidence but collide with
/// the scalar sentinel if installed in the product lattice before observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RawDemand {
    pub(crate) cardinality: Cardinality,
    pub(crate) consumption: Consumption,
}

impl RawDemand {
    pub(crate) const ZERO: Self = Self {
        cardinality: Cardinality::Absent,
        consumption: Consumption::Dead,
    };

    pub(crate) const LINEAR_ONCE: Self = Self {
        cardinality: Cardinality::Once,
        consumption: Consumption::Linear,
    };

    pub(crate) const fn new(cardinality: Cardinality, consumption: Consumption) -> Self {
        Self {
            cardinality,
            consumption,
        }
    }

    /// TF-14 contribution from a projected destination to its source.
    pub(crate) const fn projected(self) -> Self {
        Self::new(self.cardinality, Consumption::Affine)
    }

    /// TF-14 contribution from an L-9-excluded scalar Project destination.
    ///
    /// A live scalar result is copied out once at the Project instruction, so
    /// later scalar reuse cannot promote the managed source to `Many`. A dead
    /// scalar result retains the ordinary pending dead-projection evidence
    /// until the enclosing block walk is observed.
    pub(crate) const fn scalar_project_contribution(live: bool) -> Self {
        if live {
            Self::new(Cardinality::Once, Consumption::Affine)
        } else {
            Self::ZERO.projected()
        }
    }

    /// Compose two contributions on the same execution path.
    #[must_use]
    pub(crate) fn seq_add(self, other: Self) -> Self {
        Self {
            cardinality: self.cardinality.seq_add(other.cardinality),
            consumption: self.consumption.seq_add(other.consumption),
        }
    }

    /// Join already-separated alternative execution paths.
    #[must_use]
    pub(crate) fn alt_join(self, other: Self) -> Self {
        Self {
            cardinality: self.cardinality.alt_join(other.cardinality),
            consumption: self.consumption.join(other.consumption),
        }
    }

    /// Apply CN-1 once at a demand observation boundary.
    #[must_use]
    pub(crate) fn observe(self) -> Self {
        if self.cardinality == Cardinality::Absent || self.consumption == Consumption::Dead {
            Self::ZERO
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eager_observation_does_not_commute_with_sequential_composition() {
        let projection = RawDemand::new(Cardinality::Absent, Consumption::Affine);
        let eager = projection
            .observe()
            .seq_add(RawDemand::LINEAR_ONCE)
            .observe();
        let deferred = projection.seq_add(RawDemand::LINEAR_ONCE).observe();

        assert_eq!(
            eager,
            RawDemand::new(Cardinality::Once, Consumption::Linear)
        );
        assert_eq!(
            deferred,
            RawDemand::new(Cardinality::Once, Consumption::Unrestricted)
        );
    }

    #[test]
    fn sequential_composition_is_order_independent_before_observation() {
        let contributions = [
            RawDemand::new(Cardinality::Absent, Consumption::Affine),
            RawDemand::LINEAR_ONCE,
            RawDemand::new(Cardinality::Once, Consumption::Affine),
        ];
        let forward = contributions
            .into_iter()
            .fold(RawDemand::ZERO, RawDemand::seq_add);
        let reverse = contributions
            .into_iter()
            .rev()
            .fold(RawDemand::ZERO, RawDemand::seq_add);

        assert_eq!(forward, reverse);
        assert_eq!(forward.observe(), reverse.observe());
    }

    #[test]
    fn scalar_project_contribution_is_live_once_or_pending_dead() {
        assert_eq!(
            RawDemand::scalar_project_contribution(true),
            RawDemand::new(Cardinality::Once, Consumption::Affine)
        );
        assert_eq!(
            RawDemand::scalar_project_contribution(false),
            RawDemand::new(Cardinality::Absent, Consumption::Affine)
        );
    }
}
