//! Per-spawn protocol-nonce generation for worker isolation.

use std::sync::atomic::{AtomicU64, Ordering};

/// Generate the per-spawn protocol nonce stamped onto every protocol line.
///
/// Unguessable by test code: 16 bytes of `/dev/urandom` when available,
/// otherwise a randomly-keyed hash of time + pid + a spawn counter (the
/// hasher keys come from OS entropy). 32 hex chars either way.
pub(super) fn generate_protocol_token() -> String {
    urandom_token().unwrap_or_else(fallback_token)
}

/// Read 16 random bytes from `/dev/urandom`, hex-encoded.
#[cfg(unix)]
fn urandom_token() -> Option<String> {
    use std::fmt::Write as _;
    use std::io::Read as _;
    let mut bytes = [0u8; 16];
    std::fs::File::open("/dev/urandom")
        .ok()?
        .read_exact(&mut bytes)
        .ok()?;
    let mut token = String::with_capacity(2 * bytes.len());
    for byte in bytes {
        // Writing into a String cannot fail.
        let _ = write!(token, "{byte:02x}");
    }
    Some(token)
}

/// No `/dev/urandom` off unix: always defer to the hash fallback.
#[cfg(not(unix))]
fn urandom_token() -> Option<String> {
    None
}

/// Hash time + pid + a spawn counter through randomly-keyed hashers.
fn fallback_token() -> String {
    use std::hash::{BuildHasher, Hash, Hasher};
    static SPAWN_COUNTER: AtomicU64 = AtomicU64::new(0);
    // RandomState keys are seeded from OS entropy per process.
    let state = std::collections::hash_map::RandomState::new();
    let word = |salt: u64| {
        let mut hasher = state.build_hasher();
        salt.hash(&mut hasher);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        std::process::id().hash(&mut hasher);
        SPAWN_COUNTER
            .fetch_add(1, Ordering::Relaxed)
            .hash(&mut hasher);
        hasher.finish()
    };
    format!("{:016x}{:016x}", word(0), word(1))
}
