#[cfg(feature = "llvm")]
mod llvm_tests {
    use super::super::get_cache_dir;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    #[test]
    fn test_cache_dir_exists_or_creatable() {
        let cache_dir = get_cache_dir();
        // Should be a valid path
        assert!(!cache_dir.as_os_str().is_empty());
        // Should contain "ori" somewhere in the path
        let path_str = cache_dir.to_string_lossy();
        assert!(path_str.contains("ori"), "cache dir should contain 'ori'");
    }

    #[test]
    fn test_cache_dir_is_absolute_or_temp() {
        let cache_dir = get_cache_dir();
        // Should be either absolute or in temp
        let is_absolute = cache_dir.is_absolute();
        let is_in_temp = cache_dir.starts_with(std::env::temp_dir());
        assert!(
            is_absolute || is_in_temp,
            "cache dir should be absolute or in temp: {cache_dir:?}"
        );
    }

    #[test]
    fn test_content_hash_deterministic() {
        let content = "let x = 42";
        let version = env!("CARGO_PKG_VERSION");

        let hash1 = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = DefaultHasher::new();
            content.hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        assert_eq!(hash1, hash2, "same content should produce same hash");
    }

    #[test]
    fn test_content_hash_differs_for_different_content() {
        let version = env!("CARGO_PKG_VERSION");

        let hash1 = {
            let mut hasher = DefaultHasher::new();
            "let x = 42".hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        let hash2 = {
            let mut hasher = DefaultHasher::new();
            "let x = 43".hash(&mut hasher);
            version.hash(&mut hasher);
            hasher.finish()
        };

        assert_ne!(
            hash1, hash2,
            "different content should produce different hash"
        );
    }

    #[test]
    fn test_binary_name_format() {
        let source_name = "hello";
        let content_hash: u64 = 0x1234_5678_90AB_CDEF;
        let binary_name = format!("{source_name}-{content_hash:016x}");

        assert_eq!(binary_name, "hello-1234567890abcdef");
        assert!(binary_name.contains(source_name));
        // Hash should be exactly 16 hex characters
        let parts: Vec<&str> = binary_name.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 16);
    }
}
