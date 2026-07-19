//! Deterministic breadth runner for evaluator and bytecode-VM parity.

#[path = "vm_breadth/app.rs"]
mod app;
#[path = "vm_breadth/contract.rs"]
mod contract;
#[path = "vm_breadth/digest.rs"]
mod digest;
#[path = "vm_breadth/errors.rs"]
mod errors;
#[path = "vm_breadth/manifest.rs"]
mod manifest;
#[path = "vm_breadth/metrics.rs"]
mod metrics;
#[path = "vm_breadth/runner.rs"]
mod runner;
#[path = "vm_breadth/supervisor.rs"]
mod supervisor;
#[path = "vm_breadth/values.rs"]
mod values;
#[path = "vm_breadth/worker.rs"]
mod worker;

#[cfg(test)]
#[path = "vm_breadth/tests.rs"]
mod tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    match app::run(std::env::args_os()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("vm breadth harness failed: {error}");
            ExitCode::from(2)
        }
    }
}
