import sys
import time

sys.setrecursionlimit(10000000)

def ack(m, n):
    if m == 0:
        return n + 1
    elif n == 0:
        return ack(m - 1, 1)
    else:
        return ack(m - 1, ack(m, n - 1))

start = time.perf_counter()
result = ack(4, 1)
elapsed = time.perf_counter() - start
print(f"Python 3.12: A(4,1) = {result}, {elapsed:.3f}s")
