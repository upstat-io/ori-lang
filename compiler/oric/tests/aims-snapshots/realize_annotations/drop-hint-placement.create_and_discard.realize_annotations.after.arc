fn @create_and_discard() -> () [entry: bb0]
  bb0:
    burden_inc %0
    %0: str [FatVal] = "temporary"
    %1: () [Scalar] = ()
    %2: str [FatVal] = %0
    burden_dec %0
    %3: () [Scalar] = Apply @ori_print(%2 [borrow])
    %4: () [Scalar] = ()
    %5: () [Scalar] = ()
    RcDec %2 [FatPtr]
    Return %5
