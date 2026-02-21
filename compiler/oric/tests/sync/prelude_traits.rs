//! Prelude sync validation — Section 05.2.
//!
//! Verifies that `library/std/prelude.ori` defines all derived traits
//! with matching trait names, method names, and supertrait constraints
//! from `DerivedTrait::ALL`.

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

/// Check if `text` contains `pub` + whitespace + `trait` + whitespace + `name`
/// followed by a non-alphanumeric character (word boundary).
fn has_pub_trait_def(text: &str, name: &str) -> bool {
    let bytes = text.as_bytes();
    let pub_bytes = b"pub";
    let trait_bytes = b"trait";

    let mut i = 0;
    while i + pub_bytes.len() < bytes.len() {
        // Find "pub"
        if &bytes[i..i + pub_bytes.len()] != pub_bytes {
            i += 1;
            continue;
        }
        let mut j = i + pub_bytes.len();

        // Skip whitespace (at least one)
        let ws_start = j;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j == ws_start {
            i += 1;
            continue;
        }

        // Match "trait"
        if j + trait_bytes.len() > bytes.len() || &bytes[j..j + trait_bytes.len()] != trait_bytes {
            i += 1;
            continue;
        }
        j += trait_bytes.len();

        // Skip whitespace (at least one)
        let ws_start = j;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j == ws_start {
            i += 1;
            continue;
        }

        // Match trait name
        let name_bytes = name.as_bytes();
        if j + name_bytes.len() > bytes.len() || &bytes[j..j + name_bytes.len()] != name_bytes {
            i += 1;
            continue;
        }
        j += name_bytes.len();

        // Word boundary: next char must not be alphanumeric or underscore
        if j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
            i += 1;
            continue;
        }

        return true;
    }
    false
}

/// Check if `text` contains `@` + `method_name` followed by whitespace or `(`.
/// This anchors to method declarations rather than stray mentions in comments.
fn has_method_decl(text: &str, method_name: &str) -> bool {
    let pattern = format!("@{method_name}");
    let mut start = 0;
    while let Some(pos) = text[start..].find(&pattern) {
        let abs = start + pos + pattern.len();
        if abs >= text.len() {
            return true; // at end of file — still a match
        }
        let next = text.as_bytes()[abs];
        if next == b'(' || next.is_ascii_whitespace() {
            return true;
        }
        start = start + pos + 1;
    }
    false
}

/// Check if `text` contains `trait` + ws + `name` + optional-ws + `:` + optional-ws + `super_name`.
fn has_supertrait(text: &str, trait_name: &str, super_name: &str) -> bool {
    let pattern = format!("trait {trait_name}");
    let mut start = 0;
    while let Some(pos) = text[start..].find(&pattern) {
        let abs = start + pos + pattern.len();
        let rest = &text[abs..];

        // Skip optional whitespace
        let rest = rest.trim_start();

        // Expect ':'
        if let Some(rest) = rest.strip_prefix(':') {
            // Skip optional whitespace
            let rest = rest.trim_start();

            // Match supertrait name with word boundary
            if rest.starts_with(super_name) {
                let after = rest.as_bytes().get(super_name.len());
                if after.is_none()
                    || !(after.unwrap().is_ascii_alphanumeric() || *after.unwrap() == b'_')
                {
                    return true;
                }
            }
        }
        start = start + pos + 1;
    }
    false
}

#[test]
fn prelude_defines_all_derived_traits() {
    let prelude_path = prelude_path();
    let prelude = std::fs::read_to_string(&prelude_path)
        .unwrap_or_else(|e| panic!("Cannot read {}: {e}", prelude_path.display()));

    for &trait_kind in DerivedTrait::ALL {
        let trait_name = trait_kind.trait_name();
        let method_name = trait_kind.method_name();

        // Whitespace-tolerant trait definition check
        assert!(
            has_pub_trait_def(&prelude, trait_name),
            "prelude.ori missing trait definition for '{trait_name}'"
        );

        // Method declaration anchored to `@name(` or `@name ` (not stray mentions)
        assert!(
            has_method_decl(&prelude, method_name),
            "prelude.ori missing method '@{method_name}' for trait '{trait_name}'"
        );

        // Supertrait constraint validation
        if let Some(required) = trait_kind.requires_supertrait() {
            assert!(
                has_supertrait(&prelude, trait_name, required.trait_name()),
                "prelude.ori missing supertrait constraint '{trait_name}: {}' \
                 (required by DerivedTrait metadata)",
                required.trait_name()
            );
        }
    }
}
