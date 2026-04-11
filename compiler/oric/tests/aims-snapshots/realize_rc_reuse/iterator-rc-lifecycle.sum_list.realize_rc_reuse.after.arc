fn @sum_list(%0: [int] [own]) -> int [entry: bb0]
  bb0:
    %1: [int] [RcPtr] = %0
    %2: int [Scalar] = 0
    %3: (int, int) -> int [FatVal] = PartialApply @__lambda_sum_list_0()
    %4: int [Scalar] = Invoke @fold(%1 [borrow], %2 [borrow], %3 [borrow]) normal bb1 unwind bb2
  bb1:
    RcDec %1 [HeapPtr]
    RcDec %3 [Closure]
    Return %4
  bb2:
    RcDec %1 [HeapPtr]
    RcDec %3 [Closure]
    Resume
