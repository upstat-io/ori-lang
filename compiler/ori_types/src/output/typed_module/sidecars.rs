//! Typed-module metadata and type-directed canonicalization sidecars.

use ori_ir::canon::IndexDispatch;
use ori_ir::{ExprId, Name, ReprAttrKind};

use crate::Idx;

/// Lexical origin selected for one implicit capability-provider argument.
///
/// The origin is retained for diagnostics and auditability, while Canon
/// materializes the source-erased argument through the capability namespace.
/// Provider values never participate in specialization identity; only their
/// concrete types do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CapabilityProviderSource {
    /// Provider forwarded from the current function's hidden parameter.
    Parameter { provider_var_id: u32 },
    /// Provider introduced by the innermost matching `with ... in` binding.
    WithBinding { provider: ExprId },
    /// Stateless provider selected from a `def impl`.
    DefaultImpl,
}

/// One ordered implicit provider selected at a concrete call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CapabilityProvider {
    /// Capability/trait namespace bound by the provider.
    pub capability: Name,
    /// Concrete provider type at this lexical call site.
    pub provider_type: Idx,
    /// Exact lexical source of the provider value.
    pub source: CapabilityProviderSource,
}

/// Frozen capability-provider selection for one free-function call.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapabilityCallSite {
    /// Exact checker-selected free callable identity.
    pub callee: Name,
    /// Ordered value providers; marker capabilities are intentionally absent.
    pub providers: Vec<CapabilityProvider>,
}

/// Per-type metadata exported for cross-module repr plan construction.
///
/// The Merkle hash identifies the type; its `#repr` and visibility metadata
/// remain outside structural identity. Importing modules use the metadata to
/// protect public and explicitly represented types from ABI-breaking narrowing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ExportedTypeMetadata {
    /// Merkle hash of the type's Pool representation.
    ///
    /// Used to map back to a local `Idx` in the importing module's pool
    /// via `Pool::lookup_by_hash()`.
    pub merkle_hash: u64,

    /// Representation attribute (`#repr("c")`, `#repr("packed")`, etc.).
    ///
    /// `None` means default layout (all optimizations permitted).
    pub repr: Option<ReprAttrKind>,

    /// Whether this type is `pub` in its defining module.
    ///
    /// Public types have ABI contracts that narrowing must not violate.
    pub is_public: bool,
}

/// Type-directed desugar plan for one `ExprKind::AssignTarget` chain.
///
/// `level_types[k]` is the resolved type of the receiver-read after applying
/// the first `k` access steps; it contains `steps.len() + 1` entries.
/// `step_routes[k]` freezes the semantic route for access step `k` before
/// canonical lowering rebuilds the chain as pure reassignment.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssignDesugar {
    /// Resolved receiver-read type at each chain level (length `steps + 1`).
    pub level_types: Vec<Idx>,
    /// Type-checker-selected route for each access step.
    pub step_routes: Vec<AssignStepRoute>,
}

/// Frozen semantic route for one assignment-target access step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AssignStepRoute {
    Field,
    Index(IndexDispatch),
}

/// Type-directed canonicalization plan for an iterator method.
///
/// `iter_ty` types a synthesized `receiver.iter()` node when the source
/// receiver is only `Iterable`; `None` preserves an already-iterator receiver.
/// `adapter_ty` is present for eager collection methods whose iterator adapter
/// must be collected immediately. `collect_ty` freezes a type-checker-selected
/// `Collect` target so canonicalization never rediscovers it from method text.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct IterMethodRoute {
    /// Type of the synthesized `receiver.iter()` node, if one is required.
    pub iter_ty: Option<Idx>,
    /// Type of the intermediate iterator adapter collected by canonicalization.
    pub adapter_ty: Option<Idx>,
    /// Exact result collection selected by bidirectional type checking.
    pub collect_ty: Option<Idx>,
}

/// Pool `Idx` values for the builtin `FormatSpec` struct and its `Option<_>`
/// field types, captured at type-check time.
///
/// Canonical lowering uses them to type synthesized formatting nodes precisely
/// enough for LLVM layout. Successful checking always registers the builtin.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FormatSpecTypes {
    /// The `FormatSpec` struct type.
    pub spec: Idx,
    /// `Option<char>` — the `fill` field type.
    pub opt_char: Idx,
    /// `Option<Alignment>` — the `align` field type.
    pub opt_alignment: Idx,
    /// `Option<Sign>` — the `sign` field type.
    pub opt_sign: Idx,
    /// `Option<int>` — the `width` and `precision` field types.
    pub opt_int: Idx,
    /// `Option<FormatType>` — the `format_type` field type.
    pub opt_format_type: Idx,
    /// The `Alignment` enum type (variant-`Ident` node type).
    pub alignment: Idx,
    /// The `Sign` enum type (variant-`Ident` node type).
    pub sign: Idx,
    /// The `FormatType` enum type (variant-`Ident` node type).
    pub format_type: Idx,
}
