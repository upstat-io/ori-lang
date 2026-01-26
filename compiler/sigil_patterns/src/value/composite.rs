//! Composite value types: structs, functions, and ranges.
//!
//! These types are more complex than primitive values and have
//! their own internal structure.

// Arc is used for immutable sharing of captures between function values
#![expect(clippy::disallowed_types, reason = "Arc for immutable HashMap sharing in FunctionValue")]

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use sigil_ir::{ExprArena, ExprId, Name, SharedArena};

use super::Value;

// =============================================================================
// StructLayout
// =============================================================================

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

// =============================================================================
// StructValue
// =============================================================================

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
        let field_names: Vec<Name> = field_values.keys().copied().collect();
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

// =============================================================================
// FunctionValue
// =============================================================================

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

// =============================================================================
// RangeValue
// =============================================================================

/// Range value.
#[derive(Clone, Debug)]
pub struct RangeValue {
    /// Start of range (inclusive).
    pub start: i64,
    /// End of range.
    pub end: i64,
    /// Whether end is inclusive.
    pub inclusive: bool,
}

impl RangeValue {
    /// Create an exclusive range.
    pub fn exclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end,
            inclusive: false,
        }
    }

    /// Create an inclusive range.
    pub fn inclusive(start: i64, end: i64) -> Self {
        RangeValue {
            start,
            end,
            inclusive: true,
        }
    }

    /// Iterate over the range values.
    pub fn iter(&self) -> impl Iterator<Item = i64> {
        let end = if self.inclusive { self.end + 1 } else { self.end };
        self.start..end
    }

    /// Get the length of the range.
    pub fn len(&self) -> usize {
        let end = if self.inclusive { self.end + 1 } else { self.end };
        let diff = (end - self.start).max(0);
        usize::try_from(diff).unwrap_or(usize::MAX)
    }

    /// Check if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if a value is contained in the range.
    pub fn contains(&self, value: i64) -> bool {
        if self.inclusive {
            value >= self.start && value <= self.end
        } else {
            value >= self.start && value < self.end
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sigil_ir::ExprArena;

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
        captures.insert(Name::new(0, 1), Value::Int(42));
        let func = FunctionValue::new(vec![], ExprId::new(0), captures, dummy_arena());
        assert!(func.has_captures());
        assert_eq!(func.get_capture(Name::new(0, 1)), Some(&Value::Int(42)));
    }
}
