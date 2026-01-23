# Roots of a quadratic function

**Problem:** Write a program to find the roots of a quadratic equation, i.e., solve the equation ax^2 + bx + c = 0. Your program must correctly handle non-real roots, but it need not check that a 0. The problem of solving a quadratic equation is a good example of how dangerous it can be to ignore the peculiarities of floating-point arithmetic. The obvious way to implement the quadratic formula suffers catastrophic loss of accuracy when one of the roots to be found is much closer to 0 than the other.

**Requirements:**
- Your program must correctly handle non-real roots, but it need not check that a 0.
- The obvious way to implement the quadratic formula suffers catastrophic loss of accuracy when one of the roots to be found is much closer to 0 than the other.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
