//! Backend-neutral monomorphization metadata and call-target realization.
//!
//! Transforms [`MonoInstance`] records (from the type checker) into
//! [`MonoFunction`] values ready for any executable backend. Each `MonoFunction`
//! carries a mangled name and a fully-concrete [`FunctionSig`] — existing
//! `declare_function()` / `define_function_body()` work unchanged.

mod collect;
mod derived_mono;
mod mangle;
mod targets;

use rustc_hash::FxHashMap;

use ori_ir::canon::{CanId, CanonResult, MonoInstanceId};
use ori_ir::{DerivedImplId, Name};
use ori_types::{
    FunctionSig, GenericArg, Idx, ImplMethodId, MethodProducer, MonoConstBinding, MonoInstance,
    Pool, Tag,
};

use crate::executable::{
    validate_external_callables, ExternalCallable, ExternalCallableMetadata, RealizationError,
};

pub use collect::collect_mono_functions;
pub use derived_mono::{materialize_derived_mono_for_receiver, DerivedMonoMaterializationError};
pub use mangle::mangle_mono_name;
pub use targets::{callee_shadows_builtin_method, rewrite_apply_targets_for_monos, MonoTargetMaps};

// ImportSig

/// A cross-module imported function's declared signature.
///
/// `name` is the call-site local/aliased [`Name`] (the `codegen_ctx.functions`
/// key `resolve_callee` probes); `symbol` is the exporting module's exact
/// mangled symbol (never re-mangled against the host module path).
#[derive(Clone, Debug)]
pub struct ImportSig {
    pub name: Name,
    pub symbol: String,
    pub sig: FunctionSig,
    /// Required final producer facts for ownership/effect realization.
    ///
    /// There is deliberately no absent or default state: an imported callable
    /// cannot participate in AIMS until its producer artifact has supplied
    /// this carrier.
    pub metadata: ExternalCallableMetadata,
}

impl ImportSig {
    fn external_callable(&self) -> ExternalCallable {
        ExternalCallable::from_imported_metadata(
            self.name,
            self.symbol.clone(),
            self.sig.param_types.clone(),
            self.sig.return_type,
            self.metadata.clone(),
        )
    }
}

/// Reconstruct and validate every imported callable before AIMS consumes its
/// contract.
///
/// The importer-pool signature is checked against the producer's stable
/// identity, including the exact link symbol. Duplicate aliases and aliases
/// that disagree about one producer symbol are rejected as one batch.
pub fn realize_imported_callables(
    imports: &[ImportSig],
    pool: &Pool,
) -> Result<Vec<ExternalCallable>, RealizationError> {
    let callables: Vec<_> = imports.iter().map(ImportSig::external_callable).collect();
    validate_external_callables(&callables, pool)?;
    Ok(callables)
}

// MonoFunction

/// Semantic source of a monomorphized function body.
///
/// Imported and local source declarations both remain [`Self::Source`];
/// [`MonoFunction::is_imported`] retains that transport distinction. Generated
/// derived methods carry the accepted type-checker identity so later phases can
/// join the specialization to its generated Canon root without rediscovering a
/// derive from source attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonoFunctionOrigin {
    /// An ordinary source declaration, whether local or imported.
    Source,
    /// Exact local source/default impl body selected by type checking.
    Impl(ImplMethodId),
    /// A compiler-generated method accepted by type checking and coherence.
    Derived(DerivedImplId),
}

/// Checker-issued identity coordinates retained by a realized mono function.
///
/// [`MonoInstance`] is the canonical owner of these facts. This projection
/// keeps only the coordinates needed after representation selection and can
/// be constructed only from a validated instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MonoFunctionIdentity {
    original_name: Name,
    method_producer: Option<MethodProducer>,
    method_args: Vec<GenericArg>,
    const_bindings: Vec<MonoConstBinding>,
    instance_ids: Vec<MonoInstanceId>,
    receiver_type: Option<Idx>,
}

impl MonoFunctionIdentity {
    /// Project one exact checker-issued instance into representation identity.
    #[must_use]
    pub fn new(instance: &MonoInstance, instance_id: MonoInstanceId) -> Self {
        Self::from((instance, instance_id))
    }

    /// Project a compiler-generated instance that has no source call-site id.
    #[must_use]
    pub fn generated(instance: &MonoInstance) -> Self {
        Self::from(instance)
    }

    fn from_instance(instance: &MonoInstance, instance_ids: Vec<MonoInstanceId>) -> Self {
        Self {
            original_name: instance.fn_name,
            method_producer: instance.method_producer.clone(),
            method_args: instance.method_args.clone(),
            const_bindings: instance.const_bindings.clone(),
            instance_ids,
            receiver_type: instance.receiver_type,
        }
    }

    /// Original generic callable name used for body and fallback lookup.
    #[must_use]
    pub fn original_name(&self) -> Name {
        self.original_name
    }

    /// Exact checker-selected producer for a method specialization.
    #[must_use]
    pub fn method_producer(&self) -> Option<&MethodProducer> {
        self.method_producer.as_ref()
    }

    /// Concrete method-binder arguments that distinguish this specialization.
    #[must_use]
    pub fn method_args(&self) -> &[GenericArg] {
        &self.method_args
    }

    /// Named const values injected while lowering the specialized body.
    #[must_use]
    pub fn const_bindings(&self) -> &[MonoConstBinding] {
        &self.const_bindings
    }

    /// Checker instance ids deduplicated to this realized function.
    #[must_use]
    pub fn instance_ids(&self) -> &[MonoInstanceId] {
        &self.instance_ids
    }

    /// Concrete receiver selected for a method specialization.
    #[must_use]
    pub fn receiver_type(&self) -> Option<Idx> {
        self.receiver_type
    }

    /// Record another checker instance deduplicated to the same mono function.
    pub fn push_instance_id(&mut self, instance_id: MonoInstanceId) {
        self.instance_ids.push(instance_id);
    }
}

impl From<&MonoInstance> for MonoFunctionIdentity {
    fn from(instance: &MonoInstance) -> Self {
        Self::from_instance(instance, Vec::new())
    }
}

impl From<(&MonoInstance, MonoInstanceId)> for MonoFunctionIdentity {
    fn from((instance, instance_id): (&MonoInstance, MonoInstanceId)) -> Self {
        Self::from_instance(instance, vec![instance_id])
    }
}

/// A monomorphized function ready for LLVM codegen.
///
/// Produced by [`collect_mono_functions`] from type-checker `MonoInstance` records.
/// The `mangled_name` is unique per (function, type args) combination.
#[derive(Clone, Debug)]
pub struct MonoFunction {
    /// Mangled name for the specialization (e.g., `identity$m$int`).
    pub mangled_name: Name,
    /// Semantic body provenance used to resolve the canonical body owner.
    pub origin: MonoFunctionOrigin,
    /// Checker-owned callable, producer, argument, binding, and receiver facts.
    pub identity: MonoFunctionIdentity,
    /// Concrete function signature (non-generic: empty `type_params`).
    pub sig: FunctionSig,
    /// Generic `Idx` → concrete `Idx` map for ARC lowering.
    pub body_type_map: FxHashMap<Idx, Idx>,
    /// True when this instance was resolved via the `import_sigs` lookup chain
    /// (i.e., the generic function is imported).
    ///
    /// Test-observable provenance marker: no production consumer branches on
    /// this field, and the body-import path does not branch on it — local and
    /// imported mono functions flow through identical `declare_mono_functions`
    /// + `prepare_mono_cached` plumbing.
    pub is_imported: bool,
    /// Nominal type name of the receiver for an inherent-method specialization
    /// (`Some("Box")` for `impl<T> Box<T> { @unwrap }`), else `None` for a
    /// free-function specialization. Selects the canon-body namespace: method
    /// bodies are keyed by `(type_name, method_name)` via `method_root_for`,
    /// free-function bodies by `root_for`.
    pub receiver_type_name: Option<Name>,
}

impl MonoFunction {
    /// Resolve the canonical body root for this specialization.
    ///
    /// Method specializations look up `method_root_for(type_name, name)` (the
    /// impl-method body namespace); free functions use `root_for(name)`.
    #[must_use]
    pub fn body_root(&self, canon: &CanonResult) -> CanId {
        match self.receiver_type_name {
            Some(type_name) => canon
                .method_root_for(type_name, self.identity.original_name())
                .or_else(|| canon.root_for(self.identity.original_name()))
                .unwrap_or(canon.root),
            None => canon
                .root_for(self.identity.original_name())
                .unwrap_or(canon.root),
        }
    }
}

/// Nominal type name of a (possibly generic-instantiated) user type, for the
/// impl-method body-namespace lookup. Returns `None` for non-nominal types.
fn nominal_type_name(pool: &Pool, idx: Idx) -> Option<Name> {
    let resolved = pool.resolve_fully(idx);
    match pool.tag(resolved) {
        Tag::Applied => Some(pool.applied_name(resolved)),
        Tag::Struct => Some(pool.struct_name(resolved)),
        Tag::Enum => Some(pool.enum_name(resolved)),
        Tag::Named => Some(pool.named_name(resolved)),
        _ => None,
    }
}

/// Build the concrete (non-generic) [`FunctionSig`] for one mono instance.
///
/// Copies non-generic metadata (param names, capabilities, defaults,
/// `is_fbip`, `required_params`) from `generic_sig`, substitutes the
/// instance's concrete param / return types plus their pool Merkle hashes,
/// and empties every generic-only field so `is_generic()` returns false.
/// Shared by [`collect_mono_functions`] (local monos) and the driver's
/// imported-mono builder (cross-module monos on the merged pool).
pub fn concrete_sig_for_instance(
    instance: &MonoInstance,
    generic_sig: &FunctionSig,
    pool: &Pool,
    mangled_name: Name,
) -> FunctionSig {
    // Typecheck instances omit `self`, but ARC method signatures retain it;
    // prepend the concrete receiver when that is the sole arity difference.
    let receiver_self = instance
        .receiver_type
        .filter(|_| instance.concrete_param_types.len() + 1 == generic_sig.param_types.len());
    let param_types: Vec<Idx> = match receiver_self {
        Some(recv) => {
            let mut pt = Vec::with_capacity(instance.concrete_param_types.len() + 1);
            pt.push(recv);
            pt.extend_from_slice(&instance.concrete_param_types);
            pt
        }
        None => instance.concrete_param_types.clone(),
    };
    let param_hashes: Vec<u64> = param_types.iter().map(|&idx| pool.hash(idx)).collect();
    let return_hash = pool.hash(instance.concrete_return_type);

    FunctionSig {
        name: mangled_name,
        type_params: vec![],
        const_params: vec![],
        param_names: generic_sig.param_names.clone(),
        param_types,
        return_type: instance.concrete_return_type,
        capabilities: generic_sig.capabilities.clone(),
        is_public: false, // mono specializations are internal
        is_test: false,
        is_main: false,
        is_fbip: generic_sig.is_fbip,
        type_param_bounds: vec![],
        where_clauses: vec![],
        generic_param_mapping: vec![],
        scheme_var_ids: vec![],
        required_params: generic_sig.required_params,
        param_defaults: generic_sig.param_defaults.clone(),
        param_hashes,
        return_hash,
        return_projection: None,
    }
}

#[cfg(test)]
mod tests;
