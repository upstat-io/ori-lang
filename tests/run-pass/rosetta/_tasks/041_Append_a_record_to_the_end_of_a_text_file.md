# Append a record to the end of a text file

**Problem:** Demonstrate appending records to a text file while maintaining data integrity.

**Requirements:**
- Create and write two initial password records in `/etc/passwd` format
- Close the file and reopen it for append operations
- Append a third record safely without overwriting existing data
- Read back the file to confirm the new record appears at the end
- Record format: `account:password:UID:GID:fullname,office,extension,homephone,email:directory:shell`

**Success Criteria:**
- File contains all three records after append operation
- Records are colon-separated with GECOS field comma-separated
- Example record: `xyz:x:1003:1000:X Yz,Room 1003,(234)555-8913,(234)555-0033,xyz@rosettacode.org:/home/xyz:/bin/bash`
