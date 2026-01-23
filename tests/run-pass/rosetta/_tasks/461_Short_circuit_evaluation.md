# Short-circuit evaluation

**Problem:** Assume functions a and b return boolean values, and further, the execution of function b takes considerable resources without side effects, and is to be minimized. If we needed to compute the conjunction (and): x = a() and b() Then it would be best to not compute the value of b() if the value of a() is computed as false, as the value of x can then only ever be false.

**Requirements:**
- If we needed to compute the conjunction (and): x = a() and b() Then it would be best to not compute the value of b() if the value of a() is computed as false, as the value of x can then only ever be false.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
