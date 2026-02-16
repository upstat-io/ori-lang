//! ARC type classifier.
//!
//! Walks the type pool to classify each type as `Scalar`, `DefiniteRef`,
//! or `PossibleRef`. Uses memoization and cycle detection to handle
//! recursive types efficiently.

use std::cell::RefCell;

use rustc_hash::{FxHashMap, FxHashSet};

use ori_types::{Idx, Pool, Tag};

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

    /// Export the classification cache for persistence across runs.
    ///
    /// Returns a snapshot of all classifications computed so far. Feed this
    /// back into [`with_cache`](Self::with_cache) on the next run to avoid
    /// redundant type walks for unchanged modules.
    pub fn export_cache(&self) -> FxHashMap<Idx, ArcClass> {
        self.cache.borrow().clone()
    }

    /// Core classification with caching and cycle detection.
    fn classify(&self, idx: Idx) -> ArcClass {
        // Sentinel: NONE is not a real type, treat as scalar (same as TypeInfoStore).
        if idx == Idx::NONE {
            return ArcClass::Scalar;
        }

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

    /// Classify a non-primitive type by its Pool tag.
    fn classify_by_tag(&self, idx: Idx) -> ArcClass {
        match self.pool.tag(idx) {
            // Scalars (primitives -- caught by fast path, but handle gracefully)
            Tag::Int
            | Tag::Float
            | Tag::Bool
            | Tag::Char
            | Tag::Byte
            | Tag::Unit
            | Tag::Never
            | Tag::Error
            | Tag::Duration
            | Tag::Size
            | Tag::Ordering => ArcClass::Scalar,

            // DefiniteRef: heap-allocated or closure types
            Tag::Str
            | Tag::List
            | Tag::Map
            | Tag::Set
            | Tag::Channel
            | Tag::Function
            | Tag::Iterator
            | Tag::DoubleEndedIterator => ArcClass::DefiniteRef,

            // Transitive: single child
            Tag::Option => self.classify(self.pool.option_inner(idx)),
            Tag::Range => self.classify(self.pool.range_elem(idx)),

            // Transitive: two children
            Tag::Result => {
                self.classify_children(&[self.pool.result_ok(idx), self.pool.result_err(idx)])
            }

            // Transitive: variable children
            Tag::Tuple => self.classify_children(&self.pool.tuple_elems(idx)),

            Tag::Struct => {
                let fields = self.pool.struct_fields(idx);
                let field_types: Vec<Idx> = fields.into_iter().map(|(_, ty)| ty).collect();
                self.classify_children(&field_types)
            }

            Tag::Enum => {
                let variants = self.pool.enum_variants(idx);
                let all_field_types: Vec<Idx> = variants
                    .into_iter()
                    .flat_map(|(_, fields)| fields)
                    .collect();
                self.classify_children(&all_field_types)
            }

            // Named type resolution
            Tag::Named | Tag::Applied | Tag::Alias => {
                match self.pool.resolve(idx) {
                    Some(resolved) => self.classify(resolved),
                    // Unresolved named type — conservative fallback.
                    None => ArcClass::PossibleRef,
                }
            }

            // Type variables, borrowed references, schemes, and special types (conservative)
            Tag::Var
            | Tag::BoundVar
            | Tag::RigidVar
            | Tag::Borrowed
            | Tag::Scheme
            | Tag::Projection
            | Tag::ModuleNs
            | Tag::Infer
            | Tag::SelfType => ArcClass::PossibleRef,
        }
    }

    /// Classify a compound type by its children.
    ///
    /// The transitive rule: if ANY child is `DefiniteRef`, the compound is
    /// `DefiniteRef`. If ANY child is `PossibleRef` (and none is `DefiniteRef`),
    /// the compound is `PossibleRef`. Otherwise `Scalar`.
    ///
    /// After monomorphization, all children should be concrete, so the result
    /// is always either `Scalar` or `DefiniteRef`.
    fn classify_children(&self, children: &[Idx]) -> ArcClass {
        let mut has_possible = false;

        for &child in children {
            match self.classify(child) {
                ArcClass::Scalar => {}
                // Short-circuit: DefiniteRef is the "strongest" classification.
                ArcClass::DefiniteRef => return ArcClass::DefiniteRef,
                ArcClass::PossibleRef => has_possible = true,
            }
        }

        if has_possible {
            ArcClass::PossibleRef
        } else {
            ArcClass::Scalar
        }
    }
}

impl ArcClassification for ArcClassifier<'_> {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        self.classify(idx)
    }
}

#[cfg(test)]
mod tests;
