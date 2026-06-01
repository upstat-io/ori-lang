//! Bucketed storage for `Value::Map` and `Value::Set`.
//!
//! Map and set entries are bucketed by a string bucket key. Primitive keys use
//! the type-prefixed `to_map_key()` string (`i:42`, `s:hello`, ...) and occupy a
//! single-entry bucket — the string IS the key identity, so lookup is a direct
//! `BTreeMap` probe with no element comparison. Non-primitive keys (struct / enum
//! / newtype) use the bucket key `h:{hash}` derived from the user `@hash`; the
//! bucket `Vec` holds every key that hashes to that value, and collisions are
//! resolved by the caller's `@eq` predicate. Primitive (`i:` / `s:` / ...) and
//! non-primitive (`h:`) bucket strings never collide — distinct prefixes.
//!
//! Each entry stores the original key `Value` alongside its value, so iteration /
//! `.keys()` / display recover the real key even when the bucket string is a hash.

use std::collections::BTreeMap;

use super::Value;

/// Prefix marking a non-primitive (user-`Hashable`) bucket key.
///
/// Primitive bucket keys carry their `to_map_key()` type prefix (`i:`, `s:`, ...);
/// this prefix is reserved for hash buckets and never produced by `to_map_key()`.
const HASH_BUCKET_PREFIX: char = 'h';

/// Bucketed map/set storage keyed by a string bucket key.
///
/// Buckets iterate in `BTreeMap` order; within a bucket, entries iterate in
/// insertion order. Primitive keys produce single-entry buckets, preserving the
/// historical primitive-key iteration order exactly.
#[derive(Clone, Default)]
pub struct MapData {
    buckets: BTreeMap<String, Vec<(Value, Value)>>,
}

impl MapData {
    /// Create an empty map.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: BTreeMap::new(),
        }
    }

    /// Build the bucket key string for a non-primitive key from its user `@hash`.
    ///
    /// Primitive bucket keys carry their `to_map_key()` type prefix (`i:`, `s:`,
    /// ...); this `h:` prefix is reserved for hash buckets and never produced by
    /// `to_map_key()`, so primitive and non-primitive buckets never collide.
    #[must_use]
    pub fn hash_bucket_key(user_hash: i64) -> String {
        format!("{HASH_BUCKET_PREFIX}:{:016x}", user_hash.cast_unsigned())
    }

    /// Build a primitive-keyed map directly from `to_map_key()` string entries.
    ///
    /// Each `(bucket_key, value)` becomes a single-entry bucket whose stored key
    /// `Value` is decoded from the bucket string via `Value::from_map_key`. This
    /// is the construction path for maps built entirely from primitive keys.
    #[must_use]
    pub fn from_primitive_entries(entries: BTreeMap<String, Value>) -> Self {
        let buckets = entries
            .into_iter()
            .map(|(k, v)| {
                let key_val = Value::from_map_key(&k);
                (k, vec![(key_val, v)])
            })
            .collect();
        Self { buckets }
    }

    /// Total number of entries across all buckets.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buckets.values().map(Vec::len).sum()
    }

    /// Whether the map holds zero entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buckets.values().all(Vec::is_empty)
    }

    /// Iterate `(original_key, value)` pairs in deterministic order.
    pub fn iter(&self) -> impl Iterator<Item = (&Value, &Value)> {
        self.buckets
            .values()
            .flat_map(|bucket| bucket.iter().map(|(k, v)| (k, v)))
    }

    /// Iterate the original key `Value`s in deterministic order.
    pub fn keys(&self) -> impl Iterator<Item = &Value> {
        self.iter().map(|(k, _)| k)
    }

    /// Iterate the value `Value`s in deterministic order.
    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.iter().map(|(_, v)| v)
    }

    // Primitive fast path — bucket string IS the key identity (single-entry bucket).

    /// Look up a value by its primitive bucket-key string.
    #[must_use]
    pub fn get_primitive(&self, bucket_key: &str) -> Option<&Value> {
        self.buckets
            .get(bucket_key)
            .and_then(|bucket| bucket.first())
            .map(|(_, v)| v)
    }

    /// Whether a primitive bucket-key string is present.
    #[must_use]
    pub fn contains_primitive(&self, bucket_key: &str) -> bool {
        self.buckets.contains_key(bucket_key)
    }

    /// Insert a primitive key (single-entry bucket; replaces any prior value).
    pub fn insert_primitive(&mut self, bucket_key: String, value: Value) {
        let key_val = Value::from_map_key(&bucket_key);
        self.buckets.insert(bucket_key, vec![(key_val, value)]);
    }

    /// Remove a primitive key by its bucket-key string.
    pub fn remove_primitive(&mut self, bucket_key: &str) {
        self.buckets.remove(bucket_key);
    }

    // Non-primitive path — bucket holds `@hash`-colliding keys; the caller drives
    // `@eq` collision resolution (the comparison needs interpreter access, which
    // lives in `ori_eval`, so these primitives are closure-free).

    /// Borrow the stored key `Value`s of a hash bucket (insertion order).
    ///
    /// The caller clones the candidates it needs, drops this borrow, then runs
    /// the `@eq` comparison loop without holding a `MapData` borrow.
    #[must_use]
    pub fn bucket_keys(&self, bucket_key: &str) -> Option<&[(Value, Value)]> {
        self.buckets.get(bucket_key).map(Vec::as_slice)
    }

    /// Read the value at `index` within a hash bucket.
    #[must_use]
    pub fn bucket_value_at(&self, bucket_key: &str, index: usize) -> Option<&Value> {
        self.buckets
            .get(bucket_key)
            .and_then(|bucket| bucket.get(index))
            .map(|(_, v)| v)
    }

    /// Insert into a hash bucket: replace the value at `at` if `Some`, else append.
    ///
    /// `at` is the `@eq`-match index the caller computed against `bucket_keys`.
    pub fn bucket_put(&mut self, bucket_key: String, at: Option<usize>, key: Value, value: Value) {
        let bucket = self.buckets.entry(bucket_key).or_default();
        match at {
            Some(idx) if idx < bucket.len() => bucket[idx] = (key, value),
            _ => bucket.push((key, value)),
        }
    }

    /// Remove the entry at `index` from a hash bucket; drop the bucket if emptied.
    pub fn bucket_remove_at(&mut self, bucket_key: &str, index: usize) {
        if let Some(bucket) = self.buckets.get_mut(bucket_key) {
            if index < bucket.len() {
                bucket.remove(index);
            }
            if bucket.is_empty() {
                self.buckets.remove(bucket_key);
            }
        }
    }
}

impl PartialEq for MapData {
    /// Structural equality: same multiset of `(key, value)` entries.
    ///
    /// Order-independent over buckets; for hash buckets this compares stored key
    /// `Value`s structurally via `Value::equals`, sufficient for the
    /// `ori_patterns`-side `Eq`/`Hash` contracts (the `@eq`-driven path lives in
    /// `ori_eval`).
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        // Primitive buckets compare directly by string identity; hash buckets
        // compare structurally. Walk self's entries; require each to appear in
        // other under the same bucket key with a structurally-equal key+value.
        for (bucket_key, bucket) in &self.buckets {
            let Some(other_bucket) = other.buckets.get(bucket_key) else {
                if bucket.is_empty() {
                    continue;
                }
                return false;
            };
            if bucket.len() != other_bucket.len() {
                return false;
            }
            for (k, v) in bucket {
                let found = other_bucket
                    .iter()
                    .any(|(ok, ov)| k.equals(ok) && v.equals(ov));
                if !found {
                    return false;
                }
            }
        }
        true
    }
}

impl Eq for MapData {}

impl std::hash::Hash for MapData {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Order-independent: hash length only (entry order is bucket-dependent).
        // Matches the prior BTreeMap-based contract's reliance on length + the
        // discriminant already hashed by the Value Hash impl.
        self.len().hash(state);
    }
}

impl std::fmt::Debug for MapData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
