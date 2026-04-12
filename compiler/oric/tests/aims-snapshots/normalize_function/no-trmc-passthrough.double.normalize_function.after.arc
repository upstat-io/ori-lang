fn @double(%0: int [own]) -> int [entry: bb0]
  bb0:
    %1: int [Scalar] = %0
    %2: int [Scalar] = 2
    %3: int [Scalar] = %1 * %2
    Return %3
