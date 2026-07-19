//! Command-line inspection and tool-availability helpers.

use std::process::Command;

pub fn command_args(cmd: &Command) -> Vec<String> {
    cmd.get_args()
        .map(|argument| argument.to_string_lossy().to_string())
        .collect()
}

pub fn command_has_arg(cmd: &Command, argument: &str) -> bool {
    command_args(cmd)
        .iter()
        .any(|candidate| candidate.contains(argument))
}

pub fn command_has_arg_before(cmd: &Command, argument: &str, before: &str) -> bool {
    let args = command_args(cmd);
    let argument_position = args
        .iter()
        .position(|candidate| candidate.contains(argument));
    let before_position = args.iter().position(|candidate| candidate.contains(before));
    matches!((argument_position, before_position), (Some(a), Some(b)) if a < b)
}

pub fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

pub fn wasm_ld_available() -> bool {
    tool_available("wasm-ld")
}

pub fn clang_available() -> bool {
    tool_available("clang")
}

pub fn llvm_objdump_available() -> bool {
    tool_available("llvm-objdump")
}

pub fn wasm_opt_available() -> bool {
    tool_available("wasm-opt")
}

#[macro_export]
macro_rules! assert_command_args {
    ($cmd:expr, $($arg:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::command_has_arg(&$cmd, $arg),
                "Expected argument '{}' not found in command. Args: {:?}",
                $arg,
                $crate::util::command_args(&$cmd)
            );
        )+
    };
}
