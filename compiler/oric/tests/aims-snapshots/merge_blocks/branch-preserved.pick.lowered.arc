fn @pick(%0: bool [own]) -> int [entry: bb0]
  bb0:
    %1: bool [Scalar] = %0
    Branch %1 ? bb1 : bb2
  bb1:
    %2: int [Scalar] = 1
    Jump bb3(%2)
  bb2:
    %3: int [Scalar] = 2
    Jump bb3(%3)
  bb3: (%4: int)
    Return %4
