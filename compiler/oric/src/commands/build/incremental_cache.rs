//! Correctness-bounded object caching for single-source `ori build`.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use ori_llvm::aot::incremental::cache::CacheError;
use ori_llvm::aot::incremental::{ArtifactCache, CacheConfig, CacheKey, ContentHash};
use ori_llvm::aot::TargetConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{BuildOptions, DebugLevel, LtoMode, OptLevel};

const CACHE_FORMAT_VERSION: &str = "ori-build-object-v2";
const CACHE_MANIFEST_SCHEMA: u32 = 1;
type Sha256Digest = [u8; 32];

pub(super) enum CacheProbe {
    Hit(PathBuf),
    Miss(CacheMiss),
    Bypass(CacheBypass),
}

pub(super) struct CacheMiss {
    cache: ArtifactCache,
    key: CacheKey,
    request_sha256: Sha256Digest,
}

impl CacheMiss {
    pub(super) fn publish(&self, object: &Path) -> Result<(), CacheError> {
        let (object_sha256, object_size) =
            hash_file_sha256(object).map_err(|error| CacheError::IoError {
                path: object.to_path_buf(),
                message: error.to_string(),
            })?;
        let manifest = ObjectManifest {
            schema: CACHE_MANIFEST_SCHEMA,
            request_sha256: digest_hex(&self.request_sha256),
            object_sha256: digest_hex(&object_sha256),
            object_size,
        };
        let encoded = serde_json::to_vec(&manifest).map_err(|error| CacheError::Invalid {
            message: format!("could not encode object manifest: {error}"),
        })?;
        self.cache
            .publish_file(&self.key, &manifest.object_sha256, object, &encoded)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObjectManifest {
    schema: u32,
    request_sha256: String,
    object_sha256: String,
    object_size: u64,
}

pub(super) struct CacheBypass {
    reason: String,
    warn: bool,
}

impl CacheBypass {
    pub(super) fn report(&self, verbose: bool) {
        if self.warn {
            eprintln!(
                "warning: incremental cache: bypass ({}); rebuild proceeds without reuse",
                self.reason
            );
        } else if verbose {
            eprintln!("  incremental cache: bypass ({})", self.reason);
        }
    }
}

/// Probe the object cache only when every frontend and codegen input is owned.
pub(super) fn probe(
    db: &oric::CompilerDb,
    parse_result: &crate::parser::ParseOutput,
    source_path: &Path,
    source: &str,
    options: &BuildOptions,
    target: &TargetConfig,
) -> CacheProbe {
    let eligibility = ineligibility_reason(
        has_unfingerprinted_imports(parse_result),
        options,
        std::env::vars_os().map(|(name, _)| name),
    );
    if let Some(reason) = eligibility {
        return CacheProbe::Bypass(CacheBypass {
            reason,
            warn: false,
        });
    }
    if target.is_wasm() {
        return quiet_bypass("WebAssembly builds use a separate emission pipeline");
    }

    let resolved = match resolve_fingerprint_inputs(db, parse_result, source_path) {
        Ok(resolved) => resolved,
        Err(bypass) => return CacheProbe::Bypass(bypass),
    };
    let (cache, config) = match open_cache(options, target) {
        Ok(cache) => cache,
        Err(bypass) => return CacheProbe::Bypass(bypass),
    };
    let inputs = FingerprintInputs {
        source,
        source_spelling: source_path,
        canonical_source: &resolved.canonical_source,
        compiler_sha256: resolved.compiler_sha256,
        dependency_sha256: resolved.dependency_sha256,
        llvm_sha256: resolved.llvm_sha256,
        target: target.triple(),
        cpu: target.cpu(),
        features: target.features(),
        opt_level: options.opt_level,
        debug_level: options.debug_level,
        narrowing_policy: options.narrowing_policy,
        sanitizer: options.sanitizer_env.as_deref(),
        lto: options.lto,
        release: options.release,
        compiler_version: env!("CARGO_PKG_VERSION"),
        compiler_git_sha: env!("ORI_GIT_SHA"),
        compiler_git_dirty: env!("ORI_GIT_DIRTY"),
    };
    let request_sha256 = request_digest(&inputs);
    let key = cache_key(&request_sha256, &config);
    let hit = cache.get_verified(&key, |object, manifest| {
        verify_cache_entry(object, manifest, &request_sha256)
    });
    match hit {
        Some(object) => CacheProbe::Hit(object),
        None => match cache.remove(&key) {
            Ok(()) => CacheProbe::Miss(CacheMiss {
                cache,
                key,
                request_sha256,
            }),
            Err(error) => CacheProbe::Bypass(CacheBypass {
                reason: format!(
                    "could not invalidate an unverified entry: {error}; fix the cache directory permissions"
                ),
                warn: true,
            }),
        },
    }
}

struct ResolvedFingerprintInputs {
    canonical_source: PathBuf,
    compiler_sha256: Sha256Digest,
    dependency_sha256: Sha256Digest,
    llvm_sha256: Sha256Digest,
}

fn resolve_fingerprint_inputs(
    db: &oric::CompilerDb,
    parse_result: &crate::parser::ParseOutput,
    source_path: &Path,
) -> Result<ResolvedFingerprintInputs, CacheBypass> {
    let canonical_source = source_path.canonicalize().map_err(|error| CacheBypass {
        reason: format!(
            "could not resolve source identity for '{}': {error}; check that the path is readable",
            source_path.display()
        ),
        warn: true,
    })?;
    let compiler_path = std::env::current_exe().map_err(|error| CacheBypass {
        reason: format!(
            "could not locate the running compiler: {error}; rerun with a readable compiler executable"
        ),
        warn: true,
    })?;
    let compiler_sha256 = hash_file_sha256(&compiler_path)
        .map(|(digest, _)| digest)
        .map_err(|error| CacheBypass {
            reason: format!(
                "could not hash compiler executable '{}': {error}; check that the executable remains readable",
                compiler_path.display()
            ),
            warn: true,
        })?;
    let dependency_sha256 = prelude_digest(db, parse_result, source_path)?;
    let llvm_sha256 = llvm_runtime_digest().map_err(|reason| CacheBypass {
        reason,
        warn: false,
    })?;

    Ok(ResolvedFingerprintInputs {
        canonical_source,
        compiler_sha256,
        dependency_sha256,
        llvm_sha256,
    })
}

fn open_cache(
    options: &BuildOptions,
    target: &TargetConfig,
) -> Result<(ArtifactCache, CacheConfig), CacheBypass> {
    let cache_dir = cache_directory().map_err(|reason| CacheBypass { reason, warn: true })?;
    let config = CacheConfig::new(cache_dir)
        .with_version(CACHE_FORMAT_VERSION)
        .with_opt_level(opt_level_name(options.opt_level))
        .with_target(target.triple());
    let cache = ArtifactCache::new(config.clone()).map_err(|error| CacheBypass {
        reason: format!(
            "{error}; set XDG_CACHE_HOME to a writable directory or fix its permissions"
        ),
        warn: true,
    })?;
    Ok((cache, config))
}

fn quiet_bypass(reason: impl Into<String>) -> CacheProbe {
    CacheProbe::Bypass(CacheBypass {
        reason: reason.into(),
        warn: false,
    })
}

fn prelude_digest(
    db: &oric::CompilerDb,
    parse_result: &crate::parser::ParseOutput,
    source_path: &Path,
) -> Result<Sha256Digest, CacheBypass> {
    let resolved_imports = crate::imports::resolve_imports(db, parse_result, source_path);
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, b"domain", b"ori-build-prelude-v1");
    let Some(prelude) = resolved_imports.prelude.as_ref() else {
        update_digest_field(&mut hasher, b"prelude", b"none");
        return Ok(hasher.finalize().into());
    };
    let Some(source_file) = prelude.source_file else {
        return Err(CacheBypass {
            reason: "the implicit prelude has no source identity to fingerprint".to_string(),
            warn: false,
        });
    };
    update_digest_field(
        &mut hasher,
        b"prelude-path",
        &path_bytes(source_file.path(db)),
    );
    update_digest_field(
        &mut hasher,
        b"prelude-source",
        source_file.text(db).as_bytes(),
    );
    Ok(hasher.finalize().into())
}

#[cfg(target_os = "linux")]
fn llvm_runtime_digest() -> Result<Sha256Digest, String> {
    use std::collections::BTreeSet;

    let maps = std::fs::read_to_string("/proc/self/maps").map_err(|error| {
        format!(
            "could not inspect the loaded LLVM runtime through /proc/self/maps: {error}; \
             ensure procfs is mounted and readable"
        )
    })?;
    let mut libraries = BTreeSet::new();
    for line in maps.lines() {
        let Some(path_start) = line.find('/') else {
            continue;
        };
        let path = decode_proc_map_path(&line[path_start..]);
        let is_llvm = path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.starts_with("libLLVM"));
        if is_llvm {
            libraries.insert(path);
        }
    }
    if libraries.is_empty() {
        return Err(
            "could not identify the loaded LLVM shared library; incremental reuse is disabled"
                .to_string(),
        );
    }

    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, b"domain", b"ori-build-llvm-runtime-v1");
    for library in libraries {
        let canonical = library.canonicalize().map_err(|error| {
            format!(
                "could not resolve loaded LLVM library '{}': {error}; check that it remains readable",
                library.display()
            )
        })?;
        let (digest, size) = hash_file_sha256(&canonical).map_err(|error| {
            format!(
                "could not hash loaded LLVM library '{}': {error}; check that it remains readable",
                canonical.display()
            )
        })?;
        update_digest_field(&mut hasher, b"library-path", &path_bytes(&canonical));
        update_digest_field(&mut hasher, b"library-size", &size.to_le_bytes());
        update_digest_field(&mut hasher, b"library-sha256", &digest);
    }
    Ok(hasher.finalize().into())
}

#[cfg(target_os = "linux")]
fn decode_proc_map_path(encoded: &str) -> PathBuf {
    use std::os::unix::ffi::OsStringExt;

    let bytes = encoded.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let octal = &bytes[index + 1..index + 4];
            if octal.iter().all(|byte| matches!(*byte, b'0'..=b'7')) {
                let value = (octal[0] - b'0') * 64 + (octal[1] - b'0') * 8 + (octal[2] - b'0');
                decoded.push(value);
                index += 4;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    PathBuf::from(OsString::from_vec(decoded))
}

#[cfg(windows)]
fn llvm_runtime_digest() -> Result<Sha256Digest, String> {
    let mut hasher = Sha256::new();
    update_digest_field(
        &mut hasher,
        b"domain",
        b"ori-build-static-llvm-covered-by-compiler-v1",
    );
    Ok(hasher.finalize().into())
}

#[cfg(not(any(target_os = "linux", windows)))]
fn llvm_runtime_digest() -> Result<Sha256Digest, String> {
    Err(
        "the dynamically loaded LLVM library cannot yet be fingerprinted on this platform; \
         incremental reuse is disabled"
            .to_string(),
    )
}

fn has_unfingerprinted_imports(parse_result: &crate::parser::ParseOutput) -> bool {
    !parse_result.module.imports.is_empty() || !parse_result.module.extension_imports.is_empty()
}

fn ineligibility_reason(
    has_explicit_imports: bool,
    options: &BuildOptions,
    env_names: impl IntoIterator<Item = OsString>,
) -> Option<String> {
    if has_explicit_imports {
        return Some(
            "module imports are not cached until dependency content is fingerprinted".to_string(),
        );
    }
    if options.emit.is_some() {
        return Some("--emit requests a non-link cache artifact".to_string());
    }
    if options.lib || options.dylib {
        return Some("library builds use a different link contract".to_string());
    }
    if options.wasm || options.js_bindings || options.wasm_opt {
        return Some("WebAssembly output uses a separate emission pipeline".to_string());
    }
    if options.emit_rc_remarks.is_some() {
        return Some("--emit-rc-remarks requires a fresh realization pass".to_string());
    }
    if options.sanitizer_env.is_some() {
        return Some(
            "sanitizer builds depend on an external Clang toolchain that is not fingerprinted"
                .to_string(),
        );
    }

    for name in env_names {
        if is_unfingerprinted_environment(&name) {
            return Some(format!(
                "{} is set and is not part of the cache fingerprint; unset it to enable reuse",
                name.to_string_lossy()
            ));
        }
    }
    None
}

fn is_unfingerprinted_environment(name: &OsStr) -> bool {
    let name = name.to_string_lossy();
    if name.starts_with("ORI_") {
        return name != crate::debug_flags::ORI_SANITIZE
            && name != crate::debug_flags::ORI_NO_REPR_OPT;
    }
    name.starts_with("LLVM_")
        || name.starts_with("CLANG_")
        || name.starts_with("LD_")
        || name.starts_with("DYLD_")
        || name == "RUST_LOG"
        || name == "SOURCE_DATE_EPOCH"
}

fn cache_directory() -> Result<PathBuf, String> {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        let root = PathBuf::from(xdg);
        if !root.is_absolute() {
            return Err(
                "XDG_CACHE_HOME must be an absolute path; set it to a writable absolute directory"
                    .to_string(),
            );
        }
        return Ok(root.join("ori").join("build").join(CACHE_FORMAT_VERSION));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        let root = PathBuf::from(home);
        if root.is_absolute() {
            return Ok(root
                .join(".cache")
                .join("ori")
                .join("build")
                .join(CACHE_FORMAT_VERSION));
        }
    }
    Ok(std::env::temp_dir()
        .join("ori-cache")
        .join("build")
        .join(CACHE_FORMAT_VERSION))
}

struct FingerprintInputs<'a> {
    source: &'a str,
    source_spelling: &'a Path,
    canonical_source: &'a Path,
    compiler_sha256: Sha256Digest,
    dependency_sha256: Sha256Digest,
    llvm_sha256: Sha256Digest,
    target: &'a str,
    cpu: &'a str,
    features: &'a str,
    opt_level: OptLevel,
    debug_level: DebugLevel,
    narrowing_policy: ori_repr::NarrowingPolicy,
    sanitizer: Option<&'a str>,
    lto: LtoMode,
    release: bool,
    compiler_version: &'a str,
    compiler_git_sha: &'a str,
    compiler_git_dirty: &'a str,
}

fn request_digest(inputs: &FingerprintInputs<'_>) -> Sha256Digest {
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, b"domain", b"ori-build-object-request-v2");
    update_digest_field(&mut hasher, b"format", CACHE_FORMAT_VERSION.as_bytes());
    update_digest_field(&mut hasher, b"source", inputs.source.as_bytes());
    update_digest_field(
        &mut hasher,
        b"source-spelling",
        &path_bytes(inputs.source_spelling),
    );
    update_digest_field(
        &mut hasher,
        b"canonical-source",
        &path_bytes(inputs.canonical_source),
    );
    update_digest_field(&mut hasher, b"compiler-sha256", &inputs.compiler_sha256);
    update_digest_field(&mut hasher, b"dependency-sha256", &inputs.dependency_sha256);
    update_digest_field(&mut hasher, b"llvm-sha256", &inputs.llvm_sha256);
    update_digest_field(&mut hasher, b"target", inputs.target.as_bytes());
    update_digest_field(&mut hasher, b"cpu", inputs.cpu.as_bytes());
    update_digest_field(&mut hasher, b"features", inputs.features.as_bytes());
    update_digest_field(
        &mut hasher,
        b"opt-level",
        opt_level_name(inputs.opt_level).as_bytes(),
    );
    update_digest_field(
        &mut hasher,
        b"debug-level",
        debug_level_name(inputs.debug_level).as_bytes(),
    );
    update_digest_field(
        &mut hasher,
        b"narrowing-policy",
        narrowing_name(inputs.narrowing_policy).as_bytes(),
    );
    update_digest_field(
        &mut hasher,
        b"sanitizer",
        inputs.sanitizer.unwrap_or("none").as_bytes(),
    );
    update_digest_field(&mut hasher, b"lto", lto_name(inputs.lto).as_bytes());
    update_digest_field(&mut hasher, b"release", &[u8::from(inputs.release)]);
    update_digest_field(
        &mut hasher,
        b"compiler-version",
        inputs.compiler_version.as_bytes(),
    );
    update_digest_field(
        &mut hasher,
        b"compiler-git-sha",
        inputs.compiler_git_sha.as_bytes(),
    );
    update_digest_field(
        &mut hasher,
        b"compiler-git-dirty",
        inputs.compiler_git_dirty.as_bytes(),
    );
    hasher.finalize().into()
}

fn cache_key(request_sha256: &Sha256Digest, config: &CacheConfig) -> CacheKey {
    let mut source_bytes = [0_u8; 8];
    source_bytes.copy_from_slice(&request_sha256[..8]);
    let mut dependency_bytes = [0_u8; 8];
    dependency_bytes.copy_from_slice(&request_sha256[8..16]);
    CacheKey::new(
        ContentHash::new(u64::from_le_bytes(source_bytes)),
        ContentHash::new(u64::from_le_bytes(dependency_bytes)),
        config,
    )
}

fn verify_cache_entry(
    object: &Path,
    encoded_manifest: &[u8],
    request_sha256: &Sha256Digest,
) -> bool {
    let Ok(manifest) = serde_json::from_slice::<ObjectManifest>(encoded_manifest) else {
        return false;
    };
    if manifest.schema != CACHE_MANIFEST_SCHEMA
        || manifest.request_sha256 != digest_hex(request_sha256)
    {
        return false;
    }
    let Ok((object_sha256, object_size)) = hash_file_sha256(object) else {
        return false;
    };
    manifest.object_size == object_size && manifest.object_sha256 == digest_hex(&object_sha256)
}

fn hash_file_sha256(path: &Path) -> io::Result<(Sha256Digest, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        size = size
            .checked_add(read as u64)
            .ok_or_else(|| io::Error::other("file length exceeds u64"))?;
    }
    Ok((hasher.finalize().into(), size))
}

fn update_digest_field(hasher: &mut Sha256, label: &[u8], value: &[u8]) {
    hasher.update((label.len() as u64).to_le_bytes());
    hasher.update(label);
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

fn digest_hex(digest: &Sha256Digest) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rendered = String::with_capacity(64);
    for byte in digest {
        rendered.push(char::from(HEX[usize::from(byte >> 4)]));
        rendered.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    rendered
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().to_string_lossy().into_owned().into_bytes()
}

const fn opt_level_name(level: OptLevel) -> &'static str {
    match level {
        OptLevel::O0 => "O0",
        OptLevel::O1 => "O1",
        OptLevel::O2 => "O2",
        OptLevel::O3 => "O3",
        OptLevel::Os => "Os",
        OptLevel::Oz => "Oz",
    }
}

const fn debug_level_name(level: DebugLevel) -> &'static str {
    match level {
        DebugLevel::None => "none",
        DebugLevel::LineTablesOnly => "line-tables",
        DebugLevel::Full => "full",
    }
}

const fn narrowing_name(policy: ori_repr::NarrowingPolicy) -> &'static str {
    match policy {
        ori_repr::NarrowingPolicy::Aggressive => "aggressive",
        ori_repr::NarrowingPolicy::Conservative => "conservative",
        ori_repr::NarrowingPolicy::Disabled => "disabled",
    }
}

const fn lto_name(mode: LtoMode) -> &'static str {
    match mode {
        LtoMode::Off => "off",
        LtoMode::Thin => "thin",
        LtoMode::Full => "full",
    }
}

#[cfg(test)]
mod tests;
