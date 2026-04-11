fn @count_entries(%0: {str: int} [own]) -> int [entry: bb0]
  bb0:
    %1: {str: int} [RcPtr] = %0
    %2: int [Scalar] = Invoke @len(%1 [borrow]) normal bb1 unwind bb2
  bb1:
    RcDec %1 [HeapPtr]
    Return %2
  bb2:
    RcDec %1 [HeapPtr]
    Resume
