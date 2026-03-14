//! AIMS verification passes.
//!
//! Post-realization verifiers that cross-check analysis contracts
//! against the emitted IR. Catches inconsistencies between what
//! `extract_contract()` claims and what realization achieved.

pub mod fip;
