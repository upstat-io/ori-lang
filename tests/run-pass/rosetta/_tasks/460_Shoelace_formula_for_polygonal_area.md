# Shoelace formula for polygonal area

**Problem:** Given the n + 1 vertices x[0], y[0] .. x[N], y[N] of a simple polygon described in a clockwise direction, then the polygon's area can be calculated by: abs( (sum(x[0]*y[1] + ... x[n-1]*y[n]) + x[N]*y[0]) - (sum(x[1]*y[0] + ... x[n]*y[n-1]) + x[0]*y[N]) (Where abs returns the absolute value) Write a function/method/routine to use the the Shoelace formula to calculate the area of the polygon described by the ordered points: (3,4), (5,11), (12,8), (9,5), and (5,6) Show the answer here, on this page

**Requirements:**
- x[n]*y[n-1]) + x[0]*y[N]) (Where abs returns the absolute value) Write a function/method/routine to use the the Shoelace formula to calculate the area of the polygon described by the ordered points: (3,4), (5,11), (12,8), (9,5), and (5,6) Show the answer here, on this page

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
