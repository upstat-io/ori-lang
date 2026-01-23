# Delegates

**Problem:** A delegate is a helper object used by another object. The delegator may send the delegate certain messages, and provide a default implementation when there is no delegate or the delegate does not respond to a message. This pattern is heavily used in Cocoa framework on Mac OS X. See also wp:Delegation pattern.

**Requirements:**
- Objects responsibilities:
- Keep an optional delegate instance.
- Implement "operation" method, returning the delegate "thing" if the delegate respond to "thing", or the string "default implementation".
- Implement "thing" and return the string "delegate implementation"
- Show how objects are created and used. First, without a delegate, then with a delegate that does not implement "thing", and last with a delegate that implements "thing".

**Success Criteria:**
- Task completed according to Rosetta Code specification
