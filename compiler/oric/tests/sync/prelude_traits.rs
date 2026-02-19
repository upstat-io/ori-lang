//! Prelude sync validation — Section 05.2.
//!
//! Verifies that `library/std/prelude.ori` defines all derived traits
//! with matching trait names and method names from `DerivedTrait::ALL`.

use ori_ir::DerivedTrait;

/// Path to prelude relative to the workspace root.
fn prelude_path() -> std::path::PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    std::path::Path::new(&manifest)
        .parent()
        .expect("oric has no parent")
        .parent()
        .expect("compiler has no parent")
        .join("library/std/prelude.ori")
}

#[test]
fn prelude_defines_all_derived_traits() {
    let prelude_path = prelude_path();
    let prelude = std::fs::read_to_string(&prelude_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", prelude_path.display()));

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = trait_kind.trait_name();
        let method_name = trait_kind.method_name();

        // Check that the trait is defined in prelude
        let trait_pattern = format!("pub trait {trait_name}");
        assert!(
            prelude.contains(&trait_pattern),
            "prelude.ori missing trait definition for '{trait_name}'"
        );

        // Check that the method is declared
        let method_pattern = format!("@{method_name}");
        assert!(
            prelude.contains(&method_pattern),
            "prelude.ori missing method '@{method_name}' for trait '{trait_name}'"
        );
    }
}
