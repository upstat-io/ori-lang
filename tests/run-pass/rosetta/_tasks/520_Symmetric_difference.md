# Symmetric difference

**Problem:** Given two sets A and B, compute (A B) (B A). That is, enumerate the items that are in A or B but not both. This set is called the symmetric difference of A and B. In other words: (A B) (A B) (the set of items that are in at least one of A or B minus the set of items that are in both A and B). Optionally, give the individual differences (A B and B A) as well. A = {John, Bob, Mary, Serena} B = {Jim, Mary, John, Bob}

**Requirements:**
- In the mathematical notation above A \ B gives the set of items in A that are not in B; A ∪ B gives the set of items in both A and B, (their union); and A ∩ B gives the set of items that are in both A and B (their intersection).

**Success Criteria:**
- If your code uses lists of items to represent sets then ensure duplicate items in lists are correctly handled. For example two lists representing sets of a = ["John", "Serena", "Bob", "Mary", "Serena"] and b = ["Jim", "Mary", "John", "Jim", "Bob"] should produce the result of just two strings: ["Serena", "Jim"], in any order.
