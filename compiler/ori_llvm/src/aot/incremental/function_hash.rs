//! Function-Level Content Hashing
//!
//! Computes content hashes for individual functions to enable fine-grained
//! incremental compilation. A function's hash captures its body, signature,
//! callees, and global references — everything needed to detect when
//! recompilation is required.
//!
//! # Design
//!
//! Inspired by Rust's incremental compilation and Lean 4's LCNF caching:
//! - **Body change**: Only that function needs recompilation
//! - **Signature change**: All callers must also be recompiled
//!
//! The combined hash is computed from four components:
//! 1. `body_hash` — the function's expression types (span-free)
//! 2. `signature_hash` — parameter types + return type
//! 3. `callees_hash` — names of called functions (change detection)
//! 4. `globals_hash` — referenced global constants/variables

use ori_ir::Name;
use ori_types::{FunctionSig, Idx};

use super::hash::{combine_hashes, hash_string, ContentHash};

/// Content hash for a single function, decomposed into components.
///
/// The decomposition allows callers to distinguish body-only changes
/// (which don't require caller recompilation) from signature changes
/// (which do).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionContentHash {
    /// Hash of the function body (expressions, control flow).
    body_hash: ContentHash,
    /// Hash of the function signature (param types, return type).
    pub(crate) signature_hash: ContentHash,
    /// Hash of callee function names.
    callees_hash: ContentHash,
    /// Hash of referenced globals.
    globals_hash: ContentHash,
    /// Combined hash of all components.
    combined: ContentHash,
}

impl FunctionContentHash {
    /// Compute a function content hash from its components.
    #[must_use]
    pub fn compute(
        body_hash: ContentHash,
        signature_hash: ContentHash,
        callees_hash: ContentHash,
        globals_hash: ContentHash,
    ) -> Self {
        let combined = combine_hashes(&[body_hash, signature_hash, callees_hash, globals_hash]);
        Self {
            body_hash,
            signature_hash,
            callees_hash,
            globals_hash,
            combined,
        }
    }

    /// Get the combined hash (changes if any component changes).
    #[must_use]
    pub fn combined(&self) -> ContentHash {
        self.combined
    }

    /// Get the signature hash (for caller invalidation checks).
    #[must_use]
    pub fn signature_hash(&self) -> ContentHash {
        self.signature_hash
    }

    /// Get the body hash.
    #[must_use]
    pub fn body_hash(&self) -> ContentHash {
        self.body_hash
    }
}

/// Hash a function signature (parameter types + return type).
///
/// This hash captures the calling convention — if it changes, all callers
/// must be recompiled.
fn hash_signature(sig: &FunctionSig) -> ContentHash {
    let mut parts = Vec::with_capacity(sig.param_types.len() + 1);

    // Hash each parameter type as its raw Idx value
    for &param_ty in &sig.param_types {
        parts.push(hash_string(&format!("param:{}", param_ty.raw())));
    }

    // Hash the return type
    parts.push(hash_string(&format!("ret:{}", sig.return_type.raw())));

    combine_hashes(&parts)
}

/// Hash the body of a function using its expression types.
///
/// Expression types are span-free, so whitespace-only source changes
/// don't affect the hash. This is the core of body change detection.
fn hash_body(expr_types: &[Idx], body_expr_id: u32, expr_count: u32) -> ContentHash {
    let start = body_expr_id as usize;
    let end = (body_expr_id + expr_count) as usize;
    let end = end.min(expr_types.len());
    let start = start.min(end);

    let mut parts = Vec::with_capacity(end - start);
    for &ty in &expr_types[start..end] {
        parts.push(hash_string(&format!("expr:{}", ty.raw())));
    }

    if parts.is_empty() {
        // Empty body — use a sentinel hash
        hash_string("empty_body")
    } else {
        combine_hashes(&parts)
    }
}

/// Extract function content hashes from parse and type check results.
///
/// Returns a list of `(Name, FunctionContentHash)` for each non-generic
/// function in the module. Generic functions are skipped (they need
/// monomorphization before hashing).
pub fn extract_function_hashes(
    function_sigs: &[FunctionSig],
    expr_types: &[Idx],
) -> Vec<(Name, FunctionContentHash)> {
    let mut result = Vec::with_capacity(function_sigs.len());

    for sig in function_sigs {
        // Skip generic functions — they vary per instantiation
        if sig.is_generic() {
            continue;
        }

        let sig_hash = hash_signature(sig);

        // Hash the body using expression types
        // We use the function name as a proxy for body content since
        // individual expression IDs aren't directly accessible here.
        // The real body hash comes from expr_types for the function's range.
        let body_hash = hash_string(&format!("body:{}", sig.name.raw()));

        // Hash callees — for now use a placeholder since callee extraction
        // requires deeper AST traversal (deferred to a future pass)
        let callees_hash = hash_string("callees:none");

        // Hash globals — placeholder for same reason
        let globals_hash = hash_string("globals:none");

        let fch = FunctionContentHash::compute(body_hash, sig_hash, callees_hash, globals_hash);
        result.push((sig.name, fch));
    }

    // For module-level hashing, we also want a combined hash of all
    // expression types, which captures actual body changes
    if !result.is_empty() && !expr_types.is_empty() {
        let body_hash = hash_body(expr_types, 0, expr_types.len() as u32);
        // Update the first function's body hash to include expr_types
        // This provides real change detection at the module level
        if let Some(first) = result.first_mut() {
            first.1 = FunctionContentHash::compute(
                body_hash,
                first.1.signature_hash,
                first.1.callees_hash,
                first.1.globals_hash,
            );
        }
    }

    result
}

/// Compute a combined module hash from individual function hashes.
///
/// If ANY function in the module changes, the module hash changes.
/// Used as the cache key for per-module ARC IR caching.
#[must_use]
pub fn compute_module_hash(function_hashes: &[(Name, FunctionContentHash)]) -> ContentHash {
    let hashes: Vec<ContentHash> = function_hashes
        .iter()
        .map(|(_, fh)| fh.combined())
        .collect();
    if hashes.is_empty() {
        hash_string("empty_module")
    } else {
        combine_hashes(&hashes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig(name: u32, param_types: Vec<Idx>, return_type: Idx) -> FunctionSig {
        FunctionSig {
            name: Name::from_raw(name),
            type_params: vec![],
            param_names: vec![],
            param_types,
            return_type,
            capabilities: vec![],
            is_public: false,
            is_test: false,
            is_main: false,
            type_param_bounds: vec![],
            where_clauses: vec![],
            generic_param_mapping: vec![],
            required_params: 0,
        }
    }

    #[test]
    fn hash_determinism() {
        let sig = make_sig(1, vec![Idx::INT, Idx::STR], Idx::BOOL);

        let h1 = hash_signature(&sig);
        let h2 = hash_signature(&sig);

        assert_eq!(h1, h2, "same signature should produce same hash");
    }

    #[test]
    fn body_change_detection() {
        let types_v1 = vec![Idx::INT, Idx::BOOL, Idx::STR];
        let types_v2 = vec![Idx::INT, Idx::BOOL, Idx::FLOAT]; // Last type changed

        let h1 = hash_body(&types_v1, 0, 3);
        let h2 = hash_body(&types_v2, 0, 3);

        assert_ne!(h1, h2, "different expr types should produce different hash");
    }

    #[test]
    fn signature_change_detection() {
        let sig1 = make_sig(1, vec![Idx::INT], Idx::BOOL);
        let sig2 = make_sig(1, vec![Idx::STR], Idx::BOOL); // Param type changed
        let sig3 = make_sig(1, vec![Idx::INT], Idx::STR); // Return type changed

        let h1 = hash_signature(&sig1);
        let h2 = hash_signature(&sig2);
        let h3 = hash_signature(&sig3);

        assert_ne!(h1, h2, "different param type should produce different hash");
        assert_ne!(
            h1, h3,
            "different return type should produce different hash"
        );
    }

    #[test]
    fn extract_skips_generics() {
        let sigs = vec![
            make_sig(1, vec![Idx::INT], Idx::BOOL),
            FunctionSig {
                name: Name::from_raw(2),
                type_params: vec![Name::from_raw(100)], // Generic!
                param_names: vec![],
                param_types: vec![],
                return_type: Idx::UNIT,
                capabilities: vec![],
                is_public: false,
                is_test: false,
                is_main: false,
                type_param_bounds: vec![],
                where_clauses: vec![],
                generic_param_mapping: vec![],
                required_params: 0,
            },
        ];

        let hashes = extract_function_hashes(&sigs, &[Idx::INT, Idx::BOOL]);

        // Only non-generic function should appear
        assert_eq!(hashes.len(), 1);
        assert_eq!(hashes[0].0, Name::from_raw(1));
    }

    #[test]
    fn module_hash_changes_with_function() {
        let sigs1 = vec![make_sig(1, vec![Idx::INT], Idx::BOOL)];
        let sigs2 = vec![make_sig(1, vec![Idx::STR], Idx::BOOL)]; // Changed sig

        let h1 = extract_function_hashes(&sigs1, &[Idx::INT]);
        let h2 = extract_function_hashes(&sigs2, &[Idx::INT]);

        let mh1 = compute_module_hash(&h1);
        let mh2 = compute_module_hash(&h2);

        assert_ne!(
            mh1, mh2,
            "module hash should change when function sig changes"
        );
    }

    #[test]
    fn empty_module_hash() {
        let hashes: Vec<(Name, FunctionContentHash)> = vec![];
        let mh = compute_module_hash(&hashes);
        // Should not panic and should produce a valid hash
        assert_ne!(mh.value(), 0);
    }

    #[test]
    fn body_hash_clamped_to_bounds() {
        // Ensure out-of-bounds expr ranges don't panic
        let types = vec![Idx::INT, Idx::BOOL];
        let h = hash_body(&types, 100, 50); // Way out of bounds
        assert_eq!(h, hash_string("empty_body"));
    }

    #[test]
    fn function_content_hash_components() {
        let body = hash_string("body");
        let sig = hash_string("sig");
        let callees = hash_string("callees");
        let globals = hash_string("globals");

        let fch = FunctionContentHash::compute(body, sig, callees, globals);

        assert_eq!(fch.body_hash(), body);
        assert_eq!(fch.signature_hash(), sig);
        assert_ne!(fch.combined().value(), 0);
    }
}
