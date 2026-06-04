fn @use_twice(%0: str [own]) -> str [entry: bb0]
  bb0:
    burden_inc %1
    RcInc %0 [FatPtr]
    %1: str [FatVal] = %0
    %2: () [Scalar] = Apply @ori_print(%1 [borrow])
    RcDec %1 [FatPtr]
    burden_dec %1
    %3: () [Scalar] = ()
    %4: str [FatVal] = %0
    Return %4
