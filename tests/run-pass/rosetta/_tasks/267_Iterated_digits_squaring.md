# Iterated digits squaring

**Problem:** If you add the square of the digits of a Natural number (an integer bigger than zero), you always end with either 1 or 89: 15 -> 26 -> 40 -> 16 -> 37 -> 58 -> 89 7 -> 49 -> 97 -> 130 -> 10 -> 1 An example in Python: >>> step = lambda x: sum(int(d) ** 2 for d in str(x)) >>> iterate = lambda x: x if x in [1, 89] else iterate(step(x)) >>> [iterate(x) for x in xrange(1, 20)] [1, 89, 89, 89, 89, 89, 1, 89, 89, 1, 89, 89, 1, 89, 89, 89, 89, 89, 1] Count how many number chains for integers 1

**Requirements:**
- Implement the task according to the specification

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
