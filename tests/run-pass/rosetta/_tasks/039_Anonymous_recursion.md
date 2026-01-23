# Anonymous recursion

**Problem:** Demonstrate anonymous recursion - enabling a function to call itself without using its own name.

**Requirements:**
- Implement a recursive Fibonacci function
- Check for negative arguments before recursion begins
- Use anonymous recursion techniques (via labels, local functions, Y combinators, or closures)
- Avoid creating separate named helper functions that pollute the namespace

**Success Criteria:**
- Calculate Fibonacci numbers for range 0-20
- Handle negative inputs gracefully
- fib(0) = 0, fib(1) = 1, fib(10) = 55, fib(20) = 6765
