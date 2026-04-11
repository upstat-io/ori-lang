fn @count_entries(%0: {str: int} [own]) -> int [entry: bb0]
  bb0:
    %1: {str: int} [RcPtr] = %0
    %2: int [Scalar] = Invoke @len(%1 [own]) normal bb1 unwind bb2
  bb1:
    Return %2
  bb2:
    Resume
