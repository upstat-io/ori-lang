//! Parallel file parsing using Rayon.
//!
//! This module provides file-level parallelism for parsing Sigil source files.
//! Each file is parsed independently, making this embarrassingly parallel.

use std::path::{Path, PathBuf};
use std::fs;
use rayon::prelude::*;

use crate::intern::StringInterner;
use crate::syntax::{Lexer, Parser, ExprArena, Item, Span, Import, ImportPath};
use crate::errors::Diagnostic;
use super::ParallelConfig;

/// Configuration for the parallel parser.
#[derive(Clone, Debug)]
pub struct ParserConfig {
    /// Whether to continue parsing after errors.
    pub recover_errors: bool,
    /// Maximum number of errors before stopping.
    pub max_errors: usize,
    /// Whether to preserve comments for formatting.
    pub preserve_comments: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        ParserConfig {
            recover_errors: true,
            max_errors: 100,
            preserve_comments: false,
        }
    }
}

/// Result of parsing a single file.
pub struct ParsedFile {
    /// Path to the source file.
    pub path: PathBuf,
    /// Parsed items (functions, types, etc.).
    pub items: Vec<Item>,
    /// Expression arena for this file.
    pub arena: ExprArena,
    /// Parse errors encountered.
    pub errors: Vec<Diagnostic>,
    /// Whether parsing succeeded.
    pub success: bool,
    /// Module name derived from path.
    pub module_name: String,
    /// Import dependencies (module paths).
    pub imports: Vec<String>,
}

impl std::fmt::Debug for ParsedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParsedFile")
            .field("path", &self.path)
            .field("items", &self.items.len())
            .field("errors", &self.errors.len())
            .field("success", &self.success)
            .field("module_name", &self.module_name)
            .field("imports", &self.imports)
            .finish()
    }
}

impl ParsedFile {
    /// Check if the file has any errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get the number of items parsed.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

/// Parallel file parser using Rayon.
pub struct ParallelParser<'a> {
    interner: &'a StringInterner,
    parallel_config: ParallelConfig,
    parser_config: ParserConfig,
}

impl<'a> ParallelParser<'a> {
    /// Create a new parallel parser.
    pub fn new(interner: &'a StringInterner, parallel_config: ParallelConfig) -> Self {
        ParallelParser {
            interner,
            parallel_config,
            parser_config: ParserConfig::default(),
        }
    }

    /// Create a parallel parser with custom parser configuration.
    pub fn with_parser_config(
        interner: &'a StringInterner,
        parallel_config: ParallelConfig,
        parser_config: ParserConfig,
    ) -> Self {
        ParallelParser {
            interner,
            parallel_config,
            parser_config,
        }
    }

    /// Parse multiple files in parallel.
    pub fn parse_files<P: AsRef<Path> + Sync>(&self, paths: &[P]) -> Vec<ParsedFile> {
        if paths.len() <= 1 || self.parallel_config.num_threads == 1 {
            // Sequential parsing for single file or single-threaded config
            paths.iter().map(|p| self.parse_file(p.as_ref())).collect()
        } else {
            // Parallel parsing with Rayon
            paths
                .par_iter()
                .map(|p| self.parse_file(p.as_ref()))
                .collect()
        }
    }

    /// Parse a single file.
    pub fn parse_file(&self, path: &Path) -> ParsedFile {
        let module_name = self.derive_module_name(path);

        // Read the file
        let source = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                return ParsedFile {
                    path: path.to_path_buf(),
                    items: Vec::new(),
                    arena: ExprArena::new(),
                    errors: vec![Diagnostic::error(
                        format!("Failed to read file: {}", e),
                        Span::DUMMY,
                    )],
                    success: false,
                    module_name,
                    imports: Vec::new(),
                };
            }
        };

        self.parse_source(path, &source, module_name)
    }

    /// Parse source code directly (useful for testing).
    pub fn parse_source(&self, path: &Path, source: &str, module_name: String) -> ParsedFile {
        // Lex the source
        let lexer = Lexer::new(source, self.interner);
        let tokens = lexer.lex_all();

        // Parse the tokens
        let parser = Parser::new(&tokens, self.interner);
        let result = parser.parse_module();

        // Extract imports
        let imports = self.extract_imports(&result.imports);

        let success = result.diagnostics.is_empty();
        ParsedFile {
            path: path.to_path_buf(),
            items: result.items,
            arena: result.arena,
            errors: result.diagnostics,
            success,
            module_name,
            imports,
        }
    }

    /// Derive module name from file path.
    fn derive_module_name(&self, path: &Path) -> String {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    /// Extract import dependencies from parsed imports.
    fn extract_imports(&self, imports: &[Import]) -> Vec<String> {
        imports
            .iter()
            .map(|import| {
                match &import.path {
                    ImportPath::Relative(path) => path.clone(),
                    ImportPath::Module(names) => {
                        names
                            .iter()
                            .map(|n| self.interner.lookup(*n))
                            .collect::<Vec<_>>()
                            .join(".")
                    }
                }
            })
            .collect()
    }
}

/// Parse a single source string (for testing/benchmarking).
pub fn parse_source_single(
    source: &str,
    interner: &StringInterner,
) -> (Vec<Item>, ExprArena, Vec<Diagnostic>) {
    let lexer = Lexer::new(source, interner);
    let tokens = lexer.lex_all();
    let parser = Parser::new(&tokens, interner);
    let result = parser.parse_module();
    (result.items, result.arena, result.diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_source_single() {
        let interner = StringInterner::new();
        let source = r#"
            @add (a: int, b: int) -> int = a + b
        "#;

        let (items, _arena, errors) = parse_source_single(source, &interner);
        assert!(errors.is_empty());
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_parallel_parser_single_file() {
        let interner = StringInterner::new();
        let config = ParallelConfig::default();
        let parser = ParallelParser::new(&interner, config);

        // Create a temp file
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "@main () -> void = print(\"hello\")").unwrap();

        let results = parser.parse_files(&[temp.path()]);
        assert_eq!(results.len(), 1);
        assert!(results[0].success || results[0].errors.is_empty());
    }

    #[test]
    fn test_parallel_parser_multiple_files() {
        let interner = StringInterner::new();
        let config = ParallelConfig::default();
        let parser = ParallelParser::new(&interner, config);

        // Create temp files
        let mut temps = Vec::new();
        for i in 0..4 {
            let mut temp = NamedTempFile::new().unwrap();
            writeln!(temp, "@func{} () -> int = {}", i, i).unwrap();
            temps.push(temp);
        }

        let paths: Vec<_> = temps.iter().map(|t| t.path()).collect();
        let results = parser.parse_files(&paths);

        assert_eq!(results.len(), 4);
    }

    #[test]
    fn test_derive_module_name() {
        let interner = StringInterner::new();
        let config = ParallelConfig::default();
        let parser = ParallelParser::new(&interner, config);

        assert_eq!(
            parser.derive_module_name(Path::new("/path/to/math.si")),
            "math"
        );
        assert_eq!(
            parser.derive_module_name(Path::new("utils.si")),
            "utils"
        );
    }

    #[test]
    fn test_parser_config_default() {
        let config = ParserConfig::default();
        assert!(config.recover_errors);
        assert_eq!(config.max_errors, 100);
        assert!(!config.preserve_comments);
    }
}
