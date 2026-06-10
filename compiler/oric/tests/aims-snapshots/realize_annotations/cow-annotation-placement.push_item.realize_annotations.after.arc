fn @push_item(%0: [int] [borrow], %1: int [own]) -> [int] [entry: bb0]
  bb0:
    %2: [int] [RcPtr] = %0
    %3: int [Scalar] = %1
    RcInc %2 [HeapPtr]
    %4: [int] [RcPtr] = Invoke @push(%2 [own], %3 [own]) normal bb1 unwind bb2
  bb1:
    RcDec %2 [HeapPtr]
    Return %4
  bb2:
    RcDec %2 [HeapPtr]
    Resume
