fn @get_name(%0: Person [borrow]) -> str [entry: bb0]
  bb0:
    %1: Person [Aggregate] = %0
    %2: str [FatVal] = Project %1.0
    RcInc %2 [FatPtr]
    Return %2
