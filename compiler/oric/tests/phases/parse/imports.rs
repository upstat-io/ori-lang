//! Parser tests for import items.
//!
//! Validates that the parser handles all `import_item` grammar forms:
//! - Grammar: `import_item = [ "::" ] identifier [ "without" "def" ] [ "as" identifier ] | "$" identifier .`

use crate::common::{parse_err, parse_ok};

// Constant imports: `$NAME`

#[test]
fn test_import_constant_basic() {
    let output = parse_ok("use std.config { $MAX_SIZE }\n@main () -> void = ();");
    assert_eq!(output.module.imports.len(), 1);
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_constant);
    assert!(!items[0].is_private);
    assert!(!items[0].without_def);
    assert!(items[0].alias.is_none());
}

#[test]
fn test_import_constant_multiple() {
    let output = parse_ok("use std.config { $TIMEOUT, $MAX_RETRIES }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 2);
    assert!(items[0].is_constant);
    assert!(items[1].is_constant);
}

#[test]
fn test_import_constant_mixed_with_regular() {
    let output = parse_ok("use \"./module\" { add, $LIMIT }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 2);
    assert!(!items[0].is_constant);
    assert!(items[1].is_constant);
}

#[test]
fn test_import_constant_mixed_with_private() {
    let output = parse_ok("use \"./module\" { ::internal, $LIMIT }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 2);
    assert!(items[0].is_private);
    assert!(!items[0].is_constant);
    assert!(!items[1].is_private);
    assert!(items[1].is_constant);
}

// Without def imports: `Trait without def`

#[test]
fn test_import_without_def_basic() {
    let output = parse_ok("use std.net.http { Http without def }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].without_def);
    assert!(!items[0].is_private);
    assert!(!items[0].is_constant);
    assert!(items[0].alias.is_none());
}

#[test]
fn test_import_without_def_with_alias() {
    let output = parse_ok("use std.net.http { Http without def as H }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].without_def);
    assert!(items[0].alias.is_some());
}

#[test]
fn test_import_without_def_mixed_with_regular() {
    let output = parse_ok("use std.net { Http without def, connect }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 2);
    assert!(items[0].without_def);
    assert!(!items[1].without_def);
}

#[test]
fn test_import_without_def_private() {
    let output = parse_ok("use \"./module\" { ::Trait without def }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_private);
    assert!(items[0].without_def);
}

// Private imports: `::name` (regression guard — already working)

#[test]
fn test_import_private_basic() {
    let output = parse_ok("use \"./module\" { ::internal }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_private);
    assert!(!items[0].is_constant);
    assert!(!items[0].without_def);
}

#[test]
fn test_import_private_with_alias() {
    let output = parse_ok("use \"./module\" { ::internal as helper }\n@main () -> void = ();");
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 1);
    assert!(items[0].is_private);
    assert!(items[0].alias.is_some());
}

// Combined imports: mix of all forms

#[test]
fn test_import_all_forms_combined() {
    let output = parse_ok(
        "use \"./module\" { add, ::internal, $LIMIT, Trait without def }\n@main () -> void = ();",
    );
    let items = &output.module.imports[0].items;
    assert_eq!(items.len(), 4);

    // Regular
    assert!(!items[0].is_private);
    assert!(!items[0].is_constant);
    assert!(!items[0].without_def);

    // Private
    assert!(items[1].is_private);
    assert!(!items[1].is_constant);
    assert!(!items[1].without_def);

    // Constant
    assert!(!items[2].is_private);
    assert!(items[2].is_constant);
    assert!(!items[2].without_def);

    // Without def
    assert!(!items[3].is_private);
    assert!(!items[3].is_constant);
    assert!(items[3].without_def);
}

// Error cases

#[test]
fn test_import_without_missing_def() {
    // `without` without `def` should error (after consuming `without` as ident,
    // parser expects `def` keyword)
    parse_err(
        "use std.net { Http without }\n@main () -> void = ();",
        "expected def",
    );
}
