//! Ingest for the AIMS RC-survivor remark JSONL stream (layer-2 analyzer).
//!
//! Consumes the wire format emitted by `oric --emit-rc-remarks` (the JSONL
//! stream: a `record: "header"` envelope line followed by one remark object per
//! line). The wire format is the contract — these record types mirror the JSON
//! shape, not `ori_arc` internals, so the analyzer stays decoupled from the
//! compiler. Downstream opt-tooling analogs (opt-stats / opt-diff / opt-viewer)
//! build on [`ingest`].

use serde::Deserialize;

pub mod diff;
pub mod stats;
pub mod view;

/// Stream-header envelope (the first line of a valid stream).
#[derive(Debug, Clone, Deserialize)]
pub struct StreamHeader {
    /// RC-remark schema version the stream was produced under.
    pub schema_version: u32,
    /// Mangled SHA of the producing compiler build.
    pub compiler_sha: String,
    /// Source file the stream describes.
    pub source_file: String,
}

/// Resolved source location of a remark (absent for synthetic ops).
#[derive(Debug, Clone, Deserialize)]
pub struct DebugLoc {
    /// Source file path.
    pub file: String,
    /// 1-based line.
    pub line: u32,
    /// 1-based column.
    pub column: u32,
}

/// Why the RC op survived: the proof-failure + lattice-dimension attribution.
#[derive(Debug, Clone, Deserialize)]
pub struct Cause {
    /// The proof-failure clause (e.g. `burden_inc_kept_by_whole_var_disposition`).
    pub proof_failure: String,
    /// The blocking lattice dimension (e.g. `locality`), when attributable.
    #[serde(default)]
    pub lattice_dim: Option<String>,
    /// Human-readable detail (e.g. `consumption=Affine locality=FunctionLocal`).
    #[serde(default)]
    pub detail: Option<String>,
}

/// A single surviving (non-eliminated) RC op remark.
#[derive(Debug, Clone, Deserialize)]
pub struct Remark {
    /// `missed` (survived) / `passed` (eliminated) / `analysis`.
    pub kind: String,
    /// Emitting pass (e.g. `aims-burden-elim`).
    pub pass: String,
    /// Remark name (e.g. `rc-inc-not-elided`).
    pub name: String,
    /// The surviving burden op (e.g. `burden_inc`).
    pub rc_op: String,
    /// Enclosing function (the function-level fallback when `debug_loc` is absent).
    #[serde(default)]
    pub function: Option<String>,
    /// Resolved source location, when available.
    #[serde(default)]
    pub debug_loc: Option<DebugLoc>,
    /// SSA value id of the surviving var.
    #[serde(default)]
    pub ssa_value: Option<u64>,
    /// Exit block id, when applicable.
    #[serde(default)]
    pub exit_block: Option<u64>,
    /// Survival cause (proof-failure + lattice dim).
    #[serde(default)]
    pub cause: Option<Cause>,
    /// Net burden delta, when applicable.
    #[serde(default)]
    pub burden_net: Option<i64>,
    /// Attribution args (e.g. `def_kind=...`, `var_repr=...`, `span=...`).
    #[serde(default)]
    pub args: Vec<String>,
    /// COW mode, when applicable.
    #[serde(default)]
    pub cow_mode: Option<u64>,
}

/// An ingested stream: the optional header + the parsed remarks in stream order.
#[derive(Debug, Clone, Default)]
pub struct Stream {
    /// The header envelope, when the stream carried one.
    pub header: Option<StreamHeader>,
    /// The remarks, in stream order.
    pub remarks: Vec<Remark>,
}

/// Schema versions this analyzer can interpret.
///
/// A stream declaring any other version is rejected at ingest: its record
/// shapes are unknown, so analyzing it would report a verdict derived from the
/// wrong semantics.
pub const SUPPORTED_SCHEMA_VERSIONS: &[u32] = &[1, 2];

/// A JSONL ingest failure, with the 1-based stream line that failed.
#[derive(Debug)]
pub enum IngestError {
    /// A line was not valid JSON, or did not match the header / remark shape.
    Json {
        /// 1-based line number in the stream.
        line: usize,
        /// The underlying JSON parse error.
        source: serde_json::Error,
    },
    /// The header declared a schema version outside [`SUPPORTED_SCHEMA_VERSIONS`].
    UnsupportedSchemaVersion {
        /// 1-based line number of the header.
        line: usize,
        /// The version the stream declared.
        found: u32,
    },
    /// A remark was reached before any header, so its wire generation is unknown.
    MissingStreamHeader {
        /// 1-based line number of the first unversioned remark.
        line: usize,
    },
}

impl IngestError {
    /// The 1-based stream line the failure occurred on.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::Json { line, .. }
            | Self::UnsupportedSchemaVersion { line, .. }
            | Self::MissingStreamHeader { line } => *line,
        }
    }
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json { line, source } => {
                write!(f, "rc-remarks ingest: line {line}: {source}")
            }
            Self::UnsupportedSchemaVersion { line, found } => {
                let supported: Vec<String> = SUPPORTED_SCHEMA_VERSIONS
                    .iter()
                    .map(u32::to_string)
                    .collect();
                write!(
                    f,
                    "rc-remarks ingest: line {line}: stream declares schema version {found}, \
                     which this analyzer does not understand (it supports {}).\n  \
                     Rebuild ori-rc-remarks from the compiler revision that produced this \
                     stream, or regenerate the stream with the current compiler.",
                    supported.join(", ")
                )
            }
            Self::MissingStreamHeader { line } => write!(
                f,
                "rc-remarks ingest: line {line}: a remark appears before any \
                 `{{\"record\":\"header\"}}` line, so the stream declares no schema \
                 version and its records cannot be safely interpreted.\n  \
                 Regenerate the stream with `ori build --emit-rc-remarks <path>`, \
                 which always writes the header first."
            ),
        }
    }
}

impl std::error::Error for IngestError {}

/// Parse a JSONL remark stream into a [`Stream`].
///
/// Blank lines are skipped. A line whose `record` field is `"header"` becomes
/// the [`Stream::header`]; every other non-blank line is parsed as a [`Remark`].
///
/// # Errors
///
/// Returns the first [`IngestError`] (carrying its 1-based stream line) when a
/// non-blank line is not valid JSON or does not match the header / remark shape.
pub fn ingest(input: &str) -> Result<Stream, IngestError> {
    let mut stream = Stream::default();
    for (idx, raw) in input.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let line_no = idx + 1;
        let value: serde_json::Value =
            serde_json::from_str(line)
                .map_err(|source| IngestError::Json { line: line_no, source })?;
        let is_header = value.get("record").and_then(serde_json::Value::as_str) == Some("header");
        if is_header {
            let header: StreamHeader = serde_json::from_value(value)
                .map_err(|source| IngestError::Json { line: line_no, source })?;
            // Gate at the ingest boundary, before any remark is admitted, so no
            // consumer can compute a verdict over records whose shape this
            // build does not understand.
            if !SUPPORTED_SCHEMA_VERSIONS.contains(&header.schema_version) {
                return Err(IngestError::UnsupportedSchemaVersion {
                    line: line_no,
                    found: header.schema_version,
                });
            }
            stream.header = Some(header);
        } else {
            // A remark before any header is UNVERSIONED. Admitting it would let a
            // consumer compute a verdict over records whose generation this build
            // never checked -- the same fail-open the version gate above closes,
            // reached by omitting the header instead of declaring a bad one.
            if stream.header.is_none() {
                return Err(IngestError::MissingStreamHeader { line: line_no });
            }
            stream.remarks.push(
                serde_json::from_value(value)
                    .map_err(|source| IngestError::Json { line: line_no, source })?,
            );
        }
    }
    Ok(stream)
}

#[cfg(test)]
mod tests;
