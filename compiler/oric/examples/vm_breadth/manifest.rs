//! Frozen breadth-corpus manifest loading and semantic validation.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::contract::{FrozenFrontendStatus, FrozenOracleStatus, FrozenVmStatus, SCHEMA_VERSION};
use crate::digest::sha256;

const FROZEN_SELECTED: usize = 207;
const FROZEN_FRONTEND_VALID: usize = 194;
const FROZEN_ORACLE_ELIGIBLE: usize = 48;
const FROZEN_ORACLE_MATCH: usize = 47;
const FROZEN_ORACLE_MISMATCH: usize = 1;
const FROZEN_VM_FRONTEND_REJECTED: usize = 13;
const FROZEN_VM_UNSUPPORTED: usize = 122;
const FROZEN_VM_RUNTIME: usize = 21;
const FROZEN_VM_TIMEOUT: usize = 3;
const FROZEN_PATH_LIST_SHA256: &str =
    "5c001b9b79bfa41620c9c59bef988077172cd10e3d255cf40c433d04c8f267f6";
const FROZEN_SOURCE_INDEX_SHA256: &str =
    "32d847df4ceb2ac66d451570e4e633bb2fc611808f9d2c33537d8a2a0ae26d95";
const SWEEP_SHA256: &str = "02ab0830ceea4c58acb1f8f8242394f3cd9ba3c49f34463d4818685ebf7b03d1";
const ORACLE_SHA256: &str = "b9429e03a71a8dbcd88e39881080499d58492f7fa5cc49e6ded27497ab336993";
const CHECK_SHA256: &str = "b1ddb232424bcdcead75223081d99932e0862b7ff8c20f092fd01b6193234197";

/// Strictly typed, versioned corpus document.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub(crate) schema_version: u32,
    pub(crate) corpus_id: String,
    pub(crate) selected_count: usize,
    pub(crate) frontend_valid_count: usize,
    pub(crate) oracle_eligible_count: usize,
    pub(crate) path_list_sha256: String,
    pub(crate) source_index_sha256: String,
    pub(crate) provenance: Provenance,
    pub(crate) selection: Selection,
    pub(crate) cases: Vec<ManifestCase>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Provenance {
    #[serde(rename = "vm_sweep_sha256")]
    vm: String,
    #[serde(rename = "oracle_sweep_sha256")]
    oracle: String,
    #[serde(rename = "frontend_check_sweep_sha256")]
    frontend_check: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    roots: Vec<String>,
    extension: String,
    requires_main_attribute: bool,
    excluded_path_components: Vec<String>,
    ordering: ManifestOrdering,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ManifestOrdering {
    PathLexicographic,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestCase {
    pub(crate) path: String,
    pub(crate) source_sha256: String,
    pub(crate) frontend_status: FrozenFrontendStatus,
    pub(crate) vm_status: FrozenVmStatus,
    pub(crate) oracle_status: FrozenOracleStatus,
}

/// Validated manifest with canonical source-root and raw identity.
#[derive(Debug)]
pub(crate) struct LoadedManifest {
    pub(crate) document: Manifest,
    pub(crate) root: PathBuf,
    pub(crate) raw_sha256: String,
}

/// Machine-readable result of the standalone manifest validation command.
#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ManifestValidationRecord {
    pub(crate) schema_version: u32,
    pub(crate) corpus_id: String,
    pub(crate) manifest_sha256: String,
    pub(crate) selected_count: usize,
    pub(crate) frontend_valid_count: usize,
    pub(crate) oracle_eligible_count: usize,
    pub(crate) path_list_sha256: String,
    pub(crate) source_index_sha256: String,
}

impl From<&LoadedManifest> for ManifestValidationRecord {
    fn from(loaded: &LoadedManifest) -> Self {
        Self {
            schema_version: loaded.document.schema_version,
            corpus_id: loaded.document.corpus_id.clone(),
            manifest_sha256: loaded.raw_sha256.clone(),
            selected_count: loaded.document.selected_count,
            frontend_valid_count: loaded.document.frontend_valid_count,
            oracle_eligible_count: loaded.document.oracle_eligible_count,
            path_list_sha256: loaded.document.path_list_sha256.clone(),
            source_index_sha256: loaded.document.source_index_sha256.clone(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ManifestError {
    #[error("cannot {action} `{path}`: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("manifest `{path}` is not valid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("manifest contract violation: {detail}")]
    Contract { detail: String },
}

pub(crate) fn default_manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/vm_benchmark/vm_breadth_manifest.json")
}

pub(crate) fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub(crate) fn load(path: &Path, root: &Path) -> Result<LoadedManifest, ManifestError> {
    let raw = fs::read(path).map_err(|source| ManifestError::Io {
        action: "read manifest",
        path: path.to_path_buf(),
        source,
    })?;
    let document: Manifest =
        serde_json::from_slice(&raw).map_err(|source| ManifestError::Json {
            path: path.to_path_buf(),
            source,
        })?;
    let canonical_root = root.canonicalize().map_err(|source| ManifestError::Io {
        action: "canonicalize repository root",
        path: root.to_path_buf(),
        source,
    })?;
    validate(&document, &canonical_root)?;
    Ok(LoadedManifest {
        document,
        root: canonical_root,
        raw_sha256: sha256(&raw),
    })
}

fn validate(manifest: &Manifest, root: &Path) -> Result<(), ManifestError> {
    validate_header(manifest)?;
    validate_selection(&manifest.selection)?;
    validate_cases(manifest, root)?;
    validate_frozen_counts(manifest)?;
    Ok(())
}

fn validate_header(manifest: &Manifest) -> Result<(), ManifestError> {
    require(
        manifest.schema_version == SCHEMA_VERSION,
        format!(
            "schema_version is {}, expected {SCHEMA_VERSION}",
            manifest.schema_version
        ),
    )?;
    require(
        manifest.selected_count == FROZEN_SELECTED,
        format!(
            "selected_count is {}, expected {FROZEN_SELECTED}",
            manifest.selected_count
        ),
    )?;
    require(
        manifest.frontend_valid_count == FROZEN_FRONTEND_VALID,
        format!(
            "frontend_valid_count is {}, expected {FROZEN_FRONTEND_VALID}",
            manifest.frontend_valid_count
        ),
    )?;
    require(
        manifest.oracle_eligible_count == FROZEN_ORACLE_ELIGIBLE,
        format!(
            "oracle_eligible_count is {}, expected {FROZEN_ORACLE_ELIGIBLE}",
            manifest.oracle_eligible_count
        ),
    )?;
    require(
        manifest.path_list_sha256 == FROZEN_PATH_LIST_SHA256,
        "path-list digest does not match the frozen corpus".to_owned(),
    )?;
    require(
        manifest.source_index_sha256 == FROZEN_SOURCE_INDEX_SHA256,
        "source-index digest does not match the frozen corpus".to_owned(),
    )?;
    require(
        manifest.provenance.vm == SWEEP_SHA256
            && manifest.provenance.oracle == ORACLE_SHA256
            && manifest.provenance.frontend_check == CHECK_SHA256,
        "historical sweep provenance digest changed".to_owned(),
    )
}

fn validate_selection(selection: &Selection) -> Result<(), ManifestError> {
    require(
        selection.extension == "ori"
            && selection.requires_main_attribute
            && selection.ordering == ManifestOrdering::PathLexicographic,
        "selection contract changed".to_owned(),
    )?;
    require(
        selection.excluded_path_components == ["_test"],
        "selection exclusions must contain only `_test`".to_owned(),
    )?;
    require(
        selection.roots
            == [
                "tests/benchmarks",
                "tests/run-pass",
                "tests/spec",
                "tests/valgrind",
            ],
        "selection roots changed".to_owned(),
    )
}

fn validate_cases(manifest: &Manifest, root: &Path) -> Result<(), ManifestError> {
    require(
        manifest.cases.len() == manifest.selected_count,
        format!(
            "cases has {} entries, selected_count is {}",
            manifest.cases.len(),
            manifest.selected_count
        ),
    )?;
    let mut seen = HashSet::with_capacity(manifest.cases.len());
    let mut path_list = Vec::new();
    let mut source_index = Vec::new();
    let mut previous: Option<&str> = None;
    for case in &manifest.cases {
        validate_case_path(case, root, &manifest.selection, previous)?;
        require(
            seen.insert(case.path.as_str()),
            format!("duplicate case path `{}`", case.path),
        )?;
        let source_path = root.join(&case.path);
        let source = fs::read(&source_path).map_err(|source| ManifestError::Io {
            action: "read frozen source",
            path: source_path,
            source,
        })?;
        require(
            sha256(&source) == case.source_sha256,
            format!("source digest changed for `{}`", case.path),
        )?;
        path_list.extend_from_slice(case.path.as_bytes());
        path_list.push(b'\n');
        source_index.extend_from_slice(case.path.as_bytes());
        source_index.push(0);
        source_index.extend_from_slice(case.source_sha256.as_bytes());
        source_index.push(b'\n');
        previous = Some(&case.path);
    }
    require(
        sha256(&path_list) == manifest.path_list_sha256,
        "computed path-list digest differs from manifest".to_owned(),
    )?;
    require(
        sha256(&source_index) == manifest.source_index_sha256,
        "computed source-index digest differs from manifest".to_owned(),
    )
}

fn validate_case_path(
    case: &ManifestCase,
    root: &Path,
    selection: &Selection,
    previous: Option<&str>,
) -> Result<(), ManifestError> {
    let relative = Path::new(&case.path);
    require(
        !relative.is_absolute()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        format!("case path `{}` is not a contained relative path", case.path),
    )?;
    require(
        selection
            .roots
            .iter()
            .any(|prefix| relative.starts_with(prefix)),
        format!("case path `{}` is outside selection roots", case.path),
    )?;
    require(
        relative
            .extension()
            .is_some_and(|extension| extension == "ori"),
        format!("case path `{}` is not an Ori source", case.path),
    )?;
    require(
        !relative.components().any(|component| {
            selection
                .excluded_path_components
                .iter()
                .any(|excluded| component.as_os_str() == OsStr::new(excluded))
        }),
        format!("case path `{}` contains an excluded component", case.path),
    )?;
    if let Some(prior) = previous {
        require(
            prior < case.path.as_str(),
            format!("case path `{}` is not in strict lexical order", case.path),
        )?;
    }
    let canonical = root
        .join(relative)
        .canonicalize()
        .map_err(|source| ManifestError::Io {
            action: "canonicalize frozen source",
            path: root.join(relative),
            source,
        })?;
    require(
        canonical.starts_with(root),
        format!("case path `{}` escapes the repository root", case.path),
    )
}

fn validate_frozen_counts(manifest: &Manifest) -> Result<(), ManifestError> {
    let count = |predicate: fn(&ManifestCase) -> bool| {
        manifest.cases.iter().filter(|case| predicate(case)).count()
    };
    require(
        count(|case| case.frontend_status == FrozenFrontendStatus::Valid) == FROZEN_FRONTEND_VALID,
        "frozen frontend-valid denominator changed".to_owned(),
    )?;
    require(
        count(|case| case.vm_status == FrozenVmStatus::FullVm) == FROZEN_ORACLE_ELIGIBLE,
        "frozen full-VM denominator changed".to_owned(),
    )?;
    require(
        count(|case| case.vm_status == FrozenVmStatus::NotRunFrontendRejected)
            == FROZEN_VM_FRONTEND_REJECTED
            && count(|case| case.vm_status == FrozenVmStatus::Unsupported) == FROZEN_VM_UNSUPPORTED
            && count(|case| case.vm_status == FrozenVmStatus::Runtime) == FROZEN_VM_RUNTIME
            && count(|case| case.vm_status == FrozenVmStatus::Timeout) == FROZEN_VM_TIMEOUT
            && count(|case| case.vm_status == FrozenVmStatus::Verifier) == 0
            && count(|case| case.vm_status == FrozenVmStatus::Fallback) == 0,
        "frozen VM classification denominators changed".to_owned(),
    )?;
    require(
        count(|case| case.oracle_status == FrozenOracleStatus::Match) == FROZEN_ORACLE_MATCH
            && count(|case| case.oracle_status == FrozenOracleStatus::Mismatch)
                == FROZEN_ORACLE_MISMATCH,
        "frozen oracle comparison denominator changed".to_owned(),
    )?;
    require(
        manifest.cases.iter().all(|case| {
            (case.frontend_status == FrozenFrontendStatus::Rejected)
                == (case.vm_status == FrozenVmStatus::NotRunFrontendRejected)
        }),
        "frontend rejection and executable-VM admission disagree".to_owned(),
    )?;
    require(
        manifest.cases.iter().all(|case| {
            (case.vm_status == FrozenVmStatus::FullVm)
                == (case.oracle_status != FrozenOracleStatus::NotRun)
        }),
        "oracle eligibility and full-VM classification disagree".to_owned(),
    )
}

fn require(condition: bool, detail: String) -> Result<(), ManifestError> {
    if condition {
        Ok(())
    } else {
        Err(ManifestError::Contract { detail })
    }
}
