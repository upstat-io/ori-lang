# Validate International Securities Identification Number

**Problem:** An International Securities Identification Number (ISIN) is a unique international identifier for a financial security such as a stock or bond. Write a function or program that takes a string as input, and checks whether it is a valid ISIN. It is only valid if it has the correct format, and the embedded checksum is correct. Demonstrate that your code passes the test-cases listed below.

**Requirements:**
- Replace letters with digits, by converting each character from base 36 to base 10, e.g. AU0000XVGZA3 1030000033311635103.
- Perform the Luhn test on this base-10 number.There is a separate task for this test: Luhn test of credit card numbers.You don't have to replicate the implementation of this test here ─── you can just call the existing function from that task. (Add a comment stating if you did this.)
- Luhn test of credit card numbers
- Interactive online ISIN validator

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
