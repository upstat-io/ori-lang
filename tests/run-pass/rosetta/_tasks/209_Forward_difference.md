# Forward difference

**Problem:** Provide code that produces a list of numbers which is the nth order forward difference, given a non-negative integer (specifying the order) and a list of numbers. The first-order forward difference of a list of numbers A is a new list B, where Bn = An+1 - An. List B should have one fewer element as a result. The second-order forward difference of A will be: tdefmodule Diff do def forward(arr,i\\1) do forward(arr,[],i) def forward([_|[]],diffs,i) do if i == 1 do IO.

**Requirements:**
- Iterate through all previous forward differences and re-calculate a new array each time.
- Use this formula (from Wikipedia):

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
