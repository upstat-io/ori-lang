//! Cross-crate enforcement: every `PRELUDE_FUNCTIONS` entry must be registered
//! in the evaluator's `register_prelude()`.
//!
//! This test reads the evaluator source file and verifies that each canonical
//! prelude function name from `ori_registry` appears as a `register_function_val`
//! call. This structurally guards against drift between the type checker (which
//! queries `PRELUDE_FUNCTIONS`) and the evaluator (which independently registers
//! runtime function values).

/// Verify every entry in `PRELUDE_FUNCTIONS` has a corresponding
/// `register_function_val` call in the evaluator's prelude source.
#[test]
fn prelude_functions_registered_in_evaluator() {
    let prelude_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ori_eval/src/interpreter/prelude.rs"
    ))
    .expect("should be able to read evaluator prelude source");

    let mut missing = Vec::new();

    for func in ori_registry::PRELUDE_FUNCTIONS {
        // Look for register_function_val("name", ...) in the source
        let pattern = format!("register_function_val(\"{}\",", func.name);
        if !prelude_src.contains(&pattern) {
            missing.push(func.name);
        }
    }

    assert!(
        missing.is_empty(),
        "PRELUDE_FUNCTIONS entries missing from evaluator register_prelude(): {missing:?}\n\
         Each canonical prelude function must have a register_function_val() call \
         in compiler/ori_eval/src/interpreter/prelude.rs"
    );
}

/// Verify the evaluator doesn't register conversion functions that are NOT
/// in `PRELUDE_FUNCTIONS` (negative: catches stale evaluator entries).
#[test]
fn evaluator_conversion_functions_in_registry() {
    let prelude_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../ori_eval/src/interpreter/prelude.rs"
    ))
    .expect("should be able to read evaluator prelude source");

    // Known evaluator-only registrations that are NOT prelude conversion functions:
    // - "Error" is a type constructor, not a conversion function
    // - "thread_id" is a runtime function, not a prelude conversion
    let evaluator_only = ["Error", "thread_id"];

    // Extract all register_function_val("name", ...) names from source
    let mut eval_names: Vec<&str> = Vec::new();
    for line in prelude_src.lines() {
        if let Some(start) = line.find("register_function_val(\"") {
            let rest = &line[start + "register_function_val(\"".len()..];
            if let Some(end) = rest.find('"') {
                eval_names.push(&rest[..end]);
            }
        }
    }

    let registry_names: Vec<&str> = ori_registry::PRELUDE_FUNCTIONS
        .iter()
        .map(|f| f.name)
        .collect();

    let mut untracked = Vec::new();
    for name in &eval_names {
        if !registry_names.contains(name) && !evaluator_only.contains(name) {
            untracked.push(*name);
        }
    }

    assert!(
        untracked.is_empty(),
        "Evaluator registers functions not in PRELUDE_FUNCTIONS or evaluator_only: {untracked:?}\n\
         Either add them to PRELUDE_FUNCTIONS (for type-checked prelude functions) \
         or to evaluator_only (for runtime-only registrations)."
    );
}
