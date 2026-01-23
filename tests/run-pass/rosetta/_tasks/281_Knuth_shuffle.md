# Knuth shuffle

**Problem:** The Knuth shuffle (a.k.a. the Fisher-Yates shuffle) is an algorithm for randomly shuffling the elements of an array. Implement the Knuth shuffle for an integer array (or, if possible, an array of any type). Specification: Given an array items with indices ranging from 0 to last, the algorithm can be defined as follows (pseudo-code): for i from last downto 1 do: let j = random integer in range 0 j i swap items[i] with items[j] Test cases: {| class="wikitable" ! Input array ! Possible output array

**Requirements:**
- It modifies the input array in-place.
- The algorithm can also be amended to iterate from left to right, if that is more convenient.
- Sattolo cycle

**Success Criteria:**
- If that is unreasonable in your programming language, you may amend the algorithm to return the shuffled items as a new array instead.
