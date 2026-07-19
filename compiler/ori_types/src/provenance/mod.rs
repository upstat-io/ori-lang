//! Read-only provenance DAG over the type pool.
//!
//! Walks the session [`Pool`] from one root [`Idx`], collecting the full
//! provenance edge set for the nested-generic-mono bug class:
//!
//! - STRUCTURE edges ([`StructureEdge`]) — struct-field and enum-variant-payload
//!   children, from [`Pool::struct_fields`] / [`Pool::enum_variants`].
//! - RESOLUTION edges ([`ResolutionEdge`]) — `Named`/`Applied -> concrete` from
//!   the pool's resolution map, read per node via the read-only [`Pool::resolve`]
//!   accessor (never [`Pool::set_resolution`]).
//! - MONOMORPHIZATION edges ([`MonoEdge`]) — `(source_idx -> substituted_idx)`
//!   projected 1:1 off [`MonoInstance::body_type_map`] via [`ProvenanceDag::with_mono_edges`].
//!
//! Plus a generic-leaf divergence detector ([`GenericLeafDivergence`]): a node
//! whose resolved body carries a generic-param leaf (a type variable OR an
//! unresolved `Named`) while a parallel monomorphization substitutes it to a
//! concrete body — the `Wrap#217{inner: T#141}` vs concrete `#231{inner: int#0}`
//! shape the whole diagnostic exists to surface.
//!
//! Every read is strictly read-only — the walk NEVER mutates the pool, the
//! resolution map, or any compilation state. It is a diagnostic view over the
//! stable session type-pool `Idx`, surfaced by `oric` under `ORI_TRACE_IDX=<n>`.
//!
//! The walk is depth-bounded ([`MAX_DEPTH`]) AND cycle-guarded (a visited set of
//! resolved indices) so a recursive type graph (e.g. `Wrap<Wrap<int>>`, a known
//! typeck stack-overflow shape) cannot recurse unboundedly.

use std::fmt::Write;

use ori_ir::StringInterner;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{Idx, MonoInstance, Pool, Tag};

/// Hard depth cap for the provenance walk. Independent of the visited-set cycle
/// guard — both bounds apply from the first recursion step.
pub const MAX_DEPTH: usize = 64;

/// One STRUCTURE edge: a struct field of `parent_idx` or an enum-variant
/// payload of `parent_idx`, pointing at `child_idx`.
///
/// For struct fields, `field_name` is the field name. For enum-variant
/// payloads, `field_name` is the variant name (each payload slot of a variant
/// is one edge under that variant's name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureEdge {
    /// Resolved index of the aggregate carrying this edge.
    pub parent_idx: Idx,
    /// Field name (struct) or variant name (enum payload).
    pub field_name: ori_ir::Name,
    /// Resolved-or-surface index of the field / payload type.
    pub child_idx: Idx,
}

/// One RESOLUTION edge: a `Named`/`Applied` index that resolves (via the pool's
/// resolution map) to a concrete `Struct`/`Enum` index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionEdge {
    /// The `Named`/`Applied` index, pre-resolution.
    pub applied_idx: Idx,
    /// The concrete index it resolves to.
    pub concrete_idx: Idx,
}

/// One MONOMORPHIZATION edge: a `(source_idx -> substituted_idx)` entry from a
/// [`MonoInstance::body_type_map`] — a generic body index and the concrete index
/// it was substituted to for one instantiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonoEdge {
    /// The generic (pre-substitution) body index.
    pub source_idx: Idx,
    /// The concrete (post-substitution) index.
    pub substituted_idx: Idx,
}

/// One generic-leaf divergence: a walked node (`generic_idx`) whose resolved
/// body carries a generic-param leaf, paired with a concrete monomorphization
/// (`concrete_idx`) of that same source — the `#217`-vs-`#231` shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericLeafDivergence {
    /// The node whose resolved body carries a generic-param leaf.
    pub generic_idx: Idx,
    /// The concrete monomorphization the source diverges from.
    pub concrete_idx: Idx,
}

/// One CONSUMER edge: attributes a logical drop plan back to the `Idx` chain
/// whose drop-synthesis walk produced it.
///
/// Populated read-only by the backend-neutral drop-descriptor walk over the
/// same root the DAG walks. Physical projections may give the plan a symbol,
/// bytecode identity, or runtime-table entry without changing this edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerEdge {
    /// The type whose logical drop plan consumes the walked chain.
    pub type_idx: Idx,
    /// The field/projection chain the drop-synthesis walk descended to reach
    /// `type_idx`, root-first (the last entry equals `type_idx`).
    pub walked_chain: Vec<Idx>,
}

/// A read-only provenance DAG rooted at one type-pool [`Idx`], carrying the full
/// edge set (structure + resolution + monomorphization) plus the generic-leaf
/// divergence verdicts.
#[derive(Debug, Clone)]
pub struct ProvenanceDag {
    /// The root index the walk started from (surface index, pre-resolution).
    pub root: Idx,
    /// STRUCTURE edges in walk (pre-order) order.
    pub edges: Vec<StructureEdge>,
    /// RESOLUTION edges (`Named`/`Applied -> concrete`) for walked nodes.
    pub resolution_edges: Vec<ResolutionEdge>,
    /// MONOMORPHIZATION edges; empty until [`Self::with_mono_edges`] is applied.
    pub mono_edges: Vec<MonoEdge>,
    /// Generic-leaf divergence verdicts: a generic-param leaf reached under a
    /// concrete root (computed by [`Self::walk`]), refined to the precise mono
    /// substitution by [`Self::with_mono_edges`] when an instance supplies one.
    pub divergences: Vec<GenericLeafDivergence>,
    /// CONSUMER edges; empty until [`Self::with_consumer_edges`] is applied.
    pub consumer_edges: Vec<ConsumerEdge>,
}

impl ProvenanceDag {
    /// Walk the pool from `root`, collecting STRUCTURE + RESOLUTION edges and the
    /// generic-leaf node set.
    ///
    /// Depth-bounded by `max_depth` AND cycle-guarded by a visited set of
    /// resolved indices. Out-of-range / unresolvable indices yield no edges
    /// (never a panic). Mono edges + divergences are empty until
    /// [`Self::with_mono_edges`] is applied.
    #[must_use]
    pub fn walk(pool: &Pool, root: Idx, max_depth: usize) -> Self {
        let mut state = WalkState {
            pool,
            max_depth,
            visited: FxHashSet::default(),
            edges: Vec::new(),
            resolution_edges: Vec::new(),
            generic_leaf_nodes: Vec::new(),
        };
        state.walk_from(root, 0);
        // Why: a generic-param leaf reached under a CONCRETE root is a divergence
        // (the param survived unsubstituted into a concrete instantiation). The
        // concrete counterpart is the resolved root, refined by `with_mono_edges`
        // when a mono instance supplies one; a generic ROOT yields no divergence.
        let resolved_root = pool.resolve_fully(root);
        let mut divergences = Vec::new();
        if pool.is_valid_idx(resolved_root) && !Self::is_generic_leaf(pool, resolved_root) {
            let mut seen = FxHashSet::default();
            for &generic in &state.generic_leaf_nodes {
                if seen.insert(generic) {
                    divergences.push(GenericLeafDivergence {
                        generic_idx: generic,
                        concrete_idx: resolved_root,
                    });
                }
            }
        }
        Self {
            root,
            edges: state.edges,
            resolution_edges: state.resolution_edges,
            mono_edges: Vec::new(),
            divergences,
            consumer_edges: Vec::new(),
        }
    }

    /// Project MONOMORPHIZATION edges off the supplied instances'
    /// [`MonoInstance::body_type_map`]s and compute generic-leaf divergences.
    ///
    /// A `(source, substituted)` entry becomes a [`MonoEdge`]. A walked node
    /// carrying a generic-param leaf (collected by [`Self::walk`]) whose
    /// `source_idx` has a concrete substitution (a mono edge whose
    /// `substituted_idx` carries NO generic leaf) is a [`GenericLeafDivergence`].
    /// Read-only: consumes the already-built `body_type_map`, never re-substitutes.
    #[must_use]
    pub fn with_mono_edges(mut self, pool: &Pool, instances: &[MonoInstance]) -> Self {
        let mut mono_edges = Vec::new();
        let mut concrete_of: FxHashMap<Idx, Idx> = FxHashMap::default();
        for inst in instances {
            for &(source, substituted) in &inst.body_type_map {
                mono_edges.push(MonoEdge {
                    source_idx: source,
                    substituted_idx: substituted,
                });
                // A concrete substitution (the substituted body carries no
                // generic leaf) records the source's concrete counterpart.
                if !Self::body_carries_generic_leaf(pool, substituted) {
                    concrete_of.entry(source).or_insert(substituted);
                }
            }
        }
        // Refine each structural divergence's concrete counterpart to the PRECISE
        // mono substitution when an instance supplies one (the concrete `int#0`
        // the param resolves to), falling back to the resolved-root context.
        for div in &mut self.divergences {
            if let Some(&concrete) = concrete_of.get(&div.generic_idx) {
                div.concrete_idx = concrete;
            }
        }
        self.mono_edges = mono_edges;
        self
    }

    /// Attach CONSUMER edges attributing generated drop-glue symbols back to the
    /// `Idx` chains that produced them. Read-only — the edges are computed by the
    /// drop-descriptor walk (`ori_arc::drop`) and threaded in verbatim; this
    /// builder only stores them.
    #[must_use]
    pub fn with_consumer_edges(mut self, consumer_edges: Vec<ConsumerEdge>) -> Self {
        self.consumer_edges = consumer_edges;
        self
    }

    /// True when the DAG carries no STRUCTURE edges (a leaf / unresolvable root).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// True when the resolved body of `idx` carries a generic-param leaf — a
    /// type variable (`Var`/`BoundVar`/`RigidVar`) OR an unresolved `Named`
    /// /`Applied` (one whose [`Pool::resolve`] yields nothing). The unresolved
    /// `Named` arm is the gotcha [`Tag::is_type_variable`] alone misses.
    fn is_generic_leaf(pool: &Pool, idx: Idx) -> bool {
        if !pool.is_valid_idx(idx) {
            return false;
        }
        let tag = pool.tag(idx);
        if tag.is_type_variable() {
            return true;
        }
        // A `Named`/`Applied` that does not resolve to a concrete definition is
        // an unresolved generic-param reference (the `T#141` leaf shape).
        matches!(tag, Tag::Named | Tag::Applied) && pool.resolve(idx).is_none()
    }

    /// True when `idx` itself OR any STRUCTURE descendant of its resolved body
    /// carries a generic-param leaf. Depth-bounded + cycle-guarded, mirroring the
    /// main walk, so a recursive body cannot recurse unboundedly.
    fn body_carries_generic_leaf(pool: &Pool, idx: Idx) -> bool {
        let mut visited = FxHashSet::default();
        Self::body_carries_generic_leaf_from(pool, idx, MAX_DEPTH, 0, &mut visited)
    }

    fn body_carries_generic_leaf_from(
        pool: &Pool,
        current: Idx,
        max_depth: usize,
        depth: usize,
        visited: &mut FxHashSet<Idx>,
    ) -> bool {
        if depth >= max_depth {
            return false;
        }
        if Self::is_generic_leaf(pool, current) {
            return true;
        }
        let resolved = pool.resolve_fully(current);
        if !pool.is_valid_idx(resolved) || !visited.insert(resolved) {
            return false;
        }
        if Self::is_generic_leaf(pool, resolved) {
            return true;
        }
        match pool.tag(resolved) {
            Tag::Struct => pool.struct_fields(resolved).into_iter().any(|(_, child)| {
                Self::body_carries_generic_leaf_from(pool, child, max_depth, depth + 1, visited)
            }),
            Tag::Enum => pool
                .enum_variants(resolved)
                .into_iter()
                .any(|(_, payloads)| {
                    payloads.into_iter().any(|child| {
                        Self::body_carries_generic_leaf_from(
                            pool,
                            child,
                            max_depth,
                            depth + 1,
                            visited,
                        )
                    })
                }),
            _ => false,
        }
    }

    /// Render the DAG using the existing [`Pool::format_type_with_idx`] renderer.
    ///
    /// One header line, then one section each for STRUCTURE, RESOLUTION, MONO
    /// edges and DIVERGENCE verdicts. Read-only — takes `&Pool`.
    #[must_use]
    pub fn render(&self, pool: &Pool, interner: &StringInterner) -> String {
        let mut out = String::new();
        // `format_type_with_idx` -> `Pool::tag` indexes `items` without a bounds
        // check; an out-of-range root would panic. `walk` guards its traversal,
        // so guard the root here too — `render` is panic-free for any root.
        if !pool.is_valid_idx(self.root) {
            return out;
        }
        let _ = writeln!(
            out,
            "=== Provenance DAG for Idx#{}: {} ===",
            self.root.raw(),
            pool.format_type_with_idx(self.root, interner),
        );
        let _ = writeln!(out, "  {} structure edge(s)", self.edges.len());
        for edge in &self.edges {
            let _ = writeln!(
                out,
                "  {} --{}--> {}",
                Self::render_idx(pool, edge.parent_idx, interner),
                interner.lookup(edge.field_name),
                Self::render_idx(pool, edge.child_idx, interner),
            );
        }
        let _ = writeln!(out, "  {} resolution edge(s)", self.resolution_edges.len());
        for edge in &self.resolution_edges {
            let _ = writeln!(
                out,
                "  {} ~resolves~> {}",
                Self::render_idx(pool, edge.applied_idx, interner),
                Self::render_idx(pool, edge.concrete_idx, interner),
            );
        }
        let _ = writeln!(out, "  {} mono edge(s)", self.mono_edges.len());
        for edge in &self.mono_edges {
            let _ = writeln!(
                out,
                "  {} =mono=> {}",
                Self::render_idx(pool, edge.source_idx, interner),
                Self::render_idx(pool, edge.substituted_idx, interner),
            );
        }
        let _ = writeln!(out, "  {} divergence(s)", self.divergences.len());
        for div in &self.divergences {
            let _ = writeln!(
                out,
                "  generic {} <> concrete {}",
                Self::render_idx(pool, div.generic_idx, interner),
                Self::render_idx(pool, div.concrete_idx, interner),
            );
        }
        let _ = writeln!(out, "  {} consumer edge(s)", self.consumer_edges.len());
        for edge in &self.consumer_edges {
            let chain: Vec<String> = edge
                .walked_chain
                .iter()
                .map(|&idx| Self::render_idx(pool, idx, interner))
                .collect();
            let _ = writeln!(
                out,
                "  drop-plan {} <=consumes= {}",
                Self::render_idx(pool, edge.type_idx, interner),
                chain.join(" -> "),
            );
        }
        let _ = writeln!(out, "=== END Provenance DAG ===");
        out
    }

    /// Render one index, guarding the unchecked `format_type_with_idx` index.
    fn render_idx(pool: &Pool, idx: Idx, interner: &StringInterner) -> String {
        if pool.is_valid_idx(idx) {
            pool.format_type_with_idx(idx, interner)
        } else {
            format!("Idx#{}(out-of-range)", idx.raw())
        }
    }
}

/// Mutable accumulator threaded through the recursive structure walk — bundles
/// the pool borrow, depth cap, cycle-guard visited set, and the three output
/// collections so the recursion is one `&mut self` method, not an 8-arg helper.
struct WalkState<'a> {
    pool: &'a Pool,
    max_depth: usize,
    visited: FxHashSet<Idx>,
    edges: Vec<StructureEdge>,
    resolution_edges: Vec<ResolutionEdge>,
    generic_leaf_nodes: Vec<Idx>,
}

impl WalkState<'_> {
    fn walk_from(&mut self, current: Idx, depth: usize) {
        if depth >= self.max_depth {
            return;
        }
        // RESOLUTION edge: a `Named`/`Applied` current resolving to a distinct
        // concrete index is one resolution edge (read-only `resolve`).
        if let Some(concrete) = self.pool.resolve(current) {
            if concrete != current {
                self.resolution_edges.push(ResolutionEdge {
                    applied_idx: current,
                    concrete_idx: concrete,
                });
            }
        }
        // GENERIC-LEAF signal: a node whose body carries a generic-param leaf is
        // a divergence candidate (paired against mono edges later).
        if ProvenanceDag::is_generic_leaf(self.pool, current) {
            self.generic_leaf_nodes.push(current);
        }
        // `resolve_fully` is itself bounds- and cycle-guarded; it returns the
        // input unchanged for an out-of-range index.
        let resolved = self.pool.resolve_fully(current);
        // `Pool::tag` indexes `items` without a bounds check; guard before it.
        if !self.pool.is_valid_idx(resolved) {
            return;
        }
        // Cycle guard: a resolved index already on the walk stops the recursion.
        if !self.visited.insert(resolved) {
            return;
        }
        match self.pool.tag(resolved) {
            Tag::Struct => {
                for (field_name, child_idx) in self.pool.struct_fields(resolved) {
                    self.edges.push(StructureEdge {
                        parent_idx: resolved,
                        field_name,
                        child_idx,
                    });
                    self.walk_from(child_idx, depth + 1);
                }
            }
            Tag::Enum => {
                for (variant_name, payloads) in self.pool.enum_variants(resolved) {
                    for child_idx in payloads {
                        self.edges.push(StructureEdge {
                            parent_idx: resolved,
                            field_name: variant_name,
                            child_idx,
                        });
                        self.walk_from(child_idx, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
