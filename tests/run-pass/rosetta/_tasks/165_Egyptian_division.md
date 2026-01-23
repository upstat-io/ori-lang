# Egyptian division

**Problem:** Egyptian division is a method of dividing integers using addition and doubling that is similar to the algorithm of Ethiopian multiplication Algorithm: Given two numbers where the dividend is to be divided by the divisor: Start the construction of a table of two columns: powersf, and doublings; by a first row of a 1 (i.e.

**Requirements:**
- 2) in the first column and 1 times the divisor in the first row second column.
- Create the second row with columns of 2 (i.e 2), and 2 * divisor in order.
- Continue with successive i’th rows of 2 and 2 * divisor.
- Stop adding rows, and keep only those rows, where 2 * divisor is less than or equal to the dividend.
- We now assemble two separate sums that both start as zero, called here answer and accumulator
- Consider each row of the table, in the reverse order of its construction.
- If the current value of the accumulator added to the doublings cell would be less than or equal to the dividend then add it to the accumulator, as well as adding the powersf cell value to the answer.
- When the first row has been considered as above, then the integer division of dividend by divisor is given by answer. (And the remainder is given by the absolute value of accumulator - dividend).

**Success Criteria:**
- Task completed according to Rosetta Code specification
