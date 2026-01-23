// ARC Runtime Implementation Generation
//
// Generates the sigil_arc.c implementation file that provides the
// actual ARC runtime library functions.

use super::super::traits::ArcConfig;
use super::templates;

/// Generate the sigil_arc.c implementation file
pub fn generate_impl(config: &ArcConfig) -> String {
    let mut output = String::new();

    // Preamble
    output.push_str(templates::IMPL_PREAMBLE);

    // Refcount macros (must be defined before use)
    if config.thread_safe {
        output.push_str(r#"
#include <stdatomic.h>
#define SIGIL_REFCOUNT_INC(rc) atomic_fetch_add(&(rc), 1)
#define SIGIL_REFCOUNT_DEC(rc) atomic_fetch_sub(&(rc), 1)
#define SIGIL_REFCOUNT_LOAD(rc) atomic_load(&(rc))
#define SIGIL_REFCOUNT_INIT(rc, val) atomic_init(&(rc), val)
"#);
    } else {
        output.push_str(r#"
#define SIGIL_REFCOUNT_INC(rc) (++(rc))
#define SIGIL_REFCOUNT_DEC(rc) (--(rc))
#define SIGIL_REFCOUNT_LOAD(rc) (rc)
#define SIGIL_REFCOUNT_INIT(rc, val) ((rc) = (val))
"#);
    }

    // SSO threshold and string helper
    output.push_str(&format!(
        "\n#define SIGIL_SSO_THRESHOLD {}\n",
        config.sso_threshold
    ));
    output.push_str("#define SIGIL_STRING_IS_SSO(s) ((s)->storage.sso.flags & 1)\n");

    // Core ARC implementation
    output.push_str(templates::ARC_IMPL);

    // String implementation
    output.push_str(templates::STRING_IMPL);

    // List implementation
    output.push_str(templates::LIST_IMPL);

    // Debug tracking (conditional)
    if config.debug_tracking {
        output.push_str(templates::DEBUG_IMPL);
    }

    output
}

/// Generate an inline implementation suitable for embedding
pub fn generate_inline_impl(config: &ArcConfig) -> String {
    let mut output = String::new();

    output.push_str("/* Sigil ARC Runtime (inline implementation) */\n\n");

    // Refcount macros
    if config.thread_safe {
        output.push_str(r#"
#define SIGIL_REFCOUNT_INC(rc) atomic_fetch_add(&(rc), 1)
#define SIGIL_REFCOUNT_DEC(rc) atomic_fetch_sub(&(rc), 1)
#define SIGIL_REFCOUNT_LOAD(rc) atomic_load(&(rc))
#define SIGIL_REFCOUNT_INIT(rc, val) atomic_init(&(rc), val)
"#);
    } else {
        output.push_str(r#"
#define SIGIL_REFCOUNT_INC(rc) (++(rc))
#define SIGIL_REFCOUNT_DEC(rc) (--(rc))
#define SIGIL_REFCOUNT_LOAD(rc) (rc)
#define SIGIL_REFCOUNT_INIT(rc, val) ((rc) = (val))
"#);
    }

    // Minimal inline implementations
    output.push_str(r#"
static inline void* sigil_arc_alloc(size_t size) {
    SigilArcHeader* header = (SigilArcHeader*)malloc(sizeof(SigilArcHeader) + size);
    if (!header) abort();
    SIGIL_REFCOUNT_INIT(header->refcount, 1);
    return (char*)header + sizeof(SigilArcHeader);
}

static inline void sigil_arc_retain(void* ptr) {
    if (!ptr) return;
    SIGIL_REFCOUNT_INC(sigil_arc_header(ptr)->refcount);
}

static inline bool sigil_arc_release(void* ptr) {
    if (!ptr) return false;
    SigilArcHeader* header = sigil_arc_header(ptr);
    if (SIGIL_REFCOUNT_DEC(header->refcount) == 1) {
        free(header);
        return true;
    }
    return false;
}
"#);

    output
}

/// Generate just the string implementation (for testing)
pub fn generate_string_impl(config: &ArcConfig) -> String {
    let mut output = String::new();

    // Refcount macros
    if config.thread_safe {
        output.push_str("#include <stdatomic.h>\n");
        output.push_str("#define SIGIL_REFCOUNT_INC(rc) atomic_fetch_add(&(rc), 1)\n");
        output.push_str("#define SIGIL_REFCOUNT_DEC(rc) atomic_fetch_sub(&(rc), 1)\n");
        output.push_str("#define SIGIL_REFCOUNT_INIT(rc, val) atomic_init(&(rc), val)\n");
    } else {
        output.push_str("#define SIGIL_REFCOUNT_INC(rc) (++(rc))\n");
        output.push_str("#define SIGIL_REFCOUNT_DEC(rc) (--(rc))\n");
        output.push_str("#define SIGIL_REFCOUNT_INIT(rc, val) ((rc) = (val))\n");
    }

    output.push_str(&format!(
        "#define SIGIL_SSO_THRESHOLD {}\n",
        config.sso_threshold
    ));
    output.push_str("#define SIGIL_STRING_IS_SSO(s) ((s)->storage.sso.flags & 1)\n\n");

    output.push_str(templates::STRING_IMPL);

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_impl_default() {
        let config = ArcConfig::default();
        let impl_code = generate_impl(&config);

        assert!(impl_code.contains("sigil_arc_alloc"));
        assert!(impl_code.contains("sigil_arc_retain"));
        assert!(impl_code.contains("sigil_arc_release"));
        assert!(impl_code.contains("sigil_string_new"));
        assert!(impl_code.contains("sigil_list_new"));
    }

    #[test]
    fn test_generate_impl_thread_safe() {
        let config = ArcConfig::thread_safe();
        let impl_code = generate_impl(&config);

        assert!(impl_code.contains("atomic_fetch_add"));
        assert!(impl_code.contains("atomic_fetch_sub"));
    }

    #[test]
    fn test_generate_impl_debug() {
        let config = ArcConfig::debug();
        let impl_code = generate_impl(&config);

        assert!(impl_code.contains("SIGIL_DEBUG_ARC"));
        assert!(impl_code.contains("sigil_debug_record_alloc"));
    }

    #[test]
    fn test_generate_inline_impl() {
        let config = ArcConfig::default();
        let impl_code = generate_inline_impl(&config);

        assert!(impl_code.contains("static inline void* sigil_arc_alloc"));
        assert!(impl_code.contains("static inline void sigil_arc_retain"));
    }

    #[test]
    fn test_sso_threshold_custom() {
        let mut config = ArcConfig::default();
        config.sso_threshold = 32;
        let impl_code = generate_impl(&config);

        assert!(impl_code.contains("#define SIGIL_SSO_THRESHOLD 32"));
    }
}
