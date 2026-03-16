//! Object file verification helpers for AOT tests.

/// Object file verification result.
#[derive(Debug, Default)]
pub struct ObjectVerification {
    /// Object file format.
    pub format: ObjectFormat,
    /// Architecture.
    pub architecture: String,
    /// Symbols (name, kind).
    pub symbols: Vec<(String, SymbolKind)>,
    /// Sections.
    pub sections: Vec<String>,
    /// Whether the object contains debug info.
    pub has_debug_info: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub enum ObjectFormat {
    #[default]
    Unknown,
    Elf,
    MachO,
    Coff,
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Text,
    Data,
    Bss,
    Unknown,
}

/// Parse an object file and extract verification information.
///
/// # Errors
///
/// Returns an error if the object file is invalid.
pub fn parse_object(bytes: &[u8]) -> Result<ObjectVerification, String> {
    use object::{Object, ObjectSection, ObjectSymbol};

    let obj = object::File::parse(bytes).map_err(|e| format!("Object parse error: {e}"))?;

    let format = match obj.format() {
        object::BinaryFormat::Elf => ObjectFormat::Elf,
        object::BinaryFormat::MachO => ObjectFormat::MachO,
        object::BinaryFormat::Coff | object::BinaryFormat::Pe => ObjectFormat::Coff,
        object::BinaryFormat::Wasm => ObjectFormat::Wasm,
        _ => ObjectFormat::Unknown,
    };

    let architecture = format!("{:?}", obj.architecture());

    let symbols: Vec<_> = obj
        .symbols()
        .filter_map(|sym| {
            let name = sym.name().ok()?.to_string();
            let kind = match sym.section() {
                object::SymbolSection::Section(idx) => {
                    if let Ok(section) = obj.section_by_index(idx) {
                        if section.name().ok()?.contains("text") {
                            SymbolKind::Text
                        } else if section.name().ok()?.contains("data") {
                            SymbolKind::Data
                        } else if section.name().ok()?.contains("bss") {
                            SymbolKind::Bss
                        } else {
                            SymbolKind::Unknown
                        }
                    } else {
                        SymbolKind::Unknown
                    }
                }
                _ => SymbolKind::Unknown,
            };
            Some((name, kind))
        })
        .collect();

    let sections: Vec<_> = obj
        .sections()
        .filter_map(|s| s.name().ok().map(ToString::to_string))
        .collect();

    let has_debug_info = sections
        .iter()
        .any(|s| s.contains("debug") || s.starts_with(".debug") || s.starts_with("__debug"));

    Ok(ObjectVerification {
        format,
        architecture,
        symbols,
        sections,
        has_debug_info,
    })
}

/// Check if an object file contains a symbol with the given name.
pub fn object_has_symbol(verification: &ObjectVerification, name: &str) -> bool {
    verification.symbols.iter().any(|(n, _)| n.contains(name))
}

/// Check if an object file contains a section with the given name.
pub fn object_has_section(verification: &ObjectVerification, name: &str) -> bool {
    verification.sections.iter().any(|s| s.contains(name))
}

/// Assert that an object file has specific symbols.
#[macro_export]
macro_rules! assert_object_symbols {
    ($verification:expr, $($symbol:expr),+ $(,)?) => {
        $(
            assert!(
                $crate::util::object_has_symbol(&$verification, $symbol),
                "Expected symbol '{}' not found in object. Symbols: {:?}",
                $symbol,
                $verification.symbols.iter().map(|(n, _)| n).collect::<Vec<_>>()
            );
        )+
    };
}
