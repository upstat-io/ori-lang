//! Deterministic JSONL serialization.

use std::io::Write;

use crate::contract::{RecordEnvelope, RecordKind};
use crate::supervisor::SupervisorError;

pub(crate) fn write_jsonl<W: Write, T: serde::Serialize>(
    writer: &mut W,
    kind: RecordKind,
    payload: &T,
) -> Result<(), RunnerError> {
    serde_json::to_writer(
        &mut *writer,
        &RecordEnvelope {
            record: kind,
            payload,
        },
    )?;
    writer.write_all(b"\n")?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RunnerError {
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
    #[error("cannot encode breadth JSONL: {0}")]
    Json(#[from] serde_json::Error),
    #[error("cannot write breadth JSONL: {0}")]
    Io(#[from] std::io::Error),
}
