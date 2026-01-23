# Accumulator factory

**Problem:** Create a function that returns an accumulator function (closure with mutable state).

**Requirements:**
- `foo(n)` returns accumulator function `g`
- `g(i)` adds `i` to running total and returns new sum
- Must work with both integers and floats
- State persists across calls, no global variables

**Example:**
```
x = foo(1)
x(5)     # returns 6
x(2.3)   # returns 8.3
```

**Success Criteria:**
- Demonstrates closures with mutable state
- Each accumulator is independent
- Handles numeric types correctly
