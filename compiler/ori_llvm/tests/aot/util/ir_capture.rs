//! IR capture and inspection utilities for AOT integration tests.
//!
//! Provides `compile_and_capture_ir()`, `extract_function_ir()`, and
//! structural IR analysis helpers (`count_bridge_blocks`, etc.).

use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use ori_llvm::aot::mangle::Mangler;
use tempfile::TempDir;

use super::binary::{ir_capture_binary, ori_binary, stdlib_path};

/// Compile an Ori program and capture its LLVM IR (via `ORI_DEBUG_LLVM=1`).
///
/// Uses the debug `ori` binary for IR capture. Returns the IR string from
/// compilation stderr. Panics if compilation fails.
pub fn compile_and_capture_ir(source: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("test_ir_{id}.ori"));
    let binary_path = temp_dir
        .path()
        .join(format!("test_ir_{id}{}", std::env::consts::EXE_SUFFIX));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ir_capture_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "-o",
            binary_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .env("ORI_DEBUG_LLVM", "1")
        .output()
        .expect("Failed to execute ori build");

    assert!(
        compile_result.status.success(),
        "Compilation failed:\n{}",
        String::from_utf8_lossy(&compile_result.stderr)
    );

    String::from_utf8_lossy(&compile_result.stderr).to_string()
}

/// Extract a single function's LLVM IR from a full module dump.
///
/// Finds the exact `define ... @func_name(` line and returns that function's
/// body. Calls and declarations using the same symbol do not match.
pub fn extract_function_ir<'a>(full_ir: &'a str, func_name: &str) -> &'a str {
    let mut offset = 0;
    let define_start = full_ir
        .split_inclusive('\n')
        .find_map(|line| {
            let line_start = offset;
            offset += line.len();
            (line.trim_start().starts_with("define ")
                && llvm_function_symbol(line) == Some(func_name))
            .then(|| line_start + line.find("define ").unwrap_or(0))
        })
        .unwrap_or_else(|| {
            panic!(
                "function {func_name} not found in IR.\n\
             Available functions: {:?}",
                full_ir
                    .lines()
                    .filter_map(llvm_function_symbol)
                    .collect::<Vec<_>>()
            )
        });

    let rest = &full_ir[define_start..];
    let end = rest.find("\n}").map_or(rest.len(), |pos| pos + 2);

    &full_ir[define_start..define_start + end]
}

/// Resolve the unique closed-executable body for a derived method.
///
/// Derived methods use collision-safe artifact identities such as
/// `eq$derived$0`; they do not retain the source-level `Type.eq` symbol. The
/// fixtures using this helper must therefore contain exactly one realized
/// derived body for `method_name`.
pub fn resolve_derived_function_name<'a>(ir: &'a str, method_name: &str) -> &'a str {
    let artifact_prefix = Mangler::new().mangle_function("", &format!("{method_name}$derived$"));
    let mut resolved = None;

    for symbol in ir.lines().filter_map(llvm_function_symbol) {
        let Some(id) = symbol.strip_prefix(&artifact_prefix) else {
            continue;
        };
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        if let Some(previous) = resolved {
            assert!(
                previous == symbol,
                "multiple derived {method_name} functions found in IR: \
                 {previous} and {symbol}"
            );
        } else {
            resolved = Some(symbol);
        }
    }

    resolved.unwrap_or_else(|| {
        panic!(
            "derived {method_name} function not found in IR.\n\
             Available functions: {:?}",
            ir.lines()
                .filter_map(llvm_function_symbol)
                .collect::<Vec<_>>()
        )
    })
}

fn llvm_function_symbol(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if !line.starts_with("define ") && !line.starts_with("declare ") {
        return None;
    }

    let symbol = line.split_once('@')?.1;
    if let Some(quoted) = symbol.strip_prefix('"') {
        let end = quoted.find("\"(")?;
        Some(&quoted[..end])
    } else {
        let end = symbol.find('(')?;
        Some(&symbol[..end])
    }
}

/// Resolve a function's attributes through its LLVM `#N` attribute group.
///
/// Searches declarations and definitions and accepts both plain and quoted
/// symbol spellings.
pub fn resolve_function_attrs(ir: &str, func_name: &str) -> String {
    let search_plain = format!("@{func_name}(");
    let search_quoted = format!("@\"{func_name}\"(");
    let declaration = ir
        .lines()
        .find(|line| {
            (line.contains("declare") || line.contains("define"))
                && (line.contains(&search_plain) || line.contains(&search_quoted))
        })
        .unwrap_or_else(|| panic!("{func_name} should be declared or defined in IR"));

    let line = declaration.trim_end_matches('{').trim();
    let group_ref = line
        .rsplit_once('#')
        .map(|(_, number)| format!("#{}", number.trim()))
        .unwrap_or_default();
    if group_ref.is_empty() {
        return String::new();
    }

    let group_prefix = format!("attributes {group_ref} = ");
    ir.lines()
        .find(|line| line.starts_with(&group_prefix))
        .map(|line| line[group_prefix.len()..].to_string())
        .unwrap_or_default()
}

/// Count "bridge-only" blocks in a function's LLVM IR.
///
/// A bridge-only block is one whose only non-comment, non-blank instruction
/// is `br label %target` — no phi, no computation, no conditional branch.
/// These represent redundant blocks from invoke splitting that should have
/// been merged by the ARC block merge pass.
pub fn count_bridge_blocks(function_ir: &str) -> usize {
    let mut bridge_count = 0;
    let mut in_block = false;
    let mut block_instrs: Vec<&str> = Vec::new();

    for line in function_ir.lines() {
        let trimmed = line.trim();

        // Detect block start: a label line (e.g., "bb1:", "entry:", "4:")
        let is_label =
            trimmed.ends_with(':') && !trimmed.starts_with(';') && !trimmed.contains("  ; preds");

        // Also match "labelname: ; preds = ..." pattern
        let is_label = is_label
            || trimmed.contains(':') && !trimmed.starts_with(';') && {
                let colon_pos = trimmed.find(':');
                colon_pos.is_some_and(|p| {
                    let before = &trimmed[..p];
                    !before.is_empty()
                        && before
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '%')
                })
            };

        if is_label || trimmed == "{" {
            // Flush previous block.
            if in_block && is_bridge_only(&block_instrs) {
                bridge_count += 1;
            }
            block_instrs.clear();
            in_block = is_label;
            continue;
        }

        if trimmed == "}" {
            // End of function — flush.
            if in_block && is_bridge_only(&block_instrs) {
                bridge_count += 1;
            }
            break;
        }

        if in_block {
            block_instrs.push(trimmed);
        }
    }

    bridge_count
}

/// Check if a block's instructions consist of only an unconditional `br label`.
fn is_bridge_only(instrs: &[&str]) -> bool {
    let meaningful: Vec<&&str> = instrs
        .iter()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty() && !l.starts_with(';')
        })
        .collect();

    meaningful.len() == 1 && meaningful[0].starts_with("br label ")
}

/// Count phi nodes with exactly one incoming edge in a function's IR.
///
/// A single-predecessor phi `%x = phi T [ %v, %bb ]` with one incoming
/// edge is redundant — equivalent to using `%v` directly.
pub fn count_single_pred_phis(function_ir: &str) -> usize {
    function_ir
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with(';') {
                return false;
            }
            if !trimmed.contains(" = phi ") {
                return false;
            }
            // Single incoming edge = exactly one `[` bracket pair.
            trimmed.matches('[').count() == 1
        })
        .count()
}

/// Count phi nodes whose result has no uses in the function.
///
/// A dead phi `%vN = phi T [...]` defines a value that is never referenced
/// as an operand in any other instruction.
pub fn count_dead_phis(function_ir: &str) -> usize {
    let lines: Vec<&str> = function_ir.lines().collect();

    lines
        .iter()
        .enumerate()
        .filter(|(i, line)| {
            let trimmed = line.trim();
            if !trimmed.contains(" = phi ") || trimmed.starts_with(';') {
                return false;
            }
            let name = match trimmed.split_whitespace().next() {
                Some(n) if n.starts_with('%') => n,
                _ => return false,
            };
            // Dead if name never appears in any OTHER non-comment line.
            !lines.iter().enumerate().any(|(j, other)| {
                if i == &j {
                    return false;
                }
                let ot = other.trim();
                if ot.starts_with(';') {
                    return false;
                }
                is_ssa_var_used_in(name, ot)
            })
        })
        .count()
}

/// Check whether `%name` appears as a standalone SSA variable in `line`.
/// Requires a word boundary so `%v3` does not match `%v34`.
fn is_ssa_var_used_in(var_name: &str, line: &str) -> bool {
    let mut search_from = 0;
    while let Some(pos) = line[search_from..].find(var_name) {
        let abs_pos = search_from + pos;
        let after = abs_pos + var_name.len();
        let next_char = line[after..].chars().next();
        let is_boundary = next_char.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if is_boundary {
            return true;
        }
        search_from = abs_pos + 1;
    }
    false
}

/// Compile an Ori program to LLVM IR and return the IR text.
///
/// Returns `Ok(ir_text)` on success, `Err(stderr)` on compilation failure.
#[must_use = "success or failure must be handled"]
pub fn compile_to_llvm_ir(source: &str) -> Result<String, String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let source_path = temp_dir.path().join(format!("ir_test_{id}.ori"));
    let ir_path = temp_dir.path().join(format!("ir_test_{id}.ll"));

    fs::write(&source_path, source).expect("Failed to write source");

    let compile_result = Command::new(ori_binary())
        .args([
            "build",
            source_path.to_str().unwrap(),
            "--emit=llvm-ir",
            "-o",
            ir_path.to_str().unwrap(),
        ])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("Failed to execute ori build");

    if !compile_result.status.success() {
        return Err(String::from_utf8_lossy(&compile_result.stderr).to_string());
    }

    fs::read_to_string(&ir_path).map_err(|e| format!("Failed to read IR file: {e}"))
}

#[cfg(test)]
mod tests;
