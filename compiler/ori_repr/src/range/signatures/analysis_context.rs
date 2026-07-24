use core::fmt;
use ori_arc::graph::call_graph::CallGraph;
use ori_arc::graph::scc::Scc;
use ori_arc::ir::ArcFunction;
use ori_ir::Name;
use ori_types::Pool;
use rustc_hash::FxHashMap;

use crate::plan::ReprPlan;
use crate::range::fixpoint::RangeFixpointResult;
use crate::range::RangeAnalysisConfig;

use super::FunctionRangeInfo;

/// Shared inputs for one interprocedural range-propagation run.
#[derive(Clone, Copy)]
pub(super) struct RangePropagationContext<'a> {
    /// Strongly connected components in propagation order.
    pub(super) sccs: &'a [Scc],
    /// Call edges used to discover recursive and downstream dependencies.
    pub(super) call_graph: &'a CallGraph,
    /// Function bodies addressable by their stable names.
    pub(super) func_map: &'a FxHashMap<Name, &'a ArcFunction>,
    /// Type pool used by each local range analysis.
    pub(super) pool: &'a Pool,
    /// Range-analysis configuration shared across functions.
    pub(super) config: &'a RangeAnalysisConfig,
    /// Representation plan receiving range-driven decisions.
    pub(super) plan: &'a ReprPlan,
}

/// Mutable summaries produced by interprocedural range propagation.
pub(super) struct RangePropagationState<'a> {
    /// Per-function fixpoint results accumulated during propagation.
    pub(super) results: &'a mut FxHashMap<Name, RangeFixpointResult>,
    /// Interprocedural summaries accumulated during propagation.
    pub(super) func_infos: &'a mut FxHashMap<Name, FunctionRangeInfo>,
}

// Why: Propagation contexts retain whole-program graphs and plans; report cardinalities only.
impl fmt::Debug for RangePropagationContext<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RangePropagationContext")
            .field("scc_count", &self.sccs.len())
            .field("function_count", &self.func_map.len())
            .finish()
    }
}

// Why: Propagation state holds complete result maps; their sizes identify progress.
impl fmt::Debug for RangePropagationState<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RangePropagationState")
            .field("result_count", &self.results.len())
            .field("function_info_count", &self.func_infos.len())
            .finish()
    }
}
