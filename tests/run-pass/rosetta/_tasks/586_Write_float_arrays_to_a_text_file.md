# Write float arrays to a text file

**Problem:** Write two equal-sized numerical arrays 'x' and 'y' to a two-column text file named 'filename'. The first column of the file contains values from an 'x'-array with a given 'xprecision', the second -- values from 'y'-array with 'yprecision'. For example, considering: x = {1, 2, 3, 1e11}; y = {1, 1.4142135623730951, 1.7320508075688772, 316227.76601683791}; /* sqrt(x) */ xprecision = 3; yprecision = 5; The file should look like: 1e+011 3.

**Requirements:**
- For example, considering: x = {1, 2, 3, 1e11}; y = {1, 1.4142135623730951, 1.7320508075688772, 316227.76601683791}; /* sqrt(x) */ xprecision = 3; yprecision = 5; The file should look like: 1e+011 3.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
