//! ARC type classifier.
//!
//! Walks the type pool to classify each type as `Scalar`, `DefiniteRef`,
//! or `PossibleRef`. Uses memoization and cycle detection to handle
//! recursive types efficiently.

use std::cell::RefCell;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::{Idx, Pool};

use crate::{ArcClass, ArcClassification};

/// Type classifier for ARC analysis.
///
/// Wraps a `Pool` reference with classification caching and cycle detection.
/// This mirrors the `TypeInfoStore` pattern in `ori_llvm` but lives in a
/// separate crate with no LLVM dependency.
///
/// # Interior Mutability
///
/// Uses `RefCell` for the cache and cycle-detection set because the
/// [`ArcClassification`] trait takes `&self`. This is the same pattern
/// used by `TypeInfoStore::is_trivial()`.
//
// NOTE: `ori_registry::MemoryStrategy` could provide a fast path for
// builtin type classification (e.g., int → Copy, str → Arc) without
// going through the Pool. This would require mapping Idx → TypeTag for
// known builtins. Not needed for current workloads — tracked for a
// future optimization pass.
pub struct ArcClassifier<'pool> {
    pool: &'pool Pool,
    cache: RefCell<FxHashMap<Idx, ArcClass>>,
    /// Tracks indices currently being classified for cycle detection.
    /// If we encounter an Idx already in this set, we have a recursive
    /// type — which requires heap indirection and is thus `DefiniteRef`.
    classifying: RefCell<FxHashSet<Idx>>,
}

impl<'pool> ArcClassifier<'pool> {
    /// Create a new classifier for the given type pool.
    pub fn new(pool: &'pool Pool) -> Self {
        Self {
            pool,
            cache: RefCell::new(FxHashMap::default()),
            classifying: RefCell::new(FxHashSet::default()),
        }
    }

    /// Create a classifier with a pre-populated cache from a previous run.
    ///
    /// Safe because classification is purely a function of `Pool` structure —
    /// same `Idx` + same `Pool` entry = same `ArcClass`. When the `Pool` is
    /// identical (e.g., module hash hasn't changed), cached classifications
    /// are guaranteed correct and skip redundant type walks.
    pub fn with_cache(pool: &'pool Pool, cache: FxHashMap<Idx, ArcClass>) -> Self {
        Self {
            pool,
            cache: RefCell::new(cache),
            classifying: RefCell::new(FxHashSet::default()),
        }
    }

    /// Access the underlying pool.
    pub fn pool(&self) -> &'pool Pool {
        self.pool
    }

    /// Consume the classifier and return its classification cache.
    ///
    /// Feed this back into [`with_cache`](Self::with_cache) on the next run
    /// to avoid redundant type walks for unchanged modules. Prefer this over
    /// [`export_cache`](Self::export_cache) when the classifier is no longer
    /// needed, as it avoids cloning the entire map.
    pub fn into_cache(self) -> FxHashMap<Idx, ArcClass> {
        self.cache.into_inner()
    }

    /// Export a snapshot of the classification cache (clones).
    ///
    /// Use [`into_cache`](Self::into_cache) instead if the classifier is no
    /// longer needed after export.
    pub fn export_cache(&self) -> FxHashMap<Idx, ArcClass> {
        self.cache.borrow().clone()
    }

    /// Core classification with caching and cycle detection.
    fn classify(&self, idx: Idx) -> ArcClass {
        // Sentinel: NONE is not a real type, treat as scalar (same as TypeInfoStore).
        if idx == Idx::NONE {
            return ArcClass::Scalar;
        }

        // Resolve type variables: follow VarState::Link chains from inference.
        // The type checker unifies variables but may leave the original Var index
        // in compound types (e.g., Option<Var(96)> where Var(96) → int).
        let idx = self.pool.resolve_fully(idx);

        // Fast path: pre-interned primitives (indices 0-11) can be classified
        // by raw index without any hash map lookup.
        if idx.is_primitive() {
            return Self::classify_primitive(idx);
        }

        // Cache hit — return immediately.
        if let Some(&cached) = self.cache.borrow().get(&idx) {
            return cached;
        }

        // Cycle detection: if this Idx is already being classified, we have
        // a recursive type. Recursive types require heap indirection → DefiniteRef.
        if !self.classifying.borrow_mut().insert(idx) {
            return ArcClass::DefiniteRef;
        }

        let result = self.classify_by_tag(idx);

        self.classifying.borrow_mut().remove(&idx);
        self.cache.borrow_mut().insert(idx, result);
        result
    }

    /// Fast path for pre-interned primitives (indices 0-11).
    ///
    /// These are known at compile time, so we can match on the raw index
    /// directly without going through the Pool's tag lookup.
    #[inline]
    fn classify_primitive(idx: Idx) -> ArcClass {
        match idx {
            Idx::INT
            | Idx::FLOAT
            | Idx::BOOL
            | Idx::CHAR
            | Idx::BYTE
            | Idx::UNIT
            | Idx::NEVER
            | Idx::ERROR
            | Idx::DURATION
            | Idx::SIZE
            | Idx::ORDERING => ArcClass::Scalar,

            // Str is heap-allocated.
            Idx::STR => ArcClass::DefiniteRef,

            // Unreachable for valid primitives, but be conservative.
            _ => ArcClass::PossibleRef,
        }
    }

    /// Classify a non-primitive type by delegating to the canonical
    /// `classify_triviality()` in `ori_types`.
    ///
    /// This is the single-source-of-truth unification (§02.1): all
    /// classification logic lives in `ori_types::triviality`, and
    /// `ArcClassifier` maps the result to `ArcClass`.
    ///
    /// Mapping: `Trivial → Scalar`, `NonTrivial → DefiniteRef`,
    /// `Unknown → PossibleRef`.
    fn classify_by_tag(&self, idx: Idx) -> ArcClass {
        use ori_types::triviality::{classify_triviality, Triviality};
        match classify_triviality(idx, self.pool) {
            Triviality::Trivial => ArcClass::Scalar,
            Triviality::NonTrivial => ArcClass::DefiniteRef,
            Triviality::Unknown => ArcClass::PossibleRef,
        }
    }
}

impl ArcClassification for ArcClassifier<'_> {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        debug_assert!(
            self.classifying.borrow().is_empty(),
            "classifying set should be empty at top-level arc_class() entry"
        );
        self.classify(idx)
    }
}

#[cfg(test)]
mod tests;
