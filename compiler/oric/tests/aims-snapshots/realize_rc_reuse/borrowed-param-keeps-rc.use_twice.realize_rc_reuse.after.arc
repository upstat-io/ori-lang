fn @use_twice(%0: str [own]) -> str [entry: bb0]
  bb0:
    %1: str [FatVal] = %0
    %2: () [Scalar] = Apply @ori_print(%1 [borrow])
    %3: () [Scalar] = ()
    %4: str [FatVal] = %0
    Return %4
