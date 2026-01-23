# Tree datastructures

**Problem:** The following shows a tree of data with nesting denoted by visual levels of indentation: RosettaCode A common datastructure for trees is to define node structures having a name and a, (possibly empty), list of child nodes. The nesting of nodes captures the indentation of the tree. Lets call this the nest form. RosettaCode(rocks(code, ...), ...) Another datastructure for trees is to construct from the root an ordered list of the nodes level of indentation and the name of that node.

**Requirements:**
- E.g. if child nodes are surrounded by brackets
- and separated by commas then:
- But only an _example_
- Create/use a nest datastructure format and textual representation for arbitrary trees.
- Create/use an indent datastructure format and textual representation for arbitrary trees.
- Create methods/classes/proceedures/routines etc to:
- Change from a nest tree datastructure to an indent one.
- Change from an indent tree datastructure to a nest one
- Use the above to encode the example at the start into the nest format, and show it.
- transform the initial nest format to indent format and show it.

**Success Criteria:**
- Compare initial and final nest formats which should be the same.
- Comparing nested datastructures is secondary - saving formatted output as a string then a string compare would suffice for this task, if its easier.
