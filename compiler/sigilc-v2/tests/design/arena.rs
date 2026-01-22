//! Design Tests: Expression Arena
//!
//! These tests validate the arena allocation architecture from the design plan:
//! - ExprId(u32) indices instead of Box<Expr>
//! - ExprRange { start: u32, len: u16 } for expression lists
//! - O(1) allocation and lookup
//!
//! Reference: docs/compiler-design/v2/specs/A-data-structures.md

use sigilc_v2::syntax::{ExprArena, ExprId, ExprRange, Expr, ExprKind, Span};

/// Helper to create an int expression
fn int_expr(n: i64) -> Expr {
    Expr {
        kind: ExprKind::Int(n),
        span: Span::default(),
    }
}

// =============================================================================
// ExprId Design Contracts
// =============================================================================

/// Design: ExprId(u32) is a 32-bit handle
#[test]
fn design_exprid_is_32_bits() {
    assert_eq!(std::mem::size_of::<ExprId>(), 4);
}

/// Design: ExprId is Copy for efficient passing
#[test]
fn design_exprid_is_copy() {
    fn takes_copy<T: Copy>(_: T) {}

    let mut arena = ExprArena::new();
    let id = arena.alloc(int_expr(42));
    takes_copy(id);
    takes_copy(id); // Can use again
}

// =============================================================================
// ExprRange Design Contracts
// =============================================================================

/// Design: ExprRange uses u32 start + u16 len (6 bytes)
#[test]
fn design_expr_range_size() {
    // Design specifies { start: u32, len: u16 }
    // With padding this may be 8 bytes, but logically it's 6 bytes of data
    assert!(std::mem::size_of::<ExprRange>() <= 8);
}

/// Design: ExprRange can represent up to 65535 elements
#[test]
fn design_expr_range_capacity() {
    // Design: len: u16 means max 65535 elements
    let range = ExprRange::new(0, 65535);
    assert_eq!(range.len, 65535);
}

// =============================================================================
// Arena Allocation Design Contracts
// =============================================================================

/// Design: Arena allocation returns sequential IDs
#[test]
fn design_sequential_allocation() {
    let mut arena = ExprArena::new();

    let id1 = arena.alloc(int_expr(1));
    let id2 = arena.alloc(int_expr(2));
    let id3 = arena.alloc(int_expr(3));

    // IDs should be sequential (implementation detail, but validates design)
    assert!(id1.index() < id2.index());
    assert!(id2.index() < id3.index());
}

/// Design: Arena lookup is O(1)
#[test]
fn design_o1_lookup() {
    let mut arena = ExprArena::new();

    // Allocate many expressions
    let mut ids = Vec::new();
    for i in 0..1000 {
        ids.push(arena.alloc(int_expr(i)));
    }

    // All lookups should work (O(1) access)
    for (i, id) in ids.iter().enumerate() {
        let expr = arena.get(*id);
        // Verify it's the right value
        match &expr.kind {
            ExprKind::Int(n) => assert_eq!(*n, i as i64),
            _ => panic!("Expected Int"),
        }
    }
}

/// Design: List allocation returns contiguous range
#[test]
fn design_list_allocation() {
    let mut arena = ExprArena::new();

    let id1 = arena.alloc(int_expr(1));
    let id2 = arena.alloc(int_expr(2));
    let id3 = arena.alloc(int_expr(3));

    let range = arena.alloc_expr_list([id1, id2, id3]);

    // Range should cover all elements
    assert_eq!(range.len, 3);
}

/// Design: Arena can be reset for reuse
#[test]
fn design_arena_reset() {
    let mut arena = ExprArena::new();

    // Allocate some expressions
    arena.alloc(int_expr(1));
    arena.alloc(int_expr(2));
    arena.alloc(int_expr(3));

    // Reset
    arena.reset();

    // New allocations should start from beginning
    let id = arena.alloc(int_expr(42));
    assert_eq!(id.index(), 0);
}

/// Design: Arena handles empty lists
#[test]
fn design_empty_list() {
    let mut arena = ExprArena::new();

    let range = arena.alloc_expr_list([]);

    assert_eq!(range.len, 0);
}

// =============================================================================
// Memory Layout Design Contracts
// =============================================================================

/// Design: Arena uses indices, not pointers (cache-friendly)
#[test]
fn design_indices_not_pointers() {
    // This is validated by ExprId being u32, not *const Expr
    assert_eq!(std::mem::size_of::<ExprId>(), 4);

    // Pointer would be 8 bytes on 64-bit
    assert!(std::mem::size_of::<ExprId>() < std::mem::size_of::<*const ()>());
}

/// Design: Multiple arenas can coexist (thread-local per parser)
#[test]
fn design_multiple_arenas() {
    let mut arena1 = ExprArena::new();
    let mut arena2 = ExprArena::new();

    let id1 = arena1.alloc(int_expr(1));
    let id2 = arena2.alloc(int_expr(2));

    // Each arena has its own space - lookups work
    let expr1 = arena1.get(id1);
    let expr2 = arena2.get(id2);

    match (&expr1.kind, &expr2.kind) {
        (ExprKind::Int(1), ExprKind::Int(2)) => {}
        _ => panic!("Wrong values"),
    }
}
