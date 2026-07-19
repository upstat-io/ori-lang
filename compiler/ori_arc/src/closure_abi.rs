//! Frozen backend-neutral closure-call adaptation facts.
//!
//! Residual indirect calls have one convention: the caller retains ownership
//! of every explicit argument. A closure adapter bridges that borrowed
//! convention to the concrete target signature. This module is the single
//! owner of the bridge decision and of the logical ownership-credit topology;
//! physical projections map the frozen topology through their own layouts.

use std::fmt;
use std::hash::BuildHasher;

use ori_ir::Name;
use ori_types::{Idx, Pool, Tag, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::aims::contract::{CalleeOwnerDemand, MemoryContract};
use crate::ir::{ArcFunction, ArcInstr, CtorKind};

/// Stable index into a [`RetainPlanTable`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RetainPlanId(u32);

impl RetainPlanId {
    /// Construct an unvalidated table identity. Executable artifact closure
    /// validates bounds and graph closure before any projection can consume it.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// Return the stable serialized identity without host-width conversion.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Return this plan's zero-based table index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    fn from_index(index: usize) -> Result<Self, ClosureAbiError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ClosureAbiError::TooManyRetainPlans { count: index })
    }
}

/// One logical owned-field edge in a retain topology.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RetainPlanEdge {
    /// Logical field index. Physical field offsets are deliberately absent.
    pub field: u32,
    /// Retain topology for the field's value.
    pub child: RetainPlanId,
}

/// Backend-neutral way to create one additional logical owner.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RetainPlanKind {
    /// Credit the value's own shared identity once. A projection decides where
    /// that identity lives (collection allocation, string data, closure env,
    /// boxed recursive aggregate, and so on).
    SelfOwnedIdentity,
    /// Credit the listed logical fields of an inline product.
    OwnedFields(Box<[RetainPlanEdge]>),
    /// Select the active logical variant, then credit its listed fields.
    OwnedVariants(Box<[Box<[RetainPlanEdge]>]>),
}

/// One closed node in the logical retain-plan graph.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RetainPlanNode {
    /// Semantic type whose ownership topology this node describes.
    pub ty: Idx,
    /// Logical ownership-credit operation.
    pub kind: RetainPlanKind,
}

/// Closed, deterministic retain-plan graph shared by executable projections.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RetainPlanTable {
    nodes: Box<[RetainPlanNode]>,
}

impl RetainPlanTable {
    /// Construct an unvalidated logical graph for transport into executable
    /// artifact closure. The transport owner must validate bounds, acyclicity,
    /// ordering, and reachability.
    #[must_use]
    pub fn from_nodes(nodes: Vec<RetainPlanNode>) -> Self {
        Self {
            nodes: nodes.into_boxed_slice(),
        }
    }

    /// Return all nodes in stable ID order.
    #[must_use]
    pub fn nodes(&self) -> &[RetainPlanNode] {
        &self.nodes
    }

    /// Resolve a stable logical plan identity.
    #[must_use]
    pub fn get(&self, id: RetainPlanId) -> Option<&RetainPlanNode> {
        self.nodes.get(id.index())
    }
}

/// Where an adapter obtains a target parameter value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClosureAdapterSource {
    /// The value is stored in the closure environment, which remains owned by
    /// the closure while the target invocation is active.
    EnvironmentCapture,
    /// The value is supplied by an indirect caller under the borrowed
    /// residual-call convention.
    BorrowedCallArgument,
}

/// Exact logical ownership action before invoking the concrete target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClosureAdapterAction {
    /// The concrete target also borrows the value.
    Borrow,
    /// The concrete target owns a trivially copyable value.
    Copy,
    /// The concrete target needs one independent owner, created by the named
    /// backend-neutral retain topology.
    Retain(RetainPlanId),
}

/// One target parameter in a frozen closure adapter signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ClosureAdapterSlot {
    pub source: ClosureAdapterSource,
    pub ty: Idx,
    pub demand: CalleeOwnerDemand,
    pub action: ClosureAdapterAction,
}

/// Complete target signature needed by a closure adapter.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosureAdapterPlan {
    capture_count: usize,
    slots: Box<[ClosureAdapterSlot]>,
}

/// Backend-neutral semantic signature of a function-typed SSA value.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClosureValueSignature {
    ty: Idx,
    parameters: Box<[Idx]>,
    result: Idx,
}

impl ClosureValueSignature {
    /// Construct an unvalidated callable signature for artifact transport.
    /// Executable-program closure validates its exact type and use-site
    /// relationships before a backend can consume it.
    #[must_use]
    pub fn from_parts(ty: Idx, parameters: Vec<Idx>, result: Idx) -> Self {
        Self {
            ty,
            parameters: parameters.into_boxed_slice(),
            result,
        }
    }

    /// Exact function-type identity carried by the SSA register.
    #[must_use]
    pub const fn ty(&self) -> Idx {
        self.ty
    }

    /// Explicit residual parameters in call order.
    #[must_use]
    pub fn parameters(&self) -> &[Idx] {
        &self.parameters
    }

    /// Number of explicit residual arguments.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.parameters.len()
    }

    /// Semantic result type.
    #[must_use]
    pub const fn result(&self) -> Idx {
        self.result
    }
}

/// Function-local callable facts parallel to `ArcFunction::var_types`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionCallableFacts {
    register_signatures: Box<[Option<ClosureValueSignature>]>,
}

impl FunctionCallableFacts {
    /// Construct unvalidated register-parallel callable facts for artifact
    /// transport.
    #[must_use]
    pub fn from_register_signatures(
        register_signatures: Vec<Option<ClosureValueSignature>>,
    ) -> Self {
        Self {
            register_signatures: register_signatures.into_boxed_slice(),
        }
    }

    /// Signature facts for every SSA register, including explicit `None`
    /// entries for non-callable values.
    #[must_use]
    pub fn register_signatures(&self) -> &[Option<ClosureValueSignature>] {
        &self.register_signatures
    }

    /// Resolve the signature of one register.
    #[must_use]
    pub fn signature(&self, register: crate::ArcVarId) -> Option<&ClosureValueSignature> {
        self.register_signatures
            .get(register.index())
            .and_then(Option::as_ref)
    }
}

impl ClosureAdapterPlan {
    /// Construct an unvalidated adapter fact. The executable artifact validates
    /// it against the realized target before a physical projection can consume
    /// it.
    #[must_use]
    pub fn from_slots(capture_count: usize, slots: Vec<ClosureAdapterSlot>) -> Self {
        Self {
            capture_count,
            slots: slots.into_boxed_slice(),
        }
    }

    /// Number of values loaded from the closure environment.
    #[must_use]
    pub const fn capture_count(&self) -> usize {
        self.capture_count
    }

    /// Number of explicit arguments accepted by the residual closure.
    #[must_use]
    pub fn explicit_arity(&self) -> usize {
        self.slots.len() - self.capture_count
    }

    /// Target parameters in concrete call order (captures, then explicit
    /// residual arguments).
    #[must_use]
    pub fn slots(&self) -> &[ClosureAdapterSlot] {
        &self.slots
    }

    /// Whether bypassing the adapter would omit a required logical retain.
    #[must_use]
    pub fn requires_retain(&self) -> bool {
        self.slots
            .iter()
            .any(|slot| matches!(slot.action, ClosureAdapterAction::Retain(_)))
    }
}

/// Complete shared output of closure ABI freezing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrozenClosureAdapters {
    pub adapters: FxHashMap<Name, ClosureAdapterPlan>,
    pub retain_plans: RetainPlanTable,
    pub callable_facts: FxHashMap<Name, FunctionCallableFacts>,
}

/// Failure to freeze a total, executable closure adapter signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClosureAbiError {
    MissingTarget {
        target: Name,
    },
    CaptureArityMismatch {
        target: Name,
        expected: usize,
        actual: usize,
    },
    CaptureTypeMismatch {
        target: Name,
        capture: usize,
        expected: Idx,
        actual: Idx,
    },
    InvalidCaptureCount {
        target: Name,
        captures: usize,
        parameters: usize,
    },
    ParameterMetadataMissing {
        target: Name,
        parameter: usize,
    },
    OwnedParameterNotShareable {
        target: Name,
        parameter: usize,
        ty: Idx,
        reason: &'static str,
    },
    MissingContract {
        target: Name,
    },
    ContractArityMismatch {
        target: Name,
        parameters: usize,
        contract_parameters: usize,
    },
    TooManyRetainPlans {
        count: usize,
    },
}

impl fmt::Display for ClosureAbiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTarget { target } => write!(
                f,
                "closure target name_id={} is absent from the realized program",
                target.raw()
            ),
            Self::CaptureArityMismatch {
                target,
                expected,
                actual,
            } => write!(
                f,
                "closure target name_id={} requires {expected} captures, but a closure site supplies {actual}",
                target.raw()
            ),
            Self::CaptureTypeMismatch {
                target,
                capture,
                expected,
                actual,
            } => write!(
                f,
                "closure target name_id={} capture {capture} has type idx {}, but the target prefix requires type idx {}",
                target.raw(),
                actual.raw(),
                expected.raw()
            ),
            Self::InvalidCaptureCount {
                target,
                captures,
                parameters,
            } => write!(
                f,
                "closure target name_id={} declares {captures} captures but has only {parameters} parameters",
                target.raw()
            ),
            Self::ParameterMetadataMissing { target, parameter } => write!(
                f,
                "closure target name_id={} parameter {parameter} has no realized variable metadata",
                target.raw()
            ),
            Self::OwnedParameterNotShareable {
                target,
                parameter,
                ty,
                reason,
            } => write!(
                f,
                "closure target name_id={} owned parameter {parameter} (type idx {}) cannot cross the borrowed closure ABI: {reason}",
                target.raw(),
                ty.raw()
            ),
            Self::MissingContract { target } => write!(
                f,
                "closure target name_id={} has no final memory contract",
                target.raw()
            ),
            Self::ContractArityMismatch {
                target,
                parameters,
                contract_parameters,
            } => write!(
                f,
                "closure target name_id={} has {parameters} parameters but {contract_parameters} final contract entries",
                target.raw()
            ),
            Self::TooManyRetainPlans { count } => {
                write!(f, "closure retain topology contains too many nodes: {count}")
            }
        }
    }
}

impl std::error::Error for ClosureAbiError {}

/// Freeze adapter plans for exactly the functions used as closure targets.
///
/// Every closure site is checked against the target's capture prefix. Unknown
/// targets and affine owned slots are errors rather than projection-specific
/// fallbacks.
pub fn freeze_closure_adapter_plans<S: BuildHasher>(
    functions: &[ArcFunction],
    contracts: &std::collections::HashMap<Name, MemoryContract, S>,
    pool: &Pool,
    type_registry: &TypeRegistry,
) -> Result<FrozenClosureAdapters, Vec<ClosureAbiError>> {
    let callable_facts = freeze_function_callable_facts(functions, pool);
    let by_name: FxHashMap<Name, &ArcFunction> = functions
        .iter()
        .map(|function| (function.name, function))
        .collect();
    let mut targets = Vec::new();
    let mut errors = Vec::new();

    for function in functions {
        for block in &function.blocks {
            for instruction in &block.body {
                let site = match instruction {
                    ArcInstr::PartialApply { func, args, .. }
                    | ArcInstr::Construct {
                        ctor: CtorKind::Closure { func },
                        args,
                        ..
                    } => Some((*func, args.as_slice())),
                    _ => None,
                };
                let Some((target, capture_args)) = site else {
                    continue;
                };
                let Some(target_function) = by_name.get(&target) else {
                    errors.push(ClosureAbiError::MissingTarget { target });
                    continue;
                };
                if target_function.num_captures != capture_args.len() {
                    errors.push(ClosureAbiError::CaptureArityMismatch {
                        target,
                        expected: target_function.num_captures,
                        actual: capture_args.len(),
                    });
                    continue;
                }
                validate_capture_types(function, target_function, capture_args, &mut errors);
                targets.push(target);
            }
        }
    }

    targets.sort_unstable_by_key(|name| name.raw());
    targets.dedup();
    let mut builder = RetainPlanBuilder::new(pool, type_registry);
    let mut adapters = FxHashMap::default();
    for target in targets {
        let function = by_name[&target];
        let Some(contract) = contracts.get(&target) else {
            errors.push(ClosureAbiError::MissingContract { target });
            continue;
        };
        match freeze_closure_adapter_plan(function, contract, &mut builder) {
            Ok(plan) => {
                adapters.insert(target, plan);
            }
            Err(error) => errors.push(error),
        }
    }

    if errors.is_empty() {
        Ok(FrozenClosureAdapters {
            adapters,
            retain_plans: builder.finish(),
            callable_facts,
        })
    } else {
        Err(errors)
    }
}

/// Freeze exact callable signatures for every SSA register in every function.
///
/// The type pool is consulted once at the semantic owner. Executable artifact
/// closure and physical backends consume only these closed facts.
#[must_use]
pub fn freeze_function_callable_facts(
    functions: &[ArcFunction],
    pool: &Pool,
) -> FxHashMap<Name, FunctionCallableFacts> {
    functions
        .iter()
        .map(|function| {
            let register_signatures = function
                .var_types
                .iter()
                .map(|&ty| closure_signature(pool, ty))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            (
                function.name,
                FunctionCallableFacts {
                    register_signatures,
                },
            )
        })
        .collect()
}

fn closure_signature(pool: &Pool, ty: Idx) -> Option<ClosureValueSignature> {
    let resolved = pool.resolve_fully(ty);
    if resolved.raw() as usize >= pool.len() || pool.tag(resolved) != Tag::Function {
        return None;
    }
    Some(ClosureValueSignature {
        ty,
        parameters: pool.function_params(resolved).into_boxed_slice(),
        result: pool.function_return(resolved),
    })
}

fn validate_capture_types(
    enclosing: &ArcFunction,
    target: &ArcFunction,
    capture_args: &[crate::ArcVarId],
    errors: &mut Vec<ClosureAbiError>,
) {
    for (capture, (&argument, parameter)) in capture_args.iter().zip(&target.params).enumerate() {
        let Some(&actual) = enclosing.var_types.get(argument.index()) else {
            errors.push(ClosureAbiError::CaptureTypeMismatch {
                target: target.name,
                capture,
                expected: parameter.ty,
                actual: Idx::NONE,
            });
            continue;
        };
        if actual != parameter.ty {
            errors.push(ClosureAbiError::CaptureTypeMismatch {
                target: target.name,
                capture,
                expected: parameter.ty,
                actual,
            });
        }
    }
}

fn freeze_closure_adapter_plan(
    function: &ArcFunction,
    contract: &MemoryContract,
    builder: &mut RetainPlanBuilder<'_>,
) -> Result<ClosureAdapterPlan, ClosureAbiError> {
    if function.num_captures > function.params.len() {
        return Err(ClosureAbiError::InvalidCaptureCount {
            target: function.name,
            captures: function.num_captures,
            parameters: function.params.len(),
        });
    }
    if contract.params.len() != function.params.len() {
        return Err(ClosureAbiError::ContractArityMismatch {
            target: function.name,
            parameters: function.params.len(),
            contract_parameters: contract.params.len(),
        });
    }

    let mut slots = Vec::with_capacity(function.params.len());
    for (parameter, (param, param_contract)) in
        function.params.iter().zip(&contract.params).enumerate()
    {
        if function.var_types.get(param.var.index()) != Some(&param.ty) {
            return Err(ClosureAbiError::ParameterMetadataMissing {
                target: function.name,
                parameter,
            });
        }
        let source = if parameter < function.num_captures {
            ClosureAdapterSource::EnvironmentCapture
        } else {
            ClosureAdapterSource::BorrowedCallArgument
        };
        let demand = param_contract.callee_owner_demand();
        let action = match demand {
            CalleeOwnerDemand::Borrow => ClosureAdapterAction::Borrow,
            CalleeOwnerDemand::WholeValue => match builder.duplication_for(param.ty) {
                Ok(Duplication::Copy) => ClosureAdapterAction::Copy,
                Ok(Duplication::Retain(plan)) => ClosureAdapterAction::Retain(plan),
                Err(reason) => {
                    return Err(ClosureAbiError::OwnedParameterNotShareable {
                        target: function.name,
                        parameter,
                        ty: param.ty,
                        reason,
                    });
                }
            },
        };
        slots.push(ClosureAdapterSlot {
            source,
            ty: param.ty,
            demand,
            action,
        });
    }

    Ok(ClosureAdapterPlan {
        capture_count: function.num_captures,
        slots: slots.into_boxed_slice(),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Duplication {
    Copy,
    Retain(RetainPlanId),
}

struct RetainPlanBuilder<'a> {
    pool: &'a Pool,
    type_registry: &'a TypeRegistry,
    nodes: Vec<RetainPlanNode>,
    interned: FxHashMap<RetainPlanNode, RetainPlanId>,
    visiting: FxHashSet<Idx>,
}

impl<'a> RetainPlanBuilder<'a> {
    fn new(pool: &'a Pool, type_registry: &'a TypeRegistry) -> Self {
        Self {
            pool,
            type_registry,
            nodes: Vec::new(),
            interned: FxHashMap::default(),
            visiting: FxHashSet::default(),
        }
    }

    fn finish(self) -> RetainPlanTable {
        RetainPlanTable {
            nodes: self.nodes.into_boxed_slice(),
        }
    }

    fn duplication_for(&mut self, ty: Idx) -> Result<Duplication, &'static str> {
        // Retain `ty` as the frozen semantic identity. Resolution supplies the
        // topology view only; it may cross a nominal-to-layout boundary.
        let resolved = self.pool.resolve_fully(ty);
        if resolved == Idx::NONE || resolved.raw() as usize >= self.pool.len() {
            return Err("the parameter type is unresolved");
        }
        if crate::lower::type_has_user_drop(ty, self.type_registry)
            || crate::lower::type_has_user_drop(resolved, self.type_registry)
        {
            return Err("the type has user-defined drop behavior");
        }

        let tag = self.pool.tag(resolved);
        if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
            return Err("iterators are affine and have no duplication operation");
        }
        if matches!(tag, Tag::Struct | Tag::Enum) && self.pool.aggregate_type_is_recursive(resolved)
        {
            return self
                .intern(ty, RetainPlanKind::SelfOwnedIdentity)
                .map(Duplication::Retain)
                .map_err(|_| "the retain-plan table exceeds its stable identity range");
        }

        if !self.visiting.insert(resolved) {
            return Err("the inline ownership topology is cyclic");
        }
        let duplication = self.duplication_for_resolved(ty, resolved, tag);
        self.visiting.remove(&resolved);
        duplication
    }

    fn duplication_for_resolved(
        &mut self,
        identity: Idx,
        resolved: Idx,
        tag: Tag,
    ) -> Result<Duplication, &'static str> {
        match tag {
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => Ok(Duplication::Copy),
            Tag::Str | Tag::List | Tag::Map | Tag::Set | Tag::Channel | Tag::Function => self
                .intern(identity, RetainPlanKind::SelfOwnedIdentity)
                .map(Duplication::Retain)
                .map_err(|_| "the retain-plan table exceeds its stable identity range"),
            Tag::Tuple => {
                let fields = self.pool.tuple_elems(resolved);
                self.product_duplication(identity, fields)
            }
            Tag::Struct => {
                let fields = self
                    .pool
                    .struct_fields(resolved)
                    .into_iter()
                    .map(|(_, field)| field)
                    .collect();
                self.product_duplication(identity, fields)
            }
            Tag::Option => self.variant_duplication(
                identity,
                vec![vec![self.pool.option_inner(resolved)], Vec::new()],
            ),
            Tag::Result => self.variant_duplication(
                identity,
                vec![
                    vec![self.pool.result_ok(resolved)],
                    vec![self.pool.result_err(resolved)],
                ],
            ),
            Tag::Enum => {
                let variants = self
                    .pool
                    .enum_variants(resolved)
                    .into_iter()
                    .map(|(_, fields)| fields)
                    .collect();
                self.variant_duplication(identity, variants)
            }
            Tag::Range => match self.duplication_for(self.pool.range_elem(resolved))? {
                Duplication::Copy => Ok(Duplication::Copy),
                Duplication::Retain(_) => {
                    Err("a range with ownership-bearing endpoints has no frozen retain topology")
                }
            },
            Tag::Iterator | Tag::DoubleEndedIterator => {
                Err("iterators are affine and have no duplication operation")
            }
            Tag::Error
            | Tag::Borrowed
            | Tag::Named
            | Tag::Applied
            | Tag::Alias
            | Tag::Var
            | Tag::BoundVar
            | Tag::RigidVar
            | Tag::Scheme
            | Tag::Projection
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => Err("the parameter type is unresolved or not duplicable"),
        }
    }

    fn product_duplication(
        &mut self,
        ty: Idx,
        fields: Vec<Idx>,
    ) -> Result<Duplication, &'static str> {
        let edges = self.field_edges(fields)?;
        if edges.is_empty() {
            Ok(Duplication::Copy)
        } else {
            self.intern(ty, RetainPlanKind::OwnedFields(edges.into_boxed_slice()))
                .map(Duplication::Retain)
                .map_err(|_| "the retain-plan table exceeds its stable identity range")
        }
    }

    fn variant_duplication(
        &mut self,
        ty: Idx,
        variants: Vec<Vec<Idx>>,
    ) -> Result<Duplication, &'static str> {
        let mut any_retain = false;
        let mut plans = Vec::with_capacity(variants.len());
        for fields in variants {
            let edges = self.field_edges(fields)?;
            any_retain |= !edges.is_empty();
            plans.push(edges.into_boxed_slice());
        }
        if any_retain {
            self.intern(ty, RetainPlanKind::OwnedVariants(plans.into_boxed_slice()))
                .map(Duplication::Retain)
                .map_err(|_| "the retain-plan table exceeds its stable identity range")
        } else {
            Ok(Duplication::Copy)
        }
    }

    fn field_edges(&mut self, fields: Vec<Idx>) -> Result<Vec<RetainPlanEdge>, &'static str> {
        let mut edges = Vec::new();
        for (field, field_ty) in fields.into_iter().enumerate() {
            if let Duplication::Retain(child) = self.duplication_for(field_ty)? {
                let field =
                    u32::try_from(field).map_err(|_| "an aggregate has too many logical fields")?;
                edges.push(RetainPlanEdge { field, child });
            }
        }
        Ok(edges)
    }

    fn intern(&mut self, ty: Idx, kind: RetainPlanKind) -> Result<RetainPlanId, ClosureAbiError> {
        let node = RetainPlanNode { ty, kind };
        if let Some(&id) = self.interned.get(&node) {
            return Ok(id);
        }
        let id = RetainPlanId::from_index(self.nodes.len())?;
        self.nodes.push(node.clone());
        self.interned.insert(node, id);
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aims::lattice::AccessClass;
    use crate::ir::{
        ArcBlock, ArcBlockId, ArcParam, ArcTerminator, ArcVarId, RcStrategy, ValueRepr,
        VariableMetadataState,
    };
    use crate::Ownership;

    fn realized_target(
        name: Name,
        params: &[(Idx, Ownership, ValueRepr, Option<RcStrategy>)],
        captures: usize,
    ) -> ArcFunction {
        let arc_params = params
            .iter()
            .enumerate()
            .map(|(index, (ty, ownership, _, _))| {
                let Ok(index) = u32::try_from(index) else {
                    panic!("test target has more parameters than ArcVarId can represent")
                };
                ArcParam {
                    var: ArcVarId::new(index),
                    ty: *ty,
                    ownership: *ownership,
                }
            })
            .collect();
        ArcFunction {
            name,
            params: arc_params,
            var_types: params.iter().map(|entry| entry.0).collect(),
            var_reprs: params.iter().map(|entry| entry.2).collect(),
            var_rc_strategies: params.iter().map(|entry| entry.3).collect(),
            var_metadata_state: VariableMetadataState::Realized,
            num_captures: captures,
            ..ArcFunction::default()
        }
    }

    fn caller(target: Name, captures: &[(ArcVarId, Idx)]) -> ArcFunction {
        let Ok(destination) = u32::try_from(captures.len()) else {
            panic!("test closure has more captures than ArcVarId can represent")
        };
        ArcFunction {
            name: Name::from_raw(99),
            blocks: vec![ArcBlock {
                id: ArcBlockId::new(0),
                params: Vec::new(),
                body: vec![ArcInstr::PartialApply {
                    dst: ArcVarId::new(destination),
                    ty: Idx::NONE,
                    func: target,
                    args: captures.iter().map(|entry| entry.0).collect(),
                }],
                terminator: ArcTerminator::Unreachable,
            }],
            var_types: captures.iter().map(|entry| entry.1).collect(),
            ..ArcFunction::default()
        }
    }

    fn contract_for(target: &ArcFunction) -> MemoryContract {
        let mut contract = MemoryContract::conservative(target.params.len());
        for (param, facts) in target.params.iter().zip(&mut contract.params) {
            facts.access = match param.ownership {
                Ownership::Owned => AccessClass::Owned,
                Ownership::Borrowed => AccessClass::Borrowed,
            };
            facts.borrowed_cow_consumed = false;
            facts.borrowed_cow_mutated = false;
            facts.iter_consumes = false;
        }
        contract
    }

    fn contract_map(target: Name, contract: MemoryContract) -> FxHashMap<Name, MemoryContract> {
        [(target, contract)].into_iter().collect()
    }

    fn freeze_successfully(
        functions: &[ArcFunction],
        contracts: &FxHashMap<Name, MemoryContract>,
        pool: &Pool,
        registry: &TypeRegistry,
    ) -> FrozenClosureAdapters {
        match freeze_closure_adapter_plans(functions, contracts, pool, registry) {
            Ok(frozen) => frozen,
            Err(errors) => panic!("test closure adapter facts must freeze: {errors:?}"),
        }
    }

    #[test]
    fn plan_adapts_captures_and_explicit_arguments_through_logical_ids() {
        let pool = Pool::new();
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(10);
        let target = realized_target(
            target_name,
            &[
                (
                    Idx::STR,
                    Ownership::Owned,
                    ValueRepr::FatValue,
                    Some(RcStrategy::FatPointer),
                ),
                (Idx::INT, Ownership::Owned, ValueRepr::Scalar, None),
                (
                    Idx::STR,
                    Ownership::Borrowed,
                    ValueRepr::FatValue,
                    Some(RcStrategy::FatPointer),
                ),
            ],
            1,
        );
        let caller = caller(target_name, &[(ArcVarId::new(0), Idx::STR)]);
        let contracts = contract_map(target_name, contract_for(&target));

        let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
        let plan = &frozen.adapters[&target_name];
        assert_eq!(plan.capture_count(), 1);
        assert_eq!(plan.explicit_arity(), 2);
        let ClosureAdapterAction::Retain(id) = plan.slots()[0].action else {
            panic!("owned str must retain through a logical plan")
        };
        assert!(matches!(
            frozen.retain_plans.get(id).map(|node| &node.kind),
            Some(RetainPlanKind::SelfOwnedIdentity)
        ));
        assert_eq!(plan.slots()[1].action, ClosureAdapterAction::Copy);
        assert_eq!(plan.slots()[2].action, ClosureAdapterAction::Borrow);
    }

    #[test]
    fn retain_plan_root_preserves_nominal_adapter_slot_identity() {
        let mut pool = Pool::new();
        let bundle_name = Name::from_raw(30);
        let items_name = Name::from_raw(31);
        let nominal_bundle = pool.named(bundle_name);
        let list_str = pool.list(Idx::STR);
        let bundle_body = pool.struct_type(bundle_name, &[(items_name, list_str)]);
        pool.set_resolution(nominal_bundle, bundle_body);
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(32);
        let target = realized_target(
            target_name,
            &[(
                nominal_bundle,
                Ownership::Owned,
                ValueRepr::Aggregate,
                Some(RcStrategy::AggregateFields),
            )],
            0,
        );
        let caller = caller(target_name, &[]);
        let contracts = contract_map(target_name, contract_for(&target));

        let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
        let slot = frozen.adapters[&target_name].slots()[0];
        let ClosureAdapterAction::Retain(root) = slot.action else {
            panic!("owned Bundle must retain its nested list")
        };
        let Some(root_node) = frozen.retain_plans.get(root) else {
            panic!("Bundle retain root must exist")
        };

        assert_ne!(nominal_bundle, bundle_body);
        assert_eq!(slot.ty, nominal_bundle);
        assert_eq!(root_node.ty, nominal_bundle);
        assert!(matches!(root_node.kind, RetainPlanKind::OwnedFields(_)));
    }

    #[test]
    fn owned_iterator_is_rejected_before_physical_projection() {
        let mut pool = Pool::new();
        let iterator = pool.iterator(Idx::INT);
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(11);
        let target = realized_target(
            target_name,
            &[(
                iterator,
                Ownership::Owned,
                ValueRepr::RcPointer,
                Some(RcStrategy::Iterator),
            )],
            0,
        );
        let caller = caller(target_name, &[]);
        let contracts = contract_map(target_name, contract_for(&target));

        assert!(matches!(
            freeze_closure_adapter_plans(&[caller, target], &contracts, &pool, &registry),
            Err(errors)
                if matches!(errors.as_slice(), [ClosureAbiError::OwnedParameterNotShareable { parameter: 0, .. }])
        ));
    }

    #[test]
    fn inline_variant_retains_exact_active_payload_topology() {
        let mut pool = Pool::new();
        let option_str = pool.option(Idx::STR);
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(12);
        let target = realized_target(
            target_name,
            &[(
                option_str,
                Ownership::Owned,
                ValueRepr::Aggregate,
                Some(RcStrategy::InlineEnum),
            )],
            0,
        );
        let caller = caller(target_name, &[]);
        let contracts = contract_map(target_name, contract_for(&target));

        let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
        let ClosureAdapterAction::Retain(root) = frozen.adapters[&target_name].slots()[0].action
        else {
            panic!("Option<str> must retain its active payload")
        };
        let Some(RetainPlanNode {
            kind: RetainPlanKind::OwnedVariants(variants),
            ..
        }) = frozen.retain_plans.get(root)
        else {
            panic!("Option<str> needs a variant-aware root")
        };
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].len(), 1);
        assert!(variants[1].is_empty());
    }

    #[test]
    fn zero_capture_copy_and_borrow_signature_needs_no_wrapper_retain() {
        let pool = Pool::new();
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(13);
        let target = realized_target(
            target_name,
            &[
                (Idx::INT, Ownership::Owned, ValueRepr::Scalar, None),
                (
                    Idx::STR,
                    Ownership::Borrowed,
                    ValueRepr::FatValue,
                    Some(RcStrategy::FatPointer),
                ),
            ],
            0,
        );
        let caller = caller(target_name, &[]);
        let contracts = contract_map(target_name, contract_for(&target));

        let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
        assert!(!frozen.adapters[&target_name].requires_retain());
    }

    #[test]
    fn batch_freezer_rejects_capture_count_drift() {
        let pool = Pool::new();
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(14);
        let target = realized_target(
            target_name,
            &[(Idx::INT, Ownership::Owned, ValueRepr::Scalar, None)],
            1,
        );
        let caller = caller(target_name, &[]);
        let contracts = contract_map(target_name, contract_for(&target));

        assert!(matches!(
            freeze_closure_adapter_plans(&[caller, target], &contracts, &pool, &registry),
            Err(errors)
                if matches!(errors.as_slice(), [ClosureAbiError::CaptureArityMismatch { .. }])
        ));
    }

    #[test]
    fn borrowed_iter_consume_demands_a_whole_value_credit() {
        let mut pool = Pool::new();
        let list = pool.list(Idx::INT);
        let registry = TypeRegistry::new();
        let target_name = Name::from_raw(15);
        let target = realized_target(
            target_name,
            &[(
                list,
                Ownership::Borrowed,
                ValueRepr::RcPointer,
                Some(RcStrategy::HeapPointer),
            )],
            0,
        );
        let caller = caller(target_name, &[]);
        let mut contract = contract_for(&target);
        contract.params[0].iter_consumes = true;
        let contracts = contract_map(target_name, contract);

        let frozen = freeze_successfully(&[caller, target], &contracts, &pool, &registry);
        let slot = frozen.adapters[&target_name].slots()[0];
        assert_eq!(slot.demand, CalleeOwnerDemand::WholeValue);
        assert!(matches!(slot.action, ClosureAdapterAction::Retain(_)));
    }
}
