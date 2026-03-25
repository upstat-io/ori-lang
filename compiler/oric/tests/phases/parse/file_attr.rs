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
    let output = parse_ok("#!target(os: \"linux\")\n@main () -> void = ();");
    assert!(output.module.file_attr.is_some());
    let attr = output.module.file_attr.unwrap();
    match attr {
        FileAttr::Target { attr: target, span } => {
            assert!(target.os.is_some(), "os should be set");
            assert!(target.arch.is_none(), "arch should not be set");
            assert!(span.start < span.end, "span should be non-empty");
        }
        FileAttr::Cfg { .. } => panic!("expected Target file attribute"),
    }
}

#[test]
fn test_file_attr_target_arch() {
    let output = parse_ok("#!target(arch: \"x86_64\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.os.is_none());
            assert!(target.arch.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_multiple_params() {
    let output = parse_ok("#!target(os: \"linux\", arch: \"x86_64\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.os.is_some());
            assert!(target.arch.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_family() {
    let output = parse_ok("#!target(family: \"unix\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.family.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_not_os() {
    let output = parse_ok("#!target(not_os: \"windows\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.not_os.is_some());
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

// File-level cfg attribute

#[test]
fn test_file_attr_cfg_debug() {
    let output = parse_ok("#!cfg(debug)\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.debug, "debug should be true");
            assert!(!cfg.release, "release should be false");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_release() {
    let output = parse_ok("#!cfg(release)\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(!cfg.debug);
            assert!(cfg.release);
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_feature() {
    let output = parse_ok("#!cfg(feature: \"logging\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.feature.is_some());
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_cfg_not_debug() {
    let output = parse_ok("#!cfg(not_debug)\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.not_debug);
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

// No file attribute

#[test]
fn test_no_file_attr() {
    let output = parse_ok("@main () -> void = ();");
    assert!(output.module.file_attr.is_none());
}

#[test]
fn test_no_file_attr_with_item_attrs() {
    let output = parse_ok("#skip(\"reason\")\n@test_foo tests _ () -> void = ();");
    assert!(
        output.module.file_attr.is_none(),
        "item-level # should not be parsed as file-level #!"
    );
}

// File attribute with imports and declarations

#[test]
fn test_file_attr_before_imports() {
    let output =
        parse_ok("#!target(os: \"linux\")\nuse std.testing { assert }\n@main () -> void = ();");
    assert!(output.module.file_attr.is_some());
    assert_eq!(output.module.imports.len(), 1);
}

// Error cases

#[test]
fn test_file_attr_invalid_derive() {
    parse_err(
        "#!derive(Eq)\n@main () -> void = ();",
        "not valid as a file-level attribute",
    );
}

#[test]
fn test_file_attr_invalid_skip() {
    parse_err(
        "#!skip(\"reason\")\n@main () -> void = ();",
        "not valid as a file-level attribute",
    );
}

#[test]
fn test_file_attr_invalid_repr() {
    parse_err(
        "#!repr(\"c\")\n@main () -> void = ();",
        "not valid as a file-level attribute",
    );
}

// TPR-01-053: Full conditional-attribute matrix (Spec §25)

#[test]
fn test_file_attr_target_any_os() {
    let output = parse_ok("#!target(any_os: [\"linux\", \"macos\"])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert_eq!(target.any_os.len(), 2, "any_os should have 2 entries");
            assert!(target.os.is_none(), "os should not be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_any_arch() {
    let output = parse_ok("#!target(any_arch: [\"x86_64\", \"aarch64\"])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert_eq!(target.any_arch.len(), 2, "any_arch should have 2 entries");
            assert!(target.arch.is_none(), "arch should not be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_not_arch() {
    let output = parse_ok("#!target(not_arch: \"wasm32\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.not_arch.is_some(), "not_arch should be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_not_family() {
    let output = parse_ok("#!target(not_family: \"wasm\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.not_family.is_some(), "not_family should be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_cfg_any_feature() {
    let output = parse_ok("#!cfg(any_feature: [\"ssl\", \"tls\"])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert_eq!(
                cfg.any_feature.len(),
                2,
                "any_feature should have 2 entries"
            );
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

// Edge cases for list parsing

#[test]
fn test_file_attr_target_any_os_single_element() {
    let output = parse_ok("#!target(any_os: [\"linux\"])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert_eq!(target.any_os.len(), 1, "any_os should have 1 entry");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

#[test]
fn test_file_attr_target_any_os_trailing_comma() {
    let output = parse_ok("#!target(any_os: [\"linux\", \"macos\",])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert_eq!(target.any_os.len(), 2, "trailing comma should be allowed");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

// Combined: new params alongside existing params

#[test]
fn test_file_attr_target_not_arch_with_os() {
    let output = parse_ok("#!target(os: \"linux\", not_arch: \"wasm32\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Target { attr: target, .. } => {
            assert!(target.os.is_some(), "os should be set");
            assert!(target.not_arch.is_some(), "not_arch should be set");
        }
        FileAttr::Cfg { .. } => panic!("expected Target"),
    }
}

// TPR-01-055: Feature name identifier validation (Spec §25.3.2)

#[test]
fn test_cfg_feature_valid_identifier() {
    let output = parse_ok("#!cfg(feature: \"ssl\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.feature.is_some(), "feature should be set");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_cfg_feature_valid_with_underscore() {
    let output = parse_ok("#!cfg(feature: \"_private_feat\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.feature.is_some(), "feature should be set");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_cfg_feature_valid_with_digits() {
    let output = parse_ok("#!cfg(feature: \"Feature123\")\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert!(cfg.feature.is_some(), "feature should be set");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_cfg_feature_invalid_hyphen() {
    parse_err(
        "#!cfg(feature: \"invalid-name\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_feature_invalid_starts_with_digit() {
    parse_err(
        "#!cfg(feature: \"123start\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_feature_invalid_special_chars() {
    parse_err(
        "#!cfg(feature: \"feat!@#\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_feature_invalid_dot() {
    parse_err(
        "#!cfg(feature: \"my.feature\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_feature_invalid_empty() {
    parse_err(
        "#!cfg(feature: \"\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_not_feature_invalid() {
    parse_err(
        "#!cfg(not_feature: \"bad-name\")\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_any_feature_invalid_in_list() {
    parse_err(
        "#!cfg(any_feature: [\"good\", \"bad-name\"])\n@main () -> void = ();",
        "invalid feature name",
    );
}

#[test]
fn test_cfg_any_feature_all_valid() {
    let output =
        parse_ok("#!cfg(any_feature: [\"ssl\", \"tls\", \"_internal\"])\n@main () -> void = ();");
    match output.module.file_attr.unwrap() {
        FileAttr::Cfg { attr: cfg, .. } => {
            assert_eq!(cfg.any_feature.len(), 3, "all 3 valid features accepted");
        }
        FileAttr::Target { .. } => panic!("expected Cfg"),
    }
}

#[test]
fn test_file_attr_unknown_name() {
    parse_err("#!foobar()\n@main () -> void = ();", "unknown attribute");
}

// TPR-01-054: Item-level conditional attributes (Spec §25.4)

#[test]
fn test_item_target_on_function() {
    let output = parse_ok("#target(os: \"linux\")\n@platform_func () -> void = ();");
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    assert!(
        func.target_attr.is_some(),
        "function should have target_attr"
    );
    let target = func.target_attr.as_ref().unwrap();
    assert!(target.os.is_some(), "target os should be set");
}

#[test]
fn test_item_cfg_on_function() {
    let output = parse_ok("#cfg(debug)\n@debug_func () -> void = ();");
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    assert!(func.cfg_attr.is_some(), "function should have cfg_attr");
    let cfg = func.cfg_attr.as_ref().unwrap();
    assert!(cfg.debug, "cfg debug should be true");
}

#[test]
fn test_item_cfg_feature_on_function() {
    let output = parse_ok("#cfg(feature: \"ssl\")\n@ssl_func () -> void = ();");
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    assert!(func.cfg_attr.is_some(), "function should have cfg_attr");
    let cfg = func.cfg_attr.as_ref().unwrap();
    assert!(cfg.feature.is_some(), "cfg feature should be set");
}

#[test]
fn test_item_target_on_type() {
    let output = parse_ok("#target(os: \"windows\")\ntype Handle = int;");
    assert_eq!(output.module.types.len(), 1);
    let ty = &output.module.types[0];
    assert!(ty.target_attr.is_some(), "type should have target_attr");
    let target = ty.target_attr.as_ref().unwrap();
    assert!(target.os.is_some(), "target os should be set");
}

#[test]
fn test_item_cfg_on_type() {
    let output = parse_ok("#cfg(debug)\ntype DebugConfig = { verbose: bool };");
    assert_eq!(output.module.types.len(), 1);
    let ty = &output.module.types[0];
    assert!(ty.cfg_attr.is_some(), "type should have cfg_attr");
    let cfg = ty.cfg_attr.as_ref().unwrap();
    assert!(cfg.debug, "cfg debug should be true");
}

#[test]
fn test_function_without_conditional_attrs() {
    let output = parse_ok("@main () -> void = ();");
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    assert!(func.target_attr.is_none(), "no target_attr expected");
    assert!(func.cfg_attr.is_none(), "no cfg_attr expected");
}

#[test]
fn test_item_target_multifield_on_function() {
    let output = parse_ok("#target(os: \"linux\", arch: \"x86_64\")\n@f () -> void = ();");
    assert_eq!(output.module.functions.len(), 1);
    let func = &output.module.functions[0];
    let target = func.target_attr.as_ref().unwrap();
    assert!(target.os.is_some(), "os should be set");
    assert!(target.arch.is_some(), "arch should be set");
}
