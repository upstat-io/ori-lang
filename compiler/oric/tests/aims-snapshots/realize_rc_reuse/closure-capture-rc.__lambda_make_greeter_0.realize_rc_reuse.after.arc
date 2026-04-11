fn @__lambda_make_greeter_0(%0: str [borrow]) -> () [entry: bb0]
  captures: 1
  bb0:
    %1: str [FatVal] = %0
    %2: () [Scalar] = Apply @ori_print(%1 [borrow])
    %3: () [Scalar] = ()
    Return %3
