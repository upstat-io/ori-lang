# Generator/Exponential

**Problem:** A generator is an executable entity (like a function or procedure) that contains code that yields a sequence of values, one at a time, so that each time you call the generator, the next value in the sequence is provided. Generators are often built on top of coroutines or objects so that the internal state of the object is handled “naturally”.

**Requirements:**
- Use it to create a generator of:
- Squares.
- Cubes.
- Create a new generator that filters all cubes from the generator of squares.
- Generator

**Success Criteria:**
- Create a function that returns a generation of the m'th powers of the positive integers starting from zero, in order, and without obvious or simple upper limit. (Any upper limit to the generator should not be stated in the source but should be down to factors such as the languages natural integer size limit or computational time/size).
- Drop the first 20 values from this last generator of filtered results, and then show the next 10 values.
