//! Cross-driver parity for the shared pre-codegen realization seam.
//!
//! Three drivers reach the same realization entry: the AOT driver
//! (`ori build`), the JIT test-runner driver (`ori test --backend=llvm
//! --__worker`), and the VM-corpus driver
//! (`oric::test_support::compile_to_executable`). Each pin below asserts one
//! input the drivers must agree on — the realized function and monomorphic
//! instance inventory, the externally reachable roots, the command-line entry,
//! the narrowing policy and the analysis-only-functions verdict that together
//! govern the representation plan, the ARC verification gate, and the
//! imported-callable input the drivers still supply differently.
#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Fixed per-run nonce (a real parent generates a fresh one per spawn).
const TOKEN: &str = "r34l1z3dpr0gr4mp4r1ty0123456789a";

/// Env var carrying the per-spawn protocol nonce (`debug_flags::ORI_TEST_PROTOCOL_TOKEN`).
const TOKEN_VAR: &str = "ORI_TEST_PROTOCOL_TOKEN";

/// Env var gating the realized-artifact dump (`debug_flags::ORI_DUMP_AFTER_ARC`).
const DUMP_VAR: &str = "ORI_DUMP_AFTER_ARC";

/// Header the realized-artifact dump opens with.
const DUMP_HEADER: &str = "=== ARC IR after lowering:";

/// Footer closing the realized-artifact dump.
const DUMP_FOOTER: &str = "=== END ARC IR ===";

/// Name the AOT driver writes its emitted binary under.
const AOT_BINARY_NAME: &str = "parity_aot";

/// The one test body the JIT driver realizes and the AOT driver does not.
const TEST_BODY: &str = "scaled_point_uses_the_doubled_factor";

/// Carries two non-`main` parent functions plus a non-generic impl method, so
/// the roots set and the analysis-only-functions verdict are both observable.
const PARITY_FIXTURE: &str = r#"
type Point = { x: int, y: int }

impl Point {
    @scaled (self, factor: int) -> int = self.x * factor + self.y;
}

@double (n: int) -> int = n * 2;

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    p.scaled(factor: double(n: 3))
}

@scaled_point_uses_the_doubled_factor tests @double () -> void = {
    if double(n: 3) != 6 then panic(msg: "double must return twice its input")
}
"#;

/// Same shape without a command-line entry: realization must still succeed and
/// report no entry rather than demanding an interned `main`.
const NO_ENTRY_FIXTURE: &str = r"
@double (n: int) -> int = n * 2;

@triple (n: int) -> int = n * 3;
";

/// The three monomorphic instances every driver must realize from
/// [`MONO_FIXTURE`]'s single generic definition.
const MONO_INSTANCES: [&str; 3] = ["identity$m$3_int", "identity$m$3_str", "identity$m$4_bool"];

/// Instantiates one generic at three distinct types, so the realized artifact
/// carries a monomorphic-instance inventory rather than only plain bodies.
const MONO_FIXTURE: &str = r#"
type Point = { x: int, y: int }

impl Point {
    @scaled (self, factor: int) -> int = self.x * factor + self.y;
}

@identity<T> (value: T) -> T = value;

@double (n: int) -> int = n * 2;

@main () -> int = {
    let p = Point { x: identity(value: 1), y: 2 };
    let tag = identity(value: true);
    let label = identity(value: "k");
    if tag && label != "" then p.scaled(factor: double(n: 3)) else 0
}

@mono_instances_round_trip tests @identity () -> void = {
    if identity(value: 7) != 7 then panic(msg: "the int instance must round-trip")
}
"#;

/// Carries a multi-field struct and no impl method, so the narrowing verdict
/// follows the driver-selected policy instead of analysis-only suppression.
const LAYOUT_FIXTURE: &str = r"
type Counter = { hits: int, misses: int }

@bump (c: Counter) -> Counter = Counter { ...c, hits: c.hits + 1 };

@main () -> int = {
    let c = Counter { hits: 1, misses: 2 };
    let d = bump(c: c);
    d.hits + d.misses
}
";

/// Exports one callable the importing fixture reaches across a module boundary.
const IMPORTED_HELPER: &str = "pub @triple (n: int) -> int = n * 3;\n";

/// Calls an imported callable from both the entry point and a declared test,
/// so each driver's imported-callable path is exercised end to end.
const IMPORTING_FIXTURE: &str = r#"
use "./helper" { triple };

@double (n: int) -> int = n * 2;

@main () -> int = triple(n: double(n: 2));

@imported_callable_round_trips tests @double () -> void = {
    if triple(n: 2) != 6 then panic(msg: "the imported callable must be realized")
}
"#;

/// Exit status the [`IMPORTING_FIXTURE`] binary reports: `triple(double(2))`.
const IMPORTING_FIXTURE_EXIT: i32 = 12;

/// Tail of the module-qualified name the JIT driver re-lowers `triple` under.
const IMPORTED_CALLABLE_SUFFIX: &str = "$function$triple";

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

fn run_to_output(cmd: &mut Command) -> Output {
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to spawn real ori binary: {e}"))
}

/// Drive the real AOT entry point with the realized-artifact dump enabled.
fn aot_dump(fixture: &Path, out_dir: &Path) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("build")
        .arg(fixture)
        .arg("-o")
        .arg(out_dir.join(AOT_BINARY_NAME))
        .env(DUMP_VAR, "1");
    let output = run_to_output(&mut cmd);
    assert!(
        output.status.success(),
        "`ori build` must succeed on the parity fixture: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Drive the real JIT test-runner worker entry point with the dump enabled.
fn jit_dump(fixture: &Path) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test")
        .arg("--backend=llvm")
        .arg("--__worker")
        .arg(fixture)
        .env(TOKEN_VAR, TOKEN)
        .env(DUMP_VAR, "1");
    let output = run_to_output(&mut cmd);
    assert!(
        output.status.success(),
        "the real JIT worker must succeed on the parity fixture: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Extract every realized body name from the compiled module's dump.
///
/// A module that imports another emits one dump per module in dependency
/// order, so the last block is always the module named on the command line.
fn dumped_body_names(dump: &str, driver: &str) -> Vec<String> {
    let start = dump.rfind(DUMP_HEADER).unwrap_or_else(|| {
        panic!("the {driver} driver must emit the realized-artifact dump:\n{dump}")
    });
    let block = &dump[start..];
    let end = block.find(DUMP_FOOTER).unwrap_or(block.len());
    let mut names: Vec<String> = block[..end]
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("fn @"))
        .filter_map(|rest| rest.split('(').next())
        .map(str::to_owned)
        .collect();
    assert!(
        !names.is_empty(),
        "the {driver} realized-artifact dump must list bodies:\n{dump}"
    );
    names.sort_unstable();
    names
}

/// Names of every externally reachable root in the realized artifact.
fn root_names(program: &ori_repr::executable::ExecutableProgram) -> Vec<String> {
    let symbols = program.symbols();
    program
        .roots()
        .iter()
        .map(|&root| symbols.lookup(program.function(root).name).to_owned())
        .collect()
}

fn realize_through_vm_corpus_driver(
    name: &str,
    source: &str,
) -> ori_repr::executable::ExecutableProgram {
    realize_through_vm_corpus_driver_under(name, source, ori_repr::NarrowingPolicy::Aggressive)
}

/// Drive the VM-corpus entry point under one driver-selected narrowing policy.
fn realize_through_vm_corpus_driver_under(
    name: &str,
    source: &str,
    narrowing: ori_repr::NarrowingPolicy,
) -> ori_repr::executable::ExecutableProgram {
    oric::test_support::compile_to_executable(name, source, narrowing)
        .unwrap_or_else(|error| panic!("the VM-corpus driver must realize `{name}`: {error}"))
}

/// Names of every realized body in the VM-corpus driver's artifact.
fn realized_body_names(program: &ori_repr::executable::ExecutableProgram) -> Vec<String> {
    let symbols = program.symbols();
    let mut names: Vec<String> = program
        .functions()
        .iter()
        .map(|function| symbols.lookup(function.name).to_owned())
        .collect();
    names.sort_unstable();
    names
}

/// Byte offsets of every struct field the realized representation plan carries.
fn struct_field_offsets(program: &ori_repr::executable::ExecutableProgram) -> Vec<u32> {
    let plan = program.repr_plan();
    plan.decision_indices()
        .filter_map(|idx| match plan.get_repr(idx) {
            Some(ori_repr::MachineRepr::Struct(layout)) => Some(layout),
            _ => None,
        })
        .flat_map(|layout| layout.fields.iter().map(|field| field.offset))
        .collect()
}

/// Drive the real JIT worker entry point with additional environment.
fn jit_dump_with(fixture: &Path, extra: &[(&str, &str)]) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test")
        .arg("--backend=llvm")
        .arg("--__worker")
        .arg(fixture)
        .env(TOKEN_VAR, TOKEN)
        .env(DUMP_VAR, "1");
    for (key, value) in extra {
        cmd.env(key, value);
    }
    let output = run_to_output(&mut cmd);
    assert!(
        output.status.success(),
        "the real JIT worker must succeed with {extra:?}: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The JIT driver must reach the same realized-artifact observation surface as
/// the AOT driver, and both must realize the same body inventory.
#[test]
fn aot_and_jit_drivers_realize_the_same_body_inventory() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "parity.ori", PARITY_FIXTURE);

    let aot = aot_dump(&fixture, dir.path());
    let jit = jit_dump(&fixture);

    let aot_bodies = dumped_body_names(&aot, "AOT");
    let jit_bodies = dumped_body_names(&jit, "JIT");

    for symbol in ["main", "double"] {
        assert!(
            aot_bodies.iter().any(|name| name == symbol),
            "the AOT realized artifact must contain `{symbol}`: {aot_bodies:?}"
        );
        assert!(
            jit_bodies.iter().any(|name| name == symbol),
            "the JIT realized artifact must contain `{symbol}`: {jit_bodies:?}"
        );
    }

    // The JIT driver additionally realizes declared test bodies; every other
    // body must be realized identically by both drivers.
    let mut expected_jit = aot_bodies;
    expected_jit.push(TEST_BODY.to_owned());
    expected_jit.sort_unstable();
    assert_eq!(
        jit_bodies, expected_jit,
        "AOT and JIT must realize identical bodies apart from declared test bodies"
    );
}

/// Every non-lambda parent is externally reachable, not just the entry point.
#[test]
fn vm_corpus_driver_roots_cover_every_parent_not_only_the_entry() {
    let program = realize_through_vm_corpus_driver("parity.ori", PARITY_FIXTURE);
    let roots = root_names(&program);

    assert!(
        roots.iter().any(|name| name == "main"),
        "the command-line entry must be a root: {roots:?}"
    );
    assert!(
        roots.iter().any(|name| name == "double"),
        "every non-lambda parent must be a root, not only the entry: {roots:?}"
    );
    assert!(
        program.cli_entry().is_some(),
        "a module declaring `@main` must realize a command-line entry"
    );
}

/// A module whose impl methods enter analysis without a codegen body must
/// suppress narrowing on every driver, not only on the compiled ones.
#[test]
fn analysis_only_impl_methods_suppress_narrowing_on_the_vm_corpus_driver() {
    let program = realize_through_vm_corpus_driver("parity.ori", PARITY_FIXTURE);

    assert!(
        !program.repr_plan().is_narrowing_safe_for_codegen(),
        "a module with non-generic impl methods carries analysis-only functions, \
         so narrowing must be suppressed exactly as the compiled drivers suppress it"
    );
}

/// Negative pin: absence of `@main` is a realized artifact without a
/// command-line entry, never a missing-root realization failure.
#[test]
fn module_without_an_entry_realizes_without_a_command_line_entry() {
    let program = realize_through_vm_corpus_driver("no_entry.ori", NO_ENTRY_FIXTURE);
    let roots = root_names(&program);

    assert_eq!(
        program.cli_entry(),
        None,
        "a module without `@main` must realize no command-line entry"
    );
    assert!(
        roots.iter().any(|name| name == "double") && roots.iter().any(|name| name == "triple"),
        "every parent must remain externally reachable without an entry: {roots:?}"
    );
}

/// One generic definition must yield the same monomorphic-instance inventory on
/// the AOT driver, the JIT driver, and the VM-corpus driver.
#[test]
fn every_driver_realizes_the_same_monomorphic_instance_inventory() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "mono.ori", MONO_FIXTURE);

    let aot_bodies = dumped_body_names(&aot_dump(&fixture, dir.path()), "AOT");
    let jit_bodies = dumped_body_names(&jit_dump(&fixture), "JIT");
    let vm_bodies =
        realized_body_names(&realize_through_vm_corpus_driver("mono.ori", MONO_FIXTURE));

    // Guards the equalities below against passing over an inventory that
    // realized no instance at all.
    for instance in MONO_INSTANCES {
        for (driver, bodies) in [
            ("AOT", &aot_bodies),
            ("JIT", &jit_bodies),
            ("VM-corpus", &vm_bodies),
        ] {
            assert!(
                bodies.iter().any(|name| name == instance),
                "the {driver} driver must realize the `{instance}` instance: {bodies:?}"
            );
        }
    }

    assert_eq!(
        vm_bodies, aot_bodies,
        "the VM-corpus and AOT drivers must realize identical inventories"
    );

    let mut expected_jit = aot_bodies;
    expected_jit.push("mono_instances_round_trip".to_owned());
    expected_jit.sort_unstable();
    assert_eq!(
        jit_bodies, expected_jit,
        "the JIT driver must add only its declared test bodies"
    );
}

/// The driver-selected narrowing policy must reach the realized representation
/// plan: a disabled policy stops before the struct-layout pass, so no field
/// receives a computed offset.
#[test]
fn the_driver_selected_narrowing_policy_reaches_the_realized_representation_plan() {
    let aggressive = realize_through_vm_corpus_driver_under(
        "layout.ori",
        LAYOUT_FIXTURE,
        ori_repr::NarrowingPolicy::Aggressive,
    );
    let disabled = realize_through_vm_corpus_driver_under(
        "layout.ori",
        LAYOUT_FIXTURE,
        ori_repr::NarrowingPolicy::Disabled,
    );

    assert!(
        struct_field_offsets(&aggressive).iter().any(|&o| o > 0),
        "an aggressive policy must run the struct-layout pass and place fields"
    );
    assert!(
        struct_field_offsets(&disabled).iter().all(|&o| o == 0),
        "a disabled policy must reach the plan and leave every field unplaced"
    );
    assert!(
        aggressive.repr_plan().is_narrowing_safe_for_codegen(),
        "a module without impl methods carries no analysis-only functions"
    );
}

/// The ARC oracle gate is a verification switch, not a content switch: the real
/// JIT worker realizes the same inventory whether or not it is set.
///
/// A green fixture exposes no observable difference between running and not
/// running the oracle, so this pin covers the content claim only; whether the
/// evaluator's gate agrees with the gate the artifact was realized under stays
/// a source-level property of the driver that resolves the policy once.
#[test]
fn the_arc_verification_gate_does_not_change_the_realized_inventory() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "parity.ori", PARITY_FIXTURE);

    let ungated = dumped_body_names(&jit_dump_with(&fixture, &[]), "JIT");
    let gated = dumped_body_names(
        &jit_dump_with(&fixture, &[("ORI_VERIFY_ARC", "1")]),
        "JIT (verified)",
    );

    assert_eq!(
        gated, ungated,
        "enabling the ARC oracle must verify the artifact, never change it"
    );
}

/// The externals input is the one realization input the drivers still supply
/// differently, because only the AOT driver links a separately compiled module.
/// The difference is confined to the imported callable: the AOT driver realizes
/// it as an external, the JIT driver re-lowers it as a module-qualified local
/// body, every other local body matches, and both execute the imported call.
#[test]
fn the_externals_divergence_is_confined_to_the_imported_callable() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    write_fixture(&dir, "helper.ori", IMPORTED_HELPER);
    let fixture = write_fixture(&dir, "importing.ori", IMPORTING_FIXTURE);
    let binary = dir.path().join(AOT_BINARY_NAME);

    let aot_bodies = dumped_body_names(&aot_dump(&fixture, dir.path()), "AOT");
    let jit_bodies = dumped_body_names(&jit_dump(&fixture), "JIT");

    assert!(
        !aot_bodies.iter().any(|name| name.ends_with("triple")),
        "the AOT driver realizes an imported callable as an external: {aot_bodies:?}"
    );
    assert_eq!(
        jit_bodies
            .iter()
            .filter(|name| name.ends_with(IMPORTED_CALLABLE_SUFFIX))
            .count(),
        1,
        "the JIT driver re-lowers exactly the imported callable: {jit_bodies:?}"
    );

    let jit_local: Vec<String> = jit_bodies
        .iter()
        .filter(|name| !name.ends_with(IMPORTED_CALLABLE_SUFFIX))
        .filter(|name| name.as_str() != "imported_callable_round_trips")
        .cloned()
        .collect();
    assert_eq!(
        jit_local, aot_bodies,
        "outside the imported callable the drivers must realize identical bodies"
    );

    // The JIT worker already ran the fixture's imported-callable test, which
    // panics on a wrong value; running the emitted binary proves the AOT
    // driver's external reaches the same callable.
    let status = run_to_output(&mut Command::new(&binary)).status;
    assert_eq!(
        status.code(),
        Some(IMPORTING_FIXTURE_EXIT),
        "the emitted binary must call the imported callable: {status:?}"
    );
}
