# Largest int from concatenated ints

**Problem:** Given a set of positive integers, write a function to order the integers in such a way that the concatenation of the numbers forms the largest possible integer and return this integer. Use the following two sets of integers as tests and show your program output here. Possible algorithms:

**Requirements:**
- {1, 34, 3, 98, 9, 76, 45, 4}
- {54, 546, 548, 60}
- Another way to solve this is to note that in the best arrangement, for any two adjacent original integers X and Y, the concatenation X followed by Y will be numerically greater than or equal to the concatenation Y followed by X.
- Yet another way to solve this is to pad the integers to the same size by repeating the digits then sort using these repeated integers as a sort key.
- Algorithms: What is the most efficient way to arrange the given numbers to form the biggest number?
- Constructing the largest number possible by rearranging a list

**Success Criteria:**
- A solution could be found by trying all combinations and return the best.
