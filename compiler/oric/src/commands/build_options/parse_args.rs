//! Single CLI argument parsing for build options.

use std::path::PathBuf;

use super::{BuildOptions, DebugLevel, EmitType, LinkMode, LtoMode, OptLevel};

/// Parse a single CLI argument into `BuildOptions`.
pub(super) fn parse_single_arg(options: &mut BuildOptions, arg: &str) {
    if parse_profile_arg(options, arg)
        || parse_output_arg(options, arg)
        || parse_link_arg(options, arg)
        || parse_target_arg(options, arg)
    {
        return;
    }
    parse_representation_arg(options, arg);
}

fn parse_profile_arg(options: &mut BuildOptions, arg: &str) -> bool {
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
    } else {
        return false;
    }
    true
}

fn parse_output_arg(options: &mut BuildOptions, arg: &str) -> bool {
    if let Some(output) = arg.strip_prefix("-o=") {
        options.output = Some(PathBuf::from(output));
    } else if let Some(output) = arg.strip_prefix("--output=") {
        options.output = Some(PathBuf::from(output));
    } else if let Some(dir) = arg.strip_prefix("--out-dir=") {
        options.out_dir = Some(PathBuf::from(dir));
    } else if let Some(remarks) = arg.strip_prefix("--emit-rc-remarks=") {
        options.emit_rc_remarks = Some(PathBuf::from(remarks));
    } else if let Some(emit) = arg.strip_prefix("--emit=") {
        if let Some(e) = EmitType::parse(emit) {
            options.emit = Some(e);
        } else {
            eprintln!("warning: unknown emit type '{emit}', options: obj, llvm-ir, llvm-bc, asm");
        }
    } else {
        return false;
    }
    true
}

fn parse_link_arg(options: &mut BuildOptions, arg: &str) -> bool {
    if arg == "--lib" {
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
    } else {
        return false;
    }
    true
}

fn parse_target_arg(options: &mut BuildOptions, arg: &str) -> bool {
    if let Some(jobs) = arg.strip_prefix("--jobs=") {
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
    } else {
        return false;
    }
    true
}

fn parse_representation_arg(options: &mut BuildOptions, arg: &str) {
    if arg == "--no-repr-opt" {
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
