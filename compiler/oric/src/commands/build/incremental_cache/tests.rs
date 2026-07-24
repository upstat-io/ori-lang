use super::*;

use oric::{CompilerDb, Db};

fn inputs<'a>(source: &'a str, path: &'a Path) -> FingerprintInputs<'a> {
    FingerprintInputs {
        source,
        source_spelling: path,
        canonical_source: path,
        compiler_sha256: [7; 32],
        dependency_sha256: [8; 32],
        llvm_sha256: [9; 32],
        target: "x86_64-unknown-linux-gnu",
        cpu: "generic",
        features: "",
        opt_level: OptLevel::O0,
        debug_level: DebugLevel::Full,
        narrowing_policy: ori_repr::NarrowingPolicy::Aggressive,
        sanitizer: None,
        lto: LtoMode::Off,
        release: false,
        compiler_version: "test-version",
        compiler_git_sha: "test-sha",
        compiler_git_dirty: "0",
    }
}

fn fingerprint(inputs: &FingerprintInputs<'_>) -> String {
    digest_hex(&request_digest(inputs))
}

#[test]
fn import_and_unaccounted_ori_environment_are_ineligible() {
    let options = BuildOptions::default();
    assert!(ineligibility_reason(true, &options, Vec::<OsString>::new(),).is_some());
    for name in [
        "ORI_STDLIB",
        "ORI_WORKSPACE_DIR",
        "RUST_LOG",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
    ] {
        let reason =
            ineligibility_reason(false, &options, [OsString::from(name)]).unwrap_or_else(|| {
                panic!("external compiler inputs must conservatively bypass the object cache")
            });
        assert!(reason.contains(name));
    }
}

#[test]
fn every_parsed_import_form_is_ineligible() {
    let db = CompilerDb::new();
    let parse = |source: &str| {
        let tokens = crate::lex(source, db.interner());
        crate::parser::parse(&tokens, db.interner())
    };

    assert!(has_unfingerprinted_imports(&parse(
        "use std.testing { assert };"
    )));
    assert!(has_unfingerprinted_imports(&parse(
        "pub use \"./helpers\" { helper };"
    )));
    assert!(has_unfingerprinted_imports(&parse(
        "extension std.iter.extensions { Iterator.count }"
    )));
    assert!(!has_unfingerprinted_imports(&parse("@main () -> int = 0;")));
}

#[test]
fn request_fingerprint_covers_every_owned_codegen_input() {
    let path = Path::new("/tmp/main.ori");
    let base = inputs("@main () -> int = 0;", path);
    let base_fingerprint = fingerprint(&base);

    let mutations = [
        fingerprint(&FingerprintInputs {
            source: "@main () -> int = 1;",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            source_spelling: Path::new("./main.ori"),
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            canonical_source: Path::new("/tmp/other.ori"),
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            compiler_sha256: [10; 32],
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            dependency_sha256: [11; 32],
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            llvm_sha256: [12; 32],
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            compiler_git_sha: "other-sha",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            target: "aarch64-unknown-linux-gnu",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            cpu: "native",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            features: "+avx2",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            opt_level: OptLevel::O1,
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            debug_level: DebugLevel::None,
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            narrowing_policy: ori_repr::NarrowingPolicy::Disabled,
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            sanitizer: Some("address"),
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            lto: LtoMode::Thin,
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            release: true,
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            compiler_version: "other-version",
            ..inputs(base.source, path)
        }),
        fingerprint(&FingerprintInputs {
            compiler_git_dirty: "1",
            ..inputs(base.source, path)
        }),
    ];

    assert!(mutations
        .iter()
        .all(|candidate| candidate != &base_fingerprint));
}

#[test]
fn cache_manifest_rejects_every_integrity_mismatch() {
    let directory =
        tempfile::tempdir().unwrap_or_else(|_| panic!("temporary directory must be creatable"));
    let object = directory.path().join("object.o");
    std::fs::write(&object, b"complete object")
        .unwrap_or_else(|_| panic!("object fixture must be writable"));
    let request = [21; 32];
    let (object_digest, object_size) =
        hash_file_sha256(&object).unwrap_or_else(|_| panic!("object fixture must be hashable"));
    let valid = ObjectManifest {
        schema: CACHE_MANIFEST_SCHEMA,
        request_sha256: digest_hex(&request),
        object_sha256: digest_hex(&object_digest),
        object_size,
    };
    let encode = |manifest: &ObjectManifest| {
        serde_json::to_vec(manifest).unwrap_or_else(|_| panic!("test manifest must serialize"))
    };

    assert!(verify_cache_entry(&object, &encode(&valid), &request));
    assert!(!verify_cache_entry(&object, b"not-json", &request));
    assert!(!verify_cache_entry(
        &object,
        &encode(&ObjectManifest {
            schema: CACHE_MANIFEST_SCHEMA + 1,
            ..valid_manifest(&valid)
        }),
        &request,
    ));
    assert!(!verify_cache_entry(
        &object,
        &encode(&ObjectManifest {
            request_sha256: digest_hex(&[22; 32]),
            ..valid_manifest(&valid)
        }),
        &request,
    ));
    assert!(!verify_cache_entry(
        &object,
        &encode(&ObjectManifest {
            object_sha256: digest_hex(&[23; 32]),
            ..valid_manifest(&valid)
        }),
        &request,
    ));
    assert!(!verify_cache_entry(
        &object,
        &encode(&ObjectManifest {
            object_size: object_size + 1,
            ..valid_manifest(&valid)
        }),
        &request,
    ));

    std::fs::write(&object, b"corrupted object")
        .unwrap_or_else(|_| panic!("object fixture must be mutable"));
    assert!(!verify_cache_entry(&object, &encode(&valid), &request));
}

fn valid_manifest(manifest: &ObjectManifest) -> ObjectManifest {
    ObjectManifest {
        schema: manifest.schema,
        request_sha256: manifest.request_sha256.clone(),
        object_sha256: manifest.object_sha256.clone(),
        object_size: manifest.object_size,
    }
}

#[cfg(target_os = "linux")]
#[test]
fn loaded_llvm_runtime_has_a_strong_identity() {
    assert!(llvm_runtime_digest().is_ok());
}

#[cfg(target_os = "linux")]
#[test]
fn proc_map_paths_decode_octal_escapes() {
    assert_eq!(
        decode_proc_map_path("/tmp/LLVM\\040runtime/libLLVM.so"),
        PathBuf::from("/tmp/LLVM runtime/libLLVM.so")
    );
}
