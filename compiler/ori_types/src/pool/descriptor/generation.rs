//! Descriptor generation: turning a pool [`Idx`] into a portable
//! [`TypeDescriptor`] (and topologically-sorted transitive sequences).

use rustc_hash::FxHashSet;

use crate::{Idx, Tag};

use super::{Pool, TypeDescriptor, VariantDescriptor};

impl Pool {
    /// Generate a portable [`TypeDescriptor`] for a single type.
    ///
    /// The descriptor references children by Merkle hash, not by Idx.
    /// Uses Pool accessor methods internally — does not depend on extra array layout.
    pub fn describe(&self, idx: Idx) -> TypeDescriptor {
        let tag = self.tag(idx);

        // Simple containers: data = child Idx
        if tag.has_child_in_data() {
            let child_idx = Idx::from_raw(self.data(idx));
            return TypeDescriptor::Container {
                tag,
                child_hash: self.hash(child_idx),
            };
        }

        // Complex types with extra data
        if tag.uses_extra() {
            return self.describe_complex(tag, idx);
        }

        // Leaf types: primitives, variables, specials
        if tag.is_type_variable() {
            return TypeDescriptor::Variable {
                tag,
                var_id: self.data(idx),
            };
        }

        // Primitives and other leaves (Error, Never, Infer, SelfType, ModuleNs)
        TypeDescriptor::Primitive(tag)
    }

    /// Generate a descriptor for a complex type (one that uses the extra array).
    ///
    /// Dispatches each tag group to a dedicated helper so this function and
    /// every helper stay small.
    fn describe_complex(&self, tag: Tag, idx: Idx) -> TypeDescriptor {
        match tag {
            Tag::Map | Tag::Result => self.describe_two_child(tag, idx),
            Tag::Borrowed => self.describe_borrowed(idx),
            Tag::Function => self.describe_function(idx),
            Tag::Tuple => self.describe_tuple(idx),
            Tag::Struct => self.describe_struct(idx),
            Tag::Enum => self.describe_enum(idx),
            Tag::Applied => self.describe_applied(idx),
            Tag::Named => TypeDescriptor::Named {
                name: self.named_name(idx),
            },
            Tag::Scheme => self.describe_scheme(idx),
            // Alias, Projection — opaque/transient, treat as primitives
            Tag::Alias | Tag::Projection => TypeDescriptor::Primitive(tag),
            _ => unreachable!(
                "uses_extra() returned true for {tag:?} but describe_complex has no handler"
            ),
        }
    }

    /// Two-child containers (`Map<K, V>`, `Result<T, E>`).
    fn describe_two_child(&self, tag: Tag, idx: Idx) -> TypeDescriptor {
        let (c1, c2) = match tag {
            Tag::Map => (self.map_key(idx), self.map_value(idx)),
            Tag::Result => (self.result_ok(idx), self.result_err(idx)),
            _ => unreachable!(),
        };
        TypeDescriptor::TwoChild {
            tag,
            child1_hash: self.hash(c1),
            child2_hash: self.hash(c2),
        }
    }

    /// Borrowed reference (`&T` with lifetime).
    fn describe_borrowed(&self, idx: Idx) -> TypeDescriptor {
        TypeDescriptor::Borrowed {
            inner_hash: self.hash(self.borrowed_inner(idx)),
            lifetime_id: self.borrowed_lifetime(idx).raw(),
        }
    }

    /// Function type `(P1, P2, ...) -> R`.
    fn describe_function(&self, idx: Idx) -> TypeDescriptor {
        let params = self.function_params(idx);
        let ret = self.function_return(idx);
        TypeDescriptor::Function {
            param_hashes: params.iter().map(|&p| self.hash(p)).collect(),
            return_hash: self.hash(ret),
        }
    }

    /// Tuple type `(T1, T2, ...)`.
    fn describe_tuple(&self, idx: Idx) -> TypeDescriptor {
        let elems = self.tuple_elems(idx);
        TypeDescriptor::Tuple {
            element_hashes: elems.iter().map(|&e| self.hash(e)).collect(),
        }
    }

    /// Struct type `struct Name { fields... }`.
    fn describe_struct(&self, idx: Idx) -> TypeDescriptor {
        let name = self.struct_name(idx);
        let fields = self.struct_fields(idx);
        TypeDescriptor::Struct {
            name,
            field_names: fields.iter().map(|&(n, _)| n).collect(),
            field_type_hashes: fields.iter().map(|&(_, t)| self.hash(t)).collect(),
        }
    }

    /// Enum type `enum Name { variants... }`.
    fn describe_enum(&self, idx: Idx) -> TypeDescriptor {
        let name = self.enum_name(idx);
        let variants = self.enum_variants(idx);
        TypeDescriptor::Enum {
            name,
            variants: variants
                .into_iter()
                .map(|(vname, field_types)| VariantDescriptor {
                    name: vname,
                    field_type_hashes: field_types.iter().map(|&t| self.hash(t)).collect(),
                })
                .collect(),
        }
    }

    /// Applied generic type `Foo<A, B, ...>`.
    fn describe_applied(&self, idx: Idx) -> TypeDescriptor {
        let name = self.applied_name(idx);
        let args = self.applied_args(idx);
        TypeDescriptor::Applied {
            name,
            arg_hashes: args.iter().map(|&a| self.hash(a)).collect(),
        }
    }

    /// Type scheme `forall vars. body`.
    fn describe_scheme(&self, idx: Idx) -> TypeDescriptor {
        let vars = self.scheme_vars(idx);
        let body = self.scheme_body(idx);
        TypeDescriptor::Scheme {
            var_count: vars.len(),
            var_ids: vars.to_vec(),
            body_hash: self.hash(body),
        }
    }

    /// Generate a topologically sorted descriptor sequence for a type and
    /// all its transitive dependencies.
    ///
    /// Leaves (primitives) appear first. Each descriptor's child hashes reference
    /// types that appear earlier in the sequence. Deduplicates by hash.
    ///
    /// Returns `(merkle_hash, descriptor)` pairs.
    pub fn describe_transitive(&self, idx: Idx) -> Vec<(u64, TypeDescriptor)> {
        let mut result = Vec::new();
        let mut visited = FxHashSet::default();
        self.describe_recursive(idx, &mut result, &mut visited);
        result
    }

    /// Recursively describe a type, visiting children first (topological order).
    ///
    /// Public to allow callers (e.g., export descriptor generation) to share
    /// a single `visited` set across multiple root types, avoiding duplicates.
    pub fn describe_recursive(
        &self,
        idx: Idx,
        result: &mut Vec<(u64, TypeDescriptor)>,
        visited: &mut FxHashSet<u64>,
    ) {
        let hash = self.hash(idx);
        if !visited.insert(hash) {
            return; // Already described
        }

        // Recurse into children first (topological order)
        self.visit_children(idx, |child| {
            self.describe_recursive(child, result, visited);
        });

        // Then add self
        result.push((hash, self.describe(idx)));
    }
}
