# Ackermann function

**Problem:** Implement the Ackermann function (classic non-primitive recursive function).

**Definition:**
```
A(m, n) = n + 1                  if m = 0
A(m, n) = A(m-1, 1)              if m > 0, n = 0
A(m, n) = A(m-1, A(m, n-1))      if m > 0, n > 0
```

**Requirements:**
- Handle non-negative integer arguments
- Arbitrary precision preferred (grows very quickly)

**Success Criteria:**
- A(3, 4) = 125
- A(4, 1) = 65533
- A(3, 5) = 253
