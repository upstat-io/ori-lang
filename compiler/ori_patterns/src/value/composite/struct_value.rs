//! Struct value types: `StructLayout` and `StructValue`.

// Arc is used for immutable sharing of struct fields and layout between clones
#![expect(
    clippy::disallowed_types,
    reason = "Arc for immutable sharing of struct fields and layout"
)]

use rustc_hash::FxHashMap;
use std::sync::Arc;

use ori_ir::Name;

use crate::value::Value;

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

    /// Iterate over (`field_name`, `field_index`) pairs.
    pub fn field_names(&self) -> impl Iterator<Item = (Name, usize)> + '_ {
        self.field_indices.iter().map(|(&name, &idx)| (name, idx))
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
