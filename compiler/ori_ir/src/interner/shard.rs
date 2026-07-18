use rustc_hash::FxHashMap;

use crate::Name;

const INITIAL_KEY_CHUNK_LEN: usize = 4096;

/// Per-shard storage for interned strings.
#[derive(Debug)]
pub(super) struct InternShard {
    /// Map from string content to local index.
    map: FxHashMap<&'static str, u32>,
    /// Local index to a process-lifetime byte span in the chunk pool.
    spans: Vec<&'static str>,
    /// Total byte length of all interned spans.
    total_bytes: usize,
    /// Remaining writable bytes in the current process-lifetime key chunk.
    key_remaining: &'static mut [u8],
    next_key_chunk_len: usize,
}

/// Error when interning a string fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InternError {
    /// Shard exceeded the 28-bit local index capacity.
    ShardOverflow { shard_idx: usize, count: usize },
    /// Shard byte storage exceeded the u32 span representation.
    ByteStorageOverflow {
        shard_idx: usize,
        bytes: usize,
        additional: usize,
    },
}

impl std::fmt::Display for InternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InternError::ShardOverflow { shard_idx, count } => write!(
                f,
                "interner shard {} exceeded capacity: {} strings (0x{:X}), max is {} (0x{:X})",
                shard_idx,
                count,
                count,
                Name::MAX_LOCAL,
                Name::MAX_LOCAL
            ),
            InternError::ByteStorageOverflow {
                shard_idx,
                bytes,
                additional,
            } => write!(
                f,
                "interner shard {} byte storage exceeded span capacity: {} bytes plus {} more, max is {}",
                shard_idx,
                bytes,
                additional,
                u32::MAX
            ),
        }
    }
}

impl std::error::Error for InternError {}

impl InternShard {
    pub(super) fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            spans: Vec::with_capacity(256),
            total_bytes: 0,
            key_remaining: Vec::new().leak(),
            next_key_chunk_len: INITIAL_KEY_CHUNK_LEN,
        }
    }

    pub(super) fn with_empty() -> Self {
        let mut shard = Self::new();
        let empty: &'static str = "";
        shard.map.insert(empty, 0);
        shard.spans.push(empty);
        shard
    }

    fn next_local(&self, shard_idx: usize) -> Result<u32, InternError> {
        let count = self.spans.len();
        if count > Name::MAX_LOCAL as usize {
            return Err(InternError::ShardOverflow { shard_idx, count });
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "count is checked against Name::MAX_LOCAL"
        )]
        Ok(count as u32)
    }

    fn append_key(&mut self, shard_idx: usize, s: &str) -> Result<&'static str, InternError> {
        let bytes = self.total_bytes;
        let additional = s.len();
        let new_len = bytes
            .checked_add(additional)
            .ok_or(InternError::ByteStorageOverflow {
                shard_idx,
                bytes,
                additional,
            })?;
        if new_len > u32::MAX as usize || additional > u32::MAX as usize {
            return Err(InternError::ByteStorageOverflow {
                shard_idx,
                bytes,
                additional,
            });
        }

        if self.key_remaining.len() < s.len() {
            let chunk_len = self.next_key_chunk_len.max(s.len());
            self.next_key_chunk_len = chunk_len.saturating_mul(2).max(INITIAL_KEY_CHUNK_LEN);
            self.key_remaining = vec![0; chunk_len].leak();
        }

        let key = if s.is_empty() {
            ""
        } else {
            let remaining = std::mem::replace(&mut self.key_remaining, Vec::new().leak());
            let (slot, rest) = remaining.split_at_mut(s.len());
            slot.copy_from_slice(s.as_bytes());
            self.key_remaining = rest;
            match std::str::from_utf8(slot) {
                Ok(key) => key,
                Err(err) => unreachable!("interned key bytes were copied from a valid str: {err}"),
            }
        };
        self.total_bytes = new_len;
        Ok(key)
    }

    pub(super) fn span_str(&self, local: usize) -> Option<&'static str> {
        self.spans.get(local).copied()
    }

    pub(super) fn local_for(&self, s: &str) -> Option<u32> {
        self.map.get(s).copied()
    }

    pub(super) fn insert_new(&mut self, shard_idx: usize, s: &str) -> Result<u32, InternError> {
        let local = self.next_local(shard_idx)?;
        let key = self.append_key(shard_idx, s)?;
        self.spans.push(key);
        self.map.insert(key, local);
        Ok(local)
    }

    #[cfg(test)]
    pub(super) fn remove_map_entry_for_test(&mut self, key: &str) -> bool {
        self.map.remove(key).is_some()
    }
}
