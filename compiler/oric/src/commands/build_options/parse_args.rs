//! Single CLI argument parsing for build options.
//!
//! Extracted from `build_options/mod.rs` to keep the parent module
//! under the 500-line production file limit.

use std::path::PathBuf;

use super::{BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel};

/// Parse a single CLI argument into `BuildOptions`.
#[expect(
    clippy::cognitive_complexity,
    reason = "linear if/else CLI flag parser — one branch per flag"
)]
pub(super) fn parse_single_arg(options: &mut BuildOptions, arg: &str) {
    if arg == "--release" {
        options.release = true;
        options.opt_level = OptLevel::O2;
        options.debug_level = DebugLevel::None;
    } else if let Some(target) = arg.strip_prefix("--target=") {
        options.target = Some(target.to_string());
    } else if let Some(level) = arg.strip_prefix("--opt=") {
        if let Some(opt) = OptLevel::parse(level) {
            options.opt_level = opt;
            options.opt_level_explicit = true;
        } else {
            eprintln!("warning: unknown optimization level '{level}', using O0");
        }
    } else if let Some(level) = arg.strip_prefix("--debug=") {
        if let Some(dbg) = DebugLevel::parse(level) {
            options.debug_level = dbg;
            options.debug_level_explicit = true;
        } else {
            eprintln!("warning: unknown debug level '{level}', using full");
        }
    } else if let Some(output) = arg.strip_prefix("-o=") {
        options.output = Some(PathBuf::from(output));
    } else if let Some(output) = arg.strip_prefix("--output=") {
        options.output = Some(PathBuf::from(output));
    } else if let Some(dir) = arg.strip_prefix("--out-dir=") {
        options.out_dir = Some(PathBuf::from(dir));
    } else if let Some(emit) = arg.strip_prefix("--emit=") {
        if let Some(e) = EmitType::parse(emit) {
            options.emit = Some(e);
        } else {
            eprintln!("warning: unknown emit type '{emit}', options: obj, llvm-ir, llvm-bc, asm");
        }
    } else if arg == "--lib" {
        options.lib = true;
    } else if arg == "--dylib" {
        options.dylib = true;
    } else if arg == "--wasm" {
        options.wasm = true;
    } else if let Some(linker) = arg.strip_prefix("--linker=") {
        options.linker = Some(linker.to_string());
    } else if let Some(link) = arg.strip_prefix("--link=") {
        if let Some(mode) = LinkMode::parse(link) {
            options.link_mode = mode;
            options.link_mode_explicit = true;
        } else {
            eprintln!("warning: unknown link mode '{link}', using static");
        }
    } else if let Some(lto) = arg.strip_prefix("--lto=") {
        if let Some(mode) = LtoMode::parse(lto) {
            options.lto = mode;
            options.lto_explicit = true;
        } else {
            eprintln!("warning: unknown LTO mode '{lto}', using off");
        }
    } else if let Some(jobs) = arg.strip_prefix("--jobs=") {
        options.jobs_explicit = true;
        if jobs == "auto" {
            options.jobs = None;
        } else if let Ok(n) = jobs.parse() {
            options.jobs = Some(n);
        } else {
            eprintln!("warning: invalid jobs count '{jobs}', using auto");
        }
    } else if arg == "-j" {
        options.jobs = None;
        options.jobs_explicit = true;
    } else if let Some(cpu) = arg.strip_prefix("--cpu=") {
        options.cpu = Some(cpu.to_string());
    } else if let Some(features) = arg.strip_prefix("--features=") {
        options.features = Some(features.to_string());
    } else if arg == "--js-bindings" {
        options.js_bindings = true;
    } else if arg == "--wasm-opt" {
        options.wasm_opt = true;
    } else if arg == "-v" || arg == "--verbose" {
        options.verbose = true;
    } else if arg == "--no-repr-opt" {
        options.narrowing_policy = ori_repr::NarrowingPolicy::Disabled;
        options.narrowing_policy_explicit = true;
    } else if let Some(policy) = arg.strip_prefix("--repr-opt=") {
        match policy {
            "aggressive" => {
                options.narrowing_policy = ori_repr::NarrowingPolicy::Aggressive;
                options.narrowing_policy_explicit = true;
            }
            "conservative" => {
                options.narrowing_policy = ori_repr::NarrowingPolicy::Conservative;
                options.narrowing_policy_explicit = true;
            }
            "disabled" => {
                options.narrowing_policy = ori_repr::NarrowingPolicy::Disabled;
                options.narrowing_policy_explicit = true;
            }
            _ => eprintln!(
                "warning: unknown repr-opt policy '{policy}', options: aggressive, conservative, disabled"
            ),
        }
    }
}
