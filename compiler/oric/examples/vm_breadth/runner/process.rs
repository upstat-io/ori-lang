//! Process-termination classification for supervised workers.

use crate::contract::Termination;
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};

pub(super) fn termination_succeeded(termination: &Termination) -> bool {
    matches!(
        termination,
        Termination::Exited { code_decimal } if code_decimal == "0"
    )
}

pub(super) fn failed_termination_error(termination: &Termination) -> Option<ErrorRecord> {
    let (kind, message) = match termination {
        Termination::Exited { code_decimal } if code_decimal != "0" => (
            HarnessErrorKind::ProcessCrash,
            format!(
                "worker process exited with status {code_decimal}; inspect the captured stderr artifact for the compiler failure"
            ),
        ),
        Termination::Signaled { signal_decimal } => (
            HarnessErrorKind::ProcessCrash,
            format!(
                "worker process terminated by signal {signal_decimal}; inspect the captured stderr artifact for the compiler failure"
            ),
        ),
        Termination::SpawnFailed { message } => (
            HarnessErrorKind::ProcessSpawn,
            format!(
                "worker process could not be spawned: {message}; verify the executable snapshot and process permissions"
            ),
        ),
        Termination::WaitFailed { message } => (
            HarnessErrorKind::ProcessWait,
            format!(
                "worker process could not be observed to completion: {message}; inspect the process supervisor"
            ),
        ),
        Termination::Exited { .. } | Termination::TimedOut { .. } => return None,
    };
    Some(ErrorRecord::new(
        ErrorKind::Harness { kind },
        message,
        &format!("{termination:?}"),
    ))
}
