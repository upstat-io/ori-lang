//! The `demangle` command: decode mangled Ori symbol names.

/// Demangle an Ori symbol name.
///
/// Takes a mangled symbol like `_ori_MyModule_foo` and outputs
/// the demangled form like `MyModule.@foo`.
#[cfg(feature = "llvm")]
pub fn demangle_symbol(symbol: &str) {
    use ori_llvm::aot::{demangle, is_ori_symbol};

    if !is_ori_symbol(symbol) {
        // Not an Ori symbol, print as-is
        println!("{symbol}");
        return;
    }

    match demangle(symbol) {
        Some(demangled) => println!("{demangled}"),
        None => {
            // Couldn't demangle, print original
            println!("{symbol}");
        }
    }
}

/// Demangle when LLVM feature is not enabled.
#[cfg(not(feature = "llvm"))]
pub fn demangle_symbol(_symbol: &str) {
    eprintln!("error: the 'demangle' command requires the LLVM backend");
    eprintln!();
    eprintln!("The Ori compiler was built without LLVM support.");
    eprintln!("To enable demangling, rebuild with the 'llvm' feature:");
    eprintln!();
    eprintln!("  cargo build --features llvm");
    std::process::exit(1);
}

#[cfg(test)]
mod tests;
