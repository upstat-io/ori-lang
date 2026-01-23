# Atomic updates

**Problem:** Create a thread-safe data structure with buckets containing integers where the total sum is preserved.

**Requirements:**
- Manage a fixed number of buckets with nonnegative integer values
- Retrieve any bucket's current value
- Transfer amounts between buckets (clamping to keep values non-negative)
- Run three concurrent tasks:
  1. Equalization: make two buckets closer to equal
  2. Redistribution: arbitrarily redistribute values between two buckets
  3. Monitoring: display total and individual bucket values
- Ensure transfers are atomic to preserve sum invariant

**Success Criteria:**
- Total sum remains constant despite concurrent modifications
- No bucket value becomes negative
- No deadlocks occur
