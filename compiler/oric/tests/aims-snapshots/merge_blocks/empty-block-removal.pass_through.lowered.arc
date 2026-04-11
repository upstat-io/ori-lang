fn @pass_through(%0: int [own]) -> int [entry: bb0]
  bb0:
    %1: int [Scalar] = %0
    Return %1
