fn @create_and_discard() -> () [entry: bb0]
  bb0:
    %0: str [FatVal] = "temporary"
    %1: () [Scalar] = ()
    %2: str [FatVal] = %0
    %3: () [Scalar] = Apply @ori_print(%2 [own])
    %4: () [Scalar] = ()
    %5: () [Scalar] = ()
    Return %5
