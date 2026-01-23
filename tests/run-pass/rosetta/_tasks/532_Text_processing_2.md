# Text processing/2

**Problem:** The following task concerns data that came from a pollution monitoring station with twenty-four instruments monitoring twenty-four aspects of pollution in the air. Periodically a record is added to the file, each record being a line of 49 fields separated by white-space, which can be one or more space or tab characters. The fields (from the left) are: DATESTAMP [ VALUEn FLAGn ] * 24 i.e.

**Requirements:**
- Confirm the general field format of the file.
- Identify any DATESTAMPs that are duplicated.
- Report the number of records that have good readings for all instruments.

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
