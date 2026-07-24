//! Emitted-artifact pins for the JIT test-runner emission path.
//!
//! `ori test --backend=llvm` reaches LLVM emission through its own driver,
//! separate from the `ori build` codegen pipeline. These pins drive the real
//! worker entry point on a real source file and assert the module it actually
//! emitted, so dropping the prepared-body batch, the derive batch, the test
//! wrapper batch, emitting a body twice, or widening the nounwind batch past
//! its own analysis cannot ship unnoticed.
#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixed per-run nonce (a real parent generates a fresh one per spawn).
const TOKEN: &str = "j1t3m1tt3d4rt1f4ct0123456789abcd";

/// Env var carrying the per-spawn protocol nonce (`debug_flags::ORI_TEST_PROTOCOL_TOKEN`).
const TOKEN_VAR: &str = "ORI_TEST_PROTOCOL_TOKEN";

/// Env var gating the emitted-module dump (`debug_flags::ORI_DUMP_AFTER_LLVM`).
const DUMP_VAR: &str = "ORI_DUMP_AFTER_LLVM";

/// Header the emitted-module dump opens with.
const DUMP_HEADER: &str = "LLVM IR:";

/// A user function, a derive-bearing type, an entry point, and a test — one
/// source exercising every emission batch the JIT driver runs.
const FIXTURE: &str = r"use std.testing { assert_eq };

type Pair = { left: int, right: int };

#derive(Eq)
type Tagged = { value: int };

pub @doubled (n: int) -> int = n * 2;

@combined (p: Pair) -> int = doubled(n: p.left) + p.right;

@main () -> int = combined(p: Pair { left: 20, right: 2 });

@t_combined tests @combined () -> void = {
    assert_eq(actual: combined(p: Pair { left: 20, right: 2 }), expected: 42);
}
";

fn stdlib_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../library")
        .canonicalize()
        .unwrap_or_else(|e| panic!("the workspace stdlib directory must resolve: {e}"))
}

fn write_fixture(dir: &tempfile::TempDir, name: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, FIXTURE).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

/// Drive the real JIT worker entry point with the emitted-module dump enabled
/// and return the module text it printed.
fn emitted_module(fixture: &Path) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("test")
        .arg("--backend=llvm")
        .arg("--__worker")
        .arg(fixture)
        .env(TOKEN_VAR, TOKEN)
        .env("ORI_STDLIB", stdlib_path())
        .env(DUMP_VAR, "1")
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn the real ori binary: {e}"));

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        output.status.success(),
        "the real JIT worker must succeed on the fixture: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status
    );
    assert!(
        stdout.contains("result t_combined pass"),
        "the emitted module must actually run the fixture's test:\nstdout: {stdout}"
    );

    let start = stderr
        .find(DUMP_HEADER)
        .unwrap_or_else(|| panic!("the JIT driver must emit the module dump:\n{stderr}"));
    // The driver prints the module through its escaped debug form; restore the
    // literal text so definition headers can be scanned directly.
    stderr[start + DUMP_HEADER.len()..]
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
}

/// One emitted definition: the symbol its header names and the attribute
/// group that header references. LLVM quotes a name containing mangling
/// separators, so both spellings are accepted.
struct Definition {
    name: String,
    attribute_group: Option<String>,
}

fn definitions(module: &str) -> Vec<Definition> {
    module
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("define "))
        .filter_map(|header| {
            let rest = &header[header.find('@')? + 1..];
            let (name, after) = match rest.strip_prefix('"') {
                Some(quoted) => quoted.split_once('"')?,
                None => rest.split_once('(')?,
            };
            let attribute_group = after
                .rsplit_once('#')
                .and_then(|(_, tail)| tail.split_whitespace().next())
                .map(str::to_owned);
            Some(Definition {
                name: name.to_owned(),
                attribute_group,
            })
        })
        .collect()
}

fn defined_symbols(module: &str) -> Vec<String> {
    definitions(module)
        .into_iter()
        .map(|definition| definition.name)
        .collect()
}

/// The attribute list the named definition's header references, empty when the
/// header references no attribute group at all.
fn attributes_of(module: &str, symbol: &str) -> String {
    let Some(group) = definitions(module)
        .into_iter()
        .find(|definition| definition.name == symbol)
        .unwrap_or_else(|| panic!("the emitted JIT module must define `{symbol}`:\n{module}"))
        .attribute_group
    else {
        return String::new();
    };
    let marker = format!("attributes #{group} = {{");
    let start = module.find(&marker).unwrap_or_else(|| {
        panic!("the emitted JIT module must declare attribute group #{group}:\n{module}")
    }) + marker.len();
    let body = &module[start..];
    let end = body
        .find('}')
        .unwrap_or_else(|| panic!("attribute group #{group} must terminate:\n{module}"));
    body[..end].trim().to_owned()
}

fn assert_defines(module: &str, symbol: &str, why: &str) {
    let symbols = defined_symbols(module);
    assert!(
        symbols.iter().any(|name| name == symbol),
        "the emitted JIT module must define `{symbol}` ({why}); it defined {symbols:?}"
    );
}

/// The JIT driver's emitted module carries every function the source declares,
/// the derived-trait method, and the test wrapper. Dropping any emission batch
/// from the shared projection removes one of these definitions.
#[test]
fn jit_worker_emits_every_declared_body_derive_and_test_wrapper() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let module = emitted_module(&write_fixture(&dir, "jit_emitted_artifact.ori"));

    assert_defines(&module, "_ori_doubled", "an ordinary prepared body");
    assert_defines(&module, "_ori_combined", "the body its test exercises");
    assert_defines(&module, "_ori_main", "the entry point");
    assert_defines(
        &module,
        "_ori_eq$24derived$240",
        "the derived-trait method batch",
    );
    assert_defines(
        &module,
        "_ori_test_t_combined_body",
        "the test wrapper batch",
    );
    assert_defines(
        &module,
        "_ori_test_t_combined",
        "the test wrapper's unwind-catching entry",
    );
}

/// The nounwind batch stays no wider than what it proves. A body reaching a
/// panicking overflow path, and a body calling one, both stay unwind-capable
/// in the emitted module; widening the batch past its own analysis marks them.
#[test]
fn jit_worker_leaves_panicking_bodies_unwind_capable() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let module = emitted_module(&write_fixture(&dir, "jit_nounwind.ori"));

    for symbol in ["_ori_doubled", "_ori_combined"] {
        let attributes = attributes_of(&module, symbol);
        assert!(
            !attributes.contains("nounwind"),
            "`{symbol}` reaches a panicking overflow path, so it must stay \
             unwind-capable; got `{attributes}`"
        );
    }
}

/// A module the JIT driver emits declares no body twice. A collapse that
/// re-ran a per-function emission batch would produce a duplicate definition
/// that LLVM's own verifier accepts only because the second one renames.
#[test]
fn jit_worker_emits_each_declared_body_exactly_once() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let module = emitted_module(&write_fixture(&dir, "jit_no_duplicates.ori"));

    let symbols = defined_symbols(&module);
    for symbol in [
        "_ori_doubled",
        "_ori_combined",
        "_ori_main",
        "_ori_test_t_combined_body",
    ] {
        let defines = symbols.iter().filter(|name| *name == symbol).count();
        assert_eq!(
            defines, 1,
            "the emitted JIT module must define `{symbol}` exactly once, found {defines} in {symbols:?}"
        );
    }
}
