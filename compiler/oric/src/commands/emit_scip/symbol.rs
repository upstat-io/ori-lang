//! SCIP symbol-string minting for Ori definition entities.
//!
//! Builds a `scip::types::Symbol` per entity and renders it through the
//! crate's `format_symbol`.
//!
//! Every symbol carries the source module's identity in the SCIP package-name
//! slot, so two same-named definitions in different source files mint distinct
//! strings — the occurrence -> definition resolution (exact string equality)
//! never mis-resolves a cross-file homonym.
//!
//! Each builder is deterministic — no timestamps, no hashed disambiguators, no
//! absolute paths, no map-iteration ordering.

use ori_ir::Span;
use protobuf::MessageField;
use scip::symbol::format_symbol;
use scip::types::symbol_information::Kind;
use scip::types::{descriptor::Suffix, Descriptor, Package, Symbol};

/// SCIP symbol scheme for Ori symbols.
pub const SCIP_SCHEME: &str = "ori";

/// How the identifier-token span is anchored within a definition's source
/// extent. The name token is located by skipping a known declaration prefix
/// from the item span's start — anchored at the declaration keyword, never a
/// bare substring search (which can false-match the name inside a doc comment,
/// string literal, or an earlier identifier).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NameAnchor {
    /// `@name (...) -> ...` — a function / method. The name follows the `@`
    /// sigil (and any leading `pub ` / whitespace).
    AtSigil,
    /// `type Name`, `trait Name` — a type / trait declaration. The name follows
    /// the declaration keyword (and any leading `pub ` / whitespace).
    Keyword,
    /// A field / variant / variant-field — the item span starts at the name
    /// token itself (no leading declaration keyword).
    Direct,
}

/// One emitted definition: its SCIP symbol string, display name, kind, and the
/// source extent + name-anchor used to derive its identifier-token range and
/// body-spanning enclosing range for its Definition-role occurrence.
pub struct ScipDef {
    /// The globally-stable SCIP symbol string.
    pub symbol: String,
    /// The source identifier (used as `SymbolInformation.display_name`).
    pub display_name: String,
    /// The SCIP symbol kind.
    pub kind: Kind,
    /// The definition's full source extent (body included) — the basis for the
    /// Definition occurrence's `enclosing_range`.
    pub span: Span,
    /// How to locate the identifier token within `span`.
    pub name_anchor: NameAnchor,
}

fn descriptor(name: &str, suffix: Suffix) -> Descriptor {
    Descriptor {
        name: name.to_string(),
        suffix: suffix.into(),
        ..Default::default()
    }
}

/// Mints SCIP symbols scoped to one source module.
///
/// The module identity rides in the package-name component so definitions of
/// the same name in different files never collide on the exact-equality join.
pub struct ModuleMinter {
    module: String,
}

impl ModuleMinter {
    /// Bind the minter to a project-relative module identity (the source path
    /// with its `.ori` extension dropped, forward-slash normalized).
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }

    /// The SCIP package carrying this module's identity in the name slot. An
    /// empty module yields an absent package (`format_symbol` renders the
    /// missing components as `.`).
    fn package(&self) -> MessageField<Package> {
        if self.module.is_empty() {
            MessageField::none()
        } else {
            MessageField::some(Package {
                name: self.module.clone(),
                ..Default::default()
            })
        }
    }

    /// Render a descriptor chain into a SCIP symbol string under the `ori`
    /// scheme with this module's package identity.
    fn mint(&self, descriptors: Vec<Descriptor>) -> String {
        format_symbol(Symbol {
            scheme: SCIP_SCHEME.to_string(),
            package: self.package(),
            descriptors,
            ..Default::default()
        })
    }

    /// `<fn>().` — a top-level function (Method-suffix descriptor).
    pub fn function(&self, name: &str, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![descriptor(name, Suffix::Method)]),
            display_name: name.to_string(),
            kind: Kind::Function,
            span,
            name_anchor: NameAnchor::AtSigil,
        }
    }

    /// `<Type>#` — a struct / sum / newtype declaration; `kind` distinguishes them.
    pub fn type_def(&self, name: &str, kind: Kind, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![descriptor(name, Suffix::Type)]),
            display_name: name.to_string(),
            kind,
            span,
            name_anchor: NameAnchor::Keyword,
        }
    }

    /// `<Trait>#` — a trait declaration.
    pub fn trait_def(&self, name: &str, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![descriptor(name, Suffix::Type)]),
            display_name: name.to_string(),
            kind: Kind::Trait,
            span,
            name_anchor: NameAnchor::Keyword,
        }
    }

    /// `<Type>#<field>.` — a struct field (Type then Term descriptor).
    pub fn field(&self, type_name: &str, field_name: &str, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![
                descriptor(type_name, Suffix::Type),
                descriptor(field_name, Suffix::Term),
            ]),
            display_name: field_name.to_string(),
            kind: Kind::Field,
            span,
            name_anchor: NameAnchor::Direct,
        }
    }

    /// `<Type>#<Variant>.` — a sum-type variant.
    pub fn variant(&self, type_name: &str, variant_name: &str, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![
                descriptor(type_name, Suffix::Type),
                descriptor(variant_name, Suffix::Term),
            ]),
            display_name: variant_name.to_string(),
            kind: Kind::EnumMember,
            span,
            name_anchor: NameAnchor::Direct,
        }
    }

    /// `<Type>#<Variant>.<field>.` — a field of a sum-type variant.
    pub fn variant_field(
        &self,
        type_name: &str,
        variant_name: &str,
        field_name: &str,
        span: Span,
    ) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![
                descriptor(type_name, Suffix::Type),
                descriptor(variant_name, Suffix::Term),
                descriptor(field_name, Suffix::Term),
            ]),
            display_name: field_name.to_string(),
            kind: Kind::Field,
            span,
            name_anchor: NameAnchor::Direct,
        }
    }

    /// `<Type>#<method>().` — an impl method, trait-method, def-impl method, or
    /// extension method declaration. `parent` is the trait name for def-impl
    /// methods and the target type for extensions / inherent impls.
    pub fn method(&self, parent: &str, method_name: &str, span: Span) -> ScipDef {
        ScipDef {
            symbol: self.mint(vec![
                descriptor(parent, Suffix::Type),
                descriptor(method_name, Suffix::Method),
            ]),
            display_name: method_name.to_string(),
            kind: Kind::Method,
            span,
            name_anchor: NameAnchor::AtSigil,
        }
    }
}
