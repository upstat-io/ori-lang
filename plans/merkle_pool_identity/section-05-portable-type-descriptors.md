---
section: "05"
title: "Portable Type Descriptors"
status: complete
goal: "Enable zero-AST type reconstruction across module boundaries via self-contained, pool-independent type descriptors"
inspired_by:
  - "Git pack files — self-contained object bundles for efficient transfer"
  - "Protocol Buffers / FlatBuffers — self-describing, schema-independent serialization"
  - "Nix derivations — content-addressed build artifacts that carry their own dependency graph"
sections:
  - id: "05.1"
    title: "TypeDescriptor Design"
    status: complete
  - id: "05.2"
    title: "Descriptor Generation"
    status: complete
  - id: "05.3"
    title: "Reconstruction Algorithm"
    status: complete
  - id: "05.4"
    title: "Integration with TypeCheckResult"
    status: complete
---

# Section 05: Portable Type Descriptors

new Pool without access to the originating Pool or AST. This eliminates the AST fallback
path entirely, making cross-module type resolution fully pool-independent.

**Why this matters:** Section 04's hash-first import resolves types in O(1) when they
already exist in the local pool, but falls back to AST walking when they don't. For the
first module that imports a novel type (user-defined struct, complex generic instantiation),
this fallback fires. Portable Type Descriptors eliminate the fallback by carrying enough
information to reconstruct the type bottom-up from its content-addressed description.

This becomes critical with monomorphization: each generic instantiation
(`Matrix<int, 3, 3>`, `Matrix<float, 4, 4>`, ...) is a novel type in the importing module's
pool. Without descriptors, each instantiation triggers an AST walk. With descriptors, each
is a compact descriptor lookup + bottom-up construction.

**This section** is optional for correctness — Section 04's AST fallback is sufficient.
It is an optimization for large codebases with heavy generic usage. Implement after
Section 04 is working and benchmarked.

**This section** depends on Section 03 (hash-forwarded signatures). It can be developed
in parallel with Section 06 (backend integration).

---

## 05.1 TypeDescriptor Design — COMPLETE

by Merkle hash rather than by Idx.

**Design:**

```rust
/// A self-contained type description that can reconstruct a type in any Pool.
///
/// Unlike Pool types (which use Idx references), TypeDescriptors reference
/// children by their Merkle hashes. This makes them portable across Pool
/// instances — they carry no pool-local state.
///
/// Descriptors are topologically sorted: if descriptor D references children
/// with hashes H1, H2, ..., those children appear earlier in the descriptor
/// sequence. This enables single-pass bottom-up reconstruction.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeDescriptor {
    /// Primitive type (int, str, bool, etc.)
    /// Merkle hash: hash(tag)
    Primitive(Tag),

    /// Simple container (List<T>, Option<T>, Set<T>, Iterator<T>, etc.)
    /// Merkle hash: hash(tag, child_hash)
    Container {
        tag: Tag,
        child_hash: u64,
    },

    /// Two-child container (Map<K,V>, Result<T,E>)
    /// Merkle hash: hash(tag, child1_hash, child2_hash)
    TwoChild {
        tag: Tag,
        child1_hash: u64,
        child2_hash: u64,
    },

    /// Borrowed reference (&T with lifetime)
    /// Merkle hash: hash(BORROWED, inner_hash, lifetime_id)
    Borrowed {
        inner_hash: u64,
        lifetime_id: u32,
    },

    /// Function type ((P1, P2, ...) -> R)
    /// Merkle hash: hash(FUNCTION, count, param_hashes..., return_hash)
    Function {
        param_hashes: Vec<u64>,
        return_hash: u64,
    },

    /// Tuple type ((T1, T2, ...))
    /// Merkle hash: hash(TUPLE, count, element_hashes...)
    Tuple {
        element_hashes: Vec<u64>,
    },

    /// Struct type (struct Name { fields... })
    /// Merkle hash: hash(STRUCT, name, field_count, [name, type_hash]...)
    Struct {
        name: Name,
        field_names: Vec<Name>,
        field_type_hashes: Vec<u64>,
    },

    /// Enum type (enum Name { variants... })
    /// Merkle hash: hash(ENUM, name, variant_count, [name, fc, type_hashes...]...)
    Enum {
        name: Name,
        variants: Vec<VariantDescriptor>,
    },

    /// Applied generic type (Foo<A, B, ...>)
    /// Merkle hash: hash(APPLIED, name, arg_count, arg_hashes...)
    Applied {
        name: Name,
        arg_hashes: Vec<u64>,
    },

    /// Named type reference (unresolved)
    /// Merkle hash: hash(NAMED, name)
    Named {
        name: Name,
    },

    /// Type scheme (forall vars. body)
    /// Merkle hash: hash(SCHEME, var_count, var_ids..., body_hash)
    Scheme {
        var_count: usize,
        var_ids: Vec<u32>,
        body_hash: u64,
    },

    /// Type variable (Var, BoundVar, RigidVar)
    /// Merkle hash: hash(tag, var_id)
    Variable {
        tag: Tag,
        var_id: u32,
    },
}

/// Variant descriptor within an enum TypeDescriptor.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VariantDescriptor {
    pub name: Name,
    pub field_type_hashes: Vec<u64>,
}
```

**Properties:**
- **Pool-independent:** No Idx references. All type references via Merkle hash.
- **Self-describing:** The descriptor carries all information needed to reconstruct
  the type (tag, name, children). No external schema required.
- **Compact:** Minimal representation — no flags, no var_states, no extra array offsets.
- **Hashable:** The descriptor can compute its own Merkle hash (for verification).

**Size estimates:**
- Primitive: 2 bytes (tag only)
- Container: 10 bytes (tag + hash)
- Function with 3 params: 34 bytes (3 × 8 + 8 + 2 overhead)
- Struct with 5 fields: ~90 bytes (name + 5 × (name + hash))
- Typical imported function's types: 5-10 descriptors, ~100-300 bytes total

**File:** `compiler/ori_types/src/pool/descriptor.rs` (new file)

**Exit Criteria:**
- [x] `TypeDescriptor` enum defined with all needed variants
- [x] `VariantDescriptor` defined
- [x] All descriptor variants reference children by Merkle hash
- [x] `TypeDescriptor` derives `Clone, Debug, PartialEq, Eq, Hash`
- [x] Size estimates documented

---

## 05.2 Descriptor Generation — COMPLETE

**Algorithm:**

```rust
impl Pool {
    /// Generate a portable TypeDescriptor for a type.
    ///
    /// The descriptor references children by Merkle hash, not by Idx.
    pub fn describe(&self, idx: Idx) -> TypeDescriptor {
        let tag = self.tag(idx);
        match tag {
            // Primitives
            tag if tag.is_primitive() => TypeDescriptor::Primitive(tag),

            // Simple containers
            Tag::List | Tag::Option | Tag::Set | Tag::Channel
            | Tag::Range | Tag::Iterator | Tag::DoubleEndedIterator => {
                TypeDescriptor::Container {
                    tag,
                    child_hash: self.hashes[self.data(idx) as usize],
                }
            }

            // Map, Result
            Tag::Map | Tag::Result => {
                let extra_idx = self.data(idx) as usize;
                TypeDescriptor::TwoChild {
                    tag,
                    child1_hash: self.hashes[self.extra[extra_idx] as usize],
                    child2_hash: self.hashes[self.extra[extra_idx + 1] as usize],
                }
            }

            // Function
            Tag::Function => {
                let extra_idx = self.data(idx) as usize;
                let count = self.extra[extra_idx] as usize;
                let param_hashes = (0..count)
                    .map(|i| self.hashes[self.extra[extra_idx + 1 + i] as usize])
                    .collect();
                let return_hash = self.hashes[self.extra[extra_idx + 1 + count] as usize];
                TypeDescriptor::Function { param_hashes, return_hash }
            }

            // ... (similar for Tuple, Struct, Enum, Applied, Named, Scheme, Variable)

            _ => TypeDescriptor::Primitive(tag),  // Special types: opaque
        }
    }

    /// Generate a topologically sorted descriptor sequence for a type and all
    /// its transitive dependencies.
    ///
    /// Leaves (primitives) appear first. Each descriptor's child hashes reference
    /// types that appear earlier in the sequence. Deduplicates by hash.
    pub fn describe_transitive(&self, idx: Idx) -> Vec<(u64, TypeDescriptor)> {
        let mut result = Vec::new();
        let mut visited = FxHashSet::default();
        self.describe_recursive(idx, &mut result, &mut visited);
        result
    }

    fn describe_recursive(
        &self,
        idx: Idx,
        result: &mut Vec<(u64, TypeDescriptor)>,
        visited: &mut FxHashSet<u64>,
    ) {
        let hash = self.hash(idx);
        if !visited.insert(hash) {
            return;  // Already described
        }

        // Recurse into children first (topological order)
        let tag = self.tag(idx);
        if tag.has_child_in_data() {
            self.describe_recursive(Idx::from_raw(self.data(idx)), result, visited);
        }
        if tag.has_children_in_extra() {
            // Walk extra array for child Idx values (tag-specific)
            for child_idx in self.extra_child_indices(idx) {
                self.describe_recursive(child_idx, result, visited);
            }
        }

        // Then add self
        result.push((hash, self.describe(idx)));
    }
}
```

**File:** `compiler/ori_types/src/pool/descriptor.rs`

**Exit Criteria:**
- [x] `Pool::describe()` generates correct descriptor for each tag variant
- [x] `Pool::describe_transitive()` produces topologically sorted sequence
- [x] Deduplication by hash (each type appears once)
- [x] Test: `describe_transitive(List<Map<str, int>>)` produces 4 descriptors
  (int, str, Map<str,int>, List<Map<str,int>>) in topological order

---

## 05.3 Reconstruction Algorithm — COMPLETE

**Algorithm:**

```rust
impl Pool {
    /// Reconstruct types from a topologically sorted descriptor sequence.
    ///
    /// For each descriptor:
    /// 1. Check if the type already exists (by Merkle hash) → skip
    /// 2. If not: resolve child hashes to local Idx values, intern the type
    ///
    /// Returns a mapping from Merkle hash → local Idx for all reconstructed types.
    pub fn reconstruct(
        &mut self,
        descriptors: &[(u64, TypeDescriptor)],
    ) -> FxHashMap<u64, Idx> {
        let mut hash_to_idx = FxHashMap::default();

        for &(hash, ref desc) in descriptors {
            // Already exists?
            if let Some(&idx) = self.intern_map.get(&hash) {
                hash_to_idx.insert(hash, idx);
                continue;
            }

            // Reconstruct from descriptor
            let idx = match desc {
                TypeDescriptor::Primitive(tag) => {
                    // Primitives are always at fixed indices — should always hit above
                    unreachable!("Primitive not in pool: {:?}", tag);
                }

                TypeDescriptor::Container { tag, child_hash } => {
                    let child = hash_to_idx[child_hash];
                    self.intern(*tag, child.raw())
                }

                TypeDescriptor::TwoChild { tag, child1_hash, child2_hash } => {
                    let c1 = hash_to_idx[child1_hash];
                    let c2 = hash_to_idx[child2_hash];
                    self.intern_complex(*tag, &[c1.raw(), c2.raw()])
                }

                TypeDescriptor::Function { param_hashes, return_hash } => {
                    let params: Vec<Idx> = param_hashes.iter()
                        .map(|h| hash_to_idx[h])
                        .collect();
                    let ret = hash_to_idx[return_hash];
                    self.function(&params, ret)
                }

                TypeDescriptor::Struct { name, field_names, field_type_hashes } => {
                    let fields: Vec<(Name, Idx)> = field_names.iter()
                        .zip(field_type_hashes)
                        .map(|(&name, &hash)| (name, hash_to_idx[&hash]))
                        .collect();
                    self.struct_type(*name, &fields)
                }

                // ... similar for Tuple, Enum, Applied, Named, Scheme, Variable
                _ => todo!(),
            };

            hash_to_idx.insert(hash, idx);
        }

        hash_to_idx
    }
}
```

**Correctness invariant:** Since descriptors are topologically sorted, every child hash
referenced by a descriptor is guaranteed to already exist in `hash_to_idx` (either from
an earlier descriptor or from a pre-existing pool entry). The `hash_to_idx[child_hash]`
lookup never fails.

**Performance:**
- O(n) where n = number of unique types in the descriptor sequence
- Each type: one hash lookup (existing check) + one interning operation (if new)
- No AST access, no foreign arena, no recursive type resolution
- For 50 imported functions with avg 6 types each ≈ 300 descriptors,
  most deduplicating to ~100 unique types → ~100 hash lookups + ~30 new interns
  (assuming 70% hit rate from prelude/common types)

**Exit Criteria:**
- [x] `Pool::reconstruct()` handles all descriptor variants
- [x] Topological ordering respected (children before parents)
- [x] Existing types skipped (hash lookup hit)
- [x] New types correctly interned with Merkle hash
- [x] Test: reconstruct `(Map<str, List<int>>) -> Option<bool>` in empty pool

---

## 05.4 Integration with TypeCheckResult — COMPLETE

doesn't need AST access.

**Approach:** Add a `type_descriptors` field to `TypedModule`:

```rust
pub struct TypedModule {
    // ... existing fields ...

    /// Portable type descriptors for all types referenced in exported signatures.
    ///
    /// Topologically sorted: leaves first. Each entry is (merkle_hash, descriptor).
    /// Importing modules can reconstruct any exported type from these descriptors
    /// without accessing the originating Pool or AST.
    pub type_descriptors: Vec<(u64, TypeDescriptor)>,
}
```

**Population:** After type checking completes, generate descriptors for all types
referenced in exported function signatures:

```rust
fn generate_export_descriptors(
    pool: &Pool,
    functions: &[FunctionSig],
) -> Vec<(u64, TypeDescriptor)> {
    let mut all_descriptors = Vec::new();
    let mut visited = FxHashSet::default();

    for sig in functions {
        if !sig.is_public {
            continue;  // Only export public function types
        }
        for &idx in &sig.param_types {
            pool.describe_recursive(idx, &mut all_descriptors, &mut visited);
        }
        pool.describe_recursive(sig.return_type, &mut all_descriptors, &mut visited);
    }

    all_descriptors
}
```

**Salsa compatibility:** `TypeDescriptor` derives `Clone, Eq, PartialEq, Hash, Debug`.
`Vec<(u64, TypeDescriptor)>` satisfies all Salsa requirements.

**Size impact on TypeCheckResult:**
- Typical module with 20 public functions, ~50 unique types in signatures
- ~50 descriptors × ~20 bytes avg = ~1KB per module
- Acceptable for Salsa memoization

**Exit Criteria:**
- [x] `type_descriptors` field added to `TypedModule`
- [x] Descriptor generation after type checking
- [x] Only public function types exported (minimize size)
- [x] Salsa compatibility maintained
- [x] `cargo c` and `./test-all.sh` pass

---

## Section 05 Completion Checklist

- [x] `TypeDescriptor` enum with all variants (05.1)
- [x] `Pool::describe()` and `Pool::describe_transitive()` (05.2)
- [x] `Pool::reconstruct()` from descriptor sequence (05.3)
- [x] Round-trip test: describe → reconstruct → verify hash equality (05.3)
- [x] Integration with `TypedModule` (05.4)
- [x] Import path uses descriptors when available, AST fallback otherwise
- [x] All tests pass
