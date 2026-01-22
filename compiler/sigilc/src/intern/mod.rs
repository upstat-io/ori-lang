use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::RwLock;

use std::collections::hash_map::DefaultHasher;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StringId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(pub u32);

impl StringId {
    pub fn new(id: u32) -> Self {
        StringId(id)
    }
}

impl TypeId {
    pub fn new(id: u32) -> Self {
        TypeId(id)
    }
}

impl ValueId {
    pub fn new(id: u32) -> Self {
        ValueId(id)
    }
}

// Placeholder types - these should be imported from the actual AST/types module
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Type {
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Value {
    data: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericKey {
    base: TypeId,
    args: Vec<TypeId>,
}

#[derive(Debug, Clone)]
pub struct FunctionId {
    pub module: usize,
    pub index: usize,
}

pub struct Interner {
    local: Local,
    shared: Shared,
    revision: AtomicU64,
}

pub struct Local {
    strings: Vec<String>,
    types: Vec<TypeId>,
    arena: LocalArena,
}

pub struct Shared {
    strings: ShardedMap<String, StringId>,
    types: ShardedMap<Type, TypeId>,
    values: ShardedMap<Value, ValueId>,
}

#[derive(Default)]
pub struct LocalArena {
    data: Vec<u8>,
}

impl LocalArena {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn alloc_string(&mut self, s: String) -> String {
        s
    }
}

pub struct ShardedMap<K, V> {
    shards: Vec<RwLock<HashMap<K, V>>>,
}

impl<K: Clone + Hash + Eq, V: Clone> ShardedMap<K, V> {
    pub fn new() -> Self {
        let num_shards = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);

        let mut shards = Vec::with_capacity(num_shards);
        for _ in 0..num_shards {
            shards.push(RwLock::new(HashMap::new()));
        }

        Self { shards }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        let shard_idx = self.determine_shard(key);
        let shard = self.shards[shard_idx].read().unwrap();
        shard.get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) -> V {
        let shard_idx = self.determine_shard(&key);
        let mut shard = self.shards[shard_idx].write().unwrap();
        shard.insert(key, value.clone());
        value
    }

    pub fn len(&self) -> usize {
        self.shards
            .iter()
            .map(|shard| shard.read().unwrap().len())
            .sum()
    }

    fn determine_shard(&self, key: &K) -> usize {
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shards.len()
    }
}

impl<K: Clone + Hash + Eq, V: Clone> Default for ShardedMap<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self {
            strings: ShardedMap::new(),
            types: ShardedMap::new(),
            values: ShardedMap::new(),
        }
    }
}

impl Default for Local {
    fn default() -> Self {
        Self {
            strings: Vec::new(),
            types: Vec::new(),
            arena: LocalArena::new(),
        }
    }
}

impl Default for Interner {
    fn default() -> Self {
        Self::new()
    }
}

impl Interner {
    pub fn new() -> Self {
        Self {
            local: Local::default(),
            shared: Shared::default(),
            revision: AtomicU64::new(1),
        }
    }

    pub fn intern_string(&self, s: String) -> StringId {
        if let Some(id) = self.shared.strings.get(&s) {
            return id;
        }

        let id = self.shared.strings.len() as u32;
        self.shared.strings.insert(s.clone(), StringId::new(id));

        StringId::new(id)
    }

    pub fn intern_type(&self, ty: Type) -> TypeId {
        if let Some(id) = self.shared.types.get(&ty) {
            return id;
        }

        let id = self.shared.types.len() as u32;
        self.shared.types.insert(ty.clone(), TypeId::new(id));
        TypeId::new(id)
    }

    pub fn intern_int(&self, i: i64) -> ValueId {
        let value = Value {
            data: i.to_string(),
        };
        if let Some(id) = self.shared.values.get(&value) {
            return id;
        }

        let id = self.shared.values.len() as u32;
        self.shared.values.insert(value.clone(), ValueId::new(id));
        ValueId::new(id)
    }

    pub fn intern_bool(&self, b: bool) -> ValueId {
        let value = Value {
            data: b.to_string(),
        };
        if let Some(id) = self.shared.values.get(&value) {
            return id;
        }

        let id = self.shared.values.len() as u32;
        self.shared.values.insert(value.clone(), ValueId::new(id));
        ValueId::new(id)
    }

    pub fn intern_generic_value(&self, base: TypeId, args: Vec<TypeId>) -> ValueId {
        let key = Value {
            data: format!("{:?}:{:?}", base, args),
        };
        if let Some(id) = self.shared.values.get(&key) {
            return id;
        }

        let id = self.shared.values.len() as u32;
        self.shared.values.insert(key.clone(), ValueId::new(id));
        ValueId::new(id)
    }

    pub fn intern_with_dedup(&self, s: String) -> StringId {
        for shard in self.shared.strings.shards.iter() {
            if let Some(id) = shard.read().unwrap().get(&s) {
                return *id;
            }
        }

        let id = self.shared.strings.len() as u32;
        let string_id = StringId::new(id);

        for shard in self.shared.strings.shards.iter() {
            let mut shard = shard.write().unwrap();
            shard.insert(s.clone(), string_id);
        }

        string_id
    }

    pub fn increment_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn current_revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
}

impl Local {
    pub fn alloc_string(&mut self, s: String) -> String {
        self.arena.alloc_string(s)
    }

    pub fn store_string(&mut self, s: String) {
        self.strings.push(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interning() {
        let interner = Interner::new();

        let id1 = interner.intern_string("hello".to_string());
        let id2 = interner.intern_string("world".to_string());
        let id3 = interner.intern_string("hello".to_string());

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_string_deduplication() {
        let interner = Interner::new();

        let id1 = interner.intern_with_dedup("test".to_string());
        let id2 = interner.intern_with_dedup("test".to_string());

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_type_interning() {
        let interner = Interner::new();

        let type1 = Type {
            name: "int".to_string(),
        };
        let type2 = Type {
            name: "string".to_string(),
        };
        let type3 = Type {
            name: "int".to_string(),
        };

        let id1 = interner.intern_type(type1.clone());
        let id2 = interner.intern_type(type2.clone());
        let id3 = interner.intern_type(type3.clone());

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_value_interning() {
        let interner = Interner::new();

        let id1 = interner.intern_int(42);
        let id2 = interner.intern_int(100);
        let id3 = interner.intern_int(42);

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);

        let bool_id1 = interner.intern_bool(true);
        let bool_id2 = interner.intern_bool(false);
        let bool_id3 = interner.intern_bool(true);

        assert_ne!(bool_id1, bool_id2);
        assert_eq!(bool_id1, bool_id3);
    }

    #[test]
    fn test_generic_value_interning() {
        let interner = Interner::new();

        let base_type = Type {
            name: "List".to_string(),
        };
        let type_id = interner.intern_type(base_type);

        let id1 = interner.intern_generic_value(type_id, vec![]);
        let id2 = interner.intern_generic_value(type_id, vec![TypeId::new(1)]);
        let id3 = interner.intern_generic_value(type_id, vec![]);

        assert_ne!(id1, id2);
        assert_eq!(id1, id3);
    }

    #[test]
    fn test_revision_tracking() {
        let interner = Interner::new();

        let initial_revision = interner.current_revision();

        let new_revision = interner.increment_revision();
        assert_eq!(new_revision, initial_revision + 1);
        assert_eq!(interner.current_revision(), new_revision);

        interner.increment_revision();
        interner.increment_revision();
        assert_eq!(interner.current_revision(), initial_revision + 3);
    }

    #[test]
    fn test_local_arena() {
        let mut local = Local::default();

        let string1 = local.alloc_string("test1".to_string());
        let string2 = local.alloc_string("test2".to_string());

        assert_ne!(string1, string2);

        local.store_string("stored".to_string());
        assert_eq!(local.strings.len(), 1);
        assert_eq!(local.strings[0], "stored");
    }
}
