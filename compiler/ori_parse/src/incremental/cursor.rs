//! Cursor navigation for finding reusable declarations.

use ori_ir::incremental::ChangeMarker;
use ori_ir::{ExprArena, Module};

use super::decl::{collect_declarations, DeclRef};

/// Statistics for cursor navigation (debugging/tuning).
#[derive(Clone, Debug, Default)]
pub struct CursorStats {
    /// Total number of `find_at()` calls.
    pub lookups: u32,
    /// Declarations skipped during forward scan.
    pub skipped: u32,
    /// Declarations that could not be reused (intersected change).
    pub intersected: u32,
}

impl CursorStats {
    /// Total declarations examined.
    #[inline]
    pub fn total_examined(&self) -> u32 {
        self.skipped + self.intersected
    }
}

/// Navigator for finding reusable declarations in an old AST.
pub struct SyntaxCursor<'old> {
    module: &'old Module,
    arena: &'old ExprArena,
    marker: ChangeMarker,
    declarations: Vec<DeclRef>,
    current_index: usize,
    stats: CursorStats,
}

impl<'old> SyntaxCursor<'old> {
    /// Create a new cursor for navigating the old AST.
    pub fn new(module: &'old Module, arena: &'old ExprArena, marker: ChangeMarker) -> Self {
        let declarations = collect_declarations(module);
        SyntaxCursor {
            module,
            arena,
            marker,
            declarations,
            current_index: 0,
            stats: CursorStats::default(),
        }
    }

    /// Get the change marker.
    pub fn marker(&self) -> &ChangeMarker {
        &self.marker
    }

    /// Get reference to the old module.
    pub fn module(&self) -> &'old Module {
        self.module
    }

    /// Get reference to the old arena.
    pub fn arena(&self) -> &'old ExprArena {
        self.arena
    }

    /// Find a reusable declaration at or after the given position.
    ///
    /// Returns `Some(decl_ref)` if a declaration exists that:
    /// 1. Starts at or after `pos`
    /// 2. Does not intersect the affected region
    ///
    /// Returns `None` if no suitable declaration is found.
    pub fn find_at(&mut self, pos: u32) -> Option<DeclRef> {
        self.stats.lookups += 1;

        // Advance past declarations that end before pos
        while self.current_index < self.declarations.len() {
            let decl = self.declarations[self.current_index];
            if decl.span.end > pos {
                break;
            }
            self.stats.skipped += 1;
            self.current_index += 1;
        }

        if self.current_index >= self.declarations.len() {
            return None;
        }

        let decl = self.declarations[self.current_index];

        // Check if the declaration can be reused (doesn't intersect affected region)
        if self.marker.intersects(decl.span) {
            self.stats.intersected += 1;
            None
        } else {
            Some(decl)
        }
    }

    /// Advance the cursor past a declaration (after reusing it).
    pub fn advance(&mut self) {
        if self.current_index < self.declarations.len() {
            self.current_index += 1;
        }
    }

    /// Check if all declarations have been processed.
    pub fn is_exhausted(&self) -> bool {
        self.current_index >= self.declarations.len()
    }

    /// Get cursor navigation statistics.
    pub fn stats(&self) -> &CursorStats {
        &self.stats
    }

    /// Get the total number of declarations in the old AST.
    pub fn total_declarations(&self) -> usize {
        self.declarations.len()
    }
}
