//! AOT pins for Duration/Size factory associated functions
//! (`Duration.from_seconds`, `Size.from_kilobytes`, ...).
//!
//! Real AOT compile + run + leak-check (`assert_cell_output` enables
//! `ORI_CHECK_LEAKS=1`) asserting the exact base-unit value — the runtime gate
//! the §09.2 typeck pin (`ori_types` `s09_2_builtin_duration_ctor_records_instance`)
//! cannot reach. The factories lower via `try_emit_builtin_associated`
//! (`codegen/arc_emitter/builtins/associated.rs`); the printed value is the
//! negative pin against a wrong scale factor. Duration is i64 nanoseconds, Size
//! is i64 bytes; the `from_*` multipliers mirror `ori_eval`'s `duration_from_int`
//! / `size_from_int` (`ori_eval/src/methods/units.rs`).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_cell_output;

// Every Duration `from_*` factory, read back in base-unit nanoseconds.
const DURATION_FACTORIES_SRC: &str = r#"
@main () -> void = print(msg: `{Duration.from_nanoseconds(ns: 7).nanoseconds()} {Duration.from_microseconds(us: 4).nanoseconds()} {Duration.from_milliseconds(ms: 3).nanoseconds()} {Duration.from_seconds(s: 5).nanoseconds()} {Duration.from_minutes(m: 2).nanoseconds()} {Duration.from_hours(h: 1).nanoseconds()}`);
"#;

#[test]
fn test_duration_from_factories_aot() {
    assert_cell_output(
        DURATION_FACTORIES_SRC,
        "duration_from_factories",
        "7 4000 3000000 5000000000 120000000000 3600000000000",
    );
}

// Every Size `from_*` factory, read back in base-unit bytes (SI 1000-based).
const SIZE_FACTORIES_SRC: &str = r#"
@main () -> void = print(msg: `{Size.from_bytes(b: 9).bytes()} {Size.from_kilobytes(kb: 3).bytes()} {Size.from_megabytes(mb: 5).bytes()} {Size.from_gigabytes(gb: 1).bytes()} {Size.from_terabytes(tb: 2).bytes()}`);
"#;

#[test]
fn test_size_from_factories_aot() {
    assert_cell_output(
        SIZE_FACTORIES_SRC,
        "size_from_factories",
        "9 3000 5000000 1000000000 2000000000000",
    );
}

// Round-trip through the unit accessor: factory scale then inverse accessor.
const FACTORY_ROUNDTRIP_SRC: &str = r#"
@main () -> void = {
    let d = Duration.from_seconds(s: 5);
    let sz = Size.from_kilobytes(kb: 3);
    print(msg: `{d.seconds()} {sz.kilobytes()}`)
}
"#;

#[test]
fn test_unit_factory_accessor_roundtrip_aot() {
    assert_cell_output(FACTORY_ROUNDTRIP_SRC, "unit_factory_roundtrip", "5 3");
}
