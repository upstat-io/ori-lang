# Echo server

**Problem:** Create a network service that sits on TCP port 12321, which accepts connections on that port, and which echoes complete lines (using a carriage-return/line-feed sequence as line separator) back to clients. No error handling is required. For the purposes of testing, it is only necessary to support connections from localhost (127.0.0.1 or perhaps ::1).

**Requirements:**
- Logging of connection information to standard output is recommended.
- The implementation must not stop responding to other clients if one client sends a partial line or stops reading responses.

**Success Criteria:**
- Task completed according to Rosetta Code specification
