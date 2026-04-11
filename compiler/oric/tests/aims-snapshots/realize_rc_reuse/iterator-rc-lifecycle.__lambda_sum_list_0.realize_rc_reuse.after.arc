fn @__lambda_sum_list_0(%0: int [own], %1: int [own]) -> int [entry: bb0]
  bb0:
    %2: int [Scalar] = %0
    %3: int [Scalar] = %1
    %4: int [Scalar] = %2 + %3
    Return %4
