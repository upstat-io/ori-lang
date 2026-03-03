//! Build script for `ori_llvm`.
//!
//! Detects platform-specific native system libraries required by `ori_rt`
//! (a Rust staticlib) using `rustc --print native-static-libs`. This replaces
//! hardcoded per-platform library lists that drift across toolchain versions.

use std::env;
use std::process::{Command, Stdio};

fn main() {
    // Rebuild when toolchain or target changes
    println!("cargo:rerun-if-env-changed=RUSTUP_TOOLCHAIN");
    println!("cargo:rerun-if-env-changed=RUSTC");
    println!("cargo:rerun-if-env-changed=TARGET");

    let target = env::var("TARGET").expect("TARGET env var not set by Cargo");
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());

    let libs = detect_native_static_libs(&rustc, &target);
    println!("cargo:rustc-env=ORI_RT_NATIVE_LIBS={libs}");
}

/// Run `rustc --print native-static-libs` and parse the library names.
///
/// Parses the `note: native-static-libs: -lfoo -lbar ...` line from stderr.
/// Returns a comma-separated string of library names (without `-l` prefix).
fn detect_native_static_libs(rustc: &str, target: &str) -> String {
    // `rustc --print native-static-libs` requires input; passing `-` reads from
    // stdin. We null stdin so it gets an empty crate (enough to trigger the note).
    // Direct output to OUT_DIR to avoid polluting the source tree with
    // `librust_out.a` / `rust_out.lib` artifacts.
    let out_dir = env::var("OUT_DIR").unwrap_or_else(|_| env::temp_dir().display().to_string());
    let output = Command::new(rustc)
        .args([
            "--target",
            target,
            "--print",
            "native-static-libs",
            "--crate-type",
            "staticlib",
            "--out-dir",
            &out_dir,
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            panic!(
                "failed to run `{rustc} --print native-static-libs`: {e}\n\
                 This should never happen inside a Cargo build."
            );
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);

    // The note line looks like:
    //   note: native-static-libs: -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
    // or on Windows:
    //   note: native-static-libs: advapi32.lib bcrypt.lib ...
    for line in stderr.lines() {
        if let Some(libs_str) = line.strip_prefix("note: native-static-libs: ") {
            let libs: Vec<&str> = libs_str
                .split_whitespace()
                .filter_map(|lib| {
                    // Unix: "-lfoo" → "foo"
                    if let Some(name) = lib.strip_prefix("-l") {
                        return Some(name);
                    }
                    // Windows: "foo.lib" → "foo"
                    if let Some(name) = lib.strip_suffix(".lib") {
                        return Some(name);
                    }
                    // MSVC linker directive: "/defaultlib:msvcrt" → skip.
                    // The MSVC linker handles these implicitly via CRT selection.
                    if lib.starts_with('/') {
                        return None;
                    }
                    // Unknown format — pass through as-is
                    Some(lib)
                })
                .collect();

            return libs.join(",");
        }
    }

    panic!(
        "failed to parse native-static-libs from rustc output.\n\
         Target: {target}\n\
         Stderr:\n{stderr}\n\
         This may indicate an incompatible rustc version."
    );
}
