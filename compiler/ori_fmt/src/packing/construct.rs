//! Construct kinds for packing decisions.

/// The kind of container being formatted.
///
/// This enum enumerates all container types that need packing decisions.
/// Each variant maps to a specific packing strategy via `determine_packing()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConstructKind {
    // ========================================================================
    // Always Stacked (Spec lines 78-90)
    // ========================================================================
    /// Top-level run expression: `run(...)` at function body level
    ///
    /// Always stacked with each statement on its own line.
    RunTopLevel,

    /// Try expression: `try(...)`
    ///
    /// Always stacked.
    Try,

    /// Match expression: `match(val, ...)`
    ///
    /// Always stacked with each arm on its own line.
    Match,

    /// Recurse expression: `recurse(...)`
    ///
    /// Always stacked with named args on their own lines.
    Recurse,

    /// Parallel expression: `parallel(...)`
    ///
    /// Always stacked.
    Parallel,

    /// Spawn expression: `spawn(...)`
    ///
    /// Always stacked.
    Spawn,

    /// Nursery expression: `nursery(...)`
    ///
    /// Always stacked.
    Nursery,

    // ========================================================================
    // Width-Based: One Per Line When Broken (Spec lines 64-74)
    // ========================================================================
    /// Function parameters: `@foo (x: int, y: int)`
    ///
    /// Inline if fits, one per line otherwise.
    FunctionParams,

    /// Function arguments: `foo(x: 1, y: 2)`
    ///
    /// Inline if fits, one per line otherwise.
    FunctionArgs,

    /// Generic parameters: `<T, U>`
    ///
    /// Inline if fits, one per line otherwise.
    GenericParams,

    /// Where constraints: `where T: Clone`
    ///
    /// Inline if fits, one per line otherwise.
    WhereConstraints,

    /// Capability list: `uses Http, FileSystem`
    ///
    /// Inline if fits, one per line otherwise.
    Capabilities,

    /// Struct field definitions: `type Foo = { x: int, y: int }`
    ///
    /// Inline if fits, one per line otherwise.
    StructFieldsDef,

    /// Struct literal fields: `Point { x: 1, y: 2 }`
    ///
    /// Inline if fits, one per line otherwise.
    StructFieldsLiteral,

    /// Sum type variants: `A | B | C`
    ///
    /// Inline if fits, one per line otherwise.
    SumVariants,

    /// Map entries: `{ "key": value }`
    ///
    /// Inline if fits, one per line otherwise.
    MapEntries,

    /// Tuple elements: `(a, b, c)`
    ///
    /// Inline if fits, one per line otherwise.
    TupleElements,

    /// Import items: `use "./foo" { a, b, c }`
    ///
    /// Inline if fits, one per line otherwise.
    ImportItems,

    // ========================================================================
    // Width-Based: Multiple Per Line for Simple Items (Spec line 75)
    // ========================================================================
    /// Simple list (literals, identifiers only): `[1, 2, 3]`
    ///
    /// Can pack multiple per line when broken.
    ListSimple,

    // ========================================================================
    // Width-Based: One Per Line for Complex Items (Spec line 76)
    // ========================================================================
    /// Complex list (structs, calls, nested): `[foo(), bar()]`
    ///
    /// One per line when broken.
    ListComplex,

    // ========================================================================
    // Context-Dependent
    // ========================================================================
    /// Nested run expression: `run(...)` inside another expression
    ///
    /// Width-based (can inline if fits), unlike top-level run.
    RunNested,

    /// Match arms (the arm list itself)
    ///
    /// Always one per line regardless of width.
    MatchArms,
}

impl ConstructKind {
    /// Check if this construct is always stacked (never inline).
    #[inline]
    pub fn is_always_stacked(self) -> bool {
        matches!(
            self,
            ConstructKind::RunTopLevel
                | ConstructKind::Try
                | ConstructKind::Match
                | ConstructKind::Recurse
                | ConstructKind::Parallel
                | ConstructKind::Spawn
                | ConstructKind::Nursery
                | ConstructKind::MatchArms
        )
    }

    /// Check if this construct uses comma separators.
    #[inline]
    pub fn uses_commas(self) -> bool {
        !matches!(self, ConstructKind::SumVariants)
    }

    /// Check if this is a run construct (top-level or nested).
    #[inline]
    pub fn is_run(self) -> bool {
        matches!(self, ConstructKind::RunTopLevel | ConstructKind::RunNested)
    }

    /// Check if this is a list construct.
    #[inline]
    pub fn is_list(self) -> bool {
        matches!(self, ConstructKind::ListSimple | ConstructKind::ListComplex)
    }

    /// Get a human-readable name for this construct.
    pub fn name(self) -> &'static str {
        match self {
            ConstructKind::RunTopLevel => "run (top-level)",
            ConstructKind::Try => "try",
            ConstructKind::Match => "match",
            ConstructKind::Recurse => "recurse",
            ConstructKind::Parallel => "parallel",
            ConstructKind::Spawn => "spawn",
            ConstructKind::Nursery => "nursery",
            ConstructKind::FunctionParams => "function params",
            ConstructKind::FunctionArgs => "function args",
            ConstructKind::GenericParams => "generic params",
            ConstructKind::WhereConstraints => "where constraints",
            ConstructKind::Capabilities => "capabilities",
            ConstructKind::StructFieldsDef => "struct fields (def)",
            ConstructKind::StructFieldsLiteral => "struct fields (literal)",
            ConstructKind::SumVariants => "sum variants",
            ConstructKind::MapEntries => "map entries",
            ConstructKind::TupleElements => "tuple elements",
            ConstructKind::ImportItems => "import items",
            ConstructKind::ListSimple => "list (simple)",
            ConstructKind::ListComplex => "list (complex)",
            ConstructKind::RunNested => "run (nested)",
            ConstructKind::MatchArms => "match arms",
        }
    }
}
