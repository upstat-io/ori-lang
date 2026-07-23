//! Tests for `ori_repr` types, organized by representation responsibility.

mod aggregate_triviality;
mod applied_metadata;
mod canonical_collections;
mod canonical_kind_matrix;
mod canonical_primitives;
mod canonical_shapes;
mod collection_surfaces;
mod core_repr;
mod enum_plan_fallback;
mod imported_metadata;
mod layout_recursion;
mod plan_decisions;
mod plan_pipeline;
mod repr_attributes;
mod resolved_metadata;
mod triviality_validation;
mod yield_allocations;

use crate::canonical::{canonical, canonical_cached};
use crate::enum_repr::{EnumTag, VariantRepr};
use crate::escape::EscapeInfo;
use crate::plan::{DecisionReason, DecisionSource, NarrowingPolicy, RcStrategy, ReprDecision};
use crate::range::ValueRange;
use crate::repr::{FloatWidth, IntWidth, MachineRepr};
use crate::struct_repr::{ClosureRepr, FatRepr, FieldRepr, RcRepr, StructRepr, TupleRepr};
use crate::ReprAttribute;
use crate::ReprPlan;
use ori_arc::ir::{
    AllocationSiteId, ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId,
    ArgOwnership, LitValue, ValueRepr, YieldAllocationFact, YieldAllocationLocality, YieldExtent,
};
use ori_arc::ArcBlockId;
use ori_ir::Name;
use ori_types::{ExportedTypeMetadata, Idx, Pool};
