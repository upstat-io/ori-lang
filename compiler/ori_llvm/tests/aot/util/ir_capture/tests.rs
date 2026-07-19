use super::{extract_function_ir, resolve_derived_function_name, resolve_function_attrs};

#[test]
fn derived_function_lookup_reaches_quoted_artifact_body_and_attributes() {
    let ir = r#"
define i1 @_ori_main() #0 {
entry:
  %result = call fastcc i1 @"_ori_eq$24derived$240"(ptr null, ptr null)
  ret i1 %result
}

define fastcc i1 @"_ori_eq$24derived$240"(ptr %left, ptr %right) #1 {
entry:
  ret i1 true
}

attributes #0 = { nounwind }
attributes #1 = { nounwind }
"#;

    let symbol = resolve_derived_function_name(ir, "eq");
    assert_eq!(symbol, "_ori_eq$24derived$240");
    let body = extract_function_ir(ir, symbol);
    assert!(body.contains("ret i1 true"));
    assert!(!body.contains("call fastcc"));
    assert!(resolve_function_attrs(ir, symbol).contains("nounwind"));
}
