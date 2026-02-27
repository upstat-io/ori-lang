//! Type-specific accessor methods for [`Pool`].
//!
//! Provides read access to compound type structures (functions, tuples, structs,
//! enums, etc.) stored in the pool's extra array.

use crate::{Idx, LifetimeId, Tag};

use super::{Pool, VarState};

impl Pool {
    // === Function Accessors ===

    /// Get function parameter count.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_param_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx] as usize
    }

    /// Get a function parameter type by index.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type or if `param_idx` is out of bounds.
    pub fn function_param(&self, idx: Idx, param_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        debug_assert!(param_idx < count);
        Idx::from_raw(self.extra[extra_idx + 1 + param_idx])
    }

    /// Get function parameter types as a Vec.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_params(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 1 + i]))
            .collect()
    }

    /// Get function return type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Function type.
    pub fn function_return(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx::from_raw(self.extra[extra_idx + 1 + count])
    }

    // === Tuple Accessors ===

    /// Get tuple element count.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type.
    pub fn tuple_elem_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx] as usize
    }

    /// Get a tuple element type by index.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type or if `elem_idx` is out of bounds.
    pub fn tuple_elem(&self, idx: Idx, elem_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        debug_assert!(elem_idx < count);
        Idx::from_raw(self.extra[extra_idx + 1 + elem_idx])
    }

    /// Get tuple element types as a Vec.
    ///
    /// # Panics
    /// Panics if `idx` is not a Tuple type.
    pub fn tuple_elems(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Tuple);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 1 + i]))
            .collect()
    }

    /// Look up an existing tuple type without creating it.
    ///
    /// Returns `Some(idx)` if the tuple `(elems...)` was previously interned
    /// (e.g. by the type checker), `None` otherwise.
    pub fn find_tuple(&self, elems: &[Idx]) -> Option<Idx> {
        if elems.is_empty() {
            return Some(Idx::UNIT);
        }
        let mut extra = Vec::with_capacity(elems.len() + 1);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "element count fits u32 — pool layout uses u32 words"
        )]
        extra.push(elems.len() as u32);
        for &e in elems {
            extra.push(e.raw());
        }
        let hash = self.merkle_hash(Tag::Tuple, 0, &extra);
        self.intern_map.get(&hash).copied()
    }

    // === Map/Result Accessors ===

    /// Get map key type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Map type.
    pub fn map_key(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Map);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get map value type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Map type.
    pub fn map_value(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Map);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx + 1])
    }

    /// Get result ok type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Result type.
    pub fn result_ok(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Result);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get result error type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Result type.
    pub fn result_err(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Result);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx + 1])
    }

    // === Borrowed & Container Accessors ===

    /// Get the inner type of a borrowed reference.
    ///
    /// For `&T`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Borrowed type.
    pub fn borrowed_inner(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Borrowed);
        let extra_idx = self.data(idx) as usize;
        Idx::from_raw(self.extra[extra_idx])
    }

    /// Get the lifetime of a borrowed reference.
    ///
    /// # Panics
    /// Panics if `idx` is not a Borrowed type.
    pub fn borrowed_lifetime(&self, idx: Idx) -> LifetimeId {
        debug_assert_eq!(self.tag(idx), Tag::Borrowed);
        let extra_idx = self.data(idx) as usize;
        LifetimeId::from_raw(self.extra[extra_idx + 1])
    }

    /// Get option inner type.
    ///
    /// For `Option<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not an Option type.
    pub fn option_inner(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Option);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get list element type.
    ///
    /// For `[T]`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a List type.
    pub fn list_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::List);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get range element type.
    ///
    /// For `Range<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Range type.
    pub fn range_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Range);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get set element type.
    ///
    /// For `Set<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Set type.
    pub fn set_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Set);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get channel element type.
    ///
    /// For `chan<T>`, returns `T`.
    ///
    /// # Panics
    /// Panics if `idx` is not a Channel type.
    pub fn channel_elem(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Channel);
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    /// Get iterator element type.
    ///
    /// Works for both `Iterator<T>` and `DoubleEndedIterator<T>`.
    ///
    /// # Panics
    /// Panics if `idx` is not an iterator type.
    pub fn iterator_elem(&self, idx: Idx) -> Idx {
        debug_assert!(
            self.tag(idx).is_iterator(),
            "expected Iterator or DoubleEndedIterator, got {:?}",
            self.tag(idx)
        );
        // Simple container: data field is the child index directly
        Idx::from_raw(self.data(idx))
    }

    // === Scheme/Generic Accessors ===

    /// Get scheme quantified variable IDs.
    ///
    /// # Panics
    /// Panics if `idx` is not a Scheme type.
    pub fn scheme_vars(&self, idx: Idx) -> &[u32] {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        &self.extra[extra_idx + 1..extra_idx + 1 + count]
    }

    /// Get scheme body type.
    ///
    /// # Panics
    /// Panics if `idx` is not a Scheme type.
    pub fn scheme_body(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Scheme);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx::from_raw(self.extra[extra_idx + 1 + count])
    }

    /// Get the name of an applied generic type.
    ///
    /// For `List<int>`, returns the `Name` for "List".
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        // Name is stored as two u32s for future 64-bit expansion,
        // but currently only uses the low 32 bits
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }

    /// Get the number of type arguments for an applied type.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_arg_count(&self, idx: Idx) -> usize {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        self.extra[extra_idx + 2] as usize
    }

    /// Get a specific type argument by index.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type or `arg_idx` is out of bounds.
    pub fn applied_arg(&self, idx: Idx, arg_idx: usize) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;
        debug_assert!(arg_idx < count);
        Idx::from_raw(self.extra[extra_idx + 3 + arg_idx])
    }

    /// Get all type arguments for an applied type.
    ///
    /// For `Map<str, int>`, returns `[Idx::STR, Idx::INT]`.
    ///
    /// # Panics
    /// Panics if `idx` is not an Applied type.
    pub fn applied_args(&self, idx: Idx) -> Vec<Idx> {
        debug_assert_eq!(self.tag(idx), Tag::Applied);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx + 2] as usize;

        (0..count)
            .map(|i| Idx::from_raw(self.extra[extra_idx + 3 + i]))
            .collect()
    }

    /// Get the name of a named type reference.
    ///
    /// # Panics
    /// Panics if `idx` is not a Named type.
    pub fn named_name(&self, idx: Idx) -> ori_ir::Name {
        debug_assert_eq!(self.tag(idx), Tag::Named);
        let extra_idx = self.data(idx) as usize;
        // Name is stored as two u32s for future 64-bit expansion,
        // but currently only uses the low 32 bits
        let name_lo = self.extra[extra_idx];
        ori_ir::Name::from_raw(name_lo)
    }

    // === Named Type Resolution ===

    /// Register a resolution from a Named/Applied type to its concrete definition.
    ///
    /// After type registration creates a Pool Struct/Enum entry, this links the
    /// Named Idx (from `pool.named(name)`) to the concrete Struct/Enum Idx so
    /// codegen can resolve types without accessing `TypeRegistry`.
    pub fn set_resolution(&mut self, named: Idx, concrete: Idx) {
        self.resolutions.insert(named, concrete);
    }

    /// Resolve a Named/Applied type to its concrete Struct/Enum definition.
    ///
    /// Follows resolution chains (e.g., alias -> named -> struct) with a depth
    /// limit of 16 to prevent infinite loops from cyclic references.
    ///
    /// Returns `None` if no resolution exists (e.g., generic type parameters
    /// that are only resolved during monomorphization).
    pub fn resolve(&self, idx: Idx) -> Option<Idx> {
        const MAX_DEPTH: u32 = 16;

        let mut current = idx;
        for _ in 0..MAX_DEPTH {
            match self.resolutions.get(&current) {
                Some(&resolved) => {
                    // If the resolved type points to itself, stop
                    if resolved == current {
                        return Some(resolved);
                    }
                    current = resolved;
                }
                None => {
                    // Only return Some if we followed at least one resolution
                    return if current == idx { None } else { Some(current) };
                }
            }
        }

        // Depth limit hit — return what we have so far
        tracing::warn!(
            idx = ?idx,
            depth = MAX_DEPTH,
            "Resolution chain depth limit reached"
        );
        Some(current)
    }

    /// Resolve a type index by following inference variable links first,
    /// then Named/Applied resolution chains.
    ///
    /// Unlike `resolve()`, which only follows the `resolutions` hashmap,
    /// this method also follows `VarState::Link` chains left by the unifier.
    /// After type checking, inference variables retain their `VarState::Link`
    /// targets in the Pool. This method follows those links to find the
    /// concrete type.
    ///
    /// For `Applied` types with no direct resolution (common for user-defined
    /// types like `Shape` which the type checker records as `Applied("Shape", [])`),
    /// falls back to searching for a `Named` resolution with the same name.
    ///
    /// Returns the fully-resolved type, or the input if no resolution exists.
    pub fn resolve_fully(&self, idx: Idx) -> Idx {
        // Bounds check: return early for indices outside the pool.
        if idx.raw() as usize >= self.items.len() {
            return idx;
        }

        // Step 1: Follow VarState::Link chains (inference variable resolution).
        let mut current = idx;
        for _ in 0..16 {
            if current.raw() as usize >= self.items.len() {
                break;
            }
            if self.tag(current) != Tag::Var {
                break;
            }
            let var_id = self.data(current);
            match self.var_state(var_id) {
                VarState::Link { target } => current = *target,
                _ => break,
            }
        }

        // Step 2: Follow Named/Applied resolution chains.
        if let Some(resolved) = self.resolve(current) {
            return resolved;
        }

        // Step 3: For Applied types, try matching with resolved args.
        //
        // The type checker creates Applied(Pair, [Var(X), Var(Y)]) during inference,
        // where Var(X)/Var(Y) are later unified with concrete types via VarState::Link.
        // But the resolution was registered for Applied(Pair, [Int, Int]) — a different Idx.
        // Resolve the args through Var links and match against registered Applied resolutions.
        if self.tag(current) == Tag::Applied {
            let name = self.applied_name(current);
            let args = self.applied_args(current);

            // Resolve each arg through Var links to get the canonical types.
            let resolved_args: Vec<Idx> = args.iter().map(|&a| self.resolve_fully(a)).collect();

            // Check for an Applied resolution whose resolved args match.
            // Both sides must be resolved: the input's args may be Var-linked,
            // and the key's args may be Applied types that resolve further.
            // Example: Wrapper<Pair<int, bool>> — input arg resolves to a
            // concrete Pair struct, and the key's arg Applied(Pair, [int, bool])
            // also resolves to the same struct.
            for &key in self.resolutions.keys() {
                if self.tag(key) == Tag::Applied && self.applied_name(key) == name {
                    let key_args = self.applied_args(key);
                    if key_args.len() == resolved_args.len()
                        && key_args
                            .iter()
                            .zip(&resolved_args)
                            .all(|(k, r)| self.resolve_fully(*k) == *r)
                    {
                        if let Some(concrete) = self.resolve(key) {
                            return concrete;
                        }
                    }
                }
            }

            // Fall back to Named resolution (non-generic Applied -> Named struct).
            for &key in self.resolutions.keys() {
                if self.tag(key) == Tag::Named && self.named_name(key) == name {
                    if let Some(concrete) = self.resolve(key) {
                        return concrete;
                    }
                }
            }
        }

        current
    }

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
}
