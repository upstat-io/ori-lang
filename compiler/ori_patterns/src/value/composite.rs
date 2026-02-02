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

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};

use ori_ir::{ExprArena, ExprId, Name, SharedArena};

use super::Value;

// StructLayout

/// Layout information for O(1) struct field access.
#[derive(Clone, Debug)]
pub struct StructLayout {
    /// Map from field name to index.
    field_indices: HashMap<Name, usize>,
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
    pub fn new(name: Name, field_values: HashMap<Name, Value>) -> Self {
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
/// # Arena Requirement (Thread Safety)
/// Every function carries its own arena reference. This is required for thread
/// safety in parallel execution - when functions are called from different
/// contexts (e.g., parallel test runner), they must use their own arena to
/// resolve `ExprId` values correctly.
#[derive(Clone)]
pub struct FunctionValue {
    /// Parameter names.
    pub params: Vec<Name>,
    /// Body expression.
    pub body: ExprId,
    /// Captured environment (frozen at creation).
    ///
    /// No `RwLock` needed since captures are immutable after creation.
    captures: Arc<HashMap<Name, Value>>,
    /// Arena for expression resolution.
    ///
    /// Required for thread safety - the body `ExprId` must be resolved
    /// against this arena, not whatever arena happens to be in scope
    /// at call time.
    arena: SharedArena,
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
    /// * `body` - Body expression ID
    /// * `captures` - Captured environment (frozen at creation)
    /// * `arena` - Arena for expression resolution (required for thread safety)
    pub fn new(
        params: Vec<Name>,
        body: ExprId,
        captures: HashMap<Name, Value>,
        arena: SharedArena,
    ) -> Self {
        FunctionValue {
            params,
            body,
            captures: Arc::new(captures),
            arena,
            capabilities: Vec::new(),
        }
    }

    /// Create a function value with capabilities.
    ///
    /// # Arguments
    /// * `params` - Parameter names
    /// * `body` - Body expression ID
    /// * `captures` - Captured environment (frozen at creation)
    /// * `arena` - Arena for expression resolution (required for thread safety)
    /// * `capabilities` - Required capabilities from `uses` clause
    pub fn with_capabilities(
        params: Vec<Name>,
        body: ExprId,
        captures: HashMap<Name, Value>,
        arena: SharedArena,
        capabilities: Vec<Name>,
    ) -> Self {
        FunctionValue {
            params,
            body,
            captures: Arc::new(captures),
            arena,
            capabilities,
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
    /// * `body` - Body expression ID
    /// * `captures` - Shared captured environment
    /// * `arena` - Arena for expression resolution (required for thread safety)
    /// * `capabilities` - Required capabilities from `uses` clause
    pub fn with_shared_captures(
        params: Vec<Name>,
        body: ExprId,
        captures: Arc<HashMap<Name, Value>>,
        arena: SharedArena,
        capabilities: Vec<Name>,
    ) -> Self {
        FunctionValue {
            params,
            body,
            captures,
            arena,
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

    /// Get the arena for this function.
    pub fn arena(&self) -> &ExprArena {
        &self.arena
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
            .field("body", &self.body)
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
    cache: Arc<RwLock<HashMap<MemoKey, Value>>>,
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
            cache: Arc::new(RwLock::new(HashMap::new())),
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
// cast_possible_wrap: Tests use small literal values (0-10) that fit in i64 without wrapping
#[allow(clippy::cast_possible_wrap)]
mod tests {
    use super::*;
    use ori_ir::ExprArena;

    fn dummy_arena() -> SharedArena {
        SharedArena::new(ExprArena::new())
    }

    #[test]
    fn test_range_exclusive() {
        let range = RangeValue::exclusive(0, 5);
        let values: Vec<_> = range.iter().collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4]);
        assert_eq!(range.len(), 5);
        assert!(range.contains(0));
        assert!(range.contains(4));
        assert!(!range.contains(5));
    }

    #[test]
    fn test_range_inclusive() {
        let range = RangeValue::inclusive(0, 5);
        let values: Vec<_> = range.iter().collect();
        assert_eq!(values, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(range.len(), 6);
        assert!(range.contains(5));
    }

    #[test]
    fn test_function_value_new() {
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        assert!(func.params.is_empty());
        assert!(!func.has_captures());
    }

    #[test]
    fn test_function_value_with_captures() {
        let mut captures = HashMap::new();
        captures.insert(Name::new(0, 1), Value::int(42));
        let func = FunctionValue::new(vec![], ExprId::new(0), captures, dummy_arena());
        assert!(func.has_captures());
        assert_eq!(func.get_capture(Name::new(0, 1)), Some(&Value::int(42)));
    }

    // Edge case tests for None cases

    #[test]
    fn test_struct_layout_get_index_missing_field() {
        let field_names = vec![Name::new(0, 1), Name::new(0, 2)];
        let layout = StructLayout::new(&field_names);
        // Query a field that doesn't exist
        let missing_field = Name::new(0, 999);
        assert_eq!(layout.get_index(missing_field), None);
    }

    #[test]
    fn test_struct_layout_get_index_existing_field() {
        let field_a = Name::new(0, 1);
        let field_b = Name::new(0, 2);
        let layout = StructLayout::new(&[field_a, field_b]);
        assert!(layout.get_index(field_a).is_some());
        assert!(layout.get_index(field_b).is_some());
    }

    #[test]
    fn test_struct_value_get_field_missing() {
        let type_name = Name::new(0, 100);
        let field_a = Name::new(0, 1);
        let mut fields = HashMap::new();
        fields.insert(field_a, Value::int(42));
        let sv = StructValue::new(type_name, fields);

        // Query a field that doesn't exist
        let missing_field = Name::new(0, 999);
        assert_eq!(sv.get_field(missing_field), None);
    }

    #[test]
    fn test_struct_value_get_field_existing() {
        let type_name = Name::new(0, 100);
        let field_a = Name::new(0, 1);
        let mut fields = HashMap::new();
        fields.insert(field_a, Value::int(42));
        let sv = StructValue::new(type_name, fields);

        assert_eq!(sv.get_field(field_a), Some(&Value::int(42)));
    }

    #[test]
    fn test_function_value_get_capture_missing() {
        let mut captures = HashMap::new();
        captures.insert(Name::new(0, 1), Value::int(42));
        let func = FunctionValue::new(vec![], ExprId::new(0), captures, dummy_arena());

        // Query a capture that doesn't exist
        let missing_name = Name::new(0, 999);
        assert_eq!(func.get_capture(missing_name), None);
    }

    #[test]
    fn test_memoized_function_get_cached_uncached() {
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        let memoized = MemoizedFunctionValue::new(func);

        // Query with args that haven't been cached
        let args = vec![Value::int(1), Value::int(2)];
        assert_eq!(memoized.get_cached(&args), None);
    }

    #[test]
    fn test_memoized_function_cache_and_retrieve() {
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        let memoized = MemoizedFunctionValue::new(func);

        // Cache a result
        let args = vec![Value::int(1), Value::int(2)];
        let result = Value::int(3);
        memoized.cache_result(&args, result.clone());

        // Retrieve it
        assert_eq!(memoized.get_cached(&args), Some(result));
        assert_eq!(memoized.cache_size(), 1);
    }

    #[test]
    fn test_memoized_function_different_args_not_cached() {
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        let memoized = MemoizedFunctionValue::new(func);

        // Cache with one set of args
        let args1 = vec![Value::int(1)];
        memoized.cache_result(&args1, Value::int(10));

        // Query with different args
        let args2 = vec![Value::int(2)];
        assert_eq!(memoized.get_cached(&args2), None);
    }

    #[test]
    fn test_memoized_function_cache_eviction() {
        use super::MAX_MEMO_CACHE_SIZE;

        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        let memoized = MemoizedFunctionValue::new(func);

        // Fill the cache to capacity
        for i in 0..MAX_MEMO_CACHE_SIZE {
            let args = vec![Value::int(i as i64)];
            memoized.cache_result(&args, Value::int(i as i64 * 10));
        }
        assert_eq!(memoized.cache_size(), MAX_MEMO_CACHE_SIZE);

        // Verify first entry is still present
        assert_eq!(memoized.get_cached(&[Value::int(0)]), Some(Value::int(0)));

        // Add one more entry - should evict the oldest (key 0)
        let new_args = vec![Value::int(MAX_MEMO_CACHE_SIZE as i64)];
        memoized.cache_result(&new_args, Value::int(999));

        // Size should still be at capacity
        assert_eq!(memoized.cache_size(), MAX_MEMO_CACHE_SIZE);

        // First entry should be evicted
        assert_eq!(memoized.get_cached(&[Value::int(0)]), None);

        // New entry should be present
        assert_eq!(
            memoized.get_cached(&[Value::int(MAX_MEMO_CACHE_SIZE as i64)]),
            Some(Value::int(999))
        );

        // Entry 1 (second oldest) should still be present
        assert_eq!(memoized.get_cached(&[Value::int(1)]), Some(Value::int(10)));
    }

    #[test]
    fn test_memoized_function_cache_update_no_eviction() {
        let func = FunctionValue::new(vec![], ExprId::new(0), HashMap::new(), dummy_arena());
        let memoized = MemoizedFunctionValue::new(func);

        // Cache initial value
        let args = vec![Value::int(42)];
        memoized.cache_result(&args, Value::int(100));
        assert_eq!(memoized.cache_size(), 1);

        // Update same key - should not increase size or cause eviction
        memoized.cache_result(&args, Value::int(200));
        assert_eq!(memoized.cache_size(), 1);
        assert_eq!(memoized.get_cached(&args), Some(Value::int(200)));
    }
}
