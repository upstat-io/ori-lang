fn @get_name(%0: Person [own]) -> str [entry: bb0]
  bb0:
    %1: Person [Aggregate] = %0
    %2: str [FatVal] = Project %1.0
    RcDec %1 [AggFields]
    Return %2
