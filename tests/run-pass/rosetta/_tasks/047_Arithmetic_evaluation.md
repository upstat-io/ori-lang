# Arithmetic evaluation

**Problem:** Parse and evaluate arithmetic expressions by building an abstract syntax tree (AST).

**Requirements:**
- Parse input string expressions like "(1+3)*7"
- Build an AST from the parsed input
- Evaluate by traversing the AST (no direct eval() allowed)
- Support operators: +, -, *, /
- Handle parentheses for precedence control
- Respect operator precedence: parentheses > multiplication/division > addition/subtraction

**Success Criteria:**
- `2 * -3 - -4 + -0.25` → -2.25
- `1 + 2 * (3 + (4 * 5 + 6 * 7 * 8) - 9) / 10` → 71
- `(1 + 2) * 10 / 100` → 0.3
