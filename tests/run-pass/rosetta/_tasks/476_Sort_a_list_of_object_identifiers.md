# Sort a list of object identifiers

**Problem:** Object identifiers (OID) are strings used to identify objects in network data. Show how to sort a list of OIDs, in their natural sort order. {| class="wikitable" ! Input (list of strings) ! Output (list of strings) 1.3.6.1.4.1.11.2.17.19.3.4.0.10 1.3.6.1.4.1.11.2.17.5.2.0.79 1.3.6.1.4.1.11.2.17.19.3.4.0.4 1.3.6.1.4.1.11150.3.4.0.1 1.3.6.1.4.1.11.2.17.19.3.4.0.1 1.3.6.1.4.1.11150.3.4.0 1.3.6.1.4.1.11.2.17.5.2.0.79 1.3.6.1.4.1.11.2.17.19.3.4.0.1 1.3.6.1.4.1.11.2.17.19.3.4.0.4 1.3.6.1.4.1.11.2.17.

**Requirements:**
- An OID consists of one or more non-negative integers in base 10, separated by dots. It starts and ends with a number.
- Their natural sort order is lexicographical with regard to the dot-separated fields, using numeric comparison between fields.
- Natural sorting
- Sort using a custom comparator

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
