use ori_types::{Idx, Pool, Tag, TypeRegistry};
use rustc_hash::{FxHashMap, FxHashSet};

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

    fn from_index(index: usize) -> Result<Self, std::num::TryFromIntError> {
        u32::try_from(index).map(Self)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Duplication {
    Copy,
    Retain(RetainPlanId),
}

/// Why an owned closure argument has no total duplication operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DuplicationFailure {
    /// The parameter type has not reached a resolved executable form.
    UnresolvedType,
    /// The type's user-defined drop prevents a synthesized duplication plan.
    UserDefinedDrop,
    /// Iterator values are affine and cannot be duplicated.
    AffineIterator,
    /// The retain-plan table cannot represent another stable identity.
    RetainPlanIndexOverflow(std::num::TryFromIntError),
    /// Inline ownership traversal encountered an unsupported cycle.
    CyclicInlineTopology,
    /// A range endpoint carries ownership that has no frozen retain topology.
    OwnershipBearingRange,
    /// The resolved type has no supported duplication operation.
    NotDuplicable,
    /// An aggregate field index exceeds the stable edge representation.
    LogicalFieldIndexOverflow(std::num::TryFromIntError),
}

impl DuplicationFailure {
    pub(super) const fn reason(&self) -> &'static str {
        match self {
            Self::UnresolvedType => "the parameter type is unresolved",
            Self::UserDefinedDrop => "the type has user-defined drop behavior",
            Self::AffineIterator => "iterators are affine and have no duplication operation",
            Self::RetainPlanIndexOverflow(_) => {
                "the retain-plan table exceeds its stable identity range"
            }
            Self::CyclicInlineTopology => "the inline ownership topology is cyclic",
            Self::OwnershipBearingRange => {
                "a range with ownership-bearing endpoints has no frozen retain topology"
            }
            Self::NotDuplicable => "the parameter type is unresolved or not duplicable",
            Self::LogicalFieldIndexOverflow(_) => "an aggregate has too many logical fields",
        }
    }

    pub(super) const fn conversion_source(&self) -> Option<&std::num::TryFromIntError> {
        match self {
            Self::RetainPlanIndexOverflow(source) | Self::LogicalFieldIndexOverflow(source) => {
                Some(source)
            }
            _ => None,
        }
    }
}

pub(super) struct RetainPlanBuilder<'a> {
    pool: &'a Pool,
    type_registry: &'a TypeRegistry,
    nodes: Vec<RetainPlanNode>,
    interned: FxHashMap<RetainPlanNode, RetainPlanId>,
    visiting: FxHashSet<Idx>,
}

impl<'a> RetainPlanBuilder<'a> {
    pub(super) fn new(pool: &'a Pool, type_registry: &'a TypeRegistry) -> Self {
        Self {
            pool,
            type_registry,
            nodes: Vec::new(),
            interned: FxHashMap::default(),
            visiting: FxHashSet::default(),
        }
    }

    pub(super) fn finish(self) -> RetainPlanTable {
        RetainPlanTable {
            nodes: self.nodes.into_boxed_slice(),
        }
    }

    pub(super) fn duplication_for(&mut self, ty: Idx) -> Result<Duplication, DuplicationFailure> {
        // Retain `ty` as the frozen semantic identity. Resolution supplies the
        // topology view only; it may cross a nominal-to-layout boundary.
        let resolved = self.pool.resolve_fully(ty);
        if resolved == Idx::NONE || resolved.raw() as usize >= self.pool.len() {
            return Err(DuplicationFailure::UnresolvedType);
        }
        if crate::lower::type_has_user_drop(ty, self.type_registry)
            || crate::lower::type_has_user_drop(resolved, self.type_registry)
        {
            return Err(DuplicationFailure::UserDefinedDrop);
        }

        let tag = self.pool.tag(resolved);
        if matches!(tag, Tag::Iterator | Tag::DoubleEndedIterator) {
            return Err(DuplicationFailure::AffineIterator);
        }
        if matches!(tag, Tag::Struct | Tag::Enum) && self.pool.aggregate_type_is_recursive(resolved)
        {
            return self
                .intern(ty, RetainPlanKind::SelfOwnedIdentity)
                .map(Duplication::Retain)
                .map_err(DuplicationFailure::RetainPlanIndexOverflow);
        }

        if !self.visiting.insert(resolved) {
            return Err(DuplicationFailure::CyclicInlineTopology);
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
    ) -> Result<Duplication, DuplicationFailure> {
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
                .map_err(DuplicationFailure::RetainPlanIndexOverflow),
            Tag::Tuple => self.product_duplication(identity, self.pool.tuple_elems(resolved)),
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
                Duplication::Retain(_) => Err(DuplicationFailure::OwnershipBearingRange),
            },
            Tag::Iterator | Tag::DoubleEndedIterator => Err(DuplicationFailure::AffineIterator),
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
            | Tag::SelfType => Err(DuplicationFailure::NotDuplicable),
        }
    }

    fn product_duplication(
        &mut self,
        ty: Idx,
        fields: Vec<Idx>,
    ) -> Result<Duplication, DuplicationFailure> {
        let edges = self.field_edges(fields)?;
        if edges.is_empty() {
            Ok(Duplication::Copy)
        } else {
            self.intern(ty, RetainPlanKind::OwnedFields(edges.into_boxed_slice()))
                .map(Duplication::Retain)
                .map_err(DuplicationFailure::RetainPlanIndexOverflow)
        }
    }

    fn variant_duplication(
        &mut self,
        ty: Idx,
        variants: Vec<Vec<Idx>>,
    ) -> Result<Duplication, DuplicationFailure> {
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
                .map_err(DuplicationFailure::RetainPlanIndexOverflow)
        } else {
            Ok(Duplication::Copy)
        }
    }

    fn field_edges(&mut self, fields: Vec<Idx>) -> Result<Vec<RetainPlanEdge>, DuplicationFailure> {
        let mut edges = Vec::new();
        for (field, field_ty) in fields.into_iter().enumerate() {
            if let Duplication::Retain(child) = self.duplication_for(field_ty)? {
                let field =
                    u32::try_from(field).map_err(DuplicationFailure::LogicalFieldIndexOverflow)?;
                edges.push(RetainPlanEdge { field, child });
            }
        }
        Ok(edges)
    }

    fn intern(
        &mut self,
        ty: Idx,
        kind: RetainPlanKind,
    ) -> Result<RetainPlanId, std::num::TryFromIntError> {
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
