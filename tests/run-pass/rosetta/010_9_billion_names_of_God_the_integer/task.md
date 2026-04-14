# 9 billion names of God the integer

**Problem:** Generate a number triangle representing integer partitions.

**Requirements:**
- Display first 25 rows of the partition triangle
- Row n shows partition counts beginning with each possible value
- Implement G(n) returning sum of n-th row (equals partition function P(n))
- Compute G() for: 23, 123, 1234, 12345

**Triangle structure (first rows):**
```
1
1 1
1 1 1
1 2 1 1
1 2 2 1 1
1 3 3 2 1 1
```

**Success Criteria:**
- Accurate 25-row triangle
- Correct row sums matching partition function
- Handle large integers (results exceed 64-bit for larger inputs)
