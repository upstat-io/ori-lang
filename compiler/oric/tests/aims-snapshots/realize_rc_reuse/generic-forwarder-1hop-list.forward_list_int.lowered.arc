fn @forward_list_int(%0: [int] [own]) -> [int] [entry: bb0]
  bb0:
    %1: [int] [RcPtr] = %0
    Return %1
