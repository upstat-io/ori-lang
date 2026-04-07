//! Linker driver orchestration — command construction, execution, retry, response files.

use std::ffi::OsString;
use std::io::Write;
use std::process::{Command, Output};

use super::{
    GccLinker, LinkInput, LinkerDetection, LinkerDriver, LinkerError, LinkerFlavor, LinkerImpl,
    MsvcLinker, WasmLinker,
};

impl LinkerDriver {
    /// Link object files into an executable or library.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The linker is not found
    /// - Linking fails
    /// - Response file creation fails
    pub fn link(&self, input: &LinkInput) -> Result<(), LinkerError> {
        // Validate input
        if input.objects.is_empty() {
            return Err(LinkerError::InvalidConfig {
                message: "no object files to link".to_string(),
            });
        }

        // Select linker flavor (with cross-compilation-aware fallback)
        let flavor = if let Some(explicit) = input.linker {
            // User explicitly chose a linker — respect it
            explicit
        } else {
            let preferred = LinkerFlavor::for_target(self.target.components());
            if LinkerDetection::is_available_for_target(preferred, &self.target) {
                preferred
            } else {
                // Fall back to next available linker for this target,
                // using cross-compilation-aware detection.
                let detection = LinkerDetection::detect_for_target(&self.target);
                if let Some(fallback) = detection.preferred() {
                    fallback
                } else if LinkerDetection::is_cross_compiling(&self.target) {
                    // No suitable cross-linker found — fail early with clear error.
                    return Err(LinkerDetection::cross_compilation_error(&self.target));
                } else {
                    // Native compilation but no linker at all.
                    return Err(LinkerError::LinkerNotFound {
                        linker: format!("{preferred:?}"),
                        message: format!(
                            "no linker found for native target '{}'. \
                             Install a C compiler (gcc or clang) to enable linking.",
                            self.target.triple()
                        ),
                    });
                }
            }
        };

        // Create the appropriate linker using enum dispatch (not trait objects)
        // This provides exhaustiveness checking, static dispatch, and no heap allocation
        let mut linker = self.create_linker(flavor);

        // Configure linker
        Self::configure_linker(&mut linker, input)?;

        // Get the final command
        let cmd = linker.finalize();

        // Execute with retry logic
        self.execute_with_retry(cmd, input, false)
    }

    /// Configure the linker with all input settings.
    pub fn configure_linker(linker: &mut LinkerImpl, input: &LinkInput) -> Result<(), LinkerError> {
        // Set output kind first (affects other options)
        linker.set_output_kind(input.output_kind);

        // Add object files
        for obj in &input.objects {
            linker.add_object(obj);
        }

        // Add library search paths
        for path in &input.library_paths {
            linker.add_library_path(path);
        }

        // Add libraries
        for lib in &input.libraries {
            if let Some(ref path) = lib.search_path {
                linker.add_library_path(path);
            }
            linker.link_library(&lib.name, lib.kind);
        }

        // Configure optimizations
        if input.gc_sections {
            linker.gc_sections(true);
        }

        if input.strip {
            linker.strip_symbols(true);
        }

        // Export symbols
        if !input.exported_symbols.is_empty() {
            linker.export_symbols(&input.exported_symbols);
        }

        // Add extra arguments
        for arg in &input.extra_args {
            linker.add_arg(arg);
        }

        // Set output last (some linkers are order-sensitive)
        linker.set_output(&input.output);

        Ok(())
    }

    /// Create the appropriate linker implementation for the given flavor.
    fn create_linker(&self, flavor: LinkerFlavor) -> LinkerImpl {
        let cross = LinkerDetection::is_cross_compiling(&self.target);

        match flavor {
            LinkerFlavor::Gcc => {
                if cross {
                    // Use the target-prefixed cross-compiler (e.g., aarch64-linux-gnu-gcc)
                    if let Some(name) =
                        LinkerDetection::gcc_cross_compiler_name(self.target.components())
                    {
                        LinkerImpl::Gcc(GccLinker::with_path(&self.target, &name))
                    } else {
                        LinkerImpl::Gcc(GccLinker::new(&self.target))
                    }
                } else {
                    LinkerImpl::Gcc(GccLinker::new(&self.target))
                }
            }
            LinkerFlavor::Lld => {
                if self.target.is_windows() {
                    LinkerImpl::Msvc(MsvcLinker::with_lld(&self.target))
                } else if self.target.is_wasm() {
                    LinkerImpl::Wasm(WasmLinker::new(&self.target))
                } else {
                    // Use clang with -fuse-ld=lld
                    let mut gcc = GccLinker::with_path(&self.target, "clang");
                    gcc.cmd().arg("-fuse-ld=lld");
                    LinkerImpl::Gcc(gcc)
                }
            }
            LinkerFlavor::Msvc => LinkerImpl::Msvc(MsvcLinker::new(&self.target)),
            LinkerFlavor::WasmLd => LinkerImpl::Wasm(WasmLinker::new(&self.target)),
        }
    }

    /// Execute linker with retry logic for common failures.
    fn execute_with_retry(
        &self,
        cmd: Command,
        input: &LinkInput,
        is_retry: bool,
    ) -> Result<(), LinkerError> {
        // Check if we need to use a response file
        let cmd = self.maybe_use_response_file(cmd)?;

        // First attempt
        let output = Self::run_linker(&cmd)?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        // Combine stdout and stderr for pattern matching: MSVC link.exe sends
        // error details (LNK2019 unresolved symbols) to stdout, not stderr.
        // We check both streams for retryable patterns.
        let combined = format!("{stderr}\n{stdout}");

        // Check for retryable errors — only retry once to prevent infinite loops.
        if !is_retry && Self::should_retry(&combined) {
            // Retry with adjusted options
            return self.retry_link(input, &combined);
        }

        // Linking failed
        Err(LinkerError::LinkFailed {
            linker: cmd.get_program().to_string_lossy().into(),
            exit_code: output.status.code(),
            stdout,
            stderr,
            command: format!("{cmd:?}"),
        })
    }

    /// Run the linker and capture output.
    fn run_linker(cmd: &Command) -> Result<Output, LinkerError> {
        // Clone the command for execution
        // Note: Command doesn't implement Clone, so we need to reconstruct
        let program = cmd.get_program().to_owned();
        let args: Vec<OsString> = cmd.get_args().map(ToOwned::to_owned).collect();

        let mut exec_cmd = Command::new(program);
        exec_cmd.args(args);

        exec_cmd.output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                LinkerError::LinkerNotFound {
                    linker: cmd.get_program().to_string_lossy().into(),
                    message: e.to_string(),
                }
            } else {
                LinkerError::IoError {
                    message: e.to_string(),
                }
            }
        })
    }

    /// Check if the error is retryable.
    pub fn should_retry(stderr: &str) -> bool {
        // Common retryable errors (from rustc's experience)
        let retryable_patterns = [
            "unrecognized option",
            "unknown option",
            "-no-pie",
            "-static-pie",
            "-fuse-ld=lld",
        ];

        retryable_patterns
            .iter()
            .any(|pattern| stderr.contains(pattern))
    }

    /// Retry linking with adjusted options (at most once).
    fn retry_link(&self, input: &LinkInput, _stderr: &str) -> Result<(), LinkerError> {
        // Create new input with adjusted settings
        let mut adjusted = input.clone();

        // Remove potentially problematic extra args
        adjusted.extra_args.retain(|arg| {
            !arg.contains("-no-pie")
                && !arg.contains("-static-pie")
                && !arg.contains("-fuse-ld=lld")
        });

        // If we were using LLD, fall back to default
        if adjusted.linker == Some(LinkerFlavor::Lld) {
            adjusted.linker = Some(LinkerFlavor::Gcc);
        }

        // Rebuild command and retry — `is_retry: true` prevents further retries
        let flavor = adjusted.linker.unwrap_or_else(|| {
            let preferred = LinkerFlavor::for_target(self.target.components());
            if LinkerDetection::is_available_for_target(preferred, &self.target) {
                preferred
            } else {
                LinkerDetection::detect_for_target(&self.target)
                    .preferred()
                    .unwrap_or(preferred)
            }
        });
        let mut linker = self.create_linker(flavor);
        Self::configure_linker(&mut linker, &adjusted)?;
        let cmd = linker.finalize();
        self.execute_with_retry(cmd, &adjusted, true)
    }

    /// Use a response file if the command line is too long.
    fn maybe_use_response_file(&self, cmd: Command) -> Result<Command, LinkerError> {
        // Estimate command line length
        let args: Vec<_> = cmd.get_args().collect();
        let total_len: usize = args.iter().map(|a| a.len() + 1).sum();

        // Platform-specific limits:
        // - Unix: ~2MB (ARG_MAX)
        // - Windows cmd.exe: ~8KB
        // - Windows CreateProcess: ~32KB
        let limit = if self.target.is_windows() {
            8 * 1024 // Conservative for cmd.exe
        } else {
            128 * 1024 // Conservative for Unix
        };

        if total_len < limit {
            return Ok(cmd);
        }

        // Create response file
        Self::create_response_file(&cmd)
    }

    /// Create a response file and return a command that uses it.
    pub fn create_response_file(cmd: &Command) -> Result<Command, LinkerError> {
        // Create temp file
        let temp_dir = std::env::temp_dir();
        let rsp_path = temp_dir.join(format!("ori_link_{}.rsp", std::process::id()));

        let mut file =
            std::fs::File::create(&rsp_path).map_err(|e| LinkerError::ResponseFileError {
                path: rsp_path.display().to_string(),
                message: e.to_string(),
            })?;

        // Write arguments to response file
        for arg in cmd.get_args() {
            let arg_str = arg.to_string_lossy();

            // Quote arguments with spaces
            if arg_str.contains(' ') || arg_str.contains('"') {
                writeln!(file, "\"{}\"", arg_str.replace('"', "\\\""))
            } else {
                writeln!(file, "{arg_str}")
            }
            .map_err(|e| LinkerError::ResponseFileError {
                path: rsp_path.display().to_string(),
                message: e.to_string(),
            })?;
        }

        // Create new command with response file
        let mut new_cmd = Command::new(cmd.get_program());
        new_cmd.arg(format!("@{}", rsp_path.display()));

        Ok(new_cmd)
    }
}
