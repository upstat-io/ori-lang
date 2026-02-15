//! Composite value types: structs, functions, and ranges.
//!
//! These types are more complex than primitive values and have
//! their own internal structure.

// Arc is used for immutable sharing of captures between function values
// RwLock is used for memoization cache in MemoizedFunctionValue
#![expect(
    clippy::disallowed_types,
    reason = "Arc for immutable HashMap sharing, RwLock for memoization cache"
)]

use rustc_hash::FxHashMap;
use std::collections::VecDeque;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

use ori_ir::canon::{CanId, SharedCanonResult};
use ori_ir::{ExprArena, Name, SharedArena};

use super::Value;

// StructLayout

/// Layout information for O(1) struct field access.
#[derive(Clone, Debug)]
pub struct StructLayout {
    /// Map from field name to index.
    field_indices: FxHashMap<Name, usize>,
}

impl StructLayout {
    /// Create a new struct layout from field names.
    pub fn new(field_names: &[Name]) -> Self {
        let field_indices = field_names
            .iter()
            .enumerate()
            .map(|(i, name)| (*name, i))
            .collect();
        StructLayout { field_indices }
    }

    /// Get the index of a field by name.
    pub fn get_index(&self, field: Name) -> Option<usize> {
        self.field_indices.get(&field).copied()
    }

    /// Get the number of fields.
    pub fn len(&self) -> usize {
        self.field_indices.len()
    }

    /// Check if the layout has no fields.
    pub fn is_empty(&self) -> bool {
        self.field_indices.is_empty()
    }

    /// Iterate over field names and their indices.
    pub fn iter(&self) -> impl Iterator<Item = (Name, usize)> + '_ {
        self.field_indices.iter().map(|(&n, &i)| (n, i))
    }
}

// StructValue

/// Struct instance with efficient field access.
#[derive(Clone, Debug)]
pub struct StructValue {
    /// Type name of the struct.
    pub type_name: Name,
    /// Field values in layout order.
    pub fields: Arc<Vec<Value>>,
    /// Layout for O(1) field access.
    pub layout: Arc<StructLayout>,
}

impl StructValue {
    /// Create a new struct value from a name and field values.
    pub fn new(name: Name, field_values: FxHashMap<Name, Value>) -> Self {
        let mut field_names: Vec<Name> = field_values.keys().copied().collect();
        field_names.sort();
        let layout = Arc::new(StructLayout::new(&field_names));
        let mut fields = vec![Value::Void; field_names.len()];
        for (name, value) in field_values {
            if let Some(idx) = layout.get_index(name) {
                fields[idx] = value;
            }
        }
        StructValue {
            type_name: name,
            fields: Arc::new(fields),
            layout,
        }
    }

    /// Alias for `type_name` field access.
    pub fn name(&self) -> Name {
        self.type_name
    }

    /// Get a field value by name with O(1) lookup.
    pub fn get_field(&self, field: Name) -> Option<&Value> {
        let index = self.layout.get_index(field)?;
        self.fields.get(index)
    }
}

// FunctionValue

/// Function value (closure).
///
/// # Immutable Captures
/// Captures are frozen at closure creation time. Unlike the previous design
/// that used `RwLock`, this design uses a plain `Arc<HashMap>` for captures.
/// This eliminates potential race conditions and simplifies reasoning about
/// closure behavior.
///
/// # Canonical Evaluation
///
/// All evaluation goes through `eval_can(CanId)` using `can_body`/`canon`.
/// The `arena` field is retained for `create_function_interpreter` which
/// needs it for arena threading during function calls.
#[derive(Clone)]
pub struct FunctionValue {
    /// Parameter names.
    pub params: Vec<Name>,
    /// Canonical body expression. The evaluator dispatches on `CanExpr`
    /// from the canonical arena instead of `ExprKind` from `ExprArena`.
    pub can_body: CanId,
    /// Captured environment (frozen at creation).
    ///
    /// No `RwLock` needed since captures are immutable after creation.
    captures: Arc<FxHashMap<Name, Value>>,
    /// Arena for expression resolution (needed for `create_function_interpreter`).
    arena: SharedArena,
    /// Canonical IR for this function's body.
    ///
    /// When set, `can_body` indexes into this result's `CanArena`.
    /// Functions created from canonicalized modules have this; lambdas
    /// inherit it from their enclosing function.
    canon: Option<SharedCanonResult>,
    /// Default expressions for each parameter.
    /// `can_defaults[i]` is `Some(can_id)` if parameter `i` has a default value.
    can_defaults: Vec<Option<CanId>>,
    /// Required capabilities (from `uses` clause).
    ///
    /// When calling this function, capabilities with these names must be
    /// available in the calling scope and will be passed to the function's scope.
    capabilities: Vec<Name>,
}

impl FunctionValue {
    /// Create a new function value.
    ///
    /// # Arguments
    /// * `params` - Parameter names
    /// * `captures` - Captured environment (frozen at creation)
    /// * `arena` - Arena for expression resolution (required for thread safety)
    pub fn new(params: Vec<Name>, captures: FxHashMap<Name, Value>, arena: SharedArena) -> Self {
        FunctionValue {
            params,
            can_body: CanId::INVALID,
            captures: Arc::new(captures),
            arena,
            canon: None,
            can_defaults: Vec::new(),
            capabilities: Vec::new(),
        }
    }

    /// Create a function value with capabilities.
    ///
    /// # Arguments
    /// * `params` - Parameter names
    /// * `captures` - Captured environment (frozen at creation)
    /// * `arena` - Arena for expression resolution (required for thread safety)
    /// * `capabilities` - Required capabilities from `uses` clause
    pub fn with_capabilities(
        params: Vec<Name>,
        captures: FxHashMap<Name, Value>,
        arena: SharedArena,
        capabilities: Vec<Name>,
    ) -> Self {
        FunctionValue {
            params,
            can_body: CanId::INVALID,
            captures: Arc::new(captures),
            arena,
            canon: None,
            can_defaults: Vec::new(),
            capabilities,
        }
    }

    /// Count the number of required parameters (those without defaults).
    pub fn required_param_count(&self) -> usize {
        if self.can_defaults.is_empty() {
            // No defaults set — all parameters are required
            self.params.len()
        } else {
            self.can_defaults.iter().filter(|d| d.is_none()).count()
        }
    }

    /// Create a function value with shared captures.
    ///
    /// Use this when multiple functions should share the same captures
    /// (e.g., module functions for mutual recursion). This avoids cloning
    /// the captures `HashMap` for each function.
    ///
    /// # Arguments
    /// * `params` - Parameter names
    /// * `captures` - Shared captured environment
    /// * `arena` - Arena for expression resolution (required for thread safety)
    /// * `capabilities` - Required capabilities from `uses` clause
    pub fn with_shared_captures(
        params: Vec<Name>,
        captures: Arc<FxHashMap<Name, Value>>,
        arena: SharedArena,
        capabilities: Vec<Name>,
    ) -> Self {
        FunctionValue {
            params,
            can_body: CanId::INVALID,
            captures,
            arena,
            canon: None,
            can_defaults: Vec::new(),
            capabilities,
        }
    }

    /// Get a captured value by name.
    pub fn get_capture(&self, name: Name) -> Option<&Value> {
        self.captures.get(&name)
    }

    /// Iterate over all captures.
    pub fn captures(&self) -> impl Iterator<Item = (&Name, &Value)> {
        self.captures.iter()
    }

    /// Check if this function has any captures.
    pub fn has_captures(&self) -> bool {
        !self.captures.is_empty()
    }

    /// Get the legacy arena for this function (for multi-clause pattern matching).
    pub fn arena(&self) -> &ExprArena {
        &self.arena
    }

    /// Get the shared arena reference for O(1) Arc cloning.
    ///
    /// Use this instead of `arena().clone()` to avoid deep-cloning the `ExprArena`.
    /// `SharedArena` is `Arc<ExprArena>`, so `.clone()` is an atomic increment.
    pub fn shared_arena(&self) -> &SharedArena {
        &self.arena
    }

    /// Get the canonical IR for this function, if available.
    pub fn canon(&self) -> Option<&SharedCanonResult> {
        self.canon.as_ref()
    }

    /// Set canonical IR for this function.
    ///
    /// Called after construction to attach canonical data without modifying
    /// every constructor's signature. The `can_body` must index into the
    /// `canon` result's `CanArena`.
    pub fn set_canon(&mut self, can_body: CanId, canon: SharedCanonResult) {
        self.can_body = can_body;
        self.canon = Some(canon);
    }

    /// Set canonical default expressions for this function's parameters.
    ///
    /// Called after construction to attach canonicalized defaults without modifying
    /// every constructor. The `CanId` values index into the function's `canon` arena.
    pub fn set_can_defaults(&mut self, can_defaults: Vec<Option<CanId>>) {
        debug_assert!(
            can_defaults.is_empty() || can_defaults.len() == self.params.len(),
            "can_defaults length must match params length"
        );
        self.can_defaults = can_defaults;
    }

    /// Get the canonical default expressions for this function's parameters.
    pub fn can_defaults(&self) -> &[Option<CanId>] {
        &self.can_defaults
    }

    /// Get the required capabilities for this function.
    pub fn capabilities(&self) -> &[Name] {
        &self.capabilities
    }

    /// Check if this function requires any capabilities.
    pub fn has_capabilities(&self) -> bool {
        !self.capabilities.is_empty()
    }
}

impl fmt::Debug for FunctionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FunctionValue")
            .field("params", &self.params)
            .field("can_body", &self.can_body)
            .field("captures", &format!("{} bindings", self.captures.len()))
            .finish_non_exhaustive()
    }
}

// MemoizedFunctionValue

/// A wrapper around a cache key for memoization.
///
/// Uses a vector of values as the key, with custom Hash implementation
/// that hashes each element.
///
/// # Performance
/// Implements `Borrow<[Value]>` to enable zero-allocation cache lookups
/// using `&[Value]` slices.
#[derive(Clone, PartialEq, Eq)]
pub struct MemoKey(pub Vec<Value>);

impl Hash for MemoKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Must match the Hash impl for [Value] to work with Borrow trait.
        // Vec<T>::hash uses Hash::hash_slice which hashes all elements.
        self.0.hash(state);
    }
}

impl std::borrow::Borrow<[Value]> for MemoKey {
    fn borrow(&self) -> &[Value] {
        &self.0
    }
}

/// Maximum number of entries in a memoization cache.
///
/// When this limit is reached, the oldest entries are evicted to make room
/// for new ones. This prevents unbounded memory growth for functions called
/// with many distinct argument combinations.
///
/// The limit of 100,000 entries is chosen to:
/// - Allow efficient memoization of typical recursive algorithms (e.g., fib(1000))
/// - Prevent runaway memory consumption in pathological cases
/// - Provide reasonable memory bounds (~10-100MB depending on value sizes)
pub const MAX_MEMO_CACHE_SIZE: usize = 100_000;

/// Memoized function value.
///
/// Wraps a `FunctionValue` with a shared cache that stores computed results.
/// This enables efficient recursive algorithms by avoiding redundant computation.
///
/// # Cache Bounds
/// The cache is bounded to [`MAX_MEMO_CACHE_SIZE`] entries. When full, the oldest
/// entries are evicted (FIFO order) to make room for new ones.
///
/// # Thread Safety
/// The cache uses `RwLock` for thread-safe access. Multiple threads can read
/// cached values concurrently, and writes are synchronized.
///
/// # Salsa Compliance
/// This type uses `Arc<RwLock<HashMap>>` for the memoization cache, which contains
/// interior mutability. This type should NOT flow into Salsa query results, as
/// Salsa requires deterministic, hashable types for query caching. The memoization
/// cache is for runtime evaluation optimization only, separate from Salsa's
/// compile-time query caching.
#[derive(Clone)]
pub struct MemoizedFunctionValue {
    /// The underlying function to memoize.
    pub func: FunctionValue,
    /// Shared cache mapping arguments to results.
    ///
    /// The cache is shared across all clones of this memoized function,
    /// enabling recursive calls to benefit from cached results.
    ///
    /// Uses `Arc<RwLock>` for thread-safe caching during evaluation.
    /// This cache is NOT part of Salsa's query system.
    cache: Arc<RwLock<FxHashMap<MemoKey, Value>>>,
    /// Insertion order for FIFO eviction.
    ///
    /// When the cache reaches [`MAX_MEMO_CACHE_SIZE`], entries are evicted
    /// in FIFO order (oldest first) to make room for new entries.
    insertion_order: Arc<RwLock<VecDeque<MemoKey>>>,
}

impl MemoizedFunctionValue {
    /// Create a new memoized function wrapper.
    pub fn new(func: FunctionValue) -> Self {
        MemoizedFunctionValue {
            func,
            cache: Arc::new(RwLock::new(FxHashMap::default())),
            insertion_order: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Look up a cached result for the given arguments.
    ///
    /// Returns `Some(value)` if the result is cached, `None` otherwise.
    ///
    /// # Performance
    /// Uses `Borrow<[Value]>` for zero-allocation lookups - no Vec allocation needed.
    pub fn get_cached(&self, args: &[Value]) -> Option<Value> {
        self.cache.read().ok()?.get(args).cloned()
    }

    /// Store a result in the cache.
    ///
    /// If the cache has reached [`MAX_MEMO_CACHE_SIZE`], the oldest entries
    /// are evicted (FIFO order) to make room for the new entry.
    ///
    /// Note: This still allocates for the key since we need to own it for storage.
    pub fn cache_result(&self, args: &[Value], result: Value) {
        // Acquire both locks to ensure consistency
        let (Ok(mut cache), Ok(mut order)) = (self.cache.write(), self.insertion_order.write())
        else {
            return;
        };

        // Fast path: update existing entry (no clone, no eviction)
        if let Some(existing) = cache.get_mut(args) {
            *existing = result;
            return;
        }

        // Evict oldest entries if at capacity
        while cache.len() >= MAX_MEMO_CACHE_SIZE {
            if let Some(oldest_key) = order.pop_front() {
                cache.remove(&oldest_key);
            } else {
                // Order is empty but cache is full - shouldn't happen, but clear to recover
                cache.clear();
                break;
            }
        }

        // Insert new entry (one allocation for key)
        let key = MemoKey(args.to_vec());
        order.push_back(key.clone());
        cache.insert(key, result);
    }

    /// Get the number of cached entries.
    #[cfg(test)]
    pub fn cache_size(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }
}

impl fmt::Debug for MemoizedFunctionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cache_size = self.cache.read().map(|c| c.len()).unwrap_or(0);
        f.debug_struct("MemoizedFunctionValue")
            .field("func", &self.func)
            .field("cache_entries", &cache_size)
            // insertion_order is an internal implementation detail for FIFO eviction
            .finish_non_exhaustive()
    }
}

// RangeValue

/// Range value.
#[derive(Clone, Debug)]
pub struct RangeValue {
    /// Start of range (inclusive).
    pub start: i64,
    /// End of range.
    pub end: i64,
    /// Step increment (default 1). Can be negative for descending ranges.
    pub step: i64,
    /// Whether end is inclusive.
    pub inclusive: bool,
}

impl RangeValue {
    /// Create an exclusive range with step 1.
    pub fn exclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end,
            step: 1,
            inclusive: false,
        }
    }

    /// Create an inclusive range with step 1.
    pub fn inclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end,
            step: 1,
            inclusive: true,
        }
    }

    /// Create an exclusive range with custom step.
    pub fn exclusive_with_step(start: i64, end: i64, step: i64) -> Self {
        RangeValue {
            start,
            end,
            step,
            inclusive: false,
        }
    }

    /// Create an inclusive range with custom step.
    pub fn inclusive_with_step(start: i64, end: i64, step: i64) -> Self {
        RangeValue {
            start,
            end,
            step,
            inclusive: true,
        }
    }

    /// Iterate over the range values.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn iter(&self) -> impl Iterator<Item = i64> {
        let start = self.start;
        let end = self.end;
        let step = self.step;
        let inclusive = self.inclusive;

        // Check if the range is empty (start not within bounds)
        let initial = match step.cmp(&0) {
            std::cmp::Ordering::Greater => {
                if inclusive {
                    if start <= end {
                        Some(start)
                    } else {
                        None
                    }
                } else if start < end {
                    Some(start)
                } else {
                    None
                }
            }
            std::cmp::Ordering::Less => {
                if inclusive {
                    if start >= end {
                        Some(start)
                    } else {
                        None
                    }
                } else if start > end {
                    Some(start)
                } else {
                    None
                }
            }
            std::cmp::Ordering::Equal => None, // step == 0, no iteration
        };

        std::iter::successors(initial, move |&current| {
            let next = current + step;
            match step.cmp(&0) {
                std::cmp::Ordering::Greater => {
                    if inclusive {
                        if next <= end {
                            Some(next)
                        } else {
                            None
                        }
                    } else if next < end {
                        Some(next)
                    } else {
                        None
                    }
                }
                std::cmp::Ordering::Less => {
                    if inclusive {
                        if next >= end {
                            Some(next)
                        } else {
                            None
                        }
                    } else if next > end {
                        Some(next)
                    } else {
                        None
                    }
                }
                std::cmp::Ordering::Equal => None,
            }
        })
    }

    /// Get the length of the range.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn len(&self) -> usize {
        if self.step == 0 {
            return 0;
        }

        let adjusted_end = if self.inclusive {
            if self.step > 0 {
                self.end + 1
            } else {
                self.end - 1
            }
        } else {
            self.end
        };

        let diff = if self.step > 0 {
            (adjusted_end - self.start).max(0)
        } else {
            (self.start - adjusted_end).max(0)
        };

        let step_abs = self.step.abs();
        let count = (diff + step_abs - 1) / step_abs; // ceiling division
        usize::try_from(count).unwrap_or(usize::MAX)
    }

    /// Check if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a value is contained in the range.
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "range bound arithmetic on user-provided i64 values"
    )]
    pub fn contains(&self, value: i64) -> bool {
        // Check bounds first
        let in_bounds = match self.step.cmp(&0) {
            std::cmp::Ordering::Greater => {
                if self.inclusive {
                    value >= self.start && value <= self.end
                } else {
                    value >= self.start && value < self.end
                }
            }
            std::cmp::Ordering::Less => {
                if self.inclusive {
                    value <= self.start && value >= self.end
                } else {
                    value <= self.start && value > self.end
                }
            }
            std::cmp::Ordering::Equal => return false, // step == 0, no values
        };

        if !in_bounds {
            return false;
        }

        // Check alignment with step
        (value - self.start) % self.step == 0
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_wrap,
    reason = "tests use small literal values (0-10) that fit in i64 without wrapping"
)]
mod tests;
