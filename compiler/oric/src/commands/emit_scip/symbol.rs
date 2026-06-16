//! SCIP symbol-string minting for Ori definition entities.
//!
//! Builds a `scip::types::Symbol` per entity and renders it through the
//! crate's `format_symbol`, which implements the SCIP symbol-string grammar
//! (`<scheme> <package> <descriptors>`, empty package components rendered as
//! `.`).
//!
//! The minted strings are the cross-section join key: occurrence -> definition
//! resolution is exact string equality, so each builder is deterministic
//! (no timestamps, no hashed disambiguators, no map-iteration ordering).

use scip::symbol::format_symbol;
use scip::types::symbol_information::Kind;
use scip::types::{descriptor::Suffix, Descriptor, Symbol};

/// SCIP symbol scheme for Ori symbols.
pub const SCIP_SCHEME: &str = "ori";

/// One emitted definition: its SCIP symbol string, display name, and kind.
pub struct ScipDef {
    /// The globally-stable SCIP symbol string.
    pub symbol: String,
    /// The source identifier (used as `SymbolInformation.display_name`).
    pub display_name: String,
    /// The SCIP symbol kind.
    pub kind: Kind,
}

fn descriptor(name: &str, suffix: Suffix) -> Descriptor {
    Descriptor {
        name: name.to_string(),
        suffix: suffix.into(),
        ..Default::default()
    }
}

/// Render a descriptor chain into a SCIP symbol string under the `ori` scheme
/// with an empty package — `format_symbol` renders each empty package
/// component (manager / name / version) as `.`.
fn mint(descriptors: Vec<Descriptor>) -> String {
    format_symbol(Symbol {
        scheme: SCIP_SCHEME.to_string(),
        descriptors,
        ..Default::default()
    })
}

/// `<fn>().` — a top-level function (Method-suffix descriptor).
pub fn function(name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![descriptor(name, Suffix::Method)]),
        display_name: name.to_string(),
        kind: Kind::Function,
    }
}

/// `<Type>#` — a struct / sum / newtype declaration; `kind` distinguishes them.
pub fn type_def(name: &str, kind: Kind) -> ScipDef {
    ScipDef {
        symbol: mint(vec![descriptor(name, Suffix::Type)]),
        display_name: name.to_string(),
        kind,
    }
}

/// `<Trait>#` — a trait declaration.
pub fn trait_def(name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![descriptor(name, Suffix::Type)]),
        display_name: name.to_string(),
        kind: Kind::Trait,
    }
}

/// `<Type>#<field>.` — a struct field (Type then Term descriptor).
pub fn field(type_name: &str, field_name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![
            descriptor(type_name, Suffix::Type),
            descriptor(field_name, Suffix::Term),
        ]),
        display_name: field_name.to_string(),
        kind: Kind::Field,
    }
}

/// `<Type>#<Variant>.` — a sum-type variant.
pub fn variant(type_name: &str, variant_name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![
            descriptor(type_name, Suffix::Type),
            descriptor(variant_name, Suffix::Term),
        ]),
        display_name: variant_name.to_string(),
        kind: Kind::EnumMember,
    }
}

/// `<Type>#<Variant>.<field>.` — a field of a sum-type variant.
pub fn variant_field(type_name: &str, variant_name: &str, field_name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![
            descriptor(type_name, Suffix::Type),
            descriptor(variant_name, Suffix::Term),
            descriptor(field_name, Suffix::Term),
        ]),
        display_name: field_name.to_string(),
        kind: Kind::Field,
    }
}

/// `<Type>#<method>().` — an impl method or trait-method declaration.
pub fn method(type_name: &str, method_name: &str) -> ScipDef {
    ScipDef {
        symbol: mint(vec![
            descriptor(type_name, Suffix::Type),
            descriptor(method_name, Suffix::Method),
        ]),
        display_name: method_name.to_string(),
        kind: Kind::Method,
    }
}
