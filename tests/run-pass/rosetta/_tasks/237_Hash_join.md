# Hash join

**Problem:** An inner join is an operation that combines two data tables into one table, based on matching column values. The simplest way of implementing this operation is the nested loop join algorithm, but a more scalable alternative is the hash join algorithm. Implement the "hash join" algorithm, and demonstrate that it passes the test-case listed below. You should represent the tables as data structures that feel natural in your programming language.

**Requirements:**
- Hash phase: Create a multimap from one of the two tables, mapping from each join column value to all the rows that contain it.
- The multimap must support hash-based lookup which scales better than a simple linear search, because that's the whole point of this algorithm.
- Join phase: Scan the other table, and find matching rows by looking in the multimap created before.

**Success Criteria:**
- Ideally we should create the multimap for the smaller table, thus minimizing its creation time and memory size.
