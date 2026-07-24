//! Canonical subtree hashing for incremental compilation.

mod subtree;

pub use subtree::hash_canonical_subtree;

#[cfg(test)]
mod tests;
