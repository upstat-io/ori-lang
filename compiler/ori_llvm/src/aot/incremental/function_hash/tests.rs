use super::*;

fn make_sig(name: u32, param_types: Vec<Idx>, return_type: Idx) -> FunctionSig {
    FunctionSig {
        name: Name::from_raw(name),
        type_params: vec![],
        const_params: vec![],
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
        param_defaults: vec![],
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
            const_params: vec![],
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
            param_defaults: vec![],
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
