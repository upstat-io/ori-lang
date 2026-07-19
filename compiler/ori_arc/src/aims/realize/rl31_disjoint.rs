//! RL-31 backend-neutral parameter-disjointness facts.
//!
//! Spec: Annex E §AIMS RL-31. Proves two parameters are guaranteed
//! non-aliasing via the 8-clause SUFFICIENT condition over their
//! post-monomorphization transitive reachable-memory closures. LLVM `noalias`
//! is one downstream projection of the resulting neutral fact; AIMS does not
//! select or encode backend attributes.
//!
//! Producer/consumer contract: realizes the 8-clause SUFFICIENT condition.
//! Spec: Annex E §AIMS RL-31. The clauses below are that rule realized; ANY
//! change to clause semantics is a spec change, not a local edit.
//!
//! Soundness direction: the rule is SUFFICIENT, not IFF. A `true` verdict
//! MUST mean the parameters cannot alias at runtime; conservative `false`
//! is always safe. When a clause cannot be proven (unresolved type, opaque FFI,
//! user-type whose burden is not reachable at this phase), the proof FAILS
//! CLOSED and never publishes an unsound disjointness claim.
//!
//! Dual-facet soundness (Spec: Annex E §AIMS RL-31 (P2)) requires BOTH:
//! (a) every call site's actual arguments trace to disjoint source aggregates,
//! and (b) the parameters' reachable-memory type closures are disjoint. The
//! proof's negative witness `RL31_type_facet_alone_insufficient` rejects facet
//! (b) alone because type disjointness does not imply allocation disjointness.
//! This module computes facet (b) and reports facet (a) as unproven until the
//! per-call-site provenance analysis ships.

#[cfg(test)]
use ori_registry::burden::table::{burden_type_id, BurdenRegistry};
#[cfg(test)]
use ori_registry::TypeTag;
use ori_types::{Idx, Pool, Tag};
use rustc_hash::FxHashSet;

/// Frozen backend-neutral RL-31 facts for one function's parameters.
///
/// `type_disjointness[i] == true` means parameter `i` satisfies the type-level
/// facet: its reachable-memory closure is provably disjoint from every other
/// reference-class parameter under the 8-clause rule.
///
/// `call_site_provenance_disjoint` is true only when provenance proves that
/// condition at every call site. Consumers must use [`Self::proves_disjoint`],
/// which conjoins both facets and fails closed for an invalid parameter index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamDisjointnessFacts {
    type_disjointness: Vec<bool>,
    call_site_provenance_disjoint: bool,
}

impl ParamDisjointnessFacts {
    /// Return the type-level disjointness facet in declaration order.
    #[must_use]
    pub fn type_disjointness(&self) -> &[bool] {
        &self.type_disjointness
    }

    /// Return whether provenance proves disjointness at every call site.
    #[must_use]
    pub const fn call_site_provenance_disjoint(&self) -> bool {
        self.call_site_provenance_disjoint
    }

    /// Return true only when both RL-31 facets prove this parameter disjoint.
    #[must_use]
    pub fn proves_disjoint(&self, parameter: usize) -> bool {
        self.call_site_provenance_disjoint
            && self
                .type_disjointness
                .get(parameter)
                .copied()
                .unwrap_or(false)
    }
}

/// Prove backend-neutral parameter disjointness for a function signature.
///
/// `param_types` is the resolved `Idx` of each parameter in declaration order.
/// The type facet holds when a parameter is reference-class (carries reachable heap
/// memory) AND its 8-clause SUFFICIENT-condition closure is disjoint from
/// every OTHER reference-class parameter's closure.
///
/// Scalar, `Value`, and opaque parameters never satisfy the type facet because
/// they carry no aliasable reachable memory represented by this proof.
#[must_use]
pub fn prove_param_disjointness(param_types: &[Idx], pool: &Pool) -> ParamDisjointnessFacts {
    let n = param_types.len();
    let mut type_disjointness = vec![false; n];

    // Pre-compute each parameter's closure + clause-5/8 disqualifiers once.
    let analyses: Vec<ParamAnalysis> = param_types
        .iter()
        .map(|&ty| analyze_param(ty, pool))
        .collect();

    for i in 0..n {
        let a = &analyses[i];
        // Reference-class precondition: the closure must contain at least one
        // heap TypeId, else the parameter is scalar/Value/opaque and has no
        // aliasable memory represented by this fact.
        if a.closure.is_empty() || a.fails_closed {
            continue;
        }
        let mut disjoint_from_all_others = true;
        for (j, b) in analyses.iter().enumerate() {
            if i == j {
                continue;
            }
            // Only OTHER reference-class params can alias `i`. A scalar/opaque
            // `j` carries no reachable heap memory, so it cannot alias `i`.
            if b.closure.is_empty() || b.fails_closed {
                continue;
            }
            if !pair_disjoint(a, b, param_types[i], param_types[j], pool) {
                disjoint_from_all_others = false;
                break;
            }
        }
        type_disjointness[i] = disjoint_from_all_others;
    }

    // Facet (b) — the type-level closures — is computed above. Facet (a) —
    // per-call-site provenance proven at EVERY call site — is NOT shipped
    // (rules 5 + 7 unshipped), so it is UNPROVEN here.
    // The neutral fact remains false until the provenance pass covers every
    // call site. `proves_disjoint` keeps the two facets indivisible for every
    // downstream consumer.
    ParamDisjointnessFacts {
        type_disjointness,
        call_site_provenance_disjoint: false,
    }
}

/// Per-parameter precomputed analysis for the 8-clause rule.
struct ParamAnalysis {
    /// Clauses 1-3: post-monomorphization transitive closure of reachable
    /// HEAP `Idx`s, with scalar / `Value` / `Never` / fn-ptr `TypeId`s filtered.
    closure: FxHashSet<Idx>,
    /// Clause 5 (¬ transitive sum node) OR clause 8 (¬ opaque-FFI) OR clause
    /// 3 (¬ unresolved `PossibleRef` survivor) violated → fail closed.
    fails_closed: bool,
}

fn analyze_param(ty: Idx, pool: &Pool) -> ParamAnalysis {
    // Borrowed parameters dereference to the pointee per Invariant 2
    // (DefiniteRef → pointee BurdenSpec); `Tag::Borrowed` carries the pointee
    // Idx in extra[0]. Non-borrowed (Owned) params use the type directly.
    let target = deref_borrowed(ty, pool);

    let mut walk = ClosureWalk {
        pool,
        closure: FxHashSet::default(),
        visited: FxHashSet::default(),
        fails_closed: false,
    };
    walk.visit(target);

    // Clause 5: NO transitive sum-type node anywhere in the closure (SetTag
    // payload-invalidation is transitive — RL-10 / DP-5).
    if !walk.fails_closed && has_sum_node_in_closure(target, pool) {
        walk.fails_closed = true;
    }

    ParamAnalysis {
        closure: walk.closure,
        fails_closed: walk.fails_closed,
    }
}

/// Resolve a `Tag::Borrowed(inner)` to its pointee `Idx`; pass other types
/// through. `resolve_fully` is applied first so newtype/alias chains collapse.
fn deref_borrowed(ty: Idx, pool: &Pool) -> Idx {
    let resolved = pool.resolve_fully(ty);
    if pool.tag(resolved) == Tag::Borrowed {
        // Borrowed stores the pointee in the extra array (extra[0]), not as a
        // child Idx in the item's `data` field; `borrowed_inner` reads it.
        pool.borrowed_inner(resolved)
    } else {
        resolved
    }
}

/// Fixed-point reachable-reference-graph walk (Invariant 6) with cycle-safe
/// visited set. Bundling the four walk arguments into one borrow keeps the
/// recursive calls to a single `self.visit(child)` shape.
struct ClosureWalk<'a> {
    pool: &'a Pool,
    /// Heap `Idx`s of every non-trivial reachable type (filtered scalars).
    closure: FxHashSet<Idx>,
    visited: FxHashSet<Idx>,
    /// Set on an unresolvable / opaque-FFI / `PossibleRef`-survivor node
    /// (clauses 3 + 8).
    fails_closed: bool,
}

impl ClosureWalk<'_> {
    /// Add the heap `Idx` of every non-trivial type reachable from `idx`.
    fn visit(&mut self, idx: Idx) {
        if self.fails_closed {
            return;
        }
        let resolved = self.pool.resolve_fully(idx);
        if !self.visited.insert(resolved) {
            return; // cycle closed at this node
        }
        if resolved.raw() as usize >= self.pool.len() {
            self.fails_closed = true; // clause 3 `PossibleRef` survivor
            return;
        }
        match self.pool.tag(resolved) {
            // Clause-3 canonical-triviality filter: scalars carry no reachable
            // heap memory; excluded from the disjointness set.
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Ordering
            | Tag::Duration
            | Tag::Size
            | Tag::Error => {}

            // Indirect single-element collections: the element type IS reachable
            // heap memory; recurse into the element for the transitive closure.
            tag @ (Tag::List
            | Tag::Set
            | Tag::Channel
            | Tag::Iterator
            | Tag::DoubleEndedIterator) => {
                self.closure.insert(resolved);
                let elem = match tag {
                    Tag::List => self.pool.list_elem(resolved),
                    Tag::Set => self.pool.set_elem(resolved),
                    Tag::Channel => self.pool.channel_elem(resolved),
                    _ => self.pool.iterator_elem(resolved),
                };
                self.visit(elem);
            }
            // Leaf reachable-heap nodes: `str` (heap byte buffer) and a closure
            // (heap capture env) are themselves reachable memory with no child
            // to recurse into.
            Tag::Str | Tag::Function => {
                self.closure.insert(resolved);
            }
            Tag::Map => {
                self.closure.insert(resolved);
                self.visit(self.pool.map_key(resolved));
                self.visit(self.pool.map_value(resolved));
            }
            // Range element is scalar in the shipped surface; recurse for safety.
            Tag::Range => self.visit(self.pool.range_elem(resolved)),
            // Sum nodes — clause 5 disqualifies via `has_sum_node_in_closure`;
            // the walk still descends so the reachable-memory set is complete.
            Tag::Option => self.visit(self.pool.option_inner(resolved)),
            Tag::Result => {
                self.visit(self.pool.result_ok(resolved));
                self.visit(self.pool.result_err(resolved));
            }
            Tag::Tuple => {
                for elem in self.pool.tuple_elems(resolved) {
                    self.visit(elem);
                }
            }
            // The struct node is added so two distinct struct types never
            // falsely intersect on shared field TypeIds alone.
            Tag::Struct => {
                self.closure.insert(resolved);
                for (_, field_ty) in self.pool.struct_fields(resolved) {
                    self.visit(field_ty);
                }
            }
            Tag::Enum => {
                self.closure.insert(resolved);
                for (_, payload) in self.pool.enum_variants(resolved) {
                    for field_ty in payload {
                        self.visit(field_ty);
                    }
                }
            }
            // Unresolved variables / projections / Self / opaque — fail closed
            // (clause 3 requires Scalar | DefiniteRef after monomorphization;
            // clause 8 excludes opaque-FFI without explicit burden).
            Tag::Var
            | Tag::BoundVar
            | Tag::RigidVar
            | Tag::Infer
            | Tag::Projection
            | Tag::SelfType
            | Tag::Scheme
            | Tag::ModuleNs
            | Tag::Named
            | Tag::Applied
            | Tag::Alias
            | Tag::Borrowed => {
                self.fails_closed = true;
            }
        }
    }
}

/// Clause 5: `true` iff ANY node reachable from `ty` is a sum type
/// (`Option` / `Result` / `Enum`). Such a node is subject to `SetTag`
/// payload-invalidation (RL-10 / DP-5) and disqualifies the disjointness claim.
fn has_sum_node_in_closure(ty: Idx, pool: &Pool) -> bool {
    let mut visited = FxHashSet::default();
    has_sum_node_rec(ty, pool, &mut visited)
}

fn has_sum_node_rec(idx: Idx, pool: &Pool, visited: &mut FxHashSet<Idx>) -> bool {
    let resolved = pool.resolve_fully(idx);
    if !visited.insert(resolved) {
        return false;
    }
    if resolved.raw() as usize >= pool.len() {
        return false;
    }
    match pool.tag(resolved) {
        Tag::Option | Tag::Result | Tag::Enum => true,
        Tag::List | Tag::Set | Tag::Channel | Tag::Iterator | Tag::DoubleEndedIterator => {
            let elem = match pool.tag(resolved) {
                Tag::List => pool.list_elem(resolved),
                Tag::Set => pool.set_elem(resolved),
                Tag::Channel => pool.channel_elem(resolved),
                _ => pool.iterator_elem(resolved),
            };
            has_sum_node_rec(elem, pool, visited)
        }
        Tag::Map => {
            has_sum_node_rec(pool.map_key(resolved), pool, visited)
                || has_sum_node_rec(pool.map_value(resolved), pool, visited)
        }
        Tag::Range => has_sum_node_rec(pool.range_elem(resolved), pool, visited),
        Tag::Tuple => pool
            .tuple_elems(resolved)
            .into_iter()
            .any(|e| has_sum_node_rec(e, pool, visited)),
        Tag::Struct => pool
            .struct_fields(resolved)
            .into_iter()
            .any(|(_, f)| has_sum_node_rec(f, pool, visited)),
        _ => false,
    }
}

/// The 8-clause SUFFICIENT condition for a single parameter pair.
///
/// Preconditions (checked by caller): both params are reference-class
/// (non-empty closure) and neither failed closed (clauses 3/5/8 hold).
/// This function discharges clauses 4 (closure disjointness) and 7 (distinct
/// surface types reject same-binding).
fn pair_disjoint(a: &ParamAnalysis, b: &ParamAnalysis, ty_i: Idx, ty_j: Idx, pool: &Pool) -> bool {
    // Clause 4: filtered closures are disjoint.
    if !a.closure.is_disjoint(&b.closure) {
        return false;
    }
    // Clause 7: the type system rejects passing one binding to both. For
    // distinct surface types this holds by construction (no value satisfies
    // both `T_i` and `T_j`). Same surface type → the type system DOES accept
    // `f(x, x)`, so type-level disjointness is insufficient (Invariant 7);
    // omit the claim (the contract-layer per-call-site path owns that case).
    let surface_i = pool.resolve_fully(ty_i);
    let surface_j = pool.resolve_fully(ty_j);
    if surface_i == surface_j {
        return false;
    }
    true
}

/// Map a pre-interned primitive / collection `Idx` to its builtin
/// `BurdenRegistry` entry, when one exists. Self-sufficient for builtin
/// collection types (str/list/map/set) without a `TypeRegistry`. User types
/// (dynamic `Idx` without a builtin tag) return `None` — the proof's closure
/// walk already covers their structure via the `Pool`. Test-only helper
/// pinning the builtin-burden tag classification; the live proof path is the
/// closure walk, which does not consult this helper.
#[cfg(test)]
#[must_use]
pub(crate) fn builtin_burden_present(ty: Idx, pool: &Pool) -> bool {
    let resolved = pool.resolve_fully(ty);
    let tag = pool.tag(resolved);
    let type_tag = match tag {
        Tag::Str => TypeTag::Str,
        Tag::List => TypeTag::List,
        Tag::Map => TypeTag::Map,
        Tag::Set => TypeTag::Set,
        Tag::Channel => TypeTag::Channel,
        Tag::Iterator => TypeTag::Iterator,
        Tag::DoubleEndedIterator => TypeTag::DoubleEndedIterator,
        Tag::Option => TypeTag::Option,
        Tag::Result => TypeTag::Result,
        _ => return false,
    };
    BurdenRegistry::lookup_builtin(burden_type_id(type_tag)).is_some()
}

#[cfg(test)]
mod tests;
