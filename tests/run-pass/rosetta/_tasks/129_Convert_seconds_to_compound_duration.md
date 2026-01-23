# Convert seconds to compound duration

**Problem:** Task Write a function or program which: takes a positive integer representing a duration in seconds as input (e.g., 100), and returns a string which shows the same duration decomposed into: weeks, days, hours, minutes, and seconds. This is detailed below (e.g., "2 hr, 59 sec"). Demonstrate that it passes the following three test-cases: Test Cases Details The following five units should be used: However, only include quantities with non-zero values in the output (e.g., return "1 d" and not "0 wk, 1 d, 0 hr, 0 min, 0 sec").

**Requirements:**
- Give larger units precedence over smaller ones as much as possible (e.g., return 2 min, 10 sec and not 1 min, 70 sec or 130 sec)
- Mimic the formatting shown in the test-cases (quantities sorted from largest unit to smallest and separated by comma+space
- value and unit of each quantity separated by space).

**Success Criteria:**
- Task completed according to Rosetta Code specification
