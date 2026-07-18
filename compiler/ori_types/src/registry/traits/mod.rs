//! Registry for traits and their implementations.
//!
//! [`TraitRegistry`] indexes definitions by name and type, and implementations
//! by receiver and trait. Lookup, coherence, supertrait traversal, and mutation
//! operate on the same indexed entry vectors.

mod lookup;
mod registration;
mod supertraits;

use std::collections::BTreeMap;

use ori_ir::{ExprId, Name, Span};
use rustc_hash::FxHashMap;

use crate::Idx;

/// Registry for traits and their implementations.
///
/// Provides efficient lookup of trait definitions and implementations
/// for method resolution.
///
/// Traits are stored in a single `Vec<TraitEntry>`, with lookup maps
/// holding `usize` indices (same pattern as `impls`). This avoids
/// cloning `TraitEntry` on registration.
#[derive(Clone, Debug, Default)]
pub struct TraitRegistry {
    /// All registered trait definitions.
    pub(super) traits: Vec<TraitEntry>,

    /// Name → trait index (`BTreeMap` for deterministic iteration).
    pub(super) traits_by_name: BTreeMap<Name, usize>,

    /// Pool `Idx` → trait index.
    pub(super) traits_by_idx: FxHashMap<Idx, usize>,

    /// All implementations.
    pub(super) impls: Vec<ImplEntry>,

    /// Type-checker-owned semantic origin parallel to `impls`.
    pub(super) impl_origins: Vec<Option<RegisteredImplOrigin>>,

    /// Quick lookup: `self_type` -> impl indices.
    /// Enables O(1) lookup of implementations for a given type.
    pub(super) impls_by_type: FxHashMap<Idx, Vec<usize>>,

    /// Quick lookup: `trait_idx` -> impl indices.
    /// Enables coherence checking and trait method resolution.
    pub(super) impls_by_trait: FxHashMap<Idx, Vec<usize>>,
}

/// Module-local semantic origin of one registered implementation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RegisteredImplOrigin {
    /// Source or inherited-default body in a parsed impl block.
    Source { impl_index: usize },
    /// Source body in an extension block. Extension owners occupy the
    /// module-local index range immediately after parsed impl blocks.
    Extension {
        owner_index: usize,
        target_name: Name,
    },
    /// Compiler-generated accepted derive.
    Derived(ori_ir::DerivedImplId),
    /// Method templates imported from another producer module.
    Imported(FxHashMap<ExprId, ImportedMethodOrigin>),
}

/// Stable imported producer identity for one method body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ImportedMethodOrigin {
    pub(crate) symbol: Box<str>,
    pub(crate) signature_hash: u64,
}

/// A registered trait definition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraitEntry {
    /// The trait name.
    pub name: Name,

    /// Pool index for this trait type.
    pub idx: Idx,

    /// Generic type parameters (e.g., `T` in `trait Foo<T>`).
    pub type_params: Vec<Name>,

    /// Super-trait pool indices (direct parents in the inheritance DAG).
    pub super_traits: Vec<Idx>,

    /// Method signatures defined by this trait.
    pub methods: FxHashMap<Name, TraitMethodDef>,

    /// Associated types defined by this trait.
    pub assoc_types: FxHashMap<Name, TraitAssocTypeDef>,

    /// Object safety violations found in this trait's methods.
    ///
    /// Empty means the trait is object-safe (can be used as a trait object).
    /// Computed during registration by analyzing method signatures for:
    /// - `Self` in return position (can't know size at runtime)
    /// - `Self` in parameter position except receiver (can't verify type match)
    /// - Generic methods (require monomorphization, incompatible with vtable)
    pub object_safety_violations: Vec<ObjectSafetyViolation>,

    /// Source location of the definition.
    pub span: Span,
}

impl TraitEntry {
    /// Check if this trait can be used as a trait object.
    ///
    /// A trait is object-safe if none of its methods violate the three rules:
    /// 1. No `Self` in return position
    /// 2. No `Self` in parameter position (except receiver)
    /// 3. No generic methods
    #[inline]
    pub fn is_object_safe(&self) -> bool {
        self.object_safety_violations.is_empty()
    }
}

/// A reason why a trait is not object-safe.
///
/// Each variant corresponds to a rule that the trait violates.
/// A trait with any violations cannot be used as a trait object.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ObjectSafetyViolation {
    /// Method returns `Self` — unknown size at runtime (Rule 1).
    SelfReturn {
        /// The method that returns `Self`.
        method: Name,
        /// Source location of the method.
        span: Span,
    },

    /// Method takes `Self` as a non-receiver parameter — can't verify type
    /// match at runtime (Rule 2).
    SelfParam {
        /// The method with `Self` parameter.
        method: Name,
        /// The parameter name that has `Self` type.
        param: Name,
        /// Source location of the method.
        span: Span,
    },

    /// Method has its own generic type parameters — requires monomorphization,
    /// which is incompatible with vtable dispatch (Rule 3).
    GenericMethod {
        /// The method with generic parameters.
        method: Name,
        /// Source location of the method.
        span: Span,
    },
}

/// A trait method signature.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitMethodDef {
    /// Method name.
    pub name: Name,

    /// Method signature as a function type index.
    pub signature: Idx,

    /// Whether the method's first parameter is `self` (an instance method) vs a
    /// no-`self` associated function (e.g. a capability method `@get (url: str)`
    /// or `Default::default () -> Self`). Read by bound-chain dispatch to set
    /// `LookupOutcome::has_self` so the arity check skips `self` only when the
    /// method actually declares it.
    pub has_self: bool,

    /// Whether this method has a default implementation.
    pub has_default: bool,

    /// Default implementation body (if `has_default` is true).
    pub default_body: Option<ExprId>,

    /// Pool `var_id`s for the method's own quantified type variables.
    /// Mirrors `FunctionSig.scheme_var_ids` — empty for non-generic methods.
    pub scheme_var_ids: Vec<u32>,

    /// Method-level generic parameter metadata, deep-copied from the AST's
    /// `GenericParamRange` into arena-independent owned form.
    /// Empty when the method has no generic parameters.
    pub generic_param_metadata: Vec<GenericParamMeta>,

    /// Method-level where-clause constraints, deep-copied into resolved form.
    /// Empty when the method has no where clause.
    pub where_clause_metadata: Vec<WhereConstraint>,

    /// Fixed-list capacity expressions that depend on this method's const
    /// binders, retained for concrete call-site validation.
    pub fixed_list_capacity_constraints: Vec<crate::GenericConstExpr>,

    /// Source location.
    pub span: Span,
}

/// An associated type in a trait.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TraitAssocTypeDef {
    /// Associated type name.
    pub name: Name,

    /// Bounds on the associated type (trait constraints).
    pub bounds: Vec<Idx>,

    /// Default type (if any).
    pub default: Option<Idx>,

    /// Source location.
    pub span: Span,
}

/// A trait implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImplEntry {
    /// The trait being implemented (`None` for inherent impls).
    pub trait_idx: Option<Idx>,

    /// Concrete type arguments for the trait (e.g., `[INT, STR]` for
    /// `impl T: Index<int, str>`). Empty for non-generic traits or
    /// inherent impls. Used by coherence checking to distinguish different
    /// instantiations of the same generic trait.
    pub trait_type_args: Vec<Idx>,

    /// The self type for this implementation.
    pub self_type: Idx,

    /// Generic type parameters on this impl.
    pub type_params: Vec<Name>,

    /// Trait bounds per type parameter, index-aligned with `type_params`
    /// (mirrors `FunctionSig.type_param_bounds`). Captures the inline
    /// `impl<T: Eq>` bound and the derive-implied conditional bound that the
    /// trailing `where_clause` surface does not carry. Empty inner vec = an
    /// unbounded parameter. The operator-presence gate validates these against
    /// the instantiation so an unsatisfied generic impl does not count as
    /// implementing the trait.
    /// Spec: operator-rules.md "Equality"/"Ordering" — operands shall implement the trait.
    pub type_param_bounds: Vec<Vec<Name>>,

    /// Method implementations.
    pub methods: FxHashMap<Name, ImplMethodDef>,

    /// Associated type implementations.
    pub assoc_types: FxHashMap<Name, Idx>,

    /// Where clause constraints.
    pub where_clause: Vec<WhereConstraint>,

    /// How specific this implementation is (Concrete > Constrained > Generic).
    pub specificity: ImplSpecificity,

    /// Source location.
    pub span: Span,
}

/// How specific a trait implementation is.
///
/// Used for overlap detection: when multiple impls could apply, the most
/// specific one wins. Equal-specificity impls for the same trait are an
/// overlap error (E2021).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ImplSpecificity {
    /// `impl<T> T: Trait` — applies to all types.
    Generic = 0,
    /// `impl<T: Bound> T: Trait` — applies to types satisfying bounds.
    Constrained = 1,
    /// `impl ConcreteType: Trait` — applies to exactly one type.
    Concrete = 2,
}

/// A method implementation in an impl block.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImplMethodDef {
    /// Method name.
    pub name: Name,

    /// Method signature (function type).
    pub signature: Idx,

    /// Whether the first parameter is `self` (instance method vs associated function).
    pub has_self: bool,

    /// Method body expression.
    pub body: ExprId,

    /// Pool `var_id`s for the method's own quantified type variables.
    /// Mirrors `FunctionSig.scheme_var_ids` — empty for non-generic methods.
    pub scheme_var_ids: Vec<u32>,

    /// Method-level generic parameter metadata, deep-copied from the AST's
    /// `GenericParamRange` into arena-independent owned form.
    /// Empty when the method has no generic parameters.
    pub generic_param_metadata: Vec<GenericParamMeta>,

    /// Method-level where-clause constraints, deep-copied into resolved form.
    /// Empty when the method has no where clause.
    pub where_clause_metadata: Vec<WhereConstraint>,

    /// Fixed-list capacity expressions that depend on this method's const
    /// binders, retained for concrete call-site validation.
    pub fixed_list_capacity_constraints: Vec<crate::GenericConstExpr>,

    /// Count of non-`self` parameters WITH a default value.
    /// A call is arity-valid when `arg_count` is in
    /// `[total_non_self - optional_param_count, total_non_self]`; omitted
    /// trailing defaults are filled in canon. `0` = all params required
    /// (the natural value for derived / no-default methods → strict arity).
    pub optional_param_count: usize,

    /// Source location.
    pub span: Span,
}

/// A where clause constraint.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WhereConstraint {
    /// The constrained type.
    pub ty: Idx,

    /// The trait bounds on this type.
    pub bounds: Vec<Idx>,
}

/// Method-level generic parameter metadata, arena-independent.
///
/// Deep-copied from the AST's `GenericParamRange` at registration time so
/// the registry-side definition does not leak arena lifetimes — mirrors the
/// `FunctionSig.scheme_var_ids` precedent of resolving AST data into owned
/// form. All trait/index fields use the type pool's stable `Idx` and the
/// existing `WhereConstraint` shape.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GenericParamMeta {
    /// Parameter name (e.g., `T` in `<T: Eq>`).
    pub name: Name,

    /// `false` for type-generic params (`<T>`); `true` for const-generic
    /// params (`<$N: int>`).
    pub is_const: bool,

    /// Trait bounds for this parameter, fully resolved.
    /// `T: Eq + Clone` becomes `vec![Idx_of_Eq, Idx_of_Clone]`.
    pub bounds: Vec<Idx>,

    /// Default type for type-generic parameters (e.g., `<T = int>`).
    pub default_type: Option<Idx>,

    /// Type of a const-generic parameter (e.g., the `int` in `<$N: int>`).
    /// Always `None` when `is_const == false`.
    pub const_type: Option<Idx>,

    /// Default value of a const-generic parameter (e.g., `<$N: int = 42>`).
    /// Always `None` when `is_const == false`.
    pub const_default_value: Option<crate::GenericConstExpr>,

    /// Projection-bound constraints attached to this parameter
    /// (e.g., `T.Item: Eq` shape per associated-type clarification note).
    pub projection_bounds: Vec<WhereConstraint>,
}

impl TraitRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Result of a method lookup.
#[derive(Clone, Debug)]
pub enum MethodLookup<'a> {
    /// Method from an inherent impl.
    Inherent {
        /// Index of the impl block.
        impl_idx: usize,
        /// The method definition.
        method: &'a ImplMethodDef,
    },

    /// Method from a trait impl.
    Trait {
        /// The trait being implemented.
        trait_idx: Idx,
        /// Index of the impl block.
        impl_idx: usize,
        /// The method definition.
        method: &'a ImplMethodDef,
    },

    /// Method from an extension block.
    Extension {
        /// Index of the registered extension provider.
        impl_idx: usize,
        /// The method definition.
        method: &'a ImplMethodDef,
    },
}

impl<'a> MethodLookup<'a> {
    /// Get the method definition.
    #[inline]
    pub fn method(&self) -> &'a ImplMethodDef {
        match self {
            Self::Inherent { method, .. }
            | Self::Trait { method, .. }
            | Self::Extension { method, .. } => method,
        }
    }

    /// Get the impl index.
    #[inline]
    pub fn impl_idx(&self) -> usize {
        match self {
            Self::Inherent { impl_idx, .. }
            | Self::Trait { impl_idx, .. }
            | Self::Extension { impl_idx, .. } => *impl_idx,
        }
    }

    /// Check if this is an inherent method.
    #[inline]
    pub fn is_inherent(&self) -> bool {
        matches!(self, Self::Inherent { .. })
    }

    /// Get the trait index if this is a trait method.
    #[inline]
    pub fn trait_idx(&self) -> Option<Idx> {
        match self {
            Self::Inherent { .. } | Self::Extension { .. } => None,
            Self::Trait { trait_idx, .. } => Some(*trait_idx),
        }
    }
}

/// Result of a checked method lookup (with ambiguity detection).
#[derive(Clone, Debug)]
pub enum MethodLookupResult<'a> {
    /// Exactly one method found (inherent or trait).
    Found(MethodLookup<'a>),

    /// Multiple trait impls provide the same method for this type.
    Ambiguous {
        /// The trait indices and names that provide the method.
        candidates: Vec<(Idx, Name)>,
    },

    /// No method found in any impl.
    NotFound,
}

/// Result of a bound-chain method lookup.
///
/// Returned by `TraitRegistry::find_trait_method_via_bound_chain`. Distinct
/// from `MethodLookupResult` because the resolved method is a trait-level
/// `TraitMethodDef` (signature only — no concrete body), not an
/// `ImplMethodDef` from a registered impl. Dispatch sites consume this
/// when the receiver is `Tag::RigidVar` and translate the trait-method
/// signature into a callable shape (substituting the rigid var for `Self`
/// at the call site).
#[derive(Clone, Debug)]
pub enum BoundChainLookup<'a> {
    /// Exactly one bound provides this method.
    Found {
        /// The trait that contributes the method.
        trait_idx: Idx,
        /// The method definition (signature, default body if any).
        method: &'a TraitMethodDef,
    },

    /// Multiple bounds provide the same method name — ambiguous.
    Ambiguous {
        /// Trait indices and names that provide the method.
        candidates: Vec<(Idx, Name)>,
    },

    /// No bound on the rigid var provides the method.
    NotFound,
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "trait-registry tests abort when a required fixture method is absent"
)]
mod tests;
