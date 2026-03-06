//! IR Quality Tests: Loop Optimizations
//!
//! Verify range specialization (single icmp), compound assignment CSE,
//! and loop-invariant block param elimination.

use crate::util::{compile_and_capture_ir, extract_function_ir};

// Range specialization

/// Ascending exclusive range (`0..n`): single `icmp slt` in header.
///
/// With step=1 and inclusive=0 known at compile time, the general
/// 8-instruction boolean condition reduces to a single comparison.
#[test]
fn test_range_ascending_exclusive_single_icmp() {
    let ir = compile_and_capture_ir(
        r"
@count_up (n: int) -> int = {
    let count = 0;
    for i in 0..n do {
        count += 1
    };
    count
}

@main () -> int = count_up(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_count_up");

    // Header block: exactly 1 icmp (the `slt` condition), not 8 boolean ops.
    // Find the header by looking for the block with phi + icmp + br pattern.
    let header = find_loop_header(fn_ir);
    let icmp_count = header.matches("icmp").count();
    assert_eq!(
        icmp_count, 1,
        "expected exactly 1 icmp in specialized header (step=1, excl), got {icmp_count}.\n\
         Header:\n{header}"
    );
    assert!(
        header.contains("icmp slt"),
        "expected `icmp slt` for ascending exclusive range.\nHeader:\n{header}"
    );

    // No zero-step guard: step=1 is known non-zero.
    assert!(
        !fn_ir.contains("range step cannot be zero"),
        "expected no zero-step guard for literal step=1.\nIR:\n{fn_ir}"
    );
}

/// Ascending inclusive range (`0..=n`): single `icmp sle` in header.
#[test]
fn test_range_ascending_inclusive_single_icmp() {
    let ir = compile_and_capture_ir(
        r"
@count_incl (n: int) -> int = {
    let count = 0;
    for i in 0..=n do {
        count += 1
    };
    count
}

@main () -> int = count_incl(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_count_incl");
    let header = find_loop_header(fn_ir);
    let icmp_count = header.matches("icmp").count();
    assert_eq!(
        icmp_count, 1,
        "expected exactly 1 icmp in specialized header (step=1, incl), got {icmp_count}.\n\
         Header:\n{header}"
    );
    assert!(
        header.contains("icmp sle"),
        "expected `icmp sle` for ascending inclusive range.\nHeader:\n{header}"
    );
}

/// Descending exclusive range (`n..0 by -1`): single `icmp sgt` in header.
#[test]
fn test_range_descending_exclusive_single_icmp() {
    let ir = compile_and_capture_ir(
        r"
@count_down (n: int) -> int = {
    let count = 0;
    for i in n..0 by -1 do {
        count += 1
    };
    count
}

@main () -> int = count_down(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_count_down");
    let header = find_loop_header(fn_ir);
    let icmp_count = header.matches("icmp").count();
    assert_eq!(
        icmp_count, 1,
        "expected exactly 1 icmp in specialized header (step=-1, excl), got {icmp_count}.\n\
         Header:\n{header}"
    );
    assert!(
        header.contains("icmp sgt"),
        "expected `icmp sgt` for descending exclusive range.\nHeader:\n{header}"
    );
}

/// Descending inclusive range (`n..=0 by -1`): single `icmp sge` in header.
#[test]
fn test_range_descending_inclusive_single_icmp() {
    let ir = compile_and_capture_ir(
        r"
@count_down_incl (n: int) -> int = {
    let count = 0;
    for i in n..=0 by -1 do {
        count += 1
    };
    count
}

@main () -> int = count_down_incl(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_count_down_incl");
    let header = find_loop_header(fn_ir);
    let icmp_count = header.matches("icmp").count();
    assert_eq!(
        icmp_count, 1,
        "expected exactly 1 icmp in specialized header (step=-1, incl), got {icmp_count}.\n\
         Header:\n{header}"
    );
    assert!(
        header.contains("icmp sge"),
        "expected `icmp sge` for descending inclusive range.\nHeader:\n{header}"
    );
}

/// Variable step (`0..n by s`): falls back to general 8-instruction condition.
#[test]
fn test_range_variable_step_general_condition() {
    let ir = compile_and_capture_ir(
        r"
@count_step (n: int, s: int) -> int = {
    let count = 0;
    for i in 0..n by s do {
        count += 1
    };
    count
}

@main () -> int = count_step(n: 10, s: 2);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_count_step");
    let header = find_loop_header(fn_ir);
    let icmp_count = header.matches("icmp").count();

    // General path: 6 icmp instructions (step>0, step<0, incl>0, i<end, i>end, i==end).
    assert!(
        icmp_count >= 4,
        "expected >= 4 icmp in general header (variable step), got {icmp_count}.\n\
         Header:\n{header}"
    );

    // Zero-step guard should be present.
    assert!(
        fn_ir.contains("range step cannot be zero")
            || fn_ir.contains("ori_panic_cstr")
            || fn_ir.contains("ori_panic"),
        "expected zero-step guard for variable step.\nIR:\n{fn_ir}"
    );
}

// Compound assignment CSE

/// `total += i + 1; i += 1` computes `i + 1` exactly once per iteration.
///
/// The compound assignment desugaring produces two identical `PrimOp::Binary(Add)`
/// operations on the same operands (`i` and `1`). The CSE cache in
/// `emit_checked_binop` should detect the duplicate and reuse the result.
#[test]
fn test_cse_loop_duplicate_add_eliminated() {
    let ir = compile_and_capture_ir(
        r"
@sum_loop (n: int) -> int = {
    let i = 0;
    let total = 0;
    loop {
        if i >= n then break total;
        total += i + 1;
        i += 1
    }
}

@main () -> int = sum_loop(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_sum_loop");

    // Count `@llvm.sadd.with.overflow.i64` calls in the function body.
    // Before CSE: 3 (i+1 for total, total+(i+1), i+1 for i).
    // After CSE: 2 (i+1 once, total+(i+1) once — the second i+1 is reused).
    let sadd_count = fn_ir.matches("@llvm.sadd.with.overflow.i64").count();
    assert_eq!(
        sadd_count, 2,
        "expected exactly 2 sadd.with.overflow calls (CSE should eliminate duplicate i+1), \
         but found {sadd_count}.\nIR:\n{fn_ir}"
    );
}

/// CSE should not eliminate operations with different operands.
///
/// `a + b` and `a + c` (where b != c) must both be emitted, even within
/// the same ARC block.
#[test]
fn test_cse_different_operands_not_eliminated() {
    let ir = compile_and_capture_ir(
        r"
@three_adds (a: int, b: int, c: int) -> int = {
    let x = a + b;
    let y = a + c;
    let z = b + c;
    x + y + z
}

@main () -> int = three_adds(a: 1, b: 2, c: 3);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_three_adds");

    // All 5 additions are distinct: a+b, a+c, b+c, (a+b)+(a+c), result+(b+c).
    // Each should produce its own sadd.with.overflow call.
    let sadd_count = fn_ir.matches("@llvm.sadd.with.overflow.i64").count();
    assert_eq!(
        sadd_count, 5,
        "expected 5 sadd.with.overflow calls (all operands distinct), \
         but found {sadd_count}.\nIR:\n{fn_ir}"
    );
}

/// CSE across different intrinsics: `a + b` and `a - b` should NOT be CSE'd.
///
/// Even though the operands are identical, the intrinsic names differ
/// (sadd vs ssub), so they must not be merged.
#[test]
fn test_cse_different_intrinsics_not_merged() {
    let ir = compile_and_capture_ir(
        r"
@add_and_sub (a: int, b: int) -> int = {
    let sum = a + b;
    let diff = a - b;
    sum + diff
}

@main () -> int = add_and_sub(a: 10, b: 3);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_add_and_sub");

    // Should have sadd (a+b), ssub (a-b), and sadd (sum+diff) = 2 sadd + 1 ssub.
    let sadd_count = fn_ir.matches("@llvm.sadd.with.overflow.i64").count();
    let ssub_count = fn_ir.matches("@llvm.ssub.with.overflow.i64").count();
    assert_eq!(
        sadd_count, 2,
        "expected 2 sadd.with.overflow calls, but found {sadd_count}.\nIR:\n{fn_ir}"
    );
    assert_eq!(
        ssub_count, 1,
        "expected 1 ssub.with.overflow call, but found {ssub_count}.\nIR:\n{fn_ir}"
    );
}

/// CSE with identical constant operands: `x + 1` computed twice should be CSE'd.
///
/// Tests that the `CseOperand` normalization works for constants — two
/// separate `const_i64(1)` calls should match in the cache.
#[test]
fn test_cse_identical_constant_operands() {
    let ir = compile_and_capture_ir(
        r"
@inc_twice (x: int) -> int = {
    let a = x + 1;
    let b = x + 1;
    a + b
}

@main () -> int = inc_twice(x: 5);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_inc_twice");

    // `x + 1` should be computed once, `a + b` once = 2 total sadd calls.
    let sadd_count = fn_ir.matches("@llvm.sadd.with.overflow.i64").count();
    assert_eq!(
        sadd_count, 2,
        "expected 2 sadd.with.overflow calls (x+1 CSE'd, then a+b), \
         but found {sadd_count}.\nIR:\n{fn_ir}"
    );
}

// Loop-invariant block param elimination

/// A mutable binding defined before a loop but never modified inside the
/// loop should NOT produce a phi node in the loop header.
///
/// `limit` is defined before the loop as `let limit = m`. Inside the loop
/// body, `limit` is read (`total += limit`) but never assigned. The ARC
/// lowerer used to carry it as a header block param → invariant LLVM phi.
/// Phase 7 of `block_merge` eliminates the invariant param, so the LLVM IR
/// should use the function parameter `%1` directly.
#[test]
fn test_loop_invariant_binding_no_phi() {
    let ir = compile_and_capture_ir(
        r"
@use_loop (n: int, m: int) -> int = {
    let total = 0;
    let limit = m;
    let i = 0;
    loop {
        if i >= n then break total;
        total += limit;
        i += 1
    }
}

@main () -> int = use_loop(n: 5, m: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_use_loop");
    let header = find_loop_header(fn_ir);

    // Count phi nodes in the header. Should have exactly 2: total and i.
    // The invariant `limit` binding should NOT have a phi.
    let phi_count = header.matches("= phi ").count();
    assert_eq!(
        phi_count, 2,
        "expected 2 phi nodes (total, i) — invariant `limit` should not have one.\n\
         Header:\n{header}"
    );
}

/// When a pre-loop binding IS modified inside the loop, it must keep its phi.
///
/// Both `total` and `i` are modified inside the loop body, so they must
/// retain their phi nodes.
#[test]
fn test_loop_modified_binding_keeps_phi() {
    let ir = compile_and_capture_ir(
        r"
@sum_to (n: int) -> int = {
    let total = 0;
    let i = 0;
    loop {
        if i >= n then break total;
        total += i;
        i += 1
    }
}

@main () -> int = sum_to(n: 10);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_sum_to");
    let header = find_loop_header(fn_ir);

    // Both total and i are modified — both need phi nodes.
    let phi_count = header.matches("= phi ").count();
    assert_eq!(
        phi_count, 2,
        "expected 2 phi nodes (total and i both modified).\nHeader:\n{header}"
    );
}

/// Multiple invariant bindings in a loop — all eliminated.
///
/// `a`, `b`, and `c` are defined before the loop and never modified.
/// Only `total` and `i` are modified inside the loop.
#[test]
fn test_multiple_invariant_bindings_no_phi() {
    let ir = compile_and_capture_ir(
        r"
@multi_inv (n: int, a: int, b: int, c: int) -> int = {
    let total = 0;
    let x = a;
    let y = b;
    let z = c;
    let i = 0;
    loop {
        if i >= n then break total;
        total += x + y + z;
        i += 1
    }
}

@main () -> int = multi_inv(n: 3, a: 1, b: 2, c: 3);
",
    );

    if !ir.contains("define ") {
        eprintln!("skipping: release binary does not emit IR");
        return;
    }

    let fn_ir = extract_function_ir(&ir, "_ori_multi_inv");
    let header = find_loop_header(fn_ir);

    // Only total and i need phi nodes. x, y, z are invariant.
    let phi_count = header.matches("= phi ").count();
    assert_eq!(
        phi_count, 2,
        "expected 2 phi nodes (total, i) — x, y, z should be eliminated.\n\
         Header:\n{header}"
    );
}

// Helpers

/// Find the loop header block in LLVM IR (block with phi nodes + conditional branch).
fn find_loop_header(fn_ir: &str) -> String {
    let mut in_header = false;
    let mut header_lines = Vec::new();

    for line in fn_ir.lines() {
        let trimmed = line.trim();
        // Start of a new block
        if trimmed.ends_with(':') || (trimmed.contains(':') && trimmed.contains("preds")) {
            if in_header {
                break; // We were in the header, now hit the next block
            }
            // Check if this block has phi nodes (next lines)
            in_header = false;
            header_lines.clear();
            header_lines.push(line.to_string());
            continue;
        }
        if in_header
            || (!header_lines.is_empty() && trimmed.starts_with('%') && trimmed.contains("= phi "))
        {
            in_header = true;
            header_lines.push(line.to_string());
        } else if !in_header && !header_lines.is_empty() {
            // This block didn't start with phi — not the header
            header_lines.clear();
        }
    }

    header_lines.join("\n")
}
