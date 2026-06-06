//! Accessors for nominal user-defined types (Struct, Enum).
//!
//! Struct and enum pool entries use a variable-length payload in `extra`:
//! - Struct: `[name_lo, name_hi, field_count, (f_name, f_type)*]`
//! - Enum: `[name_lo, name_hi, variant_count, (v_name, v_fc, v_ftype*)*]`

use crate::{Idx, Tag};

use super::super::Pool;

impl Pool {
    // === Struct Accessors ===

    /// Get the name of a struct type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Struct type.
    pub fn struct_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Struct);
        let extra_idx = self.data(idx) as usize;
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }

    /// Get the number of fields in a struct type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Struct type.
    pub fn struct_field_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Struct);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx + 2] as usize
    }

    /// Get a single struct field by index.
    ///
    /// Returns `(field_name, field_type)`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Struct type or `field_idx` is out of bounds.
    pub fn struct_field(&self, idx: Idx, field_idx: usize) -> (ori_ir::Name, Idx) {
        debug_assert_eq!(self.tag(idx), Tag::Struct);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;
        debug_assert!(field_idx < count);
        // Fields start at offset 3, each field is 2 words: [name, type]
        let field_offset = extra_idx + 3 + field_idx * 2;
        let name = ori_ir::Name::from_raw(self.extra[field_offset]);
        let ty = Idx::from_raw(self.extra[field_offset + 1]);
        (name, ty)
    }

    /// Get all struct fields as a Vec of `(Name, Idx)` pairs.
    ///
    /// # Panics
    /// Panics if `idx` is not a Struct type.
    pub fn struct_fields(&self, idx: Idx) -> Vec<(ori_ir::Name, Idx)> {
        debug_assert_eq!(self.tag(idx), Tag::Struct);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;

        (0..count)
            .map(|i| {
                let field_offset = extra_idx + 3 + i * 2;
                let name = ori_ir::Name::from_raw(self.extra[field_offset]);
                let ty = Idx::from_raw(self.extra[field_offset + 1]);
                (name, ty)
            })
            .collect()
    }

    // === Enum Accessors ===

    /// Get the name of an enum type.
    ///
    /// # Panics
    /// Panics if `idx` is not an Enum type.
    pub fn enum_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Enum);
        let extra_idx = self.data(idx) as usize;
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }

    /// Get the number of variants in an enum type.
    ///
    /// # Panics
    /// Panics if `idx` is not an Enum type.
    pub fn enum_variant_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Enum);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx + 2] as usize
    }

    /// Get a single enum variant by index.
    ///
    /// Returns `(variant_name, field_types)`.
    ///
    /// Walks the variable-length variant data (O(n) in variant index).
    ///
    /// # Panics
    /// Panics if `idx` is not an Enum type or `variant_idx` is out of bounds.
    pub fn enum_variant(&self, idx: Idx, variant_idx: usize) -> (ori_ir::Name, Vec<Idx>) {
        debug_assert_eq!(self.tag(idx), Tag::Enum);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;
        debug_assert!(variant_idx < count);

        // Walk through variants to find the requested one
        let mut offset = extra_idx + 3;
        for _ in 0..variant_idx {
            let field_count = self.extra[offset + 1] as usize;
            offset += 2 + field_count; // skip name + field_count + field types
        }

        let name = ori_ir::Name::from_raw(self.extra[offset]);
        let field_count = self.extra[offset + 1] as usize;
        let fields = (0..field_count)
            .map(|i| Idx::from_raw(self.extra[offset + 2 + i]))
            .collect();

        (name, fields)
    }

    /// Get all enum variants as a Vec of `(Name, Vec<Idx>)` pairs.
    ///
    /// # Panics
    /// Panics if `idx` is not an Enum type.
    pub fn enum_variants(&self, idx: Idx) -> Vec<(ori_ir::Name, Vec<Idx>)> {
        debug_assert_eq!(self.tag(idx), Tag::Enum);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;

        let mut result = Vec::with_capacity(count);
        let mut offset = extra_idx + 3;

        for _ in 0..count {
            let name = ori_ir::Name::from_raw(self.extra[offset]);
            let field_count = self.extra[offset + 1] as usize;
            let fields = (0..field_count)
                .map(|i| Idx::from_raw(self.extra[offset + 2 + i]))
                .collect();
            result.push((name, fields));
            offset += 2 + field_count;
        }

        result
    }

    /// Whether an aggregate type (struct / enum / tuple / Option / Result) is
    /// SELF-REFERENTIAL — a field/variant-payload chain that reaches the type
    /// itself. A recursive type cannot have a finite inline size, so it is
    /// heap-boxed (the runtime allocates each node). Non-recursive aggregates
    /// are inline (no self-allocation; their heap children allocate separately).
    ///
    /// Returns `false` for non-aggregate tags (scalars, collections, str,
    /// function). Follows nominal resolution (Struct fields, Enum variant
    /// payloads, Tuple elements, Option/Result inners) with cycle detection.
    pub fn aggregate_type_is_recursive(&self, idx: Idx) -> bool {
        let resolved = self.resolve_fully(idx);
        let mut visiting = rustc_hash::FxHashSet::default();
        self.reaches_self(resolved, resolved, &mut visiting)
    }

    /// Whether `current` (or any nominal descendant) resolves to `target`.
    fn reaches_self(
        &self,
        current: Idx,
        target: Idx,
        visiting: &mut rustc_hash::FxHashSet<Idx>,
    ) -> bool {
        let resolved = self.resolve_fully(current);
        if !visiting.insert(resolved) {
            // Already on the walk stack → a cycle that does NOT pass through
            // `target` (target is handled by the `== target` check below).
            return false;
        }
        let hit = self.aggregate_children(resolved).iter().any(|&child| {
            let cr = self.resolve_fully(child);
            cr == target || self.reaches_self(cr, target, visiting)
        });
        visiting.remove(&resolved);
        hit
    }

    /// The directly-nested type indices of an aggregate (Struct fields, Enum
    /// variant payloads, Tuple elements, Option/Result inners). Empty for
    /// non-aggregate tags.
    fn aggregate_children(&self, idx: Idx) -> Vec<Idx> {
        let resolved = self.resolve_fully(idx);
        match self.tag(resolved) {
            Tag::Struct => self
                .struct_fields(resolved)
                .into_iter()
                .map(|(_, t)| t)
                .collect(),
            Tag::Enum => self
                .enum_variants(resolved)
                .into_iter()
                .flat_map(|(_, payloads)| payloads)
                .collect(),
            Tag::Tuple => self.tuple_elems(resolved),
            Tag::Option => vec![self.option_inner(resolved)],
            Tag::Result => vec![self.result_ok(resolved), self.result_err(resolved)],
            _ => Vec::new(),
        }
    }
}
