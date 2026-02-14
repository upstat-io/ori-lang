//! Parser tests for file-level attributes.
//!
//! Validates that the parser correctly handles `#!` file-level attributes:
//! - Grammar: `file_attribute = "#!" identifier "(" [ attribute_arg { "," attribute_arg } ] ")" .`
//! - Only `target` and `cfg` are valid at file level
//! - Must appear before imports and declarations

use crate::common::{parse_err, parse_ok};
use ori_ir::FileAttr;

// File-level target attribute

#[test]
fn test_file_attr_target_os() {
    let output = parse_ok("#!target(os: \"linux\")\n@main () -> void = ()");
    assert!(output.module.file_attr.is_some());
    let attr = output.module.file_attr.unwrap();
    match attr {
        FileAttr::Target { os, arch, .. } => {
            assert!(os.is_some(), "os should be set");
            assert!(arch.is_none(), "arch should not be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target file attribute"),
    }
}

#[test]
fn test_file_attr_target_arch() {
    let output = parse_ok("#!target(arch: \"x86_64\")\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { os, arch, .. } => {
            assert!(os.is_none());
            assert!(arch.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_multiple_params() {
    let output = parse_ok("#!target(os: \"linux\", arch: \"x86_64\")\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { os, arch, .. } => {
            assert!(os.is_some());
            assert!(arch.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_family() {
    let output = parse_ok("#!target(family: \"unix\")\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { family, .. } => {
            assert!(family.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_not_os() {
    let output = parse_ok("#!target(not_os: \"windows\")\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { not_os, .. } => {
            assert!(not_os.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

// File-level cfg attribute

#[test]
fn test_file_attr_cfg_debug() {
    let output = parse_ok("#!cfg(debug)\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { debug, release, .. } => {
            assert!(debug, "debug should be true");
            assert!(!release, "release should be false");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_release() {
    let output = parse_ok("#!cfg(release)\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { debug, release, .. } => {
            assert!(!debug);
            assert!(release);
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_feature() {
    let output = parse_ok("#!cfg(feature: \"logging\")\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { feature, .. } => {
            assert!(feature.is_some());
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_not_debug() {
    let output = parse_ok("#!cfg(not_debug)\n@main () -> void = ()");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { not_debug, .. } => {
            assert!(not_debug);
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

// No file attribute

#[test]
fn test_no_file_attr() {
    let output = parse_ok("@main () -> void = ()");
    assert!(output.module.file_attr.is_none());
}

#[test]
fn test_no_file_attr_with_item_attrs() {
    let output = parse_ok("#skip(\"reason\")\n@test_foo () -> void = ()");
    assert!(
        output.module.file_attr.is_none(),
        "item-level # should not be parsed as file-level #!"
    );
}

// File attribute with imports and declarations

#[test]
fn test_file_attr_before_imports() {
    let output =
        parse_ok("#!target(os: \"linux\")\nuse std.testing { assert }\n@main () -> void = ()");
    assert!(output.module.file_attr.is_some());
    assert_eq!(output.module.imports.len(), 1);
}

// Error cases

#[test]
fn test_file_attr_invalid_derive() {
    parse_err(
        "#!derive(Eq)\n@main () -> void = ()",
        "not valid as a file-level attribute",
    );
}

#[test]
fn test_file_attr_invalid_skip() {
    parse_err(
        "#!skip(\"reason\")\n@main () -> void = ()",
        "not valid as a file-level attribute",
    );
}

#[test]
fn test_file_attr_invalid_repr() {
    parse_err(
        "#!repr(\"c\")\n@main () -> void = ()",
        "not valid as a file-level attribute",
    );
}

#[test]
fn test_file_attr_unknown_name() {
    parse_err("#!foobar()\n@main () -> void = ()", "unknown attribute");
}
