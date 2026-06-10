fn @sum_list(%0: [int] [borrow]) -> int [entry: bb0]
  bb0:
    %1: [int] [RcPtr] = %0
    %2: int [Scalar] = 0
    %3: (int, int) -> int [FatVal] = PartialApply @__lambda_sum_list_0()
    RcDec %3 [Closure]
    %4: int [Scalar] = Invoke @fold(%1 [borrow], %2 [borrow], %3 [borrow]) normal bb1 unwind bb2
  bb1:
    Return %4
  bb2:
    Resume
