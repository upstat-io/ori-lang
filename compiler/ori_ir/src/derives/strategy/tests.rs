use super::*;
use crate::derives::DerivedTrait;

#[test]
fn eq_strategy() {
    let s = DerivedTrait::Eq.strategy();
    assert_eq!(
        s.struct_body,
        StructBody::ForEachField {
            field_op: FieldOp::Equals,
            combine: CombineOp::AllTrue,
        }
    );
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

#[test]
fn clone_strategy() {
    let s = DerivedTrait::Clone.strategy();
    assert_eq!(s.struct_body, StructBody::CloneFields);
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

#[test]
fn hashable_strategy() {
    let s = DerivedTrait::Hashable.strategy();
    assert_eq!(
        s.struct_body,
        StructBody::ForEachField {
            field_op: FieldOp::Hash,
            combine: CombineOp::HashCombine,
        }
    );
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

#[test]
fn printable_strategy() {
    let s = DerivedTrait::Printable.strategy();
    assert_eq!(
        s.struct_body,
        StructBody::FormatFields {
            open: FormatOpen::TypeNameParen,
            separator: ", ",
            suffix: ")",
            include_names: false,
        }
    );
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

#[test]
fn debug_strategy() {
    let s = DerivedTrait::Debug.strategy();
    assert_eq!(
        s.struct_body,
        StructBody::FormatFields {
            open: FormatOpen::TypeNameBrace,
            separator: ", ",
            suffix: " }",
            include_names: true,
        }
    );
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

#[test]
fn default_strategy() {
    let s = DerivedTrait::Default.strategy();
    assert_eq!(s.struct_body, StructBody::DefaultConstruct);
    assert_eq!(s.sum_body, SumBody::NotSupported);
}

#[test]
fn comparable_strategy() {
    let s = DerivedTrait::Comparable.strategy();
    assert_eq!(
        s.struct_body,
        StructBody::ForEachField {
            field_op: FieldOp::Compare,
            combine: CombineOp::Lexicographic,
        }
    );
    assert_eq!(s.sum_body, SumBody::MatchVariants);
}

/// Every derived trait has a strategy — no panics, no unimplemented.
#[test]
fn all_traits_have_strategies() {
    for &trait_kind in DerivedTrait::ALL {
        let _s = trait_kind.strategy();
    }
}

/// Traits that don't support sum types have `SumBody::NotSupported`.
#[test]
fn sum_support_matches_strategy() {
    for &trait_kind in DerivedTrait::ALL {
        let s = trait_kind.strategy();
        if trait_kind.supports_sum_types() {
            assert_eq!(
                s.sum_body,
                SumBody::MatchVariants,
                "{trait_kind:?} supports sum types but strategy says NotSupported",
            );
        } else {
            assert_eq!(
                s.sum_body,
                SumBody::NotSupported,
                "{trait_kind:?} doesn't support sum types but strategy says MatchVariants",
            );
        }
    }
}
