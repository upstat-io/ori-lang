//! Incremental test execution via function-level change detection.
//!
//! Tracks which function bodies changed between runs and determines which
//! tests must re-run. Tests whose targets are all unchanged can be skipped,
//! saving significant time in large codebases with many targeted tests.
//!
//! # Architecture
//!
//! ```text
//! CanonResult.roots ──→ FunctionChangeMap (body hash per function)
//!                            │
//! Module.tests ────────→ TestTargetIndex (bidirectional func↔test map)
//!                            │
//! Previous FunctionChangeMap ┘
//!         ↓
//!   changed_since() → FxHashSet<Name> of changed functions
//!         ↓
//!   tests_for_changed() → FxHashSet<Name> of tests to re-run
//! ```

use std::path::{Path, PathBuf};

use rustc_hash::{FxHashMap, FxHashSet};

use ori_ir::canon::{hash::hash_canonical_subtree, CanonResult};

use crate::ir::{Module, Name, TestDef};

/// Per-function body hashes for change detection.
///
/// Maps each function/test name to a `u64` hash of its canonical body.
/// Computed from `CanonResult.roots` using `hash_canonical_subtree`.
#[derive(Clone, Debug, Default)]
pub struct FunctionChangeMap {
    hashes: FxHashMap<Name, u64>,
}

impl FunctionChangeMap {
    /// Compute body hashes for all roots in a `CanonResult`.
    pub fn from_canon(canon: &CanonResult) -> Self {
        let mut hashes =
            FxHashMap::with_capacity_and_hasher(canon.roots.len(), rustc_hash::FxBuildHasher);

        for root in &canon.roots {
            let hash = hash_canonical_subtree(&canon.arena, root.body);
            hashes.insert(root.name, hash);
        }

        Self { hashes }
    }

    /// Get the body hash for a function by name.
    pub fn get(&self, name: Name) -> Option<u64> {
        self.hashes.get(&name).copied()
    }

    /// Function names whose bodies differ from a previous snapshot.
    ///
    /// A function is considered "changed" if:
    /// - It exists in `self` but not in `previous` (new function)
    /// - It exists in `previous` but not in `self` (deleted function)
    /// - Its body hash differs between the two snapshots
    pub fn changed_since(&self, previous: &Self) -> FxHashSet<Name> {
        let mut changed = FxHashSet::default();

        // New or modified functions.
        for (&name, &hash) in &self.hashes {
            match previous.hashes.get(&name) {
                Some(&prev_hash) if prev_hash == hash => {}
                _ => {
                    changed.insert(name);
                }
            }
        }

        // Deleted functions (in previous but not in current).
        for &name in previous.hashes.keys() {
            if !self.hashes.contains_key(&name) {
                changed.insert(name);
            }
        }

        changed
    }

    /// Number of tracked functions.
    pub fn len(&self) -> usize {
        self.hashes.len()
    }

    /// Returns `true` if no functions are tracked.
    pub fn is_empty(&self) -> bool {
        self.hashes.is_empty()
    }
}

/// Bidirectional index between functions and their tests.
///
/// Built from `Module.tests` by examining each test's `targets` field.
/// Enables efficient lookup in both directions:
/// - Forward: "which tests target this function?" (for change propagation)
/// - Reverse: "which functions does this test target?" (for skip decisions)
#[derive(Clone, Debug, Default)]
pub struct TestTargetIndex {
    /// Function → tests that target it.
    func_to_tests: FxHashMap<Name, Vec<Name>>,
    /// Test → functions it targets.
    test_to_funcs: FxHashMap<Name, Vec<Name>>,
}

impl TestTargetIndex {
    /// Build from a module's test definitions.
    pub fn from_module(module: &Module) -> Self {
        let mut func_to_tests: FxHashMap<Name, Vec<Name>> = FxHashMap::default();
        let mut test_to_funcs: FxHashMap<Name, Vec<Name>> = FxHashMap::default();

        for test in &module.tests {
            test_to_funcs.insert(test.name, test.targets.clone());

            for &target in &test.targets {
                func_to_tests.entry(target).or_default().push(test.name);
            }
        }

        Self {
            func_to_tests,
            test_to_funcs,
        }
    }

    /// Tests that must re-run because at least one target changed.
    pub fn tests_for_changed(&self, changed: &FxHashSet<Name>) -> FxHashSet<Name> {
        let mut affected = FxHashSet::default();

        for &func_name in changed {
            if let Some(tests) = self.func_to_tests.get(&func_name) {
                affected.extend(tests);
            }
        }

        // Tests whose own body changed must also re-run.
        // Test bodies are in CanonResult.roots alongside function bodies,
        // so they appear in `changed` if modified.
        for &name in changed {
            if self.test_to_funcs.contains_key(&name) {
                affected.insert(name);
            }
        }

        affected
    }

    /// Tests that can be skipped (all targets unchanged).
    ///
    /// A test is skippable if:
    /// - It has at least one target (floating tests are never skipped)
    /// - None of its targets are in the `changed` set
    /// - The test's own body is not in the `changed` set
    pub fn skippable_tests(&self, changed: &FxHashSet<Name>, all_tests: &[&TestDef]) -> Vec<Name> {
        all_tests
            .iter()
            .filter(|test| {
                // Floating tests (no targets) are never skipped.
                if test.targets.is_empty() {
                    return false;
                }

                // Skip if test body itself changed.
                if changed.contains(&test.name) {
                    return false;
                }

                // Skip if any target changed.
                !test.targets.iter().any(|t| changed.contains(t))
            })
            .map(|test| test.name)
            .collect()
    }

    /// Get tests targeting a specific function.
    pub fn tests_for(&self, func: Name) -> &[Name] {
        self.func_to_tests.get(&func).map_or(&[], Vec::as_slice)
    }

    /// Get targets for a specific test.
    pub fn targets_for(&self, test: Name) -> &[Name] {
        self.test_to_funcs.get(&test).map_or(&[], Vec::as_slice)
    }
}

/// Cross-run cache for incremental test execution.
///
/// Stores `FunctionChangeMap` snapshots per file, enabling change detection
/// across test runs. Initially in-memory only — value comes from watch mode
/// where the runner persists across runs.
#[derive(Clone, Debug, Default)]
pub struct TestRunCache {
    file_hashes: FxHashMap<PathBuf, FunctionChangeMap>,
}

impl TestRunCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the previous `FunctionChangeMap` for a file, if cached.
    pub fn get(&self, path: &Path) -> Option<&FunctionChangeMap> {
        self.file_hashes.get(path)
    }

    /// Store a `FunctionChangeMap` snapshot for a file.
    pub fn insert(&mut self, path: PathBuf, map: FunctionChangeMap) {
        self.file_hashes.insert(path, map);
    }

    /// Number of cached files.
    pub fn len(&self) -> usize {
        self.file_hashes.len()
    }

    /// Returns `true` if no files are cached.
    pub fn is_empty(&self) -> bool {
        self.file_hashes.is_empty()
    }
}

#[cfg(test)]
mod tests;
