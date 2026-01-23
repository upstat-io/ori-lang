// ARC Runtime Header Generation
//
// Generates the sigil_arc.h header file that provides the C interface
// for the ARC runtime library.

use super::super::traits::ArcConfig;
use super::templates;

/// Generate the sigil_arc.h header file
pub fn generate_header(config: &ArcConfig) -> String {
    let mut output = String::new();

    // Preamble
    output.push_str(templates::HEADER_PREAMBLE);

    // Refcount type definition based on thread safety
    if config.thread_safe {
        output.push_str(templates::THREAD_SAFE_REFCOUNT);
    } else {
        output.push_str(templates::SINGLE_THREADED_REFCOUNT);
    }

    // ARC header structure
    output.push_str(templates::ARC_HEADER_STRUCT);

    // Type definitions
    output.push_str(templates::STRING_TYPE);
    output.push_str(templates::LIST_TYPE);
    output.push_str(templates::MAP_TYPE);
    output.push_str(templates::CLOSURE_TYPE);

    // Function declarations
    output.push_str(templates::ARC_FUNCTIONS);
    output.push_str(templates::STRING_FUNCTIONS);
    output.push_str(templates::LIST_FUNCTIONS);
    output.push_str(templates::MAP_FUNCTIONS);

    // Debug tracking (conditional)
    if config.debug_tracking {
        output.push_str(templates::DEBUG_TRACKING);
    }

    // SSO threshold configuration
    output.push_str(&format!(
        "\n/* Configuration */\n#define SIGIL_VALUE_TYPE_THRESHOLD {}\n",
        config.value_type_threshold
    ));

    // Postamble
    output.push_str(templates::HEADER_POSTAMBLE);

    output
}

/// Generate a minimal header for embedding in generated code
pub fn generate_inline_header(config: &ArcConfig) -> String {
    let mut output = String::new();

    output.push_str("/* Sigil ARC Runtime (inline) */\n\n");

    // Refcount type
    if config.thread_safe {
        output.push_str("#include <stdatomic.h>\n");
        output.push_str("typedef _Atomic size_t sigil_refcount_t;\n");
    } else {
        output.push_str("typedef size_t sigil_refcount_t;\n");
    }

    // Minimal ARC header
    output.push_str(r#"
typedef struct { sigil_refcount_t refcount; } SigilArcHeader;

static inline SigilArcHeader* sigil_arc_header(void* ptr) {
    return (SigilArcHeader*)((char*)ptr - sizeof(SigilArcHeader));
}
"#);

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_header_default() {
        let config = ArcConfig::default();
        let header = generate_header(&config);

        assert!(header.contains("#ifndef SIGIL_ARC_H"));
        assert!(header.contains("#endif /* SIGIL_ARC_H */"));
        assert!(header.contains("typedef size_t sigil_refcount_t"));
        assert!(header.contains("SigilArcHeader"));
        assert!(header.contains("SigilString"));
    }

    #[test]
    fn test_generate_header_thread_safe() {
        let config = ArcConfig::thread_safe();
        let header = generate_header(&config);

        assert!(header.contains("#include <stdatomic.h>"));
        assert!(header.contains("_Atomic size_t sigil_refcount_t"));
    }

    #[test]
    fn test_generate_header_debug() {
        let config = ArcConfig::debug();
        let header = generate_header(&config);

        assert!(header.contains("SIGIL_DEBUG_ARC"));
        assert!(header.contains("sigil_debug_leak_report"));
    }

    #[test]
    fn test_generate_inline_header() {
        let config = ArcConfig::default();
        let header = generate_inline_header(&config);

        assert!(header.contains("sigil_refcount_t"));
        assert!(header.contains("SigilArcHeader"));
        assert!(!header.contains("#ifndef")); // No include guards
    }
}
