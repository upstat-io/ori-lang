#![deny(unsafe_code)]
//! Type system for Ori.
//!
//! Provides the unified type system based on:
//! - [`Idx`]: 32-bit type index (THE canonical type handle)
//! - [`Tag`]: Type kind discriminant
//! - [`Item`]: Compact type storage (tag + data)
//! - [`TypeFlags`]: Pre-computed metadata for O(1) queries
//! - [`Pool`]: Unified type pool with interning
//!
//! Type-pool design principles:
//! - All types have Clone, Eq, Hash for Salsa compatibility
//! - Interned type representations for efficiency
//! - Flat structures for cache locality

mod check;
mod flags;
mod idx;
mod infer;
mod item;
mod lifetime;
mod output;
mod pool;
pub mod provenance;
mod registry;
pub mod reporting;
mod salsa_assertions;
mod tag;
pub mod triviality;
mod type_error;
mod unify;
mod value_category;

pub use check::validators::{build_exempt_var_ids, validate_body_types, validate_partial_move};
pub use check::{
    check_module, check_module_with_imports, check_module_with_pool, check_module_with_registries,
    ModuleChecker,
};
pub use flags::{TypeCategory, TypeFlags};
pub use idx::Idx;
pub use infer::{check_expr, infer_expr, resolve_parsed_type, ExprIndex, InferEngine, TypeEnv};
pub use item::Item;
pub use lifetime::LifetimeId;
pub use ori_ir::{PatternKey, PatternResolution};
pub use output::{
    AssignDesugar, ConstParamInfo, ConstValue, DeferredMonoCall, DeferredVarBinding, EffectClass,
    ExportedTypeMetadata, FnWhereClause, FormatSpecTypes, FunctionSig, GenericArg, MonoInstance,
    MonoInstanceId, TypeCheckResult, TypedModule,
};
pub use pool::{
    build_mono_body_type_map, extend_var_subst_with_roots, extract_var_from_types, re_intern_sig,
    re_intern_sig_with_var_remap, re_intern_type, re_intern_type_with_var_remap,
    substitute_in_pool, walk_collection_types, BodyTypeMapSink, EnumVariant, Pool, TypeDescriptor,
    VarState, VariantDescriptor, DEFAULT_RANK,
};
pub use provenance::{
    ConsumerEdge, GenericLeafDivergence, MonoEdge, ProvenanceDag, ResolutionEdge, StructureEdge,
    MAX_DEPTH as PROVENANCE_MAX_DEPTH,
};
pub use registry::burden;
pub use registry::burden_compose;
pub use registry::{
    BoundChainLookup,
    // Type registry
    FieldDef,
    GenericParamMeta,
    // Trait registry
    ImplEntry,
    ImplMethodDef,
    ImplSpecificity,
    MethodLookup,
    MethodLookupResult,
    // Method registry
    MethodRegistry,
    ObjectSafetyViolation,
    StructDef,
    TraitAssocTypeDef,
    TraitEntry,
    TraitMethodDef,
    TraitRegistry,
    TypeEntry,
    TypeKind,
    TypeRegistry,
    VariantDef,
    VariantFields,
    Visibility,
    WhereConstraint,
};
pub use reporting::{render_type_errors, TypeErrorRenderer};
pub use tag::Tag;
pub use triviality::{classify_triviality, Triviality};
pub use type_error::{
    diff_types, edit_distance, find_closest_field, suggest_field_typo, AmbiguousTypeSite,
    ArityMismatchKind, ContextKind, ErrorContext, Expected, ExpectedOrigin, ImportErrorKind,
    NonCollectingLoopKind, OrBindingMismatchReason, SequenceKind, Severity, TypeCheckError,
    TypeCheckWarning, TypeCheckWarningKind, TypeErrorKind, TypeProblem, VoidLoopKind,
};
pub use unify::{ArityKind, Rank, UnifyContext, UnifyEngine, UnifyError};
pub use value_category::ValueCategory;
