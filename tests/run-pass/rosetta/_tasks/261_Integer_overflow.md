# Integer overflow

**Problem:** Some languages support one or more integer types of the underlying processor. This integer types have fixed size; usually 8-bit, 16-bit, 32-bit, or 64-bit. The integers supported by such a type can be signed or unsigned. Arithmetic for machine level integers can often be done by single CPU instructions. This allows high performance and is the main reason to support machine level integers.

**Requirements:**
- When the integer overflow does trigger an exception show how the exception is caught.
- When the integer overflow produces some value, print it.
- It is okay to mention, when a language supports unlimited precision integers, but this task is NOT the place to demonstrate the capabilities of unlimited precision integers.

**Success Criteria:**
- It should be explicitly noted when an integer overflow is not recognized, the program continues with wrong results.
- This should be done for signed and unsigned integers of various sizes supported by the computer programming language.
- When a language has no fixed size integer type, or when no integer overflow can occur for other reasons, this should be noted.
