fn @sum_list(%0: [int] [own]) -> int [entry: bb0]
  bb0:
    %1: [int] [RcPtr] = %0
    %2: int [Scalar] = 0
    %3: (int, int) -> int [FatVal] = PartialApply @__lambda_sum_list_0()
    %4: int [Scalar] = Invoke @fold(%1 [own], %2 [own], %3 [own]) normal bb1 unwind bb2
  bb1:
    Return %4
  bb2:
    Resume
