//! Cross-crate exhaustiveness tests for derive strategy dispatch.
//!
//! Verifies that every `DerivedTrait` produces a `DeriveStrategy` with
//! recognized `StructBody` and `SumBody` variants. Defense-in-depth:
//! Rust's exhaustive match already catches most drift, but this test
//! fires if a new trait is added with a novel strategy variant.

use ori_ir::{DerivedTrait, StructBody, SumBody};

/// Every `DerivedTrait` must produce a `DeriveStrategy` whose `struct_body`
/// and `sum_body` variants are all handled by the LLVM backend.
#[test]
#[expect(
    clippy::match_same_arms,
    reason = "exhaustive enumeration is the point of this test"
)]
fn derive_strategy_all_struct_body_variants_handled() {
    for &trait_variant in DerivedTrait::ALL {
        let strategy = trait_variant.strategy();

        // Verify struct_body is a known variant (exhaustive check)
        match strategy.struct_body {
            StructBody::ForEachField { .. } => {}
            StructBody::FormatFields { .. } => {}
            StructBody::CloneFields => {}
            StructBody::DefaultConstruct => {}
        }

        // Verify sum_body is a known variant
        match strategy.sum_body {
            SumBody::MatchVariants => {}
            SumBody::NotSupported => {}
        }
    }
}
