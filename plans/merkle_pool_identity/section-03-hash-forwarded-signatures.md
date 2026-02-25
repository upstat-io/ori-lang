---
section: "03"
title: "Hash-Forwarded Signatures"
status: not_started
goal: "Embed Merkle hashes in FunctionSig and TypeCheckResult so type identity survives cross-module transport without pool access"
inspired_by:
  - "Roc GlobalLayoutInterner — type identity preserved across compilation phases via global indices"
  - "Zig TrackedInst.Index — stable references across incremental updates"
sections:
  - id: "03.1"
    title: "FunctionSig Hash Fields"
    status: not_started
  - id: "03.2"
    title: "TypeEntry Hash Fields"
    status: not_started
  - id: "03.3"
    title: "Hash Population During Type Checking"
    status: not_started
  - id: "03.4"
    title: "Salsa Compatibility Verification"
    status: not_started
---

# Section 03: Hash-Forwarded Signatures

**Status:** Not Started
**Goal:** Add Merkle hash fields to `FunctionSig` and `TypeEntry` so that cross-module consumers
can identify types by hash without needing access to the originating Pool. This enables O(1)
type lookup at import boundaries (Section 04) and pool-independent type comparison (Section 06).

**Why this matters:** Currently, `FunctionSig.param_types` and `FunctionSig.return_type` are
`Vec<Idx>` and `Idx` — pool-local handles. When Module B receives A's `FunctionSig` via the
Salsa `typed()` query, these Idx values are meaningless in B's pool. The type checker must
re-derive types from the AST. With embedded Merkle hashes, B can do a direct O(1) lookup:
`intern_map.get(&hash)` → local `Idx`, skipping the AST walk entirely.

**This section** depends on Section 01 (Merkle hash computation) and Section 02 (stability tests).

---

## 03.1 FunctionSig Hash Fields — NOT STARTED

**Goal:** Add `param_hashes: Vec<u64>` and `return_hash: u64` to `FunctionSig`, populated
during signature inference.

**File:** `compiler/ori_types/src/output/mod.rs` (lines 244-442)

**Current FunctionSig:**
```rust
pub struct FunctionSig {
    pub name: Name,
    pub type_params: Vec<Name>,
    pub const_params: Vec<ConstParamInfo>,
    pub param_names: Vec<Name>,
    pub param_types: Vec<Idx>,           // ← pool-local
    pub return_type: Idx,                // ← pool-local
    pub capabilities: Vec<Name>,
    pub is_public: bool,
    pub is_test: bool,
    pub is_main: bool,
    pub is_fbip: bool,
    pub type_param_bounds: Vec<Vec<Name>>,
    pub where_clauses: Vec<FnWhereClause>,
    pub generic_param_mapping: Vec<Option<usize>>,
    pub scheme_var_ids: Vec<u32>,
    pub required_params: usize,
    pub param_defaults: Vec<Option<ExprId>>,
}
```

**New fields:**
```rust
pub struct FunctionSig {
    // ... existing fields unchanged ...

    /// Merkle hashes for parameter types — stable across Pool instances.
    ///
    /// `param_hashes[i]` is the content-addressed hash of `param_types[i]`.
    /// Used for cross-module type identity: receiving modules can look up
    /// types by hash in their own `intern_map` without AST re-walking.
    /// Always `param_hashes.len() == param_types.len()`.
    pub param_hashes: Vec<u64>,

    /// Merkle hash for the return type — stable across Pool instances.
    pub return_hash: u64,
}
```

**Invariants:**
- `param_hashes.len() == param_types.len()` — always in sync
- `param_hashes[i] == pool.hash(param_types[i])` — hash matches Idx
- `return_hash == pool.hash(return_type)` — hash matches Idx

**Impact on derived traits:** `FunctionSig` derives `Clone, Eq, PartialEq, Hash, Debug`.
Adding `Vec<u64>` and `u64` fields preserves all derives — `u64` and `Vec<u64>` implement
all required traits. No Salsa compatibility issues.

**Impact on existing code:** Every place that constructs a `FunctionSig` must now populate
the hash fields. Search for `FunctionSig {` in the codebase to find all construction sites.
Known sites:
- `check/signatures/mod.rs` — `infer_function_signature_with_arena()` (primary)
- Any test code that constructs FunctionSig directly

**Default for backward compatibility during migration:**
```rust
impl FunctionSig {
    /// Create hash fields from a Pool. Call after param_types/return_type are set.
    pub fn populate_hashes(&mut self, pool: &Pool) {
        self.param_hashes = self.param_types.iter()
            .map(|&idx| pool.hash(idx))
            .collect();
        self.return_hash = pool.hash(self.return_type);
    }
}
```

**Exit Criteria:**
- [ ] `param_hashes` and `return_hash` fields added to `FunctionSig`
- [ ] All `FunctionSig` construction sites updated to populate hashes
- [ ] `populate_hashes()` helper available for cases where pool is available
- [ ] `FunctionSig` still derives `Clone, Eq, PartialEq, Hash, Debug`
- [ ] `cargo c` succeeds for all crates

---

## 03.2 TypeEntry Hash Fields — NOT STARTED

**Goal:** Add Merkle hashes to `TypeEntry` (user-defined types in `TypedModule`) for
cross-module type identity of structs, enums, aliases, and newtypes.

**File:** `compiler/ori_types/src/output/mod.rs`

**Current TypeEntry structure:** (investigate exact fields — may contain `Idx` values for
field types, variant types, etc.)

**New fields:**
```rust
pub struct TypeEntry {
    // ... existing fields ...

    /// Merkle hash of the type's Pool representation.
    /// Stable across Pool instances for cross-module identity.
    pub merkle_hash: u64,
}
```

**Why:** When Module B imports a struct type from Module A, B needs to verify it's the same
struct (same name, same field types). With `merkle_hash`, this is a single `u64` comparison
instead of field-by-field structural comparison.

**Exit Criteria:**
- [ ] `merkle_hash` field added to `TypeEntry`
- [ ] All `TypeEntry` construction sites populate the hash
- [ ] Hash derived from the Pool's Merkle hash for the corresponding Idx
- [ ] `cargo c` succeeds

---

## 03.3 Hash Population During Type Checking — NOT STARTED

**Goal:** Populate hash fields at the point where `FunctionSig` is constructed during
type checking, ensuring hashes are always consistent with Idx values.

**File:** `compiler/ori_types/src/check/signatures/mod.rs` (lines 132-274)

**Current flow in `infer_function_signature_with_arena()`:**
1. Resolve parameter types → `Vec<Idx>` in local pool
2. Resolve return type → `Idx` in local pool
3. Construct `FunctionSig { param_types, return_type, ... }`

**New flow:**
1. Resolve parameter types → `Vec<Idx>` in local pool
2. Resolve return type → `Idx` in local pool
3. Compute param hashes: `param_types.iter().map(|&idx| pool.hash(idx)).collect()`
4. Compute return hash: `pool.hash(return_type)`
5. Construct `FunctionSig { param_types, return_type, param_hashes, return_hash, ... }`

**Key constraint:** The pool must be accessible at construction time. In
`infer_function_signature_with_arena()`, the checker owns the pool via `self.pool`.
After resolving all types, calling `self.pool.hash(idx)` for each type is safe
because all types are already interned.

**Also update `register_imported_function()`** (`check/mod.rs:420-438`):
After constructing the `FunctionSig`, call `sig.populate_hashes(&self.pool)` if
hashes weren't already set during signature inference.

**Test:**
```rust
#[test]
fn function_sig_hashes_match_pool() {
    let mut pool = Pool::new();
    let param_types = vec![Idx::INT, pool.list(Idx::STR)];
    let return_type = pool.option(Idx::BOOL);

    let sig = FunctionSig {
        param_types: param_types.clone(),
        return_type,
        param_hashes: param_types.iter().map(|&idx| pool.hash(idx)).collect(),
        return_hash: pool.hash(return_type),
        // ... other fields ...
    };

    assert_eq!(sig.param_hashes.len(), sig.param_types.len());
    for (i, (&idx, &hash)) in sig.param_types.iter().zip(&sig.param_hashes).enumerate() {
        assert_eq!(pool.hash(idx), hash,
            "param[{i}] hash mismatch");
    }
    assert_eq!(pool.hash(sig.return_type), sig.return_hash);
}
```

**Exit Criteria:**
- [ ] Hash population integrated into signature inference pipeline
- [ ] Hashes always computed from the same pool that holds the Idx values
- [ ] `populate_hashes()` available as fallback for code paths that construct sigs differently
- [ ] Test verifying hash/Idx consistency

---

## 03.4 Salsa Compatibility Verification — NOT STARTED

**Goal:** Verify that the new hash fields don't break Salsa's memoization or early-cutoff
behavior.

**Concerns:**
1. **`TypeCheckResult` equality:** Salsa uses `Eq` to detect when a query's output changed.
   Adding hash fields changes `Eq` — but this is correct: if hashes change, the types changed.
   If hashes don't change, the types didn't change. Hash equality is a stricter check than
   what we had before (Idx equality, which could give false positives across builds if Idx
   assignment order changed).

2. **`TypeCheckResult` Hash trait:** Salsa uses `Hash` for memoization keys. Adding `u64` fields
   to the `Hash` derivation is fine — it doesn't change the semantics, just adds more data to
   the hash.

3. **Early cutoff:** If Module A's types don't change, `typed(A)` returns the same
   `TypeCheckResult` (same Idx values, same hashes). Downstream queries see no change →
   early cutoff works. Adding hash fields doesn't break this because: same types → same hashes.

4. **Pool side-cache:** The Pool is stored in `PoolCache` outside Salsa. Merkle hashes are
   stored INSIDE the Pool (in `self.hashes[]`) AND in `FunctionSig` (in `param_hashes`/`return_hash`).
   The FunctionSig hashes survive Salsa memoization (they're part of `TypeCheckResult`).
   The Pool hashes survive via `PoolCache`. Both are consistent because they're computed from
   the same `merkle_hash()` function.

**Test:**
```rust
#[test]
fn typed_result_salsa_compat() {
    // Construct a TypeCheckResult with hash fields
    let result = TypeCheckResult { /* ... */ };

    // Verify Clone, Eq, Hash all work
    let cloned = result.clone();
    assert_eq!(result, cloned);

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    result.hash(&mut hasher);
    let h1 = hasher.finish();

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    cloned.hash(&mut hasher);
    let h2 = hasher.finish();

    assert_eq!(h1, h2, "Hash must be deterministic");
}
```

**Exit Criteria:**
- [ ] `TypeCheckResult` with hash fields passes Salsa trait requirements
- [ ] Early cutoff behavior verified (same types → same result → no re-execution)
- [ ] No performance regression in `typed()` query (hash computation is O(1) per type)
- [ ] `./test-all.sh` passes with new fields

---

## Section 03 Completion Checklist

- [ ] `param_hashes` and `return_hash` added to `FunctionSig` (03.1)
- [ ] `merkle_hash` added to `TypeEntry` (03.2)
- [ ] Hash population in signature inference pipeline (03.3)
- [ ] `populate_hashes()` helper method (03.3)
- [ ] Salsa compatibility verified (03.4)
- [ ] All construction sites updated (no uninitialized hash fields)
- [ ] `cargo c` and `./test-all.sh` pass
