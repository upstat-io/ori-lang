fn @call_forward_list_int() -> [int] [entry: bb0]
  bb0:
    %0: int [Scalar] = 1
    %1: int [Scalar] = 2
    %2: int [Scalar] = 3
    %3: [int] [RcPtr] = Construct List(%0, %1, %2)
    %4: [int] [RcPtr] = Invoke @forward_list_int(%3 [own]) normal bb1 unwind bb2
  bb1:
    Return %4
  bb2:
    Resume
