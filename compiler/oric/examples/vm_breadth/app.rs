//! Command-line dispatch for the breadth harness.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::PathBuf;
use std::time::Duration;

use crate::contract::{OraclePolicy, RecordKind};
use crate::manifest::{self, ManifestValidationRecord};

const DEFAULT_TIMEOUT_MS: u64 = 30_000;

pub(crate) fn run<I>(args: I) -> Result<(), AppError>
where
    I: IntoIterator,
    I::Item: Into<OsString>,
{
    match parse(args)? {
        Command::Validate { manifest_path } => {
            let manifest = manifest::load(&manifest_path, &manifest::repository_root())?;
            let record = ManifestValidationRecord::from(&manifest);
            let mut stdout = io::stdout().lock();
            crate::runner::write_jsonl(&mut stdout, RecordKind::ManifestValidation, &record)?;
            Ok(())
        }
        Command::Run {
            manifest_path,
            timeout,
            oracle_policy,
        } => {
            let manifest = manifest::load(&manifest_path, &manifest::repository_root())?;
            let mut stdout = io::stdout().lock();
            crate::runner::execute(&manifest, timeout, oracle_policy, &mut stdout)?;
            Ok(())
        }
        Command::WorkerEvaluator {
            source_path,
            record_path,
        } => crate::worker::evaluator(&source_path, &record_path).map_err(AppError::Worker),
        Command::WorkerGeneric {
            source_path,
            record_path,
        } => crate::worker::generic(&source_path, &record_path).map_err(AppError::Worker),
        Command::WorkerPhysical {
            source_path,
            record_path,
        } => crate::worker::physical(&source_path, &record_path).map_err(AppError::Worker),
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Validate {
        manifest_path: PathBuf,
    },
    Run {
        manifest_path: PathBuf,
        timeout: Duration,
        oracle_policy: OraclePolicy,
    },
    WorkerEvaluator {
        source_path: PathBuf,
        record_path: PathBuf,
    },
    WorkerGeneric {
        source_path: PathBuf,
        record_path: PathBuf,
    },
    WorkerPhysical {
        source_path: PathBuf,
        record_path: PathBuf,
    },
}

pub(crate) fn parse<I>(args: I) -> Result<Command, AppError>
where
    I: IntoIterator,
    I::Item: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into);
    let _binary = args.next();
    let operation = args.next().ok_or(AppError::Usage {
        detail: "missing command; use `validate` or `run`".to_owned(),
    })?;
    let remaining: Vec<_> = args.collect();
    if operation == "validate" {
        let options = parse_options(&remaining, &["--manifest"])?;
        Ok(Command::Validate {
            manifest_path: options
                .value("--manifest")
                .map_or_else(manifest::default_manifest_path, PathBuf::from),
        })
    } else if operation == "run" {
        let options = parse_options(
            &remaining,
            &["--manifest", "--timeout-ms", "--oracle-policy"],
        )?;
        let timeout_ms = options.value("--timeout-ms").map_or_else(
            || Ok(DEFAULT_TIMEOUT_MS),
            |value| {
                value
                    .to_str()
                    .ok_or_else(|| AppError::Usage {
                        detail: "`--timeout-ms` must be valid UTF-8 decimal digits".to_owned(),
                    })?
                    .parse::<u64>()
                    .map_err(|_| AppError::Usage {
                        detail: "`--timeout-ms` must be a positive decimal integer".to_owned(),
                    })
            },
        )?;
        if timeout_ms == 0 {
            return Err(AppError::Usage {
                detail: "`--timeout-ms` must be greater than zero".to_owned(),
            });
        }
        let oracle_policy = match options.value("--oracle-policy") {
            None => OraclePolicy::Historical48,
            Some(value) if value == "historical-48" => OraclePolicy::Historical48,
            Some(value) if value == "all-frontend-valid" => OraclePolicy::AllFrontendValid,
            Some(_) => {
                return Err(AppError::Usage {
                    detail: "`--oracle-policy` must be `historical-48` or `all-frontend-valid`"
                        .to_owned(),
                });
            }
        };
        Ok(Command::Run {
            manifest_path: options
                .value("--manifest")
                .map_or_else(manifest::default_manifest_path, PathBuf::from),
            timeout: Duration::from_millis(timeout_ms),
            oracle_policy,
        })
    } else if matches!(
        operation.to_str(),
        Some("__worker-evaluator" | "__worker-generic" | "__worker-physical")
    ) {
        let options = parse_options(&remaining, &["--source", "--record"])?;
        let source_path = required_path(&options, "--source")?;
        let record_path = required_path(&options, "--record")?;
        if operation == "__worker-evaluator" {
            Ok(Command::WorkerEvaluator {
                source_path,
                record_path,
            })
        } else if operation == "__worker-generic" {
            Ok(Command::WorkerGeneric {
                source_path,
                record_path,
            })
        } else {
            Ok(Command::WorkerPhysical {
                source_path,
                record_path,
            })
        }
    } else {
        Err(AppError::Usage {
            detail: format!(
                "unknown command `{}`; use `validate` or `run`",
                operation.to_string_lossy()
            ),
        })
    }
}

struct Options<'args> {
    values: Vec<(&'args str, &'args OsStr)>,
}

impl Options<'_> {
    fn value(&self, flag: &str) -> Option<&OsStr> {
        self.values
            .iter()
            .find_map(|(candidate, value)| (*candidate == flag).then_some(*value))
    }
}

fn parse_options<'args>(
    args: &'args [OsString],
    allowed: &[&'args str],
) -> Result<Options<'args>, AppError> {
    let mut values = Vec::new();
    let mut index = 0;
    while index < args.len() {
        let flag = allowed
            .iter()
            .copied()
            .find(|candidate| args[index] == *candidate)
            .ok_or_else(|| AppError::Usage {
                detail: format!("unknown option `{}`", args[index].to_string_lossy()),
            })?;
        let value = args.get(index + 1).ok_or_else(|| AppError::Usage {
            detail: format!("option `{flag}` requires a value"),
        })?;
        if values
            .iter()
            .any(|(existing, _): &(&str, &OsStr)| *existing == flag)
        {
            return Err(AppError::Usage {
                detail: format!("option `{flag}` may be provided only once"),
            });
        }
        values.push((flag, value.as_os_str()));
        index += 2;
    }
    Ok(Options { values })
}

fn required_path(options: &Options<'_>, flag: &'static str) -> Result<PathBuf, AppError> {
    options
        .value(flag)
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Usage {
            detail: format!("worker command requires `{flag}`"),
        })
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppError {
    #[error("invalid command line: {detail}")]
    Usage { detail: String },
    #[error(transparent)]
    Manifest(#[from] manifest::ManifestError),
    #[error(transparent)]
    Runner(#[from] crate::runner::RunnerError),
    #[error(transparent)]
    Worker(#[from] crate::worker::WorkerIoError),
}
