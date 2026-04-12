fn @alias_test() -> () [entry: bb0]
  bb0:
    %0: str [FatVal] = "hello"
    %1: () [Scalar] = ()
    %2: str [FatVal] = %0
    %3: () [Scalar] = Apply @ori_print(%2 [own])
    %4: () [Scalar] = ()
    %5: str [FatVal] = %0
    %6: () [Scalar] = Apply @ori_print(%5 [own])
    %7: () [Scalar] = ()
    %8: () [Scalar] = ()
    Return %8
