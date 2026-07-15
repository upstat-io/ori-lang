//! Checked planning-scratch payload accounting shared by physical analyses.

use std::mem::size_of;

use crate::bytecode::{Constant, Register};

use super::PrepareError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum KnownValue {
    Int(i64),
    Bool(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KnownFact {
    register: Register,
    value: KnownValue,
}

/// Sorted exact facts; absence means that no immediate value is proven.
#[derive(Debug, Default)]
pub(super) struct KnownFacts {
    entries: Vec<KnownFact>,
}

impl KnownFacts {
    pub(super) const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(super) fn get(&self, register: Register) -> Option<KnownValue> {
        self.search(register)
            .ok()
            .map(|index| self.entries[index].value)
    }

    pub(super) fn assign_constant(
        &mut self,
        register: Register,
        constant: Constant,
        scratch: &mut ScratchAccount,
    ) -> Result<(), PrepareError> {
        self.assign(register, known_constant(constant), scratch)
    }

    pub(super) fn assign_copy(
        &mut self,
        destination: Register,
        source: Register,
        scratch: &mut ScratchAccount,
    ) -> Result<(), PrepareError> {
        let value = self.get(source);
        self.assign(destination, value, scratch)
    }

    pub(super) fn assign_from(
        &mut self,
        destination: Register,
        snapshot: &Self,
        source: Register,
        scratch: &mut ScratchAccount,
    ) -> Result<(), PrepareError> {
        self.assign(destination, snapshot.get(source), scratch)
    }

    pub(super) fn forget(&mut self, register: Register) {
        if let Ok(index) = self.search(register) {
            self.entries.remove(index);
        }
    }

    pub(super) fn clone_tracked(&self, scratch: &mut ScratchAccount) -> Result<Self, PrepareError> {
        let entries = self.entries.clone();
        scratch.allocate_elements::<KnownFact>(entries.capacity(), "sparse known-fact payload")?;
        Ok(Self { entries })
    }

    pub(super) fn intersect_with(&mut self, incoming: &Self) -> bool {
        let previous = self.entries.len();
        self.entries
            .retain(|fact| incoming.get(fact.register) == Some(fact.value));
        self.entries.len() != previous
    }

    pub(super) fn release(self, scratch: &mut ScratchAccount) -> Result<(), PrepareError> {
        scratch.release_elements::<KnownFact>(self.entries.capacity(), "sparse known-fact payload")
    }

    fn assign(
        &mut self,
        register: Register,
        value: Option<KnownValue>,
        scratch: &mut ScratchAccount,
    ) -> Result<(), PrepareError> {
        match (self.search(register), value) {
            (Ok(index), Some(value)) => self.entries[index].value = value,
            (Ok(index), None) => {
                self.entries.remove(index);
            }
            (Err(index), Some(value)) => {
                let previous = self.entries.capacity();
                self.entries.insert(index, KnownFact { register, value });
                scratch.grow_elements::<KnownFact>(
                    previous,
                    self.entries.capacity(),
                    "sparse known-fact payload",
                )?;
            }
            (Err(_), None) => {}
        }
        Ok(())
    }

    fn search(&self, register: Register) -> Result<usize, usize> {
        self.entries
            .binary_search_by_key(&register.index(), |fact| fact.register.index())
    }
}

const fn known_constant(constant: Constant) -> Option<KnownValue> {
    match constant {
        Constant::Int(value) => Some(KnownValue::Int(value)),
        Constant::Bool(value) => Some(KnownValue::Bool(value)),
        Constant::Unit
        | Constant::Float(_)
        | Constant::String(_)
        | Constant::Char(_)
        | Constant::Null => None,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PlanningScratchMetrics {
    pub(super) current_payload_bytes: usize,
    pub(super) peak_payload_bytes_lower_bound: usize,
    pub(super) cumulative_allocation_bytes_lower_bound: usize,
}

impl PlanningScratchMetrics {
    pub(super) fn then(self, next: Self) -> Result<Self, PrepareError> {
        Ok(Self {
            current_payload_bytes: next.current_payload_bytes,
            peak_payload_bytes_lower_bound: self
                .peak_payload_bytes_lower_bound
                .max(next.peak_payload_bytes_lower_bound),
            cumulative_allocation_bytes_lower_bound: self
                .cumulative_allocation_bytes_lower_bound
                .checked_add(next.cumulative_allocation_bytes_lower_bound)
                .ok_or(PrepareError::MetricOverflow {
                    metric: "planning cumulative scratch allocation bytes",
                })?,
        })
    }
}

pub(super) struct ScratchAccount {
    function: usize,
    metrics: PlanningScratchMetrics,
}

impl ScratchAccount {
    pub(super) const fn new(function: usize) -> Self {
        Self {
            function,
            metrics: PlanningScratchMetrics {
                current_payload_bytes: 0,
                peak_payload_bytes_lower_bound: 0,
                cumulative_allocation_bytes_lower_bound: 0,
            },
        }
    }

    pub(super) const fn function_index(&self) -> usize {
        self.function
    }

    pub(super) fn allocate_elements<T>(
        &mut self,
        elements: usize,
        metric: &'static str,
    ) -> Result<(), PrepareError> {
        let bytes =
            size_of::<T>()
                .checked_mul(elements)
                .ok_or(PrepareError::PlanningScratchOverflow {
                    function: self.function,
                    metric,
                })?;
        self.allocate_bytes(bytes, metric)
    }

    pub(super) fn grow_elements<T>(
        &mut self,
        previous: usize,
        current: usize,
        metric: &'static str,
    ) -> Result<(), PrepareError> {
        let added =
            current
                .checked_sub(previous)
                .ok_or(PrepareError::PlanningScratchAccounting {
                    function: self.function,
                    metric,
                })?;
        self.allocate_elements::<T>(added, metric)
    }

    pub(super) fn release_elements<T>(
        &mut self,
        elements: usize,
        metric: &'static str,
    ) -> Result<(), PrepareError> {
        let bytes =
            size_of::<T>()
                .checked_mul(elements)
                .ok_or(PrepareError::PlanningScratchOverflow {
                    function: self.function,
                    metric,
                })?;
        self.metrics.current_payload_bytes = self
            .metrics
            .current_payload_bytes
            .checked_sub(bytes)
            .ok_or(PrepareError::PlanningScratchAccounting {
                function: self.function,
                metric,
            })?;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<PlanningScratchMetrics, PrepareError> {
        if self.metrics.current_payload_bytes != 0 {
            return Err(PrepareError::PlanningScratchRetained {
                function: self.function,
                bytes: self.metrics.current_payload_bytes,
            });
        }
        Ok(self.metrics)
    }

    fn allocate_bytes(&mut self, bytes: usize, metric: &'static str) -> Result<(), PrepareError> {
        self.metrics.current_payload_bytes = self
            .metrics
            .current_payload_bytes
            .checked_add(bytes)
            .ok_or(PrepareError::PlanningScratchOverflow {
                function: self.function,
                metric,
            })?;
        self.metrics.peak_payload_bytes_lower_bound = self
            .metrics
            .peak_payload_bytes_lower_bound
            .max(self.metrics.current_payload_bytes);
        self.metrics.cumulative_allocation_bytes_lower_bound = self
            .metrics
            .cumulative_allocation_bytes_lower_bound
            .checked_add(bytes)
            .ok_or(PrepareError::PlanningScratchOverflow {
                function: self.function,
                metric,
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PlanningScratchMetrics, ScratchAccount};
    use crate::physical::PrepareError;

    #[test]
    fn scratch_element_size_overflow_is_typed() {
        let mut account = ScratchAccount::new(3);

        assert!(matches!(
            account.allocate_elements::<[u8; 2]>(usize::MAX, "test table"),
            Err(PrepareError::PlanningScratchOverflow {
                function: 3,
                metric: "test table"
            })
        ));
    }

    #[test]
    fn scratch_total_overflow_and_accounting_mismatch_are_typed() {
        let mut overflowing = ScratchAccount::new(5);
        overflowing
            .allocate_elements::<u8>(usize::MAX, "test total")
            .unwrap_or_else(|error| panic!("maximum byte count itself must fit: {error}"));
        assert!(matches!(
            overflowing.allocate_elements::<u8>(1, "test total"),
            Err(PrepareError::PlanningScratchOverflow {
                function: 5,
                metric: "test total"
            })
        ));

        let mut underflowing = ScratchAccount::new(7);
        assert!(matches!(
            underflowing.release_elements::<u8>(1, "test release"),
            Err(PrepareError::PlanningScratchAccounting {
                function: 7,
                metric: "test release"
            })
        ));
    }

    #[test]
    fn retained_scratch_and_cross_function_accumulation_overflow_are_typed() {
        let mut retained = ScratchAccount::new(11);
        retained
            .allocate_elements::<u8>(1, "test retained")
            .unwrap_or_else(|error| panic!("one byte must fit: {error}"));
        assert!(matches!(
            retained.finish(),
            Err(PrepareError::PlanningScratchRetained {
                function: 11,
                bytes: 1
            })
        ));

        let first = PlanningScratchMetrics {
            current_payload_bytes: 0,
            peak_payload_bytes_lower_bound: 0,
            cumulative_allocation_bytes_lower_bound: usize::MAX,
        };
        let second = PlanningScratchMetrics {
            current_payload_bytes: 0,
            peak_payload_bytes_lower_bound: 0,
            cumulative_allocation_bytes_lower_bound: 1,
        };
        assert!(matches!(
            first.then(second),
            Err(PrepareError::MetricOverflow {
                metric: "planning cumulative scratch allocation bytes"
            })
        ));
    }
}
