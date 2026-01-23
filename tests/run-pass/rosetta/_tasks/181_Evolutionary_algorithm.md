# Evolutionary algorithm

**Problem:** Starting with: Note: to aid comparison, try and ensure the variables and functions mentioned in the task description appear in solutions A cursory examination of a few of the solutions reveals that the instructions have not been followed rigorously in some solutions. Specifically, Note that some of the the solutions given retain characters in the mutated string that are correct in the target string.

**Requirements:**
- The target string: "METHINKS IT IS LIKE A WEASEL".
- An array of random characters chosen from the set of upper-case letters together with the space, and of the same length as the target string. (Call it the parent).
- A fitness function that computes the ‘closeness’ of its argument to the target string.
- While the parent is not yet the target:
- copy the parent C times, each time allowing some random probability that another character might be substituted using mutate.
- Assess the fitness of the parent and all the copies to the target and make the most fit string the new parent, discarding the others.
- repeat until the parent converges, (hopefully), to the target.
- While the parent is not yet the target:
- copy the parent C times, each time allowing some random probability that another character might be substituted using mutate.

**Success Criteria:**
- A mutate function that given a string and a mutation rate returns a copy of the string, with some characters probably mutated.
