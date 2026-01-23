# Averages/Median

**Problem:** Find the median value of a vector of floating-point numbers.

**Requirements:**
- For odd-length arrays: return the middle element when sorted
- For even-length arrays: return the average of the two middle elements
- May use sorting (O(n log n)) or selection algorithm (O(n))

**Success Criteria:**
- median([4.1, 5.6, 7.2, 1.7, 9.3, 4.4, 3.2]) = 4.4 (odd length)
- median([4.1, 7.2, 1.7, 9.3, 4.4, 3.2]) = 4.25 (even length)
- median([5.1, 2.6, 8.8, 4.6, 4.1]) = 4.6 (odd length)
