//! Isolated worker-process supervision.

#[path = "supervisor/process.rs"]
mod process;

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::contract::{
    Availability, ProcessObservation, Termination, UnavailableReason, SCHEMA_VERSION,
};
use crate::digest::{sha256, ByteArtifact};
use crate::errors::{ErrorKind, ErrorRecord, HarnessErrorKind};

use process::{peak_rss, post_spawn_failure, sample_peak_rss, supervise_child, wait_failure_error};

const STDOUT_RETAIN_LIMIT: usize = 64 * 1024 * 1024;
const STDERR_RETAIN_LIMIT: usize = 4 * 1024 * 1024;

#[cfg(test)]
const TEST_WORKER_COMMAND: &str = "ORI_VM_BREADTH_TEST_WORKER_COMMAND";
#[cfg(test)]
const TEST_WORKER_SOURCE: &str = "ORI_VM_BREADTH_TEST_WORKER_SOURCE";
#[cfg(test)]
const TEST_WORKER_RECORD: &str = "ORI_VM_BREADTH_TEST_WORKER_RECORD";

#[derive(Clone, Copy, Debug)]
pub(crate) enum WorkerKind {
    Evaluator,
    Generic,
    Physical,
}

impl WorkerKind {
    const fn command(self) -> &'static str {
        match self {
            Self::Evaluator => "__worker-evaluator",
            Self::Generic => "__worker-generic",
            Self::Physical => "__worker-physical",
        }
    }
}

pub(crate) struct Supervisor {
    _snapshot_directory: tempfile::TempDir,
    executable: PathBuf,
    executable_sha256: String,
    timeout: Duration,
}

impl Supervisor {
    pub(crate) fn new(timeout: Duration) -> Result<Self, SupervisorError> {
        let current = std::env::current_exe().map_err(SupervisorError::CurrentExecutable)?;
        let bytes = fs::read(&current).map_err(|source| SupervisorError::ReadExecutable {
            path: current.clone(),
            source,
        })?;
        let directory = tempfile::tempdir().map_err(SupervisorError::SnapshotDirectory)?;
        let executable = directory.path().join("vm-breadth-worker");
        fs::copy(&current, &executable).map_err(|source| SupervisorError::SnapshotExecutable {
            source_path: current,
            target_path: executable.clone(),
            source,
        })?;
        Ok(Self {
            _snapshot_directory: directory,
            executable,
            executable_sha256: sha256(&bytes),
            timeout,
        })
    }

    pub(crate) fn executable_sha256(&self) -> &str {
        &self.executable_sha256
    }

    pub(crate) fn run<T: DeserializeOwned>(
        &self,
        kind: WorkerKind,
        source_path: &Path,
    ) -> Supervised<T> {
        match self.run_inner(kind, source_path) {
            Ok(outcome) => outcome,
            Err(error) => Supervised::spawn_failure(&error),
        }
    }

    fn run_inner<T: DeserializeOwned>(
        &self,
        kind: WorkerKind,
        source_path: &Path,
    ) -> Result<Supervised<T>, io::Error> {
        let record = tempfile::NamedTempFile::new()?;
        let mut command = Command::new(&self.executable);
        configure_worker_command(&mut command, kind, source_path, record.path());
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command.spawn()?;
        let started = Instant::now();
        let child_id = child.id();
        let Some(stdout) = child.stdout.take() else {
            return Ok(post_spawn_failure(
                &mut child,
                started,
                "worker stdout pipe was not created",
            ));
        };
        let Some(stderr) = child.stderr.take() else {
            return Ok(post_spawn_failure(
                &mut child,
                started,
                "worker stderr pipe was not created",
            ));
        };
        let stdout_reader = thread::spawn(move || capture(stdout, STDOUT_RETAIN_LIMIT));
        let stderr_reader = thread::spawn(move || capture(stderr, STDERR_RETAIN_LIMIT));

        let mut peak_rss_kib = None;
        let termination = supervise_child(
            &mut child,
            child_id,
            started,
            self.timeout,
            &mut peak_rss_kib,
        );
        let elapsed_ns_decimal = started.elapsed().as_nanos().to_string();
        sample_peak_rss(child_id, &mut peak_rss_kib);
        let stdout = join_capture(stdout_reader, "stdout");
        let stderr = join_capture(stderr_reader, "stderr");
        let observation = ProcessObservation {
            termination: termination.clone(),
            elapsed_ns_decimal: Availability::available(elapsed_ns_decimal),
            sampled_peak_rss_kib_lower_bound_decimal: peak_rss(peak_rss_kib),
            stdout: stdout.artifact,
            stderr: stderr.artifact,
        };
        let process_error = wait_failure_error(&termination);
        let capture_error = stdout.read_error.or(stderr.read_error);
        let (worker_record, decode_error) = if let Some(message) = capture_error {
            (
                None,
                Some(harness_error(
                    HarnessErrorKind::ProcessOutputTruncated,
                    &message,
                )),
            )
        } else {
            decode_worker_record(record.path())
        };
        Ok(Supervised {
            process: observation,
            record: worker_record,
            protocol_error: process_error.or(decode_error),
        })
    }
}

#[cfg(not(test))]
fn configure_worker_command(
    command: &mut Command,
    kind: WorkerKind,
    source_path: &Path,
    record_path: &Path,
) {
    command
        .arg(kind.command())
        .arg("--source")
        .arg(source_path)
        .arg("--record")
        .arg(record_path);
}

#[cfg(test)]
fn configure_worker_command(
    command: &mut Command,
    kind: WorkerKind,
    source_path: &Path,
    record_path: &Path,
) {
    command
        .args([
            "--exact",
            "tests::protocol::spawned_worker_entry",
            "--nocapture",
        ])
        .env(TEST_WORKER_COMMAND, kind.command())
        .env(TEST_WORKER_SOURCE, source_path)
        .env(TEST_WORKER_RECORD, record_path);
}

#[cfg(test)]
pub(crate) fn test_worker_args() -> Option<Vec<std::ffi::OsString>> {
    let command = std::env::var_os(TEST_WORKER_COMMAND)?;
    let source = std::env::var_os(TEST_WORKER_SOURCE)?;
    let record = std::env::var_os(TEST_WORKER_RECORD)?;
    Some(vec![
        std::ffi::OsString::from("vm_breadth"),
        command,
        std::ffi::OsString::from("--source"),
        source,
        std::ffi::OsString::from("--record"),
        record,
    ])
}

pub(crate) struct Supervised<T> {
    pub(crate) process: ProcessObservation,
    pub(crate) record: Option<T>,
    pub(crate) protocol_error: Option<ErrorRecord>,
}

impl<T> Supervised<T> {
    fn spawn_failure(error: &io::Error) -> Self {
        let message = error.to_string();
        Self {
            process: ProcessObservation {
                termination: Termination::SpawnFailed {
                    message: message.clone(),
                },
                elapsed_ns_decimal: Availability::unavailable(UnavailableReason::ProcessNotSpawned),
                sampled_peak_rss_kib_lower_bound_decimal: Availability::unavailable(
                    UnavailableReason::ProcessNotSpawned,
                ),
                stdout: ByteArtifact::complete(&[]),
                stderr: ByteArtifact::complete(&[]),
            },
            record: None,
            protocol_error: Some(harness_error(HarnessErrorKind::ProcessSpawn, &message)),
        }
    }
}

struct StreamCapture {
    artifact: ByteArtifact,
    read_error: Option<String>,
}

fn capture(mut reader: impl Read, retain_limit: usize) -> StreamCapture {
    let mut retained = Vec::new();
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 16 * 1024];
    let read_error = loop {
        match reader.read(&mut buffer) {
            Ok(0) => break None,
            Ok(count) => {
                digest.update(&buffer[..count]);
                total = total.saturating_add(count as u64);
                let available = retain_limit.saturating_sub(retained.len());
                retained.extend_from_slice(&buffer[..count.min(available)]);
            }
            Err(error) => break Some(error.to_string()),
        }
    };
    let digest: [u8; 32] = digest.finalize().into();
    StreamCapture {
        artifact: ByteArtifact::captured(total, digest, &retained),
        read_error,
    }
}

fn join_capture(
    handle: thread::JoinHandle<StreamCapture>,
    stream_name: &'static str,
) -> StreamCapture {
    handle.join().unwrap_or_else(|_| StreamCapture {
        artifact: ByteArtifact::complete(&[]),
        read_error: Some(format!("{stream_name} reader thread panicked")),
    })
}

fn decode_worker_record<T: DeserializeOwned>(path: &Path) -> (Option<T>, Option<ErrorRecord>) {
    let raw = match fs::read(path) {
        Ok(raw) => raw,
        Err(error) => {
            return (
                None,
                Some(harness_error(
                    HarnessErrorKind::WorkerRecordMissing,
                    &error.to_string(),
                )),
            );
        }
    };
    if raw.is_empty() {
        return (
            None,
            Some(harness_error(
                HarnessErrorKind::WorkerRecordMissing,
                "worker record file is empty",
            )),
        );
    }
    match serde_json::from_slice(&raw) {
        Ok(record) => (Some(record), None),
        Err(error) => (
            None,
            Some(ErrorRecord::new(
                ErrorKind::Harness {
                    kind: HarnessErrorKind::WorkerRecordInvalid,
                },
                error.to_string(),
                &format!("schema_version={SCHEMA_VERSION}; {error:?}"),
            )),
        ),
    }
}

fn harness_error(kind: HarnessErrorKind, message: &str) -> ErrorRecord {
    ErrorRecord::new(ErrorKind::Harness { kind }, message.to_owned(), message)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SupervisorError {
    #[error("cannot locate the current breadth-harness executable: {0}")]
    CurrentExecutable(io::Error),
    #[error("cannot read breadth-harness executable `{path}`: {source}")]
    ReadExecutable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("cannot create breadth-harness snapshot directory: {0}")]
    SnapshotDirectory(io::Error),
    #[error("cannot snapshot `{source_path}` to `{target_path}`: {source}")]
    SnapshotExecutable {
        source_path: PathBuf,
        target_path: PathBuf,
        #[source]
        source: io::Error,
    },
}
