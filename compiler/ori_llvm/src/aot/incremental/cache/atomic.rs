//! Publication primitives for immutable generations and atomic entry metadata.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::CacheError;

static NEXT_TEMP_FILE: AtomicU64 = AtomicU64::new(0);
static NEXT_GENERATION_FILE: AtomicU64 = AtomicU64::new(0);

pub(super) fn publish_bytes_atomically(destination: &Path, bytes: &[u8]) -> Result<(), CacheError> {
    let (temp_path, mut temp_file) = create_temp_file(destination)?;
    if let Err(error) = temp_file
        .write_all(bytes)
        .and_then(|()| temp_file.sync_all())
    {
        let _ = fs::remove_file(&temp_path);
        return Err(CacheError::IoError {
            path: destination.to_path_buf(),
            message: error.to_string(),
        });
    }
    publish_temp_file(&temp_path, destination)
}

pub(super) fn publish_generation_bytes(
    directory: &Path,
    object_stem: &str,
    bytes: &[u8],
) -> Result<String, CacheError> {
    // The generation is not discoverable until its entry metadata is
    // published, so writing the exclusively-created final path is safe.
    let (file_name, path, mut file) = create_generation_file(directory, object_stem)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&path);
        return Err(CacheError::IoError {
            path,
            message: error.to_string(),
        });
    }
    Ok(file_name)
}

pub(super) fn publish_generation_file(
    directory: &Path,
    object_stem: &str,
    source: &Path,
) -> Result<String, CacheError> {
    let mut source_file = File::open(source).map_err(|error| CacheError::IoError {
        path: source.to_path_buf(),
        message: error.to_string(),
    })?;
    let (file_name, path, mut file) = create_generation_file(directory, object_stem)?;
    if let Err(error) = std::io::copy(&mut source_file, &mut file).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&path);
        return Err(CacheError::IoError {
            path,
            message: error.to_string(),
        });
    }
    Ok(file_name)
}

fn create_generation_file(
    directory: &Path,
    object_stem: &str,
) -> Result<(String, PathBuf, File), CacheError> {
    loop {
        let generation = NEXT_GENERATION_FILE.fetch_add(1, Ordering::Relaxed);
        let file_name = format!("{object_stem}-{}-{generation}.o", std::process::id());
        let path = directory.join(&file_name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((file_name, path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(CacheError::IoError {
                    path,
                    message: error.to_string(),
                });
            }
        }
    }
}

fn create_temp_file(destination: &Path) -> Result<(PathBuf, File), CacheError> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact");
    loop {
        let nonce = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(".{file_name}.tmp-{}-{nonce}", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(CacheError::IoError {
                    path,
                    message: error.to_string(),
                });
            }
        }
    }
}

fn publish_temp_file(temp: &Path, destination: &Path) -> Result<(), CacheError> {
    match fs::rename(temp, destination) {
        Ok(()) => Ok(()),
        // Windows does not replace an existing destination. A writer that won
        // the race has already published a complete destination.
        Err(_) if destination.exists() => {
            let _ = fs::remove_file(temp);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(temp);
            Err(CacheError::IoError {
                path: destination.to_path_buf(),
                message: error.to_string(),
            })
        }
    }
}
