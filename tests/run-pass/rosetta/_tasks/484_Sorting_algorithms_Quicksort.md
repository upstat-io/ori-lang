# Sorting algorithms/Quicksort

**Problem:** Sort an array (or list) elements using the quicksort algorithm. The elements must have a strict weak order and the index of the array can be of any discrete type. For languages where this is not possible, sort an array of integers. Quicksort, also known as partition-exchange sort, uses these steps. The best pivot creates partitions of equal length (or lengths differing by 1). The worst pivot creates an empty partition (for example, if the pivot is the first or last element of a sorted array).

**Requirements:**
- Choose any element of the array to be the pivot.
- Divide all other elements (except the pivot) into two partitions.
- All elements less than the pivot must be in the first partition.
- All elements greater than the pivot must be in the second partition.
- Use recursion to sort both partitions.
- Join the first sorted partition, the pivot, and the second sorted partition.
- Quicksort is a conquer-then-divide algorithm, which does most of the work during the partitioning and the recursive calls. The subsequent reassembly of the sorted partitions involves trivial effort.
- Merge sort is a divide-then-conquer algorithm. The partioning happens in a trivial way, by splitting the input array in half. Most of the work happens during the recursive calls and the merge phase.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
