# Perfect shuffle

**Problem:** A perfect shuffle (or faro/weave shuffle) means splitting a deck of cards into equal halves, and perfectly interleaving them - so that you end up with the first card from the left half, followed by the first card from the right half, and so on: 7♠ 8♠ 9♠ J♠ Q♠ K♠ 7♠ 8♠ 9♠ J♠ Q♠ K♠ 7♠ J♠ 8♠ Q♠ 9♠ K♠ When you repeatedly perform perfect shuffles on an even-sized deck of unique cards, it will at some point arrive back at its original order.

**Requirements:**
- Write a function that can perform a perfect shuffle on an even-sized list of values.
- Call this function repeatedly to count how many shuffles are needed to get a deck back to its original order, for each of the deck sizes listed under "Test Cases" below.
- You can use a list of numbers (or anything else that's convenient) to represent a deck; just make sure that all "cards" are unique within each deck.

**Success Criteria:**
- Print out the resulting shuffle counts, to demonstrate that your program passes the test-cases.
