import sys
import time

sys.setrecursionlimit(1000000)

def ack(m, n):
    if m == 0:
        return n + 1
    elif n == 0:
        return ack(m - 1, 1)
    else:
        return ack(m - 1, ack(m, n - 1))

# Warm up
ack(3, 5)

# Benchmark A(3,7)
start = time.perf_counter()
result = ack(3, 7)
elapsed = time.perf_counter() - start
print(f"Python 3.12: A(3,7) = {result}, {elapsed:.3f}s ({693964/elapsed:.0f} calls/sec, {elapsed/693964*1e6:.2f} µs/call)")

# Benchmark A(3,8)
start = time.perf_counter()
result = ack(3, 8)
elapsed = time.perf_counter() - start
print(f"Python 3.12: A(3,8) = {result}, {elapsed:.3f}s ({2785999/elapsed:.0f} calls/sec, {elapsed/2785999*1e6:.2f} µs/call)")
