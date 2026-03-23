fn main() {
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let is_msvc = target_env == "msvc";

    let mut build = cc::Build::new();

    // --- EH implementation (platform-split) ---
    //
    // MSVC: eh_personality_msvc.cpp — C++ throw/catch so that LLVM's
    //   __CxxFrameHandler3 personality executes cleanuppad blocks during
    //   unwind. Win32 RaiseException bypasses C++ frame handlers.
    //
    // Others: eh_personality.c — Itanium personality + _Unwind_RaiseException.
    if is_msvc {
        build.file("src/eh_personality_msvc.cpp");
        build.cpp(true);
        build.flag("/EHsc"); // Enable C++ exception handling
    } else {
        build.file("src/eh_personality.c");
        build.flag_if_supported("-std=c11");
        build.pic(true); // Required: Rust links test binaries as PIE
    }

    build.warnings(true).extra_warnings(true);

    // Ensure all C functions have unwind tables so the platform unwinder
    // can walk through them. Critical for ori_raise_exception: it calls
    // _Unwind_RaiseException, and the unwinder must be able to traverse
    // its frame to reach LLVM-generated catch landing pads.
    //
    // NOTE: Do NOT add -fno-exceptions here — it suppresses .eh_frame
    // generation, which prevents the unwinder from walking through
    // ori_raise_exception, causing _URC_END_OF_STACK (code 5).
    if !is_msvc {
        build.flag("-funwind-tables");
    }

    // --- Forced-unwind test harness (Linux x86-64 + aarch64) ---
    //
    // Compiled into the same archive as the personality function.
    // The unit test module (`forced_unwind_tests` in lib.rs) calls these
    // C/assembly test functions.
    //
    // On non-test builds, the linker dead-strips unreferenced symbols.
    // On non-Linux or unsupported architectures, the test harness files
    // are omitted and the Rust test module is #[cfg]-gated to skip.
    if target_os == "linux" {
        let asm_file = match target_arch.as_str() {
            "x86_64" => Some("src/test_frames_x86_64.S"),
            "aarch64" => Some("src/test_frames_aarch64.S"),
            _ => None,
        };
        if let Some(asm) = asm_file {
            build
                .file("src/test_forced_unwind.c")
                .file(asm)
                // Ensure C test functions have proper unwind tables so
                // the unwinder can traverse them during forced unwind.
                .flag("-funwind-tables");
        }
    }

    build.compile("ori_eh");

    // Ensure Cargo rebuilds when C/C++/asm sources change.
    println!("cargo:rerun-if-changed=src/eh_personality.c");
    println!("cargo:rerun-if-changed=src/eh_personality_msvc.cpp");
    println!("cargo:rerun-if-changed=src/test_forced_unwind.c");
    println!("cargo:rerun-if-changed=src/test_frames_x86_64.S");
    println!("cargo:rerun-if-changed=src/test_frames_aarch64.S");
}
