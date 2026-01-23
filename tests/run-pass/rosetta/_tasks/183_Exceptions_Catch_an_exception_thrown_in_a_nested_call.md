# Exceptions/Catch an exception thrown in a nested call

**Problem:** Show how to create a user-defined exception and show how to catch an exception raised from several nested calls away. Show/describe what happens when the program is run.

**Requirements:**
- Create two user-defined exceptions, U0 and U1.
- Have function foo call function bar twice.
- Have function bar call function baz.
- Arrange for function baz to raise, or throw exception U0 on its first call, then exception U1 on its second.

**Success Criteria:**
- Function foo should catch only exception U0, not U1.
